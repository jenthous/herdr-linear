//! 터미널 조회 명령: login, logout, whoami, mine, search, show.

use std::io::IsTerminal;

use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};

use crate::config::{self, KeySource, Paths, Settings};
use crate::linear::client::{ApiError, LinearClient};
use crate::linear::filter::{build_issue_filter, token_filter};
use crate::linear::queries;
use crate::linear::types::{Comment, Issue, Viewer};
use crate::markdown::{self, Theme};
use crate::search::query::parse;
use crate::search::rank::{SearchIndex, merge, sort_mine};
use crate::store::{Store, remove_db_files};
use crate::ui::row::issue_row;
pub use crate::ui::style::{priority_label, state_icon};

const VIEWER_TTL_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const SEARCH_LIMIT: usize = 30;

#[derive(Parser, Debug)]
#[command(
    name = "herdr-linear",
    version,
    about = "herdr에서 Linear를 빠르게 조회"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum Command {
    /// API 키를 입력하고 검증해 저장한다
    Login,
    /// 저장된 API 키와 캐시를 지운다
    Logout,
    /// 연결된 계정과 워크스페이스
    Whoami,
    /// 나에게 할당된 열린 이슈
    Mine,
    /// 이슈 검색 (예: 로그인 l:bug s:진행 @나 #ENG p:high)
    Search {
        /// 서버 깊은 검색 (코멘트 포함, 분당 30회 제한)
        #[arg(long)]
        deep: bool,
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,
    },
    /// 이슈 상세 (예: ENG-131)
    Show { id: String },
}

