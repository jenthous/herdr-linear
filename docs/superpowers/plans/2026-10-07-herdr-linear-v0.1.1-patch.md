# herdr-linear v0.1.1 패치 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** v0.1 리뷰에서 미룬 작은 문제 가운데 사이드 패널과 상관없는 것을 고친다. 한도 초과는 잠깐만 알리고, 상세의 `q`는 닫기로 하고, 목록 스크롤·링크 번호·경고·로그·sanitize·CLI 오프라인 대체·broken pipe를 바로잡는다.

**Architecture:** 구조는 그대로 둔다. `tui::app`(순수 상태 머신) → `tui::runtime`(캐시·네트워크) → `tui::view`(그리기) 흐름에 작은 값(`Msg::DeepLimited`, `Msg::Warn`, `App::list_offset`, `Drawn::list_offset`)만 더한다. CLI는 `cli.rs`의 오프라인 대체와 출력 함수만 바꾼다.

**Tech Stack:** Rust 2024 edition(최소 1.88), ratatui 0.30.2, pulldown-cmark 0.13.4, rusqlite 0.40.2, ureq 3.4.2, 테스트는 mockito·tempfile. **새 의존성은 없다.**

**Spec:** `docs/superpowers/specs/2026-10-06-herdr-linear-design.md`
- 입력: `docs/superpowers/notes/2026-10-07-part2-followups.md`의 N1, N3, N4, N5, N7, N8, N9, N11, N12, N13과, 같은 메모 "계획에서 3부로 넘긴 것"의 broken pipe, 키 교체 직후 `mine`의 오프라인 대체.
- 3부로 남기는 것: N2(키 교체 때 세션 상태), N6(`Failed`가 대기를 푸는 범위), N10(가만히 있어도 다시 그리기), 실행 중 캐시 손상, markdown 외형(목록·인용 안 표, 체크박스 번호 목록).
- 스펙과 다르게 정한 것(Task 12에서 스펙에 반영한다):
  1. 한도 초과는 상단에 남기지 않는다. 하단에 3초 동안 언제 다시 시도할지 알리고, 리셋 시각까지 자동 서버 검색만 멈춘다(사용자 결정, 2026-10-07).
  2. 설정·범위 경고는 일반 안내(3초)와 따로 10초 동안 보이고, 키 입력 화면에서도 보인다.
  3. CLI의 오프라인 대체(`mine`, `search`, `show`)는 이 키로 워크스페이스를 확인한 캐시일 때만 쓴다.

## 시작 전에

- `main`(027f13a)에 `v0.1.0` 태그를 붙이고 작업 브랜치를 만든다: `git tag -a v0.1.0 -m "…" 027f13a && git switch -c fix/v0.1.1`.
- 각 작업의 단계: Step 1 테스트 → Step 2 실패 확인(대개 컴파일 실패) → Step 3 구현 → Step 4 통과 확인 → Step 5 커밋.
- 편집 블록: "찾아 바꾼다"는 첫 블록이 그 시점의 파일에 정확히 한 번 나온다(Edit 도구의 old_string/new_string). "테스트 모듈 끝에 넣는다"는 `mod tests`의 마지막 `}` 바로 앞에 넣는다.
- 작업마다 끝에서 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`가 통과해야 한다. 시작 시점에는 236개 통과, 1개 무시다.

## Global Constraints

- 의존성을 더하거나 버전을 바꾸지 않는다. `rust-version = "1.88"`.
- 화면 문구는 한국어다. 기존 말투(해요체)를 따른다.
- 화면·터미널에 쓰는 외부 문자열(이슈·사용자·팀 이름, 식별자, URL, 서버 오류 문구, markdown)은 모두 제어 문자를 지운 뒤 쓴다.
- 하단 안내는 3초(`FLASH_MS`), 경고는 10초(`WARN_MS`)다.
- 세는 요청(`loading`)과 그 끝맺음은 1:1이다. 새 `Msg`가 요청을 끝내면 `done_loading()`을 부른다.
- 테스트에 순번·카운터 같은 내부 값을 하드코딩하지 않는다. 요청에서 읽어 쓴다.
- 키는 Authorization 헤더와 credentials 파일에만 쓴다. 로그·화면·오류 문구에는 남기지 않는다.

## Review Focus

1. 자동 검색이 멈춘 동안 직접 새로고침했는데 또 한도에 걸린다 → 안내가 다시 뜨고 멈춤이 다시 걸린다. (Task 1 `refresh_that_hits_the_limit_again_pauses_again`)
2. 스크롤한 뒤 목록이 화면보다 짧아진다(검색 결과가 줄거나 새로 받은 탭이 짧다) → 빈 칸 없이 처음부터 보인다. (Task 3 `shorter_list_after_scrolling_starts_from_the_top`)
3. 본문이 비어 있고 코멘트에만 링크·이미지가 있다 → 번호가 1부터 시작하고 `u` 목록과 같다. (Task 4 `links_in_comments_only_start_at_one`)
4. 아주 작은 화면의 키 입력 화면에 경고가 있다 → 패닉하지 않는다. (Task 7 `tiny_key_screen_with_a_warning_does_not_panic`)
5. 오프라인인데 같은 키로 확인한 캐시가 있다 → 예전처럼 저장된 결과를 보인다(키를 바꾼 경우만 막는다). (Task 10에서 기존 오프라인 테스트에 `trust_cache`를 더해 그대로 통과)

---

### Task 1: 한도 초과는 잠깐 알리기 (+N1 남은 부분, N7)

**Files:**
- Modify: `src/tui/app.rs` (상수, `Msg`, `Problem`, `apply`, 테스트)
- Modify: `src/tui/view.rs` (`status_span`, `draw_footer`, 테스트)
- Modify: `src/tui/runtime.rs` (`DEEP_LIMIT_TEXT` 삭제, 깊은 검색 한도 처리, 테스트)

**Interfaces:**
- Produces: `app::DEEP_LIMIT_TEXT`, `Msg::DeepLimited`, `Problem`에서 `RateLimited` 삭제, `fn minutes_left(until, now) -> i64`(app.rs 비공개).

- [ ] **Step 1: 테스트를 바꾼다**

`src/tui/app.rs`에서 `rate_limit_pauses_server_search_until_reset` 테스트 전체를 찾아 아래로 바꾼다.

```rust
    #[test]
    fn rate_limit_is_a_short_notice_and_pauses_server_search() {
        let mut app = started();
        app.apply(Msg::Failed(ApiError::Offline("연결 끊김".into())), T0);
        app.apply(
            Msg::Failed(ApiError::RateLimited {
                reset_at_ms: Some(T0 + 60_000),
            }),
            T0,
        );
        assert_eq!(app.problem, None, "상단에 남기지 않는다");
        assert_eq!(
            app.flash_text(T0),
            Some("Linear API 한도를 넘었어요. 1분 후 다시 시도하세요")
        );
        assert_eq!(app.flash_text(T0 + FLASH_MS), None, "잠깐만 보인다");
        type_str(&mut app, "결제", T0);
        assert!(app.tick(T0 + 1_000).is_empty());
        assert_eq!(app.tick(T0 + 60_000).len(), 1);
    }

    #[test]
    fn rate_limit_without_reset_keeps_the_throttle_pause() {
        let mut app = started();
        app.apply(Msg::Throttled(T0 + 120_000), T0);
        app.apply(Msg::Failed(ApiError::RateLimited { reset_at_ms: None }), T0);
        assert_eq!(
            app.flash_text(T0),
            Some("Linear API 한도를 넘었어요. 잠시 뒤 다시 시도하세요")
        );
        type_str(&mut app, "결제", T0);
        assert!(app.tick(T0 + 1_000).is_empty(), "앞서 정한 멈춤이 남는다");
        assert_eq!(app.tick(T0 + 120_000).len(), 1);
    }

    #[test]
    fn refresh_that_hits_the_limit_again_pauses_again() {
        let mut app = started();
        let limited = |reset| {
            Msg::Failed(ApiError::RateLimited {
                reset_at_ms: Some(reset),
            })
        };
        app.apply(limited(T0 + 60_000), T0);
        type_str(&mut app, "결제", T0);
        app.handle(Input::Esc, T0);
        app.handle(Input::Act(Act::Refresh), T0 + 5_000);
        searched(&app.tick(T0 + 5_000), "결제");
        app.apply(limited(T0 + 60_000), T0 + 6_000);
        assert_eq!(app.loading, 0);
        assert!(app.flash_text(T0 + 6_000).is_some_and(|t| t.contains("한도")));
        app.handle(Input::Search, T0 + 6_000);
        type_str(&mut app, "x", T0 + 6_000);
        assert!(app.tick(T0 + 7_000).is_empty(), "다시 멈춘다");
    }

    #[test]
    fn deep_search_limit_keeps_local_results_and_refresh_time() {
        let mut app = started();
        type_str(&mut app, "로그인", T0 + 1_000);
        let before = ids(&app);
        app.handle(Input::Bottom, T0 + 1_000);
        let effects = app.handle(Input::Enter, T0 + 1_000);
        assert!(
            matches!(&effects[..], [Effect::DeepSearch { .. }]),
            "{effects:?}"
        );
        app.apply(Msg::DeepLimited, T0 + 2_000);
        assert_eq!(app.loading, 0);
        assert_eq!(app.flash_text(T0 + 2_000), Some(DEEP_LIMIT_TEXT));
        assert_eq!(app.updated_at, Some(T0), "갱신 시각을 바꾸지 않는다");
        assert_eq!(ids(&app), before, "로컬 결과를 그대로 둔다");
    }
