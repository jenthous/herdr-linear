# herdr-linear 2부: 팔레트 TUI + herdr 연결 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** herdr에서 단축키로 띄우는 Linear 검색 팔레트(popup)를 만든다. 목록은 Linear 색으로 그린다. 입력 즉시 캐시에서 찾고, 입력이 멈추면 서버에서도 찾는다. 상세(markdown·코멘트), 브라우저 열기·복사, 현재 브랜치 이슈 고정, 링크 Ctrl+클릭을 지원한다.

**Architecture:** `tui::app`은 화면·네트워크와 떨어진 순수 상태 머신이다(`Input`/`Msg` → `Effect`). `tui::runtime`은 `Effect`를 캐시(SQLite, 메인 스레드)와 네트워크(작업 스레드)로 처리해 `Msg`로 돌려준다. `tui::view`는 `App`을 ratatui로 그린다. herdr 액션(`open palette|url`)은 원래 pane의 맥락을 환경 변수로 넘겨 popup pane(`ui --mode palette`)을 띄운다.

**Tech Stack:** 1부와 같다. Rust 2024 edition(rustc 1.94, 최소 1.88), ratatui 0.30.2(crossterm은 `ratatui::crossterm`으로 씀), nucleo-matcher 0.3.1, rusqlite 0.40.2(bundled), pulldown-cmark 0.13.4, ureq 3.4.2, serde_json, regex, chrono를 쓰고, 테스트에는 mockito 1.7.2와 tempfile을 쓴다. **새 의존성은 없다.**

**Spec:** `docs/superpowers/specs/2026-10-06-herdr-linear-design.md`
- 이 계획은 스펙 16장 구현 순서의 4~5단계(팔레트, herdr 연결)를 다룬다. 사이드 패널(`open side`, `ui --mode side`), 변경 동작(상태·라벨·담당자·코멘트·새 이슈), 에이전트 전달은 3부에서 한다.
- 1부 최종 리뷰 후속 항목(`docs/superpowers/notes/2026-10-07-part1-review-followups.md`)의 M1~M7, M9, M10, 권고 5를 Task 1~3, 9에서 처리한다. M8(한도·인증 오류 때 저장된 결과로 대신하기)은 팔레트의 SWR 화면이 맡는다.
- 스펙과 다르게 정한 것:
  1. palette 액션의 contexts에 `selection`을 더한다. 이슈 식별자를 선택한 채로 팔레트를 열 수 있게 하려는 것이다.
  2. `open url`은 `open palette`와 같은 코드를 쓴다. 링크 클릭이면 컨텍스트의 `clicked_url`에서 식별자를 뽑는다.
  3. 이 키로 받은 viewer가 캐시에 없으면(키·워크스페이스 변경), viewer를 받아 캐시의 워크스페이스를 맞출 때까지 캐시를 보이지 않는다.
  4. 깊은 검색이 한도에 걸리면 안내만 하고 자동 검색은 멈추지 않는다(M9).
  5. 브라우저로는 http(s) 링크만 넘긴다.
  6. 최근 본 탭은 열 때마다 다시 읽는다. 이 탭은 상단의 서버 상태(갱신 시각·오프라인)를 바꾸지 않는다.
  7. 실행 중 캐시 손상은 로그만 남기고, 다음 실행 때 `Store::open`이 다시 만든다. 팔레트는 짧게 사는 프로세스이기 때문이다. 오래 도는 사이드 패널(3부)에서 다시 본다.
  8. 1부 리뷰 후속 메모의 "필요하면 처리" 항목 중 `NO_COLOR`만 넣는다(Task 4). CLI 출력의 broken pipe와 화면 외형(목록·인용 안 표의 접두사, 번호 목록 체크박스)은 그대로 둔다.

> 이 계획의 코드는 작성 시점에 임시 크레이트에서 모두 확인했다.
> - 작업마다 끝 상태에서 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`를 통과했다. 마지막에는 221개가 통과하고 성능 테스트 1개는 무시된다. 무시된 성능 테스트(5천 건, 16ms)도 release 빌드로 통과했다.
> - 계획 문서를 처음부터 그대로 따라 하는 시뮬레이션(Step 2 실패, Step 4 통과, fmt·clippy)을 통과했다.
> - 가상 터미널(pty)에서 실제 Linear 데이터로 팔레트를 띄워 목록 색, 미리보기, 선택 이동, 종료 후 터미널 복구를 확인했다.

## 시작 전에

- 작업 브랜치를 만든다: `git switch -c feat/part2-palette` (main `ea8f5a1`에서).
- 각 작업의 단계는 다음과 같다.
  - Step 1: 테스트(와 모듈 등록)를 먼저 바꾼다.
  - Step 2: 테스트가 실패하는 것을 본다. Rust에서는 대개 컴파일 실패로 나타난다.
  - Step 3: 구현을 넣는다.
  - Step 4: 테스트가 통과하는 것을 본다.
  - Step 5: 커밋한다.
- 편집 블록은 다섯 가지다. 쓰인 순서대로 적용한다.
  - "`파일` 파일을 아래 내용으로 만든다": 파일 전체를 쓴다. 새 파일이면 테스트 모듈만 먼저 쓰는 경우가 많다.
  - "`파일` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다": 블록 내용, 빈 줄 하나, 기존 내용 순서가 되게 한다.
  - "`파일`에서 아래 부분을 찾아 … 아래로 바꾼다": 첫 블록은 그 시점의 파일에 정확히 한 번 나온다. 공백까지 그대로 찾아 둘째 블록으로 바꾼다(Edit 도구의 old_string/new_string).
  - "테스트 모듈 위의 구현 전체를 아래로 바꾼다": 파일 처음부터 `#[cfg(test)]`(또는 `#[cfg(all(test, unix))]`) 줄 바로 앞까지를 바꾼다.
  - "테스트 모듈(…부터 파일 끝까지)을 아래로 바꾼다": 그 줄부터 파일 끝까지를 바꾼다.

## Global Constraints

- 1부 Global Constraints가 그대로 적용된다(엔드포인트, `Authorization` 헤더, 키 저장·권한, 경로 순서, 캐시 설정, 타임아웃 연결 5초·전체 15초, 페이지 크기, NFC·표시 폭, 한국어 문구).
- 의존성 버전은 바꾸지 않는다. 새 크레이트를 더하지 않는다. `Cargo.toml`에 `rust-version = "1.88"`을 둔다.
- herdr 최소 버전은 0.9.3이다. popup은 80%×80%다. 팔레트는 `herdr plugin pane open --plugin herdr-linear --entrypoint palette --focus --env K=V…`로 연다.
- 팔레트로 넘기는 환경 변수는 `HERDR_LINEAR_ORIGIN_PANE`, `HERDR_LINEAR_ORIGIN_AGENT`, `HERDR_LINEAR_ORIGIN_CWD`, `HERDR_LINEAR_OPEN`이다.
- 시간 값은 다음과 같다.
  - 서버 검색 디바운스 300ms, 하단 안내 3초.
  - 브랜치 → 이슈 캐시 10분, viewer 다시 받기 60분.
  - 최근 본 50개, 깊은 검색 20건(분당 30회).
- 자동 요청(서버 검색)은 남은 요청이 50 미만이면 리셋 시각까지 멈춘다. 사용자가 직접 한 동작(새로고침, 깊은 검색, 상세)은 보낸다.
- 캐시(SQLite 연결)는 메인 스레드에서만 쓴다. 네트워크는 작업 스레드에서 보낸다. UI는 응답을 기다리며 멈추지 않는다.
- 화면에 그리는 외부 문자열(이슈·사용자·팀 이름, 서버 오류 문구, markdown)은 모두 제어 문자를 지운 뒤 그린다.
- 브라우저로는 http(s) 링크만 넘긴다.
- 키는 Authorization 헤더와 credentials 파일에만 쓴다. 로그(`herdr-linear.log`, 1MB 회전), 화면, 오류 문구에는 남기지 않는다.
- 한글 입력 처리는 다음과 같다.
  - Ctrl 조합은 IME와 상관없이 동작한다.
  - 목록·상세 모드의 자모는 두벌식 영문 키로 받는다.
  - 검색어 끝에는 실제 터미널 커서를 둔다.
- 테스트는 실제 HOME 아래 경로(credentials 등)를 건드리지 않는다. 보조 위치는 항상 인자로 넘기고, 테스트에서는 `&[]`나 임시 경로를 쓴다.
- 모든 작업은 끝날 때 `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`를 통과해야 한다.

## Review Focus

스펙이 암시하지만 기능 테스트만으로는 놓치기 쉬운 입력·실패 상황이다. 각 항목의 테스트는 해당 작업에 들어 있다.

- **아주 작은 창**: herdr popup의 최소 크기나 좁은 split에서는 1×1까지 줄어들 수 있다. 이때 그리기가 패닉이나 무한 루프 없이 끝나야 한다. (Task 7 `tiny_screens_do_not_panic`)
- **서버·이슈 문자열 속 터미널 제어 문자**: 오류 문구, 제목, 라벨에 ESC·BEL이 있으면 화면이 지워지거나 클립보드 쓰기(OSC 52)가 실행될 수 있다. (Task 4 `row_strips_control_characters`, Task 7 `server_text_cannot_inject_terminal_codes`)
- **늦게 도착한 응답**: 이전 검색어의 결과나 이미 떠난 이슈의 상세가 지금 화면을 덮으면 안 된다. (Task 5 `stale_search_results_are_ignored`, `late_detail_for_another_issue_is_ignored`)
- **키·워크스페이스가 바뀐 첫 실행**: 다른 워크스페이스의 캐시를 보여 주거나, 새로 받은 결과와 섞으면 안 된다. (Task 9 `new_key_waits_for_viewer_before_using_cache`)
- **herdr가 팝업을 거절하거나 herdr가 없을 때**: 설정 화면·복사 모드 때문에 `ui_busy`가 날 수 있고, herdr 실행 자체가 실패할 수도 있다. 액션 출력은 보이지 않으니 알림이나 오류로 알려야 한다. (Task 10 `busy_herdr_is_reported_by_notification`, `missing_herdr_binary_is_an_error`)

## 파일 구조

| 파일 | 책임 |
|---|---|
| `src/ui/style.rs` | Linear 색 → 터미널 색, 상태 아이콘·색, 라벨 색, 우선순위 이름, 시간 표시, 표시 폭 자르기 |
| `src/ui/row.rs` | 이슈 한 줄 (팔레트 목록·CLI 공용) |
| `src/tui/app.rs` | 팔레트 상태 머신 (순수 로직) |
| `src/tui/keys.rs` | 키 이벤트 → `Input`, 두벌식 자모 매핑 |
| `src/tui/view.rs` | `App` → ratatui 화면 |
| `src/tui/runtime.rs` | `Effect` 처리(캐시·네트워크·운영체제), 이벤트 루프, 패닉 로그 |
| `src/tui/system.rs` | 브라우저 열기·클립보드 (`System` trait) |
| `src/tui/mod.rs` | `palette()` 진입점 |
| `src/context.rs` | 플러그인 컨텍스트, 원래 pane, 브랜치 → 식별자 |
| `src/herdr.rs` | herdr CLI 호출 (pane 열기, 알림) |
| `src/log.rs` | 로그 파일 (1MB 회전) |
| `herdr-plugin.toml` | herdr 플러그인 매니페스트 |
| `README.md` | 설치·키·단축키·사용법 |

고치는 파일은 `Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/config.rs`, `src/cli.rs`, `src/markdown.rs`, `src/search/rank.rs`, `src/linear/client.rs`, `src/linear/queries.rs`다.

---

### Task 1: 1부 리뷰 후속: 설정 범위, 키 보조 위치, 오류 문구, 타임아웃

1부 최종 리뷰에서 2부로 미룬 Minor 가운데 설정·키·문구·클라이언트 항목을 먼저 처리한다(`docs/superpowers/notes/2026-10-07-part1-review-followups.md`).

- **M1** 숫자 설정에 범위를 둔다(`side.refresh_seconds` 0~3600, `cache.retention_days` 1~3650, `agent.include_comments` 0~50). 범위를 벗어나면 그 항목만 기본값을 쓰고 경고한다. 보관 기간 계산은 `saturating_mul`/`saturating_sub`로 한다.
- **키 보조 위치**: CLI는 `~/.config/herdr-linear`, 플러그인은 `~/.config/herdr/plugins/config/herdr-linear`를 설정 디렉터리로 쓴다. 한쪽에서 로그인한 키를 다른 쪽에서도 찾도록 두 credentials 위치를 보조로 읽고, `logout`은 모두 지운다. 보조 위치는 인자(`fallbacks`)로 받는다. 테스트는 실제 HOME 경로를 절대 넘기지 않는다(사용자 키를 지울 수 있다).
- **M7** 환경 변수 키의 인증 실패는 "LINEAR_API_KEY 환경 변수의 키가 유효하지 않아요"라고 알린다. 한도 초과는 몇 분 뒤 다시 할지 알린다.
- **M2** `whoami`·`login` 문구와 `main`의 오류 출력에서 제어 문자를 지운다.
- **M6** config의 `teams`와 맞는 팀이 하나도 없으면 "모든 팀에서 찾아요" 경고를 낸다(`scope_warning`).
- **M10** 클라이언트 타임아웃을 정해서 만들 수 있게 하고(`with_timeouts`), 응답하지 않는 서버가 오프라인으로 끝나는지 테스트한다.

