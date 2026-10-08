//! herdr 팝업 팔레트와 사이드 pane TUI. 둘은 같은 화면이다.

pub mod app;
pub mod keys;
pub mod runtime;
pub mod system;
pub mod view;

use std::sync::Arc;

use anyhow::Result;

use crate::cli::{now_ms, open_store};
use crate::config::{self, Paths};
use crate::context::Origin;
use crate::herdr::Herdr;
use crate::i18n::t;
use crate::linear::client::LinearClient;
use crate::log::Logger;
use crate::side::SidePanes;
use app::{App, Effect, Msg};
use runtime::{MakeClient, Runtime};
use system::RealSystem;

/// `ui --mode palette`: herdr popup 안에서 팔레트를 띄운다.
/// 키가 없으면 키 입력 화면으로, `HERDR_LINEAR_OPEN`이 있으면 그 이슈 상세로 시작한다.
/// 화면을 띄우기 전에 실패하면 로그에도 남긴다 (pane이 닫히면 오류 출력은 남지 않는다).
pub fn palette(paths: Paths) -> Result<()> {
    let log = Logger::new(paths.log_file());
    let (rt, app, effects) = log.on_err(
        t().log_palette_start,
        prepare(paths, std::env::var("LINEAR_API_KEY").ok(), false),
    )?;
    runtime::run(rt, app, effects)
}

/// `ui --mode side`: 사이드 pane에서 팔레트 화면을 상시로 띄운다.
/// 시작할 때 자기 pane을 `side-panes.json`에 적고 끝날 때 지운다 (`open side`가 이 기록으로 닫는다).
pub fn side(paths: Paths) -> Result<()> {
    let log = Logger::new(paths.log_file());
    let panes = SidePanes::new(&paths.state_dir);
    let me = own_pane(|k| std::env::var(k).ok(), &Herdr::from_env());
    match &me {
        Some((pane, workspace)) => {
            log.write(&(t().log_side_started)(pane, workspace));
            let _ = log.on_err(t().log_side_record, panes.set(workspace, pane));
        }
        None => log.write(t().log_side_no_pane),
    }
    let result = log
        .on_err(
            t().log_side_start,
            prepare(paths, std::env::var("LINEAR_API_KEY").ok(), true),
        )
        .and_then(|(rt, app, effects)| runtime::run(rt, app, effects));
    if let Some((pane, workspace)) = &me {
        let _ = log.on_err(t().log_side_record_remove, panes.remove(workspace, pane));
    }
    result
}

/// 사이드 pane 자신의 (pane id, 워크스페이스 id). herdr가 준 환경 변수를 먼저 쓰고,
/// 없는 값은 `herdr pane current`로 얻는다. 그것도 안 되면 `None`.
fn own_pane(get: impl Fn(&str) -> Option<String>, herdr: &Herdr) -> Option<(String, String)> {
    let var = |k: &str| get(k).filter(|v| !v.trim().is_empty());
    if let (Some(pane), Some(workspace)) = (var("HERDR_PANE_ID"), var("HERDR_WORKSPACE_ID")) {
        return Some((pane, workspace));
    }
    let (pane, workspace) = herdr.current_pane().ok()?;
    Some((
        var("HERDR_PANE_ID").unwrap_or(pane),
        var("HERDR_WORKSPACE_ID").unwrap_or(workspace),
    ))
}

/// 설정·키·캐시를 읽어 런타임과 앱을 만든다. `side`면 앱을 사이드 모드로 만든다.
fn prepare(
    paths: Paths,
    env_key: Option<String>,
    side: bool,
) -> Result<(Runtime, App, Vec<Effect>)> {
    let (settings, warnings) = config::load_settings(&paths.config_file());
    let key = config::resolve_api_key(env_key, &paths, &config::default_credential_fallbacks())?;
    let now = now_ms();
    let store = open_store(&paths, &settings, now)?;
    let origin = Origin::from_env(|k| std::env::var(k).ok());
    let (mut app, effects) = match &key {
        Some(_) => App::start(origin.open.clone()),
        None => (App::onboarding(false), Vec::new()),
    };
    if side {
        app = app.into_side(settings.side_refresh_seconds);
    }
    if !warnings.is_empty() {
        app.apply(
            Msg::Warn((t().settings_warning)(&warnings.join(" · "))),
            now,
        );
    }
    let make_client: MakeClient = Arc::new(|k: String| LinearClient::new(k));
    let rt = Runtime::new(
        paths,
        settings,
        store,
        key,
        origin,
        Box::new(RealSystem),
        make_client,
    );
    Ok((rt, app, effects))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn start_failure_is_logged() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            state_dir: dir.path().join("state"),
        };
        // 키 파일 자리에 디렉터리가 있어서 키를 읽을 수 없다
        std::fs::create_dir_all(paths.credentials_file()).unwrap();
        let log = Logger::new(paths.log_file());
        assert!(
            log.on_err("팔레트 시작", prepare(paths.clone(), None, false))
                .is_err()
        );
        let text = std::fs::read_to_string(paths.log_file()).unwrap();
        assert!(text.contains("팔레트 시작: "), "{text}");
    }

    #[test]
    fn side_mode_app_comes_from_prepare() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            state_dir: dir.path().join("state"),
        };
        let (_, app, _) = prepare(paths.clone(), Some("lin_api_test".into()), true).unwrap();
        assert!(app.side);
        let (_, app, _) = prepare(paths, Some("lin_api_test".into()), false).unwrap();
        assert!(!app.side);
    }

    #[test]
    fn own_pane_prefers_herdr_env() {
        // 없는 실행 파일이라 herdr에 물으면 실패한다
        let herdr = Herdr::new(PathBuf::from("/nonexistent/herdr"));
        let env = |k: &str| match k {
            "HERDR_PANE_ID" => Some("w1:p3".to_string()),
            "HERDR_WORKSPACE_ID" => Some("w1".to_string()),
            _ => None,
        };
        assert_eq!(own_pane(env, &herdr), Some(("w1:p3".into(), "w1".into())));
        let blank = |k: &str| k.starts_with("HERDR_").then(String::new);
        assert_eq!(own_pane(blank, &herdr), None, "빈 값은 없는 것");
        assert_eq!(own_pane(|_| None, &herdr), None);
    }

    #[cfg(unix)]
    #[test]
    fn own_pane_asks_herdr_without_env() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("herdr");
        std::fs::write(
            &bin,
            "#!/bin/sh\necho '{\"result\":{\"pane\":{\"pane_id\":\"w2:p1\",\"workspace_id\":\"w2\"}}}'\n",
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let herdr = Herdr::new(bin);
        assert_eq!(
            own_pane(|_| None, &herdr),
            Some(("w2:p1".into(), "w2".into()))
        );
        let only_pane = |k: &str| (k == "HERDR_PANE_ID").then(|| "w2:p7".to_string());
        assert_eq!(
            own_pane(only_pane, &herdr),
            Some(("w2:p7".into(), "w2".into()))
        );
    }
}