```

`src/tui/view.rs`에서 `status_shows_offline_and_rate_limit` 테스트 전체를 찾아 아래로 바꾼다.

```rust
    #[test]
    fn rate_limit_shows_briefly_and_status_keeps_refresh_time() {
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
        assert_eq!(status_span(&a, T0).content, "방금 갱신");
        let (rows, _, _) = screen(&a, 80, 12);
        assert_eq!(
            rows[11],
            " Linear API 한도를 넘었어요. 2분 후 다시 시도하세요"
        );
    }
```

`src/tui/runtime.rs`의 `deep_search_limit_explains_without_pausing`에서 아래를 찾아

```rust
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
```

아래로 바꾼다.

```rust
        assert_eq!(
            settle(&mut fx.rt, T0),
            vec![Msg::DeepLimited],
            "50분 뒤 리셋 시각으로 자동 검색을 멈추지 않는다"
        );
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib tui::`
Expected: 컴파일 실패 (`Msg::DeepLimited`, `DEEP_LIMIT_TEXT`가 app에 없음)

- [ ] **Step 3: 구현**

`src/tui/app.rs`:

찾아 바꾼다:
```rust
/// 하단 안내 문구를 보여주는 시간.
pub const FLASH_MS: i64 = 3_000;
```
→
```rust
/// 하단 안내 문구를 보여주는 시간.
pub const FLASH_MS: i64 = 3_000;
/// 깊은 검색이 분당 한도에 걸렸을 때 안내.
pub const DEEP_LIMIT_TEXT: &str = "깊은 검색은 분당 30회까지예요. 잠시 뒤 다시 시도하세요";
```

찾아 바꾼다:
```rust
    /// 남은 요청이 적다. 이 시각(리셋)까지 자동 서버 검색을 멈춘다
    Throttled(i64),
    Flash(String),
}
```
→
```rust
    /// 남은 요청이 적다. 이 시각(리셋)까지 자동 서버 검색을 멈춘다
    Throttled(i64),
    /// 깊은 검색이 분당 한도에 걸렸다. 세어 둔 요청 하나를 끝내고 안내만 한다
    DeepLimited,
    Flash(String),
}
```

찾아 바꾼다:
```rust
/// 상단에 보이는 문제 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    Offline(String),
    RateLimited(Option<i64>),
    Error(String),
}
```
→
```rust
/// 상단에 보이는 문제 상태. 한도 초과는 하단에 잠깐 알리기만 해서 여기 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    Offline(String),
    Error(String),
}
```

찾아 바꾼다:
```rust
    fn done_loading(&mut self) {
        self.loading = self.loading.saturating_sub(1);
    }
```
→
```rust
    fn done_loading(&mut self) {
        self.loading = self.loading.saturating_sub(1);
    }

    /// `until`까지 자동 서버 검색을 멈춘다. 이미 더 늦게까지 멈춰 있으면 그대로 둔다.
    fn pause_until(&mut self, until: i64) {
        self.paused_until = Some(self.paused_until.map_or(until, |p| p.max(until)));
    }
```

찾아 바꾼다:
```rust
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
```
→
```rust
                self.tabs[Tab::All.index()].loading_more = false;
                match e {
                    ApiError::RateLimited { reset_at_ms } => {
                        // 서버는 응답했으니 오프라인·오류 표시는 지운다. 한도는 잠깐 알리기만 한다
                        self.problem = None;
                        let text = match reset_at_ms {
                            Some(reset) => {
                                self.pause_until(reset);
                                format!(
                                    "Linear API 한도를 넘었어요. {}분 후 다시 시도하세요",
                                    minutes_left(reset, now)
                                )
                            }
                            None => "Linear API 한도를 넘었어요. 잠시 뒤 다시 시도하세요".into(),
                        };
                        self.set_flash(text, now);
                    }
                    ApiError::Offline(m) => self.problem = Some(Problem::Offline(m)),
                    other => self.problem = Some(Problem::Error(other.to_string())),
                }
            }
            Msg::Throttled(until) => {
                self.pause_until(until);
                self.set_flash(
                    format!(
                        "API 한도가 얼마 남지 않아 {}분 동안 자동 서버 검색을 멈춰요",
                        minutes_left(until, now)
                    ),
                    now,
                );
            }
            Msg::DeepLimited => {
                self.done_loading();
                self.set_flash(DEEP_LIMIT_TEXT, now);
            }
```

`#[cfg(test)]` 바로 위(`impl App`이 끝난 뒤)에 넣는다:
```rust
/// `until`까지 남은 분 (올림).
fn minutes_left(until: i64, now: i64) -> i64 {
    ((until - now).max(0) + 59_999) / 60_000
}
```

`src/tui/view.rs`:

찾아 바꾼다:
```rust
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
```
→
```rust
/// 상단 오른쪽 상태: 갱신 중 / n분 전 갱신 / 오프라인 / 오류. 한도 초과는 하단에 잠깐 알린다.
pub fn status_span(app: &App, now: i64) -> Span<'static> {
    if app.loading > 0 {
        return Span::styled("갱신 중…", DIM);
    }
    match &app.problem {
        Some(Problem::Offline(_)) => Span::styled("오프라인", WARN),
        Some(Problem::Error(_)) => Span::styled("오류", ERROR),
```

찾아 바꾼다:
```rust
            Some(Problem::RateLimited(_)) => {
                Span::styled(" 한도를 넘어서 저장된 결과만 보여줘요", ERROR)
            }
            Some(Problem::Error(m)) =>
```
→
```rust
            Some(Problem::Error(m)) =>
```

`src/tui/runtime.rs`:

찾아 바꾼다(상수 삭제):
```rust
/// "최근 본" 개수.
pub const RECENT_LIMIT: usize = 50;
/// 깊은 검색이 분당 한도에 걸렸을 때 안내.
pub const DEEP_LIMIT_TEXT: &str = "깊은 검색은 분당 30회까지예요. 잠시 뒤 다시 시도하세요";
```
→
```rust
/// "최근 본" 개수.
pub const RECENT_LIMIT: usize = 50;
```

찾아 바꾼다:
```rust
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
```
→
```rust
            Done::Search {
                deep: true,
                result: Err(ApiError::RateLimited { .. }),
                ..
            } => {
                // 깊은 검색에는 분당 30회 한도가 따로 있다. 응답의 리셋 시각은 시간당 요청 한도
                // 기준이라 맞지 않으니, 자동 검색은 멈추지 않고 로컬 결과를 둔 채 안내만 한다.
                // 서버 결과를 받은 게 아니라서 갱신 시각도 바꾸지 않는다
                self.log.write("깊은 검색: 한도 초과");
                vec![Msg::DeepLimited]
            }
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: 모두 통과

- [ ] **Step 5: 커밋**

```bash
git add src/tui/app.rs src/tui/view.rs src/tui/runtime.rs
git commit -m "fix(tui): 한도 초과는 하단에 잠깐 알리기 — 상단 고정 표시를 없애고, 깊은 검색 한도는 갱신 시각을 바꾸지 않고, 앞서 정한 자동 검색 멈춤을 지우지 않는다"
```

---

### Task 2: 상세에서도 `q`는 닫기 (N3)

**Files:**
- Modify: `src/tui/keys.rs` (`q` → `Act::Quit`, 테스트)
- Modify: `src/tui/view.rs` (상세 하단 안내, 테스트)
- Modify: `src/tui/app.rs` (테스트)

- [ ] **Step 1: 테스트를 바꾼다**

`src/tui/keys.rs`의 `list_mode_letters_are_actions_and_jamo_maps`에서 찾아 바꾼다:
```rust
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('ㅂ'))),
            Some(Input::Esc)
        );
```
→
```rust
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('ㅂ'))),
            Some(Input::Act(Act::Quit))
        );
```

`src/tui/keys.rs` 테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn q_closes_in_list_and_detail_but_types_in_search() {
        for mode in [Mode::List, Mode::Detail] {
            assert_eq!(
                translate(mode, false, key(KeyCode::Char('q'))),
                Some(Input::Act(Act::Quit)),
                "{mode:?}"
            );
        }
        assert_eq!(
            translate(Mode::Search, false, key(KeyCode::Char('q'))),
            Some(Input::Char('q'))
        );
    }
```

`src/tui/app.rs` 테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn quit_from_a_detail_opened_in_search_mode_closes() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        assert_eq!(app.mode, Mode::Detail);
        app.handle(Input::Act(Act::Quit), T0);
        assert!(app.quit);
        assert_eq!(app.query, "", "검색어에 들어가지 않는다");
    }
