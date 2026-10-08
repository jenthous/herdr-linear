# herdr-linear 설계

- 작성일: 2026-10-06
- 상태: 사용자 리뷰 대기
- 근거 리서치: `docs/research/2026-10-06-herdr-linear-landscape.md`
- 2026-10-07 공개하면서 herdr 플러그인 id를 `jh.linear`로 바꿨다. 같은 id를 쓰는 다른 Linear 플러그인과 겹치지 않게 하려는 것이다. 앱·바이너리·CLI 디렉터리 이름은 `herdr-linear` 그대로다. 아래 매니페스트·단축키·플러그인 디렉터리의 `herdr-linear`는 `jh.linear`로 읽는다(예: `jh.linear.palette`, `~/.config/herdr/plugins/config/jh.linear`).

## 1. 목적

herdr 안에서 Linear를 **빠르게 조회**하는 플러그인이다. 핵심은 이슈 본문 읽기, 검색, 상태·라벨 확인이다. 필요하면 그 자리에서 가벼운 변경과 에이전트 전달까지 한다. 사용자가 할 설정은 Linear Personal API 키를 한 번 붙여넣는 것뿐이다.

### 1.1 사용자와 환경

- 1차 사용자는 이 저장소 주인이다. herdr 0.9.3을 매일 쓰고(prefix `ctrl+a`) 한글 IME를 쓴다.
- 주 환경은 macOS(arm64)이고 Linux도 지원한다.
- 공개(GitHub `jenthous/herdr-linear` 또는 다른 이름, herdr 마켓플레이스 등록)는 기능이 갖춰진 뒤에 따로 진행한다.

### 1.2 성공 기준

1. 단축키를 누르면 팔레트가 바로 뜨고, 지난번 목록이 캐시에서 즉시 보인다. 목표는 키 입력부터 첫 화면까지 100ms 이내다.
2. 타이핑하면 캐시된 이슈가 즉시 걸러진다. 목표는 캐시 5천 건 기준 키 입력당 16ms 이내다. 서버 결과는 보통 1초 안에 합쳐진다.
3. 이슈 본문 markdown이 터미널에서 읽기 좋게 렌더링되고, 한글 폭이 정확하다.
4. 상태·라벨·담당자 변경, 코멘트 작성, 새 이슈 생성, 에이전트 전달을 팔레트나 사이드 패널 안에서 끝낼 수 있다.
5. API 키를 붙여넣는 것 외에 필수 설정이 없다.

### 1.3 범위 밖 (v1)

- 전체 워크스페이스 동기화, 백그라운드 데몬, `[[startup]]` 훅
- worktree 생성·열기. 현재 브랜치에 연결된 이슈를 보여주는 것만 한다.
- 에이전트 상태를 Linear에 자동 반영하는 것, Linear 변경으로 herdr 동작을 자동 실행하는 것
- OAuth, 여러 워크스페이스 동시 사용
- 우선순위·프로젝트·사이클 변경, 코멘트 답글·수정·삭제, 이슈 삭제
- 이미지 인라인 표시, 코드 블록 문법 강조, 초성 검색
- 공개 배포(릴리스 바이너리, 마켓플레이스 등록)

## 2. 핵심 결정

| 항목 | 결정 | 이유 |
|---|---|---|
| 언어 | Rust | 설치된 herdr 플러그인 다수가 Rust이고, 단일 바이너리라 바로 실행된다 |
| 주요 크레이트 | ratatui + crossterm, nucleo-matcher, rusqlite(bundled), pulldown-cmark, ureq, serde / serde_json, toml, clap, tui-textarea, unicode-width, unicode-normalization | 버전은 구현 계획에서 확정한다 |
| 데이터 | 조회할 때 자동 저장되는 캐시(stale-while-revalidate). 전체 동기화는 하지 않는다 | API 사용과 구현을 단순하게 유지한다 |
| 인증 | Linear Personal API 키만 쓴다 | 키를 한 번 붙여넣으면 끝난다 |
| 화면 | 팔레트(popup)와 사이드 패널(오른쪽 split) | 잠깐 찾을 때와 띄워두고 볼 때를 나눈다 |
| 검색 | 로컬 캐시에서 즉시 걸러내고, `issues` 필터 검색을 디바운스해서 보낸다. 사용자가 고를 때만 `searchIssues`를 쓴다 | `searchIssues`는 분당 30회 제한이 있다 |
| worktree | 만들지 않는다. 현재 브랜치의 이슈만 보여준다 | 사용자 결정 |

## 3. herdr 연결

### 3.1 매니페스트 (`herdr-plugin.toml`)

```toml
id = "herdr-linear"
name = "Linear"
version = "0.1.0"
min_herdr_version = "0.9.3"
description = "Fast Linear lookup in herdr: search, read, triage, and hand issues to agents."
platforms = ["macos", "linux"]

[[build]]
command = ["cargo", "build", "--release"]

[[actions]]
id = "palette"
title = "Linear: 검색 팔레트"
description = "Linear 이슈 검색 팔레트를 연다"
contexts = ["global", "workspace", "pane"]
command = ["./target/release/herdr-linear", "open", "palette"]

[[actions]]
id = "side"
title = "Linear: 사이드 pane 열기/닫기"
description = "오른쪽에 Linear pane을 열거나 닫는다"
contexts = ["global", "workspace", "pane"]
command = ["./target/release/herdr-linear", "open", "side"]

[[actions]]
id = "open-url"
title = "Linear: 링크로 이슈 열기"
description = "클릭한 linear.app 이슈 링크를 팔레트에서 연다"
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

[[panes]]
id = "side"
title = "Linear"
placement = "split"
command = ["./target/release/herdr-linear", "ui", "--mode", "side"]

[[link_handlers]]
id = "linear-issue"
title = "Linear 이슈 열기"
pattern = "^https://linear\\.app/[^/]+/issue/[A-Za-z0-9]+-[0-9]+"
action = "open-url"
```

### 3.2 키 바인딩

플러그인은 키를 직접 등록할 수 없다. 사용자가 `~/.config/herdr/config.toml`에 아래를 추가한다. 지금 사용자 설정에서 비어 있는 키를 골랐다.

