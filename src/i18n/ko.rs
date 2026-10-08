//! 한국어. 값은 옮기기 전 코드의 문자열과 한 글자도 다르지 않다.

use super::Texts;

pub const KO: Texts = Texts {
    cli_about: "herdr에서 Linear를 빠르게 조회",
    cli_login: "API 키를 입력하고 검증해 저장한다",
    cli_logout: "저장된 API 키와 캐시를 지운다",
    cli_whoami: "연결된 계정과 워크스페이스",
    cli_mine: "나에게 할당된 열린 이슈",
    cli_search: "이슈 검색 (예: 로그인 l:bug s:진행 @나 #ENG p:high)",
    cli_search_deep: "서버 깊은 검색 (코멘트 포함, 분당 30회 제한)",
    cli_show: "이슈 상세 (예: ENG-131)",
    cli_open: "herdr 액션: 원래 pane의 맥락을 넘겨 팔레트를 띄우거나 사이드 pane을 열고 닫는다",
    cli_open_target: "palette: 단축키·명령 팔레트, url: Linear 이슈 링크 Ctrl+클릭, side: 사이드 pane 열기/닫기",
    cli_ui: "herdr pane 안에서 도는 화면",
    write_failed: |e| format!("오류: 출력하지 못했어요: {e}"),
    error_line: |m| format!("오류: {m}"),
    warning_line: |w| format!("경고: {w}"),
    no_api_key: "API 키가 없어요. 먼저 `herdr-linear login`을 실행하세요",
    no_config_dir: "설정 디렉터리를 정할 수 없어요 (HOME이 없어요)",
    no_state_dir: "상태 디렉터리를 정할 수 없어요 (HOME이 없어요)",
    config_read_failed: |e| format!("config.toml을 읽지 못했어요: {e}"),
    config_invalid: |e| format!("config.toml 형식이 잘못돼서 기본값을 써요: {e}"),
    teams_not_list: "teams는 문자열 배열이어야 해요. 기본값을 써요",
    template_not_string: "agent.template은 문자열이어야 해요. 기본값을 써요",
    int_out_of_range: |section, key, min, max| {
        format!("{section}.{key}는 {min}~{max} 사이의 정수여야 해요. 기본값을 써요")
    },
    language_unsupported: |v| {
        format!("language = \"{v}\"는 지원하지 않아요. 영어를 써요 (en, ko, ja, zh-CN, de)")
    },
    language_not_string: "language는 문자열이어야 해요. 영어를 써요",
    read_failed: |p| format!("{p}을 읽지 못했어요"),
    empty_key: "빈 키는 저장할 수 없어요",
    delete_failed: |p| format!("{p}을 지우지 못했어요"),
    mkdir_failed: |p| format!("{p} 디렉터리를 만들지 못했어요"),
};
