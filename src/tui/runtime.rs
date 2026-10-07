//! 팔레트 런타임: 앱이 요청한 일([`Effect`])을 캐시·네트워크·운영체제로 처리하고
//! 결과를 [`Msg`]로 돌려준다.
//!
//! 네트워크 요청은 스레드에서 보내고 결과([`Done`])는 채널로 받는다.
//! 캐시(SQLite 연결)는 메인 스레드에서만 만진다.

use std::io::stdout;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event,
};
use ratatui::crossterm::execute;

use crate::cli::{
    VIEWER_TTL_MS, cached_viewer, now_ms, save_viewer, scope_team_ids, scope_teams, scope_warning,
    team_keys,
};
use crate::config::{self, ApiKey, KeySource, Paths, Settings};
use crate::context::{Origin, current_branch, identifier_in_branch};
use crate::linear::client::{ApiError, LinearClient};
use crate::linear::filter::{build_issue_filter, token_filter};
use crate::linear::queries;
use crate::linear::types::{Issue, IssueDetail, Viewer};
use crate::log::Logger;
use crate::search::query::parse;
use crate::search::rank::sort_mine;
use crate::store::Store;
use crate::tui::app::{App, Effect, Input, Msg, Tab};
use crate::tui::system::System;
use crate::tui::{keys, view};

/// 브랜치 → 이슈 결과를 믿는 시간.
pub const BRANCH_TTL_MS: i64 = 10 * 60 * 1000;
/// "최근 본" 개수.
pub const RECENT_LIMIT: usize = 50;
/// 깊은 검색이 분당 한도에 걸렸을 때 안내.
pub const DEEP_LIMIT_TEXT: &str = "깊은 검색은 분당 30회까지예요. 잠시 뒤 다시 시도하세요";

const VIEW_MINE: &str = "mine";
const VIEW_ALL: &str = "all";

/// 키로 클라이언트를 만든다. 테스트에서는 가짜 서버 주소를 쓴다.
pub type MakeClient = Arc<dyn Fn(String) -> LinearClient + Send + Sync>;

/// 스레드에서 끝난 일. 키가 들어 있을 수 있어서 Debug를 만들지 않는다.
pub enum Done {
    Viewer(Result<Viewer, ApiError>),
    Page {
        tab: Tab,
        append: bool,
        result: Result<Page, ApiError>,
    },
    Search {
        seq: u64,
        /// 깊은 검색(`searchIssues`)인지
        deep: bool,
        result: Result<Vec<Issue>, ApiError>,
    },
    Detail {
        id: String,
        result: Result<Option<IssueDetail>, ApiError>,
    },
    /// 원래 pane의 (저장소, 브랜치)
    Branch(Option<(String, String)>),
    Pinned {
        repo: String,
        branch: String,
        result: Result<Option<Issue>, ApiError>,
    },
    Key {
        key: String,
        result: Result<Viewer, ApiError>,
    },
}

/// 목록 한 페이지.
pub struct Page {
    pub issues: Vec<Issue>,
    pub has_more: bool,
    pub cursor: Option<String>,
}

pub struct Runtime {
    paths: Paths,
    settings: Settings,
    store: Store,
    client: Option<Arc<LinearClient>>,
    key_source: KeySource,
    origin: Origin,
    system: Box<dyn System>,
    make_client: MakeClient,
    log: Logger,
    viewer: Option<Viewer>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    /// 스레드로 보내고 아직 돌아오지 않은 일 수
    in_flight: usize,
    all_cursor: Option<String>,
    /// 이 키로 받은 viewer가 캐시에 없으면, 캐시가 다른 워크스페이스 것일 수 있다.
    /// viewer를 받아 워크스페이스를 맞출 때까지 온 일을 여기 미뤄 둔다.
    gate: Option<Vec<Effect>>,
    /// 자동 검색을 멈추라고 이미 알린 리셋 시각
    throttled: Option<i64>,
    scope_warned: bool,
    /// viewer를 받는 중인지 (같은 확인을 두 번 보내지 않는다)
    viewer_pending: bool,
}

impl Runtime {
    pub fn new(
        paths: Paths,
        settings: Settings,
        store: Store,
        key: Option<ApiKey>,
        origin: Origin,
        system: Box<dyn System>,
        make_client: MakeClient,
    ) -> Runtime {
        let (tx, rx) = mpsc::channel();
        let log = Logger::new(paths.log_file());
        let key_source = key.as_ref().map_or(KeySource::File, |k| k.source);
        let client = key.map(|k| Arc::new(make_client(k.value)));
        Runtime {
            paths,
            settings,
            store,
            client,
            key_source,
            origin,
            system,
            make_client,
            log,
            viewer: None,
            tx,
            rx,
            in_flight: 0,
            all_cursor: None,
            gate: None,
            throttled: None,
            scope_warned: false,
            viewer_pending: false,
        }
    }

