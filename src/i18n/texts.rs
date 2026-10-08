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
    // ── 우선순위·시각 (`ui::style`) ──
    pub priority_urgent: &'static str,
    pub priority_high: &'static str,
    pub priority_medium: &'static str,
    pub priority_low: &'static str,
    pub priority_none: &'static str,
    pub just_now: &'static str,
    /// n분 전
    pub minutes_ago: fn(i64) -> String,
    /// n시간 전
    pub hours_ago: fn(i64) -> String,
    /// n일 전
    pub days_ago: fn(i64) -> String,
    // ── 관계 칸 (`ui::relations`) ──
    pub rel_parent: &'static str,
    pub rel_blocked_by: &'static str,
    pub rel_blocking: &'static str,
    pub rel_related: &'static str,
    pub rel_child: &'static str,
    /// 하위가 한 번에 받는 개수(n)를 넘는다
    pub children_over: fn(usize) -> String,
    /// 하위 n개가 모두 끝났다
    pub children_all_done: fn(usize) -> String,
    /// (전체, 남은 수) → (흐리게 그릴지, 글자) 조각들. 어순이 언어마다 달라서 조각 순서를 카탈로그가 정한다
    pub children_left: fn(usize, usize) -> Vec<(bool, String)>,
    /// 하위가 더 있을 때의 마지막 줄 (앞에 칸 이름 폭만큼 공백이 붙는다)
    pub more_children: &'static str,
    // ── 마크다운 (`markdown`) ──
    /// 본문의 이미지 자리 (번호, 이름)
    pub image_placeholder: fn(usize, &str) -> String,
    // ── Linear 오류 (`linear::client`) ──
    pub api_rate_limited: &'static str,
    pub api_auth: &'static str,
    /// Linear가 준 오류 문구
    pub api_graphql: fn(&str) -> String,
    /// 연결 오류 문구
    pub api_offline: fn(&str) -> String,
    /// 해석 오류 문구
    pub api_decode: fn(&str) -> String,
    /// HTTP 상태 코드
    pub server_error: fn(u16) -> String,
    pub no_data: &'static str,
    // ── 캐시·외부 프로그램 (`store`, `tui::system`, `herdr`) ──
    pub cache_recreate_failed: &'static str,
    pub cache_open_failed: &'static str,
    /// 프로그램을 실행하지 못했다 (프로그램 이름·경로)
    pub run_failed: fn(&str) -> String,
}
