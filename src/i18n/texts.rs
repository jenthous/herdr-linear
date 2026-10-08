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
    // ── 앱 (`tui::app`) ──
    pub tab_mine: &'static str,
    pub tab_recent: &'static str,
    pub tab_all: &'static str,
    pub deep_limit: &'static str,
    pub menu_actions: &'static str,
    pub menu_copy_url: &'static str,
    /// PR 이름 (`PR #12`)
    pub menu_copy_pr: fn(&str) -> String,
    pub menu_copy_id: &'static str,
    pub menu_open: &'static str,
    pub menu_browser: &'static str,
    pub menu_links: &'static str,
    pub menu_relations: &'static str,
    pub menu_deep_search: &'static str,
    pub menu_refresh: &'static str,
    pub menu_back: &'static str,
    pub menu_close: &'static str,
    pub links_title: &'static str,
    pub links_none: &'static str,
    pub relations_title: &'static str,
    pub relations_loading: &'static str,
    pub relations_failed: &'static str,
    pub relations_none: &'static str,
    /// 이슈 식별자
    pub no_open_pr: fn(&str) -> String,
    /// (이름, 워크스페이스 이름)
    pub connected: fn(&str, &str) -> String,
    pub key_expired_paste: &'static str,
    /// 다시 시도할 수 있을 때까지 남은 분
    pub rate_limited_retry_in: fn(i64) -> String,
    pub rate_limited_retry_later: &'static str,
    /// 자동 서버 검색을 멈추는 분
    pub throttled: fn(i64) -> String,
    // ── 런타임 (`tui::runtime`) ──
    pub key_invalid: &'static str,
    pub key_offline: &'static str,
    /// 오류 문구
    pub key_save_failed: fn(&str) -> String,
    pub only_web_links: &'static str,
    pub opened_in_browser: &'static str,
    /// 오류 문구
    pub browser_failed: fn(&str) -> String,
    /// 복사한 것 (`ENG-1 URL`)
    pub copied: fn(&str) -> String,
    /// 오류 문구
    pub copy_failed: fn(&str) -> String,
    // ── 로그 (`tui::runtime`, `tui`) ──
    pub log_viewer: &'static str,
    pub log_list: &'static str,
    pub log_search: &'static str,
    pub log_detail: &'static str,
    pub log_branch: &'static str,
    pub log_deep_limited: &'static str,
    pub log_cache_error: fn(&str) -> String,
    pub log_key_save_failed: fn(&str) -> String,
    pub log_browser_failed: fn(&str) -> String,
    pub log_copy_failed: fn(&str) -> String,
    pub log_exit: fn(&str) -> String,
    pub log_panic: fn(&str) -> String,
    pub log_palette_start: &'static str,
    pub log_side_start: &'static str,
    /// (pane id, 워크스페이스 id)
    pub log_side_started: fn(&str, &str) -> String,
    pub log_side_no_pane: &'static str,
    pub log_side_record: &'static str,
    pub log_side_record_remove: &'static str,
    /// 설정 경고들을 ` · `로 이은 것
    pub settings_warning: fn(&str) -> String,
    // ── 화면 (`tui::view`) ──
    pub status_updating: &'static str,
    pub status_offline: &'static str,
    pub status_error: &'static str,
    /// 마지막 갱신 (`ago`의 결과)
    pub status_updated: fn(&str) -> String,
    pub search_placeholder: &'static str,
    pub search_prompt: &'static str,
    pub loading: &'static str,
    pub no_results: &'static str,
    pub pinned_header: &'static str,
    pub deep_search_row: &'static str,
    /// 우선순위 이름
    pub priority_named: fn(&str) -> String,
    pub no_assignee: &'static str,
    /// PR 이름 (`PR #12`)
    pub pr_open: fn(&str) -> String,
    pub pr_draft: &'static str,
    /// 프로젝트 이름
    pub project_named: fn(&str) -> String,
    /// 사이클 이름이나 번호
    pub cycle_named: fn(&str) -> String,
    /// 상위 이슈 식별자
    pub parent_named: fn(&str) -> String,
    /// 예상 점수
    pub estimate_named: fn(&str) -> String,
    /// 마감일
    pub due_named: fn(&str) -> String,
    pub no_body: &'static str,
    /// (코멘트 수, 더 있는지)
    pub comments_header: fn(usize, bool) -> String,
    pub unknown_user: &'static str,
    pub more_comments_tui: &'static str,
    pub comments_loading: &'static str,
    /// 이슈 식별자
    pub detail_gone_id: fn(&str) -> String,
    /// 이슈 식별자
    pub detail_loading_id: fn(&str) -> String,
    /// 이슈 식별자
    pub detail_failed_id: fn(&str) -> String,
    pub detail_gone: &'static str,
    pub hints_search: &'static str,
    pub hints_list: &'static str,
    pub hints_detail: &'static str,
    pub hints_onboarding: &'static str,
    /// 오프라인 이유
    pub offline_footer: fn(&str) -> String,
    /// 오류 문구
    pub error_footer: fn(&str) -> String,
    pub onboarding_title: &'static str,
    pub env_key_invalid: &'static str,
    pub env_key_fix: &'static str,
    pub esc_close: &'static str,
    pub paste_key: &'static str,
    pub checking: &'static str,
    /// 키 입력 칸 이름표. 커서 위치가 이 폭을 따른다
    pub key_label: &'static str,
    pub confirm_close: &'static str,
}