**Files:**
- Modify: `src/main.rs`, `src/cli.rs`, `src/config.rs`, `src/linear/client.rs`
- Test: `src/cli.rs`, `src/config.rs`, `src/linear/client.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: 1부의 `config`, `cli`, `linear::client` 모듈
- Produces:
  - `config::credential_fallbacks(home: &Path) -> Vec<PathBuf>` (CLI·플러그인 credentials 두 곳)
  - `config::default_credential_fallbacks() -> Vec<PathBuf>` (실제 HOME 기준. 테스트에서 쓰지 않는다)
  - `config::resolve_api_key(env_value: Option<String>, paths: &Paths, fallbacks: &[PathBuf]) -> Result<Option<ApiKey>>`
  - `config::delete_credentials(paths: &Paths, fallbacks: &[PathBuf]) -> Result<()>`
  - `cli::Ctx`에 `pub key_source: KeySource` 필드
  - `cli::logout(paths: &Paths, fallbacks: &[PathBuf]) -> Result<String>`
  - `cli::scope_warning(viewer: &Viewer, settings: &Settings) -> Option<String>`
  - `linear::client::{CONNECT_TIMEOUT, GLOBAL_TIMEOUT}: Duration` (5초, 15초)
  - `LinearClient::with_timeouts(api_key, endpoint, connect: Duration, global: Duration) -> LinearClient`

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            settings: Settings::default(),
            store: Store::open_in_memory().unwrap(),
            client: LinearClient::with_endpoint("lin_api_test", endpoint),
            now_ms: NOW,
            color: false,
            width: 60,
````

아래로 바꾼다:

````rust
            settings: Settings::default(),
            store: Store::open_in_memory().unwrap(),
            client: LinearClient::with_endpoint("lin_api_test", endpoint),
            key_source: KeySource::File,
            now_ms: NOW,
            color: false,
            width: 60,
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
        let (_d, ctx) = test_ctx(OFFLINE.into());
        config::save_api_key(&ctx.paths, "lin_api_x").unwrap();
        Store::open(&ctx.paths.cache_db()).unwrap();
        logout(&ctx.paths).unwrap();
        assert!(!ctx.paths.credentials_file().exists());
        assert!(!ctx.paths.cache_db().exists());
    }
````

아래로 바꾼다:

````rust
        let (_d, ctx) = test_ctx(OFFLINE.into());
        config::save_api_key(&ctx.paths, "lin_api_x").unwrap();
        Store::open(&ctx.paths.cache_db()).unwrap();
        logout(&ctx.paths, &[]).unwrap();
        assert!(!ctx.paths.credentials_file().exists());
        assert!(!ctx.paths.cache_db().exists());
    }
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
        assert_eq!(ago(NOW, NOW - 3 * 3_600_000), "3시간 전");
        assert_eq!(ago(NOW, NOW - 2 * 86_400_000), "2일 전");
    }
}
````

아래로 바꾼다:

````rust
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
}
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
    fn env_key_wins_over_file() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "lin_api_file").unwrap();
        let k = resolve_api_key(Some(" lin_api_env ".into()), &paths)
            .unwrap()
            .unwrap();
        assert_eq!(k.value, "lin_api_env");
````

아래로 바꾼다:

````rust
    fn env_key_wins_over_file() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "lin_api_file").unwrap();
        let k = resolve_api_key(Some(" lin_api_env ".into()), &paths, &[])
            .unwrap()
            .unwrap();
        assert_eq!(k.value, "lin_api_env");
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
    fn file_key_used_when_env_empty() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "  lin_api_file  ").unwrap();
        let k = resolve_api_key(Some("".into()), &paths).unwrap().unwrap();
        assert_eq!(k.value, "lin_api_file");
        assert_eq!(k.source, KeySource::File);
    }
````

아래로 바꾼다:

````rust
    fn file_key_used_when_env_empty() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "  lin_api_file  ").unwrap();
        let k = resolve_api_key(Some("".into()), &paths, &[])
            .unwrap()
            .unwrap();
        assert_eq!(k.value, "lin_api_file");
        assert_eq!(k.source, KeySource::File);
    }
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
    #[test]
    fn missing_or_blank_file_means_no_key() {
        let (_d, paths) = temp_paths();
        assert_eq!(resolve_api_key(None, &paths).unwrap(), None);
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(paths.credentials_file(), "  \n").unwrap();
        assert_eq!(resolve_api_key(None, &paths).unwrap(), None);
    }

    #[test]
````

아래로 바꾼다:

````rust
    #[test]
    fn missing_or_blank_file_means_no_key() {
        let (_d, paths) = temp_paths();
        assert_eq!(resolve_api_key(None, &paths, &[]).unwrap(), None);
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(paths.credentials_file(), "  \n").unwrap();
        assert_eq!(resolve_api_key(None, &paths, &[]).unwrap(), None);
    }

    #[test]
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
    fn delete_is_idempotent() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "lin_api_x").unwrap();
        delete_credentials(&paths).unwrap();
        delete_credentials(&paths).unwrap();
        assert_eq!(resolve_api_key(None, &paths).unwrap(), None);
    }

    #[test]
````

아래로 바꾼다:

````rust
    fn delete_is_idempotent() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "lin_api_x").unwrap();
        delete_credentials(&paths, &[]).unwrap();
        delete_credentials(&paths, &[]).unwrap();
        assert_eq!(resolve_api_key(None, &paths, &[]).unwrap(), None);
    }

    #[test]
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
        assert!(!shown.contains("secret"));
        assert!(shown.contains("<redacted>"));
    }
}
````

아래로 바꾼다:

````rust
        assert!(!shown.contains("secret"));
        assert!(shown.contains("<redacted>"));
    }

    #[test]
    fn out_of_range_values_fall_back() {
        let (_d, paths) = temp_paths();
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            paths.config_file(),
            "[side]\nrefresh_seconds = 99999\n[cache]\nretention_days = 99999999999\n[agent]\ninclude_comments = -1\n",
        )
        .unwrap();
        let (s, w) = load_settings(&paths.config_file());
        assert_eq!(s, Settings::default());
        assert_eq!(w.len(), 3, "{w:?}");
        assert!(w.iter().any(|m| m.contains("1~3650")), "{w:?}");
    }

    #[test]
    fn fallback_credentials_are_found() {
        let (d, paths) = temp_paths();
        let other = d.path().join("other").join("credentials");
        fs::create_dir_all(other.parent().unwrap()).unwrap();
        fs::write(&other, "lin_api_other\n").unwrap();
        let k = resolve_api_key(None, &paths, std::slice::from_ref(&other))
            .unwrap()
            .unwrap();
        assert_eq!(k.value, "lin_api_other");
        // 주 위치에 키가 있으면 그쪽이 먼저다
        save_api_key(&paths, "lin_api_primary").unwrap();
        let k = resolve_api_key(None, &paths, &[other]).unwrap().unwrap();
        assert_eq!(k.value, "lin_api_primary");
    }

    #[test]
    fn delete_removes_fallback_keys_too() {
        let (d, paths) = temp_paths();
        let other = d.path().join("other").join("credentials");
        fs::create_dir_all(other.parent().unwrap()).unwrap();
        fs::write(&other, "lin_api_other\n").unwrap();
        save_api_key(&paths, "lin_api_primary").unwrap();
        delete_credentials(&paths, std::slice::from_ref(&other)).unwrap();
        assert!(!paths.credentials_file().exists());
        assert!(!other.exists());
    }

    #[test]
    fn fallback_locations_cover_cli_and_plugin() {
        let f = credential_fallbacks(Path::new("/home/me"));
        assert_eq!(
            f,
            vec![
                PathBuf::from("/home/me/.config/herdr-linear/credentials"),
                PathBuf::from("/home/me/.config/herdr/plugins/config/herdr-linear/credentials"),
            ]
        );
    }
}
````

`src/linear/client.rs`에서 아래 부분을 찾아:

````rust
    }

    #[test]
    fn invalid_json_is_decode_error() {
        let mut server = mockito::Server::new();
        server
````

아래로 바꾼다:

````rust
    }

    #[test]
    fn slow_server_times_out_as_offline() {
        // 연결은 받고 응답은 하지 않는 서버
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let _conn = listener.accept().unwrap();
            std::thread::sleep(Duration::from_secs(5));
        });
        let c = LinearClient::with_timeouts(
            "lin_api_test",
            format!("http://{addr}/graphql"),
            Duration::from_secs(1),
            Duration::from_millis(300),
        );
        let started = std::time::Instant::now();
        let r: Result<ViewerData, ApiError> = c.execute("query { viewer { id } }", json!({}));
        assert!(matches!(r, Err(ApiError::Offline(_))), "{r:?}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn invalid_json_is_decode_error() {
        let mut server = mockito::Server::new();
        server
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib -- config:: cli:: linear::client::`
Expected: 컴파일 실패 — `credential_fallbacks`, `with_timeouts`, `scope_warning` 같은 이름이 없고 `resolve_api_key`·`delete_credentials`·`logout`의 인자 수가 맞지 않는다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/main.rs`에서 아래 부분을 찾아:

````rust
            }
        }
        Err(e) => {
            eprintln!("오류: {e:#}");
            std::process::exit(1);
        }
    }
````

아래로 바꾼다:

````rust
            }
        }
        Err(e) => {
            // 서버가 준 오류 문구가 섞일 수 있어 제어 문자를 지운다
            eprintln!(
                "오류: {}",
                herdr_linear::markdown::sanitize(&format!("{e:#}"))
            );
            std::process::exit(1);
        }
    }
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};

use crate::config::{self, Paths, Settings};
use crate::linear::client::{ApiError, LinearClient};
use crate::linear::filter::{build_issue_filter, token_filter};
use crate::linear::queries;
````

아래로 바꾼다:

````rust
use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};

use crate::config::{self, KeySource, Paths, Settings};
use crate::linear::client::{ApiError, LinearClient};
use crate::linear::filter::{build_issue_filter, token_filter};
use crate::linear::queries;
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    pub settings: Settings,
    pub store: Store,
    pub client: LinearClient,
    pub now_ms: i64,
    /// 색을 쓸지 (표준 출력이 터미널일 때)
    pub color: bool,
````

아래로 바꾼다:

````rust
    pub settings: Settings,
    pub store: Store,
    pub client: LinearClient,
    /// 키를 어디서 얻었는지 (환경 변수 키의 인증 실패를 따로 안내한다)
    pub key_source: KeySource,
    pub now_ms: i64,
    /// 색을 쓸지 (표준 출력이 터미널일 때)
    pub color: bool,
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
        for w in warnings {
            eprintln!("경고: {w}");
        }
        let key = config::resolve_api_key(std::env::var("LINEAR_API_KEY").ok(), &paths)?
            .ok_or_else(|| anyhow!("API 키가 없어요. 먼저 `herdr-linear login`을 실행하세요"))?;
        let store = open_cache(&paths)?;
        let now = now_ms();
        store.evict_older_than(now - settings.cache_retention_days as i64 * DAY_MS)?;
        let width = ratatui::crossterm::terminal::size()
            .map(|(w, _)| w)
            .unwrap_or(100)
````

아래로 바꾼다:

````rust
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
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            settings,
            store,
            client: LinearClient::new(key.value),
            now_ms: now,
            color: std::io::stdout().is_terminal(),
            width,
````

아래로 바꾼다:

````rust
            settings,
            store,
            client: LinearClient::new(key.value),
            key_source: key.source,
            now_ms: now,
            color: std::io::stdout().is_terminal(),
            width,
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    let paths = Paths::from_env()?;
    match cli.command {
        Command::Login => return login(&paths),
        Command::Logout => return logout(&paths),
        _ => {}
    }
    let ctx = Ctx::open(paths)?;
````

아래로 바꾼다:

````rust
    let paths = Paths::from_env()?;
    match cli.command {
        Command::Login => return login(&paths),
        Command::Logout => return logout(&paths, &config::default_credential_fallbacks()),
        _ => {}
    }
    let ctx = Ctx::open(paths)?;
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    config::save_api_key(paths, key)?;
    let store = open_cache(paths)?;
    save_viewer(&store, &viewer, now_ms(), &client.key_fingerprint())?;
    Ok(format!(
        "{}님, {} 워크스페이스에 연결됐어요",
        viewer.name, viewer.organization.name
    ))
}

/// 캐시를 연다. 열 수 없으면 경고하고, 이번 실행은 저장 없이 메모리 캐시로 계속한다.
````

아래로 바꾼다:

````rust
    config::save_api_key(paths, key)?;
    let store = open_cache(paths)?;
    save_viewer(&store, &viewer, now_ms(), &client.key_fingerprint())?;
    Ok(markdown::sanitize(&format!(
        "{}님, {} 워크스페이스에 연결됐어요",
        viewer.name, viewer.organization.name
    )))
}

/// 캐시를 연다. 열 수 없으면 경고하고, 이번 실행은 저장 없이 메모리 캐시로 계속한다.
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    })
}

pub fn logout(paths: &Paths) -> Result<String> {
    config::delete_credentials(paths)?;
    remove_db_files(&paths.cache_db());
    Ok("API 키와 캐시를 지웠어요".to_string())
}
````

아래로 바꾼다:

````rust
    })
}

/// 키(보조 위치 포함)와 캐시를 지운다. `fallbacks`는 실제 실행에서만 HOME 기준 위치를 넘긴다.
pub fn logout(paths: &Paths, fallbacks: &[std::path::PathBuf]) -> Result<String> {
    config::delete_credentials(paths, fallbacks)?;
    remove_db_files(&paths.cache_db());
    Ok("API 키와 캐시를 지웠어요".to_string())
}
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            Ok(Some(v))
        }
        Err(ApiError::Offline(_)) => Ok(cached.map(|(v, _)| v)),
        Err(e) => Err(api_error(e, ctx.now_ms)),
    }
}

````

아래로 바꾼다:

````rust
            Ok(Some(v))
        }
        Err(ApiError::Offline(_)) => Ok(cached.map(|(v, _)| v)),
        Err(e) => Err(api_error(ctx, e)),
    }
}

````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            scope_team_ids(&v, &ctx.settings).len()
        ),
    ];
    let rate = ctx.client.rate_limit();
    if let Some(r) = rate.requests_remaining {
        out.push(format!("남은 요청: {r} (시간당)"));
    }
    Ok(out.join("\n"))
}

pub fn mine(ctx: &Ctx) -> Result<String> {
````

아래로 바꾼다:

````rust
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
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            )),
            None => Err(anyhow!("오프라인이고 저장된 결과도 없어요: {msg}")),
        },
        Err(e) => Err(api_error(e, ctx.now_ms)),
    }
}

````

아래로 바꾼다:

````rust
            )),
            None => Err(anyhow!("오프라인이고 저장된 결과도 없어요: {msg}")),
        },
        Err(e) => Err(api_error(ctx, e)),
    }
}

````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            .as_ref()
            .map(|v| scope_team_ids(v, &ctx.settings))
            .unwrap_or_default();
        queries::filter_issues(&ctx.client, &build_issue_filter(&q, &scope))
    };
    match server {
````

아래로 바꾼다:

````rust
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
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            &local[..local.len().min(SEARCH_LIMIT)],
            Some(&format!("오프라인: 저장된 이슈에서만 찾았어요 · {msg}")),
        )),
        Err(e) => Err(api_error(e, ctx.now_ms)),
    }
}

````

아래로 바꾼다:

````rust
            &local[..local.len().min(SEARCH_LIMIT)],
            Some(&format!("오프라인: 저장된 이슈에서만 찾았어요 · {msg}")),
        )),
        Err(e) => Err(api_error(ctx, e)),
    }
}

````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
                Some(&format!("오프라인: 저장된 내용 · {msg}")),
            ))
        }
        Err(e) => Err(api_error(e, ctx.now_ms)),
    }
}

````

아래로 바꾼다:

````rust
                Some(&format!("오프라인: 저장된 내용 · {msg}")),
            ))
        }
        Err(e) => Err(api_error(ctx, e)),
    }
}

````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    }
}

/// 한도 초과면 언제 다시 시도할 수 있는지 알려준다.
fn api_error(e: ApiError, now_ms: i64) -> anyhow::Error {
    match e {
        ApiError::RateLimited {
            reset_at_ms: Some(reset),
        } => {
            let mins = ((reset - now_ms).max(0) + 59_999) / 60_000;
            anyhow!("Linear API 한도를 넘었어요. {mins}분 후 다시 시도하세요")
        }
        other => other.into(),
    }
````

아래로 바꾼다:

````rust
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
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
    }
    read_u64(
        &table,
        "side",
        "refresh_seconds",
        0,
        &mut s.side_refresh_seconds,
        &mut warnings,
    );
    read_u64(
        &table,
        "cache",
        "retention_days",
        1,
        &mut s.cache_retention_days,
        &mut warnings,
    );
    read_u64(
        &table,
        "agent",
        "include_comments",
        0,
        &mut s.agent_include_comments,
        &mut warnings,
    );
````

아래로 바꾼다:

````rust
    }
    read_u64(
        &table,
        ("side", "refresh_seconds"),
        0..=3600,
        &mut s.side_refresh_seconds,
        &mut warnings,
    );
    read_u64(
        &table,
        ("cache", "retention_days"),
        1..=3650,
        &mut s.cache_retention_days,
        &mut warnings,
    );
    read_u64(
        &table,
        ("agent", "include_comments"),
        0..=50,
        &mut s.agent_include_comments,
        &mut warnings,
    );
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
    (s, warnings)
}

fn read_u64(
    table: &toml::Table,
    section: &str,
    key: &str,
    min: u64,
    target: &mut u64,
    warnings: &mut Vec<String>,
) {
    let Some(v) = table.get(section).and_then(|t| t.get(key)) else {
        return;
    };
    match v.as_integer() {
        Some(n) if n >= min as i64 => *target = n as u64,
        _ => warnings.push(format!(
            "{section}.{key}는 {min} 이상의 정수여야 해요. 기본값을 써요"
        )),
    }
}
````

아래로 바꾼다:

````rust
    (s, warnings)
}

/// `[section] key = 정수`를 읽는다. 범위를 벗어나면 기본값을 두고 경고한다.
fn read_u64(
    table: &toml::Table,
    (section, key): (&str, &str),
    range: std::ops::RangeInclusive<u64>,
    target: &mut u64,
    warnings: &mut Vec<String>,
) {
    let Some(v) = table.get(section).and_then(|t| t.get(key)) else {
        return;
    };
    match v.as_integer().and_then(|n| u64::try_from(n).ok()) {
        Some(n) if range.contains(&n) => *target = n,
        _ => warnings.push(format!(
            "{section}.{key}는 {}~{} 사이의 정수여야 해요. 기본값을 써요",
            range.start(),
            range.end()
        )),
    }
}
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
    }
}

/// 키를 찾는다. 순서: `LINEAR_API_KEY` 환경 변수 → credentials 파일.
pub fn resolve_api_key(env_value: Option<String>, paths: &Paths) -> Result<Option<ApiKey>> {
    if let Some(v) = env_value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
````

아래로 바꾼다:

````rust
    }
}

/// CLI(`~/.config/herdr-linear`)와 herdr 플러그인(`~/.config/herdr/plugins/config/herdr-linear`)은
/// 설정 디렉터리가 다르다. 한쪽에서 로그인한 키를 다른 쪽에서도 찾도록 보조로 읽는 위치.
pub fn credential_fallbacks(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".config").join(PLUGIN_ID).join("credentials"),
        home.join(".config")
            .join("herdr")
            .join("plugins")
            .join("config")
            .join(PLUGIN_ID)
            .join("credentials"),
    ]
}

/// 실제 HOME 기준 보조 위치. 테스트에서는 쓰지 말고 임시 경로를 넘긴다.
pub fn default_credential_fallbacks() -> Vec<PathBuf> {
    std::env::var_os("HOME")
        .map(|h| credential_fallbacks(Path::new(&h)))
        .unwrap_or_default()
}

/// 키를 찾는다. 순서: `LINEAR_API_KEY` 환경 변수 → credentials 파일 → `fallbacks`.
pub fn resolve_api_key(
    env_value: Option<String>,
    paths: &Paths,
    fallbacks: &[PathBuf],
) -> Result<Option<ApiKey>> {
    if let Some(v) = env_value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
            source: KeySource::Env,
        }));
    }
    match fs::read_to_string(paths.credentials_file()) {
        Ok(text) => {
            let v = text.trim().to_string();
            Ok((!v.is_empty()).then_some(ApiKey {
                value: v,
                source: KeySource::File,
            }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("credentials 파일을 읽지 못했어요"),
    }
}

/// 키를 credentials 파일에 0600 권한으로 저장한다.
````

아래로 바꾼다:

````rust
            source: KeySource::Env,
        }));
    }
    let primary = paths.credentials_file();
    let candidates = std::iter::once(&primary).chain(fallbacks.iter().filter(|p| **p != primary));
    for path in candidates {
        match fs::read_to_string(path) {
            Ok(text) => {
                let v = text.trim();
                if !v.is_empty() {
                    return Ok(Some(ApiKey {
                        value: v.to_string(),
                        source: KeySource::File,
                    }));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| format!("{}을 읽지 못했어요", path.display()));
            }
        }
    }
    Ok(None)
}

/// 키를 credentials 파일에 0600 권한으로 저장한다.
````

`src/config.rs`에서 아래 부분을 찾아:

````rust
    write_private_file(&paths.credentials_file(), format!("{key}\n").as_bytes())
}

/// credentials 파일을 지운다. 없으면 아무것도 하지 않는다.
pub fn delete_credentials(paths: &Paths) -> Result<()> {
    match fs::remove_file(paths.credentials_file()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).context("credentials 파일을 지우지 못했어요"),
    }
}

/// 디렉터리를 만들고(없으면) 권한을 0700으로 맞춘다.
````

아래로 바꾼다:

````rust
    write_private_file(&paths.credentials_file(), format!("{key}\n").as_bytes())
}

/// credentials 파일과 보조 위치의 키를 모두 지운다. 없으면 건너뛴다.
pub fn delete_credentials(paths: &Paths, fallbacks: &[PathBuf]) -> Result<()> {
    for path in std::iter::once(&paths.credentials_file()).chain(fallbacks) {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| format!("{}을 지우지 못했어요", path.display()));
            }
        }
    }
    Ok(())
}

/// 디렉터리를 만들고(없으면) 권한을 0700으로 맞춘다.
````

`src/linear/client.rs`에서 아래 부분을 찾아:

````rust

/// 남은 요청이 이보다 적으면 자동 요청(주기 새로고침·서버 검색)을 멈춘다.
pub const AUTO_PAUSE_THRESHOLD: i64 = 50;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
````

아래로 바꾼다:

````rust

/// 남은 요청이 이보다 적으면 자동 요청(주기 새로고침·서버 검색)을 멈춘다.
pub const AUTO_PAUSE_THRESHOLD: i64 = 50;

/// 연결 타임아웃.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// 요청 하나의 전체 타임아웃. 넘으면 오프라인으로 본다.
pub const GLOBAL_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
````

`src/linear/client.rs`에서 아래 부분을 찾아:

````rust

    /// 테스트에서 가짜 서버 주소를 넣을 때 쓴다.
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .build()
            .into();
````

아래로 바꾼다:

````rust

    /// 테스트에서 가짜 서버 주소를 넣을 때 쓴다.
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        LinearClient::with_timeouts(api_key, endpoint, CONNECT_TIMEOUT, GLOBAL_TIMEOUT)
    }

    /// 타임아웃을 정해서 만든다. 테스트에서 짧게 줄일 때 쓴다.
    pub fn with_timeouts(
        api_key: impl Into<String>,
        endpoint: impl Into<String>,
        connect: Duration,
        global: Duration,
    ) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(connect))
            .timeout_global(Some(global))
            .http_status_as_error(false)
            .build()
            .into();
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib -- config:: cli:: linear::client::`
Expected: `test result: ok. 56 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 134개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/main.rs src/cli.rs src/config.rs src/linear/client.rs
git commit -m "fix: 1부 리뷰 후속 — 설정 범위, 키 보조 위치, 오류 문구, 타임아웃" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 2: 검색 정확도: 글자 그대로 매칭, 단어별 AND, 색인 갱신

타이핑 검색의 로컬 순위를 서버 필터와 같은 뜻으로 맞춘다(M4, M5). 팔레트가 받은 이슈를 검색 색인에 바로 반영하는 `upsert`도 더한다.

- **M4** 검색어의 `!`, `^`, `$` 같은 nucleo 문법을 글자 그대로 찾는다. 단어마다 `Atom::new(단어, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy, false)`를 만든다.
- **M5** 단어는 AND로 묶되, 단어마다 제목이나 본문 중 한 곳에만 있으면 된다(서버의 `title or description` 조건과 같다). 순위 단계는 다음과 같다.
  - 0: 식별자·번호가 일치하고 나머지 단어가 모두 제목이나 본문에 있다.
  - 1: 모든 단어가 제목에 있다(퍼지 점수 합).
  - 2: 단어마다 제목이나 본문에 있다. 서버가 준 결과도 여기에 남긴다.
  - 3: 텍스트 없이 토큰만 있다.
- `SearchIndex::upsert`는 같은 id의 항목을 바꾸고, 보관·삭제된 이슈는 뺀다.

**Files:**
- Modify: `src/search/rank.rs`
- Test: `src/search/rank.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: 1부의 `search::rank::{SearchIndex, merge}`, `search::query::ParsedQuery`
- Produces:
  - `SearchIndex::upsert(&mut self, issues: &[Issue])`

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/search/rank.rs`에서 아래 부분을 찾아:

````rust
        assert_eq!(r.len(), 5000);
        assert!(elapsed.as_millis() < 16, "{elapsed:?}");
    }
}
````

아래로 바꾼다:

````rust
        assert_eq!(r.len(), 5000);
        assert!(elapsed.as_millis() < 16, "{elapsed:?}");
    }

    #[test]
    fn fuzzy_syntax_characters_are_literal() {
        let idx = SearchIndex::new(vec![
            IssueBuilder::new("a", "ENG-1", "로그인 오류").build(),
            IssueBuilder::new("b", "ENG-2", "결제 실패").build(),
        ]);
        // `!`가 부정으로 해석되면 "결제 실패"가 나온다
        assert!(idx.search(&parse("!로그인"), None).is_empty());
    }

    #[test]
    fn words_may_split_between_title_and_body() {
        let idx = SearchIndex::new(vec![
            IssueBuilder::new("a", "ENG-1", "로그인 버그")
                .description("결제 흐름에서 발생")
                .build(),
            IssueBuilder::new("b", "ENG-2", "로그인 화면").build(),
        ]);
        assert_eq!(ids(&idx.search(&parse("로그인 결제"), None)), vec!["ENG-1"]);
    }

    #[test]
    fn number_match_still_needs_the_other_words() {
        let idx = SearchIndex::new(vec![
            IssueBuilder::new("a", "OPS-131", "결제 페이지").build(),
            IssueBuilder::new("b", "ENG-5", "로그인 버그 131").build(),
        ]);
        assert_eq!(ids(&idx.search(&parse("로그인 131"), None)), vec!["ENG-5"]);
        assert_eq!(ids(&idx.search(&parse("131"), None))[0], "OPS-131");
    }

    #[test]
    fn upsert_replaces_and_drops_gone_issues() {
        let mut idx = SearchIndex::new(vec![
            IssueBuilder::new("a", "ENG-1", "옛 제목").build(),
            IssueBuilder::new("b", "ENG-2", "보관될 이슈").build(),
        ]);
        idx.upsert(&[
            IssueBuilder::new("a", "ENG-1", "새 제목").build(),
            IssueBuilder::new("b", "ENG-2", "보관될 이슈")
                .archived()
                .build(),
            IssueBuilder::new("c", "ENG-3", "새 이슈").build(),
        ]);
        assert_eq!(idx.len(), 2);
        assert_eq!(ids(&idx.search(&parse("새"), None)), vec!["ENG-1", "ENG-3"]);
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib search::rank`
Expected: 컴파일 실패 — `SearchIndex`에 `upsert`가 없다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/search/rank.rs`에서 아래 부분을 찾아:

````rust
use std::cmp::Ordering;
use std::collections::HashSet;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use unicode_normalization::UnicodeNormalization;

````

아래로 바꾼다:

````rust
use std::cmp::Ordering;
use std::collections::HashSet;

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use unicode_normalization::UnicodeNormalization;

````

`src/search/rank.rs`에서 아래 부분을 찾아:

````rust
    pub fn search(&self, q: &ParsedQuery, viewer_id: Option<&str>) -> Vec<Issue> {
        rank(&self.entries, &HashSet::new(), q, viewer_id)
    }
}

/// 로컬 결과와 서버 결과를 합친다. 같은 id면 서버 쪽(최신)을 쓴다.
````

아래로 바꾼다:

````rust
    pub fn search(&self, q: &ParsedQuery, viewer_id: Option<&str>) -> Vec<Issue> {
        rank(&self.entries, &HashSet::new(), q, viewer_id)
    }

    /// 새로 받은 이슈를 색인에 넣거나 바꾼다. 보관·휴지통 이슈는 뺀다.
    pub fn upsert(&mut self, issues: &[Issue]) {
        for issue in issues {
            self.entries.retain(|e| e.issue.id != issue.id);
            if !issue.is_gone() {
                self.entries.push(Entry::new(issue.clone()));
            }
        }
    }
}

/// 로컬 결과와 서버 결과를 합친다. 같은 id면 서버 쪽(최신)을 쓴다.
````

`src/search/rank.rs`에서 아래 부분을 찾아:

````rust

#[derive(Debug, Clone, Copy)]
struct Rank {
    /// 0 식별자·번호 일치, 1 제목 퍼지, 2 본문 포함, 3 텍스트 조건 없음
    tier: u8,
    score: u32,
}
````

아래로 바꾼다:

````rust

#[derive(Debug, Clone, Copy)]
struct Rank {
    /// 0 식별자·번호 일치, 1 모든 단어가 제목에, 2 단어마다 제목 또는 본문에, 3 텍스트 조건 없음
    tier: u8,
    score: u32,
}
````

`src/search/rank.rs`에서 아래 부분을 찾아:

````rust
    viewer_id: Option<&str>,
) -> Vec<Issue> {
    let terms = text_terms(q);
    let pattern = (!terms.is_empty())
        .then(|| Pattern::parse(&terms.join(" "), CaseMatching::Ignore, Normalization::Smart));
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Vec::new();
    let mut scored: Vec<(Rank, &Issue)> = Vec::new();
````

아래로 바꾼다:

````rust
    viewer_id: Option<&str>,
) -> Vec<Issue> {
    let terms = text_terms(q);
    // `!`, `^`, `$`, `'` 같은 퍼지 문법을 해석하지 않도록 단어마다 리터럴 atom을 만든다
    let atoms: Vec<Atom> = terms
        .iter()
        .map(|t| {
            Atom::new(
                t,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
                false,
            )
        })
        .collect();
    // text_terms는 자유 텍스트 단어를 맨 앞에 둔다
    let word_count = q.words.len();
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Vec::new();
    let mut scored: Vec<(Rank, &Issue)> = Vec::new();
````

`src/search/rank.rs`에서 아래 부분을 찾아:

