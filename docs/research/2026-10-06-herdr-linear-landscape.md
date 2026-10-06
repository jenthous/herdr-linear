# herdr × Linear 연동 리서치 (2026-10-06)

> 목적: "API 키만 넣으면 동작하는 herdr용 Linear 완전 연동 플러그인" 설계 근거 수집
> 방법: deep-research 워크플로(5개 검색 축 → 20개 출처 → 100개 주장 추출 → 상위 25개 3표 교차검증: 21 통과 / 4 반박)
> + 로컬 herdr 0.9.3 직접 조사 + 설계 핵심 주장 1차 출처 직접 재확인

**검증 표기**
- `검증`: 3표 교차검증 통과
- `직접확인`: 1차 출처 원문에서 직접 확인
- `로컬확인`: 이 머신의 herdr 0.9.3(API protocol 22)에서 확인
- `미검증`: 수집만 되고 검증 단계를 거치지 않음. 설계에 쓰기 전에 다시 확인 필요

---

## TL;DR

1. **herdr에는 정식 플러그인 시스템(v1)이 있다.** `herdr-plugin.toml` 매니페스트 + 프로세스 밖에서 도는 명령으로 구성되고 언어 제약이 없다. SDK는 따로 없고 **herdr CLI 전체가 곧 플러그인 API**다. (`검증`)
2. **이미 herdr-Linear 플러그인이 10개 이상 있다.** 다만 지배적인 것은 없다(최다 19★). "이슈 → worktree → 에이전트 시작"까지는 최소 5번 구현됐다. (`검증`/`미검증` 혼재)
3. **빈틈은 "그 이후"다.** 에이전트 상태 → Linear 상태·코멘트 반영, PR 연결, 브랜치 → 이슈 역매핑, 정리 같은 **생명주기 동기화**를 하는 API 키 기반 플러그인은 없다. 유일하게 동기화하는 `civitaspo/herdr-linear-agent`는 워크스페이스 관리자가 만든 **OAuth 앱이 필요**하다.
4. **로컬 플러그인은 Linear webhook을 받을 수 없다.** 공개 HTTPS 주소와 관리자 권한이 필요하다. 그래서 Linear → herdr 방향은 **폴링**해야 하고, API 키 한도(시간당 2,500회, 사용자 단위로 공유) 안에서 설계해야 한다. (`직접확인`)
5. **`openworktree`가 무엇인지 확인이 필요하다.**
   - GitHub의 `Saktawdi/OpenWorktree`: Java·React 기반 티켓 작업대. Linear 연동이 없고 macOS 바이너리도 없다.
   - herdr 내장 기능: `open_worktree` 키 액션, `herdr worktree open`, `worktree.opened` 이벤트. 가리키는 대상이 이쪽일 가능성이 높다.

---

## 1. herdr 플러그인 시스템

### 1.1 구조 (`검증` + `로컬확인`)

| 매니페스트 블록 | 하는 일 | 비고 |
|---|---|---|
| 최상위 `id`, `name`, `version`, `min_herdr_version`, `platforms` | 메타데이터 | `min_herdr_version` 필수 |
| `[[build]]` | 설치 후 빌드(신뢰 미리보기 후 실행) | 사전 빌드 바이너리를 받고, 실패하면 cargo/go로 빌드하는 패턴이 흔함 |
| `[[startup]]` | 세션 복원 후 1회 실행 | **감독되는 데몬이 아님.** 상태를 복원하고 종료해야 함 (`직접확인`) |
| `[[actions]]` | 명령 팔레트·키로 실행하는 동작 | `contexts = ["global","workspace","pane"]` |
| `[[events]]` | herdr 이벤트에 반응 | `on = "worktree.created"` 등 |
| `[[panes]]` | 플러그인 전용 터미널 UI | `placement = overlay / popup / split / tab` |
| `[[link_handlers]]` | 터미널 URL을 Ctrl+클릭하면 정규식에 맞는 액션으로 보냄 | linear.app URL에 활용 가능 |

- **플러그인은 키를 직접 바인딩할 수 없다.** 사용자가 `config.toml`에 `[[keys.command]] type = "plugin_action" command = "<plugin_id>.<action_id>"`를 추가해야 한다. (`검증`)
- 설치: `herdr plugin install owner/repo[/subdir] [--ref]`(GitHub만 지원). 로컬 개발: `herdr plugin link <dir>`. (`검증`)
- **UI는 터미널 전용이다.** v1에는 네이티브 UI도, 런타임 액션 등록도 없다. (`검증`)