```toml
[[keys.command]]
key = "prefix+i"
type = "plugin_action"
command = "herdr-linear.palette"
description = "Linear 검색"

[[keys.command]]
key = "prefix+shift+i"
type = "plugin_action"
command = "herdr-linear.side"
description = "Linear 사이드 패널"
```

- 한글 입력 상태에서는 prefix 뒤의 글자 키가 IME에 먹힌다. 그래서 같은 액션에 `ctrl+alt+i` 같은 직접 바인딩을 하나 더 두는 것을 권한다(선택).
- 액션은 설치된 command-palette 플러그인에도 자동으로 나타난다.

### 3.3 진입 흐름

- **`open palette`**
  - `HERDR_PLUGIN_CONTEXT_JSON`에서 `focused_pane_id`, `focused_pane_agent`, `focused_pane_cwd`(없으면 `workspace_cwd`), `selected_text`를 읽는다.
  - 읽은 값을 `herdr plugin pane open --plugin herdr-linear --entrypoint palette --focus`에 `--env`로 넘긴다. 넘기는 변수는 `HERDR_LINEAR_ORIGIN_PANE`, `HERDR_LINEAR_ORIGIN_AGENT`, `HERDR_LINEAR_ORIGIN_CWD`, `HERDR_LINEAR_OPEN`이다.
  - `selected_text`가 이슈 식별자 형식(`[A-Za-z][A-Za-z0-9]*-[0-9]+`)이면 `HERDR_LINEAR_OPEN`에 넣는다. 팔레트는 그 이슈의 상세 화면으로 시작한다.
- **`open side`** (자세한 흐름은 `docs/superpowers/specs/2026-10-08-side-pane-design.md` 7장)
  - 워크스페이스 id는 `HERDR_WORKSPACE_ID`, 컨텍스트 JSON의 `workspace_id`, 포커스된 pane의 `herdr pane get` 순서로 얻는다.
  - state 디렉터리의 `side-panes.json`(workspace id → pane id)에 그 워크스페이스 pane이 있으면 `herdr pane process-info`로 그 pane에서 도는 명령을 본다. 우리 사이드 화면(`herdr-linear ui --mode side`)이면 `herdr plugin pane close`로 닫고, 아니면 기록만 지운다(그 pane은 닫지 않는다).
  - 닫지 않았으면 `herdr plugin pane open --plugin jh.linear --entrypoint side --placement split --direction right --focus`에 팔레트와 같은 `--env`를 붙여 연다.
  - 사이드 패널 프로세스는 시작할 때 자기 pane id(`HERDR_PANE_ID`, 없으면 `herdr pane current`)를 이 파일에 기록하고, 정상 종료할 때 자기 기록만 지운다.
- **`open url`**: 컨텍스트의 `clicked_url`에서 식별자를 뽑아 `HERDR_LINEAR_OPEN`으로 팔레트를 연다.
- **`logout`**: `credentials`와 `cache.db`를 지우고 `herdr notification show`로 결과를 알린다.
- popup 열기가 `ui_busy`로 거절되면 `herdr notification show`로 알린다. 설정 화면, 복사 모드, 다른 모달이 떠 있을 때 이렇게 된다.

### 3.4 현재 맥락 활용

- **브랜치 → 이슈**
  1. `git -C <origin_cwd> rev-parse --show-toplevel`과 `git -C <origin_cwd> rev-parse --abbrev-ref HEAD`로 레포와 브랜치를 얻는다.
  2. 브랜치명에서 `(?i)([a-z][a-z0-9]*)-([0-9]+)`를 찾는다. 앞부분이 범위 팀 키 중 하나와 일치하면 그 식별자로 이슈를 찾는다(캐시를 먼저 보고, 없으면 `issue(id:)`).
  3. 찾지 못하면 `issueVcsBranchSearch(branchName)`를 한 번 부른다.
  4. 결과는 "없음"까지 포함해 `(레포, 브랜치)`별로 10분 캐시한다.
- **선택 텍스트**: 식별자 형식이면 그 이슈 상세로 바로 연다(3.3).
- **에이전트 전달의 기본 대상**
  - 팔레트: `HERDR_LINEAR_ORIGIN_AGENT`가 있으면 원래 pane(`HERDR_LINEAR_ORIGIN_PANE`)이다.
  - 사이드 패널: 같은 탭에서 바로 왼쪽 pane에 에이전트가 있으면 그 pane이다.

## 4. 화면과 조작

### 4.1 팔레트

popup 80%×80%이고 검색 모드로 시작한다.

```
Linear   [내 이슈]  최근 본  전체                              2분 전 갱신
> █  (ID·제목·본문 검색 · l:라벨 s:상태 @담당자)
────────────────────────────────────────┬──────────────────────────────────
 현재 브랜치                            │ ENG-131  로그인 버튼 비활성 버그
   ◐ ENG-142  로그인 세션 만료 처리     │ ◐ In Progress  ↑ High  김민수
 ───────────                            │ bug  frontend  ·  Auth 개편  ·  Cycle 12
 ▶ ◐ ENG-131  로그인 버튼 비활성 버그   │ ──────────────────────────────
   ○ ENG-140  결제 페이지 리팩터링      │ 재현 방법
   ○ ENG-152  알림 설정 저장 실패       │  1. 로그인 후 30분 대기
   ◌ ENG-160  다크 모드 색상 정리       │  2. 새로고침 시 401 발생
────────────────────────────────────────┴──────────────────────────────────
⏎ 상세   Tab 보기 전환   Ctrl+K 액션 메뉴   Esc 목록 모드
```

- 왼쪽 목록에서 선택을 옮기면 오른쪽 미리보기가 바로 바뀐다.
- 상태 아이콘(◇ triage, ◌ backlog, ○ unstarted, ◐ started, ● completed, ✕ canceled·duplicate)과 라벨에는 Linear에 설정된 색을 그대로 쓴다.
- 팝업 폭이 100칸 미만이면 미리보기를 숨기고 목록만 보여준다.
- 목록 줄은 식별자 뒤에 우선순위 칸(3칸)을 둔다. 긴급은 빨간 `!`, 높음·보통·낮음은 `▂▄▆` 중 3·2·1개를 밝히고 나머지는 흐리게, 없음은 빈칸이다(사이드 pane 설계 4장).