    /// 앱이 요청한 일을 처리한다. 캐시로 바로 답할 수 있는 것은 바로 돌려주고,
    /// 네트워크 일은 스레드로 보낸다 (결과는 [`Runtime::absorb`]로).
    pub fn execute(&mut self, effect: Effect, now: i64) -> Vec<Msg> {
        let effect = match effect {
            Effect::OpenUrl(url) => return vec![self.open_url(&url)],
            Effect::Copy { text, what } => return vec![self.copy(&text, &what)],
            Effect::ValidateKey(key) => {
                let make = self.make_client.clone();
                self.spawn(move || {
                    let result = queries::viewer(&make(key.clone()));
                    Done::Key { key, result }
                });
                return Vec::new();
            }
            other => other,
        };
        let Some(client) = self.client.clone() else {
            return vec![Msg::AuthFailed { env: false }];
        };
        if effect != Effect::Init
            && let Some(queue) = self.gate.as_mut()
        {
            queue.push(effect);
            // 확인이 실패해 멈춰 있었다면 이 요청을 계기로 다시 확인한다
            self.fetch_viewer(&client);
            return Vec::new();
        }
        match effect {
            Effect::Init => self.init(&client, now),
            Effect::LoadTab(tab) => self.load_tab(&client, tab),
            Effect::LoadMore => {
                let after = self.all_cursor.clone();
                let scope = self.scope();
                self.spawn(move || {
                    let result = match after {
                        Some(after) => fetch_page(&client, Tab::All, &scope, Some(&after)),
                        None => Ok(Page {
                            issues: Vec::new(),
                            has_more: false,
                            cursor: None,
                        }),
                    };
                    Done::Page {
                        tab: Tab::All,
                        append: true,
                        result,
                    }
                });
                Vec::new()
            }
            Effect::Search { seq, query } => {
                let filter = build_issue_filter(&parse(&query), &self.scope());
                self.spawn(move || Done::Search {
                    seq,
                    deep: false,
                    result: queries::filter_issues(&client, &filter),
                });
                Vec::new()
            }
            Effect::DeepSearch { seq, query } => {
                let q = parse(&query);
                let (term, filter) = (q.text(), token_filter(&q));
                self.spawn(move || Done::Search {
                    seq,
                    deep: true,
                    result: queries::deep_search(&client, &term, filter.as_ref()),
                });
                Vec::new()
            }
            Effect::OpenDetail(id) => self.open_detail(&client, id, now),
            Effect::ResolvePinned => match self.origin.cwd.clone() {
                // git은 프로세스를 띄우니 스레드에서 읽는다
                Some(cwd) => {
                    self.spawn(move || Done::Branch(current_branch(&cwd)));
                    Vec::new()
                }
                None => vec![Msg::Pinned(None)],
            },
            Effect::OpenUrl(_) | Effect::Copy { .. } | Effect::ValidateKey(_) => Vec::new(),
        }
    }

    /// 스레드에서 끝난 일을 캐시에 반영하고 앱에 알릴 것을 돌려준다.
    pub fn absorb(&mut self, done: Done, now: i64) -> Vec<Msg> {
        self.in_flight = self.in_flight.saturating_sub(1);
        if matches!(done, Done::Viewer(_)) {
            self.viewer_pending = false;
        }
        let mut msgs = match done {
            Done::Viewer(Ok(v)) => {
                if let Some(c) = &self.client {
                    let fp = c.key_fingerprint();
                    self.note(save_viewer(&self.store, &v, now, &fp));
                }
                let mut msgs = self.viewer_known(v);
                msgs.extend(self.open_gate(now));
                msgs
            }
            Done::Viewer(Err(ApiError::Auth)) => {
                self.gate = None;
                self.failed(ApiError::Auth, "내 정보")
            }
            Done::Viewer(Err(e)) => {
                self.log.write(&format!("내 정보: {e}"));
                self.fail_gate(e)
            }
            Done::Page {
                tab,
                append,
                result: Ok(page),
            } => {
                let issues = self.keep(page.issues, now);
                if tab == Tab::All {
                    self.all_cursor = page.cursor;
                }
                if !append {
                    let key = if tab == Tab::Mine {
                        VIEW_MINE
                    } else {
                        VIEW_ALL
                    };
                    let ids: Vec<String> = issues.iter().map(|i| i.id.clone()).collect();
                    self.note(self.store.set_view(key, &ids, now));
                }
                vec![Msg::Tab {
                    tab,
                    issues,
                    fresh: true,
                    has_more: page.has_more,
                    append,
                }]
            }
            Done::Page { result: Err(e), .. } => self.failed(e, "목록"),
            Done::Search {
                seq,
                result: Ok(found),
                ..
            } => vec![Msg::Search {
                seq,
                issues: self.keep(found, now),
            }],
            Done::Search {
                seq,
                deep: true,
                result: Err(ApiError::RateLimited { .. }),
            } => {
                // 깊은 검색에는 분당 30회 한도가 따로 있다. 응답의 리셋 시각은 시간당 요청 한도
                // 기준이라 맞지 않으니, 자동 검색은 멈추지 않고 로컬 결과를 둔 채 안내만 한다
                self.log.write("깊은 검색: 한도 초과");
                vec![
                    Msg::Search {
                        seq,
                        issues: Vec::new(),
                    },
                    Msg::Flash(DEEP_LIMIT_TEXT.into()),
                ]
            }
            Done::Search { result: Err(e), .. } => self.failed(e, "검색"),
            Done::Detail {
                id,
                result: Ok(Some(d)),
            } if !d.issue.is_gone() => {
                self.note(
                    self.store
                        .upsert_issues(std::slice::from_ref(&d.issue), now),
                );
                self.note(self.store.set_comments(&d.issue.id, &d.comments, now));
                self.note(self.store.mark_viewed(&d.issue.id, now));
                vec![Msg::Detail {
                    id,
                    issue: d.issue,
                    comments: d.comments,
                    more: d.more_comments,
                    fresh: true,
                }]
            }
            Done::Detail {
                id,
                result: Ok(found),
            } => {
                // 보관·삭제됐거나 없다: 캐시에서도 지우고 검색 색인을 다시 만든다
                let gone = match found {
                    Some(d) => Some(d.issue.id),
                    None => self.note(self.store.get_issue(&id)).flatten().map(|i| i.id),
                };
                if let Some(gone) = gone {
                    self.note(self.store.remove_issue(&gone));
                }
                vec![Msg::DetailGone(id), Msg::Index(self.all_issues())]
            }
            Done::Detail { result: Err(e), .. } => self.failed(e, "상세"),
            Done::Branch(None) => vec![Msg::Pinned(None)],
            Done::Branch(Some((repo, branch))) => self.pin_branch(repo, branch, now),
            Done::Pinned {
                repo,
                branch,
                result: Ok(found),
            } => {
                let found = found.and_then(|i| self.keep(vec![i], now).pop());
                let ident = found.as_ref().map(|i| i.identifier.as_str());
                self.note(self.store.branch_set(&repo, &branch, ident, now));
                vec![Msg::Pinned(found)]
            }
            Done::Pinned {
                result: Err(ApiError::Auth),
                ..
            } => self.failed(ApiError::Auth, "브랜치 이슈"),
            Done::Pinned { result: Err(e), .. } => {
                // 세지 않은 요청이라 앱에는 알리지 않는다
                self.log.write(&format!("브랜치 이슈: {e}"));
                Vec::new()
            }
            Done::Key { key, result: Ok(v) } => self.accept_key(key, v, now),
            Done::Key { result: Err(e), .. } => vec![Msg::KeyBad(match e {
                ApiError::Auth => "키가 유효하지 않아요. Linear에서 키를 다시 확인하세요".into(),
                ApiError::Offline(_) => "오프라인이라 키를 확인할 수 없어요".into(),
                other => other.to_string(),
            })],
        };
        msgs.extend(self.throttle(now));
        msgs
    }

