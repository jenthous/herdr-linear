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
use app::{App, Msg};
use runtime::{MakeClient, Runtime};
use system::RealSystem;

/// `ui --mode palette`: herdr popup 안에서 팔레트를 띄운다.
/// 키가 없으면 키 입력 화면으로, `HERDR_LINEAR_OPEN`이 있으면 그 이슈 상세로 시작한다.
pub fn palette(paths: Paths) -> Result<()> {
    let (settings, warnings) = config::load_settings(&paths.config_file());
    let key = config::resolve_api_key(
        std::env::var("LINEAR_API_KEY").ok(),
        &paths,
        &config::default_credential_fallbacks(),
    )?;
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
    runtime::run(rt, app, effects)
}