/// 명령 실행에 필요한 것들.
pub struct Ctx {
    pub paths: Paths,
    pub settings: Settings,
    pub store: Store,
    pub client: LinearClient,
    /// 키를 어디서 얻었는지 (환경 변수 키의 인증 실패를 따로 안내한다)
    pub key_source: KeySource,
    pub now_ms: i64,
    /// 색을 쓸지 (표준 출력이 터미널일 때)
    pub color: bool,
    /// markdown 렌더링 폭
    pub width: u16,
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// 색을 쓸지: 표준 출력이 터미널이고 `NO_COLOR`가 비어 있지 않은 값으로 설정되지 않았을 때 (no-color.org).
pub fn use_color(is_terminal: bool, no_color: Option<&std::ffi::OsStr>) -> bool {
    is_terminal && no_color.is_none_or(|v| v.is_empty())
}

impl Ctx {
    pub fn open(paths: Paths) -> Result<Ctx> {
        let (settings, warnings) = config::load_settings(&paths.config_file());
        for w in warnings {
            eprintln!("경고: {w}");
        }
        let key = config::resolve_api_key(
            std::env::var("LINEAR_API_KEY").ok(),
            &paths,
            &config::default_credential_fallbacks(),
        )?
        .ok_or_else(|| anyhow!("API 키가 없어요. 먼저 `herdr-linear login`을 실행하세요"))?;
        let store = open_cache(&paths)?;
        let now = now_ms();
        let retention = (settings.cache_retention_days as i64).saturating_mul(DAY_MS);
        store.evict_older_than(now.saturating_sub(retention))?;
        let width = ratatui::crossterm::terminal::size()
            .map(|(w, _)| w)
            .unwrap_or(100)
            .min(120);
        Ok(Ctx {
            paths,
            settings,
            store,
            client: LinearClient::new(key.value),
            key_source: key.source,
            now_ms: now,
            color: use_color(
                std::io::stdout().is_terminal(),
                std::env::var_os("NO_COLOR").as_deref(),
            ),
            width,
        })
    }
}

/// 명령을 실행하고 출력할 문자열을 돌려준다.
pub fn run(cli: Cli) -> Result<String> {
    let paths = Paths::from_env()?;
    match cli.command {
        Command::Login => return login(&paths),
        Command::Logout => return logout(&paths, &config::default_credential_fallbacks()),
        _ => {}
    }
    let ctx = Ctx::open(paths)?;
    let out = match cli.command {
        Command::Whoami => whoami(&ctx),
        Command::Mine => mine(&ctx),
        Command::Search { deep, query } => search(&ctx, &query.join(" "), deep),
        Command::Show { id } => show(&ctx, &id),
        Command::Login | Command::Logout => Ok(String::new()),
    };
    if std::env::var_os("HERDR_LINEAR_DEBUG").is_some() {
        let r = ctx.client.rate_limit();
        eprintln!(
            "[debug] remaining={:?} complexity={:?} reset_ms={:?}",
            r.requests_remaining, r.complexity, r.requests_reset_ms
        );
    }
    out
}

pub fn login(paths: &Paths) -> Result<String> {
    let key = rpassword::prompt_password(
        "Linear API 키 (Linear → Settings → Security & access → Personal API keys): ",
    )?;
    let mut out = login_with_key(paths, &key, LinearClient::new)?;
    if std::env::var("LINEAR_API_KEY").is_ok_and(|v| !v.trim().is_empty()) {
        out.push_str("\n참고: LINEAR_API_KEY 환경 변수가 설정돼 있어서 그 값이 우선 쓰여요");
    }
    Ok(out)
}

/// 키를 `viewer` 쿼리로 검증하고, 맞으면 저장한다.
pub fn login_with_key(
    paths: &Paths,
    key: &str,
    make_client: impl Fn(String) -> LinearClient,
) -> Result<String> {
    let key = key.trim();
    if key.is_empty() {
        bail!("키가 비어 있어요");
    }
    let client = make_client(key.to_string());
    let viewer = match queries::viewer(&client) {
        Ok(v) => v,
        Err(ApiError::Auth) => bail!("키가 유효하지 않아요. Linear에서 키를 다시 확인하세요"),
        Err(e) => return Err(e.into()),
    };
    config::save_api_key(paths, key)?;
    let store = open_cache(paths)?;
    save_viewer(&store, &viewer, now_ms(), &client.key_fingerprint())?;
    Ok(markdown::sanitize(&format!(
        "{}님, {} 워크스페이스에 연결됐어요",
        viewer.name, viewer.organization.name
    )))
}

/// 캐시를 연다. 열 수 없으면 경고하고, 이번 실행은 저장 없이 메모리 캐시로 계속한다.
fn open_cache(paths: &Paths) -> Result<Store> {
    Store::open(&paths.cache_db()).or_else(|e| {
        eprintln!("경고: 캐시를 열지 못해 이번에는 저장 없이 실행해요 ({e:#})");
        Store::open_in_memory()
    })
}

/// 키(보조 위치 포함)와 캐시를 지운다. `fallbacks`는 실제 실행에서만 HOME 기준 위치를 넘긴다.
pub fn logout(paths: &Paths, fallbacks: &[std::path::PathBuf]) -> Result<String> {
    config::delete_credentials(paths, fallbacks)?;
    remove_db_files(&paths.cache_db());
    Ok("API 키와 캐시를 지웠어요".to_string())
}

fn save_viewer(store: &Store, v: &Viewer, now: i64, key_fp: &str) -> Result<()> {
    store.ensure_org(&v.organization.id)?;
    store.meta_set("viewer", &serde_json::to_string(v)?)?;
    store.meta_set("viewer_at", &now.to_string())?;
    store.meta_set("viewer_key", key_fp)?;
    Ok(())
}

/// 내 정보. 같은 키로 60분 안에 받은 것이 있으면 캐시를 쓴다.
/// 키가 바뀌었으면(다른 워크스페이스일 수 있음) TTL과 상관없이 다시 받고, 그때 캐시의 워크스페이스를 맞춘다.
/// 오프라인이면 같은 키로 받은 오래된 캐시라도 쓰고, 그것도 없으면 `None`.
pub fn load_viewer(ctx: &Ctx, force: bool) -> Result<Option<Viewer>> {
    let key_fp = ctx.client.key_fingerprint();
    let same_key = ctx.store.meta_get("viewer_key")?.as_deref() == Some(key_fp.as_str());
    let cached: Option<(Viewer, i64)> = match (
        same_key,
        ctx.store.meta_get("viewer")?,
        ctx.store.meta_get("viewer_at")?,
    ) {
        (true, Some(v), Some(at)) => serde_json::from_str(&v)
            .ok()
            .map(|v| (v, at.parse().unwrap_or(0))),
        _ => None,
    };
    if !force
        && let Some((v, at)) = &cached
        && ctx.now_ms - at < VIEWER_TTL_MS
    {
        return Ok(Some(v.clone()));
    }
    match queries::viewer(&ctx.client) {
        Ok(v) => {
            save_viewer(&ctx.store, &v, ctx.now_ms, &key_fp)?;
            Ok(Some(v))
        }
        Err(ApiError::Offline(_)) => Ok(cached.map(|(v, _)| v)),
        Err(e) => Err(api_error(ctx, e)),
    }
}

/// 검색 범위 팀 id. config의 `teams`가 비어 있으면 내가 속한 팀 전부.
pub fn scope_team_ids(viewer: &Viewer, settings: &Settings) -> Vec<String> {
    viewer
        .teams
        .nodes
        .iter()
        .filter(|t| {
            settings.teams.is_empty()
                || settings
                    .teams
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(&t.key))
        })
        .map(|t| t.id.clone())
        .collect()
}

pub fn whoami(ctx: &Ctx) -> Result<String> {
    let v = load_viewer(ctx, true)?
        .ok_or_else(|| anyhow!("오프라인이라 계정 정보를 가져올 수 없어요"))?;
    let teams: Vec<String> = v
        .teams
        .nodes
        .iter()
        .map(|t| format!("{} ({})", t.key, t.name))
        .collect();
    let mut out = vec![
        format!("{} ({}) <{}>", v.name, v.display_name, v.email),
        format!(
            "워크스페이스: {} ({})",
            v.organization.name, v.organization.url_key
        ),
        format!("팀: {}", teams.join(", ")),
        format!(
            "검색 범위: 팀 {}개",
            scope_team_ids(&v, &ctx.settings).len()
        ),
    ];
    if let Some(w) = scope_warning(&v, &ctx.settings) {
        out.push(w);
    }
    let rate = ctx.client.rate_limit();
    if let Some(r) = rate.requests_remaining {
        out.push(format!("남은 요청: {r} (시간당)"));
    }
    // 이름·팀 이름은 워크스페이스 구성원이 정할 수 있는 값이라 제어 문자를 지운다
    Ok(markdown::sanitize(&out.join("\n")))
}