### 4.2 사이드 패널

오른쪽 split이고, 화면은 팔레트와 같다. 검색 모드로 시작하고, 넓으면 목록·미리보기를 가로로 놓고, 상세에는 관계 칸이 있다. 자세한 것은 `docs/superpowers/specs/2026-10-08-side-pane-design.md`.

- 목록 모드에서 Esc로 닫히지 않는다. `q`·Ctrl+C나 열고 닫는 키로 닫는다.
- 지금 보이는 것을 `side.refresh_seconds`(기본 60초)마다 다시 불러온다. 상세는 그 상세, 검색어가 있으면 서버 검색, 그 밖에는 지금 탭과 현재 브랜치 이슈다. 0이면 자동 새로고침을 끈다.
- 앞 요청을 기다리는 중이거나, 한도로 자동 요청을 멈췄거나, 키 입력 화면이면 그 회차를 건너뛴다. `r`을 누르면 다음 자동 새로고침은 그때부터 한 주기 뒤다.

### 4.3 상단 상태 표시

`방금 갱신` / `n분 전 갱신` / `갱신 중…` / `오프라인` / `오류` 중 하나를 보여준다.

- 한도 초과는 상단에 남기지 않는다. 하단에 3초 동안 언제 다시 시도할지 알리고, 리셋 시각까지 자동 서버 검색만 멈춘다.
- 설정·범위 경고는 하단(키 입력 화면에서는 맨 아래 줄)에 10초 동안 보이고, 일반 안내에 덮이지 않는다.

### 4.4 보기 탭 (Tab / Shift+Tab)

| 탭 | 내용 | 정렬 |
|---|---|---|
| 내 이슈 | 나에게 할당됐고 상태 타입이 completed·canceled·duplicate가 아닌 이슈 | started → unstarted → backlog → triage 순, 그다음 우선순위(긴급 → 낮음, 없음은 마지막), 그다음 최근 수정 |
| 최근 본 | 상세 화면을 연 이슈 최근 50개 | 최근 본 순 |
| 전체 | 범위 팀의 이슈(6.4) | 최근 수정 순. 끝까지 스크롤하면 50건씩 더 불러온다 |

검색어가 비어 있고 현재 브랜치 이슈가 있으면 맨 위 "현재 브랜치" 칸에 고정한다.

### 4.5 검색

검색어는 자유 텍스트와 필터 토큰을 섞어 쓴다. 여러 조건은 AND로 묶인다. 값에 공백이 있으면 따옴표로 감싼다. 예: `s:"In Progress"`.

| 토큰 | 의미 | 예 |
|---|---|---|
| `s:값` | 상태 이름 또는 상태 타입 키워드(7.3) | `s:진행`, `s:started` |
| `l:값` | 라벨 이름 | `l:bug` |
| `@값` | 담당자. `@나`, `@me`는 나를 뜻한다 | `@민수` |
| `#KEY` | 팀 키 | `#ENG` |
| `p:값` | 우선순위. `urgent`/`긴급`/`1`, `high`/`높음`/`2`, `medium`/`보통`/`3`, `low`/`낮음`/`4`, `none`/`없음`/`0` | `p:high` |

**로컬 단계 (즉시)**: 캐시된 이슈에 토큰 조건을 먼저 적용한 뒤 아래 순서로 순위를 매긴다.

1. 식별자 일치(`ENG-131`) 또는 번호 일치(`131`)
2. `식별자 제목` 문자열에 대한 nucleo 퍼지 점수
3. 본문 부분 문자열 일치(대소문자 무시, NFC 정규화 후 비교)

점수가 같으면 상태 타입 순서(started → unstarted → backlog → triage → completed → canceled)로, 그다음 최근 수정 순으로 정렬한다.

**서버 단계 (입력이 300ms 멈추면)**

- `issues` 필터 검색을 1회 보낸다(7.3). 서버 결과는 첫 50건만 가져온다. 더 필요하면 검색어를 좁힌다.
- 결과는 캐시에 저장하고, 로컬 결과와 합쳐 같은 규칙으로 다시 정렬한다.
- 합쳐도 선택 항목은 id 기준으로 유지한다.
- 이전 검색어에 대한 응답이 늦게 도착하면 요청 순번으로 걸러서 버린다.

**깊은 검색**: 결과 맨 아래에 항상 "서버에서 검색 (코멘트 포함)" 줄이 있다. 이 줄을 고르면 `searchIssues(term, includeComments: true, first: 20)`를 1회 부른다.

### 4.6 상세 화면

- **머리**: 식별자, 제목, 상태, 우선순위, 담당자, 열린 PR(번호·저장소), 라벨, 프로젝트, 사이클, 예상치, 마감일. 목록 옆 미리보기에서는 부모 이슈도 머리에 보인다.
- **관계**: 머리 아래에 상위·막힘·막는 중·관련·하위를 상태 색과 함께 보여 준다. `t` 메뉴나 관계 줄 클릭으로 열고, Esc로 한 단계씩 돌아온다. 자세한 규칙은 `2026-10-07-issue-relations-design.md`를 따른다.
- **본문**: markdown 렌더링(5장)
- **코멘트**: 오래된 것부터 작성자, 시각, 본문(markdown)을 보여준다. 50개를 넘으면 "브라우저에서 더 보기"를 표시한다.
- 저장된 내용을 먼저 보여주고 이슈와 코멘트를 1회 다시 불러와 바꾼다. 상세 화면을 열면 "최근 본"에 기록한다.

### 4.7 키