```

`src/tui/view.rs`의 `detail_shows_body_comments_and_max_scroll`에서 찾아 바꾼다:
```rust
        assert!(drawn.detail_max_scroll.unwrap() > 0);
    }
```
→
```rust
        assert!(drawn.detail_max_scroll.unwrap() > 0);
        assert!(rows[11].ends_with("Esc 뒤로  q 닫기"), "{}", rows[11]);
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib tui::`
Expected: FAIL — `q`가 `Input::Esc`, 하단 안내에 `q 닫기` 없음

- [ ] **Step 3: 구현**

`src/tui/keys.rs` 찾아 바꾼다:
```rust
        (_, 'q') => Some(Input::Esc),
```
→
```rust
        (_, 'q') => act(Act::Quit),
```

`src/tui/view.rs` 찾아 바꾼다:
```rust
        Mode::Detail => " j/k 스크롤  u 링크  y URL 복사  Y PR 링크  ^K 메뉴  Esc 뒤로",
```
→
```rust
        Mode::Detail => " j/k 스크롤  u 링크  y URL 복사  Y PR 링크  ^K 메뉴  Esc 뒤로  q 닫기",
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 5: 커밋**

```bash
git add src/tui/keys.rs src/tui/view.rs src/tui/app.rs
git commit -m "fix(tui): 상세에서도 q는 닫기, Esc만 뒤로 — 검색 모드에서 연 상세의 두 번째 q가 검색어로 들어가던 문제"
```

---

### Task 3: 목록 스크롤 위치를 이어 쓰기 (N11)

**Files:**
- Modify: `src/tui/view.rs` (`Drawn::list_offset`, `draw`, `draw_list`, 테스트)
- Modify: `src/tui/app.rs` (`App::list_offset`)
- Modify: `src/tui/runtime.rs` (`event_loop`)

**Interfaces:**
- Produces: `pub list_offset: Option<usize>` (Drawn), `pub list_offset: usize` (App). 런타임은 그린 뒤 `Drawn::list_offset`을 `App::list_offset`에 넣는다.

- [ ] **Step 1: 테스트**

`src/tui/view.rs` 테스트 모듈 끝에 넣는다:
```rust
    fn numbered(n: usize) -> Vec<Issue> {
        (1..=n)
            .map(|k| {
                IssueBuilder::new(&format!("i{k}"), &format!("ENG-{k}"), &format!("이슈 {k}"))
                    .build()
            })
            .collect()
    }

    /// 이슈 20개. 80×12 화면이면 목록은 셋째 줄부터 9줄이다.
    fn twenty() -> App {
        let (mut a, _) = App::start(None);
        a.apply(
            Msg::Tab {
                tab: Tab::Mine,
                issues: numbered(20),
                fresh: true,
                has_more: false,
                append: false,
            },
            T0,
        );
        a
    }

    /// 한 프레임을 그리고 스크롤 위치를 넘겨준 뒤, 선택 표시(▶)가 있는 화면 줄을 돌려준다.
    fn frame(a: &mut App) -> usize {
        let (rows, drawn, _) = screen(a, 80, 12);
        if let Some(offset) = drawn.list_offset {
            a.list_offset = offset;
        }
        rows.iter().position(|r| r.starts_with('▶')).expect("선택 줄")
    }

    #[test]
    fn list_keeps_its_scroll_position_when_moving_back_up() {
        let mut a = twenty();
        for _ in 0..12 {
            a.handle(Input::Down, T0);
            frame(&mut a);
        }
        assert_eq!(frame(&mut a), 10, "맨 아래 줄");
        a.handle(Input::Up, T0);
        assert_eq!(frame(&mut a), 9, "한 줄 올라간다 (아래에 붙지 않는다)");
    }

    #[test]
    fn shorter_list_after_scrolling_starts_from_the_top() {
        let mut a = twenty();
        a.list_offset = 10;
        a.selected = 15;
        a.apply(
            Msg::Tab {
                tab: Tab::Mine,
                issues: numbered(3),
                fresh: true,
                has_more: false,
                append: false,
            },
            T0,
        );
        let (rows, drawn, _) = screen(&a, 80, 12);
        assert_eq!(drawn.list_offset, Some(0));
        assert!(rows[2].contains("ENG-1"), "{rows:?}");
    }
```

- [ ] **Step 2: 실패 확인** — `cargo test --lib tui::view` → 컴파일 실패(`list_offset` 없음)

- [ ] **Step 3: 구현**

`src/tui/view.rs` 찾아 바꾼다:
```rust
    /// 누를 수 있는 곳. 나중에 그린 것(메뉴)이 뒤에 온다
    pub hits: Vec<Hit>,
}
```
→
```rust
    /// 누를 수 있는 곳. 나중에 그린 것(메뉴)이 뒤에 온다
    pub hits: Vec<Hit>,
    /// 목록을 그렸으면 그 스크롤 위치 (다음 프레임에 이어 쓴다)
    pub list_offset: Option<usize>,
}
```

찾아 바꾼다:
```rust
            drawn.hits.extend(draw_list(f, app, list));
            draw_preview(f, app, preview);
        } else {
            drawn.hits.extend(draw_list(f, app, body));
        }
```
→
```rust
            draw_list(f, app, list, &mut drawn);
            draw_preview(f, app, preview);
        } else {
            draw_list(f, app, body, &mut drawn);
        }
```

찾아 바꾼다:
```rust
/// 목록 줄의 위치를 함께 돌려준다 (마우스로 선택·열기).
fn draw_list(f: &mut Frame, app: &App, area: Rect) -> Vec<Hit> {
    if app.rows.is_empty() {
        let text = if app.loading > 0 {
            "불러오는 중…"
        } else {
            "결과가 없어요"
        };
        f.render_widget(Paragraph::new(Span::styled(format!("  {text}"), DIM)), area);
        return Vec::new();
    }
```
→
```rust
/// 목록을 그리고, 줄의 위치(마우스로 선택·열기)와 스크롤 위치를 `drawn`에 남긴다.
fn draw_list(f: &mut Frame, app: &App, area: Rect, drawn: &mut Drawn) {
    if app.rows.is_empty() {
        let text = if app.loading > 0 {
            "불러오는 중…"
        } else {
            "결과가 없어요"
        };
        f.render_widget(Paragraph::new(Span::styled(format!("  {text}"), DIM)), area);
        return;
    }
```

찾아 바꾼다:
```rust
    let mut state = ListState::default().with_selected(Some(selected_item));
    f.render_stateful_widget(
        List::new(items).highlight_style(SELECTED_BG),
        area,
        &mut state,
    );
    (0..area.height)
        .filter_map(|k| {
            let row = (*item_rows.get(state.offset() + usize::from(k))?)?;
            Some(Hit {
                area: Rect::new(area.x, area.y + k, area.width, 1),
                target: Target::Row(row),
            })
        })
        .collect()
}
```
→
```rust
    // 항목은 모두 한 줄이라, 칸을 다 채울 수 있는 만큼만 이전 스크롤 위치를 이어 쓴다
    let max_offset = items.len().saturating_sub(usize::from(area.height));
    let mut state = ListState::default()
        .with_offset(app.list_offset.min(max_offset))
        .with_selected(Some(selected_item));
    f.render_stateful_widget(
        List::new(items).highlight_style(SELECTED_BG),
        area,
        &mut state,
    );
    drawn.list_offset = Some(state.offset());
    drawn.hits.extend((0..area.height).filter_map(|k| {
        let row = (*item_rows.get(state.offset() + usize::from(k))?)?;
        Some(Hit {
            area: Rect::new(area.x, area.y + k, area.width, 1),
            target: Target::Row(row),
        })
    }));
}
```

`src/tui/app.rs` 찾아 바꾼다:
```rust
    pub rows: Vec<Row>,
    pub selected: usize,
    pub detail: Option<Detail>,
```
→
```rust
    pub rows: Vec<Row>,
    pub selected: usize,
    /// 목록의 스크롤 위치. 런타임이 그린 결과(`Drawn::list_offset`)를 넣어 준다
    pub list_offset: usize,
    pub detail: Option<Detail>,
```

찾아 바꾼다:
```rust
            rows: Vec::new(),
            selected: 0,
            detail: None,
```
→
```rust
            rows: Vec::new(),
            selected: 0,
            list_offset: 0,
            detail: None,
```

`src/tui/runtime.rs` 찾아 바꾼다:
```rust
            if let Some(max) = d.detail_max_scroll {
                app.set_detail_max_scroll(max);
            }
            last = d;
```
→
```rust
            if let Some(max) = d.detail_max_scroll {
                app.set_detail_max_scroll(max);
            }
            if let Some(offset) = d.list_offset {
                app.list_offset = offset;
            }
            last = d;
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 5: 커밋**

```bash
git add src/tui/view.rs src/tui/app.rs src/tui/runtime.rs
git commit -m "fix(tui): 목록 스크롤 위치를 프레임마다 이어 쓰기 — 위로 올릴 때 선택 줄이 화면 아래에 붙던 문제"
```

---

### Task 4: 코멘트 링크 번호를 `u` 목록과 맞추기 (N4)

**Files:**
- Modify: `src/markdown.rs` (`render_numbered`, `Renderer::first_link`, 테스트)
- Modify: `src/tui/view.rs` (`detail_lines` 분리, 테스트)
- Modify: `src/tui/app.rs` (`open_links`)

**Interfaces:**
- Produces: `markdown::render_numbered(md: &str, width: u16, theme: &Theme, team_keys: &[String], first: usize) -> Rendered`, `view::detail_lines(d: &Detail, width: u16, keys: &[String]) -> Vec<Line<'static>>`(비공개).
- 테스트 도우미(view.rs): `comment(id, body) -> Comment`, `detail_of(issue, comments) -> App`, `link_menu(&mut App) -> Vec<String>` — Task 5도 `detail_of`를 쓴다.