````rust
        if !kept && !passes_filters(&e.issue, q, viewer_id) {
            continue;
        }
        let rank = if !q.has_text() {
            Rank { tier: 3, score: 0 }
        } else if exact_match(&e.issue, q) {
            Rank { tier: 0, score: 0 }
        } else if let Some(score) = pattern
            .as_ref()
            .and_then(|p| p.score(Utf32Str::new(&e.hay, &mut buf), &mut matcher))
        {
            Rank { tier: 1, score }
        } else if terms.iter().all(|t| e.desc.contains(t.as_str())) || kept {
            Rank { tier: 2, score: 0 }
        } else {
            continue;
````

아래로 바꾼다:

````rust
        if !kept && !passes_filters(&e.issue, q, viewer_id) {
            continue;
        }
        if !q.has_text() {
            scored.push((Rank { tier: 3, score: 0 }, &e.issue));
            continue;
        }
        let hay = Utf32Str::new(&e.hay, &mut buf);
        let title: Vec<Option<u16>> = atoms.iter().map(|a| a.score(hay, &mut matcher)).collect();
        // 단어마다 제목(퍼지) 또는 본문(포함) 어느 한쪽에 있으면 된다
        let found = |i: usize| title[i].is_some() || e.desc.contains(terms[i].as_str());
        let rank = if exact_match(&e.issue, q) && (0..word_count).all(found) {
            Rank { tier: 0, score: 0 }
        } else if title.iter().all(Option::is_some) {
            let score = title.iter().map(|s| u32::from(s.unwrap_or(0))).sum();
            Rank { tier: 1, score }
        } else if (0..terms.len()).all(found) || kept {
            Rank { tier: 2, score: 0 }
        } else {
            continue;
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib search::rank`
Expected: `test result: ok. 13 passed` (성능 테스트 1개는 ignored로 보인다)

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 138개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/search/rank.rs
git commit -m "fix(search): 검색어를 글자 그대로, 단어별 AND, 색인 갱신" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 3: markdown 보강: 표 안 링크 번호, 팀 키 강조, 코드 블록 배경

상세 화면에 쓸 markdown 렌더러를 다듬는다.

- **M3** 표 셀 안의 링크·이미지에도 번호(`[1]`)를 붙이고 링크 목록에 넣는다. 상세 화면의 `u` 목록이 표 안 링크도 연다.
- **권고 5** 이슈 식별자 강조를 알려진 팀 키로 제한한다. `UTF-8`, `SHA-256` 같은 글자는 강조하지 않는다. 새 `render_with(md, width, theme, team_keys)`가 키 목록을 받고, 기존 `render`는 빈 목록(모두 강조)으로 그대로 동작한다.
- 코드 블록 줄을 폭 끝까지 배경색으로 칠한다(1부에서는 글자에만 칠해졌다).
- CLI `show`는 viewer의 팀 키(`cli::team_keys`)를 넘긴다.

**Files:**
- Modify: `src/cli.rs`, `src/markdown.rs`
- Test: `src/cli.rs`, `src/markdown.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: 1부의 `markdown::{render, Rendered, Theme}`, `cli::show`
- Produces:
  - `markdown::render_with(md: &str, width: u16, theme: &Theme, team_keys: &[String]) -> Rendered`
  - `cli::team_keys(viewer: Option<&Viewer>) -> Vec<String>`

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/markdown.rs`에서 아래 부분을 찾아:

````rust

    #[test]
    fn code_block_keeps_indent_and_hard_wraps() {
        assert_eq!(
            plain("```rust\nfn main() {}\n```", 20),
            vec!["  fn main() {}"]
        );
        assert_eq!(
            plain("```\n0123456789012345678901234\n```", 20),
            vec!["  012345678901234567", "  8901234"]
        );
    }
````

아래로 바꾼다:

````rust

    #[test]
    fn code_block_keeps_indent_and_hard_wraps() {
        let trimmed = |md: &str| -> Vec<String> {
            plain(md, 20)
                .into_iter()
                .map(|l| l.trim_end().to_string())
                .collect()
        };
        assert_eq!(
            trimmed("```rust\nfn main() {}\n```"),
            vec!["  fn main() {}"]
        );
        assert_eq!(
            trimmed("```\n0123456789012345678901234\n```"),
            vec!["  012345678901234567", "  8901234"]
        );
    }
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
        let text = to_plain(&r.lines);
        assert!(!has_control(&text), "{text:?}");
    }
}
````

아래로 바꾼다:

````rust
        let text = to_plain(&r.lines);
        assert!(!has_control(&text), "{text:?}");
    }

    #[test]
    fn code_block_background_fills_width() {
        let theme = Theme::default();
        let r = render("```\nx\n```", 20, &theme);
        assert_eq!(r.lines[0].width(), 20);
        assert!(r.lines[0].spans.iter().all(|s| s.style == theme.code));
    }

    #[test]
    fn links_and_images_inside_tables_are_numbered() {
        let r = render(
            "| 문서 | 그림 |\n|---|---|\n| [가이드](https://x.dev/g) | ![](https://x.dev/a.png) |\n",
            60,
            &Theme::default(),
        );
        assert_eq!(r.links.len(), 2, "{:?}", r.links);
        assert_eq!(r.links[0].url, "https://x.dev/g");
        assert_eq!(r.links[0].label, "가이드");
        assert_eq!(r.links[1].kind, LinkKind::Image);
        let text = to_plain(&r.lines);
        assert!(text.contains("가이드 [1]"), "{text}");
        assert!(text.contains("[이미지 2: a.png]"), "{text}");
    }

    #[test]
    fn identifier_highlight_can_be_limited_to_team_keys() {
        let theme = Theme::default();
        let highlighted = |r: &Rendered| -> Vec<String> {
            r.lines[0]
                .spans
                .iter()
                .filter(|s| s.style == theme.identifier)
                .map(|s| s.content.to_string())
                .collect()
        };
        let md = "UTF-8 문서와 ENG-12, SHA-256";
        assert_eq!(
            highlighted(&render(md, 80, &theme)),
            vec!["UTF-8", "ENG-12", "SHA-256"]
        );
        assert_eq!(
            highlighted(&render_with(md, 80, &theme, &["eng".to_string()])),
            vec!["ENG-12"]
        );
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib -- markdown:: cli::`
Expected: 컴파일 실패 — `render_with`가 없다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/cli.rs`에서 아래 부분을 찾아:

````rust

pub fn show(ctx: &Ctx, id: &str) -> Result<String> {
    // 다른 워크스페이스의 이슈가 이전 캐시에 섞이지 않도록 먼저 워크스페이스를 맞춘다
    load_viewer(ctx, false)?;
    match queries::issue_detail(&ctx.client, id) {
        Ok(Some(d)) => {
            let removed = ctx
````

아래로 바꾼다:

````rust

pub fn show(ctx: &Ctx, id: &str) -> Result<String> {
    // 다른 워크스페이스의 이슈가 이전 캐시에 섞이지 않도록 먼저 워크스페이스를 맞춘다
    let viewer = load_viewer(ctx, false)?;
    let keys = team_keys(viewer.as_ref());
    match queries::issue_detail(&ctx.client, id) {
        Ok(Some(d)) => {
            let removed = ctx
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
                &d.comments,
                d.more_comments,
                None,
            ))
        }
        Ok(None) => {
````

아래로 바꾼다:

````rust
                &d.comments,
                d.more_comments,
                None,
                &keys,
            ))
        }
        Ok(None) => {
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
                &comments,
                false,
                Some(&format!("오프라인: 저장된 내용 · {msg}")),
            ))
        }
        Err(e) => Err(api_error(ctx, e)),
````

아래로 바꾼다:

````rust
                &comments,
                false,
                Some(&format!("오프라인: 저장된 내용 · {msg}")),
                &keys,
            ))
        }
        Err(e) => Err(api_error(ctx, e)),
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    out.join("\n")
}

fn format_detail(
    ctx: &Ctx,
    issue: &Issue,
    comments: &[Comment],
    more: bool,
    banner: Option<&str>,
) -> String {
    let theme = Theme::default();
    let mut out = Vec::new();
````

아래로 바꾼다:

````rust
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
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    if body.is_empty() {
        out.push("(본문 없음)".to_string());
    } else {
        out.push(render_md(ctx, body, &theme));
    }
    if !comments.is_empty() {
        out.push(String::new());
````

아래로 바꾼다:

````rust
    if body.is_empty() {
        out.push("(본문 없음)".to_string());
    } else {
        out.push(render_md(ctx, body, &theme, team_keys));
    }
    if !comments.is_empty() {
        out.push(String::new());
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
                "{who} · {}",
                short_time(&c.created_at)
            )));
            out.push(render_md(ctx, &c.body, &theme));
        }
        if more {
            out.push(String::new());
````

아래로 바꾼다:

````rust
                "{who} · {}",
                short_time(&c.created_at)
            )));
            out.push(render_md(ctx, &c.body, &theme, team_keys));
        }
        if more {
            out.push(String::new());
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    out.join("\n")
}

fn render_md(ctx: &Ctx, md: &str, theme: &Theme) -> String {
    let r = markdown::render(md, ctx.width, theme);
    let mut text = if ctx.color {
        markdown::to_ansi(&r.lines)
    } else {
````

아래로 바꾼다:

````rust
    out.join("\n")
}

fn render_md(ctx: &Ctx, md: &str, theme: &Theme, team_keys: &[String]) -> String {
    let r = markdown::render_with(md, ctx.width, theme, team_keys);
    let mut text = if ctx.color {
        markdown::to_ansi(&r.lines)
    } else {
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
}

/// markdown을 `width` 칸에 맞춰 그린다. 폭은 최소 10칸으로 본다.
/// 그리기 전에 NFC로 정규화하고 제어 문자를 지운다.
pub fn render(md: &str, width: u16, theme: &Theme) -> Rendered {
    let md = sanitize(&md.nfc().collect::<String>());
    let mut r = Renderer::new(usize::from(width).max(10), *theme);
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for ev in Parser::new_ext(&md, opts) {
        r.event(ev);
````

아래로 바꾼다:

````rust
}

/// markdown을 `width` 칸에 맞춰 그린다. 폭은 최소 10칸으로 본다.
/// 그리기 전에 NFC로 정규화하고 제어 문자를 지운다. 이슈 식별자 형식은 모두 강조한다.
pub fn render(md: &str, width: u16, theme: &Theme) -> Rendered {
    render_with(md, width, theme, &[])
}

/// `render`와 같되, `team_keys`가 있으면 그 팀 키의 식별자만 강조한다 (`UTF-8` 같은 오탐 방지).
pub fn render_with(md: &str, width: u16, theme: &Theme, team_keys: &[String]) -> Rendered {
    let md = sanitize(&md.nfc().collect::<String>());
    let mut r = Renderer::new(usize::from(width).max(10), *theme);
    r.team_keys = team_keys.iter().map(|k| k.to_uppercase()).collect();
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for ev in Parser::new_ext(&md, opts) {
        r.event(ev);
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
static IDENTIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z][A-Z0-9]{0,9}-[0-9]+").expect("valid regex"));

fn find_identifiers(s: &str) -> Vec<(usize, usize)> {
    IDENTIFIER
        .find_iter(s)
        .filter(|m| {
````

아래로 바꾼다:

````rust
static IDENTIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z][A-Z0-9]{0,9}-[0-9]+").expect("valid regex"));

/// `team_keys`가 비어 있지 않으면 그 키로 시작하는 식별자만 고른다.
fn find_identifiers(s: &str, team_keys: &[String]) -> Vec<(usize, usize)> {
    IDENTIFIER
        .find_iter(s)
        .filter(|m| {
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-')
        })
        .map(|m| (m.start(), m.end()))
        .collect()
````

아래로 바꾼다:

````rust
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-')
        })
        .filter(|m| {
            team_keys.is_empty()
                || m.as_str()
                    .split_once('-')
                    .is_some_and(|(key, _)| team_keys.iter().any(|k| k == key))
        })
        .map(|m| (m.start(), m.end()))
        .collect()
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
    row: Vec<String>,
    cell: Option<String>,
    header_rows: usize,
}

struct Renderer {
````

아래로 바꾼다:

````rust
    row: Vec<String>,
    cell: Option<String>,
    header_rows: usize,
    /// 셀 안 링크: (주소, 셀 텍스트에서 링크 글자가 시작하는 위치)
    link: Option<(String, usize)>,
    /// 셀 안 이미지: (주소, alt 텍스트)
    image: Option<(String, String)>,
}

struct Renderer {
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
    image: Option<(String, String)>,
    table: Option<Table>,
    need_blank: bool,
}

impl Renderer {
````

아래로 바꾼다:

````rust
    image: Option<(String, String)>,
    table: Option<Table>,
    need_blank: bool,
    /// 강조할 이슈 식별자의 팀 키 (비어 있으면 모두)
    team_keys: Vec<String>,
}

impl Renderer {
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
            image: None,
            table: None,
            need_blank: false,
        }
    }

````

아래로 바꾼다:

````rust
            image: None,
            table: None,
            need_blank: false,
            team_keys: Vec::new(),
        }
    }

````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
            return;
        }
        let mut last = 0;
        for (start, end) in find_identifiers(s) {
            self.push_seg(s[last..start].to_string(), style);
            self.push_seg(
                s[start..end].to_string(),
````

아래로 바꾼다:

````rust
            return;
        }
        let mut last = 0;
        for (start, end) in find_identifiers(s, &self.team_keys) {
            self.push_seg(s[last..start].to_string(), style);
            self.push_seg(
                s[start..end].to_string(),
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
    }

    fn event(&mut self, ev: Event<'_>) {
        if let Some(t) = self.table.as_mut() {
            match ev {
                Event::Start(Tag::TableCell) => t.cell = Some(String::new()),
                Event::End(TagEnd::TableCell) => {
                    let c = t.cell.take().unwrap_or_default();
                    t.row.push(c.trim().to_string());
                }
                Event::End(TagEnd::TableHead) => {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                    t.header_rows = 1;
                }
                Event::End(TagEnd::TableRow) => {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
                Event::Text(s) | Event::Code(s) | Event::Html(s) | Event::InlineHtml(s) => {
                    if let Some(c) = t.cell.as_mut() {
                        c.push_str(&s);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if let Some(c) = t.cell.as_mut() {
                        c.push(' ');
                    }
                }
                Event::End(TagEnd::Table) => {
                    let t = self.table.take().unwrap_or_default();
                    self.render_table(t);
                }
                _ => {}
            }
            return;
        }
````

아래로 바꾼다:

````rust
    }

    fn event(&mut self, ev: Event<'_>) {
        if let Some(mut t) = self.table.take() {
            if !self.table_event(&mut t, ev) {
                self.table = Some(t);
            }
            return;
        }
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
                        .iter()
                        .map(|(s, _)| s.as_str())
                        .collect();
                    let index = self.links.len() + 1;
                    self.links.push(LinkTarget {
                        index,
                        kind: LinkKind::Link,
                        url,
                        label: label.trim().to_string(),
                    });
                    let st = self.theme.dim;
                    self.push_seg(format!(" [{index}]"), st);
                }
````

아래로 바꾼다:

````rust
                        .iter()
                        .map(|(s, _)| s.as_str())
                        .collect();
                    let index = self.add_link(url, label.trim().to_string());
                    let st = self.theme.dim;
                    self.push_seg(format!(" [{index}]"), st);
                }
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
    }

    fn push_image(&mut self, url: String, alt: String) {
        let index = self.links.len() + 1;
        let label = if alt.trim().is_empty() {
            file_name(&url)
````

아래로 바꾼다:

````rust
    }

    fn push_image(&mut self, url: String, alt: String) {
        let text = self.add_image(url, alt);
        let st = self.theme.dim;
        self.push_seg(text, st);
    }

    /// 링크를 번호 목록에 넣고 번호를 돌려준다.
    fn add_link(&mut self, url: String, label: String) -> usize {
        let index = self.links.len() + 1;
        self.links.push(LinkTarget {
            index,
            kind: LinkKind::Link,
            url,
            label,
        });
        index
    }

    /// 이미지를 번호 목록에 넣고 본문에 보일 자리 표시 문구를 돌려준다.
    fn add_image(&mut self, url: String, alt: String) -> String {
        let index = self.links.len() + 1;
        let label = if alt.trim().is_empty() {
            file_name(&url)
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
            url,
            label: label.clone(),
        });
        let st = self.theme.dim;
        self.push_seg(format!("[이미지 {index}: {label}]"), st);
    }

    /// 목록·인용 접두사. (첫 줄, 이어지는 줄)
````

아래로 바꾼다:

````rust
            url,
            label: label.clone(),
        });
        format!("[이미지 {index}: {label}]")
    }

    /// 표 안의 이벤트. 셀 텍스트를 모으고, 링크·이미지에도 번호를 매긴다. 표를 다 그렸으면 true.
    fn table_event(&mut self, t: &mut Table, ev: Event<'_>) -> bool {
        if let Some((_, alt)) = t.image.as_mut() {
            match ev {
                Event::Text(s) | Event::Code(s) => alt.push_str(&s),
                Event::End(TagEnd::Image) => {
                    if let Some((url, alt)) = t.image.take() {
                        let text = self.add_image(url, alt);
                        if let Some(c) = t.cell.as_mut() {
                            c.push_str(&text);
                        }
                    }
                }
                _ => {}
            }
            return false;
        }
        match ev {
            Event::Start(Tag::TableCell) => t.cell = Some(String::new()),
            Event::End(TagEnd::TableCell) => {
                let c = t.cell.take().unwrap_or_default();
                t.row.push(c.trim().to_string());
            }
            Event::End(TagEnd::TableHead) => {
                let row = std::mem::take(&mut t.row);
                t.rows.push(row);
                t.header_rows = 1;
            }
            Event::End(TagEnd::TableRow) => {
                let row = std::mem::take(&mut t.row);
                t.rows.push(row);
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let start = t.cell.as_ref().map_or(0, String::len);
                t.link = Some((dest_url.to_string(), start));
            }
            Event::End(TagEnd::Link) => {
                if let Some((url, start)) = t.link.take()
                    && let Some(c) = t.cell.as_mut()
                {
                    let label = c.get(start..).unwrap_or("").trim().to_string();
                    let index = self.add_link(url, label);
                    c.push_str(&format!(" [{index}]"));
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                t.image = Some((dest_url.to_string(), String::new()));
            }
            Event::Text(s) | Event::Code(s) | Event::Html(s) | Event::InlineHtml(s) => {
                if let Some(c) = t.cell.as_mut() {
                    c.push_str(&s);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(c) = t.cell.as_mut() {
                    c.push(' ');
                }
            }
            Event::End(TagEnd::Table) => {
                let table = std::mem::take(t);
                self.render_table(table);
                return true;
            }
            _ => {}
        }
        false
    }

    /// 목록·인용 접두사. (첫 줄, 이어지는 줄)
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
        for raw in code.trim_end_matches('\n').split('\n') {
            let text = raw.replace('\t', "    ");
            let segs = vec![(text, self.theme.code)];
            self.out
                .extend(wrap(&segs, &first, &rest, self.width, true));
        }
        self.need_blank = true;
    }
````

아래로 바꾼다:

````rust
        for raw in code.trim_end_matches('\n').split('\n') {
            let text = raw.replace('\t', "    ");
            let segs = vec![(text, self.theme.code)];
            for mut line in wrap(&segs, &first, &rest, self.width, true) {
                // 코드 블록 배경을 폭 끝까지 칠한다
                let w = line.width();
                if w < self.width {
                    line.spans
                        .push(Span::styled(" ".repeat(self.width - w), self.theme.code));
                }
                self.out.push(line);
            }
        }
        self.need_blank = true;
    }
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib -- markdown:: cli::`
Expected: `test result: ok. 53 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 141개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/cli.rs src/markdown.rs
git commit -m "feat(markdown): 표 안 링크 번호, 팀 키 강조, 코드 블록 배경" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 4: Linear 색과 이슈 줄 (ui), CLI 목록 색

팔레트와 CLI가 같이 쓰는 표시 부품을 만든다. 사용자가 요청한 "리스트에서 색상"이 여기서 나온다.

- `ui::style`: Linear 색 문자열(`#rrggbb`, `#rgb`)을 터미널 색으로 바꾼다. 상태 아이콘과 상태 색(색이 없으면 종류별 기본색), 라벨 색, 우선순위 이름, 표시 폭 기준 자르기(`…`)를 제공한다.
- `ui::row::issue_row(issue, width)`: 한 줄에 상태 아이콘(상태 색), 식별자(흐리게, 9칸), 제목, 라벨(라벨 색), `@담당자`(흐리게)를 넣는다. 폭이 모자라면 꼬리(라벨·담당자)부터 버리고 제목을 자른다. 제목이 12칸보다 좁아지지 않게 한다.
- CLI 목록(`mine`, `search`)은 표준 출력이 터미널이면 같은 줄을 ANSI 색으로 출력한다. `NO_COLOR`가 비어 있지 않은 값으로 설정돼 있으면 색을 쓰지 않는다(no-color.org).

**Files:**
- Create: `src/ui/mod.rs`, `src/ui/row.rs`, `src/ui/style.rs`
- Modify: `src/lib.rs`, `src/cli.rs`
- Test: `src/ui/row.rs`, `src/ui/style.rs`, `src/cli.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `linear::types::{Issue, StateRef, LabelRef}`, `markdown::{sanitize, to_ansi}`
- Produces:
  - `ui::style::{DIM, ACCENT}: Style`
  - `ui::style::hex_color(&str) -> Option<Color>`, `state_icon(state_type: &str) -> &'static str`, `state_style(&StateRef) -> Style`, `label_style(&LabelRef) -> Style`, `priority_label(i64) -> &'static str`, `truncate(s: &str, width: usize) -> String`
  - `ui::row::issue_row(issue: &Issue, width: u16) -> Line<'static>`
  - `cli::use_color(is_terminal: bool, no_color: Option<&OsStr>) -> bool`
  - `cli`는 `state_icon`, `priority_label`을 `ui::style`에서 다시 내보낸다(`pub use`)

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/lib.rs`에서 아래 부분을 찾아:

````rust
pub mod markdown;
pub mod search;
pub mod store;

#[cfg(test)]
pub mod test_support;
````

아래로 바꾼다:

````rust
pub mod markdown;
pub mod search;
pub mod store;
pub mod ui;

#[cfg(test)]
pub mod test_support;
````

`src/ui/mod.rs` 파일을 아래 내용으로 만든다:

````rust
//! 화면 공통: Linear 색, 상태 아이콘, 목록 한 줄.

pub mod row;
pub mod style;
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            Some("경고: config의 teams(EGN)와 맞는 팀이 없어서 모든 팀에서 찾아요")
        );
    }
}
````

아래로 바꾼다:

````rust
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
````

`src/ui/row.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::to_plain;
    use crate::test_support::IssueBuilder;
    use ratatui::style::Color;

    fn issue() -> Issue {
        IssueBuilder::new("i1", "UP-1812", "데이터 손상 수정")
            .state("In Progress", "started")
            .labels(&["Bug", "Backend"])
            .assignee("u1", "jhhan")
            .build()
    }

    #[test]
    fn row_shows_colored_icon_labels_and_assignee() {
        let line = issue_row(&issue(), 80);
        assert_eq!(
            to_plain(std::slice::from_ref(&line)),
            "◐ UP-1812   데이터 손상 수정  Bug Backend  @jhhan"
        );
        assert_eq!(line.spans[0].style.fg, Some(Color::Rgb(94, 106, 210)));
        let bug = line.spans.iter().find(|s| s.content == "Bug").unwrap();
        assert_eq!(bug.style.fg, Some(Color::Rgb(235, 87, 87)));
    }

    #[test]
    fn narrow_row_drops_tail_then_truncates_title() {
        let line = issue_row(&issue(), 24);
        let text = to_plain(std::slice::from_ref(&line));
        assert!(!text.contains("Bug"), "{text}");
        assert!(line.width() <= 24, "{text}");
        assert!(text.ends_with('…'), "{text}");
    }

    #[test]
    fn row_strips_control_characters() {
        let i = IssueBuilder::new("i1", "UP-1", "제목\u{1b}[2J").build();
        let text = to_plain(&[issue_row(&i, 80)]);
        assert!(!text.contains('\u{1b}'), "{text:?}");
    }
}
````

`src/ui/style.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;

    #[test]
    fn hex_colors_parse() {
        assert_eq!(hex_color("#5e6ad2"), Some(Color::Rgb(94, 106, 210)));
        assert_eq!(hex_color(" #ABC "), Some(Color::Rgb(170, 187, 204)));
        for bad in ["5e6ad2", "#zzzzzz", "#12", "#가나다", "", "#1234567"] {
            assert_eq!(hex_color(bad), None, "{bad}");
        }
    }

    #[test]
    fn state_style_uses_linear_color_with_fallback() {
        let issue = IssueBuilder::new("i", "ENG-1", "t")
            .state("Doing", "started")
            .build();
        assert_eq!(state_style(&issue.state).fg, Some(Color::Rgb(94, 106, 210)));
        let mut broken = issue.state.clone();
        broken.color = "blue".into();
        assert_eq!(state_style(&broken).fg, Some(Color::Yellow));
    }

    #[test]
    fn truncate_by_display_width() {
        assert_eq!(truncate("로그인 버튼", 20), "로그인 버튼");
        assert_eq!(truncate("로그인 버튼 비활성", 8), "로그인…");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 0), "");
        assert!(truncate("가나다라마바", 7).width() <= 7);
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib -- ui:: cli::`
Expected: 컴파일 실패 — `ui` 모듈의 함수(`hex_color`, `issue_row` 등)와 `use_color`가 없다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/cli.rs`에서 아래 부분을 찾아:

````rust
use crate::search::query::parse;
use crate::search::rank::{SearchIndex, merge, sort_mine};
use crate::store::{Store, remove_db_files};

const VIEWER_TTL_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
````

아래로 바꾼다:

````rust
use crate::search::query::parse;
use crate::search::rank::{SearchIndex, merge, sort_mine};
use crate::store::{Store, remove_db_files};
use crate::ui::row::issue_row;
pub use crate::ui::style::{priority_label, state_icon};

const VIEWER_TTL_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    chrono::Utc::now().timestamp_millis()
}

impl Ctx {
    pub fn open(paths: Paths) -> Result<Ctx> {
        let (settings, warnings) = config::load_settings(&paths.config_file());
````

아래로 바꾼다:

````rust
    chrono::Utc::now().timestamp_millis()
}

/// 색을 쓸지: 표준 출력이 터미널이고 `NO_COLOR`가 비어 있지 않은 값으로 설정되지 않았을 때 (no-color.org).
pub fn use_color(is_terminal: bool, no_color: Option<&std::ffi::OsStr>) -> bool {
    is_terminal && no_color.is_none_or(|v| v.is_empty())
}

impl Ctx {
    pub fn open(paths: Paths) -> Result<Ctx> {
        let (settings, warnings) = config::load_settings(&paths.config_file());
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            client: LinearClient::new(key.value),
            key_source: key.source,
            now_ms: now,
            color: std::io::stdout().is_terminal(),
            width,
        })
    }
````

아래로 바꾼다:

````rust
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
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            sort_mine(&mut issues);
            let ids: Vec<String> = issues.iter().map(|i| i.id.clone()).collect();
            ctx.store.set_view("mine", &ids, ctx.now_ms)?;
            Ok(format_list(&issues, None))
        }
        Err(ApiError::Offline(msg)) => match ctx.store.get_view("mine")? {
            Some((issues, at)) => Ok(format_list(
                &issues,
                Some(&format!(
                    "오프라인: {} 저장된 결과 · {msg}",
````

아래로 바꾼다:

````rust
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
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            ctx.store.upsert_issues(&found, ctx.now_ms)?;
            let found: Vec<Issue> = found.into_iter().filter(|i| !i.is_gone()).collect();
            let merged = merge(&local, &found, &q, viewer_id);
            Ok(format_list(&merged[..merged.len().min(SEARCH_LIMIT)], None))
        }
        Err(ApiError::Offline(msg)) => Ok(format_list(
            &local[..local.len().min(SEARCH_LIMIT)],
            Some(&format!("오프라인: 저장된 이슈에서만 찾았어요 · {msg}")),
        )),
````

아래로 바꾼다:

````rust
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
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    }
}

pub fn state_icon(state_type: &str) -> &'static str {
    match state_type {
        "triage" => "◇",
        "backlog" => "◌",
        "unstarted" => "○",
        "started" => "◐",
        "completed" => "●",
        "canceled" | "duplicate" => "✕",
        _ => "·",
    }
}

pub fn priority_label(p: i64) -> &'static str {
    match p {
        1 => "긴급",
        2 => "높음",
        3 => "보통",
        4 => "낮음",
        _ => "없음",
    }
}

pub fn ago(now_ms: i64, then_ms: i64) -> String {
    let mins = (now_ms - then_ms).max(0) / 60_000;
    match mins {
````

아래로 바꾼다:

````rust
    }
}

pub fn ago(now_ms: i64, then_ms: i64) -> String {
    let mins = (now_ms - then_ms).max(0) / 60_000;
    match mins {
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    markdown::sanitize(&s)
}

fn format_list(issues: &[Issue], banner: Option<&str>) -> String {
    let mut out = Vec::new();
    if let Some(b) = banner {
        out.push(format!("({b})"));
````

아래로 바꾼다:

````rust
    markdown::sanitize(&s)
}

/// 목록 출력. 터미널이면 상태·라벨에 Linear 색을 입힌다.
fn format_list(ctx: &Ctx, issues: &[Issue], banner: Option<&str>) -> String {
    let mut out = Vec::new();
    if let Some(b) = banner {
        out.push(format!("({b})"));
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    if issues.is_empty() {
        out.push("결과가 없어요".to_string());
    }
    out.extend(issues.iter().map(issue_line));
    out.join("\n")
}

````

아래로 바꾼다:

````rust
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

````

`src/ui/row.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
//! 목록 한 줄.

use ratatui::text::{Line, Span};

use super::style::{DIM, label_style, state_icon, state_style, truncate};
use crate::linear::types::Issue;
use crate::markdown::sanitize;

/// 제목이 이보다 좁아지면 라벨과 담당자를 숨긴다.
const MIN_TITLE: usize = 12;

/// 목록 한 줄: 상태 아이콘(상태 색) · 식별자 · 제목 · 라벨(라벨 색) · @담당자.
/// 폭이 모자라면 라벨·담당자를 먼저 빼고, 그래도 넘치면 제목을 `…`로 줄인다.
pub fn issue_row(issue: &Issue, width: u16) -> Line<'static> {
    let width = usize::from(width);
    let icon = Span::styled(
        format!("{} ", state_icon(&issue.state.state_type)),
        state_style(&issue.state),
    );
    let id = Span::styled(format!("{:<9} ", issue.identifier), DIM);
    let fixed = icon.width() + id.width();
    let mut tail: Vec<Span<'static>> = Vec::new();
    for (i, label) in issue.labels.nodes.iter().enumerate() {
        tail.push(Span::raw(if i == 0 { "  " } else { " " }));
        tail.push(Span::styled(sanitize(&label.name), label_style(label)));
    }
    if let Some(a) = &issue.assignee {
        tail.push(Span::styled(
            format!("  @{}", sanitize(&a.display_name)),
            DIM,
        ));
    }
    let tail_width: usize = tail.iter().map(Span::width).sum();
    let title = sanitize(&issue.title);
    let (room, tail) = if fixed + MIN_TITLE + tail_width <= width {
        (width - fixed - tail_width, tail)
    } else {
        (width.saturating_sub(fixed), Vec::new())
    };
    let mut spans = vec![icon, id, Span::raw(truncate(&title, room))];
    spans.extend(tail);
    Line::from(spans)
}

````

`src/ui/style.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
//! Linear의 색과 상태를 터미널 스타일로 바꾼다.

use ratatui::style::{Color, Modifier, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::linear::types::{LabelRef, StateRef};

/// 흐린 글자 (식별자, 담당자, 보조 정보)
pub const DIM: Style = Style::new().fg(Color::DarkGray);
/// 강조 (활성 탭, 제목)
pub const ACCENT: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);

/// `#rrggbb` 또는 `#rgb`를 RGB 색으로 바꾼다. 형식이 틀리면 `None`.
pub fn hex_color(s: &str) -> Option<Color> {
    let h = s.trim().strip_prefix('#')?;
    if !h.is_ascii() {
        return None;
    }
    let byte = |i: usize, n: usize| u8::from_str_radix(&h[i..i + n], 16).ok();
    match h.len() {
        6 => Some(Color::Rgb(byte(0, 2)?, byte(2, 2)?, byte(4, 2)?)),
        3 => Some(Color::Rgb(
            byte(0, 1)? * 17,
            byte(1, 1)? * 17,
            byte(2, 1)? * 17,
        )),
        _ => None,
    }
}

/// 상태 타입별 아이콘.
pub fn state_icon(state_type: &str) -> &'static str {
    match state_type {
        "triage" => "◇",
        "backlog" => "◌",
        "unstarted" => "○",
        "started" => "◐",
        "completed" => "●",
        "canceled" | "duplicate" => "✕",
        _ => "·",
    }
}

/// 상태 아이콘 스타일: Linear에 설정된 상태 색. 색이 없거나 틀리면 타입별 기본색.
pub fn state_style(state: &StateRef) -> Style {
    let fallback = match state.state_type.as_str() {
        "started" => Color::Yellow,
        "completed" => Color::Green,
        "canceled" | "duplicate" => Color::DarkGray,
        _ => Color::Gray,
    };
    Style::new().fg(hex_color(&state.color).unwrap_or(fallback))
}

/// 라벨 스타일: Linear에 설정된 라벨 색.
pub fn label_style(label: &LabelRef) -> Style {
    Style::new().fg(hex_color(&label.color).unwrap_or(Color::Gray))
}

/// 우선순위 이름.
pub fn priority_label(p: i64) -> &'static str {
    match p {
        1 => "긴급",
        2 => "높음",
        3 => "보통",
        4 => "낮음",
        _ => "없음",
    }
}