### 1.2 런타임 환경 변수 (`검증` + `직접확인`)

- 공통: `HERDR_BIN_PATH`, `HERDR_SOCKET_PATH`, `HERDR_PLUGIN_ID`, `HERDR_PLUGIN_ROOT`, `HERDR_PLUGIN_CONFIG_DIR`, `HERDR_PLUGIN_STATE_DIR`, `HERDR_PLUGIN_CONTEXT_JSON`. 가능하면 `HERDR_WORKSPACE_ID`, `HERDR_TAB_ID`, `HERDR_PANE_ID`도 들어온다.
- 액션: `HERDR_PLUGIN_ACTION_ID`
- 이벤트·startup 훅: `HERDR_PLUGIN_EVENT`. startup 훅에서는 값이 `startup`이다.
- 이벤트 훅에만: `HERDR_PLUGIN_EVENT_JSON`
- pane 명령: `HERDR_PLUGIN_ENTRYPOINT_ID`
- `HERDR_PLUGIN_CONTEXT_JSON`에는 workspace, tab, focused pane, worktree, agent, 선택한 텍스트, 클릭한 URL이 들어갈 수 있다.
- **비밀값은 `HERDR_PLUGIN_CONFIG_DIR`에 둔다**(`herdr plugin config-dir <id>`). `HERDR_PLUGIN_ROOT`는 관리되는 checkout이라 금지다. herdr가 관리하는 저장소·비밀값 API는 없으므로 형식은 플러그인이 정한다.
- 플러그인 명령은 PATH가 최소한으로 잡혀 실행된다. reviewr는 `/opt/homebrew/bin` 등을 직접 추가한다. (`로컬확인`)

### 1.3 연동에 쓸 herdr 기본 기능 (`검증` + `로컬확인`)

| 기능 | 명령 / API | 용도 |
|---|---|---|
| worktree 생성 | `herdr worktree create --cwd <repo> --branch <b> --base <ref> --label <t> --focus --json` | 브랜치가 있으면 checkout, 없으면 base 또는 HEAD에서 생성. 결과 workspace는 부모 레포 아래로 묶임. 기본 경로는 `<worktrees.directory>/<repo>/<branch-slug>` |
| worktree 열기 | `herdr worktree open --branch/--path` | 이미 열려 있으면 그 workspace를 반환 |
| worktree 삭제 | `herdr worktree remove [--force]` | 브랜치는 지우지 않음 |
| 에이전트 시작 | `herdr agent start <name> --kind <claude\|codex\|...> --pane <id>` | 빈 셸 pane이 먼저 있어야 함. 24종 지원 |
| 프롬프트 전달 | `herdr agent prompt <target> <text> --wait --until done` | 상태: idle / working / blocked / done / unknown |
| 사이드바 표시 | `herdr workspace report-metadata --source <id> --token NAME=VALUE` | `[ui.sidebar.spaces] rows`에서 `$NAME`으로 표시 |
| 알림 | `herdr notification show` | 토스트 |
| 플러그인 pane | `herdr plugin pane open --plugin <id> --entrypoint <pane>` | 선택기 UI |
| 이벤트 스트림 | 소켓 `events.subscribe` / `events.wait` | 아래 이벤트 목록 |

**구독 가능한 주요 이벤트** (`로컬확인`, API 스키마 기준):
- pane: `pane.agent_status_changed`, `pane.agent_detected`, `pane.created`, `pane.closed`, `pane.exited`, `pane.output_matched`
- worktree: `worktree.created`, `worktree.opened`(`data.already_open` 포함), `worktree.removed`
- workspace: `workspace.created`, `workspace.closed`, `workspace.focused`, `workspace.metadata_updated`
- 그 외: `tab.*`, `layout.updated`

### 1.4 주의할 제약

- **에이전트 상태는 화면 내용을 보고 추정한 값이다.** 종료 코드나 신호는 노출되지 않아 크래시와 정상 종료를 구분할 수 없다. (`미검증`, talent-factory 문서)
- **이벤트 기록은 영속되지 않는다.** 구독자가 뒤처지면 `events_lost`를 받고, 재구독한 뒤 `session.snapshot`으로 상태를 다시 읽어야 한다. (`미검증`, socket-api 문서)
- herdr의 agent/pane/tab CLI 형식은 릴리스마다 바뀌어 왔다. → `min_herdr_version`을 명시하고 `--json` 출력을 방어적으로 파싱해야 한다. (`미검증`)