| 모드 | 키 |
|---|---|
| 검색 모드 (팔레트·사이드 패널 기본) | 글자 입력 = 검색어, ↑/↓ 또는 Ctrl+P/Ctrl+N = 이동, Enter = 상세, Tab/Shift+Tab = 보기 전환, Ctrl+K = 액션 메뉴, Esc = 목록 모드 |
| 목록 모드 | j/k·↑/↓ = 이동, g/G = 처음/끝, `/` = 검색 모드, Enter = 상세, Tab = 보기 전환, `r` = 새로고침, `s` 상태, `l` 라벨, `a` 담당자, `c` 코멘트, `p` 에이전트 전달, `n` 새 이슈, `o` 브라우저, `y` 티켓 URL 복사, `Y` 열린 PR 링크 복사(ID 복사는 Ctrl+K 메뉴), Ctrl+K = 액션 메뉴, `q` = 닫기(팔레트는 Esc도 닫기, 사이드 패널은 Esc로 닫지 않음) |
| 상세 화면 | j/k = 스크롤, Ctrl+D/Ctrl+U = 반 페이지, g/G = 처음/끝, `t` = 관계 메뉴, `u` = 본문 링크·이미지 목록, `r` = 새로고침, 목록 모드와 같은 동작 키, Esc = 뒤로(관계 이슈에서 왔으면 앞 이슈로), `q` = 닫기 |
| 마우스 | 휠 = 목록 이동·상세 스크롤·메뉴 이동, 줄 클릭 = 선택(선택된 줄을 다시 클릭하면 상세), 탭 이름 클릭 = 보기 전환, 메뉴 항목 클릭 = 실행, 상세의 관계 줄 클릭 = 그 이슈 열기, 누를 수 있는 곳에 올리면 옅게 밝힘(고른 줄은 선택 색 그대로) |
| 메뉴(상태·라벨·담당자·액션) | 글자 입력 = 거르기, ↑/↓ = 이동, Enter = 적용, Space = 켜기/끄기(라벨), Esc = 취소 |
| 입력(코멘트·새 이슈·에이전트 전달) | Ctrl+S = 보내기, Ctrl+E = 외부 편집기, Tab = 다음 칸(새 이슈), Esc = 취소(내용이 있으면 확인) |

- **Ctrl+K 액션 메뉴**는 모든 모드에서 동작한다. 현재 선택한 이슈에 할 수 있는 모든 동작을 퍼지로 골라 실행한다. Ctrl 조합은 IME를 거치지 않으므로 한글 입력 중에도 항상 쓸 수 있다.
- **두벌식 자모 매핑**: 목록·상세 모드에서 한 글자 키가 한글 자모로 들어오면 같은 영문 키로 처리한다.
  - ㄴ→s, ㅣ→l, ㅁ→a, ㅊ→c, ㅔ→p, ㅜ→n, ㅐ→o, ㅛ→y, ㅓ→j, ㅏ→k, ㅎ→g, ㄱ→r, ㅕ→u, ㅂ→q, ㅅ→t
  - IME로는 대문자를 구분할 수 없어서 `Y`와 `G`는 영문 입력에서만 동작한다.

## 5. markdown 렌더링

pulldown-cmark(GFM 표·취소선·체크박스 옵션 켬)로 파싱해 ratatui `Text`로 그린다.

| 요소 | 표시 |
|---|---|
| 제목 h1~h3 | 굵게 + 강조색, h1·h2 아래에 가는 선 |
| 제목 h4~h6 | 굵게 |
| 굵게 / 기울임 / 취소선 | 터미널 속성 그대로 |
| 인라인 코드 | 배경색 |
| 코드 블록 | 배경색 블록, 들여쓰기 유지, 문법 강조 없음 |
| 인용 | 왼쪽 `▌` 막대 + 흐린 색 |
| 목록 | 중첩 들여쓰기, `•` / `1.` |
| 체크박스 | `☐` / `☑` |
| 표 | 박스 문자. 열 폭은 표시 폭 기준이고, 넘치면 `…`로 자른다 |
| 링크 | 밑줄 텍스트 + 번호 `[1]` |
| 이미지 | `[이미지 2: alt 또는 파일명]` |
| 가로선 | 전체 폭 가는 선 |
| 이슈 식별자(`ENG-123`) | 강조색 |

- 폭 계산에는 unicode-width를 쓴다. 줄바꿈은 단어 단위로 하되, 한글은 글자 단위로 끊을 수 있다.
- 링크와 이미지에는 등장 순서대로 번호를 매긴다. 상세 화면에서 `u`를 누르면 그 목록이 나오고, 고른 것을 브라우저로 연다.
- 모르는 문법은 원문 텍스트로 보여준다.

## 6. 데이터

### 6.1 파일

| 파일 | 위치 | 내용 |
|---|---|---|
| `config.toml` | `HERDR_PLUGIN_CONFIG_DIR` | 사용자 설정(11장) |
| `credentials` | `HERDR_PLUGIN_CONFIG_DIR`, 권한 0600 | API 키 한 줄 |
| `cache.db` | `HERDR_PLUGIN_STATE_DIR`, 권한 0600 | SQLite 캐시(WAL 모드) |
| `side-panes.json` | `HERDR_PLUGIN_STATE_DIR`, 권한 0600 | workspace id → 사이드 패널 pane id. 임시 파일에 쓰고 이름을 바꾼다. 읽지 못하면 빈 기록으로 본다 |
| `herdr-linear.log` | `HERDR_PLUGIN_STATE_DIR` | 로그. 키는 남기지 않는다. 1MB를 넘으면 `.1`로 하나 백업하고 새로 쓴다 |

herdr 밖에서 실행할 때(개발·테스트)는 `HERDR_LINEAR_CONFIG_DIR`과 `HERDR_LINEAR_STATE_DIR` 환경 변수를 쓴다. 그것도 없으면 `~/.config/herdr-linear`와 `~/.local/state/herdr-linear`를 쓴다.

### 6.2 캐시 스키마