/// 표시 폭 `width`에 맞게 자른다. 넘치면 끝을 `…`로 바꾼다.
pub fn truncate(s: &str, width: usize) -> String {
    if s.width() <= width {
        return s.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    let mut out = out.trim_end().to_string();
    out.push('…');
    out
}

````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib -- ui:: cli::`
Expected: `test result: ok. 34 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 149개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/ui/mod.rs src/cli.rs src/ui/row.rs src/ui/style.rs
git commit -m "feat(ui): Linear 색으로 그리는 이슈 줄, CLI 목록 색" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 5: 팔레트 상태 머신 (tui::app)

팔레트의 동작을 화면·네트워크와 떨어진 순수 상태 머신으로 만든다. 런타임은 키를 `Input`으로 바꿔 `App::handle`에 넘기고, 캐시·네트워크 결과를 `Msg`로 `App::apply`에 넘긴다. 앱은 할 일을 `Effect`로 돌려준다. 시간은 인자(`now`, 밀리초)로 받아서 테스트가 시계를 정한다.

- **모드**: 키 입력(Onboarding), 검색(기본, 글자 = 검색어), 목록(한 글자 키 = 동작), 상세.
- **탭**: 내 이슈 · 최근 본 · 전체. 탭은 처음 열 때 한 번 불러온다. 최근 본은 열 때마다 다시 읽는다. 전체 탭은 끝에 닿으면 다음 페이지를 부른다.
- **검색**: 입력하면 로컬 색인에서 바로 찾는다. 입력이 300ms 멈추면 서버 검색 `Effect::Search{seq}`를 낸다. 늦게 온 이전 순번의 응답은 버린다. 결과를 합쳐도 고른 이슈는 id로 유지하고, 검색어가 바뀌면 처음으로 간다. 검색어가 있으면 맨 아래에 "서버에서 검색 (코멘트 포함)" 줄을 둔다.
- **현재 브랜치 이슈**: 검색어가 비어 있으면 맨 위에 고정하고, 목록에서는 중복을 뺀다.
- **상세**: 캐시 내용을 먼저 보이고 새 응답으로 바꾼다. 다른 이슈의 늦은 응답은 무시한다. 보관·삭제된 이슈(`DetailGone`, 요청한 id 또는 식별자)는 표시하고 목록에서 뺀다.
- **메뉴**: Ctrl+K 동작 메뉴, 본문·코멘트 링크 목록(`u`). 글자를 치면 거른다.
- **불러오는 중 표시**: 세는 요청은 LoadTab, LoadMore, Search, DeepSearch, OpenDetail이다. Tab(fresh)·Search·Detail(fresh)·DetailGone·Failed가 하나씩 끝내고, AuthFailed는 0으로 되돌린다. 최근 본은 로컬이라 갱신 시각과 오프라인 표시를 바꾸지 않는다.
- **한도**: `Failed(RateLimited)`면 리셋까지 자동 서버 검색을 멈춘다. `Throttled(리셋)`(남은 요청이 50 미만)도 같은 방식으로 멈추고 안내한다. 직접 새로고침하면 바로 보낸다.

**Files:**
- Create: `src/tui/mod.rs`, `src/tui/app.rs`
- Modify: `src/lib.rs`
- Test: `src/tui/app.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `search::rank::{SearchIndex, merge}`(Task 2 `upsert` 포함), `search::query::parse`, `markdown::render`, `linear::types`, `linear::client::ApiError`
- Produces:
  - `tui::app::{SEARCH_DEBOUNCE_MS (300), FLASH_MS (3000)}`
  - `enum Tab { Mine, Recent, All }` (`Tab::ALL`, `title()`), `enum Mode { Onboarding, Search, List, Detail }`
  - `enum Act { Open, Browser, CopyId, CopyUrl, Refresh, DeepSearch, Links, Back, Quit, OpenUrl(String) }`
  - `enum Input { Char(char), Paste(String), Backspace, ClearLine, Up, Down, PageUp, PageDown, Top, Bottom, Enter, Esc, NextTab, PrevTab, Menu, Search, Act(Act), Quit }`
  - `enum Effect { Init, LoadTab(Tab), LoadMore, Search { seq: u64, query: String }, DeepSearch { seq: u64, query: String }, OpenDetail(String), ResolvePinned, OpenUrl(String), Copy { text: String, what: String }, ValidateKey(String) }`
  - `enum Msg { Viewer(Viewer), Index(Vec<Issue>), Tab { tab, issues, fresh, has_more, append }, Search { seq, issues }, Detail { id, issue, comments, more, fresh }, DetailGone(String), Pinned(Option<Issue>), KeyOk(Viewer), KeyBad(String), AuthFailed { env: bool }, Failed(ApiError), Throttled(i64), Flash(String) }`
  - `enum Problem { Offline(String), RateLimited(Option<i64>), Error(String) }`, `enum Row { Pinned(Issue), Issue(Issue), DeepSearch }`, `struct Detail`, `struct Menu`
  - `App::start(open: Option<String>) -> (App, Vec<Effect>)`, `App::onboarding(env_invalid: bool) -> App`
  - `App::handle(&mut self, Input, now: i64) -> Vec<Effect>`, `apply(&mut self, Msg, now) -> Vec<Effect>`, `tick(&mut self, now) -> Vec<Effect>`
  - `App::selected_issue()`, `team_keys()`, `flash_text(now) -> Option<&str>`, `set_detail_max_scroll(u16)`와 공개 필드(`mode`, `tab`, `query`, `rows`, `selected`, `detail`, `menu`, `key_input`, `key_error`, `key_checking`, `env_key_invalid`, `loading`, `updated_at`, `problem`, `flash`, `quit`, `viewer`)

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/lib.rs`에서 아래 부분을 찾아:

````rust
pub mod markdown;
pub mod search;
pub mod store;
pub mod ui;

#[cfg(test)]
````

아래로 바꾼다:

````rust
pub mod markdown;
pub mod search;
pub mod store;
pub mod tui;
pub mod ui;

#[cfg(test)]
````

`src/tui/mod.rs` 파일을 아래 내용으로 만든다:

````rust
//! herdr 팝업 팔레트 TUI.

pub mod app;
````

`src/tui/app.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;

    const T0: i64 = 1_000_000;

    fn issue(id: &str, identifier: &str, title: &str) -> Issue {
        IssueBuilder::new(id, identifier, title).build()
    }

    fn viewer() -> Viewer {
        serde_json::from_value(serde_json::json!({
            "id": "me", "name": "김민수", "displayName": "minsu", "email": "m@acme.dev",
            "organization": { "id": "org1", "name": "Acme", "urlKey": "acme" },
            "teams": { "nodes": [ { "id": "team-ENG", "key": "ENG", "name": "Eng" } ] }
        }))
        .unwrap()
    }

    fn tab_msg(tab: Tab, issues: Vec<Issue>, fresh: bool) -> Msg {
        Msg::Tab {
            tab,
            issues,
            fresh,
            has_more: false,
            append: false,
        }
    }

    fn started() -> App {
        let (mut app, _) = App::start(None);
        app.apply(Msg::Viewer(viewer()), T0);
        app.apply(
            tab_msg(
                Tab::Mine,
                vec![
                    issue("a", "ENG-1", "로그인 버그"),
                    issue("b", "ENG-2", "결제 화면"),
                ],
                true,
            ),
            T0,
        );
        app
    }

    fn ids(app: &App) -> Vec<String> {
        app.rows
            .iter()
            .map(|r| r.issue().map_or("<deep>".into(), |i| i.identifier.clone()))
            .collect()
    }

    fn type_str(app: &mut App, s: &str, now: i64) {
        for c in s.chars() {
            app.handle(Input::Char(c), now);
        }
    }

    #[test]
    fn start_loads_mine_and_pinned() {
        let (app, effects) = App::start(None);
        assert_eq!(app.mode, Mode::Search);
        assert_eq!(
            effects,
            vec![
                Effect::Init,
                Effect::LoadTab(Tab::Mine),
                Effect::ResolvePinned
            ]
        );
        assert_eq!(app.loading, 1);
    }

    #[test]
    fn start_with_identifier_opens_detail() {
        let (app, effects) = App::start(Some("ENG-7".into()));
        assert_eq!(app.mode, Mode::Detail);
        assert_eq!(effects.last(), Some(&Effect::OpenDetail("ENG-7".into())));
        assert_eq!(app.loading, 2);
    }

    #[test]
    fn cached_then_fresh_tab() {
        let (mut app, _) = App::start(None);
        app.apply(
            tab_msg(Tab::Mine, vec![issue("a", "ENG-1", "옛 제목")], false),
            T0,
        );
        assert_eq!(ids(&app), vec!["ENG-1"]);
        assert_eq!(app.loading, 1, "캐시는 요청을 끝내지 않는다");
        app.apply(
            tab_msg(Tab::Mine, vec![issue("a", "ENG-1", "새 제목")], true),
            T0 + 5,
        );
        assert_eq!(app.rows[0].issue().unwrap().title, "새 제목");
        assert_eq!(app.loading, 0);
        assert_eq!(app.updated_at, Some(T0 + 5));
    }

    #[test]
    fn typing_searches_locally_then_debounces_server_search() {
        let mut app = started();
        type_str(&mut app, "결제", T0);
        assert_eq!(ids(&app), vec!["ENG-2", "<deep>"]);
        assert!(app.tick(T0 + 100).is_empty());
        let effects = app.tick(T0 + SEARCH_DEBOUNCE_MS);
        assert_eq!(
            effects,
            vec![Effect::Search {
                seq: 1,
                query: "결제".into()
            }]
        );
        app.apply(
            Msg::Search {
                seq: 1,
                issues: vec![issue("c", "ENG-3", "결제 실패 알림")],
            },
            T0 + 400,
        );
        // 순위가 같으면 서버 결과가 먼저 온다. 검색 행은 항상 마지막
        let found = ids(&app);
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found.contains(&"ENG-2".to_string()) && found.contains(&"ENG-3".to_string()));
        assert_eq!(found.last().map(String::as_str), Some("<deep>"));
        assert_eq!(app.loading, 0);
    }

    #[test]
    fn stale_search_results_are_ignored() {
        let mut app = started();
        type_str(&mut app, "로", T0);
        app.tick(T0 + SEARCH_DEBOUNCE_MS);
        type_str(&mut app, "그", T0 + 350);
        app.tick(T0 + 350 + SEARCH_DEBOUNCE_MS);
        app.apply(
            Msg::Search {
                seq: 1,
                issues: vec![issue("x", "ENG-9", "로그 수집")],
            },
            T0 + 700,
        );
        assert!(!ids(&app).contains(&"ENG-9".to_string()));
        assert_eq!(app.loading, 1, "2번 요청은 아직 기다린다");
    }

    #[test]
    fn selection_kept_by_id_on_merge_and_reset_on_typing() {
        let mut app = started();
        type_str(&mut app, "ENG", T0);
        app.handle(Input::Down, T0);
        let chosen = app.selected_issue().unwrap().id.clone();
        app.tick(T0 + SEARCH_DEBOUNCE_MS);
        app.apply(
            Msg::Search {
                seq: 1,
                issues: vec![issue("z", "ENG-0", "ENG 맨 앞에 올 결과")],
            },
            T0 + 400,
        );
        assert_eq!(app.selected_issue().unwrap().id, chosen);
        app.handle(Input::Char('-'), T0 + 500);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn pinned_issue_comes_first_without_duplicate() {
        let mut app = started();
        app.apply(Msg::Pinned(Some(issue("b", "ENG-2", "결제 화면"))), T0);
        assert!(matches!(app.rows[0], Row::Pinned(_)));
        assert_eq!(ids(&app), vec!["ENG-2", "ENG-1"]);
        type_str(&mut app, "로그인", T0);
        assert!(!app.rows.iter().any(|r| matches!(r, Row::Pinned(_))));
    }

    #[test]
    fn enter_opens_detail_and_esc_goes_back() {
        let mut app = started();
        let effects = app.handle(Input::Enter, T0);
        assert_eq!(effects, vec![Effect::OpenDetail("a".into())]);
        assert_eq!(app.mode, Mode::Detail);
        app.apply(
            Msg::Detail {
                id: "a".into(),
                issue: issue("a", "ENG-1", "로그인 버그"),
                comments: Vec::new(),
                more: false,
                fresh: true,
            },
            T0,
        );
        assert!(!app.detail.as_ref().unwrap().loading);
        app.handle(Input::Esc, T0);
        assert_eq!(app.mode, Mode::Search);
        assert!(app.detail.is_none());
    }

    #[test]
    fn detail_by_identifier_accepts_real_id() {
        let (mut app, _) = App::start(Some("eng-1".into()));
        app.apply(
            Msg::Detail {
                id: "a".into(),
                issue: issue("a", "ENG-1", "로그인 버그"),
                comments: Vec::new(),
                more: false,
                fresh: true,
            },
            T0,
        );
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.issue.as_ref().unwrap().identifier, "ENG-1");
        assert_eq!(d.id, "a");
    }

    #[test]
    fn late_detail_for_another_issue_is_ignored() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        app.apply(
            Msg::Detail {
                id: "b".into(),
                issue: issue("b", "ENG-2", "결제 화면"),
                comments: vec![],
                more: false,
                fresh: true,
            },
            T0,
        );
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.issue.as_ref().unwrap().identifier, "ENG-1");
        assert!(d.loading, "자기 응답을 아직 기다린다");
    }

    #[test]
    fn gone_issue_is_marked_and_removed() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        app.apply(Msg::DetailGone("a".into()), T0);
        assert!(app.detail.as_ref().unwrap().gone);
        app.handle(Input::Esc, T0);
        assert_eq!(ids(&app), vec!["ENG-2"]);
    }

    #[test]
    fn gone_matches_the_requested_identifier() {
        // 캐시가 없으면 상세는 식별자만 안다
        let (mut app, _) = App::start(Some("ENG-7".into()));
        app.apply(Msg::DetailGone("ENG-7".into()), T0);
        let d = app.detail.as_ref().unwrap();
        assert!(d.gone && !d.loading);
        // 캐시가 먼저 와서 상세의 id가 바뀐 뒤에도
        let (mut app, _) = App::start(Some("eng-7".into()));
        app.apply(
            Msg::Detail {
                id: "eng-7".into(),
                issue: issue("x7", "ENG-7", "옛 이슈"),
                comments: vec![],
                more: false,
                fresh: false,
            },
            T0,
        );
        app.apply(Msg::DetailGone("eng-7".into()), T0);
        assert!(app.detail.as_ref().unwrap().gone);
        assert_eq!(app.loading, 1, "상세 요청 하나만 끝났다");
    }

    #[test]
    fn tabs_cycle_and_only_recent_reloads() {
        let mut app = started();
        assert_eq!(
            app.handle(Input::NextTab, T0),
            vec![Effect::LoadTab(Tab::Recent)]
        );
        assert_eq!(app.tab, Tab::Recent);
        assert_eq!(
            app.handle(Input::NextTab, T0),
            vec![Effect::LoadTab(Tab::All)]
        );
        assert!(app.handle(Input::NextTab, T0).is_empty(), "이미 불러온 탭");
        assert_eq!(app.tab, Tab::Mine);
        assert!(app.handle(Input::PrevTab, T0).is_empty(), "이미 불러온 탭");
        assert_eq!(app.tab, Tab::All);
        assert_eq!(
            app.handle(Input::PrevTab, T0),
            vec![Effect::LoadTab(Tab::Recent)],
            "최근 본은 열 때마다 다시 읽는다"
        );
    }

    #[test]
    fn recent_tab_keeps_server_status() {
        let mut app = started();
        app.apply(Msg::Failed(ApiError::Offline("연결 끊김".into())), T0);
        let updated = app.updated_at;
        app.handle(Input::NextTab, T0);
        app.apply(
            tab_msg(Tab::Recent, vec![issue("a", "ENG-1", "로그인 버그")], true),
            T0 + 10,
        );
        assert_eq!(ids(&app), vec!["ENG-1"]);
        assert_eq!(app.loading, 0);
        assert_eq!(app.problem, Some(Problem::Offline("연결 끊김".into())));
        assert_eq!(app.updated_at, updated);
    }

    #[test]
    fn all_tab_loads_more_at_the_end() {
        let mut app = started();
        app.handle(Input::NextTab, T0);
        app.handle(Input::NextTab, T0);
        app.apply(
            Msg::Tab {
                tab: Tab::All,
                issues: vec![issue("a", "ENG-1", "하나"), issue("b", "ENG-2", "둘")],
                fresh: true,
                has_more: true,
                append: false,
            },
            T0,
        );
        assert!(app.handle(Input::Down, T0).contains(&Effect::LoadMore));
        assert!(
            app.handle(Input::Down, T0).is_empty(),
            "불러오는 중엔 다시 안 부른다"
        );
        app.apply(
            Msg::Tab {
                tab: Tab::All,
                issues: vec![issue("c", "ENG-3", "셋")],
                fresh: true,
                has_more: false,
                append: true,
            },
            T0,
        );
        assert_eq!(ids(&app), vec!["ENG-1", "ENG-2", "ENG-3"]);
    }

    #[test]
    fn deep_search_row_runs_deep_search() {
        let mut app = started();
        type_str(&mut app, "세션", T0);
        assert_eq!(ids(&app), vec!["<deep>"]);
        assert_eq!(
            app.handle(Input::Enter, T0),
            vec![Effect::DeepSearch {
                seq: 1,
                query: "세션".into()
            }]
        );
    }

    #[test]
    fn list_mode_actions() {
        let mut app = started();
        app.handle(Input::Esc, T0);
        assert_eq!(app.mode, Mode::List);
        assert_eq!(
            app.handle(Input::Act(Act::Browser), T0),
            vec![Effect::OpenUrl(
                "https://linear.app/acme/issue/ENG-1".into()
            )]
        );
        assert_eq!(
            app.handle(Input::Act(Act::CopyId), T0),
            vec![Effect::Copy {
                text: "ENG-1".into(),
                what: "ENG-1".into()
            }]
        );
        app.handle(Input::Search, T0);
        assert_eq!(app.mode, Mode::Search);
        app.handle(Input::Esc, T0);
        app.handle(Input::Esc, T0);
        assert!(app.quit);
    }

    #[test]
    fn menu_filters_and_runs_actions() {
        let mut app = started();
        app.handle(Input::Menu, T0);
        let labels: Vec<String> = app
            .menu
            .as_ref()
            .unwrap()
            .visible()
            .iter()
            .map(|(l, _)| l.clone())
            .collect();
        assert!(labels.contains(&"상세 보기".to_string()), "{labels:?}");
        type_str(&mut app, "URL", T0);
        assert_eq!(app.menu.as_ref().unwrap().visible().len(), 1);
        assert_eq!(
            app.handle(Input::Enter, T0),
            vec![Effect::Copy {
                text: "https://linear.app/acme/issue/ENG-1".into(),
                what: "ENG-1 URL".into()
            }]
        );
        assert!(app.menu.is_none());
    }

    #[test]
    fn links_menu_lists_description_links() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        let with_links = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .description("[문서](https://x.dev/doc) ![](https://x.dev/a.png)")
            .build();
        app.apply(
            Msg::Detail {
                id: "a".into(),
                issue: with_links,
                comments: Vec::new(),
                more: false,
                fresh: true,
            },
            T0,
        );
        app.handle(Input::Act(Act::Links), T0);
        let menu = app.menu.as_ref().unwrap();
        assert_eq!(menu.items.len(), 2);
        assert_eq!(
            app.handle(Input::Enter, T0),
            vec![Effect::OpenUrl("https://x.dev/doc".into())]
        );
    }

    #[test]
    fn rate_limit_pauses_server_search_until_reset() {
        let mut app = started();
        app.apply(
            Msg::Failed(ApiError::RateLimited {
                reset_at_ms: Some(T0 + 60_000),
            }),
            T0,
        );
        assert_eq!(app.problem, Some(Problem::RateLimited(Some(T0 + 60_000))));
        type_str(&mut app, "결제", T0);
        assert!(app.tick(T0 + 1_000).is_empty());
        assert_eq!(app.tick(T0 + 60_000).len(), 1);
    }

    #[test]
    fn throttle_pauses_auto_search_but_refresh_still_searches() {
        let mut app = started();
        app.apply(Msg::Throttled(T0 + 120_000), T0);
        assert!(app.flash_text(T0).unwrap().contains("2분"));
        assert_eq!(app.loading, 0, "요청을 세지 않는다");
        type_str(&mut app, "결제", T0);
        assert!(app.tick(T0 + 1_000).is_empty(), "자동 검색은 멈춘다");
        app.handle(Input::Esc, T0 + 1_000);
        app.handle(Input::Act(Act::Refresh), T0 + 1_000);
        assert_eq!(app.tick(T0 + 1_000).len(), 1, "직접 새로고침은 보낸다");
    }

    #[test]
    fn onboarding_validates_key() {
        let mut app = App::onboarding(false);
        app.handle(Input::Paste(" lin_api_abc\n".into()), T0);
        assert_eq!(app.key_input, "lin_api_abc");
        assert_eq!(
            app.handle(Input::Enter, T0),
            vec![Effect::ValidateKey("lin_api_abc".into())]
        );
        app.apply(Msg::KeyBad("키가 유효하지 않아요".into()), T0);
        assert_eq!(app.key_error.as_deref(), Some("키가 유효하지 않아요"));
        app.handle(Input::Enter, T0);
        let effects = app.apply(Msg::KeyOk(viewer()), T0);
        assert_eq!(app.mode, Mode::Search);
        assert_eq!(effects[0], Effect::Init);
        assert_eq!(
            app.flash_text(T0),
            Some("김민수님, Acme 워크스페이스에 연결됐어요")
        );
        assert_eq!(app.flash_text(T0 + FLASH_MS), None);
    }

    #[test]
    fn auth_failure_returns_to_onboarding() {
        let mut app = started();
        app.apply(Msg::AuthFailed { env: true }, T0);
        assert_eq!(app.mode, Mode::Onboarding);
        assert!(app.env_key_invalid);
        assert!(app.handle(Input::Char('x'), T0).is_empty());
        assert!(app.key_input.is_empty());
        app.handle(Input::Esc, T0);
        assert!(app.quit);
    }

    #[test]
    fn detail_scroll_is_clamped() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        app.set_detail_max_scroll(3);
        for _ in 0..10 {
            app.handle(Input::Down, T0);
        }
        assert_eq!(app.detail.as_ref().unwrap().scroll, 3);
        app.handle(Input::Top, T0);
        assert_eq!(app.detail.as_ref().unwrap().scroll, 0);
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib tui::app`
Expected: 컴파일 실패 — `App`, `Msg`, `Effect` 같은 타입이 없다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/tui/app.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
//! 팔레트의 상태와 동작. 화면·네트워크와 떨어진 순수 로직이라 테스트로 고정한다.
//!
//! 런타임은 키를 [`Input`]으로 바꿔 [`App::handle`]에 넘기고, 네트워크·캐시 결과를
//! [`Msg`]로 [`App::apply`]에 넘긴다. 앱은 해야 할 일을 [`Effect`]로 돌려준다.

use crate::linear::client::ApiError;
use crate::linear::types::{Comment, Issue, Viewer};
use crate::markdown::{self, Theme};
use crate::search::query::parse;
use crate::search::rank::{SearchIndex, merge};

/// 서버 검색을 보내기 전에 입력이 멈춰야 하는 시간.
pub const SEARCH_DEBOUNCE_MS: i64 = 300;
/// 하단 안내 문구를 보여주는 시간.
pub const FLASH_MS: i64 = 3_000;
/// 검색 결과로 보여줄 최대 줄 수.
const MAX_RESULTS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Mine,
    Recent,
    All,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Mine, Tab::Recent, Tab::All];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Mine => "내 이슈",
            Tab::Recent => "최근 본",
            Tab::All => "전체",
        }
    }

    fn index(self) -> usize {
        match self {
            Tab::Mine => 0,
            Tab::Recent => 1,
            Tab::All => 2,
        }
    }

    fn next(self) -> Tab {
        Tab::ALL[(self.index() + 1) % 3]
    }

    fn prev(self) -> Tab {
        Tab::ALL[(self.index() + 2) % 3]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// API 키 입력 화면
    Onboarding,
    /// 글자가 검색어로 들어가는 모드 (팔레트 기본)
    Search,
    /// 한 글자 키가 동작인 모드
    List,
    Detail,
}

/// 사용자 동작. 키(목록·상세 모드)와 Ctrl+K 메뉴에서 같은 것을 쓴다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Open,
    Browser,
    CopyId,
    CopyUrl,
    Refresh,
    DeepSearch,
    Links,
    Back,
    Quit,
    OpenUrl(String),
}

/// 키 입력을 해석한 결과 (`tui::keys`가 만든다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Char(char),
    Paste(String),
    Backspace,
    ClearLine,
    Up,
    Down,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Enter,
    Esc,
    NextTab,
    PrevTab,
    Menu,
    Search,
    Act(Act),
    Quit,
}

/// 앱이 런타임에 요청하는 일.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 캐시된 viewer·검색 색인을 보내고 viewer를 새로 받는다
    Init,
    /// 탭 내용: 캐시를 먼저 보내고 서버에서 다시 받는다
    LoadTab(Tab),
    /// "전체" 탭 다음 페이지
    LoadMore,
    Search {
        seq: u64,
        query: String,
    },
    DeepSearch {
        seq: u64,
        query: String,
    },
    /// 상세: 캐시를 먼저 보내고 서버에서 다시 받는다. 최근 본에 기록한다
    OpenDetail(String),
    /// 원래 pane의 브랜치에 연결된 이슈를 찾는다
    ResolvePinned,
    OpenUrl(String),
    /// `what`은 "복사됨: …"에 보일 이름
    Copy {
        text: String,
        what: String,
    },
    ValidateKey(String),
}

/// 런타임이 앱에 알려주는 결과.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Viewer(Viewer),
    Index(Vec<Issue>),
    /// `fresh`가 false면 캐시에서 온 것
    Tab {
        tab: Tab,
        issues: Vec<Issue>,
        fresh: bool,
        has_more: bool,
        append: bool,
    },
    Search {
        seq: u64,
        issues: Vec<Issue>,
    },
    Detail {
        id: String,
        issue: Issue,
        comments: Vec<Comment>,
        more: bool,
        fresh: bool,
    },
    /// 보관·삭제됐거나 없는 이슈. `OpenDetail`에 넘긴 값(id 또는 식별자)이 그대로 온다
    DetailGone(String),
    Pinned(Option<Issue>),
    KeyOk(Viewer),
    KeyBad(String),
    /// 인증 실패. `env`면 `LINEAR_API_KEY` 환경 변수의 키다
    AuthFailed {
        env: bool,
    },
    /// 세어 둔 요청 하나가 실패했다
    Failed(ApiError),
    /// 남은 요청이 적다. 이 시각(리셋)까지 자동 서버 검색을 멈춘다
    Throttled(i64),
    Flash(String),
}

/// 상단에 보이는 문제 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    Offline(String),
    RateLimited(Option<i64>),
    Error(String),
}

/// 목록에서 고를 수 있는 줄.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// 현재 브랜치에 연결된 이슈
    Pinned(Issue),
    Issue(Issue),
    /// "서버에서 검색 (코멘트 포함)"
    DeepSearch,
}

impl Row {
    pub fn issue(&self) -> Option<&Issue> {
        match self {
            Row::Pinned(i) | Row::Issue(i) => Some(i),
            Row::DeepSearch => None,
        }
    }

    fn key(&self) -> String {
        self.issue()
            .map_or_else(|| "deep".to_string(), |i| i.id.clone())
    }
}

/// 상세 화면.
#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    /// 이슈 id 또는 식별자 (시작할 때는 식별자만 알 수 있다)
    pub id: String,
    pub issue: Option<Issue>,
    pub comments: Vec<Comment>,
    pub more_comments: bool,
    pub scroll: u16,
    pub max_scroll: u16,
    pub loading: bool,
    pub gone: bool,
    back: Mode,
}

/// Ctrl+K 메뉴 또는 링크 목록.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub title: &'static str,
    pub items: Vec<(String, Act)>,
    pub filter: String,
    pub selected: usize,
}

