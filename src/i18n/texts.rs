//! 문구 카탈로그의 모양. 언어마다 이 struct를 하나씩 채운다(`en.rs`, `ko.rs`, …).
//! 고정 문구는 `&'static str`, 값이 들어가는 문구는 `fn(…) -> String`이다.
//! 필드를 하나라도 빠뜨리면 컴파일되지 않는다.

/// 화면·CLI·알림·로그에 쓰는 문구.
pub struct Texts {
    // ── CLI 도움말 (`cli::command`) ──
    pub cli_about: &'static str,
    pub cli_login: &'static str,
    pub cli_logout: &'static str,
    pub cli_whoami: &'static str,
    pub cli_mine: &'static str,
    pub cli_search: &'static str,
    pub cli_search_deep: &'static str,
    pub cli_show: &'static str,
    pub cli_open: &'static str,
    pub cli_open_target: &'static str,
    pub cli_ui: &'static str,
    // ── 실행 결과 (`main`, `cli::Ctx::open`) ──
    /// 표준 출력에 쓰지 못했다 (오류 문구)
    pub write_failed: fn(&str) -> String,
    /// 명령이 실패했다 (오류 문구)
    pub error_line: fn(&str) -> String,
    /// 경고 한 줄
    pub warning_line: fn(&str) -> String,
    pub no_api_key: &'static str,
    // ── 설정 (`config`) ──
    pub no_config_dir: &'static str,
    pub no_state_dir: &'static str,
    /// config.toml을 읽지 못했다 (오류 문구)
    pub config_read_failed: fn(&str) -> String,
    /// config.toml 형식이 잘못됐다 (오류 문구)
    pub config_invalid: fn(&str) -> String,
    pub teams_not_list: &'static str,
    pub template_not_string: &'static str,
    /// (구역, 키, 최솟값, 최댓값)
    pub int_out_of_range: fn(&str, &str, u64, u64) -> String,
    /// 지원하지 않는 `language` 값
    pub language_unsupported: fn(&str) -> String,
    pub language_not_string: &'static str,
    /// 파일을 읽지 못했다 (경로)
    pub read_failed: fn(&str) -> String,
    pub empty_key: &'static str,
    /// 파일을 지우지 못했다 (경로)
    pub delete_failed: fn(&str) -> String,
    /// 디렉터리를 만들지 못했다 (경로)
    pub mkdir_failed: fn(&str) -> String,
}
