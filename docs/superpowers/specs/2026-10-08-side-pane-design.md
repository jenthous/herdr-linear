# 사이드 pane 설계 (v0.2)

- 작성일: 2026-10-08
- 상태: 사용자 리뷰 대기
- 버전: v0.2.0
- 바탕 스펙: `docs/superpowers/specs/2026-10-06-herdr-linear-design.md`의 3.1(매니페스트), 3.3(진입 흐름), 4.2(사이드 패널), 6.1(파일), 7.1(자동 요청 멈춤), 11(설정)
  - 이 문서와 다른 부분은 이 문서를 따른다. 특히 4.2의 화면 배치("목록 위·미리보기 아래")는 쓰지 않는다.
  - 구현이 끝나면 바탕 스펙을 이 문서대로 고친다.

## 1. 목적

팝업 대신 herdr pane에 Linear를 상시로 띄워 두고 본다. 화면은 지금 팝업 팔레트와 같고, 상시로 띄워 두는 데 필요한 것만 더한다.

### 1.1 성공 기준

1. 키 하나로 지금 pane 오른쪽에 사이드 pane을 열고, 같은 키로 닫는다. 워크스페이스마다 하나다.
2. 사이드 pane은 Esc로 닫히지 않는다. `q`(또는 Ctrl+C)나 열고 닫는 키로만 닫는다.
3. `side.refresh_seconds`(기본 60초)마다 지금 보이는 것이 저절로 새로고침된다. 선택한 줄과 스크롤 위치는 그대로다.
4. 가만히 있을 때 CPU를 거의 쓰지 않는다. 다시 그리기는 바뀐 게 있을 때와 1초에 한 번뿐이다.
5. 팝업 팔레트는 지금처럼 따로 쓴다.

### 1.2 범위 밖

- 바탕 스펙 4.2의 "목록 위·미리보기 아래" 배치 (사용자 결정: 팝업 화면 그대로)
- 키 하나와 설정으로 팝업·pane 고르기, 팝업에서 사이드 pane으로 옮기기 (사용자 결정: 키 두 개)
- 이슈 하나를 고정해 보는 화면, 목록 옆 미리보기의 관계 표시
- pane 폭 지정(herdr 기본 분할을 쓰고, 사용자가 herdr에서 조절한다)
- 변경 동작과 에이전트 전달 (v0.3)

## 2. 사용

### 2.1 키

플러그인은 키를 직접 등록할 수 없다. 사용자가 `~/.config/herdr/config.toml`에 넣는다.

```toml
[[keys.command]]
key = "prefix+i"
type = "plugin_action"
command = "jh.linear.palette"
description = "Linear 검색"

[[keys.command]]
key = "prefix+shift+i"
type = "plugin_action"
command = "jh.linear.side"
description = "Linear 사이드 pane"
```

한글 입력 중에는 prefix 뒤 글자가 IME에 먹힌다. 그래서 팝업처럼 Ctrl 조합 직접 키를 하나 더 두는 것을 권한다.

### 2.2 동작

| 상황 | 동작 |
|---|---|
| 사이드 pane이 없는 워크스페이스에서 키 | 지금 pane 오른쪽에 연다(herdr split, 폭은 herdr 기본). 포커스를 사이드 pane으로 옮긴다 |
| 사이드 pane이 있는 워크스페이스에서 키 | 그 사이드 pane을 닫는다 |
| 화면 | 팝업 팔레트와 같다. 검색 모드로 시작하고, 넓으면 목록·미리보기를 가로로 놓고, 상세에는 관계 칸이 있다 |
| Esc | 검색 모드 → 목록 모드, 메뉴 → 닫기, 상세 → 뒤로. 목록 모드에서는 아무 일도 하지 않는다 |
| `q`, Ctrl+C | 사이드 pane을 닫는다 |
| 현재 브랜치 이슈 | 사이드 pane을 연 pane의 cwd로 찾는다(팝업과 같다). 새로고침 때 다시 찾는다 |
| 새로고침 | 3장 |

## 3. 자동 새로고침

- **주기**: `side.refresh_seconds`마다 한다. 0이면 끈다. 팝업은 하지 않는다.
- **하는 일**: `r`을 누른 것과 같다.
  - 상세 화면: 그 상세(이슈·코멘트·관계)를 다시 받는다. 스크롤은 그대로다.
  - 검색어가 있으면: 서버 검색을 다시 한다.
  - 그 밖: 지금 탭 목록과 현재 브랜치 이슈를 다시 받는다.