impl Menu {
    /// 거르기 글자가 들어간 항목만.
    pub fn visible(&self) -> Vec<&(String, Act)> {
        let f = self.filter.to_lowercase();
        self.items
            .iter()
            .filter(|(label, _)| label.to_lowercase().contains(&f))
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
struct TabData {
    issues: Vec<Issue>,
    loaded: bool,
    has_more: bool,
    loading_more: bool,
}

pub struct App {
    pub mode: Mode,
    pub tab: Tab,
    pub query: String,
    pub rows: Vec<Row>,
    pub selected: usize,
    pub detail: Option<Detail>,
    pub menu: Option<Menu>,
    pub key_input: String,
    pub key_error: Option<String>,
    pub key_checking: bool,
    pub env_key_invalid: bool,
    /// 응답을 기다리는 요청 수
    pub loading: usize,
    pub updated_at: Option<i64>,
    pub problem: Option<Problem>,
    /// (문구, 보여줄 마지막 시각)
    pub flash: Option<(String, i64)>,
    pub quit: bool,
    pub viewer: Option<Viewer>,
    index: SearchIndex,
    tabs: [TabData; 3],
    pinned: Option<Issue>,
    /// 지금 검색어에 대한 서버 결과 (없으면 로컬 결과만)
    results: Option<Vec<Issue>>,
    search_seq: u64,
    search_due: Option<i64>,
    /// 한도 초과로 자동 요청을 멈출 시각
    paused_until: Option<i64>,
}

impl App {
    fn blank(mode: Mode) -> App {
        App {
            mode,
            tab: Tab::Mine,
            query: String::new(),
            rows: Vec::new(),
            selected: 0,
            detail: None,
            menu: None,
            key_input: String::new(),
            key_error: None,
            key_checking: false,
            env_key_invalid: false,
            loading: 0,
            updated_at: None,
            problem: None,
            flash: None,
            quit: false,
            viewer: None,
            index: SearchIndex::new(Vec::new()),
            tabs: Default::default(),
            pinned: None,
            results: None,
            search_seq: 0,
            search_due: None,
            paused_until: None,
        }
    }

    /// 키가 있을 때 시작한다. `open`(식별자)이 있으면 그 이슈 상세로 시작한다.
    pub fn start(open: Option<String>) -> (App, Vec<Effect>) {
        let mut app = App::blank(Mode::Search);
        let mut effects = app.init_effects();
        if let Some(id) = open {
            app.detail = Some(Detail {
                id: id.clone(),
                issue: None,
                comments: Vec::new(),
                more_comments: false,
                scroll: 0,
                max_scroll: 0,
                loading: true,
                gone: false,
                back: Mode::Search,
            });
            app.mode = Mode::Detail;
            app.loading += 1;
            effects.push(Effect::OpenDetail(id));
        }
        (app, effects)
    }

    /// 키 입력 화면으로 시작한다. `env_invalid`면 환경 변수 키가 틀렸다는 안내만 보인다.
    pub fn onboarding(env_invalid: bool) -> App {
        let mut app = App::blank(Mode::Onboarding);
        app.env_key_invalid = env_invalid;
        app
    }

    fn init_effects(&mut self) -> Vec<Effect> {
        self.loading += 1;
        self.tabs[Tab::Mine.index()].loaded = true;
        vec![
            Effect::Init,
            Effect::LoadTab(Tab::Mine),
            Effect::ResolvePinned,
        ]
    }

    pub fn selected_issue(&self) -> Option<&Issue> {
        self.rows.get(self.selected).and_then(Row::issue)
    }

    /// 상세 화면에 보이는 이슈 (없으면 목록에서 고른 이슈).
    fn current_issue(&self) -> Option<&Issue> {
        match (self.mode, &self.detail) {
            (Mode::Detail, Some(d)) => d.issue.as_ref(),
            _ => self.selected_issue(),
        }
    }

    /// 내 팀 키 (식별자 강조용).
    pub fn team_keys(&self) -> Vec<String> {
        self.viewer
            .as_ref()
            .map(|v| v.teams.nodes.iter().map(|t| t.key.clone()).collect())
            .unwrap_or_default()
    }

    pub fn flash_text(&self, now: i64) -> Option<&str> {
        self.flash
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(t, _)| t.as_str())
    }

    /// 그린 뒤 상세 화면의 최대 스크롤을 알려준다.
    pub fn set_detail_max_scroll(&mut self, max: u16) {
        if let Some(d) = self.detail.as_mut() {
            d.max_scroll = max;
            d.scroll = d.scroll.min(max);
        }
    }

    fn set_flash(&mut self, text: impl Into<String>, now: i64) {
        self.flash = Some((text.into(), now + FLASH_MS));
    }

    fn done_loading(&mut self) {
        self.loading = self.loading.saturating_sub(1);
    }

    fn succeeded(&mut self, now: i64) {
        self.problem = None;
        self.updated_at = Some(now);
    }

    /// 목록 줄을 다시 만든다. `keep`이면 고른 이슈를 id로 유지한다.
    fn rebuild(&mut self, keep: bool) {
        let keep_key = keep
            .then(|| self.rows.get(self.selected).map(Row::key))
            .flatten();
        let q = parse(&self.query);
        let mut rows = Vec::new();
        if q.is_empty() {
            if let Some(p) = &self.pinned {
                rows.push(Row::Pinned(p.clone()));
            }
            let pinned_id = self.pinned.as_ref().map(|p| p.id.as_str());
            rows.extend(
                self.tabs[self.tab.index()]
                    .issues
                    .iter()
                    .filter(|i| Some(i.id.as_str()) != pinned_id)
                    .cloned()
                    .map(Row::Issue),
            );
        } else {
            let vid = self.viewer.as_ref().map(|v| v.id.as_str());
            let local = self.index.search(&q, vid);
            let list = match &self.results {
                Some(server) => merge(&local, server, &q, vid),
                None => local,
            };
            rows.extend(list.into_iter().take(MAX_RESULTS).map(Row::Issue));
            if q.has_text() {
                rows.push(Row::DeepSearch);
            }
        }
        self.rows = rows;
        let last = self.rows.len().saturating_sub(1);
        self.selected = keep_key
            .and_then(|k| self.rows.iter().position(|r| r.key() == k))
            .unwrap_or(if keep { self.selected.min(last) } else { 0 });
    }

    fn query_changed(&mut self, now: i64) {
        self.results = None;
        self.rebuild(false);
        self.search_due = (!parse(&self.query).is_empty()).then_some(now + SEARCH_DEBOUNCE_MS);
    }

    /// 키 입력을 처리한다.
    pub fn handle(&mut self, input: Input, now: i64) -> Vec<Effect> {
        if input == Input::Quit {
            self.quit = true;
            return Vec::new();
        }
        if self.menu.is_some() {
            return self.handle_menu(input, now);
        }
        match self.mode {
            Mode::Onboarding => self.handle_onboarding(input),
            Mode::Search => self.handle_search(input, now),
            Mode::List => self.handle_list(input, now),
            Mode::Detail => self.handle_detail(input, now),
        }
    }

    fn handle_onboarding(&mut self, input: Input) -> Vec<Effect> {
        if self.env_key_invalid {
            if matches!(input, Input::Esc | Input::Enter) {
                self.quit = true;
            }
            return Vec::new();
        }
        match input {
            Input::Char(c) if !c.is_whitespace() => self.key_input.push(c),
            Input::Paste(s) => self
                .key_input
                .extend(s.chars().filter(|c| !c.is_whitespace())),
            Input::Backspace => {
                self.key_input.pop();
            }
            Input::ClearLine => self.key_input.clear(),
            Input::Enter if !self.key_input.is_empty() && !self.key_checking => {
                self.key_checking = true;
                self.key_error = None;
                return vec![Effect::ValidateKey(self.key_input.clone())];
            }
            Input::Esc => self.quit = true,
            _ => {}
        }
        Vec::new()
    }

    fn handle_search(&mut self, input: Input, now: i64) -> Vec<Effect> {
        match input {
            Input::Char(c) => {
                self.query.push(c);
                self.query_changed(now);
            }
            Input::Paste(s) => {
                self.query.push_str(&s.replace(['\n', '\r', '\t'], " "));
                self.query_changed(now);
            }
            Input::Backspace => {
                if self.query.pop().is_some() {
                    self.query_changed(now);
                }
            }
            Input::ClearLine => {
                self.query.clear();
                self.query_changed(now);
            }
            Input::Esc => self.mode = Mode::List,
            other => return self.handle_common(other, now),
        }
        Vec::new()
    }

    fn handle_list(&mut self, input: Input, now: i64) -> Vec<Effect> {
        match input {
            Input::Search => {
                self.mode = Mode::Search;
                Vec::new()
            }
            Input::Esc => {
                self.quit = true;
                Vec::new()
            }
            Input::Act(a) => self.act(a, now),
            other => self.handle_common(other, now),
        }
    }

    /// 검색·목록 모드에 공통인 이동·탭·메뉴.
    fn handle_common(&mut self, input: Input, now: i64) -> Vec<Effect> {
        let last = self.rows.len().saturating_sub(1);
        match input {
            Input::Up => self.selected = self.selected.saturating_sub(1),
            Input::Down => self.selected = (self.selected + 1).min(last),
            Input::PageUp => self.selected = self.selected.saturating_sub(10),
            Input::PageDown => self.selected = (self.selected + 10).min(last),
            Input::Top => self.selected = 0,
            Input::Bottom => self.selected = last,
            Input::Enter => return self.act(Act::Open, now),
            Input::NextTab | Input::PrevTab => {
                let tab = if input == Input::NextTab {
                    self.tab.next()
                } else {
                    self.tab.prev()
                };
                return self.switch_tab(tab, now);
            }
            Input::Menu => self.open_menu(),
            _ => {}
        }
        self.maybe_load_more()
    }

    fn handle_detail(&mut self, input: Input, now: i64) -> Vec<Effect> {
        let Some(d) = self.detail.as_mut() else {
            self.mode = Mode::List;
            return Vec::new();
        };
        match input {
            Input::Up => d.scroll = d.scroll.saturating_sub(1),
            Input::Down => d.scroll = d.scroll.saturating_add(1).min(d.max_scroll),
            Input::PageUp => d.scroll = d.scroll.saturating_sub(10),
            Input::PageDown => d.scroll = d.scroll.saturating_add(10).min(d.max_scroll),
            Input::Top => d.scroll = 0,
            Input::Bottom => d.scroll = d.max_scroll,
            Input::Esc => return self.act(Act::Back, now),
            Input::Act(a) => return self.act(a, now),
            Input::Menu => self.open_menu(),
            _ => {}
        }
        Vec::new()
    }

    fn handle_menu(&mut self, input: Input, now: i64) -> Vec<Effect> {
        let Some(menu) = self.menu.as_mut() else {
            return Vec::new();
        };
        let count = menu.visible().len();
        match input {
            Input::Char(c) => {
                menu.filter.push(c);
                menu.selected = 0;
            }
            Input::Backspace => {
                menu.filter.pop();
                menu.selected = 0;
            }
            Input::Up => menu.selected = menu.selected.saturating_sub(1),
            Input::Down => menu.selected = (menu.selected + 1).min(count.saturating_sub(1)),
            Input::Esc | Input::Menu => self.menu = None,
            Input::Enter => {
                let act = menu.visible().get(menu.selected).map(|(_, a)| a.clone());
                self.menu = None;
                if let Some(a) = act {
                    return self.act(a, now);
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn open_menu(&mut self) {
        let mut items: Vec<(String, Act)> = Vec::new();
        let has_issue = self.current_issue().is_some();
        if self.mode != Mode::Detail && has_issue {
            items.push(("상세 보기".into(), Act::Open));
        }
        if has_issue {
            items.push(("브라우저에서 열기".into(), Act::Browser));
            items.push(("ID 복사".into(), Act::CopyId));
            items.push(("URL 복사".into(), Act::CopyUrl));
        }
        if self.mode == Mode::Detail {
            items.push(("링크·이미지 목록".into(), Act::Links));
        }
        if self.mode != Mode::Detail && parse(&self.query).has_text() {
            items.push(("서버에서 깊은 검색 (코멘트 포함)".into(), Act::DeepSearch));
        }
        items.push(("새로고침".into(), Act::Refresh));
        if self.mode == Mode::Detail {
            items.push(("뒤로".into(), Act::Back));
        }
        items.push(("닫기".into(), Act::Quit));
        self.menu = Some(Menu {
            title: "동작",
            items,
            filter: String::new(),
            selected: 0,
        });
    }

    /// 상세 화면 본문과 코멘트의 링크·이미지 목록.
    fn open_links(&mut self) {
        let Some(d) = self.detail.as_ref() else {
            return;
        };
        let mut items = Vec::new();
        let theme = Theme::default();
        let mut add = |md: &str| {
            for l in markdown::render(md, 80, &theme).links {
                let n = items.len() + 1;
                items.push((
                    format!("[{n}] {} — {}", l.label, l.url),
                    Act::OpenUrl(l.url),
                ));
            }
        };
        if let Some(desc) = d.issue.as_ref().and_then(|i| i.description.as_deref()) {
            add(desc);
        }
        for c in &d.comments {
            add(&c.body);
        }
        self.menu = Some(Menu {
            title: if items.is_empty() {
                "링크 없음"
            } else {
                "링크·이미지"
            },
            items,
            filter: String::new(),
            selected: 0,
        });
    }

    fn switch_tab(&mut self, tab: Tab, now: i64) -> Vec<Effect> {
        self.tab = tab;
        self.mode = Mode::List;
        if !self.query.is_empty() {
            self.query.clear();
            self.query_changed(now);
        }
        self.rebuild(false);
        let data = &mut self.tabs[tab.index()];
        // 최근 본은 로컬 기록이라 열 때마다 다시 읽는다
        if data.loaded && tab != Tab::Recent {
            return Vec::new();
        }
        data.loaded = true;
        self.loading += 1;
        vec![Effect::LoadTab(tab)]
    }

    /// "전체" 탭 끝에 닿으면 다음 페이지를 부른다.
    fn maybe_load_more(&mut self) -> Vec<Effect> {
        let at_end = self.selected + 1 >= self.rows.len();
        let data = &mut self.tabs[Tab::All.index()];
        if self.tab == Tab::All
            && self.query.is_empty()
            && at_end
            && data.has_more
            && !data.loading_more
        {
            data.loading_more = true;
            self.loading += 1;
            return vec![Effect::LoadMore];
        }
        Vec::new()
    }

    fn act(&mut self, act: Act, now: i64) -> Vec<Effect> {
        match act {
            Act::Open => match self.rows.get(self.selected).cloned() {
                Some(Row::DeepSearch) => self.act(Act::DeepSearch, now),
                Some(row) => {
                    let Some(issue) = row.issue().cloned() else {
                        return Vec::new();
                    };
                    let back = self.mode;
                    self.detail = Some(Detail {
                        id: issue.id.clone(),
                        issue: Some(issue.clone()),
                        comments: Vec::new(),
                        more_comments: false,
                        scroll: 0,
                        max_scroll: 0,
                        loading: true,
                        gone: false,
                        back,
                    });
                    self.mode = Mode::Detail;
                    self.loading += 1;
                    vec![Effect::OpenDetail(issue.id)]
                }
                None => Vec::new(),
            },
            Act::Browser => self
                .current_issue()
                .map(|i| vec![Effect::OpenUrl(i.url.clone())])
                .unwrap_or_default(),
            Act::CopyId => self
                .current_issue()
                .map(|i| {
                    vec![Effect::Copy {
                        text: i.identifier.clone(),
                        what: i.identifier.clone(),
                    }]
                })
                .unwrap_or_default(),
            Act::CopyUrl => self
                .current_issue()
                .map(|i| {
                    vec![Effect::Copy {
                        text: i.url.clone(),
                        what: format!("{} URL", i.identifier),
                    }]
                })
                .unwrap_or_default(),
            Act::Refresh => {
                if self.mode == Mode::Detail {
                    if let Some(d) = self.detail.as_mut() {
                        d.loading = true;
                        self.loading += 1;
                        return vec![Effect::OpenDetail(d.id.clone())];
                    }
                    return Vec::new();
                }
                if !parse(&self.query).is_empty() {
                    self.search_due = Some(now);
                    self.paused_until = None;
                    return Vec::new();
                }
                self.loading += 1;
                vec![Effect::LoadTab(self.tab), Effect::ResolvePinned]
            }
            Act::DeepSearch => {
                if !parse(&self.query).has_text() {
                    return Vec::new();
                }
                self.search_seq += 1;
                self.search_due = None;
                self.loading += 1;
                vec![Effect::DeepSearch {
                    seq: self.search_seq,
                    query: self.query.clone(),
                }]
            }
            Act::Links => {
                self.open_links();
                Vec::new()
            }
            Act::Back => {
                let back = self.detail.take().map_or(Mode::List, |d| d.back);
                self.mode = back;
                Vec::new()
            }
            Act::Quit => {
                self.quit = true;
                Vec::new()
            }
            Act::OpenUrl(url) => vec![Effect::OpenUrl(url)],
        }
    }

    /// 시간이 지나 할 일 (검색 디바운스).
    pub fn tick(&mut self, now: i64) -> Vec<Effect> {
        let Some(due) = self.search_due else {
            return Vec::new();
        };
        if now < due {
            return Vec::new();
        }
        if let Some(until) = self.paused_until {
            if now < until {
                return Vec::new();
            }
            self.paused_until = None;
        }
        self.search_due = None;
        self.search_seq += 1;
        self.loading += 1;
        vec![Effect::Search {
            seq: self.search_seq,
            query: self.query.clone(),
        }]
    }

    /// 런타임이 보낸 결과를 반영한다.
    pub fn apply(&mut self, msg: Msg, now: i64) -> Vec<Effect> {
        match msg {
            Msg::Viewer(v) => self.viewer = Some(v),
            Msg::Index(issues) => {
                self.index = SearchIndex::new(issues);
                if !self.query.is_empty() {
                    self.rebuild(true);
                }
            }
            Msg::Tab {
                tab,
                issues,
                fresh,
                has_more,
                append,
            } => {
                self.index.upsert(&issues);
                let data = &mut self.tabs[tab.index()];
                if append {
                    data.issues.extend(issues);
                    data.loading_more = false;
                } else {
                    data.issues = issues;
                }
                data.has_more = has_more;
                if fresh {
                    self.done_loading();
                    // 최근 본은 로컬 기록이라 서버 상태(갱신 시각·오프라인)를 바꾸지 않는다
                    if tab != Tab::Recent {
                        self.succeeded(now);
                    }
                }
                if tab == self.tab {
                    self.rebuild(true);
                }
            }
            Msg::Search { seq, issues } => {
                self.done_loading();
                if seq != self.search_seq {
                    return Vec::new();
                }
                self.succeeded(now);
                self.index.upsert(&issues);
                self.results = Some(issues);
                self.rebuild(true);
            }
            Msg::Detail {
                id,
                issue,
                comments,
                more,
                fresh,
            } => {
                if fresh {
                    self.done_loading();
                    self.succeeded(now);
                    self.index.upsert(std::slice::from_ref(&issue));
                }
                if let Some(d) = self.detail.as_mut()
                    && (d.id == id
                        || d.id == issue.id
                        || d.id.eq_ignore_ascii_case(&issue.identifier))
                {
                    d.id = issue.id.clone();
                    d.issue = Some(issue);
                    d.comments = comments;
                    d.more_comments = more;
                    if fresh {
                        d.loading = false;
                    }
                }
            }
            Msg::DetailGone(id) => {
                self.done_loading();
                let same = |i: &Issue| i.id == id || i.identifier.eq_ignore_ascii_case(&id);
                if let Some(d) = self.detail.as_mut()
                    && (d.id.eq_ignore_ascii_case(&id) || d.issue.as_ref().is_some_and(same))
                {
                    d.gone = true;
                    d.loading = false;
                }
                for data in &mut self.tabs {
                    data.issues.retain(|i| !same(i));
                }
                if let Some(results) = self.results.as_mut() {
                    results.retain(|i| !same(i));
                }
                if self.pinned.as_ref().is_some_and(same) {
                    self.pinned = None;
                }
                self.rebuild(true);
            }
            Msg::Pinned(p) => {
                self.pinned = p;
                self.rebuild(true);
            }
            Msg::KeyOk(v) => {
                let greeting = format!(
                    "{}님, {} 워크스페이스에 연결됐어요",
                    v.name, v.organization.name
                );
                self.viewer = Some(v);
                self.key_checking = false;
                self.key_input.clear();
                self.mode = Mode::Search;
                self.set_flash(greeting, now);
                return self.init_effects();
            }
            Msg::KeyBad(text) => {
                self.key_checking = false;
                self.key_error = Some(text);
            }
            Msg::AuthFailed { env } => {
                self.mode = Mode::Onboarding;
                self.menu = None;
                self.detail = None;
                self.loading = 0;
                self.env_key_invalid = env;
                self.key_checking = false;
                self.key_input.clear();
                self.key_error = (!env)
                    .then(|| "API 키가 만료됐거나 권한이 없어요. 새 키를 붙여넣으세요".into());
            }
            Msg::Failed(e) => {
                self.done_loading();
                if let Some(d) = self.detail.as_mut() {
                    d.loading = false;
                }
                self.tabs[Tab::All.index()].loading_more = false;
                self.problem = Some(match e {
                    ApiError::Offline(m) => Problem::Offline(m),
                    ApiError::RateLimited { reset_at_ms } => {
                        self.paused_until = reset_at_ms;
                        Problem::RateLimited(reset_at_ms)
                    }
                    other => Problem::Error(other.to_string()),
                });
            }
            Msg::Throttled(until) => {
                self.paused_until = Some(until);
                let mins = ((until - now).max(0) + 59_999) / 60_000;
                self.set_flash(
                    format!("API 한도가 얼마 남지 않아 {mins}분 동안 자동 서버 검색을 멈춰요"),
                    now,
                );
            }
            Msg::Flash(text) => self.set_flash(text, now),
        }
        Vec::new()
    }
}

````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib tui::app`
Expected: `test result: ok. 24 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 173개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/tui/mod.rs src/tui/app.rs
git commit -m "feat(tui): 팔레트 상태 머신" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 6: 키 해석 (tui::keys)

crossterm 키 이벤트를 모드에 맞는 `Input`으로 바꾼다.

- Ctrl+C 종료, Ctrl+K 메뉴, Ctrl+P/N 위아래, 상세에서 Ctrl+D/U 반 페이지, 그 밖의 Ctrl+U는 검색어 지우기다. Ctrl 조합은 IME를 거치지 않아 한글 입력 중에도 동작한다.
- Tab/Shift+Tab은 탭을 바꾼다.
- 검색 모드, 키 입력, 메뉴에서는 글자가 그대로 들어간다(한글 포함).
- 목록·상세 모드의 한 글자 키는 `j k g G o y Y r q`, 목록의 `/`, 상세의 `u`다. 한글 자모로 들어오면 두벌식 자리의 영문 키로 바꾼다(ㅓ→j, ㅏ→k, ㅐ→o, ㅛ→y, ㄱ→r, ㅂ→q, ㅎ→g, ㅕ→u). 대문자 `G`, `Y`는 영문 입력에서만 동작한다.
- 키를 뗄 때 오는 이벤트(Release)는 무시한다.

**Files:**
- Create: `src/tui/keys.rs`
- Modify: `src/tui/mod.rs`
- Test: `src/tui/keys.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `tui::app::{Mode, Input, Act}`
- Produces:
  - `tui::keys::jamo_to_latin(c: char) -> char`
  - `tui::keys::translate(mode: Mode, menu_open: bool, key: KeyEvent) -> Option<Input>`

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/tui/mod.rs`에서 아래 부분을 찾아:

````rust
//! herdr 팝업 팔레트 TUI.

pub mod app;
````

아래로 바꾼다:

````rust
//! herdr 팝업 팔레트 TUI.

pub mod app;
pub mod keys;
````

`src/tui/keys.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn ctrl_keys_work_in_every_mode() {
        for mode in [Mode::Search, Mode::List, Mode::Detail, Mode::Onboarding] {
            assert_eq!(translate(mode, false, ctrl('c')), Some(Input::Quit));
            assert_eq!(translate(mode, false, ctrl('k')), Some(Input::Menu));
            assert_eq!(translate(mode, false, ctrl('n')), Some(Input::Down));
        }
        assert_eq!(
            translate(Mode::Search, false, ctrl('u')),
            Some(Input::ClearLine)
        );
        assert_eq!(
            translate(Mode::Detail, false, ctrl('u')),
            Some(Input::PageUp)
        );
        assert_eq!(
            translate(Mode::Detail, false, ctrl('d')),
            Some(Input::PageDown)
        );
    }

    #[test]
    fn search_mode_types_letters_including_korean() {
        assert_eq!(
            translate(Mode::Search, false, key(KeyCode::Char('j'))),
            Some(Input::Char('j'))
        );
        assert_eq!(
            translate(Mode::Search, false, key(KeyCode::Char('로'))),
            Some(Input::Char('로'))
        );
        assert_eq!(
            translate(Mode::Search, false, key(KeyCode::Tab)),
            Some(Input::NextTab)
        );
    }

    #[test]
    fn list_mode_letters_are_actions_and_jamo_maps() {
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('j'))),
            Some(Input::Down)
        );
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('ㅓ'))),
            Some(Input::Down)
        );
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('ㅛ'))),
            Some(Input::Act(Act::CopyId))
        );
        assert_eq!(
            translate(
                Mode::List,
                false,
                KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT)
            ),
            Some(Input::Act(Act::CopyUrl))
        );
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('/'))),
            Some(Input::Search)
        );
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('ㅂ'))),
            Some(Input::Esc)
        );
        assert_eq!(translate(Mode::List, false, key(KeyCode::Char('u'))), None);
    }

    #[test]
    fn detail_mode_has_links_key() {
        assert_eq!(
            translate(Mode::Detail, false, key(KeyCode::Char('u'))),
            Some(Input::Act(Act::Links))
        );
        assert_eq!(
            translate(Mode::Detail, false, key(KeyCode::Char('/'))),
            None
        );
    }

    #[test]
    fn menu_takes_letters_as_filter() {
        assert_eq!(
            translate(Mode::List, true, key(KeyCode::Char('j'))),
            Some(Input::Char('j'))
        );
        assert_eq!(translate(Mode::List, true, key(KeyCode::Tab)), None);
    }

    #[test]
    fn key_release_is_ignored() {
        let mut ev = key(KeyCode::Char('j'));
        ev.kind = KeyEventKind::Release;
        assert_eq!(translate(Mode::List, false, ev), None);
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib tui::keys`
Expected: 컴파일 실패 — `translate`, `jamo_to_latin`이 없다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/tui/keys.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
//! 키 입력 → [`Input`]. 한글 입력 중에도 단축키가 먹도록 두벌식 자모를 영문 키로 바꾼다.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::app::{Act, Input, Mode};

/// 두벌식 자모 → 같은 자리의 영문 키. 자모가 아니면 그대로.
pub fn jamo_to_latin(c: char) -> char {
    match c {
        'ㅂ' => 'q',
        'ㅈ' => 'w',
        'ㄷ' => 'e',
        'ㄱ' => 'r',
        'ㅅ' => 't',
        'ㅛ' => 'y',
        'ㅕ' => 'u',
        'ㅑ' => 'i',
        'ㅐ' => 'o',
        'ㅔ' => 'p',
        'ㅁ' => 'a',
        'ㄴ' => 's',
        'ㅇ' => 'd',
        'ㄹ' => 'f',
        'ㅎ' => 'g',
        'ㅗ' => 'h',
        'ㅓ' => 'j',
        'ㅏ' => 'k',
        'ㅣ' => 'l',
        'ㅋ' => 'z',
        'ㅌ' => 'x',
        'ㅊ' => 'c',
        'ㅍ' => 'v',
        'ㅠ' => 'b',
        'ㅜ' => 'n',
        'ㅡ' => 'm',
        'ㅃ' => 'Q',
        'ㅉ' => 'W',
        'ㄸ' => 'E',
        'ㄲ' => 'R',
        'ㅆ' => 'T',
        'ㅒ' => 'O',
        'ㅖ' => 'P',
        other => other,
    }
}

/// 키 하나를 해석한다. `menu_open`이면 메뉴 입력으로 본다. 쓰지 않는 키면 `None`.
pub fn translate(mode: Mode, menu_open: bool, key: KeyEvent) -> Option<Input> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        let detail = mode == Mode::Detail && !menu_open;
        return match key.code {
            KeyCode::Char('c') => Some(Input::Quit),
            KeyCode::Char('k') => Some(Input::Menu),
            KeyCode::Char('p') => Some(Input::Up),
            KeyCode::Char('n') => Some(Input::Down),
            KeyCode::Char('d') if detail => Some(Input::PageDown),
            KeyCode::Char('u') if detail => Some(Input::PageUp),
            KeyCode::Char('u') => Some(Input::ClearLine),
            _ => None,
        };
    }
    match key.code {
        KeyCode::Up => return Some(Input::Up),
        KeyCode::Down => return Some(Input::Down),
        KeyCode::PageUp => return Some(Input::PageUp),
        KeyCode::PageDown => return Some(Input::PageDown),
        KeyCode::Home => return Some(Input::Top),
        KeyCode::End => return Some(Input::Bottom),
        KeyCode::Enter => return Some(Input::Enter),
        KeyCode::Esc => return Some(Input::Esc),
        KeyCode::Backspace => return Some(Input::Backspace),
        KeyCode::Tab if !menu_open => return Some(Input::NextTab),
        KeyCode::BackTab if !menu_open => return Some(Input::PrevTab),
        _ => {}
    }
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    // 글자가 그대로 입력되는 곳: 검색창, 키 입력, 메뉴 거르기
    if menu_open || matches!(mode, Mode::Search | Mode::Onboarding) {
        return Some(Input::Char(c));
    }
    // 목록·상세: 한 글자 동작
    let act = |a| Some(Input::Act(a));
    match (mode, jamo_to_latin(c)) {
        (_, 'j') => Some(Input::Down),
        (_, 'k') => Some(Input::Up),
        (_, 'g') => Some(Input::Top),
        (_, 'G') => Some(Input::Bottom),
        (_, 'o') => act(Act::Browser),
        (_, 'y') => act(Act::CopyId),
        (_, 'Y') => act(Act::CopyUrl),
        (_, 'r') => act(Act::Refresh),
        (_, 'q') => Some(Input::Esc),
        (Mode::List, '/') => Some(Input::Search),
        (Mode::Detail, 'u') => act(Act::Links),
        _ => None,
    }
}

````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib tui::keys`
Expected: `test result: ok. 6 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 179개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/tui/mod.rs src/tui/keys.rs
git commit -m "feat(tui): 키 해석과 두벌식 자모 매핑" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 7: 팔레트 화면 (tui::view)

`App`을 ratatui로 그린다.

- 맨 위 줄은 탭과 상태다. 상태는 "갱신 중…", "n분 전 갱신", "오프라인", "한도 초과 · n분 후 재시도", "오류" 중 하나다.
- 둘째 줄은 검색 줄이다. 검색 모드면 실제 터미널 커서를 검색어 끝에 둔다. 한글 IME 조합 글자가 그 자리에 보이게 하려는 것이다.
- 본문은 목록(고른 줄 ▶와 배경색, "현재 브랜치" 칸과 구분선)이다. 폭이 100칸 이상이면 오른쪽에 미리보기(머리 정보 + markdown)를 둔다.
- 상세는 머리 정보, 본문, 코멘트(작성자·시각·본문)를 스크롤로 보여 준다. 그린 뒤 최대 스크롤 값을 돌려준다(`Drawn`). 런타임이 이 값으로 스크롤을 제한한다.
- 하단은 안내 문구(3초), 문제, 키 도움말 순으로 하나를 보여 준다.
- 메뉴는 가운데 겹쳐 그린다. 키 입력 화면은 키를 가린다(앞 8자 + •).
- 서버·이슈 문자열은 모두 제어 문자를 지우고 그린다. 아주 작은 창(1×1)에서도 패닉 없이 그린다.
- `ago`, `local_time`을 `ui::style`로 옮기고(CLI는 다시 내보낸다), 긴 문장 줄바꿈용 `markdown::wrap_text`를 더한다.

**Files:**
- Create: `src/tui/view.rs`
- Modify: `src/tui/mod.rs`, `src/cli.rs`, `src/markdown.rs`, `src/ui/style.rs`
- Test: `src/tui/view.rs`, `src/cli.rs`, `src/markdown.rs`, `src/ui/style.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `tui::app::*`(Task 5), `ui::style`, `ui::row::issue_row`(Task 4), `markdown::{render_with, sanitize}`(Task 3)
- Produces:
  - `tui::view::PREVIEW_MIN_WIDTH: u16 = 100`
  - `tui::view::Drawn { pub detail_max_scroll: Option<u16> }` (`Default`)
  - `tui::view::draw(f: &mut Frame, app: &App, now: i64) -> Drawn`
  - `tui::view::status_span(app: &App, now: i64) -> Span<'static>`, `issue_header(issue: &Issue, width: u16) -> Vec<Line<'static>>`
  - `ui::style::{ago(now_ms: i64, then_ms: i64) -> String, local_time(rfc3339: &str) -> String}` (cli에서 옮김)
  - `markdown::wrap_text(text: &str, width: u16, style: Style) -> Vec<Line<'static>>`

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/tui/mod.rs`에서 아래 부분을 찾아:

````rust

pub mod app;
pub mod keys;
````

아래로 바꾼다:

````rust

pub mod app;
pub mod keys;
pub mod view;
````

`src/tui/view.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::types::Viewer;
    use crate::test_support::IssueBuilder;
    use crate::tui::app::{Act, Input, Msg};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const T0: i64 = 1_000_000;

    fn viewer() -> Viewer {
        serde_json::from_value(serde_json::json!({
            "id": "me", "name": "김민수", "displayName": "minsu", "email": "m@acme.dev",
            "organization": { "id": "org1", "name": "Acme", "urlKey": "acme" },
            "teams": { "nodes": [ { "id": "team-ENG", "key": "ENG", "name": "Eng" } ] }
        }))
        .unwrap()
    }

    fn app() -> App {
        let (mut app, _) = App::start(None);
        app.apply(Msg::Viewer(viewer()), T0);
        app.apply(
            Msg::Tab {
                tab: Tab::Mine,
                issues: vec![
                    IssueBuilder::new("a", "ENG-1", "로그인 버그")
                        .state("In Progress", "started")
                        .labels(&["bug"])
                        .description("## 재현\n- 로그인 후 대기")
                        .build(),
                    IssueBuilder::new("b", "ENG-2", "결제 화면").build(),
                ],
                fresh: true,
                has_more: false,
                append: false,
            },
            T0,
        );
        app
    }

    /// 화면 각 줄의 글자 (넓은 글자 뒤 칸은 건너뛴다).
    fn screen(app: &App, w: u16, h: u16) -> (Vec<String>, Drawn, Terminal<TestBackend>) {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut drawn = Drawn::default();
        term.draw(|f| drawn = draw(f, app, T0)).unwrap();
        let buf = term.backend().buffer().clone();
        let rows = (0..h)
            .map(|y| {
                let mut s = String::new();
                let mut x = 0;
                while x < w {
                    let sym = buf[(x, y)].symbol();
                    s.push_str(sym);
                    x += if sym.width() == 2 { 2 } else { 1 };
                }
                s.trim_end().to_string()
            })
            .collect();
        (rows, drawn, term)
    }

    #[test]
    fn tiny_screens_do_not_panic() {
        let mut detail = app();
        detail.handle(Input::Enter, T0);
        let mut menu = app();
        menu.handle(Input::Menu, T0);
        let onboarding = App::onboarding(false);
        for app in [&app(), &detail, &menu, &onboarding] {
            for (w, h) in [(1, 1), (5, 3), (12, 4), (20, 5), (30, 8)] {
                screen(app, w, h);
            }
        }
    }

    #[test]
    fn palette_shows_tabs_status_rows_and_preview() {
        let (rows, _, _) = screen(&app(), 120, 16);
        assert!(
            rows[0].contains("[내 이슈]") && rows[0].contains("최근 본"),
            "{}",
            rows[0]
        );
        assert!(rows[0].ends_with("방금 갱신"), "{}", rows[0]);
        assert!(
            rows[2].contains("▶ ◐ ENG-1") && rows[2].contains("로그인 버그"),
            "{}",
            rows[2]
        );
        let all = rows.join("\n");
        assert!(all.contains("우선순위 없음"), "미리보기 머리\n{all}");
        assert!(all.contains("• 로그인 후 대기"), "미리보기 본문\n{all}");
    }

    #[test]
    fn narrow_palette_hides_preview() {
        let (rows, _, _) = screen(&app(), 80, 12);
        assert!(!rows.join("\n").contains("우선순위"));
    }

    #[test]
    fn search_mode_puts_cursor_after_query() {
        let mut a = app();
        a.handle(Input::Char('로'), T0);
        let (rows, _, mut term) = screen(&a, 80, 12);
        assert!(rows[1].starts_with(" > 로"), "{}", rows[1]);
        term.backend_mut().assert_cursor_position((5, 1));
    }

    #[test]
    fn pinned_section_has_label() {
        let mut a = app();
        a.apply(
            Msg::Pinned(Some(IssueBuilder::new("b", "ENG-2", "결제 화면").build())),
            T0,
        );
        let (rows, _, _) = screen(&a, 80, 12);
        assert_eq!(rows[2], " 현재 브랜치");
        assert!(rows[3].contains("ENG-2"), "{}", rows[3]);
        assert!(rows[4].contains('─'));
    }

    #[test]
    fn detail_shows_body_comments_and_max_scroll() {
        let mut a = app();
        a.handle(Input::Enter, T0);
        let long = (1..=40)
            // 줄바꿈 하나는 같은 문단이라 빈 줄로 문단을 나눈다
            .map(|i| format!("{i}번째 문단\n\n"))
            .collect::<String>();
        a.apply(
            Msg::Detail {
                id: "a".into(),
                issue: IssueBuilder::new("a", "ENG-1", "로그인 버그")
                    .description(&long)
                    .build(),
                comments: Vec::new(),
                more: false,
                fresh: true,
            },
            T0,
        );
        let (rows, drawn, _) = screen(&a, 80, 12);
        assert!(
            rows[1].contains("ENG-1") && rows[1].contains("로그인 버그"),
            "{rows:?}"
        );
        assert!(drawn.detail_max_scroll.unwrap() > 0);
    }

    #[test]
    fn menu_overlay_lists_actions() {
        let mut a = app();
        a.handle(Input::Menu, T0);
        let (rows, _, _) = screen(&a, 80, 16);
        let all = rows.join("\n");
        assert!(
            all.contains("동작") && all.contains("브라우저에서 열기"),
            "{all}"
        );
    }

    #[test]
    fn onboarding_masks_key() {
        let mut a = App::onboarding(false);
        a.handle(Input::Paste("lin_api_secret".into()), T0);
        let (rows, _, _) = screen(&a, 80, 14);
        let all = rows.join("\n");
        assert!(!all.contains("secret"), "{all}");
        assert!(all.contains("••••••••••••••"), "{all}");
    }

    #[test]
    fn status_shows_offline_and_rate_limit() {
        let mut a = app();
        a.apply(
            Msg::Failed(crate::linear::client::ApiError::Offline("x".into())),
            T0,
        );
        assert_eq!(status_span(&a, T0).content, "오프라인");
        a.apply(
            Msg::Failed(crate::linear::client::ApiError::RateLimited {
                reset_at_ms: Some(T0 + 120_000),
            }),
            T0,
        );
        assert_eq!(status_span(&a, T0).content, "한도 초과 · 2분 후 재시도");
    }

    #[test]
    fn server_text_cannot_inject_terminal_codes() {
        let mut a = app();
        a.apply(
            Msg::Failed(crate::linear::client::ApiError::GraphQl(
                "나쁜\u{1b}]52;c;eA==\u{7}응답".into(),
            )),
            T0,
        );
        let (rows, _, _) = screen(&a, 100, 12);
        assert!(
            rows.iter()
                .all(|r| !r.contains('\u{1b}') && !r.contains('\u{7}'))
        );
        assert!(rows.last().unwrap().contains("오류: 나쁜"), "{rows:?}");
    }

    #[test]
    fn flash_replaces_hints() {
        let mut a = app();
        a.apply(Msg::Flash("복사됨: ENG-1".into()), T0);
        let (rows, _, _) = screen(&a, 80, 12);
        assert_eq!(rows[11], " 복사됨: ENG-1");
        a.handle(Input::Act(Act::Back), T0);
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib -- tui::view cli::`
Expected: 컴파일 실패 — `draw`, `Drawn`, `status_span`, `wrap_text` 같은 이름이 없다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/cli.rs`에서 아래 부분을 찾아:

````rust
use crate::search::rank::{SearchIndex, merge, sort_mine};
use crate::store::{Store, remove_db_files};
use crate::ui::row::issue_row;
pub use crate::ui::style::{priority_label, state_icon};

const VIEWER_TTL_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
````

아래로 바꾼다:

````rust
use crate::search::rank::{SearchIndex, merge, sort_mine};
use crate::store::{Store, remove_db_files};
use crate::ui::row::issue_row;
use crate::ui::style::local_time;
pub use crate::ui::style::{ago, priority_label, state_icon};

const VIEWER_TTL_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
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
````

아래로 바꾼다:

````rust
    }
}

/// API 오류를 사용자 문구로 바꾼다.
/// 한도 초과면 언제 다시 시도할지, 환경 변수 키가 틀렸으면 그 사실을 알려준다.
fn api_error(ctx: &Ctx, e: ApiError) -> anyhow::Error {
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
            out.push(String::new());
            out.push(markdown::sanitize(&format!(
                "{who} · {}",
                short_time(&c.created_at)
            )));
            out.push(render_md(ctx, &c.body, &theme, team_keys));
        }
````

아래로 바꾼다:

````rust
            out.push(String::new());
            out.push(markdown::sanitize(&format!(
                "{who} · {}",
                local_time(&c.created_at)
            )));
            out.push(render_md(ctx, &c.body, &theme, team_keys));
        }
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
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
````

아래로 바꾼다:

````rust
        }
    }
    text
}