    /// 끝난 일을 기다리지 않고 모두 가져온다.
    pub fn drain(&self) -> Vec<Done> {
        self.rx.try_iter().collect()
    }

    /// 끝난 일 하나를 `timeout`까지 기다린다.
    pub fn wait(&self, timeout: Duration) -> Option<Done> {
        self.rx.recv_timeout(timeout).ok()
    }

    fn spawn(&mut self, job: impl FnOnce() -> Done + Send + 'static) {
        let tx = self.tx.clone();
        self.in_flight += 1;
        std::thread::spawn(move || {
            let _ = tx.send(job());
        });
    }

    /// 캐시 오류는 기록만 하고 넘어간다 (캐시가 없어도 조회는 된다).
    fn note<T>(&self, r: Result<T>) -> Option<T> {
        r.map_err(|e| self.log.write(&format!("캐시 오류: {e:#}")))
            .ok()
    }

    fn all_issues(&self) -> Vec<Issue> {
        self.note(self.store.all_issues()).unwrap_or_default()
    }

    /// 받은 이슈를 캐시에 넣고, 보관·삭제된 것은 뺀다.
    fn keep(&self, issues: Vec<Issue>, now: i64) -> Vec<Issue> {
        self.note(self.store.upsert_issues(&issues, now));
        issues.into_iter().filter(|i| !i.is_gone()).collect()
    }

    fn scope(&self) -> Vec<String> {
        self.viewer
            .as_ref()
            .map(|v| scope_team_ids(v, &self.settings))
            .unwrap_or_default()
    }

    /// 범위 팀 키. 맞는 팀이 없으면 (모든 팀에서 찾으니) 내 팀 전부.
    fn scope_keys(&self) -> Vec<String> {
        let Some(v) = &self.viewer else {
            return Vec::new();
        };
        let keys: Vec<String> = scope_teams(v, &self.settings)
            .iter()
            .map(|t| t.key.clone())
            .collect();
        if keys.is_empty() {
            team_keys(Some(v))
        } else {
            keys
        }
    }

    /// 세어 둔 요청의 실패. 인증 실패는 키 입력 화면으로, 나머지는 상단 상태로.
    fn failed(&mut self, e: ApiError, what: &str) -> Vec<Msg> {
        self.log.write(&format!("{what}: {e}"));
        match e {
            ApiError::Auth => vec![Msg::AuthFailed {
                env: self.key_source == KeySource::Env,
            }],
            other => vec![Msg::Failed(other)],
        }
    }

    fn init(&mut self, client: &Arc<LinearClient>, now: i64) -> Vec<Msg> {
        match self
            .note(cached_viewer(&self.store, &client.key_fingerprint()))
            .flatten()
        {
            Some((v, at)) => {
                let mut msgs = self.viewer_known(v);
                msgs.push(Msg::Index(self.all_issues()));
                if now - at >= VIEWER_TTL_MS {
                    self.fetch_viewer(client);
                }
                msgs
            }
            None => {
                self.gate.get_or_insert_with(Vec::new);
                self.fetch_viewer(client);
                Vec::new()
            }
        }
    }

    fn fetch_viewer(&mut self, client: &Arc<LinearClient>) {
        if self.viewer_pending {
            return;
        }
        self.viewer_pending = true;
        let c = client.clone();
        self.spawn(move || Done::Viewer(queries::viewer(&c)));
    }

    /// 워크스페이스를 확인하지 못했다. 캐시가 다른 워크스페이스 것일 수 있으니 계속 미뤄 두고,
    /// 미뤄 둔 세는 요청은 실패로 끝낸다 ("갱신 중…"이 남지 않게). 다음 요청 때 다시 확인한다.
    fn fail_gate(&mut self, e: ApiError) -> Vec<Msg> {
        let Some(queue) = self.gate.as_mut() else {
            return Vec::new();
        };
        std::mem::take(queue)
            .iter()
            .filter(|effect| counted(effect))
            .map(|_| Msg::Failed(e.clone()))
            .collect()
    }

    /// viewer를 기억하고 앱에 알린다. config의 팀이 하나도 안 맞으면 한 번 경고한다.
    fn viewer_known(&mut self, v: Viewer) -> Vec<Msg> {
        let warning = scope_warning(&v, &self.settings).filter(|_| !self.scope_warned);
        self.scope_warned |= warning.is_some();
        self.viewer = Some(v.clone());
        let mut msgs = vec![Msg::Viewer(v)];
        msgs.extend(warning.map(Msg::Flash));
        msgs
    }

    /// 워크스페이스를 맞췄으니 미뤄 둔 일을 한다.
    fn open_gate(&mut self, now: i64) -> Vec<Msg> {
        let Some(queue) = self.gate.take() else {
            return Vec::new();
        };
        let mut msgs = vec![Msg::Index(self.all_issues())];
        for effect in queue {
            msgs.extend(self.execute(effect, now));
        }
        msgs
    }