---

## 2. 기존 herdr-Linear 프로젝트 (경쟁 지형)

### 2.1 주요 5개

| 프로젝트 | 언어 | 인증 | 하는 일 | Linear에 쓰는 것 | 규모 |
|---|---|---|---|---|---|
| [tdi/herdr-worktree-from-linear](https://github.com/tdi/herdr-worktree-from-linear) `검증` | JS(Node, 의존성 0) | **API key**(config.json `linearApiKey`, 없으면 `LINEAR_API_KEY`) | 활성 이슈 선택 → 이슈 `branchName`으로 worktree 생성. 이미 있으면 열기 | — | **19★**, 8 fork, 라이선스 없음 |
| [talent-factory/herdr-linear](https://github.com/talent-factory/herdr-linear) `검증` | Rust | **API key**(config.toml `api_key` / `LINEAR_API_KEY`) | My·Project·Team Issues 패널(split/tab), 체크섬 검증된 사전 빌드 바이너리 | (세부 미검증) | v0.3.1(2026-10-04), 5★ |
| [mrolafsson/herdr-linear](https://github.com/mrolafsson/herdr-linear) `검증` | Go + Bubble Tea | **OAuth PKCE**(API key 미지원), keychain 저장 | popup 선택기. `w`: Linear `branchName`으로 worktree. `s`: 상태 started + 미할당이면 본인 할당 + 에이전트에 `/ticket ENG-123` 또는 기본 프롬프트 | **상태, 담당자만**. 코멘트·PR·후속 동기화 없음 | v0.6.2, 1★ |
| [zamarrowski/herdr-issues](https://github.com/zamarrowski/herdr-issues) `검증` | JS(Node 20+) | **API key**(`secrets.json` 0600. `LINEAR_API_KEY`가 우선하며 저장하지 않음). `viewer` 쿼리로 키 검증 | GitHub·Shortcut·Linear 탭 popup → worktree + `agent start` + 프롬프트 + 토스트 + 사이드바 `issue` 토큰 | (세부 미검증) | v0.1.0, 1★ |
| [civitaspo/herdr-linear-agent](https://github.com/civitaspo/herdr-linear-agent) `미검증` | Rust | **OAuth 앱**(actor=app, 관리자가 생성) | Linear에서 앱 사용자에게 **위임된** 이슈를 자동 수거 → 코디네이터 + 레포별 워커 에이전트. 워커마다 worktree | Agent Session 활동, PR 링크, 상태(started → In Review). Done으로는 옮기지 않음 | 3★. 가짜 Linear API로만 테스트됨 |

그 밖에 (`미검증`, GitHub `topic:herdr-plugin linear` 검색 결과 10개):
- `JacquesvanWyk/herdr-linear`: Shell + fzf, 8★
- `ZviBaratz/herdr-draft`: Go, 이슈·worktree·에이전트를 한 다이얼로그에서 고름
- `logocode/herdr-linear-launcher`: 보관됨
- `H3xept/herdr-ingest`: Sentry·Linear·GitHub 지원

### 2.2 빈틈 분석

- **API 키 방식 플러그인(tdi, talent-factory, zamarrowski)은 모두 "끌어와서 시작"까지만 한다.** 에이전트가 일하는 동안이나 끝난 뒤 Linear에 반영하는 기능은 확인되지 않았다.
- **mrolafsson**은 시작할 때 상태와 담당자만 쓴다. README에 "그 외에는 아무것도 바꾸지 않는다"고 명시돼 있다.
- **civitaspo**만 생명주기를 동기화한다(Agent Session, PR, In Review). 대신 OAuth 앱 생성(관리자)과 위임 기반 흐름이 필요하다. "API 키만 넣으면 됨"과 정반대다.
- talent-factory 로드맵의 "Sync Engine: herdr와 양방향 동기화" 항목은 미착수다. (`미검증`)

**→ 차별화 지점: "API 키 하나로 이슈 → worktree → 에이전트 → PR → 상태 동기화 전 구간을 잇는 것"은 아직 아무도 하지 않았다.**

### 2.3 반박된 주장 (사용하지 말 것)

- "mrolafsson은 소켓 API 6개 메서드로만 herdr와 통신한다" → 반박(0-3)
- "talent-factory가 Enter 키로 구현을 시작하고 In Progress로 이동한다" 같은 세부 → 반박(0-3)
- zamarrowski의 herdr 명령 사용 세부 목록 → 반박(0-3)

---

## 3. Linear API 연동 패턴

### 3.1 인증과 호출 (`검증`)

- 엔드포인트는 하나뿐이다: `POST https://api.linear.app/graphql`
- **Personal API key는 `Authorization: <key>`처럼 Bearer 없이 그대로 보낸다.** OAuth 토큰은 `Bearer <token>`.
- 키 발급 위치: Linear > Settings > Security & access > Personal API keys
- 키 검증: `query { viewer { id name email } }`
- 실제 API 키 기반 플러그인 3개(tdi, zamarrowski, talent-factory)는 **`@linear/sdk` 없이 raw GraphQL fetch**를 쓴다. Linear 공식 권장은 SDK다.

### 3.2 Rate limit (`직접확인`, https://linear.app/developers/rate-limiting)

| 인증 | 요청 한도 | 복잡도 한도 |
|---|---|---|
| API key | **시간당 2,500회, 사용자 단위**(같은 사용자의 모든 키가 공유) | 시간당 3,000,000점 |
| OAuth 앱 | 시간당 5,000회, 사용자 또는 앱 사용자 단위 | (미검증: 2,000,000점) |
| 비인증 | 시간당 600회, IP 단위 | 시간당 100,000점 |

- leaky bucket 방식이라 일정 속도로 채워진다(2,500/h ≈ 분당 41.7회).
- **rate limit에 걸리면 HTTP 400**이 오고, `errors[].extensions.code == "RATELIMITED"`로만 판별할 수 있다. 429가 아니다.
- 응답 헤더: `X-RateLimit-Requests-*`, `X-Complexity`, `X-RateLimit-Complexity-*`
- 단일 쿼리 복잡도 10,000점 초과는 거부된다. 페이지 크기가 곱해지므로 `first`를 작게 명시해야 한다. (`미검증`)

### 3.3 Webhook (`직접확인`, https://linear.app/developers/webhooks)

- 수신 주소는 **공개 HTTPS이면서 localhost가 아니어야** 한다.
- **워크스페이스 관리자 또는 admin scope를 가진 OAuth 앱만** webhook을 만들거나 읽을 수 있다.
- 5초 안에 200으로 응답해야 한다. 실패하면 1분, 1시간, 6시간 뒤 최대 3회 재시도하고, 계속 실패하면 비활성화될 수 있다.
- **→ 로컬 herdr 플러그인은 Linear → herdr 방향을 폴링으로 처리해야 한다.** Linear 권장 방식은 `updatedAt` 기준 정렬과 서버 측 필터이고, 이슈를 하나씩 폴링하는 것은 금지다. (`미검증`)

### 3.4 핵심 스키마 (`직접확인`, linear/linear `packages/sdk/src/schema.graphql`)

| 필드 / 연산 | 용도 |
|---|---|
| `Issue.branchName: String!` | 서버가 계산한 권장 브랜치명. `Organization.gitBranchFormat` 템플릿을 따름 → worktree 브랜치로 그대로 사용 |
| `issueVcsBranchSearch(branchName: String!)` | **브랜치 → 이슈 역조회**. 별도 로컬 매핑 없이 worktree를 이슈에 연결 |
| `issue(id: "ENG-123")` | UUID 대신 식별자로 조회 가능 (`검증`) |
| `issueUpdate(id, input: { stateId })` | 상태 변경. 팀별 `workflowStates`에서 `type`(triage / backlog / unstarted / started / completed / canceled / duplicate)으로 찾아야 함 (`미검증`) |
| `attachmentLinkGitHubPR(issueId, url)` | 이슈에 PR을 첨부 |
| `agentSessionCreateOnIssue` | Linear Agent Session. **앱 사용자(OAuth 앱)를 전제**로 하므로 API 키만으로는 부적합 (`미검증`) |
| `issueRepositorySuggestions` | 이슈에 맞는 레포를 LLM이 순위 매겨 추천 (`미검증`) |

### 3.5 GitHub 연동 활용 (`미검증`, https://linear.app/docs/github)

- Linear GitHub 연동이 설치돼 있으면 **브랜치명에 이슈 ID(ENG-123)가 들어간 PR은 자동으로 이슈에 연결**된다.
- 기본 자동화: PR이 열리면 In Progress, 머지되면 Done. 팀별로 설정할 수 있다.
- → `branchName`을 그대로 쓰면 PR 연결과 상태 전환을 상당 부분 Linear에 맡길 수 있다. mrolafsson이 이 전략을 쓴다(`검증`).

### 3.6 Linear MCP (`미검증`, https://linear.app/docs/mcp)

- 호스팅 MCP 서버 `https://mcp.linear.app/mcp`(Streamable HTTP), 읽기 전용 `/mcp/readonly`.
- `Authorization: Bearer <API key>` 방식도 허용된다. → **pane 안의 에이전트**가 Linear를 직접 읽고 쓰게 하는 보조 경로로 쓸 수 있다.

---

## 4. "openworktree"의 정체

### 4.1 Saktawdi/OpenWorktree (`미검증`, 연구 에이전트가 레포를 직접 조사)

- **가벼운 worktree 도우미가 아니다.** Java 17/Javalin 백엔드와 React 19 웹 UI(127.0.0.1:18080)로 된 **로컬 티켓 기반 에이전트 작업대**다.
- 흐름: 티켓 → 격리 worktree에서 에이전트 코딩 → presubmit 스냅샷 → 게이트 리뷰 → 사람이 승인해 publish
- 자체 SQLite 티켓 보드를 쓴다. **Linear·Jira·GitHub Issues 연동은 없다.**
- 외부에서 연결하려면 `POST /api/tickets`(loopback, `GATE_WEB_TOKEN`)를 써야 한다. 브랜치명은 슬래시 없는 `[A-Za-z0-9._-]+` 형식이어야 하므로 Linear 기본 브랜치명(`user/eng-123-...`)은 정규화가 필요하다.
- 에이전트는 Claude Code와 OpenCode만 지원한다.
- 2026-09-03 생성, 3★, v0.3.14-beta. **macOS 바이너리가 없다**(소스 빌드 또는 Docker).

### 4.2 herdr 내장 worktree 열기 (`로컬확인`)

- 키 액션 `open_worktree`(기본은 미지정), `new_worktree = "prefix+shift+g"`, `remove_worktree`
- CLI `herdr worktree open`, 이벤트 `worktree.opened`(이미 열린 경우 `data.already_open = true`)
- reviewr 플러그인이 `worktree.created`/`worktree.opened`에 반응해 pane을 자동으로 연다. → **"worktree를 열면 해당 Linear 이슈 컨텍스트를 자동 표시하거나 동기화"하는 연계는 이 이벤트로 바로 구현할 수 있다.**

---

## 5. 유사 도구 사례 (`미검증`, 벤더 문서·스펙)

| 도구 | 방식 | 참고할 점 |
|---|---|---|
| **Orca** | Settings에 Linear API 토큰 붙여넣기 → 팀 선택 | worktree 브랜치에 Linear branchName 사용. 팀별 opt-in 동기화(worktree 생성 시 In Progress). 에이전트용 `orca linear` CLI와 skill. 이슈 이미지를 프롬프트에 포함 |
| **OpenAI Symphony** (SPEC) | 폴링 데몬(30초)이 이슈마다 workspace와 에이전트 세션을 만듦 | 매 tick마다 상태 재조정: 종료 상태 → 에이전트 중지 + workspace 삭제. 성공 시 Done이 아니라 **handoff 상태(Human Review)**. 오케스트레이터는 읽기만 하고 쓰기는 에이전트가 함. **자식 프로세스 환경에서 비밀값 제거** |
| **AQ** (상용) | 라벨 `ai-task` → worktree workspace | 양방향 상태 동기화, 담당자가 소유권을 가짐. webhook 기반 |
| **Linear "Work on issue"** | Cursor·Claude Code·Codex로 1회 handoff | **이후 동기화 없음**. 바로 이 빈틈을 플러그인이 채울 수 있음 |

---

## 6. 설계 시사점

1. **포지셔닝**: API 키 하나로 생명주기 전체를 동기화. 경쟁 플러그인과 겹치는 "선택기 → worktree"는 기본기로 빠르게 맞추고, 차별화는 동기화에서 한다.
2. **인증 UX**: 첫 실행 때 키 입력 → `viewer` 쿼리로 검증 → `HERDR_PLUGIN_CONFIG_DIR`에 0600으로 저장(또는 keychain). `LINEAR_API_KEY` 환경 변수가 있으면 우선한다.
3. **이슈 → worktree**: `issue.branchName` + `herdr worktree create --branch ... --label "ENG-123 제목" --focus --json`. 이미 있으면 `worktree open`. PR 연결은 Linear GitHub 연동에 맡긴다.
4. **역매핑**: 브랜치 → `issueVcsBranchSearch`. 사이드바에 `report-metadata --token linear=ENG-123·In Progress`를 표시한다.
5. **에이전트 handoff**: `agent start` + `agent prompt`. 이슈 본문·코멘트로 프롬프트를 만든다. 에이전트용으로 Linear MCP를 쓸지는 선택이다.
6. **herdr → Linear 동기화**: `[[events]] on = "pane.agent_status_changed"`, `worktree.*`로 상태 전환과 코멘트를 남긴다. 에이전트 상태는 추정값이므로 **Done으로 자동 이동하지 않고 In Review 같은 handoff 상태까지만** 옮긴다(Symphony·civitaspo와 같은 방식).
7. **Linear → herdr 동기화**: webhook을 쓸 수 없으므로 폴링한다(`updatedAt` 필터, 작은 페이지, `RATELIMITED`일 때 backoff). `[[startup]]`은 1회성이므로 상주 동기화가 필요하면 detached 프로세스와 pidfile로 직접 관리하거나, 이벤트 시점과 UI를 열 때만 조회하는 방식을 고른다.
8. **링크 처리**: `[[link_handlers]]`로 `^https://linear\.app/.+/issue/[A-Z]+-[0-9]+` 주소를 Ctrl+클릭하면 해당 이슈 worktree를 열거나 만든다.
9. **키 바인딩**: 플러그인이 직접 바인딩할 수 없으므로 설치 안내에 `[[keys.command]] type = "plugin_action"` 예시를 넣는다.

## 7. 확인이 필요한 질문

- "openworktree"는 herdr 내장 `open_worktree`인가, `Saktawdi/OpenWorktree`인가?
- 구현 언어·런타임은? (Node 무의존 / Rust / Go / Python)
- 동기화 범위는? (herdr → Linear 단방향부터 시작할지, 양방향까지 갈지)

---

## 출처

| 출처 | 품질 | 검증 |
|---|---|---|
| https://herdr.dev/docs/plugins/ | 1차 | 검증 + 직접확인 |
| https://herdr.dev/docs/socket-api/ | 1차 | 검증 |
| https://herdr.dev/docs/cli-reference/ | 1차 | 검증 |
| https://github.com/tdi/herdr-worktree-from-linear | 1차 | 검증 |
| https://github.com/talent-factory/herdr-linear | 1차 | 검증(일부 반박) |
| https://github.com/mrolafsson/herdr-linear | 1차 | 검증(일부 반박) |
| https://github.com/zamarrowski/herdr-issues | 1차 | 검증(일부 반박) |
| https://github.com/civitaspo/herdr-linear-agent | 1차 | 미검증 |
| https://github.com/search?q=topic%3Aherdr-plugin+linear | 1차 | 미검증 |
| https://linear.app/developers/graphql | 1차 | 미검증(플러그인 소스와 교차 일치) |
| https://github.com/linear/linear/blob/master/packages/sdk/src/schema.graphql | 1차 | 직접확인 |
| https://linear.app/developers/rate-limiting | 1차 | 직접확인 |
| https://linear.app/developers/webhooks | 1차 | 직접확인 |
| https://linear.app/developers/agent-interaction | 1차 | 미검증 |
| https://linear.app/docs/mcp | 1차 | 미검증 |
| https://linear.app/docs/github | 1차 | 미검증 |
| https://github.com/Saktawdi/OpenWorktree | 1차 | 미검증 |
| https://www.onorca.dev/docs/review/linear | 1차(벤더) | 미검증 |
| https://github.com/openai/symphony/blob/main/SPEC.md | 1차 | 미검증 |
| https://aq.dev/docs/linear-integration/ | 2차(벤더) | 미검증 |
| 로컬 herdr 0.9.3 (`herdr --help`, `herdr api schema`, `~/.config/herdr/plugins.json`) | 로컬 | 로컬확인 |