#[cfg(test)]
````

`src/markdown.rs`에서 아래 부분을 찾아:

````rust
        r.event(ev);
    }
    r.finish()
}

/// 스타일 없이 텍스트만 (테스트·파이프 출력용).
````

아래로 바꾼다:

````rust
        r.event(ev);
    }
    r.finish()
}

/// markdown이 아닌 평문을 폭에 맞춰 줄바꿈한다 (제목처럼 긴 한 줄용).
pub fn wrap_text(text: &str, width: u16, style: Style) -> Vec<Line<'static>> {
    let text = sanitize(&text.nfc().collect::<String>());
    wrap(
        &[(text, style)],
        &[],
        &[],
        usize::from(width).max(10),
        false,
    )
}

/// 스타일 없이 텍스트만 (테스트·파이프 출력용).
````

`src/tui/view.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
//! 팔레트 그리기.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::app::{App, Mode, Problem, Row, Tab};
use crate::linear::types::Issue;
use crate::markdown::{self, Theme, sanitize};
use crate::ui::row::issue_row;
use crate::ui::style::{
    ACCENT, DIM, ago, label_style, local_time, priority_label, state_icon, state_style, truncate,
};

/// 이 폭 이상이면 목록 옆에 미리보기를 붙인다.
pub const PREVIEW_MIN_WIDTH: u16 = 100;

const SELECTED_BG: Style = Style::new().bg(Color::Rgb(45, 45, 60));
const WARN: Style = Style::new().fg(Color::Yellow);
const ERROR: Style = Style::new().fg(Color::Red);

/// 그린 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Drawn {
    /// 상세 화면이면 최대 스크롤
    pub detail_max_scroll: Option<u16>,
}

/// 화면 전체를 그린다.
pub fn draw(f: &mut Frame, app: &App, now: i64) -> Drawn {
    let area = f.area();
    if app.mode == Mode::Onboarding {
        draw_onboarding(f, app, area);
        return Drawn::default();
    }
    let [header, search, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(f, app, header, now);
    let mut drawn = Drawn::default();
    if app.mode == Mode::Detail {
        drawn.detail_max_scroll = Some(draw_detail(f, app, search.union(body)));
    } else {
        draw_search(f, app, search);
        if body.width >= PREVIEW_MIN_WIDTH {
            let [list, preview] =
                Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .areas(body);
            draw_list(f, app, list);
            draw_preview(f, app, preview);
        } else {
            draw_list(f, app, body);
        }
    }
    draw_footer(f, app, footer, now);
    if app.menu.is_some() {
        draw_menu(f, app, area);
    }
    drawn
}

fn draw_header(f: &mut Frame, app: &App, area: Rect, now: i64) {
    let mut spans = vec![Span::styled(" Linear ", ACCENT)];
    for tab in Tab::ALL {
        spans.push(Span::raw(" "));
        if tab == app.tab {
            spans.push(Span::styled(format!("[{}]", tab.title()), ACCENT));
        } else {
            spans.push(Span::styled(tab.title(), DIM));
        }
    }
    let status = status_span(app, now);
    let used = Line::from(spans.clone()).width() + status.width() + 1;
    spans.push(Span::raw(
        " ".repeat(usize::from(area.width).saturating_sub(used)),
    ));
    spans.push(status);
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// 상단 오른쪽 상태: 갱신 중 / n분 전 갱신 / 오프라인 / 한도 초과.
pub fn status_span(app: &App, now: i64) -> Span<'static> {
    if app.loading > 0 {
        return Span::styled("갱신 중…", DIM);
    }
    match &app.problem {
        Some(Problem::Offline(_)) => Span::styled("오프라인", WARN),
        Some(Problem::RateLimited(reset)) => {
            let text = match reset {
                Some(r) => format!(
                    "한도 초과 · {}분 후 재시도",
                    ((r - now).max(0) + 59_999) / 60_000
                ),
                None => "한도 초과".to_string(),
            };
            Span::styled(text, ERROR)
        }
        Some(Problem::Error(_)) => Span::styled("오류", ERROR),
        None => match app.updated_at {
            Some(at) => Span::styled(format!("{} 갱신", ago(now, at)), DIM),
            None => Span::raw(""),
        },
    }
}

fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let query = sanitize(&app.query);
    let line = if app.mode == Mode::Search {
        let mut spans = vec![Span::styled(" > ", ACCENT), Span::raw(query.clone())];
        if query.is_empty() {
            spans.push(Span::styled(
                " ID·제목·본문 검색 · l:라벨 s:상태 @담당자 #팀",
                DIM,
            ));
        }
        // 한글 IME가 조합 중인 글자를 제자리에 보이도록 실제 커서를 검색어 끝에 둔다
        let x = area.x + 3 + query.width() as u16;
        f.set_cursor_position(Position::new(x.min(area.right().saturating_sub(1)), area.y));
        Line::from(spans)
    } else if query.is_empty() {
        Line::from(Span::styled(" / 검색", DIM))
    } else {
        Line::from(vec![Span::styled(" / ", DIM), Span::raw(query)])
    };
    f.render_widget(Paragraph::new(line), area);
}

fn draw_list(f: &mut Frame, app: &App, area: Rect) {
    if app.rows.is_empty() {
        let text = if app.loading > 0 {
            "불러오는 중…"
        } else {
            "결과가 없어요"
        };
        f.render_widget(Paragraph::new(Span::styled(format!("  {text}"), DIM)), area);
        return;
    }
    let width = area.width.saturating_sub(2);
    let mut items = Vec::new();
    let mut selected_item = 0;
    for (i, row) in app.rows.iter().enumerate() {
        if matches!(row, Row::Pinned(_)) {
            items.push(ListItem::new(Span::styled(" 현재 브랜치", DIM)));
        }
        if i > 0 && matches!(app.rows[i - 1], Row::Pinned(_)) {
            items.push(ListItem::new(Span::styled(
                format!(" {}", "─".repeat(usize::from(width).min(30))),
                DIM,
            )));
        }
        if i == app.selected {
            selected_item = items.len();
        }
        let line = match row {
            Row::Pinned(issue) | Row::Issue(issue) => issue_row(issue, width),
            Row::DeepSearch => Line::from(Span::styled("⏎ 서버에서 검색 (코멘트 포함)", ACCENT)),
        };
        let marker = if i == app.selected { "▶ " } else { "  " };
        let mut spans = vec![Span::styled(marker, ACCENT)];
        spans.extend(line.spans);
        items.push(ListItem::new(Line::from(spans)));
    }
    let mut state = ListState::default().with_selected(Some(selected_item));
    f.render_stateful_widget(
        List::new(items).highlight_style(SELECTED_BG),
        area,
        &mut state,
    );
}

/// 이슈 머리: 식별자·제목 / 상태·우선순위·담당자 / 라벨·프로젝트·사이클·상위.
pub fn issue_header(issue: &Issue, width: u16) -> Vec<Line<'static>> {
    let mut lines = markdown::wrap_text(
        &format!("{}  {}", issue.identifier, issue.title),
        width,
        Style::new().add_modifier(Modifier::BOLD),
    );
    if let Some(first) = lines.first_mut()
        && let Some(span) = first.spans.first_mut()
        && span.content.starts_with(issue.identifier.as_str())
    {
        // 식별자만 강조색으로
        let rest = span.content[issue.identifier.len()..].to_string();
        let id = Span::styled(issue.identifier.clone(), ACCENT);
        *span = Span::styled(rest, span.style);
        first.spans.insert(0, id);
    }
    let mut meta = vec![
        Span::styled(
            format!(
                "{} {}",
                state_icon(&issue.state.state_type),
                sanitize(&issue.state.name)
            ),
            state_style(&issue.state),
        ),
        Span::styled(
            format!(" · 우선순위 {}", priority_label(issue.priority)),
            DIM,
        ),
    ];
    match &issue.assignee {
        Some(a) => meta.push(Span::styled(
            format!(" · @{}", sanitize(&a.display_name)),
            DIM,
        )),
        None => meta.push(Span::styled(" · 담당자 없음", DIM)),
    }
    lines.push(Line::from(meta));
    let mut extra: Vec<Span<'static>> = Vec::new();
    for label in &issue.labels.nodes {
        if !extra.is_empty() {
            extra.push(Span::raw(" "));
        }
        extra.push(Span::styled(sanitize(&label.name), label_style(label)));
    }
    let mut info = Vec::new();
    if let Some(p) = &issue.project {
        info.push(format!("프로젝트 {}", sanitize(&p.name)));
    }
    if let Some(c) = &issue.cycle {
        let name = c
            .name
            .clone()
            .unwrap_or_else(|| (c.number as i64).to_string());
        info.push(format!("사이클 {}", sanitize(&name)));
    }
    if let Some(p) = &issue.parent {
        info.push(format!("상위 {}", p.identifier));
    }
    if !info.is_empty() {
        let sep = if extra.is_empty() { "" } else { " · " };
        extra.push(Span::styled(format!("{sep}{}", info.join(" · ")), DIM));
    }
    if !extra.is_empty() {
        lines.push(Line::from(extra));
    }
    lines
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::LEFT).border_style(DIM);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let Some(issue) = app.selected_issue() else {
        return;
    };
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(1),
        ..inner
    };
    let mut lines = issue_header(issue, inner.width);
    lines.push(Line::default());
    match issue.description.as_deref().map(str::trim) {
        Some(body) if !body.is_empty() => lines.extend(
            markdown::render_with(body, inner.width, &Theme::default(), &app.team_keys()).lines,
        ),
        _ => lines.push(Line::from(Span::styled("(본문 없음)", DIM))),
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// 상세 화면을 그리고 최대 스크롤을 돌려준다.
fn draw_detail(f: &mut Frame, app: &App, area: Rect) -> u16 {
    let Some(d) = &app.detail else {
        return 0;
    };
    let area = Rect {
        x: area.x + 1,
        width: area.width.saturating_sub(2),
        ..area
    };
    let theme = Theme::default();
    let keys = app.team_keys();
    let mut lines: Vec<Line<'static>> = Vec::new();
    match &d.issue {
        None => lines.push(Line::from(Span::styled(
            format!("{} 불러오는 중…", sanitize(&d.id)),
            DIM,
        ))),
        Some(issue) => {
            lines.extend(issue_header(issue, area.width));
            lines.push(Line::from(Span::styled(issue.url.clone(), DIM)));
            if d.gone {
                lines.push(Line::from(Span::styled(
                    "보관되었거나 삭제된 이슈예요",
                    WARN,
                )));
            }
            lines.push(Line::default());
            match issue.description.as_deref().map(str::trim) {
                Some(body) if !body.is_empty() => {
                    lines.extend(markdown::render_with(body, area.width, &theme, &keys).lines)
                }
                _ => lines.push(Line::from(Span::styled("(본문 없음)", DIM))),
            }
            if !d.comments.is_empty() {
                lines.push(Line::default());
                lines.push(Line::from(Span::styled(
                    format!(
                        "── 코멘트 {}{} ──",
                        d.comments.len(),
                        if d.more_comments { "+" } else { "" }
                    ),
                    DIM,
                )));
                for c in &d.comments {
                    let who = c
                        .user
                        .as_ref()
                        .map_or("알 수 없음".to_string(), |u| sanitize(&u.display_name));
                    lines.push(Line::default());
                    lines.push(Line::from(vec![
                        Span::styled(who, Style::new().add_modifier(Modifier::BOLD)),
                        Span::styled(format!(" · {}", local_time(&c.created_at)), DIM),
                    ]));
                    lines.extend(markdown::render_with(&c.body, area.width, &theme, &keys).lines);
                }
                if d.more_comments {
                    lines.push(Line::default());
                    lines.push(Line::from(Span::styled(
                        "더 오래된 코멘트가 있어요. 브라우저에서 보세요 (o)",
                        DIM,
                    )));
                }
            } else if d.loading {
                lines.push(Line::default());
                lines.push(Line::from(Span::styled("코멘트 불러오는 중…", DIM)));
            }
        }
    }
    let total = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let max = total.saturating_sub(area.height);
    let scroll = d.scroll.min(max);
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
    max
}

fn hints(mode: Mode) -> &'static str {
    match mode {
        Mode::Search => " ⏎ 상세  Tab 보기  ↑↓ 이동  ^K 메뉴  Esc 목록 모드",
        Mode::List => " j/k 이동  / 검색  ⏎ 상세  o 브라우저  y ID 복사  ^K 메뉴  q 닫기",
        Mode::Detail => " j/k 스크롤  u 링크  o 브라우저  y ID 복사  ^K 메뉴  Esc 뒤로",
        Mode::Onboarding => " ⏎ 확인  Esc 닫기",
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect, now: i64) {
    let span = if let Some(text) = app.flash_text(now) {
        Span::styled(
            format!(" {}", sanitize(text)),
            Style::new().fg(Color::Green),
        )
    } else {
        match &app.problem {
            Some(Problem::Offline(m)) => Span::styled(
                format!(" 오프라인이라 저장된 내용을 보여줘요 ({})", sanitize(m)),
                WARN,
            ),
            Some(Problem::RateLimited(_)) => {
                Span::styled(" 한도를 넘어서 저장된 결과만 보여줘요", ERROR)
            }
            Some(Problem::Error(m)) => Span::styled(format!(" 오류: {}", sanitize(m)), ERROR),
            None => Span::styled(hints(app.mode), DIM),
        }
    };
    let text = truncate(&span.content, usize::from(area.width));
    f.render_widget(Paragraph::new(Span::styled(text, span.style)), area);
}

/// 가운데에 `w`×`h` 사각형.
fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn draw_menu(f: &mut Frame, app: &App, area: Rect) {
    let Some(menu) = &app.menu else {
        return;
    };
    let items = menu.visible();
    let w = (area.width * 6 / 10).max(30);
    let h = u16::try_from(items.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4);
    let rect = centered(area, w, h);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(ACCENT)
        .title(format!(" {} ", menu.title));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let [filter, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    let text = sanitize(&menu.filter);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", ACCENT),
            Span::raw(text.clone()),
        ])),
        filter,
    );
    f.set_cursor_position(Position::new(
        (filter.x + 2 + text.width() as u16).min(filter.right().saturating_sub(1)),
        filter.y,
    ));
    let label_width = usize::from(list.width.saturating_sub(2));
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|(label, _)| ListItem::new(truncate(&sanitize(label), label_width)))
        .collect();
    let mut state = ListState::default().with_selected(Some(menu.selected));
    f.render_stateful_widget(
        List::new(list_items)
            .highlight_style(SELECTED_BG)
            .highlight_symbol("▶ "),
        list,
        &mut state,
    );
}

fn draw_onboarding(f: &mut Frame, app: &App, area: Rect) {
    let rect = centered(area, 72, 11);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(ACCENT)
        .title(" Linear 연결 ");
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    if app.env_key_invalid {
        lines.push(Line::from(Span::styled(
            "LINEAR_API_KEY 환경 변수의 키가 유효하지 않아요",
            ERROR,
        )));
        lines.push(Line::from("환경 변수를 고치거나 지운 뒤 다시 열어주세요."));
        lines.push(Line::default());
        lines.push(Line::from(Span::styled("Esc 닫기", DIM)));
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }
    lines.push(Line::from("Linear Personal API 키를 붙여넣으세요."));
    lines.push(Line::from(Span::styled(
        "Linear → Settings → Security & access → Personal API keys",
        DIM,
    )));
    lines.push(Line::default());
    let masked = "•".repeat(app.key_input.chars().count());
    let shown = if app.key_checking {
        "확인 중…".to_string()
    } else {
        masked.clone()
    };
    lines.push(Line::from(vec![
        Span::styled("키: ", ACCENT),
        Span::raw(shown),
    ]));
    if let Some(err) = &app.key_error {
        lines.push(Line::from(Span::styled(sanitize(err), ERROR)));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("⏎ 확인 · Esc 닫기", DIM)));
    if !app.key_checking {
        f.set_cursor_position(Position::new(
            (inner.x + 4 + masked.width() as u16).min(inner.right().saturating_sub(1)),
            inner.y + 3,
        ));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

````

`src/ui/style.rs`에서 아래 부분을 찾아:

````rust
    }
}

/// 표시 폭 `width`에 맞게 자른다. 넘치면 끝을 `…`로 바꾼다.
pub fn truncate(s: &str, width: usize) -> String {
    if s.width() <= width {
````

아래로 바꾼다:

````rust
    }
}

/// "방금", "5분 전", "3시간 전", "2일 전".
pub fn ago(now_ms: i64, then_ms: i64) -> String {
    let mins = (now_ms - then_ms).max(0) / 60_000;
    match mins {
        0 => "방금".to_string(),
        m if m < 60 => format!("{m}분 전"),
        m if m < 60 * 24 => format!("{}시간 전", m / 60),
        m => format!("{}일 전", m / (60 * 24)),
    }
}

/// RFC 3339 시각을 로컬 시간 "YYYY-MM-DD HH:MM"으로. 해석하지 못하면 그대로.
pub fn local_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| rfc3339.to_string())
}

/// 표시 폭 `width`에 맞게 자른다. 넘치면 끝을 `…`로 바꾼다.
pub fn truncate(s: &str, width: usize) -> String {
    if s.width() <= width {
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib -- tui::view cli::`
Expected: `test result: ok. 39 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 190개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/tui/mod.rs src/cli.rs src/markdown.rs src/tui/view.rs src/ui/style.rs
git commit -m "feat(tui): 팔레트 화면" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 8: 호출 맥락 (context): 원래 pane, 브랜치 → 식별자

팔레트를 연 맥락을 읽는다. 팝업 프로세스의 작업 디렉터리는 플러그인 루트이고 `HERDR_PANE_ID`도 없다. 그래서 액션이 `HERDR_PLUGIN_CONTEXT_JSON`에서 원래 pane의 정보를 읽어 환경 변수로 넘긴다.

- `PluginContext`: `focused_pane_id`, `focused_pane_agent`, `focused_pane_cwd`, `workspace_cwd`, `selected_text`, `clicked_url`. 해석하지 못하면 빈 값이다.
- `Origin::from_context`: cwd는 pane cwd, 없으면 workspace cwd를 쓴다. 바로 열 이슈는 클릭한 링크, 없으면 선택 텍스트(식별자 형식일 때)에서 고른다.
- `Origin::from_env`: 팔레트 pane에서 쓴다. 넘겨받은 `HERDR_LINEAR_*` 환경 변수가 컨텍스트 JSON보다 앞선다. `to_env`는 그 반대 방향이다.
- `identifier_in_branch`: 브랜치명에서 `(?i)([a-z][a-z0-9]*)-([0-9]+)`를 찾는다. 앞부분이 주어진 팀 키일 때만 식별자로 본다.
- `current_branch`: `git rev-parse`로 저장소 루트와 브랜치를 얻는다. 저장소가 아니거나 detached HEAD면 `None`이다.

**Files:**
- Create: `src/context.rs`
- Modify: `src/lib.rs`
- Test: `src/context.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `search::query::parse_identifier`
- Produces:
  - `context::{ENV_PANE, ENV_AGENT, ENV_CWD, ENV_OPEN}: &str` (`HERDR_LINEAR_ORIGIN_PANE`, `…_AGENT`, `…_CWD`, `HERDR_LINEAR_OPEN`)
  - `context::PluginContext { focused_pane_id, focused_pane_agent, focused_pane_cwd, workspace_cwd, selected_text, clicked_url: Option<String> }`, `PluginContext::parse(&str)`
  - `context::Origin { pane_id: Option<String>, agent: Option<String>, cwd: Option<PathBuf>, open: Option<String> }`
  - `Origin::from_context(&PluginContext)`, `Origin::from_env(get: impl Fn(&str) -> Option<String>)`, `Origin::to_env(&self) -> Vec<(String, String)>`
  - `context::identifier_in_text(&str)`, `identifier_in_url(&str)`, `identifier_in_branch(branch: &str, team_keys: &[String])` (모두 `-> Option<String>`)
  - `context::current_branch(cwd: &Path) -> Option<(String, String)>` (저장소 루트, 브랜치)

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/lib.rs`에서 아래 부분을 찾아:

````rust

pub mod cli;
pub mod config;
pub mod linear;
pub mod markdown;
pub mod search;
````

아래로 바꾼다:

````rust

pub mod cli;
pub mod config;
pub mod context;
pub mod linear;
pub mod markdown;
pub mod search;
````

`src/context.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
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
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib context::`
Expected: 컴파일 실패 — `PluginContext`, `Origin` 같은 이름이 없다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/context.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
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

````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib context::`
Expected: `test result: ok. 7 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 197개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/context.rs
git commit -m "feat(context): 원래 pane 맥락과 브랜치 → 식별자" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 9: 팔레트 런타임 (tui::runtime, tui::system, log)

앱이 요청한 `Effect`를 캐시·네트워크·운영체제로 처리하고 결과를 `Msg`로 돌려준다. 이벤트 루프도 여기에 있다.

- **스레드**: 네트워크 요청은 작업 스레드에서 보내고, 결과(`Done`)는 채널로 받는다. SQLite 연결은 메인 스레드에서만 쓴다. 캐시 오류는 로그만 남기고 조회를 계속한다.
- **SWR**: 내 이슈·전체 탭과 상세는 캐시를 먼저 보내고(`fresh: false`) 서버 응답으로 바꾼다. 받은 이슈는 캐시에 넣고, 보관·삭제된 것은 뺀다. 최근 본은 로컬 기록(50개)이다.
- **워크스페이스 확인**: 이 키로 받은 viewer가 캐시에 없을 수 있다(키를 바꿨거나 처음 실행). 그때는 viewer를 받아 캐시의 워크스페이스를 맞출 때까지 다른 일을 미룬다. 다른 워크스페이스의 캐시를 보여 주지 않기 위해서다. viewer는 60분마다 다시 받는다.
- **현재 브랜치 이슈**: 원래 pane의 cwd에서 git으로 브랜치를 읽는다(스레드). 순서는 10분 캐시 → 브랜치명의 식별자(범위 팀 키, 캐시 → 서버) → 서버 브랜치 검색이다. "없음"도 기억한다.
- **키 입력**: 키를 `viewer`로 확인한 뒤 저장하고 그 키로 바꾼다. 틀린 키와 오프라인은 다른 문구로 알린다.
- **오류**: 인증 실패는 키 입력 화면으로 보낸다(환경 변수 키인지 함께 알림). 그 밖의 실패는 상단 상태로 보낸다. 모두 로그에 남긴다.
- **한도**: 응답을 받을 때마다 남은 요청을 본다. 50 미만이면 리셋 시각까지 자동 검색을 멈추라고 한 번 알린다(`Throttled`). 깊은 검색이 한도에 걸리면 분당 30회 한도 안내만 한다(M9). 응답의 리셋 시각은 시간당 한도 기준이라 이 경우와 맞지 않는다.
- **운영체제**: 브라우저 열기와 복사는 `System` trait로 분리해 테스트에서 바꿔 끼운다. 브라우저로는 http(s) 링크만 넘긴다. 이슈 본문 링크는 누구나 쓸 수 있기 때문이다. 복사는 OSC 52를 보낸 뒤 pbcopy(macOS), wl-copy·xclip·xsel(Linux) 순으로 시도한다.
- **로그**: 상태 디렉터리의 `herdr-linear.log`에 시각과 한 줄 문구를 쓴다. 1MB를 넘으면 `.1`로 하나 백업한다. 키는 남기지 않는다.
- **이벤트 루프**: `ratatui::init`(패닉 훅으로 터미널 복구) 전에 패닉을 로그에 남기는 훅을 건다. 붙여넣기(bracketed paste)를 켜고, 50ms마다 결과 반영 → 디바운스 → 그리기 → 입력 순으로 돈다.
- **지원 변경**: `cli`의 viewer 캐시 함수(`cached_viewer`, `save_viewer`, `scope_teams`, `VIEWER_TTL_MS`)를 공개한다. `queries::team_issues`는 범위 팀이 비면 모든 팀에서 가져온다(빈 `in` 조건은 아무것도 찾지 못한다).

**Files:**
- Create: `src/log.rs`, `src/tui/runtime.rs`, `src/tui/system.rs`
- Modify: `src/lib.rs`, `src/tui/mod.rs`, `src/cli.rs`, `src/linear/queries.rs`
- Test: `src/log.rs`, `src/tui/runtime.rs`, `src/tui/system.rs`, `src/cli.rs`, `src/linear/queries.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `tui::app`(Task 5), `tui::keys`(Task 6), `tui::view`(Task 7), `context`(Task 8), `cli::{now_ms, team_keys, scope_team_ids, scope_warning}`, `config::{save_api_key, Paths, Settings, ApiKey, KeySource}`, `linear::{queries, filter, client}`, `store::Store`
- Produces:
  - `log::{MAX_LOG_BYTES, Logger}`: `Logger::new(PathBuf)`, `Logger::write(&self, &str)`
  - `tui::system::System` trait (`open_url(&self, &str) -> Result<()>`, `copy(&self, &str) -> Result<()>`), `RealSystem`, `base64(&[u8]) -> String`
  - `tui::runtime::{BRANCH_TTL_MS, RECENT_LIMIT, DEEP_LIMIT_TEXT}`, `type MakeClient = Arc<dyn Fn(String) -> LinearClient + Send + Sync>`
  - `Runtime::new(paths: Paths, settings: Settings, store: Store, key: Option<ApiKey>, origin: Origin, system: Box<dyn System>, make_client: MakeClient) -> Runtime`
  - `Runtime::execute(&mut self, Effect, now: i64) -> Vec<Msg>`, `absorb(&mut self, Done, now) -> Vec<Msg>`, `drain(&self) -> Vec<Done>`, `wait(&self, Duration) -> Option<Done>`
  - `tui::runtime::run(rt: Runtime, app: App, effects: Vec<Effect>) -> Result<()>`, `is_web_url(&str) -> bool`
  - `cli::{VIEWER_TTL_MS, cached_viewer(store, key_fp) -> Result<Option<(Viewer, i64)>>, save_viewer (pub), scope_teams(viewer, settings) -> Vec<&TeamRef>}`

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/lib.rs`에서 아래 부분을 찾아:

````rust
pub mod config;
pub mod context;
pub mod linear;
pub mod markdown;
pub mod search;
pub mod store;
````

아래로 바꾼다:

````rust
pub mod config;
pub mod context;
pub mod linear;
pub mod log;
pub mod markdown;
pub mod search;
pub mod store;
````

`src/tui/mod.rs`에서 아래 부분을 찾아:

````rust

pub mod app;
pub mod keys;
pub mod view;
````

아래로 바꾼다:

````rust

pub mod app;
pub mod keys;
pub mod runtime;
pub mod system;
pub mod view;
````

`src/linear/queries.rs`에서 아래 부분을 찾아:

````rust
            .create();
        assert_eq!(branch_issue(&client(&server), "main").unwrap(), None);
    }
}
````

아래로 바꾼다:

````rust
            .create();
        assert_eq!(branch_issue(&client(&server), "main").unwrap(), None);
    }

    #[test]
    fn team_issues_without_scope_searches_all_teams() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/graphql")
            // 빈 객체는 부분 일치로는 무엇이든 맞으므로 본문 문자열로 확인한다 (ureq는 들여쓰기해서 보낸다)
            .match_body(Matcher::Regex(r#""filter":\s*\{\s*\}"#.into()))
            .with_body(
                json!({ "data": { "issues": { "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } } } })
                    .to_string(),
            )
            .create();
        team_issues(&client(&server), &[], None).unwrap();
        m.assert();
    }
}
````

`src/log.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_lines_and_rotates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("herdr-linear.log");
        let log = Logger::new(path.clone());
        log.write("첫 줄\n이어짐\u{1b}[2J");
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.ends_with("첫 줄 이어짐[2J\n"), "{text:?}");
        fs::write(&path, vec![b'x'; (MAX_LOG_BYTES + 1) as usize]).unwrap();
        log.write("새 파일");
        assert!(fs::read_to_string(&path).unwrap().ends_with("새 파일\n"));
        assert!(dir.path().join("herdr-linear.log.1").exists());
    }
}
````

