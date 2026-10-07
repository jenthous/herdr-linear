//! herdr 팝업 팔레트 TUI.

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
use crate::linear::client::LinearClient;
use crate::log::Logger;
use app::{App, Effect, Msg};
use runtime::{MakeClient, Runtime};
use system::RealSystem;

/// `ui --mode palette`: herdr popup 안에서 팔레트를 띄운다.
/// 키가 없으면 키 입력 화면으로, `HERDR_LINEAR_OPEN`이 있으면 그 이슈 상세로 시작한다.
/// 화면을 띄우기 전에 실패하면 로그에도 남긴다 (pane이 닫히면 오류 출력은 남지 않는다).
pub fn palette(paths: Paths) -> Result<()> {
    let log = Logger::new(paths.log_file());
    let (rt, app, effects) = log.on_err(
        "팔레트 시작",
        prepare(paths, std::env::var("LINEAR_API_KEY").ok()),
    )?;
    runtime::run(rt, app, effects)
}

/// 설정·키·캐시를 읽어 런타임과 앱을 만든다.
fn prepare(paths: Paths, env_key: Option<String>) -> Result<(Runtime, App, Vec<Effect>)> {
    let (settings, warnings) = config::load_settings(&paths.config_file());
    let key = config::resolve_api_key(env_key, &paths, &config::default_credential_fallbacks())?;
    let now = now_ms();
    let store = open_store(&paths, &settings, now)?;
    let origin = Origin::from_env(|k| std::env::var(k).ok());
    let (mut app, effects) = match &key {
        Some(_) => App::start(origin.open.clone()),
        None => (App::onboarding(false), Vec::new()),
    };
    if !warnings.is_empty() {
        app.apply(
            Msg::Warn(format!("설정 경고: {}", warnings.join(" · "))),
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
            log.on_err("팔레트 시작", prepare(paths.clone(), None))
                .is_err()
        );
        let text = std::fs::read_to_string(paths.log_file()).unwrap();
        assert!(text.contains("팔레트 시작: "), "{text}");
    }
}