- [ ] **Step 1: 테스트**

`src/markdown.rs` 테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn link_numbers_can_start_later() {
        let r = render_numbered(
            "[a](https://a.dev) ![](https://b.dev/x.png)",
            40,
            &Theme::default(),
            &[],
            3,
        );
        let numbers: Vec<usize> = r.links.iter().map(|l| l.index).collect();
        assert_eq!(numbers, vec![3, 4]);
        let text = to_plain(&r.lines);
        assert!(
            text.contains("a [3]") && text.contains("[이미지 4: x.png]"),
            "{text}"
        );
    }
```

`src/tui/view.rs` 테스트 모듈 끝에 넣는다:
```rust
    fn comment(id: &str, body: &str) -> crate::linear::types::Comment {
        serde_json::from_value(serde_json::json!({
            "id": id, "body": body, "createdAt": "2026-10-02T00:00:00.000Z",
            "editedAt": null, "user": null
        }))
        .unwrap()
    }

    /// 첫 이슈(id "a")의 상세를 열고 서버 응답을 반영한다.
    fn detail_of(issue: Issue, comments: Vec<crate::linear::types::Comment>) -> App {
        let mut a = app();
        a.handle(Input::Enter, T0);
        a.apply(
            Msg::Detail {
                id: "a".into(),
                issue,
                comments,
                more: false,
                fresh: true,
            },
            T0,
        );
        a
    }

    fn link_menu(a: &mut App) -> Vec<String> {
        a.handle(Input::Act(Act::Links), T0);
        a.menu
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|(l, _)| l.clone())
            .collect()
    }

    #[test]
    fn comment_link_numbers_continue_from_the_body() {
        let issue = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .description("[문서](https://x.dev/doc)")
            .build();
        let mut a = detail_of(issue, vec![comment("c1", "[로그](https://x.dev/log)")]);
        let text = markdown::to_plain(&detail_lines(a.detail.as_ref().unwrap(), 80, &[]));
        assert!(
            text.contains("문서 [1]") && text.contains("로그 [2]"),
            "{text}"
        );
        assert_eq!(
            link_menu(&mut a),
            vec![
                "[1] 문서 — https://x.dev/doc",
                "[2] 로그 — https://x.dev/log"
            ]
        );
    }

    #[test]
    fn links_in_comments_only_start_at_one() {
        let issue = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .description("  \n")
            .build();
        let mut a = detail_of(
            issue,
            vec![
                comment("c1", "링크 없음"),
                comment("c2", "![](https://x.dev/a.png)"),
            ],
        );
        let text = markdown::to_plain(&detail_lines(a.detail.as_ref().unwrap(), 80, &[]));
        assert!(text.contains("[이미지 1: a.png]"), "{text}");
        assert_eq!(link_menu(&mut a), vec!["[1] a.png — https://x.dev/a.png"]);
    }
```

- [ ] **Step 2: 실패 확인** — 컴파일 실패(`render_numbered`, `detail_lines` 없음)

- [ ] **Step 3: 구현**

`src/markdown.rs` 찾아 바꾼다:
```rust
/// `render`와 같되, `team_keys`가 있으면 그 팀 키의 식별자만 강조한다 (`UTF-8` 같은 오탐 방지).
pub fn render_with(md: &str, width: u16, theme: &Theme, team_keys: &[String]) -> Rendered {
    let md = sanitize(&md.nfc().collect::<String>());
    let mut r = Renderer::new(usize::from(width).max(10), *theme);
    r.team_keys = team_keys.iter().map(|k| k.to_uppercase()).collect();
```
→
```rust
/// `render`와 같되, `team_keys`가 있으면 그 팀 키의 식별자만 강조한다 (`UTF-8` 같은 오탐 방지).
pub fn render_with(md: &str, width: u16, theme: &Theme, team_keys: &[String]) -> Rendered {
    render_numbered(md, width, theme, team_keys, 1)
}

/// `render_with`와 같되 링크·이미지 번호를 `first`부터 매긴다.
/// 상세 화면처럼 본문과 코멘트를 따로 그려도 번호를 이어 가게 할 때 쓴다.
pub fn render_numbered(
    md: &str,
    width: u16,
    theme: &Theme,
    team_keys: &[String],
    first: usize,
) -> Rendered {
    let md = sanitize(&md.nfc().collect::<String>());
    let mut r = Renderer::new(usize::from(width).max(10), *theme);
    r.team_keys = team_keys.iter().map(|k| k.to_uppercase()).collect();
    r.first_link = first.max(1);
```

찾아 바꾼다:
```rust
    /// 강조할 이슈 식별자의 팀 키 (비어 있으면 모두)
    team_keys: Vec<String>,
}
```
→
```rust
    /// 강조할 이슈 식별자의 팀 키 (비어 있으면 모두)
    team_keys: Vec<String>,
    /// 첫 링크·이미지 번호 (본문과 코멘트를 이어 매길 때 1보다 크다)
    first_link: usize,
}
```

찾아 바꾼다:
```rust
            need_blank: false,
            team_keys: Vec::new(),
        }
```
→
```rust
            need_blank: false,
            team_keys: Vec::new(),
            first_link: 1,
        }
```

모두 바꾼다(`add_link`, `add_image` 두 곳, replace_all):
```rust
        let index = self.links.len() + 1;
```
→
```rust
        let index = self.first_link + self.links.len();
```

`src/tui/view.rs` 찾아 바꾼다:
```rust
use super::app::{App, Mode, Problem, Row, Tab};
```
→
```rust
use super::app::{App, Detail, Mode, Problem, Row, Tab};
```

`draw_detail` 전체(`/// 상세 화면을 그리고 최대 스크롤을 돌려준다.`부터 그 함수의 끝 `}`까지)를 아래로 바꾼다:
```rust
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
    let lines = detail_lines(d, area.width, &app.team_keys());
    let total = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let max = total.saturating_sub(area.height);
    let scroll = d.scroll.min(max);
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
    max
}

/// 상세 화면의 줄. 링크·이미지 번호는 본문에서 코멘트로 이어 매긴다 (`u` 목록과 같은 번호).
fn detail_lines(d: &Detail, width: u16, keys: &[String]) -> Vec<Line<'static>> {
    let theme = Theme::default();
    let mut lines: Vec<Line<'static>> = Vec::new();
    match &d.issue {
        None => {
            let id = sanitize(&d.id);
            let (text, style) = if d.gone {
                (format!("{id}: 찾을 수 없거나 보관·삭제된 이슈예요"), WARN)
            } else if d.loading {
                (format!("{id} 불러오는 중…"), DIM)
            } else {
                (
                    format!("{id}: 불러오지 못했어요. r로 다시 시도하세요"),
                    WARN,
                )
            };
            lines.push(Line::from(Span::styled(text, style)));
        }
        Some(issue) => {
            lines.extend(issue_header(issue, width));
            lines.push(Line::from(Span::styled(issue.url.clone(), DIM)));
            if d.gone {
                lines.push(Line::from(Span::styled(
                    "보관되었거나 삭제된 이슈예요",
                    WARN,
                )));
            }
            lines.push(Line::default());
            let mut next_link = 1;
            match issue.description.as_deref().map(str::trim) {
                Some(body) if !body.is_empty() => {
                    let r = markdown::render_numbered(body, width, &theme, keys, next_link);
                    next_link += r.links.len();
                    lines.extend(r.lines);
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
                    let r = markdown::render_numbered(&c.body, width, &theme, keys, next_link);
                    next_link += r.links.len();
                    lines.extend(r.lines);
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
    lines
}
```