    /// 탭 내용. 최근 본은 이 기기의 기록이라 바로 답하고, 나머지는 캐시를 먼저 보낸다.
    fn load_tab(&mut self, client: &Arc<LinearClient>, tab: Tab) -> Vec<Msg> {
        if tab == Tab::Recent {
            let issues = self
                .note(self.store.recent_viewed(RECENT_LIMIT))
                .unwrap_or_default();
            return vec![Msg::Tab {
                tab,
                issues,
                fresh: true,
                has_more: false,
                append: false,
            }];
        }
        let key = if tab == Tab::Mine {
            VIEW_MINE
        } else {
            VIEW_ALL
        };
        let cached = self.note(self.store.get_view(key)).flatten();
        let (c, scope) = (client.clone(), self.scope());
        self.spawn(move || Done::Page {
            tab,
            append: false,
            result: fetch_page(&c, tab, &scope, None),
        });
        cached
            .map(|(issues, _)| Msg::Tab {
                tab,
                issues,
                fresh: false,
                has_more: false,
                append: false,
            })
            .into_iter()
            .collect()
    }

    /// 상세. 캐시에 있으면 먼저 보내고 최근 본에 기록한 뒤 서버에서 다시 받는다.
    fn open_detail(&mut self, client: &Arc<LinearClient>, id: String, now: i64) -> Vec<Msg> {
        let mut msgs = Vec::new();
        if let Some(issue) = self.note(self.store.get_issue(&id)).flatten() {
            let comments = self
                .note(self.store.get_comments(&issue.id))
                .flatten()
                .map(|(c, _)| c)
                .unwrap_or_default();
            self.note(self.store.mark_viewed(&issue.id, now));
            msgs.push(Msg::Detail {
                id: id.clone(),
                issue,
                comments,
                more: false,
                fresh: false,
            });
        }
        let c = client.clone();
        self.spawn(move || {
            let result = queries::issue_detail(&c, &id);
            Done::Detail { id, result }
        });
        msgs
    }

    /// 브랜치 → 이슈: 10분 캐시 → 브랜치명의 식별자(캐시, 없으면 서버) → 서버 브랜치 검색.
    fn pin_branch(&mut self, repo: String, branch: String, now: i64) -> Vec<Msg> {
        if let Some((ident, at)) = self.note(self.store.branch_get(&repo, &branch)).flatten()
            && now - at < BRANCH_TTL_MS
        {
            match ident {
                None => return vec![Msg::Pinned(None)],
                Some(ident) => {
                    if let Some(issue) = self.note(self.store.get_issue(&ident)).flatten() {
                        return vec![Msg::Pinned(Some(issue))];
                    }
                }
            }
        }
        let ident = identifier_in_branch(&branch, &self.scope_keys());
        if let Some(ident) = &ident
            && let Some(issue) = self.note(self.store.get_issue(ident)).flatten()
        {
            self.note(
                self.store
                    .branch_set(&repo, &branch, Some(&issue.identifier), now),
            );
            return vec![Msg::Pinned(Some(issue))];
        }
        let Some(c) = self.client.clone() else {
            return vec![Msg::Pinned(None)];
        };
        self.spawn(move || {
            let result = find_branch_issue(&c, &branch, ident.as_deref());
            Done::Pinned {
                repo,
                branch,
                result,
            }
        });
        Vec::new()
    }

    /// 검증된 키를 저장하고 이 키로 바꾼다.
    fn accept_key(&mut self, key: String, v: Viewer, now: i64) -> Vec<Msg> {
        if let Err(e) = config::save_api_key(&self.paths, &key) {
            self.log.write(&format!("키 저장 실패: {e:#}"));
            return vec![Msg::KeyBad(format!("키를 저장하지 못했어요: {e:#}"))];
        }
        let client = Arc::new((self.make_client)(key));
        self.note(save_viewer(&self.store, &v, now, &client.key_fingerprint()));
        self.client = Some(client);
        self.key_source = KeySource::File;
        self.gate = None;
        self.viewer = Some(v.clone());
        vec![Msg::KeyOk(v)]
    }

    fn open_url(&self, url: &str) -> Msg {
        if !is_web_url(url) {
            return Msg::Flash("http(s) 링크만 열 수 있어요".into());
        }
        match self.system.open_url(url) {
            Ok(()) => Msg::Flash("브라우저에서 열었어요".into()),
            Err(e) => {
                self.log.write(&format!("브라우저 열기 실패: {e:#}"));
                Msg::Flash(format!("브라우저를 열지 못했어요: {e:#}"))
            }
        }
    }

    fn copy(&self, text: &str, what: &str) -> Msg {
        match self.system.copy(text) {
            Ok(()) => Msg::Flash(format!("복사됨: {what}")),
            Err(e) => {
                self.log.write(&format!("복사 실패: {e:#}"));
                Msg::Flash(format!("복사하지 못했어요: {e:#}"))
            }
        }
    }

    /// 남은 요청이 적으면 리셋 시각까지 자동 검색을 멈추라고 한 번 알린다.
    fn throttle(&mut self, now: i64) -> Option<Msg> {
        let rate = self.client.as_ref()?.rate_limit();
        let reset = rate.requests_reset_ms?;
        if !rate.should_pause_auto(now) || self.throttled == Some(reset) {
            return None;
        }
        self.throttled = Some(reset);
        Some(Msg::Throttled(reset))
    }
}

/// 앱이 "불러오는 중"으로 세는 요청인지. 끝날 때 앱에 결과나 실패를 꼭 돌려줘야 한다.
fn counted(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::LoadTab(_)
            | Effect::LoadMore
            | Effect::Search { .. }
            | Effect::DeepSearch { .. }
            | Effect::OpenDetail(_)
    )
}

/// 이슈 본문의 링크는 누구나 쓸 수 있으니 브라우저로는 http(s)만 넘긴다.
pub fn is_web_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

fn fetch_page(
    c: &LinearClient,
    tab: Tab,
    scope: &[String],
    after: Option<&str>,
) -> Result<Page, ApiError> {
    if tab == Tab::Mine {
        let mut issues = queries::my_issues(c)?;
        sort_mine(&mut issues);
        return Ok(Page {
            issues,
            has_more: false,
            cursor: None,
        });
    }
    let page = queries::team_issues(c, scope, after)?;
    Ok(Page {
        issues: page.nodes,
        has_more: page.page_info.has_next_page,
        cursor: page.page_info.end_cursor,
    })
}

