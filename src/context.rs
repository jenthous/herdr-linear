//! 팔레트를 연 맥락: 원래 pane, 그 pane의 작업 디렉터리와 브랜치, 바로 열 이슈.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::search::query::parse_identifier;

/// 액션이 팔레트 pane에 넘기는 환경 변수.
pub const ENV_PANE: &str = "HERDR_LINEAR_ORIGIN_PANE";
pub const ENV_AGENT: &str = "HERDR_LINEAR_ORIGIN_AGENT";
pub const ENV_CWD: &str = "HERDR_LINEAR_ORIGIN_CWD";
pub const ENV_OPEN: &str = "HERDR_LINEAR_OPEN";

/// `HERDR_PLUGIN_CONTEXT_JSON` 중 쓰는 필드.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct PluginContext {
    pub focused_pane_id: Option<String>,
    pub focused_pane_agent: Option<String>,
    pub focused_pane_cwd: Option<String>,
    pub workspace_cwd: Option<String>,
    pub workspace_id: Option<String>,
    pub selected_text: Option<String>,
    pub clicked_url: Option<String>,
}

impl PluginContext {
    /// 해석하지 못하면 빈 컨텍스트.
    pub fn parse(json: &str) -> PluginContext {
        serde_json::from_str(json).unwrap_or_default()
    }
}

/// 팔레트를 연 원래 pane의 정보.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Origin {
    pub pane_id: Option<String>,
    pub agent: Option<String>,
    pub cwd: Option<PathBuf>,
    /// 바로 열 이슈 식별자 (선택한 텍스트나 클릭한 링크에서)
    pub open: Option<String>,
}

impl Origin {
    /// 액션(`open palette`, `open url`)에서 플러그인 컨텍스트로 만든다.
    /// 팝업 프로세스의 작업 디렉터리는 플러그인 루트라서, 원래 pane의 cwd를 따로 챙긴다.
    pub fn from_context(ctx: &PluginContext) -> Origin {
        Origin {
            pane_id: ctx.focused_pane_id.clone(),
            agent: ctx.focused_pane_agent.clone(),
            cwd: ctx
                .focused_pane_cwd
                .clone()
                .or_else(|| ctx.workspace_cwd.clone())
                .map(PathBuf::from),
            open: ctx
                .clicked_url
                .as_deref()
                .and_then(identifier_in_url)
                .or_else(|| ctx.selected_text.as_deref().and_then(identifier_in_text)),
        }
    }

    /// 팔레트 pane에서: 넘겨받은 환경 변수를 먼저 보고, 없으면 컨텍스트 JSON을 본다.
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Origin {
        let ctx = get("HERDR_PLUGIN_CONTEXT_JSON")
            .map(|j| PluginContext::parse(&j))
            .unwrap_or_default();
        let base = Origin::from_context(&ctx);
        let var = |k: &str| get(k).filter(|v| !v.trim().is_empty());
        Origin {
            pane_id: var(ENV_PANE).or(base.pane_id),
            agent: var(ENV_AGENT).or(base.agent),
            cwd: var(ENV_CWD).map(PathBuf::from).or(base.cwd),
            open: var(ENV_OPEN).or(base.open),
        }
    }

    /// `herdr plugin pane open --env`로 넘길 값. 없는 값은 넘기지 않는다.
    pub fn to_env(&self) -> Vec<(String, String)> {
        let cwd = self.cwd.as_ref().map(|p| p.display().to_string());
        [
            (ENV_PANE, self.pane_id.as_ref()),
            (ENV_AGENT, self.agent.as_ref()),
            (ENV_CWD, cwd.as_ref()),
            (ENV_OPEN, self.open.as_ref()),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k.to_string(), v.clone())))
        .collect()
    }
}

/// 텍스트 전체가 이슈 식별자 하나면 (앞뒤 공백 무시) 대문자 식별자.
pub fn identifier_in_text(text: &str) -> Option<String> {
    parse_identifier(text.trim()).map(|(key, n)| format!("{key}-{n}"))
}

/// `https://linear.app/<워크스페이스>/issue/<식별자>/<슬러그>`에서 식별자.
pub fn identifier_in_url(url: &str) -> Option<String> {
    let rest = url.split_once("/issue/")?.1;
    identifier_in_text(rest.split(['/', '?', '#']).next()?)
}

static BRANCH_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([a-z][a-z0-9]*)-([0-9]+)").expect("valid regex"));

/// 브랜치명에서 내 팀 키의 이슈 식별자. `me/eng-123-fix` → `ENG-123`.
pub fn identifier_in_branch(branch: &str, team_keys: &[String]) -> Option<String> {
    BRANCH_ID.captures_iter(branch).find_map(|cap| {
        let key = cap[1].to_uppercase();
        team_keys
            .iter()
            .any(|k| k.eq_ignore_ascii_case(&key))
            .then(|| format!("{key}-{}", &cap[2]))
    })
}