`src/tui/runtime.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
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
````

`src/tui/system.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64("한".as_bytes()), "7ZWc");
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib -- tui::runtime tui::system log:: linear::queries cli::`
Expected: 컴파일 실패 — `Runtime`, `Logger`, `System`, `cached_viewer` 같은 이름이 없다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/cli.rs`에서 아래 부분을 찾아:

````rust
use crate::linear::client::{ApiError, LinearClient};
use crate::linear::filter::{build_issue_filter, token_filter};
use crate::linear::queries;
use crate::linear::types::{Comment, Issue, Viewer};
use crate::markdown::{self, Theme};
use crate::search::query::parse;
use crate::search::rank::{SearchIndex, merge, sort_mine};
````

아래로 바꾼다:

````rust
use crate::linear::client::{ApiError, LinearClient};
use crate::linear::filter::{build_issue_filter, token_filter};
use crate::linear::queries;
use crate::linear::types::{Comment, Issue, TeamRef, Viewer};
use crate::markdown::{self, Theme};
use crate::search::query::parse;
use crate::search::rank::{SearchIndex, merge, sort_mine};
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
use crate::ui::style::local_time;
pub use crate::ui::style::{ago, priority_label, state_icon};

const VIEWER_TTL_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const SEARCH_LIMIT: usize = 30;

````

아래로 바꾼다:

````rust
use crate::ui::style::local_time;
pub use crate::ui::style::{ago, priority_label, state_icon};

/// 내 정보(viewer)를 다시 받는 간격.
pub const VIEWER_TTL_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const SEARCH_LIMIT: usize = 30;

````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    Ok("API 키와 캐시를 지웠어요".to_string())
}

fn save_viewer(store: &Store, v: &Viewer, now: i64, key_fp: &str) -> Result<()> {
    store.ensure_org(&v.organization.id)?;
    store.meta_set("viewer", &serde_json::to_string(v)?)?;
    store.meta_set("viewer_at", &now.to_string())?;
````

아래로 바꾼다:

````rust
    Ok("API 키와 캐시를 지웠어요".to_string())
}

/// viewer를 저장한다. 워크스페이스가 바뀌었으면 캐시를 먼저 비운다.
pub fn save_viewer(store: &Store, v: &Viewer, now: i64, key_fp: &str) -> Result<()> {
    store.ensure_org(&v.organization.id)?;
    store.meta_set("viewer", &serde_json::to_string(v)?)?;
    store.meta_set("viewer_at", &now.to_string())?;
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
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
````

아래로 바꾼다:

````rust
/// 오프라인이면 같은 키로 받은 오래된 캐시라도 쓰고, 그것도 없으면 `None`.
pub fn load_viewer(ctx: &Ctx, force: bool) -> Result<Option<Viewer>> {
    let key_fp = ctx.client.key_fingerprint();
    let cached = cached_viewer(&ctx.store, &key_fp)?;
    if !force
        && let Some((v, at)) = &cached
        && ctx.now_ms - at < VIEWER_TTL_MS
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    }
}

/// 검색 범위 팀 id. config의 `teams`가 비어 있으면 내가 속한 팀 전부.
pub fn scope_team_ids(viewer: &Viewer, settings: &Settings) -> Vec<String> {
    viewer
        .teams
        .nodes
````

아래로 바꾼다:

````rust
    }
}

/// 같은 키(지문)로 저장된 viewer와 저장 시각. 다른 키로 받은 것은 쓰지 않는다.
pub fn cached_viewer(store: &Store, key_fp: &str) -> Result<Option<(Viewer, i64)>> {
    if store.meta_get("viewer_key")?.as_deref() != Some(key_fp) {
        return Ok(None);
    }
    Ok(
        match (store.meta_get("viewer")?, store.meta_get("viewer_at")?) {
            (Some(v), Some(at)) => serde_json::from_str(&v)
                .ok()
                .map(|v| (v, at.parse().unwrap_or(0))),
            _ => None,
        },
    )
}

/// 검색 범위 팀. config의 `teams`가 비어 있으면 내가 속한 팀 전부.
pub fn scope_teams<'a>(viewer: &'a Viewer, settings: &Settings) -> Vec<&'a TeamRef> {
    viewer
        .teams
        .nodes
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(&t.key))
        })
        .map(|t| t.id.clone())
        .collect()
}
````

아래로 바꾼다:

````rust
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(&t.key))
        })
        .collect()
}

/// 검색 범위 팀 id.
pub fn scope_team_ids(viewer: &Viewer, settings: &Settings) -> Vec<String> {
    scope_teams(viewer, settings)
        .into_iter()
        .map(|t| t.id.clone())
        .collect()
}
````

`src/linear/queries.rs`에서 아래 부분을 찾아:

````rust
    Ok(issues_page(c, &filter, None)?.nodes)
}

/// 범위 팀의 이슈를 최근 수정 순으로 한 페이지씩.
pub fn team_issues(
    c: &LinearClient,
    team_ids: &[String],
    after: Option<&str>,
) -> Result<IssuePage, ApiError> {
    let filter = json!({ "team": { "id": { "in": team_ids } } });
    issues_page(c, &filter, after)
}

````

아래로 바꾼다:

````rust
    Ok(issues_page(c, &filter, None)?.nodes)
}

/// 범위 팀의 이슈를 최근 수정 순으로 한 페이지씩. 팀 목록이 비어 있으면 모든 팀.
pub fn team_issues(
    c: &LinearClient,
    team_ids: &[String],
    after: Option<&str>,
) -> Result<IssuePage, ApiError> {
    let filter = if team_ids.is_empty() {
        json!({})
    } else {
        json!({ "team": { "id": { "in": team_ids } } })
    };
    issues_page(c, &filter, after)
}

````

`src/log.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
//! 로그 파일. 팔레트가 화면을 차지하는 동안 오류는 여기에 남긴다. 키는 남기지 않는다.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// 이 크기를 넘으면 `.1`로 하나 백업하고 새로 쓴다.
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

pub struct Logger {
    path: PathBuf,
}

impl Logger {
    pub fn new(path: PathBuf) -> Logger {
        Logger { path }
    }

    /// 한 줄 남긴다. 실패해도 조용히 넘어간다.
    pub fn write(&self, msg: &str) {
        if fs::metadata(&self.path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
            let mut backup = self.path.clone().into_os_string();
            backup.push(".1");
            let _ = fs::rename(&self.path, PathBuf::from(backup));
        }
        let line = format!(
            "{} {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            crate::markdown::sanitize(msg).replace('\n', " ")
        );
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

````

`src/tui/runtime.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
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
use ratatui::crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event};
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
                // 확인하지 못했어도 미뤄 둔 일은 한다 (오프라인이면 캐시로 보인다)
                self.log.write(&format!("내 정보: {e}"));
                self.open_gate(now)
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
        let c = client.clone();
        match self
            .note(cached_viewer(&self.store, &client.key_fingerprint()))
            .flatten()
        {
            Some((v, at)) => {
                let mut msgs = self.viewer_known(v);
                msgs.push(Msg::Index(self.all_issues()));
                if now - at >= VIEWER_TTL_MS {
                    self.spawn(move || Done::Viewer(queries::viewer(&c)));
                }
                msgs
            }
            None => {
                self.gate = Some(Vec::new());
                self.spawn(move || Done::Viewer(queries::viewer(&c)));
                Vec::new()
            }
        }
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
    let _ = execute!(stdout(), EnableBracketedPaste);
    let result = event_loop(&mut terminal, &mut rt, &mut app, effects);
    let _ = execute!(stdout(), DisableBracketedPaste);
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
        if let Some(max) = drawn.and_then(|d| d.detail_max_scroll) {
            app.set_detail_max_scroll(max);
        }
        if event::poll(Duration::from_millis(50))? {
            let input = match event::read()? {
                Event::Key(key) => keys::translate(app.mode, app.menu.is_some(), key),
                Event::Paste(text) => Some(Input::Paste(text)),
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
        Logger::new(path.clone()).write(&format!("패닉: {info}"));
        prev(info);
    }));
}

````

`src/tui/system.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
//! 브라우저 열기와 클립보드 복사. 테스트에서 바꿔 끼울 수 있게 trait으로 둔다.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

pub trait System {
    fn open_url(&self, url: &str) -> Result<()>;
    fn copy(&self, text: &str) -> Result<()>;
}

/// 실제 운영체제 명령을 쓴다.
pub struct RealSystem;

impl System for RealSystem {
    fn open_url(&self, url: &str) -> Result<()> {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let mut child = Command::new(opener)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("{opener}을 실행하지 못했어요"))?;
        // 기다리지 않되, 끝나면 거둬서 좀비 프로세스를 남기지 않는다
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    fn copy(&self, text: &str) -> Result<()> {
        // 원격 세션에서도 바깥 터미널 클립보드에 닿도록 OSC 52를 먼저 보낸다 (지원하지 않으면 무시된다)
        let mut out = std::io::stdout();
        let _ = write!(out, "\u{1b}]52;c;{}\u{7}", base64(text.as_bytes()));
        let _ = out.flush();
        for (cmd, args) in clipboard_commands() {
            let Ok(mut child) = Command::new(cmd)
                .args(*args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                continue;
            };
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
            }
            if child.wait()?.success() {
                return Ok(());
            }
        }
        Ok(())
    }
}

fn clipboard_commands() -> &'static [(&'static str, &'static [&'static str])] {
    if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["-b", "-i"]),
        ]
    }
}

/// 표준 base64 (OSC 52용).
pub fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(chunk[0]) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let sym = |shift: u32| TABLE[((n >> shift) & 63) as usize] as char;
        out.push(sym(18));
        out.push(sym(12));
        out.push(if chunk.len() > 1 { sym(6) } else { '=' });
        out.push(if chunk.len() > 2 { sym(0) } else { '=' });
    }
    out
}

````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib -- tui::runtime tui::system log:: linear::queries cli::`
Expected: `test result: ok. 55 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 217개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/tui/mod.rs src/cli.rs src/linear/queries.rs src/log.rs src/tui/runtime.rs src/tui/system.rs
git commit -m "feat(tui): 팔레트 런타임 — 캐시·네트워크·브라우저·클립보드" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 10: herdr 연결: open·ui 명령, 매니페스트, README

herdr 안에서 팔레트를 띄운다.

- `herdr::Herdr`: `HERDR_BIN_PATH`(없으면 PATH의 `herdr`)로 herdr CLI를 부른다. `open_pane`은 `herdr plugin pane open --plugin herdr-linear --entrypoint <id> --focus --env K=V…`, `notify`는 `herdr notification show <제목> --body <본문>`이다. 실패하면 stderr를 오류에 담는다. `ui_busy`면 "다른 창이 떠 있어요"라고 알린다.
- `herdr-linear open palette|url`: 액션이 부르는 명령이다. 컨텍스트에서 `Origin`을 만들어 팔레트 pane에 env로 넘긴다. 실패하면 액션 출력이 보이지 않으니 herdr 알림으로도 알린다.
- `herdr-linear ui --mode palette`: popup 안에서 도는 화면이다. 설정 경고는 하단에 한 번 보인다. 키가 없으면 키 입력 화면으로, `HERDR_LINEAR_OPEN`이 있으면 그 이슈 상세로 시작한다.
- `logout`을 herdr 액션으로 실행하면 결과를 알림으로도 보낸다.
- `herdr-plugin.toml`: 액션 `palette`, `open-url`, `logout`, popup pane `palette`(80%×80%), linear.app 이슈 링크 핸들러를 둔다. palette 액션의 contexts에는 `selection`도 넣는다.
- `README.md`: 설치, 키, 단축키(`prefix+i`, 한글 입력용 `ctrl+alt+i`), 키·검색어 사용법, 설정, 파일 위치를 적는다.
- `Cargo.toml`에 `rust-version = "1.88"`을 적는다(let-chain 사용).

**Files:**
- Create: `README.md`, `herdr-plugin.toml`, `src/herdr.rs`
- Modify: `src/lib.rs`, `src/tui/mod.rs`, `Cargo.toml`, `src/cli.rs`
- Test: `src/herdr.rs`, `src/cli.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `context::{Origin, PluginContext}`(Task 8), `tui::runtime::{Runtime, run, MakeClient}`, `tui::system::RealSystem`(Task 9), `tui::app::{App, Msg}`, `config::*`, `cli::now_ms`
- Produces:
  - `herdr::Herdr { bin }`: `from_env()`, `new(PathBuf)`, `open_pane(&self, entrypoint: &str, env: &[(String, String)]) -> Result<()>`, `notify(&self, title: &str, body: &str) -> Result<()>`
  - `herdr::open_palette(herdr: &Herdr, origin: &Origin) -> Result<()>`
  - `cli::Command::{Open { target: OpenTarget }, Ui { mode: UiMode }}`, `cli::OpenTarget::{Palette, Url}`, `cli::UiMode::Palette`
  - `cli::open_store(paths: &Paths, settings: &Settings, now: i64) -> Result<Store>` (열기 + 보관 기간 정리)
  - `tui::palette(paths: Paths) -> Result<()>`
  - `herdr-plugin.toml`, `README.md`