/// 브랜치명의 식별자로 찾고, 없으면 서버 브랜치 검색을 한 번 부른다.
fn find_branch_issue(
    c: &LinearClient,
    branch: &str,
    ident: Option<&str>,
) -> Result<Option<Issue>, ApiError> {
    if let Some(ident) = ident
        && let Some(d) = queries::issue_detail(c, ident)?
        && !d.issue.is_gone()
    {
        return Ok(Some(d.issue));
    }
    queries::branch_issue(c, branch)
}

/// 터미널을 잡고 이벤트 루프를 돈다. 끝나면 (패닉이 나도) 터미널을 되돌린다.
pub fn run(mut rt: Runtime, mut app: App, effects: Vec<Effect>) -> Result<()> {
    log_panics(rt.paths.log_file());
    let mut terminal = ratatui::init();
    let _ = execute!(stdout(), EnableBracketedPaste, EnableMouseCapture);
    let result = event_loop(&mut terminal, &mut rt, &mut app, effects);
    let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
    if let Err(e) = &result {
        rt.log.write(&format!("종료: {e:#}"));
    }
    result
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    rt: &mut Runtime,
    app: &mut App,
    mut effects: Vec<Effect>,
) -> Result<()> {
    // 마우스로 누른 곳은 마지막으로 그린 화면에서 찾는다
    let mut last = view::Drawn::default();
    loop {
        let now = now_ms();
        let mut msgs = Vec::new();
        for done in rt.drain() {
            msgs.extend(rt.absorb(done, now));
        }
        effects.extend(app.tick(now));
        // 결과가 새 일을 낳을 수 있어서 더 없을 때까지 돌린다
        while !effects.is_empty() || !msgs.is_empty() {
            for effect in std::mem::take(&mut effects) {
                msgs.extend(rt.execute(effect, now));
            }
            for msg in std::mem::take(&mut msgs) {
                effects.extend(app.apply(msg, now));
            }
        }
        if app.quit {
            return Ok(());
        }
        let mut drawn = None;
        terminal.draw(|f| drawn = Some(view::draw(f, app, now)))?;
        if let Some(d) = drawn {
            if let Some(max) = d.detail_max_scroll {
                app.set_detail_max_scroll(max);
            }
            last = d;
        }
        if event::poll(Duration::from_millis(50))? {
            let input = match event::read()? {
                Event::Key(key) => keys::translate(app.mode, app.menu.is_some(), key),
                Event::Paste(text) => Some(Input::Paste(text)),
                Event::Mouse(m) => keys::mouse(&last, m),
                _ => None,
            };
            if let Some(input) = input {
                effects.extend(app.handle(input, now_ms()));
            }
        }
    }
}