`src/tui/app.rs`에서 `open_links` 함수의 문서 주석부터 `self.menu = Some(Menu {` 앞까지를 찾아 바꾼다:
```rust
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
```
→
```rust
    /// 상세 화면 본문과 코멘트의 링크·이미지 목록. 번호는 상세 화면에 보이는 번호와 같다.
    fn open_links(&mut self) {
        let Some(d) = self.detail.as_ref() else {
            return;
        };
        // 상세 화면처럼 공백뿐인 본문은 빼고, 본문 → 코멘트 순서로 번호를 이어 매긴다
        let body = d
            .issue
            .as_ref()
            .and_then(|i| i.description.as_deref())
            .map(str::trim)
            .filter(|b| !b.is_empty());
        let theme = Theme::default();
        let mut items = Vec::new();
        for md in body
            .into_iter()
            .chain(d.comments.iter().map(|c| c.body.as_str()))
        {
            for l in markdown::render_numbered(md, 80, &theme, &[], items.len() + 1).links {
                items.push((
                    format!("[{}] {} — {}", l.index, l.label, l.url),
                    Act::OpenUrl(l.url),
                ));
            }
        }
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 5: 커밋**

```bash
git add src/markdown.rs src/tui/view.rs src/tui/app.rs
git commit -m "fix(tui): 코멘트 링크 번호를 본문에서 이어 매겨 u 목록 번호와 맞추기"
```

---

### Task 5: 빠진 sanitize 채우기 (N12)

**Files:**
- Modify: `src/ui/row.rs` (식별자), `src/tui/view.rs` (`issue_header`의 식별자·상위, `detail_lines`의 URL), `src/markdown.rs` (`to_ansi`)

- [ ] **Step 1: 테스트**

`src/ui/row.rs`의 `row_strips_control_characters` 전체를 찾아 아래로 바꾼다:
```rust
    #[test]
    fn row_strips_control_characters() {
        let i = IssueBuilder::new("i1", "UP-1", "제목\u{1b}[2J").build();
        let text = to_plain(&[issue_row(&i, 80)]);
        assert!(!text.contains('\u{1b}'), "{text:?}");
        let mut v = IssueBuilder::new("i2", "UP-2", "제목").json();
        v["identifier"] = serde_json::json!("UP-2\u{1b}]0;x\u{7}");
        let i: Issue = serde_json::from_value(v).unwrap();
        let text = to_plain(&[issue_row(&i, 80)]);
        assert!(!text.chars().any(char::is_control), "{text:?}");
    }
```

`src/tui/view.rs` 테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn ids_and_url_are_stripped_of_control_characters() {
        let mut v = IssueBuilder::new("a", "ENG-1", "로그인 버그").json();
        v["identifier"] = serde_json::json!("ENG-1\u{1b}[2J");
        v["url"] = serde_json::json!("https://linear.app/x\u{1b}]52;c;eA==\u{7}");
        v["parent"] =
            serde_json::json!({ "id": "p", "identifier": "ENG-0\u{1b}[5m", "title": "상위" });
        let issue: Issue = serde_json::from_value(v).unwrap();
        let clean = |lines: &[Line<'_>]| {
            lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .all(|s| !s.content.chars().any(char::is_control))
        };
        assert!(clean(&issue_header(&issue, 80)));
        let a = detail_of(issue, Vec::new());
        assert!(clean(&detail_lines(a.detail.as_ref().unwrap(), 80, &[])));
    }
```

`src/markdown.rs` 테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn ansi_output_never_carries_control_characters_from_content() {
        let line = Line::from(Span::styled(
            "a\u{1b}]52;c;eA==\u{7}b",
            Style::new().fg(Color::Red),
        ));
        let out = to_ansi(&[line]);
        assert!(!out.contains("\u{1b}]") && !out.contains('\u{7}'), "{out:?}");
        assert!(out.contains("a]52;c;eA==b"), "{out:?}");
    }
```

- [ ] **Step 2: 실패 확인** — 세 테스트 FAIL

- [ ] **Step 3: 구현**

`src/ui/row.rs` 찾아 바꾼다:
```rust
    let id = Span::styled(format!("{:<9} ", issue.identifier), DIM);
```
→
```rust
    let id = Span::styled(format!("{:<9} ", sanitize(&issue.identifier)), DIM);
```

`src/tui/view.rs` 찾아 바꾼다:
```rust
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
```
→
```rust
pub fn issue_header(issue: &Issue, width: u16) -> Vec<Line<'static>> {
    let ident = sanitize(&issue.identifier);
    let mut lines = markdown::wrap_text(
        &format!("{ident}  {}", issue.title),
        width,
        Style::new().add_modifier(Modifier::BOLD),
    );
    if let Some(first) = lines.first_mut()
        && let Some(span) = first.spans.first_mut()
        && span.content.starts_with(ident.as_str())
    {
        // 식별자만 강조색으로
        let rest = span.content[ident.len()..].to_string();
        let id = Span::styled(ident.clone(), ACCENT);
```

찾아 바꾼다:
```rust
        info.push(format!("상위 {}", p.identifier));
```
→
```rust
        info.push(format!("상위 {}", sanitize(&p.identifier)));
```

찾아 바꾼다:
```rust
            lines.push(Line::from(Span::styled(issue.url.clone(), DIM)));
```
→
```rust
            lines.push(Line::from(Span::styled(sanitize(&issue.url), DIM)));
```

`src/markdown.rs` 찾아 바꾼다:
```rust
            out.push_str(&StyledContent::new(cs, span.content.as_ref()).to_string());
```
→
```rust
            // 색(ESC [ … m)은 여기서 붙인다. 내용에는 제어 문자가 남지 않게 한 번 더 거른다
            out.push_str(&StyledContent::new(cs, sanitize(&span.content)).to_string());
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 5: 커밋**

```bash
git add src/ui/row.rs src/tui/view.rs src/markdown.rs
git commit -m "fix: 식별자·상위 이슈·URL과 CLI 색 출력에서도 제어 문자를 거르기"
```

---

### Task 6: 오류 문구가 겹치지 않게 (N13)

**Files:**
- Modify: `src/linear/client.rs` (`ApiError::GraphQl` 문구), `src/tui/view.rs` (테스트)

- [ ] **Step 1: 테스트** — `src/tui/view.rs`의 `server_text_cannot_inject_terminal_codes`에서 찾아 바꾼다:
```rust
        assert!(rows.last().unwrap().contains("오류: 나쁜"), "{rows:?}");
```
→
```rust
        let footer = rows.last().unwrap();
        assert!(
            footer.contains("오류: Linear가 요청을 처리하지 못했어요: 나쁜"),
            "{rows:?}"
        );
        assert!(!footer.contains("오류: Linear 오류"), "{footer}");
```

- [ ] **Step 2: 실패 확인** — FAIL (지금은 "오류: Linear 오류: 나쁜…")

- [ ] **Step 3: 구현** — `src/linear/client.rs` 찾아 바꾼다:
```rust
    #[error("Linear 오류: {0}")]
    GraphQl(String),
```
→
```rust
    #[error("Linear가 요청을 처리하지 못했어요: {0}")]
    GraphQl(String),
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 5: 커밋**

```bash
git add src/linear/client.rs src/tui/view.rs
git commit -m "fix: 오류 문구가 '오류: Linear 오류: …'로 겹치지 않게"
```

---

### Task 7: 설정·범위 경고가 가려지지 않게 (N8)

**Files:**
- Modify: `src/tui/app.rs` (`WARN_MS`, `App::warning`, `warning_text`, `Msg::Warn`, 테스트)
- Modify: `src/tui/view.rs` (키 입력 화면 맨 아래 줄, 하단 줄 우선순위, 테스트)
- Modify: `src/tui/mod.rs` (설정 경고 → `Msg::Warn`), `src/tui/runtime.rs` (범위 경고 → `Msg::Warn`)

**Interfaces:**
- Produces: `pub const WARN_MS: i64 = 10_000`, `Msg::Warn(String)`, `App::warning_text(&self, now) -> Option<&str>`.
- 하단 줄 우선순위: 안내(flash) → 경고 → 문제(오프라인·오류) → 키 안내.

- [ ] **Step 1: 테스트**

`src/tui/app.rs` 테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn warnings_join_and_outlast_flashes() {
        let mut app = App::onboarding(false);
        app.apply(Msg::Warn("설정 경고: A".into()), T0);
        app.apply(Msg::Warn("경고: B".into()), T0 + 1_000);
        app.apply(Msg::Flash("복사됨: ENG-1".into()), T0 + 1_000);
        assert_eq!(
            app.warning_text(T0 + 1_000 + FLASH_MS),
            Some("설정 경고: A · 경고: B")
        );
        assert_eq!(app.warning_text(T0 + 1_000 + WARN_MS), None);
    }
```

`src/tui/view.rs` 테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn warnings_show_on_the_key_screen_and_after_a_flash() {
        let mut onboarding = App::onboarding(false);
        onboarding.apply(Msg::Warn("설정 경고: teams는 문자열 배열이어야 해요".into()), T0);
        let (rows, _, _) = screen(&onboarding, 80, 14);
        assert_eq!(rows[13], " 설정 경고: teams는 문자열 배열이어야 해요");
        let mut a = app();
        a.apply(Msg::Warn("경고: teams 확인".into()), T0 - 5_000);
        a.apply(Msg::Flash("복사됨: ENG-1".into()), T0 - 4_000);
        let (rows, _, _) = screen(&a, 80, 12);
        assert_eq!(rows[11], " 경고: teams 확인", "안내가 사라진 뒤에도 남는다");
    }

    #[test]
    fn tiny_key_screen_with_a_warning_does_not_panic() {
        let mut a = App::onboarding(false);
        a.apply(Msg::Warn("설정 경고: 아주 긴 경고 문구".into()), T0);
        for (w, h) in [(1, 1), (5, 3), (12, 4), (20, 5)] {
            screen(&a, w, h);
        }
    }
```

- [ ] **Step 2: 실패 확인** — 컴파일 실패(`Msg::Warn`, `WARN_MS` 없음)

- [ ] **Step 3: 구현**

`src/tui/app.rs` 찾아 바꾼다:
```rust
/// 깊은 검색이 분당 한도에 걸렸을 때 안내.
pub const DEEP_LIMIT_TEXT: &str = "깊은 검색은 분당 30회까지예요. 잠시 뒤 다시 시도하세요";
```
→
```rust
/// 깊은 검색이 분당 한도에 걸렸을 때 안내.
pub const DEEP_LIMIT_TEXT: &str = "깊은 검색은 분당 30회까지예요. 잠시 뒤 다시 시도하세요";
/// 설정·범위 경고를 보여주는 시간. 일반 안내보다 길다.
pub const WARN_MS: i64 = 10_000;
```

찾아 바꾼다:
```rust
    Flash(String),
}
```
→
```rust
    Flash(String),
    /// 설정·범위 경고. 일반 안내보다 오래 보이고 그것에 덮이지 않는다
    Warn(String),
}
```

찾아 바꾼다:
```rust
    /// (문구, 보여줄 마지막 시각)
    pub flash: Option<(String, i64)>,