```sql
-- PRAGMA user_version = 2. 1이면 relations 테이블만 더하고, 그 밖의 다른 값이면 파일을 지우고 새로 만든다.
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,   -- org_id, viewer(JSON), teams(JSON), last_team_id
  value TEXT NOT NULL
);
CREATE TABLE issues (
  id          TEXT PRIMARY KEY,
  identifier  TEXT NOT NULL UNIQUE,
  team_id     TEXT NOT NULL,
  number      INTEGER NOT NULL,
  title       TEXT NOT NULL,
  description TEXT,
  state_type  TEXT NOT NULL,
  priority    INTEGER NOT NULL,
  assignee_id TEXT,
  updated_at  TEXT NOT NULL,
  data        TEXT NOT NULL,     -- IssueFields 전체 JSON
  fetched_at  INTEGER NOT NULL,  -- unix ms
  viewed_at   INTEGER            -- 상세 화면을 연 시각, unix ms
);
CREATE TABLE comments (
  issue_id   TEXT PRIMARY KEY,
  data       TEXT NOT NULL,      -- 코멘트 배열 JSON
  fetched_at INTEGER NOT NULL
);
CREATE TABLE view_results (
  view_key   TEXT PRIMARY KEY,   -- "mine", "all"
  issue_ids  TEXT NOT NULL,      -- 순서가 있는 id 배열 JSON
  fetched_at INTEGER NOT NULL
);
CREATE TABLE team_refs (
  team_id    TEXT PRIMARY KEY,
  data       TEXT NOT NULL,      -- 상태·라벨·멤버·기본 상태 JSON
  fetched_at INTEGER NOT NULL
);
CREATE TABLE workspace_labels (
  id         INTEGER PRIMARY KEY CHECK (id = 1),
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE branch_map (
  repo       TEXT NOT NULL,
  branch     TEXT NOT NULL,
  identifier TEXT,               -- NULL이면 연결된 이슈 없음
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY (repo, branch)
);
CREATE TABLE relations (
  issue_id   TEXT PRIMARY KEY,
  data       TEXT NOT NULL,      -- 하위·막힘·막는 중·관련 JSON
  fetched_at INTEGER NOT NULL
);
```

### 6.3 갱신 규칙 (stale-while-revalidate)

| 데이터 | 즉시 보여주는 것 | 다시 불러오는 때 |
|---|---|---|
| 보기 탭 목록 | `view_results` | 탭을 열 때. 사이드 패널은 `side.refresh_seconds`마다. `r`을 누를 때 |
| 검색 결과 | 캐시된 이슈 | 입력이 300ms 멈출 때 |
| 이슈 상세·코멘트·관계 | `issues`, `comments`, `relations` | 상세 화면을 열 때, `r`을 누를 때 |
| 팀 참조(상태·라벨·멤버)와 워크스페이스 라벨 | `team_refs`, `workspace_labels` | 변경 메뉴를 열 때 60분이 지났으면 |
| viewer와 팀 목록 | `meta` | 앱을 시작할 때 60분이 지났으면 |

- 변경 API 응답으로 받은 이슈와 코멘트는 바로 캐시에 덮어쓴다.
- 다시 불러온 이슈가 보관(`archivedAt`)이나 휴지통(`trashed`) 상태이거나 찾을 수 없으면, 캐시에서 지우고 "보관되었거나 삭제된 이슈"라고 표시한다.
- 앱을 시작할 때 `max(fetched_at, viewed_at)`이 `cache.retention_days`(기본 30일)보다 오래된 이슈와 그 코멘트·관계를 지운다. `view_results`에 남은 없는 id는 읽을 때 건너뛴다.
- 키의 워크스페이스(`organization.id`)가 저장된 `org_id`와 다르면 캐시를 모두 비운다.
- 팔레트와 사이드 패널이 같은 DB를 함께 쓰므로 WAL과 `busy_timeout` 5초를 쓴다.

### 6.4 범위 팀

- 기본은 내가 속한 모든 팀(`viewer.teams`)이다.
- config의 `teams = ["ENG", "OPS"]`로 좁힐 수 있다.
- 범위 팀은 "전체" 탭과 서버 검색에 적용된다. `#KEY` 토큰으로는 범위 밖 팀도 지정할 수 있다.

## 7. Linear API

### 7.1 클라이언트

- 요청: `POST https://api.linear.app/graphql`. 헤더는 `Authorization: <API 키>`(Bearer 없이)와 `Content-Type: application/json`이다.
- 타임아웃: 연결 5초, 전체 15초.
- 오류 분류
  - `errors[].extensions.code`가 `RATELIMITED`(HTTP 400) → 한도 초과
  - `AUTHENTICATION_ERROR` → 인증 실패
  - 그 밖의 GraphQL 오류 → 메시지를 그대로 표시
  - 연결 실패·타임아웃 → 오프라인
- 응답 헤더 `X-RateLimit-Requests-Remaining`, `X-RateLimit-Requests-Reset`, `X-Complexity`를 기록한다.
- 남은 요청이 50 미만이면 리셋 시각까지 자동 요청(사이드 패널 주기 새로고침, 서버 검색)을 멈춘다. 사용자가 직접 한 동작만 보낸다.

### 7.2 쿼리

| 용도 | 연산 |
|---|---|
| 키 검증·내 정보 | `viewer { id name displayName email organization { id name urlKey } teams { nodes { id key name } } }` |
| 내 이슈 | `issues(filter: { assignee: { isMe: { eq: true } }, state: { type: { nin: ["completed","canceled","duplicate"] } } }, orderBy: updatedAt, first: 50)` |
| 전체 | `issues(filter: { team: { id: { in: <범위 팀 id> } } }, orderBy: updatedAt, first: 50, after: <커서>)` |
| 검색 | `issues(filter: { and: [<텍스트 조건>, <토큰 조건>, <범위 팀 조건>] }, orderBy: updatedAt, first: 50)` |
| 깊은 검색 | `searchIssues(term: <텍스트>, includeComments: true, first: 20, filter: <토큰 조건>)` |
| 상세 | `issue(id: <식별자>) { ...IssueFields comments(first: 50) { nodes { id body createdAt editedAt user { id name displayName } } pageInfo { hasNextPage } } children(first: 50) { … } relations(first: 50) { … } inverseRelations(first: 50) { … } }`. 관계 필드는 `2026-10-07-issue-relations-design.md` 4.1을 따른다 |
| 브랜치 | `issueVcsBranchSearch(branchName: <브랜치>) { ...IssueFields }` |
| 팀 참조 | `team(id: <팀>) { defaultIssueState { id } states { nodes { id name type color position } } labels(first: 250) { nodes { id name color isGroup parent { id } } } members(first: 250) { nodes { id name displayName active } } }` |
| 워크스페이스 라벨 | `issueLabels(filter: { team: { null: true } }, first: 250) { nodes { id name color isGroup parent { id } } }` |