/// git 저장소 루트와 현재 브랜치. 저장소가 아니거나 detached HEAD면 `None`.
pub fn current_branch(cwd: &Path) -> Option<(String, String)> {
    let run = |args: &[&str]| -> Option<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
        (!s.is_empty()).then_some(s)
    };
    let repo = run(&["rev-parse", "--show-toplevel"])?;
    let branch = run(&["rev-parse", "--abbrev-ref", "HEAD"])?;
    (branch != "HEAD").then_some((repo, branch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn context_prefers_pane_cwd_and_clicked_link() {
        let ctx = PluginContext::parse(
            r#"{"focused_pane_id":"w1:p2","focused_pane_agent":"claude","focused_pane_cwd":"/repo/app","workspace_cwd":"/repo","selected_text":" eng-7 ","clicked_url":"https://linear.app/acme/issue/OPS-12/fix-it"}"#,
        );
        let o = Origin::from_context(&ctx);
        assert_eq!(o.pane_id.as_deref(), Some("w1:p2"));
        assert_eq!(o.agent.as_deref(), Some("claude"));
        assert_eq!(o.cwd, Some(PathBuf::from("/repo/app")));
        assert_eq!(o.open.as_deref(), Some("OPS-12"));
    }

    #[test]
    fn selected_text_opens_issue_and_workspace_cwd_is_fallback() {
        let ctx = PluginContext::parse(r#"{"workspace_cwd":"/repo","selected_text":"eng-7"}"#);
        let o = Origin::from_context(&ctx);
        assert_eq!(o.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(o.open.as_deref(), Some("ENG-7"));
        let ctx = PluginContext::parse(r#"{"selected_text":"그냥 문장"}"#);
        assert_eq!(Origin::from_context(&ctx).open, None);
    }

    #[test]
    fn broken_json_is_empty_context() {
        assert_eq!(PluginContext::parse("not json"), PluginContext::default());
    }

    #[test]
    fn env_overrides_context_json() {
        let o = Origin::from_env(env(&[
            (
                "HERDR_PLUGIN_CONTEXT_JSON",
                r#"{"focused_pane_id":"w1:p1","focused_pane_cwd":"/a"}"#,
            ),
            (ENV_CWD, "/b"),
            (ENV_OPEN, "ENG-9"),
        ]));
        assert_eq!(o.pane_id.as_deref(), Some("w1:p1"));
        assert_eq!(o.cwd, Some(PathBuf::from("/b")));
        assert_eq!(o.open.as_deref(), Some("ENG-9"));
    }

    #[test]
    fn to_env_round_trips() {
        let o = Origin {
            pane_id: Some("w1:p1".into()),
            agent: None,
            cwd: Some(PathBuf::from("/repo app")),
            open: Some("ENG-1".into()),
        };
        let pairs = o.to_env();
        assert_eq!(pairs.len(), 3);
        let lookup: HashMap<String, String> = pairs.into_iter().collect();
        let back = Origin::from_env(|k| lookup.get(k).cloned());
        assert_eq!(back, o);
    }

    #[test]
    fn identifiers_from_urls_and_branches() {
        assert_eq!(
            identifier_in_url("https://linear.app/acme/issue/eng-12/some-title?x=1").as_deref(),
            Some("ENG-12")
        );
        assert_eq!(identifier_in_url("https://linear.app/acme/project/x"), None);
        let keys = vec!["ENG".to_string()];
        assert_eq!(
            identifier_in_branch("me/eng-123-fix-login", &keys).as_deref(),
            Some("ENG-123")
        );
        assert_eq!(
            identifier_in_branch("feature-12-eng-34", &keys).as_deref(),
            Some("ENG-34")
        );
        assert_eq!(identifier_in_branch("feature-12", &keys), None);
        assert_eq!(identifier_in_branch("main", &keys), None);
    }

    #[test]
    fn current_branch_reads_git() {
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
        assert_eq!(current_branch(dir.path()), None, "저장소가 아님");
        git(&["init", "-q"]);
        git(&["commit", "-q", "--allow-empty", "-m", "init"]);
        git(&["checkout", "-q", "-b", "me/eng-5-x"]);
        let (repo, branch) = current_branch(dir.path()).unwrap();
        assert_eq!(branch, "me/eng-5-x");
        assert!(PathBuf::from(repo).ends_with(dir.path().file_name().unwrap()));
        git(&["checkout", "-q", "--detach"]);
        assert_eq!(current_branch(dir.path()), None, "detached HEAD");
    }

    #[test]
    fn context_reads_the_workspace_id() {
        let ctx = PluginContext::parse(r#"{"workspace_id":"w1","focused_pane_id":"w1:p2"}"#);
        assert_eq!(ctx.workspace_id.as_deref(), Some("w1"));
        assert_eq!(PluginContext::parse("{}").workspace_id, None);
    }
}