```
→
```rust
    /// (문구, 보여줄 마지막 시각)
    pub flash: Option<(String, i64)>,
    /// (경고 문구, 보여줄 마지막 시각)
    pub warning: Option<(String, i64)>,
```

찾아 바꾼다:
```rust
            flash: None,
            quit: false,
```
→
```rust
            flash: None,
            warning: None,
            quit: false,
```

찾아 바꾼다:
```rust
    pub fn flash_text(&self, now: i64) -> Option<&str> {
        self.flash
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(t, _)| t.as_str())
    }
```
→
```rust
    pub fn flash_text(&self, now: i64) -> Option<&str> {
        self.flash
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(t, _)| t.as_str())
    }

    pub fn warning_text(&self, now: i64) -> Option<&str> {
        self.warning
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(t, _)| t.as_str())
    }
```

찾아 바꾼다:
```rust
            Msg::Flash(text) => self.set_flash(text, now),
        }
```
→
```rust
            Msg::Flash(text) => self.set_flash(text, now),
            Msg::Warn(text) => {
                // 아직 보이는 경고가 있으면 이어 붙인다 (설정 경고 뒤에 범위 경고가 오는 경우)
                let text = match self.warning_text(now) {
                    Some(prev) => format!("{prev} · {text}"),
                    None => text,
                };
                self.warning = Some((text, now + WARN_MS));
            }
        }
```

`src/tui/view.rs` 찾아 바꾼다:
```rust
    if app.mode == Mode::Onboarding {
        draw_onboarding(f, app, area);
        return Drawn::default();
    }
```
→
```rust
    if app.mode == Mode::Onboarding {
        draw_onboarding(f, app, area);
        // 키 입력 화면에는 하단 줄이 없으니 경고만 맨 아래에 보인다
        if let Some(text) = app.warning_text(now) {
            let last = Rect {
                y: area.bottom().saturating_sub(1),
                height: area.height.min(1),
                ..area
            };
            let text = truncate(&format!(" {}", sanitize(text)), usize::from(area.width));
            f.render_widget(Paragraph::new(Span::styled(text, WARN)), last);
        }
        return Drawn::default();
    }
```

찾아 바꾼다:
```rust
            Style::new().fg(Color::Green),
        )
    } else {
        match &app.problem {
```
→
```rust
            Style::new().fg(Color::Green),
        )
    } else if let Some(text) = app.warning_text(now) {
        Span::styled(format!(" {}", sanitize(text)), WARN)
    } else {
        match &app.problem {
```

`src/tui/mod.rs` 찾아 바꾼다:
```rust
            Msg::Flash(format!("설정 경고: {}", warnings.join(" · "))),
```
→
```rust
            Msg::Warn(format!("설정 경고: {}", warnings.join(" · "))),
```

`src/tui/runtime.rs` 찾아 바꾼다:
```rust
        msgs.extend(warning.map(Msg::Flash));
```
→
```rust
        msgs.extend(warning.map(Msg::Warn));
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 5: 커밋**

```bash
git add src/tui/app.rs src/tui/view.rs src/tui/mod.rs src/tui/runtime.rs
git commit -m "fix(tui): 설정·범위 경고를 10초 동안 보이고 일반 안내에 덮이지 않게, 키 입력 화면에도 보이기"
```

---

### Task 8: 시작 전 오류와 팔레트 열기 실패를 로그에 남기기 (N9)

**Files:**
- Modify: `src/log.rs` (상태 디렉터리 만들기, `Logger::on_err`, 테스트)
- Modify: `src/tui/mod.rs` (`palette` → `prepare` 분리, 테스트)
- Modify: `src/herdr.rs` (`open_palette`에 `&Logger`), `src/cli.rs` (`open` 명령)

**Interfaces:**
- Produces: `Logger::on_err<T>(&self, what: &str, r: anyhow::Result<T>) -> anyhow::Result<T>`, `herdr::open_palette(herdr: &Herdr, origin: &Origin, log: &Logger) -> Result<()>`, `tui::prepare(paths: Paths, env_key: Option<String>) -> Result<(Runtime, App, Vec<Effect>)>`(비공개).

- [ ] **Step 1: 테스트**

`src/log.rs` 테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn creates_the_state_dir_and_logs_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("herdr-linear.log");
        let log = Logger::new(path.clone());
        let r: anyhow::Result<()> = Err(anyhow::anyhow!("키 파일을 읽지 못했어요"));
        assert!(log.on_err("팔레트 시작", r).is_err());
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.ends_with("팔레트 시작: 키 파일을 읽지 못했어요\n"),
            "{text:?}"
        );
    }
```

`src/tui/mod.rs` 파일 끝에 넣는다:
```rust
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
```

`src/herdr.rs` 테스트에서 찾아 바꾼다:
```rust
        open_palette(&herdr, &Origin::from_context(&ctx)).unwrap();
```
→
```rust
        let log = Logger::new(dir.path().join("herdr-linear.log"));
        open_palette(&herdr, &Origin::from_context(&ctx), &log).unwrap();
```

찾아 바꾼다:
```rust
        let err = open_palette(&herdr, &Origin::default()).unwrap_err();
        assert!(err.to_string().contains("다른 창이 떠 있어요"), "{err}");
```
→
```rust
        let log_path = dir.path().join("herdr-linear.log");
        let err = open_palette(&herdr, &Origin::default(), &Logger::new(log_path.clone()))
            .unwrap_err();
        assert!(err.to_string().contains("다른 창이 떠 있어요"), "{err}");
        let logged = std::fs::read_to_string(&log_path).unwrap();
        assert!(logged.contains("팔레트를 열지 못했어요"), "{logged}");
```

- [ ] **Step 2: 실패 확인** — 컴파일 실패(`on_err`, `prepare`, `open_palette` 인자)

- [ ] **Step 3: 구현**

`src/log.rs` 찾아 바꾼다:
```rust
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}
```
→
```rust
        let open = || OpenOptions::new().create(true).append(true).open(&self.path);
        // 처음 실행이면 상태 디렉터리가 아직 없을 수 있다 (캐시를 열기 전에 실패한 경우)
        let file = open().or_else(|e| match self.path.parent() {
            Some(dir) if e.kind() == std::io::ErrorKind::NotFound => {
                crate::config::ensure_private_dir(dir).map_err(std::io::Error::other)?;
                open()
            }
            _ => Err(e),
        });
        if let Ok(mut f) = file {
            let _ = f.write_all(line.as_bytes());
        }
    }

    /// 실패면 `what: 오류`로 한 줄 남기고 결과를 그대로 돌려준다.
    pub fn on_err<T>(&self, what: &str, r: anyhow::Result<T>) -> anyhow::Result<T> {
        r.inspect_err(|e| self.write(&format!("{what}: {e:#}")))
    }
}
```

`src/tui/mod.rs`에서 `use` 줄부터 테스트 모듈 앞까지(구현 전체)를 아래로 바꾼다:
```rust
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
```

`src/herdr.rs` 찾아 바꾼다:
```rust
use crate::config::PLUGIN_ID;
use crate::context::Origin;
```
→
```rust
use crate::config::PLUGIN_ID;
use crate::context::Origin;
use crate::log::Logger;
```

찾아 바꾼다:
```rust
/// `open palette`·`open url` 액션: 원래 pane의 맥락을 팔레트 pane에 넘겨 띄운다.
/// 실패하면 액션 출력은 보이지 않으니 herdr 알림으로도 알린다.
pub fn open_palette(herdr: &Herdr, origin: &Origin) -> Result<()> {
    herdr
        .open_pane("palette", &origin.to_env())
        .inspect_err(|e| {
            let _ = herdr.notify("Linear", &format!("팔레트를 열지 못했어요: {e:#}"));
        })
}
```
→
```rust
/// `open palette`·`open url` 액션: 원래 pane의 맥락을 팔레트 pane에 넘겨 띄운다.
/// 실패하면 액션 출력은 보이지 않으니 로그에 남기고 herdr 알림으로도 알린다.
pub fn open_palette(herdr: &Herdr, origin: &Origin, log: &Logger) -> Result<()> {
    herdr
        .open_pane("palette", &origin.to_env())
        .inspect_err(|e| {
            let text = format!("팔레트를 열지 못했어요: {e:#}");
            log.write(&text);
            let _ = herdr.notify("Linear", &text);
        })
}
```

`src/cli.rs` 찾아 바꾼다:
```rust
use crate::linear::types::{Comment, Issue, TeamRef, Viewer};
```
→
```rust
use crate::linear::types::{Comment, Issue, TeamRef, Viewer};
use crate::log::Logger;
```

찾아 바꾼다:
```rust
            open_palette(&Herdr::from_env(), &Origin::from_context(&ctx))?;