- **건너뛰기**: 아래 경우에는 그 회차를 건너뛰고 다음 주기에 다시 본다.
  - 앞선 요청이 아직 돌아오지 않았을 때(`loading > 0`)
  - API 한도 때문에 자동 요청이 멈춰 있을 때(바탕 스펙 7.1의 멈춤 시각 전)
  - 키 입력 화면일 때
- **시점**:
  - 첫 자동 새로고침은 시작하고 한 주기 뒤다. 시작할 때는 지금처럼 불러온다.
  - 사용자가 `r`을 누르면 다음 자동 새로고침은 그때부터 한 주기 뒤다.
- 상세를 다시 받으면 지금 흐름대로 "최근 본" 시각도 바뀐다.

## 4. 다시 그리기 (팝업·사이드 공통)

지금은 가만히 있어도 50ms마다 다시 그린다(2부 후속 N10). 이를 바꿔서 아래 경우에만 그린다.

- 키·마우스·붙여넣기 입력이나 터미널 크기 변화를 처리했을 때
- 네트워크 응답, 앱의 할 일(`Effect`), 앱에 알린 결과(`Msg`)를 처리했을 때
- 마지막으로 그린 지 1초가 지났을 때. 상단 "n분 전 갱신"이 바뀌고 3초·10초 안내가 사라지게 한다

입력을 기다리는 간격(50ms)은 그대로라서 반응 속도는 같다.

## 5. herdr 연결

### 5.1 매니페스트

```toml
[[actions]]
id = "side"
title = "Linear: 사이드 pane 열기/닫기"
description = "오른쪽에 Linear pane을 열거나 닫는다"
contexts = ["global", "workspace", "pane"]
command = ["./target/release/herdr-linear", "open", "side"]

[[panes]]
id = "side"
title = "Linear"
placement = "split"
command = ["./target/release/herdr-linear", "ui", "--mode", "side"]
```

### 5.2 열고 닫기 (`open side`)

1. **워크스페이스 id**: `HERDR_WORKSPACE_ID`를 쓴다. 없으면 컨텍스트의 `focused_pane_id`로 `herdr pane get`을 불러 `workspace_id`를 읽는다.
2. **기록 찾기**: 상태 디렉터리의 `side-panes.json`(`{ "<workspace_id>": "<pane_id>" }`)에서 그 워크스페이스의 pane을 찾는다.
3. **닫기**: 기록이 있고 `herdr pane get <pane>`이 성공하면 `herdr plugin pane close <pane>`으로 닫고 기록을 지운다.
4. **열기**: 기록이 없거나 그 pane이 이미 없으면(남은 기록은 지운다) 새로 연다. `herdr plugin pane open --plugin jh.linear --entrypoint side --placement split --direction right --focus`에 팝업과 같은 `--env`(원래 pane의 맥락)를 붙인다.
5. **실패**: 팝업처럼 로그에 남기고 herdr 알림으로 알린다.

### 5.3 사이드 프로세스 (`ui --mode side`)

- 팝업과 같은 준비를 한 뒤 앱을 사이드 모드로 시작한다. 준비는 설정·키·캐시·원래 pane 맥락을 읽는 것이다.
- **기록**:
  - 시작할 때 자기 pane id와 워크스페이스 id를 `side-panes.json`에 적는다. pane id는 `HERDR_PANE_ID`, 없으면 `herdr pane current`로 얻는다. 워크스페이스 id는 `HERDR_WORKSPACE_ID`, 없으면 `herdr pane current`로 얻는다.
  - 정상 종료할 때는 자기 기록만 지운다.
  - 비정상 종료로 남은 기록은 5.2의 3번에서 걸러진다.
- **기록 파일 쓰기**: 쓸 때마다 통째로 다시 쓴다(임시 파일에 쓰고 이름 바꾸기). 읽지 못하거나 모양이 틀리면 빈 기록으로 본다. 두 프로세스가 동시에 쓰는 경우는 드물어서 잠금은 두지 않는다.

### 5.4 구현 초기에 확인할 가정

아래는 실제 herdr에서 pane을 열어 봐야 알 수 있다. 사용자의 herdr 화면에 pane이 열리므로 사용자와 함께 확인한다.

| 가정 | 확인 방법 | 다를 때 |
|---|---|---|
| 사이드 pane 프로세스가 `HERDR_PANE_ID`·`HERDR_WORKSPACE_ID`를 받는다 | 시작할 때 두 값이 있는지 로그에 남겨 본다 | `herdr pane current`로 조회한다 |
| 액션 프로세스가 `HERDR_WORKSPACE_ID`를 받는다 | 같은 방법 | `focused_pane_id`로 `herdr pane get` |
| 사이드 프로세스가 끝나면 herdr가 그 pane을 닫는다 | `q`로 끝내 본다 | 끝나기 직전에 `herdr pane close <자기 pane>`을 부른다 |
| 닫힌 pane에는 `herdr pane get`이 실패한다 | 닫은 뒤 불러 본다 | `herdr pane list`에서 찾는다 |
| `herdr plugin pane close`가 사이드 pane을 닫는다 | 실제로 불러 본다 | `herdr pane close`를 쓴다 |