- [ ] **Step 1: 실패하는 테스트 작성**

아래 편집을 순서대로 적용한다.

`src/lib.rs`에서 아래 부분을 찾아:

````rust
pub mod cli;
pub mod config;
pub mod context;
pub mod linear;
pub mod log;
pub mod markdown;
````

아래로 바꾼다:

````rust
pub mod cli;
pub mod config;
pub mod context;
pub mod herdr;
pub mod linear;
pub mod log;
pub mod markdown;
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    }

    #[test]
    fn mine_sorts_and_saves_view() {
        let mut server = mockito::Server::new();
        mock_issues(
````

아래로 바꾼다:

````rust
    }

    #[test]
    fn cli_parses_herdr_commands() {
        let parse = |args: &[&str]| {
            Cli::try_parse_from(std::iter::once("herdr-linear").chain(args.iter().copied()))
                .map(|c| c.command)
        };
        assert_eq!(
            parse(&["open", "palette"]).unwrap(),
            Command::Open {
                target: OpenTarget::Palette
            }
        );
        assert_eq!(
            parse(&["open", "url"]).unwrap(),
            Command::Open {
                target: OpenTarget::Url
            }
        );
        assert_eq!(
            parse(&["ui", "--mode", "palette"]).unwrap(),
            Command::Ui {
                mode: UiMode::Palette
            }
        );
        assert!(parse(&["open", "side"]).is_err(), "사이드 패널은 3부");
        assert!(parse(&["ui"]).is_err());
    }

    #[test]
    fn mine_sorts_and_saves_view() {
        let mut server = mockito::Server::new();
        mock_issues(
````

`src/herdr.rs` 파일을 아래 내용으로 만든다 (테스트 모듈만 먼저):

````rust
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::context::PluginContext;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    /// 인자를 한 줄씩 `args.log`에 남기는 가짜 herdr. `plugin` 명령은 `fail_plugin`이면 실패한다.
    fn fake_herdr(dir: &Path, fail_plugin: Option<&str>) -> PathBuf {
        let bin = dir.join("herdr");
        let log = dir.join("args.log");
        let fail = match fail_plugin {
            Some(err) => format!("if [ \"$1\" = plugin ]; then echo '{err}' >&2; exit 1; fi\n"),
            None => String::new(),
        };
        let script = format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> '{log}'; done\necho '--' >> '{log}'\n{fail}exit 0\n",
            log = log.display()
        );
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    fn calls(dir: &Path) -> Vec<Vec<String>> {
        std::fs::read_to_string(dir.join("args.log"))
            .unwrap_or_default()
            .split("--\n")
            .filter(|c| !c.is_empty())
            .map(|c| c.lines().map(String::from).collect())
            .collect()
    }

    #[test]
    fn opens_palette_with_origin_env() {
        let dir = tempfile::tempdir().unwrap();
        let herdr = Herdr::new(fake_herdr(dir.path(), None));
        let ctx = PluginContext::parse(
            r#"{"focused_pane_id":"w1:p2","focused_pane_cwd":"/repo app","selected_text":"eng-7"}"#,
        );
        open_palette(&herdr, &Origin::from_context(&ctx)).unwrap();
        assert_eq!(
            calls(dir.path()),
            vec![vec![
                "plugin",
                "pane",
                "open",
                "--plugin",
                "herdr-linear",
                "--entrypoint",
                "palette",
                "--focus",
                "--env",
                "HERDR_LINEAR_ORIGIN_PANE=w1:p2",
                "--env",
                "HERDR_LINEAR_ORIGIN_CWD=/repo app",
                "--env",
                "HERDR_LINEAR_OPEN=ENG-7",
            ]]
        );
    }

    #[test]
    fn busy_herdr_is_reported_by_notification() {
        let dir = tempfile::tempdir().unwrap();
        let herdr = Herdr::new(fake_herdr(dir.path(), Some("error: ui_busy")));
        let err = open_palette(&herdr, &Origin::default()).unwrap_err();
        assert!(err.to_string().contains("다른 창이 떠 있어요"), "{err}");
        let calls = calls(dir.path());
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1][..4], ["notification", "show", "Linear", "--body"]);
        assert!(calls[1][4].starts_with("팔레트를 열지 못했어요"));
    }

    #[test]
    fn missing_herdr_binary_is_an_error() {
        let herdr = Herdr::new(PathBuf::from("/nonexistent/herdr"));
        assert!(herdr.notify("Linear", "x").is_err());
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test --lib -- herdr:: cli::`
Expected: 컴파일 실패 — `Herdr`, `open_palette`, `OpenTarget`, `UiMode`가 없다

- [ ] **Step 3: 구현**

아래 편집을 순서대로 적용한다.

`src/tui/mod.rs` 파일을 아래 내용으로 만든다:

````rust
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
            Msg::Flash(format!("설정 경고: {}", warnings.join(" · "))),
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
````

`Cargo.toml`에서 아래 부분을 찾아:

````toml
name = "herdr-linear"
version = "0.1.0"
edition = "2024"
description = "Fast Linear lookup for herdr"
license = "MIT"

````

아래로 바꾼다:

````toml
name = "herdr-linear"
version = "0.1.0"
edition = "2024"
rust-version = "1.88"
description = "Fast Linear lookup for herdr"
license = "MIT"

````

`README.md` 파일을 아래 내용으로 만든다:

````markdown
# herdr-linear

[herdr](https://herdr.dev) 안에서 Linear 이슈를 빠르게 찾고 읽는 플러그인이에요.
단축키로 팝업 팔레트를 띄워 검색하고, 본문(markdown)과 코멘트를 터미널에서 바로 봐요.

- 내 이슈 · 최근 본 · 전체 탭, 상태와 라벨은 Linear에 설정된 색으로 보여요
- 입력하는 즉시 캐시에서 찾고, 입력이 멈추면 서버에서도 찾아 합쳐요
- 지금 브랜치(`me/eng-123-...`)에 연결된 이슈를 맨 위에 고정해요
- 터미널에 보이는 linear.app 이슈 링크를 Ctrl+클릭하면 팔레트에서 열려요
- 한 번 본 내용은 캐시에 남아 다음에 바로 보이고, 오프라인에서도 볼 수 있어요

## 설치

Rust(1.88 이상)와 herdr 0.9.3 이상이 필요해요. 이 저장소 루트에서 빌드하고 연결해요.

```sh
cargo build --release
herdr plugin link .
```

## API 키

Linear → Settings → Security & access → Personal API keys에서 키를 만들어요.
팔레트를 처음 열면 키 입력 화면이 나와요. 붙여넣고 Enter를 누르면 확인한 뒤 저장해요.
터미널에서 `./target/release/herdr-linear login`으로 넣어도 돼요.

- 키는 설정 디렉터리의 `credentials` 파일에 권한 0600으로 저장돼요
- `LINEAR_API_KEY` 환경 변수가 있으면 그 키를 먼저 써요
- 키는 로그·화면·오류 메시지에 남기지 않아요

## 단축키

플러그인은 키를 직접 등록할 수 없어서 `~/.config/herdr/config.toml`에 추가해요.

```toml
[[keys.command]]
key = "prefix+i"
type = "plugin_action"
command = "herdr-linear.palette"
description = "Linear 검색"

# 한글 입력 중에는 prefix 뒤 글자가 IME에 먹혀요. 바로 쓰는 키를 하나 더 두면 편해요
[[keys.command]]
key = "ctrl+alt+i"
type = "plugin_action"
command = "herdr-linear.palette"
description = "Linear 검색"
```

액션은 command-palette 플러그인에도 나타나요. 이슈 식별자(`ENG-123`)를 선택한 채로 팔레트를 열면 그 이슈가 바로 열려요.

## 팔레트

| 모드 | 키 |
|---|---|
| 검색 (기본) | 글자 = 검색어, ↑/↓·Ctrl+P/N = 이동, Enter = 상세, Tab/Shift+Tab = 탭, Ctrl+K = 동작 메뉴, Esc = 목록 모드 |
| 목록 | j/k = 이동, g/G = 처음/끝, `/` = 검색, Enter = 상세, `r` = 새로고침, `o` = 브라우저, `y` = ID 복사, `Y` = URL 복사, `q`·Esc = 닫기 |
| 상세 | j/k = 스크롤, Ctrl+D/U = 반 페이지, g/G = 처음/끝, `u` = 본문 링크 목록, `o`·`y`·`Y`·`r`, Esc = 뒤로 |

Ctrl 조합은 한글 입력 중에도 동작하고, 목록·상세에서는 한글 자모 키도 같은 영문 키로 받아요(ㅓ→j, ㅏ→k 등).

### 검색어

자유 텍스트와 토큰을 섞어 써요. 조건은 모두 AND예요. 공백이 있는 값은 따옴표로 감싸요(`s:"In Progress"`).

| 토큰 | 뜻 | 예 |
|---|---|---|
| `s:값` | 상태 이름 또는 종류(`진행`, `할일`, `완료`, `started`…) | `s:진행` |
| `l:값` | 라벨 | `l:bug` |
| `@값` | 담당자 (`@나`, `@me`는 나) | `@민수` |
| `#KEY` | 팀 | `#ENG` |
| `p:값` | 우선순위 (`긴급`/`urgent`/`1` … `없음`/`none`/`0`) | `p:high` |

목록 맨 아래 "서버에서 검색 (코멘트 포함)"을 고르면 코멘트까지 찾아요(분당 30회 제한).

## 설정

`~/.config/herdr/plugins/config/herdr-linear/config.toml` (모든 항목은 선택)

```toml
teams = ["ENG", "OPS"]   # 검색·전체 탭 범위. 비우면 내가 속한 팀 전부

[cache]
retention_days = 30      # 이 기간 동안 안 받고 안 본 이슈는 캐시에서 지워요
```

## 파일

| 파일 | 위치 |
|---|---|
| `config.toml`, `credentials` | `~/.config/herdr/plugins/config/herdr-linear/` |
| `cache.db`, `herdr-linear.log` | `~/.local/state/herdr/plugins/herdr-linear/` |

herdr 밖에서 CLI로 쓰면 `~/.config/herdr-linear/`와 `~/.local/state/herdr-linear/`를 써요. 키는 두 위치를 서로 찾아 써요.

## CLI

```sh
herdr-linear login | logout | whoami
herdr-linear mine
herdr-linear search 로그인 l:bug @나
herdr-linear search --deep 세션 만료
herdr-linear show ENG-123
```

## 문제 해결

- **팔레트가 안 열려요**: herdr 설정 화면이나 복사 모드가 떠 있으면 팝업을 열 수 없어요. 닫고 다시 눌러요.
- **"오프라인"**: 저장된 내용으로 계속 보여 주고, 다음 조회 때 다시 시도해요.
- **"한도 초과"**: 표시된 시간이 지나면 다시 찾아요. 그동안 캐시 검색은 계속 돼요.
- **환경 변수 키 오류**: `LINEAR_API_KEY`의 키가 틀렸어요. 환경 변수를 고치거나 지워요.
- 자세한 오류는 `herdr-linear.log`에 남아요.
````

`herdr-plugin.toml` 파일을 아래 내용으로 만든다:

````toml
id = "herdr-linear"
name = "Linear"
version = "0.1.0"
min_herdr_version = "0.9.3"
description = "Fast Linear lookup in herdr: search, read, and open issues from a palette."
platforms = ["macos", "linux"]

[[build]]
command = ["cargo", "build", "--release"]

[[actions]]
id = "palette"
title = "Linear: 검색 팔레트"
description = "Linear 이슈 검색 팔레트를 연다 (이슈 식별자를 선택해 두면 그 이슈를 바로 연다)"
contexts = ["global", "workspace", "pane", "selection"]
command = ["./target/release/herdr-linear", "open", "palette"]

[[actions]]
id = "open-url"
title = "Linear: 링크로 이슈 열기"
description = "Ctrl+클릭한 linear.app 이슈 링크를 팔레트에서 연다"
contexts = ["global", "workspace", "pane"]
command = ["./target/release/herdr-linear", "open", "url"]

[[actions]]
id = "logout"
title = "Linear: 로그아웃"
description = "저장된 API 키와 캐시를 지운다"
contexts = ["global"]
command = ["./target/release/herdr-linear", "logout"]

[[panes]]
id = "palette"
title = "Linear"
placement = "popup"
width = "80%"
height = "80%"
command = ["./target/release/herdr-linear", "ui", "--mode", "palette"]

[[link_handlers]]
id = "linear-issue"
title = "Linear 이슈 열기"
pattern = "^https://linear\\.app/[^/]+/issue/[A-Za-z0-9]+-[0-9]+"
action = "open-url"
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
//! 터미널 조회 명령: login, logout, whoami, mine, search, show.

use std::io::IsTerminal;

````

아래로 바꾼다:

````rust
//! 명령: login, logout, whoami, mine, search, show와 herdr용 open, ui.

use std::io::IsTerminal;

````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
use clap::{Parser, Subcommand};

use crate::config::{self, KeySource, Paths, Settings};
use crate::linear::client::{ApiError, LinearClient};
use crate::linear::filter::{build_issue_filter, token_filter};
use crate::linear::queries;
````

아래로 바꾼다:

````rust
use clap::{Parser, Subcommand};

use crate::config::{self, KeySource, Paths, Settings};
use crate::context::{Origin, PluginContext};
use crate::herdr::{Herdr, open_palette};
use crate::linear::client::{ApiError, LinearClient};
use crate::linear::filter::{build_issue_filter, token_filter};
use crate::linear::queries;
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    },
    /// 이슈 상세 (예: ENG-131)
    Show { id: String },
}

/// 명령 실행에 필요한 것들.
````

아래로 바꾼다:

````rust
    },
    /// 이슈 상세 (예: ENG-131)
    Show { id: String },
    /// herdr 액션: 원래 pane의 맥락을 넘겨 팔레트를 띄운다
    Open {
        /// palette: 단축키·명령 팔레트, url: Linear 이슈 링크 Ctrl+클릭
        #[arg(value_enum)]
        target: OpenTarget,
    },
    /// herdr pane 안에서 도는 화면
    Ui {
        #[arg(long, value_enum)]
        mode: UiMode,
    },
}

/// 둘 다 같은 팔레트를 띄운다. 링크 클릭이면 컨텍스트의 `clicked_url` 이슈로 바로 연다.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenTarget {
    Palette,
    Url,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiMode {
    Palette,
}

/// 명령 실행에 필요한 것들.
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
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
````

아래로 바꾼다:

````rust
            &config::default_credential_fallbacks(),
        )?
        .ok_or_else(|| anyhow!("API 키가 없어요. 먼저 `herdr-linear login`을 실행하세요"))?;
        let now = now_ms();
        let store = open_store(&paths, &settings, now)?;
        let width = ratatui::crossterm::terminal::size()
            .map(|(w, _)| w)
            .unwrap_or(100)
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
    let paths = Paths::from_env()?;
    match cli.command {
        Command::Login => return login(&paths),
        Command::Logout => return logout(&paths, &config::default_credential_fallbacks()),
        _ => {}
    }
    let ctx = Ctx::open(paths)?;
````

아래로 바꾼다:

````rust
    let paths = Paths::from_env()?;
    match cli.command {
        Command::Login => return login(&paths),
        Command::Logout => {
            let out = logout(&paths, &config::default_credential_fallbacks())?;
            // herdr 액션으로 실행되면 출력이 보이지 않으니 알림으로 알린다
            if std::env::var_os("HERDR_PLUGIN_ACTION_ID").is_some() {
                let _ = Herdr::from_env().notify("Linear", &out);
            }
            return Ok(out);
        }
        Command::Open { .. } => {
            let ctx = PluginContext::parse(
                &std::env::var("HERDR_PLUGIN_CONTEXT_JSON").unwrap_or_default(),
            );
            open_palette(&Herdr::from_env(), &Origin::from_context(&ctx))?;
            return Ok(String::new());
        }
        Command::Ui {
            mode: UiMode::Palette,
        } => {
            crate::tui::palette(paths)?;
            return Ok(String::new());
        }
        _ => {}
    }
    let ctx = Ctx::open(paths)?;
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
        Command::Mine => mine(&ctx),
        Command::Search { deep, query } => search(&ctx, &query.join(" "), deep),
        Command::Show { id } => show(&ctx, &id),
        Command::Login | Command::Logout => Ok(String::new()),
    };
    if std::env::var_os("HERDR_LINEAR_DEBUG").is_some() {
        let r = ctx.client.rate_limit();
````

아래로 바꾼다:

````rust
        Command::Mine => mine(&ctx),
        Command::Search { deep, query } => search(&ctx, &query.join(" "), deep),
        Command::Show { id } => show(&ctx, &id),
        Command::Login | Command::Logout | Command::Open { .. } | Command::Ui { .. } => {
            Ok(String::new())
        }
    };
    if std::env::var_os("HERDR_LINEAR_DEBUG").is_some() {
        let r = ctx.client.rate_limit();
````

`src/cli.rs`에서 아래 부분을 찾아:

````rust
        eprintln!("경고: 캐시를 열지 못해 이번에는 저장 없이 실행해요 ({e:#})");
        Store::open_in_memory()
    })
}

/// 키(보조 위치 포함)와 캐시를 지운다. `fallbacks`는 실제 실행에서만 HOME 기준 위치를 넘긴다.
````

아래로 바꾼다:

````rust
        eprintln!("경고: 캐시를 열지 못해 이번에는 저장 없이 실행해요 ({e:#})");
        Store::open_in_memory()
    })
}

/// 캐시를 열고 보관 기간(`cache.retention_days`)이 지난 이슈를 지운다.
pub fn open_store(paths: &Paths, settings: &Settings, now: i64) -> Result<Store> {
    let store = open_cache(paths)?;
    let retention = (settings.cache_retention_days as i64).saturating_mul(DAY_MS);
    store.evict_older_than(now.saturating_sub(retention))?;
    Ok(store)
}

/// 키(보조 위치 포함)와 캐시를 지운다. `fallbacks`는 실제 실행에서만 HOME 기준 위치를 넘긴다.
````

`src/herdr.rs` 맨 위(테스트 모듈 위)에 아래 구현을 넣는다:

````rust
//! herdr CLI 호출: 플러그인 pane 열기와 알림.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::config::PLUGIN_ID;
use crate::context::Origin;

pub struct Herdr {
    bin: PathBuf,
}

impl Herdr {
    /// herdr가 액션에 넘겨 준 `HERDR_BIN_PATH`, 없으면 PATH의 `herdr`.
    pub fn from_env() -> Herdr {
        let bin = std::env::var_os("HERDR_BIN_PATH")
            .filter(|v| !v.is_empty())
            .map_or_else(|| PathBuf::from("herdr"), PathBuf::from);
        Herdr::new(bin)
    }

    pub fn new(bin: PathBuf) -> Herdr {
        Herdr { bin }
    }

    /// 이 플러그인의 pane을 띄우고 포커스를 옮긴다. `env`는 pane 프로세스의 환경 변수가 된다.
    pub fn open_pane(&self, entrypoint: &str, env: &[(String, String)]) -> Result<()> {
        let mut args: Vec<String> = [
            "plugin",
            "pane",
            "open",
            "--plugin",
            PLUGIN_ID,
            "--entrypoint",
            entrypoint,
            "--focus",
        ]
        .map(String::from)
        .to_vec();
        for (k, v) in env {
            args.push("--env".into());
            args.push(format!("{k}={v}"));
        }
        self.run(&args)
    }

    /// herdr 알림. 액션은 출력이 보이지 않아서 결과를 이걸로 알린다.
    pub fn notify(&self, title: &str, body: &str) -> Result<()> {
        self.run(&[
            "notification".into(),
            "show".into(),
            title.into(),
            "--body".into(),
            body.into(),
        ])
    }

    fn run(&self, args: &[String]) -> Result<()> {
        let out = Command::new(&self.bin)
            .args(args)
            .output()
            .with_context(|| format!("{}을 실행하지 못했어요", self.bin.display()))?;
        if out.status.success() {
            return Ok(());
        }
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.contains("ui_busy") {
            bail!("herdr에 설정·복사 모드 같은 다른 창이 떠 있어요. 닫고 다시 시도하세요");
        }
        if err.is_empty() {
            bail!("herdr 명령이 실패했어요 ({})", out.status);
        }
        bail!("herdr 명령이 실패했어요: {err}")
    }
}

/// `open palette`·`open url` 액션: 원래 pane의 맥락을 팔레트 pane에 넘겨 띄운다.
/// 실패하면 액션 출력은 보이지 않으니 herdr 알림으로도 알린다.
pub fn open_palette(herdr: &Herdr, origin: &Origin) -> Result<()> {
    herdr
        .open_pane("palette", &origin.to_env())
        .inspect_err(|e| {
            let _ = herdr.notify("Linear", &format!("팔레트를 열지 못했어요: {e:#}"));
        })
}

````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test --lib -- herdr:: cli::`
Expected: `test result: ok. 32 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, lib 테스트 221개 통과(성능 테스트 1개 ignored).

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/tui/mod.rs Cargo.toml README.md herdr-plugin.toml src/cli.rs src/herdr.rs
git commit -m "feat: herdr 연결 — open·ui 명령, 매니페스트, README" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

### Task 11: herdr에서 직접 확인

**Files:**
- Create: `docs/superpowers/notes/2026-10-07-part2-herdr-checks.md`

이 작업은 사람이 herdr 화면에서 직접 확인한다. 실행자는 빌드하고 연결 방법을 안내한다. 체크리스트를 사용자에게 보여 주고, 결과를 받아 기록한다. 플러그인 연결(`herdr plugin link`)과 herdr 설정 파일 수정은 사용자의 환경을 바꾸므로 사용자에게 맡기거나 먼저 허락을 받는다.

- [ ] **Step 1: release 빌드와 전체 점검**

````bash
cargo build --release
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: 빌드 성공, clippy 경고 0개, 테스트 모두 통과.

- [ ] **Step 2: 플러그인 연결과 단축키 (사용자)**

사용자에게 아래를 요청한다.

````bash
herdr plugin link .   # 이 저장소 루트에서
herdr plugin list     # herdr-linear (Linear) enabled 확인
````

`~/.config/herdr/config.toml`에 추가한다(README의 단축키 절과 같다).

````toml
[[keys.command]]
key = "prefix+i"
type = "plugin_action"
command = "herdr-linear.palette"
description = "Linear 검색"

[[keys.command]]
key = "ctrl+alt+i"
type = "plugin_action"
command = "herdr-linear.palette"
description = "Linear 검색"
````

Expected: `herdr plugin list`에 `herdr-linear`가 enabled로 보인다. 매니페스트 오류가 있으면 link가 거절되니, 그 문구를 고친 뒤 다시 연결한다.

- [ ] **Step 3: 체크리스트 (사용자가 확인)**

| # | 동작 | 기대 |
|---|---|---|
| 1 | `prefix+i` (또는 `ctrl+alt+i`) | 화면 80% 크기 popup에 "내 이슈"가 뜬다. 상태 아이콘·라벨이 Linear 색이고, 상단에 "갱신 중…" → "방금 갱신"이 보인다 |
| 2 | 키가 없을 때 첫 실행 (선택) | 키 입력 화면이 뜬다. 붙여넣으면 가려 보이고, Enter 뒤 "…워크스페이스에 연결됐어요"가 나온다 |
| 3 | 한글로 "로그인" 입력 | 글자가 검색 줄 커서 자리에서 조합된다. 결과가 바로 바뀌고, 잠시 뒤 서버 결과가 합쳐져도 고른 줄이 유지된다 |
| 4 | 목록 끝의 "서버에서 검색 (코멘트 포함)" | 코멘트에만 있는 단어도 찾는다 |
| 5 | Enter → 상세 | 머리 정보, markdown 본문(제목·목록·코드 블록 배경·표), 코멘트(오래된 것부터)가 보인다. `u`로 링크 목록, Esc로 뒤로 간다 |
| 6 | `o`, `y`, `Y` (목록 모드: Esc 후) | 브라우저가 열린다. 다른 곳에 붙여넣으면 ID·URL이 나온다. 하단에 "복사됨: …"이 보인다 |
| 7 | Tab | "최근 본"에 방금 연 이슈가 있다. "전체"는 끝까지 내리면 다음 페이지가 붙는다 |
| 8 | 브랜치 `<팀키>-<번호>-…`인 저장소의 pane에서 열기 | 맨 위 "현재 브랜치" 칸에 그 이슈가 있다 |
| 9 | pane에서 `ENG-123` 같은 식별자를 선택한 채로 열기 | 그 이슈 상세로 바로 시작한다 |
| 10 | 터미널의 `https://linear.app/…/issue/ENG-123/…` 링크를 Ctrl+클릭 | 팔레트가 그 이슈 상세로 열린다 |
| 11 | herdr 설정 화면이나 복사 모드를 띄운 채 단축키 | herdr 알림으로 "다른 창이 떠 있어요…"가 뜬다 |
| 12 | 일반 pane에서 `HTTPS_PROXY=http://127.0.0.1:9 ./target/release/herdr-linear ui --mode palette` | 저장된 내용이 보이고 상단에 "오프라인", 하단에 사유가 보인다. q로 나온 뒤 터미널이 원래대로 돌아온다 |
| 13 | 한글 입력 상태로 목록 모드에서 ㅓ/ㅏ(j/k), Ctrl+K | 이동과 메뉴가 동작한다 |

Expected: 13개 항목이 모두 기대대로 동작한다. 다르면 그 항목을 고치는 작업을 추가한다(테스트 먼저). 고친 뒤 이 체크리스트를 다시 확인한다.

- [ ] **Step 4: 결과 기록과 커밋**

`docs/superpowers/notes/2026-10-07-part2-herdr-checks.md`에 항목별 결과(통과/수정 내용), herdr 버전, 터미널 앱을 적는다.

````bash
git add docs/superpowers/notes/2026-10-07-part2-herdr-checks.md
git commit -m "docs: herdr 팔레트 수동 확인 결과 (2부)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````


## 끝나면

- 최종 리뷰는 위 Review Focus를 하나씩 확인한다. 특히 다음 경계를 본다.
  - 스레드: 캐시는 메인 스레드에서만 쓰는지, `in_flight`와 `loading`이 맞게 세어지는지.
  - 터미널 복구: 정상 종료, 오류, 패닉 모두에서 원래대로 돌아오는지.
- 3부로 넘길 것을 `docs/superpowers/notes/`에 적는다.
  - 사이드 패널(`open side`, 주기 새로고침, `side-panes.json`).
  - 변경 동작, 에이전트 전달.
  - 오래 도는 프로세스의 캐시 손상 처리.
  - CLI broken pipe.