/// 패닉을 로그에 남긴다. `ratatui::init`이 이 훅 앞에 터미널 복구를 끼운다.
fn log_panics(path: PathBuf) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // ratatui의 훅이 화면을 되돌린 뒤에 불린다. 마우스·붙여넣기 모드도 끈다
        let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
        Logger::new(path.clone()).write(&format!("패닉: {info}"));
        prev(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;
    use mockito::Matcher;
    use serde_json::{Value, json};
    use std::path::Path;
    use std::process::Command;
    use std::sync::Mutex;

    const T0: i64 = 1_700_000_000_000;
    const KEY: &str = "lin_api_test";

    #[derive(Clone, Default)]
    struct FakeSystem(Arc<Mutex<Vec<String>>>);

    impl System for FakeSystem {
        fn open_url(&self, url: &str) -> Result<()> {
            self.0.lock().unwrap().push(format!("open {url}"));
            Ok(())
        }

        fn copy(&self, text: &str) -> Result<()> {
            self.0.lock().unwrap().push(format!("copy {text}"));
            Ok(())
        }
    }

    struct Fixture {
        rt: Runtime,
        server: mockito::ServerGuard,
        sys: FakeSystem,
        dir: tempfile::TempDir,
    }

    fn viewer_json() -> Value {
        json!({
            "id": "me", "name": "김민수", "displayName": "minsu", "email": "m@acme.dev",
            "organization": { "id": "org1", "name": "Acme", "urlKey": "acme" },
            "teams": { "nodes": [ { "id": "team-ENG", "key": "ENG", "name": "Eng" } ] }
        })
    }

    fn viewer() -> Viewer {
        serde_json::from_value(viewer_json()).unwrap()
    }

    fn issue(id: &str, identifier: &str, title: &str) -> Issue {
        IssueBuilder::new(id, identifier, title).build()
    }

    /// `known`이면 이 키로 받은 viewer가 캐시에 있다 (워크스페이스 확인을 기다리지 않는다).
    fn fixture(key: Option<KeySource>, known: bool, origin: Origin) -> Fixture {
        let server = mockito::Server::new();
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            state_dir: dir.path().join("state"),
        };
        let store = Store::open_in_memory().unwrap();
        if known {
            let fp = LinearClient::new(KEY).key_fingerprint();
            save_viewer(&store, &viewer(), T0, &fp).unwrap();
        }
        let url = format!("{}/graphql", server.url());
        let sys = FakeSystem::default();
        let rt = Runtime::new(
            paths,
            Settings::default(),
            store,
            key.map(|source| ApiKey {
                value: KEY.into(),
                source,
            }),
            origin,
            Box::new(sys.clone()),
            Arc::new(move |k| LinearClient::with_endpoint(k, url.clone())),
        );
        Fixture {
            rt,
            server,
            sys,
            dir,
        }
    }

    fn mock(server: &mut mockito::ServerGuard, op: &str, data: Value) -> mockito::Mock {
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex(format!("query {op}\\b")))
            .with_body(json!({ "data": data }).to_string())
            .create()
    }

    fn page(issues: &[Issue], next: Option<&str>) -> Value {
        json!({ "issues": {
            "nodes": issues,
            "pageInfo": { "hasNextPage": next.is_some(), "endCursor": next }
        } })
    }

    /// 보낸 일이 모두 돌아올 때까지 기다려 반영한다.
    fn settle(rt: &mut Runtime, now: i64) -> Vec<Msg> {
        let mut msgs = Vec::new();
        while rt.in_flight > 0 {
            let done = rt.wait(Duration::from_secs(10)).expect("스레드 결과");
            msgs.extend(rt.absorb(done, now));
        }
        msgs
    }

    fn tab_titles(msg: &Msg) -> (bool, Vec<String>) {
        match msg {
            Msg::Tab { issues, fresh, .. } => {
                (*fresh, issues.iter().map(|i| i.title.clone()).collect())
            }
            other => panic!("Tab이 아님: {other:?}"),
        }
    }

    #[test]
    fn mine_tab_sends_cache_then_fresh_and_saves_view() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        let old = issue("a", "ENG-1", "옛 제목");
        fx.rt.store.upsert_issues(&[old], T0).unwrap();
        fx.rt.store.set_view("mine", &["a".into()], T0).unwrap();
        let m = mock(
            &mut fx.server,
            "Issues",
            page(&[issue("a", "ENG-1", "새 제목")], None),
        );
        let first = fx.rt.execute(Effect::LoadTab(Tab::Mine), T0);
        assert_eq!(tab_titles(&first[0]), (false, vec!["옛 제목".to_string()]));
        let fresh = settle(&mut fx.rt, T0 + 1);
        assert_eq!(tab_titles(&fresh[0]), (true, vec!["새 제목".to_string()]));
        m.assert();
        let (cached, at) = fx.rt.store.get_view("mine").unwrap().unwrap();
        assert_eq!((cached[0].title.as_str(), at), ("새 제목", T0 + 1));
    }

    #[test]
    fn recent_tab_is_local() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        fx.rt
            .store
            .upsert_issues(
                &[
                    issue("a", "ENG-1", "본 이슈"),
                    issue("b", "ENG-2", "안 본 이슈"),
                ],
                T0,
            )
            .unwrap();
        fx.rt.store.mark_viewed("a", T0).unwrap();
        let msgs = fx.rt.execute(Effect::LoadTab(Tab::Recent), T0);
        assert_eq!(tab_titles(&msgs[0]), (true, vec!["본 이슈".to_string()]));
        assert_eq!(fx.rt.in_flight, 0, "서버에 묻지 않는다");
    }

    #[test]
    fn new_key_waits_for_viewer_before_using_cache() {
        let mut fx = fixture(Some(KeySource::Env), false, Origin::default());
        // 다른 워크스페이스에서 받은 캐시
        fx.rt.store.ensure_org("other-org").unwrap();
        fx.rt
            .store
            .upsert_issues(&[issue("x", "OPS-1", "다른 워크스페이스")], T0)
            .unwrap();
        fx.rt.store.set_view("mine", &["x".into()], T0).unwrap();
        mock(&mut fx.server, "Viewer", json!({ "viewer": viewer_json() }));
        mock(
            &mut fx.server,
            "Issues",
            page(&[issue("a", "ENG-1", "내 이슈")], None),
        );
        assert!(fx.rt.execute(Effect::Init, T0).is_empty());
        assert!(
            fx.rt.execute(Effect::LoadTab(Tab::Mine), T0).is_empty(),
            "다른 워크스페이스 캐시를 보이지 않는다"
        );
        let msgs = settle(&mut fx.rt, T0);
        assert!(matches!(msgs[0], Msg::Viewer(_)));
        assert_eq!(
            msgs[1],
            Msg::Index(vec![]),
            "워크스페이스가 바뀌어 캐시를 비웠다"
        );
        let fresh = msgs.iter().find(|m| matches!(m, Msg::Tab { .. })).unwrap();
        assert_eq!(tab_titles(fresh), (true, vec!["내 이슈".to_string()]));
        assert!(fx.rt.store.get_issue("OPS-1").unwrap().is_none());
    }

    #[test]
    fn unverified_workspace_stays_hidden_when_viewer_fails() {
        let mut fx = fixture(Some(KeySource::Env), false, Origin::default());
        fx.rt.store.ensure_org("other-org").unwrap();
        fx.rt
            .store
            .upsert_issues(&[issue("x", "OPS-1", "다른 워크스페이스")], T0)
            .unwrap();
        fx.rt.store.set_view("mine", &["x".into()], T0).unwrap();
        fx.server
            .mock("POST", "/graphql")
            .with_status(503)
            .with_body("busy")
            .create();
        assert!(fx.rt.execute(Effect::Init, T0).is_empty());
        assert!(fx.rt.execute(Effect::LoadTab(Tab::Mine), T0).is_empty());
        assert_eq!(
            settle(&mut fx.rt, T0),
            vec![Msg::Failed(ApiError::Offline(
                "Linear 서버 오류 (503)".into()
            ))],
            "확인 전에는 캐시를 보이지 않고, 세는 요청만 실패로 끝낸다"
        );
        // 다음 요청 때 다시 확인하고, 확인되면 그때 불러온다
        fx.server.reset();
        mock(&mut fx.server, "Viewer", json!({ "viewer": viewer_json() }));
        mock(
            &mut fx.server,
            "Issues",
            page(&[issue("a", "ENG-1", "내 이슈")], None),
        );
        assert!(fx.rt.execute(Effect::LoadTab(Tab::Mine), T0).is_empty());
        let msgs = settle(&mut fx.rt, T0);
        assert!(matches!(msgs[0], Msg::Viewer(_)), "{msgs:?}");
        assert_eq!(
            msgs[1],
            Msg::Index(vec![]),
            "다른 워크스페이스 캐시는 비웠다"
        );
        let fresh = msgs.iter().find(|m| matches!(m, Msg::Tab { .. })).unwrap();
        assert_eq!(tab_titles(fresh), (true, vec!["내 이슈".to_string()]));
    }

    #[test]
    fn search_saves_results_and_drops_archived() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        let archived = IssueBuilder::new("z", "ENG-9", "결제 옛것")
            .archived()
            .build();
        mock(
            &mut fx.server,
            "Issues",
            page(&[issue("a", "ENG-1", "결제 화면"), archived], None),
        );
        assert!(
            fx.rt
                .execute(
                    Effect::Search {
                        seq: 3,
                        query: "결제".into()
                    },
                    T0
                )
                .is_empty()
        );
        match &settle(&mut fx.rt, T0)[0] {
            Msg::Search { seq, issues } => {
                assert_eq!(*seq, 3);
                assert_eq!(issues.len(), 1);
                assert_eq!(issues[0].identifier, "ENG-1");
            }
            other => panic!("{other:?}"),
        }
        assert!(fx.rt.store.get_issue("ENG-1").unwrap().is_some());
    }

    #[test]
    fn detail_sends_cache_then_fresh_and_marks_viewed() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        fx.rt
            .store
            .upsert_issues(&[issue("a", "ENG-1", "옛 제목")], T0)
            .unwrap();
        let mut detail = IssueBuilder::new("a", "ENG-1", "새 제목").json();
        detail["comments"] = json!({
            "nodes": [ { "id": "c1", "body": "확인했어요", "createdAt": "2026-10-02T00:00:00.000Z", "editedAt": null, "user": null } ],
            "pageInfo": { "hasNextPage": true, "endCursor": "c" }
        });
        mock(&mut fx.server, "Detail", json!({ "issue": detail }));
        let first = fx.rt.execute(Effect::OpenDetail("ENG-1".into()), T0);
        assert!(
            matches!(&first[0], Msg::Detail { fresh: false, issue, .. } if issue.title == "옛 제목")
        );
        assert_eq!(fx.rt.store.recent_viewed(10).unwrap().len(), 1);
        match &settle(&mut fx.rt, T0)[0] {
            Msg::Detail {
                id,
                issue,
                comments,
                more,
                fresh,
            } => {
                assert_eq!(id, "ENG-1");
                assert_eq!(issue.title, "새 제목");
                assert_eq!(comments[0].body, "확인했어요");
                assert!(*more && *fresh);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(fx.rt.store.get_comments("a").unwrap().unwrap().0.len(), 1);
    }

    #[test]
    fn missing_issue_is_gone_and_removed_from_cache() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        fx.rt
            .store
            .upsert_issues(&[issue("a", "ENG-1", "지워질 이슈")], T0)
            .unwrap();
        fx.server
            .mock("POST", "/graphql")
            .with_body(r#"{"data":null,"errors":[{"message":"Entity not found: Issue"}]}"#)
            .create();
        fx.rt.execute(Effect::OpenDetail("ENG-1".into()), T0);
        let msgs = settle(&mut fx.rt, T0);
        assert_eq!(
            msgs,
            vec![Msg::DetailGone("ENG-1".into()), Msg::Index(vec![])]
        );
        assert!(fx.rt.store.get_issue("ENG-1").unwrap().is_none());
    }

    #[test]
    fn env_key_auth_failure_reports_env() {
        let mut fx = fixture(Some(KeySource::Env), true, Origin::default());
        fx.server
            .mock("POST", "/graphql")
            .with_status(401)
            .with_body("unauthorized")
            .create();
        fx.rt.execute(Effect::LoadTab(Tab::Mine), T0);
        assert_eq!(settle(&mut fx.rt, T0), vec![Msg::AuthFailed { env: true }]);
    }

    #[test]
    fn valid_key_is_saved_and_used() {
        let mut fx = fixture(None, false, Origin::default());
        mock(&mut fx.server, "Viewer", json!({ "viewer": viewer_json() }));
        fx.rt.execute(Effect::ValidateKey(KEY.into()), T0);
        assert_eq!(settle(&mut fx.rt, T0), vec![Msg::KeyOk(viewer())]);
        let saved = std::fs::read_to_string(fx.dir.path().join("config/credentials")).unwrap();
        assert_eq!(saved.trim(), KEY);
        let fp = LinearClient::new(KEY).key_fingerprint();
        assert_eq!(fx.rt.client.as_ref().unwrap().key_fingerprint(), fp);
        assert!(cached_viewer(&fx.rt.store, &fp).unwrap().is_some());
        assert!(
            !fx.rt.execute(Effect::Init, T0).is_empty(),
            "저장한 viewer로 바로 시작한다"
        );
    }

    #[test]
    fn invalid_key_is_rejected() {
        let mut fx = fixture(None, false, Origin::default());
        fx.server
            .mock("POST", "/graphql")
            .with_status(401)
            .with_body("unauthorized")
            .create();
        fx.rt.execute(Effect::ValidateKey("lin_api_bad".into()), T0);
        assert_eq!(
            settle(&mut fx.rt, T0),
            vec![Msg::KeyBad(
                "키가 유효하지 않아요. Linear에서 키를 다시 확인하세요".into()
            )]
        );
        assert!(fx.rt.client.is_none());
        assert!(!fx.dir.path().join("config/credentials").exists());
    }

    #[test]
    fn open_and_copy_go_through_system() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        assert_eq!(
            fx.rt.execute(
                Effect::OpenUrl("https://linear.app/acme/issue/ENG-1".into()),
                T0
            ),
            vec![Msg::Flash("브라우저에서 열었어요".into())]
        );
        assert_eq!(
            fx.rt
                .execute(Effect::OpenUrl("file:///etc/passwd".into()), T0),
            vec![Msg::Flash("http(s) 링크만 열 수 있어요".into())]
        );
        assert_eq!(
            fx.rt.execute(
                Effect::Copy {
                    text: "ENG-1".into(),
                    what: "ID".into()
                },
                T0
            ),
            vec![Msg::Flash("복사됨: ID".into())]
        );
        assert_eq!(
            *fx.sys.0.lock().unwrap(),
            vec![
                "open https://linear.app/acme/issue/ENG-1".to_string(),
                "copy ENG-1".to_string()
            ]
        );
    }

    fn git_repo(branch: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["commit", "-q", "--allow-empty", "-m", "init"]);
        git(&["checkout", "-q", "-b", branch]);
        dir
    }

    fn origin_at(dir: &Path) -> Origin {
        Origin {
            cwd: Some(dir.to_path_buf()),
            ..Origin::default()
        }
    }

    #[test]
    fn pinned_issue_from_branch_name_uses_cache_and_remembers() {
        let repo = git_repo("me/eng-5-login");
        let mut fx = fixture(Some(KeySource::File), true, origin_at(repo.path()));
        fx.rt.execute(Effect::Init, T0);
        fx.rt
            .store
            .upsert_issues(&[issue("e5", "ENG-5", "로그인")], T0)
            .unwrap();
        assert!(fx.rt.execute(Effect::ResolvePinned, T0).is_empty());
        let msgs = settle(&mut fx.rt, T0);
        assert!(matches!(&msgs[..], [Msg::Pinned(Some(i))] if i.identifier == "ENG-5"));
        let (repo_root, _) = current_branch(repo.path()).unwrap();
        assert_eq!(
            fx.rt
                .store
                .branch_get(&repo_root, "me/eng-5-login")
                .unwrap(),
            Some((Some("ENG-5".to_string()), T0))
        );
    }

    #[test]
    fn pinned_issue_falls_back_to_branch_search() {
        let repo = git_repo("feature/login");
        let mut fx = fixture(Some(KeySource::File), true, origin_at(repo.path()));
        let m = fx
            .server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Branch\\b".into()))
            .with_body(
                json!({ "data": { "issueVcsBranchSearch": issue("e9", "ENG-9", "브랜치 검색") } })
                    .to_string(),
            )
            .expect(1)
            .create();
        fx.rt.execute(Effect::Init, T0);
        fx.rt.execute(Effect::ResolvePinned, T0);
        let msgs = settle(&mut fx.rt, T0);
        assert!(matches!(&msgs[..], [Msg::Pinned(Some(i))] if i.identifier == "ENG-9"));
        // 10분 안에는 다시 묻지 않는다
        fx.rt.execute(Effect::ResolvePinned, T0 + 60_000);
        let again = settle(&mut fx.rt, T0 + 60_000);
        assert!(matches!(&again[..], [Msg::Pinned(Some(i))] if i.identifier == "ENG-9"));
        m.assert();
    }

    #[test]
    fn no_origin_means_no_pinned_issue() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        assert_eq!(
            fx.rt.execute(Effect::ResolvePinned, T0),
            vec![Msg::Pinned(None)]
        );
    }

    #[test]
    fn load_more_uses_the_cursor() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        fx.rt.execute(Effect::Init, T0);
        mock(
            &mut fx.server,
            "Issues",
            page(&[issue("a", "ENG-1", "첫 페이지")], Some("c1")),
        );
        fx.rt.execute(Effect::LoadTab(Tab::All), T0);
        settle(&mut fx.rt, T0);
        fx.server.reset();
        let m = fx
            .server
            .mock("POST", "/graphql")
            .match_body(Matcher::PartialJson(
                json!({ "variables": { "after": "c1" } }),
            ))
            .with_body(
                json!({ "data": page(&[issue("b", "ENG-2", "둘째 페이지")], None) }).to_string(),
            )
            .create();
        fx.rt.execute(Effect::LoadMore, T0);
        match &settle(&mut fx.rt, T0)[0] {
            Msg::Tab {
                tab,
                issues,
                append,
                has_more,
                ..
            } => {
                assert_eq!(*tab, Tab::All);
                assert!(*append && !*has_more);
                assert_eq!(issues[0].identifier, "ENG-2");
            }
            other => panic!("{other:?}"),
        }
        m.assert();
    }

    #[test]
    fn low_remaining_requests_throttle_auto_search_once() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        let reset = T0 + 30 * 60_000;
        fx.server
            .mock("POST", "/graphql")
            .with_header("x-ratelimit-requests-remaining", "10")
            .with_header("x-ratelimit-requests-reset", &reset.to_string())
            .with_body(json!({ "data": page(&[], None) }).to_string())
            .create();
        let search = |seq| Effect::Search {
            seq,
            query: "결제".into(),
        };
        fx.rt.execute(search(1), T0);
        let msgs = settle(&mut fx.rt, T0);
        assert_eq!(msgs.last(), Some(&Msg::Throttled(reset)));
        fx.rt.execute(search(2), T0);
        let msgs = settle(&mut fx.rt, T0);
        assert!(
            !msgs.iter().any(|m| matches!(m, Msg::Throttled(_))),
            "한 번만 알린다"
        );
    }

    #[test]
    fn deep_search_limit_explains_without_pausing() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        fx.server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_header(
                "x-ratelimit-requests-reset",
                &(T0 + 50 * 60_000).to_string(),
            )
            .with_body(
                r#"{"errors":[{"message":"rate limited","extensions":{"code":"RATELIMITED"}}]}"#,
            )
            .create();
        fx.rt.execute(
            Effect::DeepSearch {
                seq: 4,
                query: "세션".into(),
            },
            T0,
        );
        assert_eq!(
            settle(&mut fx.rt, T0),
            vec![
                Msg::Search {
                    seq: 4,
                    issues: vec![]
                },
                Msg::Flash(DEEP_LIMIT_TEXT.into()),
            ],
            "50분 뒤 리셋 시각으로 자동 검색을 멈추지 않는다"
        );
    }

    #[test]
    fn only_web_urls_open() {
        assert!(is_web_url("https://linear.app/x"));
        assert!(is_web_url("HTTP://example.com"));
        assert!(!is_web_url("file:///etc/passwd"));
        assert!(!is_web_url("-a Calculator"));
        assert!(!is_web_url("vscode://open"));
    }
}