/// config의 `teams`가 내 팀과 하나도 맞지 않으면 경고 문구. 이때 범위 제한 없이 모든 팀에서 찾는다.
pub fn scope_warning(viewer: &Viewer, settings: &Settings) -> Option<String> {
    (!settings.teams.is_empty() && scope_team_ids(viewer, settings).is_empty()).then(|| {
        format!(
            "경고: config의 teams({})와 맞는 팀이 없어서 모든 팀에서 찾아요",
            settings.teams.join(", ")
        )
    })
}

pub fn mine(ctx: &Ctx) -> Result<String> {
    // 키의 워크스페이스가 바뀌었으면 여기서 캐시가 비워진다 (같은 키면 60분에 한 번만 요청)
    load_viewer(ctx, false)?;
    match queries::my_issues(&ctx.client) {
        Ok(issues) => {
            ctx.store.upsert_issues(&issues, ctx.now_ms)?;
            let mut issues: Vec<Issue> = issues.into_iter().filter(|i| !i.is_gone()).collect();
            sort_mine(&mut issues);
            let ids: Vec<String> = issues.iter().map(|i| i.id.clone()).collect();
            ctx.store.set_view("mine", &ids, ctx.now_ms)?;
            Ok(format_list(ctx, &issues, None))
        }
        Err(ApiError::Offline(msg)) => match ctx.store.get_view("mine")? {
            Some((issues, at)) => Ok(format_list(
                ctx,
                &issues,
                Some(&format!(
                    "오프라인: {} 저장된 결과 · {msg}",
                    ago(ctx.now_ms, at)
                )),
            )),
            None => Err(anyhow!("오프라인이고 저장된 결과도 없어요: {msg}")),
        },
        Err(e) => Err(api_error(ctx, e)),
    }
}

pub fn search(ctx: &Ctx, input: &str, deep: bool) -> Result<String> {
    let q = parse(input);
    if deep && q.text().is_empty() {
        bail!("깊은 검색에는 검색어가 필요해요");
    }
    let viewer = load_viewer(ctx, false)?;
    let viewer_id = viewer.as_ref().map(|v| v.id.as_str());
    let local = SearchIndex::new(ctx.store.all_issues()?).search(&q, viewer_id);
    let server = if deep {
        queries::deep_search(&ctx.client, &q.text(), token_filter(&q).as_ref())
    } else {
        let scope = viewer
            .as_ref()
            .map(|v| scope_team_ids(v, &ctx.settings))
            .unwrap_or_default();
        if let Some(w) = viewer
            .as_ref()
            .and_then(|v| scope_warning(v, &ctx.settings))
        {
            eprintln!("{w}");
        }
        queries::filter_issues(&ctx.client, &build_issue_filter(&q, &scope))
    };
    match server {
        Ok(found) => {
            ctx.store.upsert_issues(&found, ctx.now_ms)?;
            let found: Vec<Issue> = found.into_iter().filter(|i| !i.is_gone()).collect();
            let merged = merge(&local, &found, &q, viewer_id);
            Ok(format_list(
                ctx,
                &merged[..merged.len().min(SEARCH_LIMIT)],
                None,
            ))
        }
        Err(ApiError::Offline(msg)) => Ok(format_list(
            ctx,
            &local[..local.len().min(SEARCH_LIMIT)],
            Some(&format!("오프라인: 저장된 이슈에서만 찾았어요 · {msg}")),
        )),
        Err(e) => Err(api_error(ctx, e)),
    }
}

pub fn show(ctx: &Ctx, id: &str) -> Result<String> {
    // 다른 워크스페이스의 이슈가 이전 캐시에 섞이지 않도록 먼저 워크스페이스를 맞춘다
    let viewer = load_viewer(ctx, false)?;
    let keys = team_keys(viewer.as_ref());
    match queries::issue_detail(&ctx.client, id) {
        Ok(Some(d)) => {
            let removed = ctx
                .store
                .upsert_issues(std::slice::from_ref(&d.issue), ctx.now_ms)?;
            if !removed.is_empty() {
                return Ok(format!(
                    "{}은(는) 보관되었거나 삭제된 이슈예요",
                    d.issue.identifier
                ));
            }
            ctx.store
                .set_comments(&d.issue.id, &d.comments, ctx.now_ms)?;
            ctx.store.mark_viewed(&d.issue.id, ctx.now_ms)?;
            Ok(format_detail(
                ctx,
                &d.issue,
                &d.comments,
                d.more_comments,
                None,
                &keys,
            ))
        }
        Ok(None) => {
            if let Some(cached) = ctx.store.get_issue(id)? {
                ctx.store.remove_issue(&cached.id)?;
            }
            bail!("{id} 이슈를 찾을 수 없어요 (보관·삭제됐거나 권한이 없을 수 있어요)")
        }
        Err(ApiError::Offline(msg)) => {
            let issue = ctx
                .store
                .get_issue(id)?
                .ok_or_else(|| anyhow!("오프라인이고 저장된 {id}도 없어요: {msg}"))?;
            let comments = ctx
                .store
                .get_comments(&issue.id)?
                .map(|(c, _)| c)
                .unwrap_or_default();
            Ok(format_detail(
                ctx,
                &issue,
                &comments,
                false,
                Some(&format!("오프라인: 저장된 내용 · {msg}")),
                &keys,
            ))
        }
        Err(e) => Err(api_error(ctx, e)),
    }
}