```
→
```rust
            open_palette(
                &Herdr::from_env(),
                &Origin::from_context(&ctx),
                &Logger::new(paths.log_file()),
            )?;
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 5: 커밋**

```bash
git add src/log.rs src/tui/mod.rs src/herdr.rs src/cli.rs
git commit -m "fix: 팔레트 시작 전 오류와 팔레트 열기 실패를 로그에 남기기"
```

---

### Task 9: `logout`이 다른 위치의 캐시도 지우기 (N5)

**Files:**
- Modify: `src/config.rs` (`cache_fallbacks`, `default_cache_fallbacks`, 테스트)
- Modify: `src/cli.rs` (`logout` 인자, `run`, 테스트)

**Interfaces:**
- Produces: `config::cache_fallbacks(home: &Path) -> Vec<PathBuf>`, `config::default_cache_fallbacks() -> Vec<PathBuf>`, `cli::logout(paths: &Paths, credentials: &[PathBuf], caches: &[PathBuf]) -> Result<String>`.

- [ ] **Step 1: 테스트**

`src/config.rs` 테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn cache_locations_cover_cli_and_plugin() {
        assert_eq!(
            cache_fallbacks(Path::new("/home/me")),
            vec![
                PathBuf::from("/home/me/.local/state/herdr-linear/cache.db"),
                PathBuf::from("/home/me/.local/state/herdr/plugins/jh.linear/cache.db"),
            ]
        );
    }
```

`src/cli.rs`의 `logout_removes_key_and_cache` 전체를 찾아 아래로 바꾼다:
```rust
    #[test]
    fn logout_removes_key_and_cache() {
        let (d, ctx) = test_ctx(OFFLINE.into());
        config::save_api_key(&ctx.paths, "lin_api_x").unwrap();
        Store::open(&ctx.paths.cache_db()).unwrap();
        // 다른 위치(CLI ↔ 플러그인)의 캐시
        let other = d.path().join("other").join("cache.db");
        Store::open(&other).unwrap();
        logout(&ctx.paths, &[], std::slice::from_ref(&other)).unwrap();
        assert!(!ctx.paths.credentials_file().exists());
        assert!(!ctx.paths.cache_db().exists());
        assert!(!other.exists());
    }
```

- [ ] **Step 2: 실패 확인** — 컴파일 실패

- [ ] **Step 3: 구현**

`src/config.rs` 찾아 바꾼다:
```rust
/// 키를 찾는다. 순서: `LINEAR_API_KEY` 환경 변수 → credentials 파일 → `fallbacks`.
```
→
```rust
/// CLI(`~/.local/state/herdr-linear`)와 herdr 플러그인(`~/.local/state/herdr/plugins/jh.linear`)의
/// 캐시 위치. `logout`은 키처럼 두 곳의 캐시를 모두 지운다.
pub fn cache_fallbacks(home: &Path) -> Vec<PathBuf> {
    let state = home.join(".local").join("state");
    vec![
        state.join(APP_NAME).join("cache.db"),
        state
            .join("herdr")
            .join("plugins")
            .join(PLUGIN_ID)
            .join("cache.db"),
    ]
}

/// 실제 HOME 기준 캐시 위치. 테스트에서는 쓰지 말고 임시 경로를 넘긴다.
pub fn default_cache_fallbacks() -> Vec<PathBuf> {
    std::env::var_os("HOME")
        .map(|h| cache_fallbacks(Path::new(&h)))
        .unwrap_or_default()
}

/// 키를 찾는다. 순서: `LINEAR_API_KEY` 환경 변수 → credentials 파일 → `fallbacks`.
```

`src/cli.rs` 찾아 바꾼다:
```rust
use std::io::IsTerminal;
```
→
```rust
use std::io::IsTerminal;
use std::path::PathBuf;
```

찾아 바꾼다:
```rust
            let out = logout(&paths, &config::default_credential_fallbacks())?;
```
→
```rust
            let out = logout(
                &paths,
                &config::default_credential_fallbacks(),
                &config::default_cache_fallbacks(),
            )?;
```

찾아 바꾼다:
```rust
/// 키(보조 위치 포함)와 캐시를 지운다. `fallbacks`는 실제 실행에서만 HOME 기준 위치를 넘긴다.
pub fn logout(paths: &Paths, fallbacks: &[std::path::PathBuf]) -> Result<String> {
    config::delete_credentials(paths, fallbacks)?;
    remove_db_files(&paths.cache_db());
    Ok("API 키와 캐시를 지웠어요".to_string())
}
```
→
```rust
/// 키와 캐시를 지운다. CLI와 플러그인은 위치가 달라서 `credentials`·`caches`로 다른 위치도 함께 지운다.
/// 다른 위치는 실제 실행에서만 HOME 기준으로 넘긴다.
pub fn logout(paths: &Paths, credentials: &[PathBuf], caches: &[PathBuf]) -> Result<String> {
    config::delete_credentials(paths, credentials)?;
    remove_db_files(&paths.cache_db());
    for db in caches {
        remove_db_files(db);
    }
    Ok("API 키와 캐시를 지웠어요".to_string())
}
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 5: 커밋**

```bash
git add src/config.rs src/cli.rs
git commit -m "fix(cli): logout이 다른 위치(CLI·플러그인)의 캐시도 지우기"
```

---

### Task 10: 키를 바꾼 직후 오프라인이면 다른 워크스페이스 캐시를 보이지 않기

**Files:**
- Modify: `src/cli.rs` (`cache_is_ours`, `mine`·`search`·`show`의 오프라인 대체, 테스트)

**Interfaces:**
- Produces: `fn cache_is_ours(ctx: &Ctx) -> Result<bool>`(비공개). 테스트 도우미 `trust_cache(&Ctx)`.

- [ ] **Step 1: 테스트**

`src/cli.rs` 테스트 모듈에서 찾아 바꾼다(도우미 추가):
```rust
    /// 아무도 듣지 않는 주소 (오프라인 흉내)
    const OFFLINE: &str = "http://127.0.0.1:9/graphql";
```
→
```rust
    /// 아무도 듣지 않는 주소 (오프라인 흉내)
    const OFFLINE: &str = "http://127.0.0.1:9/graphql";

    /// 이 키로 워크스페이스를 확인해 둔 캐시 (오프라인 대체에 쓸 수 있다).
    fn trust_cache(ctx: &Ctx) {
        let v: Viewer = serde_json::from_value(viewer_json()).unwrap();
        save_viewer(&ctx.store, &v, NOW, &ctx.client.key_fingerprint()).unwrap();
    }
```

다음 다섯 테스트에서 `let (_d, ctx) = test_ctx(OFFLINE.into());`(또는 `let (_d, mut ctx) = …`) 바로 다음 줄에 `trust_cache(&ctx);`를 넣는다: `mine_offline_uses_saved_view`, `search_offline_falls_back_to_local`, `show_offline_uses_cache`, `list_and_detail_strip_control_characters`, `show_strips_entity_encoded_escapes_with_color_on`.

테스트 모듈 끝에 넣는다:
```rust
    #[test]
    fn offline_after_key_change_hides_the_other_workspace() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        // 이전 키로 확인한 워크스페이스의 캐시
        let v: Viewer = serde_json::from_value(viewer_json()).unwrap();
        save_viewer(&ctx.store, &v, NOW, "fp-of-another-key").unwrap();
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("o1", "OLD-1", "옛 워크스페이스").build()],
                NOW,
            )
            .unwrap();
        ctx.store.set_view("mine", &["o1".into()], NOW).unwrap();
        for out in [
            mine(&ctx),
            search(&ctx, "워크스페이스", false),
            show(&ctx, "OLD-1"),
        ] {
            let err = out.unwrap_err().to_string();
            assert!(err.contains("이 키로 저장된"), "{err}");
        }
    }
```

- [ ] **Step 2: 실패 확인** — `offline_after_key_change_hides_the_other_workspace` FAIL(`mine`이 OLD-1을 보임)

- [ ] **Step 3: 구현** — `src/cli.rs`:

찾아 바꾼다:
```rust
        Err(ApiError::Offline(msg)) => match ctx.store.get_view("mine")? {
            Some((issues, at)) => Ok(format_list(
                ctx,
                &issues,
                Some(&format!(
                    "오프라인: {} 저장된 결과 · {msg}",
                    ago(ctx.now_ms, at)
                )),
            )),
            None => Err(anyhow!("오프라인이고 저장된 결과도 없어요: {msg}")),
        },
```
→
```rust
        Err(ApiError::Offline(msg)) => {
            if !cache_is_ours(ctx)? {
                bail!("오프라인이고 이 키로 저장된 결과가 없어요: {msg}");
            }
            match ctx.store.get_view("mine")? {
                Some((issues, at)) => Ok(format_list(
                    ctx,
                    &issues,
                    Some(&format!(
                        "오프라인: {} 저장된 결과 · {msg}",
                        ago(ctx.now_ms, at)
                    )),
                )),
                None => Err(anyhow!("오프라인이고 저장된 결과도 없어요: {msg}")),
            }
        }
```

찾아 바꾼다:
```rust
        Err(ApiError::Offline(msg)) => Ok(format_list(
            ctx,
            &local[..local.len().min(SEARCH_LIMIT)],
            Some(&format!("오프라인: 저장된 이슈에서만 찾았어요 · {msg}")),
        )),
```
→
```rust
        Err(ApiError::Offline(msg)) => {
            if !cache_is_ours(ctx)? {
                bail!("오프라인이고 이 키로 저장된 이슈가 없어요: {msg}");
            }
            Ok(format_list(
                ctx,
                &local[..local.len().min(SEARCH_LIMIT)],
                Some(&format!("오프라인: 저장된 이슈에서만 찾았어요 · {msg}")),
            ))
        }
```

찾아 바꾼다:
```rust
        Err(ApiError::Offline(msg)) => {
            let issue = ctx
                .store
                .get_issue(id)?
```
→
```rust
        Err(ApiError::Offline(msg)) => {
            if !cache_is_ours(ctx)? {
                bail!("오프라인이고 이 키로 저장된 {id}도 없어요: {msg}");
            }
            let issue = ctx
                .store
                .get_issue(id)?
```

찾아 바꾼다:
```rust
/// 검색 범위 팀. config의 `teams`가 비어 있으면 내가 속한 팀 전부.
```
→
```rust
/// 이 키로 워크스페이스를 확인한 캐시인지. 아니면(키를 바꾼 직후 오프라인) 캐시가 다른
/// 워크스페이스 것일 수 있어서 오프라인 대체에 쓰지 않는다.
fn cache_is_ours(ctx: &Ctx) -> Result<bool> {
    Ok(cached_viewer(&ctx.store, &ctx.client.key_fingerprint())?.is_some())
}

/// 검색 범위 팀. config의 `teams`가 비어 있으면 내가 속한 팀 전부.
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 5: 커밋**

```bash
git add src/cli.rs
git commit -m "fix(cli): 키를 바꾼 직후 오프라인이면 다른 워크스페이스 캐시로 대신하지 않기 (mine·search·show)"
```

---

### Task 11: CLI 출력을 받는 파이프가 먼저 끝나도 패닉하지 않기

**Files:**
- Modify: `src/cli.rs` (`write_out`, 테스트), `src/main.rs`

**Interfaces:**
- Produces: `cli::write_out(w: &mut impl std::io::Write, out: &str) -> std::io::Result<()>` — 받는 쪽이 닫혔으면(`BrokenPipe`) `Ok`.

- [ ] **Step 1: 테스트** — `src/cli.rs` 테스트 모듈 끝에 넣는다:
```rust
    /// 받는 쪽이 먼저 끝난 파이프 (`herdr-linear mine | head`)
    struct ClosedPipe;

    impl std::io::Write for ClosedPipe {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn closed_pipe_is_not_an_error() {
        assert!(write_out(&mut ClosedPipe, "ENG-1").is_ok());
        let mut buf = Vec::new();
        write_out(&mut buf, "ENG-1").unwrap();
        assert_eq!(buf, b"ENG-1\n");
    }
```

- [ ] **Step 2: 실패 확인** — 컴파일 실패(`write_out` 없음)

- [ ] **Step 3: 구현**

`src/cli.rs` 찾아 바꾼다:
```rust
use std::io::IsTerminal;
use std::path::PathBuf;
```
→
```rust
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
```

찾아 바꾼다:
```rust
pub fn now_ms() -> i64 {
```
→
```rust
/// 출력을 쓴다. 받는 쪽이 먼저 끝났으면(`herdr-linear mine | head`) 조용히 넘어간다.
pub fn write_out(w: &mut impl Write, out: &str) -> std::io::Result<()> {
    match writeln!(w, "{out}").and_then(|()| w.flush()) {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

pub fn now_ms() -> i64 {
```

`src/main.rs` 찾아 바꾼다:
```rust
        Ok(out) => {
            if !out.is_empty() {
                println!("{out}");
            }
        }
```
→
```rust
        Ok(out) => {
            if !out.is_empty()
                && let Err(e) = herdr_linear::cli::write_out(&mut std::io::stdout().lock(), &out)
            {
                eprintln!("오류: 출력하지 못했어요: {e}");
                std::process::exit(1);
            }
        }
```

- [ ] **Step 4: 통과 확인** — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`. 실제로도 확인한다(설정·상태·HOME을 임시 디렉터리로 두고 다른 환경 변수는 비운다):

```bash
tmp=$(mktemp -d)
for i in 1 2 3 4 5; do
  env -i PATH="$PATH" HOME="$tmp" HERDR_LINEAR_CONFIG_DIR="$tmp/c" HERDR_LINEAR_STATE_DIR="$tmp/s" \
    ./target/debug/herdr-linear logout | true
done
```
Expected: `panicked` 문구가 한 번도 나오지 않는다.

- [ ] **Step 5: 커밋**

```bash
git add src/cli.rs src/main.rs
git commit -m "fix(cli): 출력을 받는 파이프가 먼저 끝나도(| head) 패닉하지 않기"
```

---

### Task 12: 문서와 버전 0.1.1

**Files:**
- Modify: `Cargo.toml`, `herdr-plugin.toml` (version 0.1.1), `Cargo.lock`(빌드로 갱신)
- Modify: `docs/superpowers/specs/2026-10-06-herdr-linear-design.md` (4.3, 4.7, 12장)
- Modify: `README.md` (상세 키, 문제 해결의 한도 초과 — 영어·한국어)
- Modify: `ROADMAP.md` (v0.1.1, v0.2 손질 항목 — 영어·한국어)
- Modify: `docs/superpowers/notes/2026-10-07-part2-followups.md` (v0.1.1에서 고친 것, 3부에 남은 것)

- [ ] **Step 1: 버전** — `Cargo.toml`과 `herdr-plugin.toml`의 `version = "0.1.0"`을 `"0.1.1"`로 바꾸고 `cargo build`로 `Cargo.lock`을 갱신한다.

- [ ] **Step 2: 스펙**
  - 4.3: "`방금 갱신` / `n분 전 갱신` / `갱신 중…` / `오프라인` / `오류` 중 하나를 보여준다." 뒤에 "한도 초과는 상단에 남기지 않고 하단에 3초 동안 언제 다시 시도할지 알린다. 설정·범위 경고는 하단(키 입력 화면에서는 맨 아래 줄)에 10초 동안 보이고 일반 안내에 덮이지 않는다."
  - 4.7 상세 화면 줄: "…, Esc = 뒤로" → "…, Esc = 뒤로, `q` = 닫기"
  - 12장 한도 초과 줄: "리셋 시각까지 자동 요청을 멈추고, 하단에 3초 동안 언제 다시 시도할지 알린다"

- [ ] **Step 3: README** — 영어·한국어 모두
  - 상세 키 줄에 `q` 닫기를 더한다.
  - 문제 해결의 한도 초과 줄: "a short notice says when to try again; automatic server search waits until then and local search keeps working" / "잠깐 뜨는 안내에 언제 다시 시도할지 나와요. 그때까지 자동 서버 검색은 멈추고 캐시 검색은 계속 돼요."

- [ ] **Step 4: ROADMAP** — 영어·한국어 모두 v0.1 아래에 "✅ v0.1.1 — v0.1 리뷰 수정"을 더하고(상세의 `q` 닫기, 코멘트 링크 번호, 목록 스크롤 위치, 한도 초과 안내, 경고 표시, CLI 파이프·`logout`·오프라인 대체), v0.2의 손질 항목에서 이미 고친 "코멘트 링크 번호"를 뺀다.

- [ ] **Step 5: 후속 메모** — `2026-10-07-part2-followups.md`에 "v0.1.1에서 고친 것" 표(항목·커밋)와 "3부에 남은 것"(N2, N6, N10, 실행 중 캐시 손상, markdown 외형, N14 기록만)을 더한다.

- [ ] **Step 6: 확인과 커밋**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && cargo build --release`

```bash
git add Cargo.toml Cargo.lock herdr-plugin.toml README.md ROADMAP.md docs/
git commit -m "docs: v0.1.1 — 스펙·README·로드맵·후속 메모 반영, 버전 0.1.1"
```