`IssueFields`는 다음과 같다.

```graphql
fragment IssueFields on Issue {
  id identifier number title description priority estimate url branchName
  dueDate createdAt updatedAt archivedAt trashed
  team { id key name }
  state { id name type color }
  assignee { id name displayName }
  project { id name }
  cycle { id number name }
  parent { id identifier title state { id name type color } }
  labels(first: 20) { nodes { id name color } }
  attachments(first: 10) { nodes { url sourceType createdAt metadata } }
}
```

열린 PR은 첨부 가운데 GitHub `…/pull/<번호>`(GitLab `…/merge_requests/<번호>`) 링크이면서 `metadata.status`가 `open`인 것이다. 여러 개면 가장 최근에 연결된 것을 쓴다. `metadata`는 연동마다 모양이 달라서 `status`, `number`, `repoName`, `draft`만 너그럽게 읽는다. 타입이 맞지 않으면 그 값만 버린다.

50건짜리 페이지의 복잡도는 구현 중에 `X-Complexity` 헤더로 실측한다. 5,000점 아래인지 확인하고(쿼리당 한도는 10,000점), 넘으면 `labels` 개수를 줄인다.

### 7.3 검색어 → 필터 변환

| 입력 | `IssueFilter` 조각 |
|---|---|
| 텍스트 단어 `w` | `{ or: [ { title: { containsIgnoreCase: w } }, { description: { containsIgnoreCase: w } } ] }`. 단어마다 하나씩 만들어 AND로 묶는다 |
| 숫자 `n` | 위 조건에 `{ number: { eq: n } }`을 OR로 추가 |
| `ABC-123` | `{ team: { key: { eqIgnoreCase: "ABC" } }, number: { eq: 123 } }` |
| `s:값` | 상태 타입 키워드면 `{ state: { type: { eq: <타입> } } }`, 아니면 `{ state: { name: { containsIgnoreCase: 값 } } }` |
| `l:값` | `{ labels: { some: { name: { containsIgnoreCase: 값 } } } }` |
| `@나`, `@me` | `{ assignee: { isMe: { eq: true } } }` |
| `@값` | `{ assignee: { or: [ { name: { containsIgnoreCase: 값 } }, { displayName: { containsIgnoreCase: 값 } } ] } }` |
| `#KEY` | `{ team: { key: { eqIgnoreCase: KEY } } }` |
| `p:값` | `{ priority: { eq: <0~4> } }` |

상태 타입 키워드는 다음과 같다. 로컬 필터도 같은 규칙을 쓴다.

| 키워드 | 상태 타입 |
|---|---|
| `triage`, `분류` | triage |
| `backlog`, `백로그` | backlog |
| `unstarted`, `todo`, `할일` | unstarted |
| `started`, `progress`, `진행` | started |
| `completed`, `done`, `완료` | completed |
| `canceled`, `취소` | canceled |

### 7.4 변경

| 동작 | mutation |
|---|---|
| 상태 | `issueUpdate(id, input: { stateId })` |
| 담당자 | `issueUpdate(id, input: { assigneeId })`. 할당 해제는 `null` |
| 라벨 | `issueUpdate(id, input: { addedLabelIds, removedLabelIds })` |
| 코멘트 | `commentCreate(input: { issueId, body })` |
| 새 이슈 | `issueCreate(input: { teamId, title, description, stateId, assigneeId, labelIds })`. 비어 있는 칸은 보내지 않는다 |

모든 mutation은 `success`와 갱신된 이슈(`IssueFields`) 또는 코멘트를 돌려받는다. 받은 값은 바로 캐시에 쓴다.

## 8. 변경 동작 UX

- **낙관적 반영**: 화면과 캐시에 먼저 반영한다. 실패하면 이전 값으로 되돌리고 하단에 사유를 보여준다.
- **상태 메뉴**: 팀 상태를 Linear 워크플로 순서(triage → backlog → unstarted → started → completed → canceled → duplicate)로 묶고, 묶음 안에서는 `position` 순으로 보여준다. 현재 상태를 표시하고, 타이핑으로 거르고, Enter로 적용한다.
- **라벨 메뉴**
  - 팀 라벨과 워크스페이스 라벨을 함께 보여준다.
  - 그룹 라벨(`isGroup`)은 제목으로만 보이고 고를 수 없다.
  - 같은 그룹의 하위 라벨은 하나만 켜진다. 하나를 켜면 형제 라벨은 꺼진다.
  - Space로 켜고 끈 뒤 Enter를 누르면 추가·제거분만 보낸다.
- **담당자 메뉴**: 맨 위에 "나"와 "할당 해제", 그 아래에 활성 팀 멤버를 보여준다.
- **코멘트**
  - 상세 화면 아래 입력창(tui-textarea)에 쓴다.
  - Ctrl+S로 보낸다. 보내기에 성공하면 코멘트 목록 끝에 붙인다.
  - Ctrl+E를 누르면 `$VISUAL` 또는 `$EDITOR`(둘 다 없으면 `vi`)에서 편집하고 돌아온다.
  - Esc는 취소이고, 내용이 있으면 확인을 받는다.
- **새 이슈**
  - 칸: 팀, 제목(필수), 본문, 상태(기본: 팀 기본 상태), 담당자(기본: 없음), 라벨. Tab으로 칸을 옮긴다.
  - Ctrl+S로 만들면 새 이슈 상세 화면으로 이동하고 "ENG-201 생성됨"을 표시한다.
  - 팀 기본값은 현재 브랜치 이슈의 팀, 없으면 `last_team_id`, 그것도 없으면 범위 팀의 첫 번째다.