## 6. 코드

| 파일 | 바뀌는 것 |
|---|---|
| `herdr-plugin.toml` | 5.1의 액션·pane |
| `src/cli.rs` | `OpenTarget::Side`, `UiMode::Side`, 실행 분기 |
| `src/herdr.rs` | `open_pane`에 배치(split·방향) 인자, `pane get`·`pane current`·`plugin pane close` 호출, `open side` 흐름 |
| `src/side.rs` (새 파일) | `side-panes.json` 읽기·쓰기·지우기(워크스페이스 → pane) |
| `src/tui/mod.rs` | `side(paths)` 진입점: 준비, 기록, 사이드 모드 앱, 종료 때 기록 지우기 |
| `src/tui/app.rs` | 사이드 모드 표시, 목록 모드 Esc, `tick`의 자동 새로고침, `r`이 다음 자동 새로고침을 미룸 |
| `src/tui/runtime.rs` | 다시 그릴지 판단하는 함수와 이벤트 루프 |
| README·ROADMAP·바탕 스펙 | 8장 |

설정 `[side] refresh_seconds`는 이미 읽고 검사한다(`Settings::side_refresh_seconds`).

## 7. 테스트

| 대상 | 내용 |
|---|---|
| `tui::app` | 아래 항목 |
| `side` | 기록·읽기·지우기, 다른 워크스페이스 기록 유지, 깨진 파일은 빈 기록, 자기 기록만 지우기 |
| `herdr` | 가짜 herdr 스크립트로 확인한다.<br>• 기록 없음 → `plugin pane open` 인자(`--placement split --direction right --focus`, `--env`)<br>• 살아 있는 기록 → `plugin pane close` 후 기록 삭제<br>• 죽은 기록 → 기록 삭제 후 열기<br>• 워크스페이스 id를 `pane get`으로 얻기 |
| `cli` | `open side`, `ui --mode side` 해석 (지금의 "`open side`는 오류" 테스트를 바꾼다) |
| `tui::runtime` | 다시 그릴지 판단: 바뀐 게 있으면 그리고, 없으면 1초가 지난 뒤에만 그린다 |

`tui::app`에서 확인할 것:
- 사이드 모드에서 목록 모드 Esc로는 끝나지 않고 `q`로 끝난다. 팝업 모드는 지금처럼 Esc로 끝난다.
- 주기마다 화면에 맞는 요청을 낸다.
  - 상세: `OpenDetail`, 스크롤 유지
  - 검색어 있음: 서버 검색
  - 그 밖: `LoadTab`과 `ResolvePinned`
- 대기 중이거나 멈춤 시각 전이거나 키 입력 화면이면 건너뛴다.
- 0이면 끈다. `r`을 누르면 다음 자동 새로고침이 미뤄진다.
- 요청 회계(`loading`)가 맞는다.

수동 확인은 herdr에서 한다.
- 키로 열고 닫기, Esc로 닫히지 않음, `q`로 닫은 뒤 다시 열기
- 1분 뒤 상단이 "방금 갱신"으로 바뀌는지
- 상세를 열어 둔 채 Linear에서 상태를 바꾸면 1분 안에 바뀌는지
- 가만히 둘 때 CPU가 거의 0%인지(활동 모니터나 `top`)
- 팝업과 함께 쓰기

## 8. 문서·버전

- README(영어·한국어)
  - 사이드 pane 소개
  - 키 설정 예시 두 개
  - `[side] refresh_seconds` 설정
  - 문제 해결: 사이드 pane이 열리지 않거나 닫히지 않을 때
- ROADMAP(영어·한국어)
  - v0.2를 "사이드 pane"으로 완료 처리한다.
  - 남은 v0.2 항목(터미널에서 바로 바꾸기, 이슈를 에이전트에게 보내기, 리뷰에서 미룬 손질)은 v0.3 "변경 동작과 에이전트 전달 (다음)"으로 옮긴다.
- 바탕 스펙: 이 문서대로 고친다.
  - 3.1 매니페스트, 3.3 `open side` 흐름
  - 4.2 화면은 팝업과 같다
  - 4.7 사이드 모드의 Esc
  - 6.1 `side-panes.json`
  - 11 설정 설명
- `Cargo.toml`과 `herdr-plugin.toml`의 버전을 0.2.0으로 올린다.