pub fn ago(now_ms: i64, then_ms: i64) -> String {
    let mins = (now_ms - then_ms).max(0) / 60_000;
    match mins {
        0 => "방금".to_string(),
        m if m < 60 => format!("{m}분 전"),
        m if m < 60 * 24 => format!("{}시간 전", m / 60),
        m => format!("{}일 전", m / (60 * 24)),
    }
}

/// API 오류를 사용자 문구로 바꾼다.
/// 한도 초과면 언제 다시 시도할지, 환경 변수 키가 틀렸으면 그 사실을 알려준다.
fn api_error(ctx: &Ctx, e: ApiError) -> anyhow::Error {
    match e {
        ApiError::RateLimited {
            reset_at_ms: Some(reset),
        } => {
            let mins = ((reset - ctx.now_ms).max(0) + 59_999) / 60_000;
            anyhow!("Linear API 한도를 넘었어요. {mins}분 후 다시 시도하세요")
        }
        ApiError::Auth if ctx.key_source == KeySource::Env => {
            anyhow!("LINEAR_API_KEY 환경 변수의 키가 유효하지 않아요")
        }
        other => other.into(),
    }
}

pub fn issue_line(issue: &Issue) -> String {
    let mut s = format!(
        "{} {:<9} {}",
        state_icon(&issue.state.state_type),
        issue.identifier,
        issue.title
    );
    let labels = issue.label_names();
    if !labels.is_empty() {
        s.push_str(&format!("  [{}]", labels.join(", ")));
    }
    if let Some(a) = &issue.assignee {
        s.push_str(&format!("  @{}", a.display_name));
    }
    markdown::sanitize(&s)
}

/// 목록 출력. 터미널이면 상태·라벨에 Linear 색을 입힌다.
fn format_list(ctx: &Ctx, issues: &[Issue], banner: Option<&str>) -> String {
    let mut out = Vec::new();
    if let Some(b) = banner {
        out.push(format!("({b})"));
    }
    if issues.is_empty() {
        out.push("결과가 없어요".to_string());
    }
    for issue in issues {
        out.push(if ctx.color {
            markdown::to_ansi(&[issue_row(issue, ctx.width)])
        } else {
            issue_line(issue)
        });
    }
    out.join("\n")
}

/// 내 팀 키 목록 (식별자 강조용). viewer가 없으면 빈 목록.
pub fn team_keys(viewer: Option<&Viewer>) -> Vec<String> {
    viewer
        .map(|v| v.teams.nodes.iter().map(|t| t.key.clone()).collect())
        .unwrap_or_default()
}

fn format_detail(
    ctx: &Ctx,
    issue: &Issue,
    comments: &[Comment],
    more: bool,
    banner: Option<&str>,
    team_keys: &[String],
) -> String {
    let theme = Theme::default();
    let mut out = Vec::new();
    if let Some(b) = banner {
        out.push(format!("({b})"));
    }
    out.push(format!("{}  {}", issue.identifier, issue.title));
    out.push(
        [
            format!(
                "{} {}",
                state_icon(&issue.state.state_type),
                issue.state.name
            ),
            format!("우선순위 {}", priority_label(issue.priority)),
            match &issue.assignee {
                Some(a) => format!("@{}", a.display_name),
                None => "담당자 없음".to_string(),
            },
        ]
        .join(" · "),
    );
    let mut extra = Vec::new();
    let labels = issue.label_names();
    if !labels.is_empty() {
        extra.push(labels.join(", "));
    }
    if let Some(p) = &issue.project {
        extra.push(format!("프로젝트 {}", p.name));
    }
    if let Some(c) = &issue.cycle {
        extra.push(format!(
            "사이클 {}",
            c.name
                .clone()
                .unwrap_or_else(|| (c.number as i64).to_string())
        ));
    }
    if let Some(p) = &issue.parent {
        extra.push(format!("상위 {}", p.identifier));
    }
    if let Some(e) = issue.estimate {
        extra.push(format!("예상 {e}"));
    }
    if let Some(d) = &issue.due_date {
        extra.push(format!("마감 {d}"));
    }
    if !extra.is_empty() {
        out.push(extra.join(" · "));
    }
    out.push(issue.url.clone());
    // 여기까지는 Linear 값이 그대로 들어간 평문이라 제어 문자를 지운다
    let mut out: Vec<String> = out.iter().map(|l| markdown::sanitize(l)).collect();
    out.push(String::new());
    let body = issue.description.as_deref().unwrap_or("").trim();
    if body.is_empty() {
        out.push("(본문 없음)".to_string());
    } else {
        out.push(render_md(ctx, body, &theme, team_keys));
    }
    if !comments.is_empty() {
        out.push(String::new());
        out.push(format!(
            "── 코멘트 {}{} ──",
            comments.len(),
            if more { "+" } else { "" }
        ));
        for c in comments {
            let who = c
                .user
                .as_ref()
                .map(|u| u.display_name.as_str())
                .unwrap_or("알 수 없음");
            out.push(String::new());
            out.push(markdown::sanitize(&format!(
                "{who} · {}",
                short_time(&c.created_at)
            )));
            out.push(render_md(ctx, &c.body, &theme, team_keys));
        }
        if more {
            out.push(String::new());
            out.push("코멘트가 더 있어요. 브라우저에서 보세요".to_string());
        }
    }
    out.join("\n")
}