- **브라우저 열기**: macOS는 `open`, Linux는 `xdg-open`을 쓴다.
- **복사**
  - macOS는 `pbcopy`를 쓴다. Linux는 `wl-copy`(Wayland), `xclip -selection clipboard`, `xsel -b` 순으로 있는 것을 쓴다.
  - OSC 52도 함께 출력한다.
  - 결과는 하단에 "복사됨: ENG-131"로 표시한다.

## 9. 에이전트 전달 (`p`)

1. **대상**
   - 3.4의 기본 대상을 쓴다.
   - 기본 대상이 없거나 사용자가 바꾸려 하면 `herdr agent list`로 목록을 보여준다. 현재 workspace의 에이전트가 먼저 오고 나머지가 뒤에 온다.
2. **편집**
   - `agent.template`으로 채운 편집창이 열린다. 맨 위 지시 칸에 한 줄을 쓸 수 있다.
   - Ctrl+E로 외부 편집기를 쓸 수 있다.
3. **전송**
   - Ctrl+S를 누르면 `herdr agent prompt <대상> <텍스트>`로 보낸다. herdr가 bracketed paste와 Enter를 한 번에 보내므로 여러 줄도 중간에 전송되지 않는다.
   - `agent_blocked`면 "에이전트가 확인을 기다리는 중이에요"를 보여주고 보내지 않는다.
4. **마무리**
   - 성공하면 `herdr agent focus <대상>`으로 포커스를 옮긴다.
   - 팔레트는 닫고, 사이드 패널은 열어둔다.

기본 템플릿은 다음과 같다.

```
{instruction}

Linear 이슈 {identifier}: {title}
{url}
상태: {state} · 우선순위: {priority} · 라벨: {labels}

{description}
{comments}
```

- `{instruction}`이 비어 있으면 그 줄과 바로 다음 빈 줄을 뺀다.
- `{comments}`에는 최근 코멘트 `agent.include_comments`개(기본 0)를 `작성자: 본문` 형식으로 넣는다. 0이면 빈 문자열이다.

## 10. 인증

- 키를 찾는 순서: `LINEAR_API_KEY` 환경 변수 → `credentials` 파일.
- `LINEAR_API_KEY`가 설정돼 있는데 인증이 실패하면, 키 입력 화면 대신 "LINEAR_API_KEY 환경 변수의 키가 유효하지 않아요"를 보여준다. 환경 변수가 항상 우선하므로, 새 키를 받아 저장해도 쓰이지 않기 때문이다.
- 환경 변수가 없고, `credentials`가 없거나 그 키로 인증이 실패하면 키 입력 화면을 띄운다.
  - 발급 위치를 안내한다: Linear → Settings → Security & access → Personal API keys.
  - 입력은 가려서 보여준다.
  - Enter를 누르면 `viewer` 쿼리로 키를 검증한다.
- 검증에 성공하면 "{viewer.name}님, {organization.name} 워크스페이스에 연결됐어요"를 띄운다. 그다음 `credentials`에 권한 0600으로 저장하고 "내 이슈"로 넘어간다.
- 키는 Authorization 헤더에만 쓴다. 로그, 화면, 에러 메시지에는 남기지 않는다.

## 11. 설정 (`config.toml`, 모든 항목 선택)

```toml
teams = []                 # 비우면 내가 속한 팀 전부

[side]
refresh_seconds = 60       # 사이드 패널 자동 새로고침 주기(0~3600초). 0이면 끔

[cache]
retention_days = 30

[agent]
include_comments = 0
template = """
{instruction}

Linear 이슈 {identifier}: {title}
{url}
상태: {state} · 우선순위: {priority} · 라벨: {labels}

{description}
{comments}
"""
```

파일이 없으면 기본값을 쓴다. 값이 잘못되면 그 항목만 기본값을 쓰고 하단에 경고를 한 번 보여준다.

## 12. 에러 처리

| 상황 | 동작 |
|---|---|
| 네트워크 끊김·타임아웃 | 캐시로 계속 조회하고 상단에 "오프라인" 표시. 다음 조회 때 다시 시도 |
| 인증 실패 | 키 입력 화면으로 이동하고 "키가 만료됐거나 권한이 없어요" 표시 |
| 한도 초과 | 리셋 시각까지 자동 요청을 멈추고, 하단에 3초 동안 언제 다시 시도할지 알린다 |
| 변경 실패 | 화면과 캐시를 되돌리고 사유 표시(예: 권한 없음) |
| 이슈 없음·보관·휴지통 | 캐시에서 지우고 "보관되었거나 삭제된 이슈" 표시 |
| herdr 명령 실패 | 하단 메시지 표시 + 로그 |
| 캐시 DB 열기 실패·손상 | 파일을 지우고 새로 만든 뒤 계속 |
| 비정상 종료(panic) | 패닉 훅에서 터미널 상태를 복구하고 로그를 남긴 뒤 종료 |

## 13. 코드 구조

```
Cargo.toml
herdr-plugin.toml
src/
  main.rs            CLI 진입점: open {palette|side|url}, ui --mode {palette|side}, logout
  config.rs          경로 결정, config.toml 읽기, API 키 읽기·저장
  context.rs         호출 맥락(env) 읽기, 브랜치 → 이슈
  herdr.rs           herdr CLI 래퍼 (HERDR_BIN_PATH, --json 출력 파싱)
  linear/
    client.rs        HTTP 전송, 오류 분류, 한도 추적
    queries.rs       쿼리·mutation 문자열과 응답 타입
    filter.rs        검색어 → IssueFilter JSON
  store/
    mod.rs           SQLite 열기, 스키마, 정리
    cache.rs         이슈·코멘트·보기 결과·참조 데이터 읽기/쓰기
  search/
    query.rs         검색어 파서(텍스트 + 토큰)
    rank.rs          로컬 매칭(nucleo), 정렬, 결과 합치기
  markdown.rs        pulldown-cmark → ratatui Text
  tui/
    app.rs           앱 상태와 이벤트 루프
    keys.rs          키 해석(자모 매핑 포함)
    views/           목록, 미리보기, 상세, 메뉴, 입력 폼, 키 입력 화면
tests/
```

- **스레드**: 메인 스레드는 이벤트 루프, 화면, DB를 맡는다. 네트워크 요청은 작업 스레드에서 실행하고 결과를 채널로 돌려준다. 그래서 요청 중에도 화면이 멈추지 않는다.
- **모듈 경계**: `linear`는 화면을 모르고, `tui`는 HTTP를 모른다. `search`, `markdown`, `linear::filter`는 순수 함수라 단독으로 테스트한다.

## 14. 테스트

| 대상 | 방법 |
|---|---|
| `linear::client` | 로컬 가짜 HTTP 서버로 성공, 페이지 넘김, `RATELIMITED`(HTTP 400), `AUTHENTICATION_ERROR`, 타임아웃, 헤더 기반 한도 추적 |
| `linear::filter` | 검색어별 IssueFilter JSON 스냅샷 |
| `store` | 저장·덮어쓰기, 30일 정리, 보관 이슈 제거, 스키마 버전 불일치 시 재생성, 워크스페이스 변경 시 비우기 |
| `search` | 토큰 해석(따옴표, 한글 키워드), 순위 규칙, NFC 정규화, 결과를 합칠 때 선택 유지, 5천 건 기준 키 입력당 16ms 이내 |
| `markdown` | 제목, 목록, 체크박스, 코드, 표(한글 폭), 링크·이미지 번호, 모르는 문법 스냅샷 |
| `herdr` | 가짜 `HERDR_BIN_PATH` 스크립트가 받은 인자를 기록해 정확한 호출 검증 |
| `context` | 브랜치명 패턴, 선택 텍스트, 컨텍스트 JSON 해석 |
| `tui` | ratatui `TestBackend` 화면 스냅샷과 키 입력 시나리오(검색 → 상세 → 상태 변경, 자모 키) |

수동 확인은 `herdr plugin link`로 붙인 뒤 실제 Linear 키로 체크리스트를 돌린다. 체크리스트는 팔레트, 사이드 패널, 링크 클릭, 각 변경 동작, 에이전트 전달, 오프라인을 다룬다.

## 15. 구현 초기에 확인할 가정

아래는 공식 문서·스키마로는 확정할 수 없어서, 구현 계획의 첫 작업에서 직접 확인한다. 가정과 다르면 오른쪽 대안으로 바꾼다.

| 가정 | 확인 방법 | 다를 때 |
|---|---|---|
| 액션 → `plugin pane open` 흐름에서 `--env`가 pane 프로세스까지 전달된다 | 최소 플러그인으로 env 덤프 | pane에서 `HERDR_PLUGIN_CONTEXT_JSON`을 직접 읽음 |
| 액션 컨텍스트에 `focused_pane_agent`, `focused_pane_cwd`, `selected_text`가 채워진다 | 같은 최소 플러그인 | `herdr pane get`·`agent list`로 보완 |
| split 사이드 패널 프로세스가 `HERDR_PANE_ID`를 받는다 | env 덤프 | `herdr pane current`로 조회 |
| 같은 탭의 왼쪽 pane을 찾을 수 있다 | `herdr pane neighbor` 사용법 확인 | 같은 탭의 에이전트 목록에서 고르기 |
| herdr가 pane의 OSC 52를 바깥 터미널로 넘긴다 | 로컬·원격 세션에서 복사 시험 | 로컬 명령(`pbcopy` 등)만 사용 |
| `herdr agent prompt`의 대상에 pane id를 쓸 수 있다 | 실제 에이전트 pane에 시험 전송 | `agent list` 결과의 대상 형식 사용 |
| `issues(orderBy: updatedAt)`가 최근 수정 순이다 | 실제 키로 조회 | 클라이언트에서 다시 정렬 |
| `containsIgnoreCase`가 한글에서 동작한다 | 실제 키로 조회 | 로컬 결과만 쓰고 깊은 검색을 안내 |
| `team.labels`에 워크스페이스 라벨이 들어 있지 않다 | 두 쿼리 결과 비교 | id 기준으로 중복 제거 |
| 같은 그룹 하위 라벨은 하나만 붙는다(Linear 규칙) | 실제 키로 `issueUpdate` 시험 | 메뉴의 형제 끄기 규칙을 API 동작에 맞춤 |
| `issue(id:)`가 보관·휴지통 이슈를 `archivedAt`·`trashed`와 함께 돌려준다 | 실제 키로 조회 | "찾을 수 없음" 오류도 같은 처리 |
| tui-textarea에서 한글 IME 입력과 커서 폭이 맞다 | 실제 herdr pane에서 입력 | 입력 위젯 교체 또는 폭 보정 |

## 16. 구현 순서 (제안)

각 단계가 끝날 때마다 실제로 써볼 수 있게 나눈다.

1. **가정 확인**: 15장 항목을 최소 플러그인과 실제 키로 확인한다.
2. **기반**: `config`, `linear::client`, 인증(키 입력·검증·저장), `store`
3. **읽기 경로**: 검색어 파서와 필터 변환, 로컬 순위, markdown 렌더러
4. **팔레트**: 보기 탭, 검색(로컬 + 서버), 미리보기, 상세 화면 → 여기서 "빠른 조회"가 완성된다
5. **herdr 연결**: 매니페스트, `open palette`·`open url`, 브랜치 → 이슈, 키 바인딩 안내
6. **사이드 패널**: `open side` 토글, 주기 새로고침
7. **변경 동작**: 상태·라벨·담당자, 코멘트, 새 이슈, 복사·브라우저
8. **에이전트 전달**
9. **마무리**: 에러 처리 점검, 수동 체크리스트, README(설치·키 바인딩)

## 17. 나중에 (v1 이후)

- **공개**
  - GitHub 저장소(`jenthous/herdr-linear` 또는 다른 이름)
  - 릴리스 바이너리와 체크섬. `[[build]]`는 바이너리를 받아오고, 실패하면 cargo로 빌드한다.
  - herdr 마켓플레이스 등록
- **기능**
  - 에이전트 작업 결과를 Linear 코멘트·상태에 반영
  - worktree 생성
  - 초성 검색
  - 우선순위 등 추가 변경
  - 코드 블록 문법 강조