fn render_md(ctx: &Ctx, md: &str, theme: &Theme, team_keys: &[String]) -> String {
    let r = markdown::render_with(md, ctx.width, theme, team_keys);
    let mut text = if ctx.color {
        markdown::to_ansi(&r.lines)
    } else {
        markdown::to_plain(&r.lines)
    };
    if !r.links.is_empty() {
        text.push('\n');
        for l in &r.links {
            text.push_str(&format!("\n[{}] {}", l.index, l.url));
        }
    }
    text
}

fn short_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| rfc3339.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;
    use mockito::Matcher;
    use serde_json::{Value, json};

    const NOW: i64 = 1_791_288_000_000;

    fn test_ctx(endpoint: String) -> (tempfile::TempDir, Ctx) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            state_dir: dir.path().join("state"),
        };
        let ctx = Ctx {
            paths,
            settings: Settings::default(),
            store: Store::open_in_memory().unwrap(),
            client: LinearClient::with_endpoint("lin_api_test", endpoint),
            key_source: KeySource::File,
            now_ms: NOW,
            color: false,
            width: 60,
        };
        (dir, ctx)
    }

    fn url(server: &mockito::Server) -> String {
        format!("{}/graphql", server.url())
    }

    /// 아무도 듣지 않는 주소 (오프라인 흉내)
    const OFFLINE: &str = "http://127.0.0.1:9/graphql";

    fn viewer_json() -> Value {
        json!({
            "id": "me", "name": "김민수", "displayName": "minsu", "email": "m@acme.dev",
            "organization": { "id": "org1", "name": "Acme", "urlKey": "acme" },
            "teams": { "nodes": [
                { "id": "team-ENG", "key": "ENG", "name": "Engineering" },
                { "id": "team-OPS", "key": "OPS", "name": "Ops" }
            ] }
        })
    }

    fn mock_viewer(server: &mut mockito::Server) -> mockito::Mock {
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Viewer".into()))
            .with_body(json!({ "data": { "viewer": viewer_json() } }).to_string())
            .create()
    }

    fn mock_issues(server: &mut mockito::Server, nodes: Vec<Value>) -> mockito::Mock {
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Issues".into()))
            .with_body(
                json!({ "data": { "issues": { "nodes": nodes, "pageInfo": { "hasNextPage": false, "endCursor": null } } } })
                    .to_string(),
            )
            .create()
    }

    #[test]
    fn cli_parses_search_with_deep_flag() {
        let cli =
            Cli::try_parse_from(["herdr-linear", "search", "--deep", "세션", "만료"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Search {
                deep: true,
                query: vec!["세션".into(), "만료".into()]
            }
        );
        assert!(Cli::try_parse_from(["herdr-linear", "search"]).is_err());
    }

    #[test]
    fn mine_sorts_and_saves_view() {
        let mut server = mockito::Server::new();
        mock_issues(
            &mut server,
            vec![
                IssueBuilder::new("i2", "ENG-2", "할 일").json(),
                IssueBuilder::new("i1", "ENG-1", "진행 중")
                    .state("In Progress", "started")
                    .labels(&["bug"])
                    .json(),
            ],
        );
        let (_d, ctx) = test_ctx(url(&server));
        let out = mine(&ctx).unwrap();
        assert_eq!(out, "◐ ENG-1     진행 중  [bug]\n○ ENG-2     할 일");
        let (saved, _) = ctx.store.get_view("mine").unwrap().unwrap();
        assert_eq!(
            saved
                .iter()
                .map(|i| i.identifier.as_str())
                .collect::<Vec<_>>(),
            vec!["ENG-1", "ENG-2"]
        );
    }

    #[test]
    fn mine_offline_uses_saved_view() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("i1", "ENG-1", "저장된 이슈").build()],
                NOW,
            )
            .unwrap();
        ctx.store
            .set_view("mine", &["i1".into()], NOW - 5 * 60_000)
            .unwrap();
        let out = mine(&ctx).unwrap();
        assert!(out.starts_with("(오프라인: 5분 전 저장된 결과"), "{out}");
        assert!(out.contains("ENG-1"));
    }

    #[test]
    fn mine_offline_without_cache_is_error() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        assert!(mine(&ctx).is_err());
    }

    #[test]
    fn search_merges_local_and_server() {
        let mut server = mockito::Server::new();
        mock_viewer(&mut server);
        let m = server
            .mock("POST", "/graphql")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("query Issues".into()),
                Matcher::PartialJson(json!({ "variables": { "filter": { "and": [
                    { "or": [ { "title": { "containsIgnoreCase": "로그인" } }, { "description": { "containsIgnoreCase": "로그인" } } ] },
                    { "team": { "id": { "in": ["team-ENG", "team-OPS"] } } }
                ] } } })),
            ]))
            .with_body(
                json!({ "data": { "issues": { "nodes": [ IssueBuilder::new("s1", "ENG-1", "로그인 버튼").json() ], "pageInfo": { "hasNextPage": false, "endCursor": null } } } })
                    .to_string(),
            )
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("l1", "OPS-9", "로그인 서버").build()],
                NOW,
            )
            .unwrap();
        let out = search(&ctx, "로그인", false).unwrap();
        assert!(out.contains("ENG-1     로그인 버튼"), "{out}");
        assert!(out.contains("OPS-9     로그인 서버"), "{out}");
        m.assert();
        // 서버 결과는 캐시에 저장된다
        assert!(ctx.store.get_issue("ENG-1").unwrap().is_some());
    }

    #[test]
    fn search_offline_falls_back_to_local() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("l1", "OPS-9", "로그인 서버").build()],
                NOW,
            )
            .unwrap();
        let out = search(&ctx, "로그인", false).unwrap();
        assert!(
            out.starts_with("(오프라인: 저장된 이슈에서만 찾았어요"),
            "{out}"
        );
        assert!(out.contains("OPS-9"));
    }

    #[test]
    fn deep_search_needs_text() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        assert!(search(&ctx, "l:bug", true).is_err());
    }

    #[test]
    fn show_renders_detail_and_marks_viewed() {
        let mut server = mockito::Server::new();
        let mut issue = IssueBuilder::new("i1", "ENG-1", "로그인 버그")
            .state("In Progress", "started")
            .priority(2)
            .assignee("me", "김민수")
            .labels(&["bug"])
            .description("## 재현\n- 로그인 후 대기\n\n[로그](https://x.dev/log)")
            .json();
        issue["comments"] = json!({
            "nodes": [ { "id": "c1", "body": "확인할게요", "createdAt": "2026-10-01T00:00:00.000Z", "editedAt": null,
                         "user": { "id": "u2", "name": "이영희", "displayName": "영희" } } ],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        });
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Detail".into()))
            .with_body(json!({ "data": { "issue": issue } }).to_string())
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        let out = show(&ctx, "ENG-1").unwrap();
        assert!(
            out.starts_with("ENG-1  로그인 버그\n◐ In Progress · 우선순위 높음 · @김민수\nbug\n"),
            "{out}"
        );
        assert!(out.contains("• 로그인 후 대기"), "{out}");
        assert!(out.contains("로그 [1]"), "{out}");
        assert!(out.contains("[1] https://x.dev/log"), "{out}");
        assert!(out.contains("── 코멘트 1 ──"), "{out}");
        assert!(out.contains("영희 · "), "{out}");
        assert!(out.contains("확인할게요"), "{out}");
        assert_eq!(ctx.store.recent_viewed(5).unwrap().len(), 1);
        assert!(ctx.store.get_comments("i1").unwrap().is_some());
    }

    #[test]
    fn show_not_found_drops_cached_copy() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Detail".into()))
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"code":"INVALID_INPUT"}}]}"#)
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        ctx.store
            .upsert_issues(&[IssueBuilder::new("i9", "ENG-9", "옛 이슈").build()], NOW)
            .unwrap();
        assert!(show(&ctx, "ENG-9").is_err());
        assert_eq!(ctx.store.get_issue("ENG-9").unwrap(), None);
    }

    #[test]
    fn show_archived_says_gone() {
        let mut server = mockito::Server::new();
        let mut issue = IssueBuilder::new("i1", "ENG-1", "보관됨").archived().json();
        issue["comments"] =
            json!({ "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } });
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Detail".into()))
            .with_body(json!({ "data": { "issue": issue } }).to_string())
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        assert_eq!(
            show(&ctx, "ENG-1").unwrap(),
            "ENG-1은(는) 보관되었거나 삭제된 이슈예요"
        );
    }

    #[test]
    fn show_offline_uses_cache() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("i1", "ENG-1", "저장된 상세")
                    .description("본문")
                    .build()],
                NOW,
            )
            .unwrap();
        let out = show(&ctx, "eng-1").unwrap();
        assert!(out.starts_with("(오프라인: 저장된 내용"), "{out}");
        assert!(out.contains("ENG-1  저장된 상세"));
        assert!(out.contains("본문"));
    }

    #[test]
    fn login_saves_key_and_viewer() {
        let mut server = mockito::Server::new();
        mock_viewer(&mut server);
        let (_d, ctx) = test_ctx(url(&server));
        let endpoint = url(&server);
        let out = login_with_key(&ctx.paths, "  lin_api_new  ", |k| {
            LinearClient::with_endpoint(k, endpoint.clone())
        })
        .unwrap();
        assert_eq!(out, "김민수님, Acme 워크스페이스에 연결됐어요");
        assert_eq!(
            std::fs::read_to_string(ctx.paths.credentials_file()).unwrap(),
            "lin_api_new\n"
        );
        let store = Store::open(&ctx.paths.cache_db()).unwrap();
        assert_eq!(store.meta_get("org_id").unwrap().as_deref(), Some("org1"));
    }

    #[test]
    fn invalid_key_saves_nothing() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Authentication required","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#)
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        let endpoint = url(&server);
        let err = login_with_key(&ctx.paths, "bad", |k| {
            LinearClient::with_endpoint(k, endpoint.clone())
        })
        .unwrap_err();
        assert!(err.to_string().contains("유효하지 않아요"));
        assert!(!ctx.paths.credentials_file().exists());
    }

    #[test]
    fn login_continues_when_cache_cannot_open() {
        let mut server = mockito::Server::new();
        mock_viewer(&mut server);
        let (_d, ctx) = test_ctx(url(&server));
        // 상태 디렉터리 자리에 파일이 있어서 캐시를 열 수 없는 상황
        std::fs::write(&ctx.paths.state_dir, b"not a dir").unwrap();
        let endpoint = url(&server);
        let out = login_with_key(&ctx.paths, "lin_api_new", |k| {
            LinearClient::with_endpoint(k, endpoint.clone())
        })
        .unwrap();
        assert_eq!(out, "김민수님, Acme 워크스페이스에 연결됐어요");
        assert!(ctx.paths.credentials_file().exists());
    }

    #[test]
    fn logout_removes_key_and_cache() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        config::save_api_key(&ctx.paths, "lin_api_x").unwrap();
        Store::open(&ctx.paths.cache_db()).unwrap();
        logout(&ctx.paths, &[]).unwrap();
        assert!(!ctx.paths.credentials_file().exists());
        assert!(!ctx.paths.cache_db().exists());
    }

    #[test]
    fn scope_follows_settings() {
        let v: Viewer = serde_json::from_value(viewer_json()).unwrap();
        assert_eq!(
            scope_team_ids(&v, &Settings::default()),
            vec!["team-ENG", "team-OPS"]
        );
        let only_ops = Settings {
            teams: vec!["ops".into()],
            ..Settings::default()
        };
        assert_eq!(scope_team_ids(&v, &only_ops), vec!["team-OPS"]);
    }

    #[test]
    fn viewer_cache_is_used_within_ttl() {
        let mut server = mockito::Server::new();
        let m = mock_viewer(&mut server).expect(1);
        let (_d, ctx) = test_ctx(url(&server));
        load_viewer(&ctx, false).unwrap().unwrap();
        load_viewer(&ctx, false).unwrap().unwrap();
        m.assert();
    }

    #[test]
    fn key_change_refetches_viewer_within_ttl() {
        let mut server = mockito::Server::new();
        let m = mock_viewer(&mut server).expect(1);
        let (_d, ctx) = test_ctx(url(&server));
        // 다른 키로 방금 저장된 viewer: TTL 안이어도 다시 물어봐야 한다
        ctx.store
            .meta_set("viewer", &viewer_json().to_string())
            .unwrap();
        ctx.store.meta_set("viewer_at", &NOW.to_string()).unwrap();
        ctx.store
            .meta_set("viewer_key", "fp-of-another-key")
            .unwrap();
        load_viewer(&ctx, false).unwrap().unwrap();
        m.assert();
    }

    #[test]
    fn mine_clears_cache_of_previous_workspace() {
        let mut server = mockito::Server::new();
        mock_viewer(&mut server);
        mock_issues(
            &mut server,
            vec![IssueBuilder::new("n1", "ENG-1", "새 워크스페이스").json()],
        );
        let (_d, ctx) = test_ctx(url(&server));
        // 이전 키(다른 워크스페이스)로 쌓인 캐시
        ctx.store.ensure_org("org-old").unwrap();
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("o1", "OLD-1", "옛 워크스페이스").build()],
                NOW,
            )
            .unwrap();
        mine(&ctx).unwrap();
        assert_eq!(ctx.store.get_issue("OLD-1").unwrap(), None);
        assert!(ctx.store.get_issue("ENG-1").unwrap().is_some());
    }

    #[test]
    fn list_and_detail_strip_control_characters() {
        let issue = IssueBuilder::new("i1", "ENG-1", "로그인\u{1b}[2J 버그")
            .assignee("u1", "민수\u{1b}]0;x\u{7}")
            .build();
        assert_eq!(issue_line(&issue), "○ ENG-1     로그인[2J 버그  @민수]0;x");
        let (_d, ctx) = test_ctx(OFFLINE.into());
        ctx.store.upsert_issues(&[issue], NOW).unwrap();
        let out = show(&ctx, "ENG-1").unwrap();
        assert!(!out.contains('\u{1b}'), "{out:?}");
        assert!(!out.contains('\u{7}'), "{out:?}");
    }

    #[test]
    fn show_strips_entity_encoded_escapes_with_color_on() {
        let (_d, mut ctx) = test_ctx(OFFLINE.into());
        ctx.color = true;
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("i1", "ENG-1", "제목")
                    .description("a &#27;]52;c;ZXZpbA==&#7; b\n\n[x](https://e.com/&#27;[2J)")
                    .build()],
                NOW,
            )
            .unwrap();
        let out = show(&ctx, "ENG-1").unwrap();
        // 색상(ESC [ … m)은 우리가 넣은 것이라 허용하고, OSC·BEL·화면 지우기는 없어야 한다
        assert!(!out.contains("\u{1b}]"), "{out:?}");
        assert!(!out.contains('\u{7}'), "{out:?}");
        assert!(!out.contains("\u{1b}[2J"), "{out:?}");
    }

    #[test]
    fn rate_limit_says_when_to_retry() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_header("x-ratelimit-requests-reset", &(NOW + 5 * 60_000).to_string())
            .with_body(r#"{"errors":[{"message":"Rate limit exceeded","extensions":{"code":"RATELIMITED"}}]}"#)
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        let err = mine(&ctx).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Linear API 한도를 넘었어요. 5분 후 다시 시도하세요"
        );
    }

    #[test]
    fn ago_is_human_readable() {
        assert_eq!(ago(NOW, NOW), "방금");
        assert_eq!(ago(NOW, NOW - 59 * 60_000), "59분 전");
        assert_eq!(ago(NOW, NOW - 3 * 3_600_000), "3시간 전");
        assert_eq!(ago(NOW, NOW - 2 * 86_400_000), "2일 전");
    }

    #[test]
    fn env_key_auth_failure_names_the_env_var() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Authentication required","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#)
            .create();
        let (_d, mut ctx) = test_ctx(url(&server));
        ctx.key_source = KeySource::Env;
        let err = mine(&ctx).unwrap_err();
        assert_eq!(
            err.to_string(),
            "LINEAR_API_KEY 환경 변수의 키가 유효하지 않아요"
        );
        ctx.key_source = KeySource::File;
        let err = mine(&ctx).unwrap_err();
        assert_eq!(err.to_string(), "API 키가 만료됐거나 권한이 없어요");
    }

    #[test]
    fn whoami_strips_control_characters() {
        let mut server = mockito::Server::new();
        let mut v = viewer_json();
        v["name"] = json!("김민수\u{1b}]52;c;eA==\u{7}");
        v["teams"]["nodes"][0]["name"] = json!("Eng\u{1b}[2J");
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Viewer".into()))
            .with_body(json!({ "data": { "viewer": v } }).to_string())
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        let out = whoami(&ctx).unwrap();
        assert!(!out.chars().any(|c| c.is_control() && c != '\n'), "{out:?}");
        assert!(out.contains("김민수]52;c;eA=="), "{out}");
    }

    #[test]
    fn scope_warning_on_unknown_teams() {
        let v: Viewer = serde_json::from_value(viewer_json()).unwrap();
        assert_eq!(scope_warning(&v, &Settings::default()), None);
        let ok = Settings {
            teams: vec!["ENG".into()],
            ..Settings::default()
        };
        assert_eq!(scope_warning(&v, &ok), None);
        let typo = Settings {
            teams: vec!["EGN".into()],
            ..Settings::default()
        };
        assert_eq!(
            scope_warning(&v, &typo).as_deref(),
            Some("경고: config의 teams(EGN)와 맞는 팀이 없어서 모든 팀에서 찾아요")
        );
    }

    #[test]
    fn no_color_turns_colors_off() {
        use std::ffi::OsStr;
        assert!(use_color(true, None));
        assert!(use_color(true, Some(OsStr::new(""))), "빈 값은 무시");
        assert!(!use_color(true, Some(OsStr::new("1"))));
        assert!(!use_color(false, None), "파이프에는 색을 쓰지 않는다");
    }

    #[test]
    fn mine_uses_linear_colors_on_a_terminal() {
        let mut server = mockito::Server::new();
        mock_issues(
            &mut server,
            vec![
                IssueBuilder::new("i1", "ENG-1", "색 확인")
                    .state("In Progress", "started")
                    .labels(&["bug"])
                    .json(),
            ],
        );
        let (_d, mut ctx) = test_ctx(url(&server));
        ctx.color = true;
        let out = mine(&ctx).unwrap();
        // 상태 색 #5e6ad2, 라벨 색 #eb5757 (truecolor)
        assert!(out.contains("\u{1b}[38;2;94;106;210m"), "{out:?}");
        assert!(out.contains("\u{1b}[38;2;235;87;87m"), "{out:?}");
        assert!(out.contains("ENG-1"), "{out:?}");
    }
}
