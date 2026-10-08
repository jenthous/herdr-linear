# Roadmap

[한국어](#로드맵)

## ✅ v0.1 — quick lookup (released 2026-10-07)

- Popup palette with three tabs (my issues, recently viewed, all), drawn in Linear's own colors
- Search as you type: cache first, then the server. Deep search also covers comments.
- Issue detail with the markdown body, comments, links, and the open PR
- Copy the issue URL or the open PR link, open in a browser, and use the mouse
- herdr integration:
  - a shortcut action
  - Ctrl+click on Linear links
  - the issue for the current git branch
  - opening an issue by its selected ID
- Works offline from the cache. A small CLI ships too (`mine`, `search`, `show`).
- Published on GitHub and listed in the herdr plugin marketplace

## ✅ v0.1.1 — fixes from the v0.1 review

- `q` closes the palette from the detail view too; Esc goes back
- Link numbers in comments match the `u` link list
- The list keeps its scroll position when you move back up
- Rate limits show as a short notice instead of a status that stays on screen
- Config warnings stay visible longer and also show on the key screen
- CLI: piping output (`herdr-linear mine | head`) no longer crashes, `logout` clears the cache in both locations, and offline fallback no longer shows another workspace's cache right after a key change

## ✅ v0.1.2 — issue relations

- The detail view shows the parent, sub-issues, blocked by, blocking, and related issues, colored by state. Open blockers stand out in red.
- Open a related issue from the `t` menu or by clicking its line. Esc walks back one issue at a time.
- `herdr-linear show` prints the same relations, and they stay readable offline.

## ✅ v0.2 — side pane

- **Side pane**: a second key (`prefix+shift+i`) opens the palette screen in a pane to the right of your work and closes it again. It stays open, refreshes what you're looking at every 60 seconds by default, and Esc doesn't close it from the list; `q`, Ctrl+C, or the key does.
- List rows show Linear-style priority bars in the palette, the side pane, and the CLI.
- Whatever you can click lights up under the mouse pointer.
- The screen is redrawn only when something changes, and otherwise once a second, so an idle pane uses almost no CPU.

## 🔜 v0.3 — actions and agent handoff (next)

- **Change issues without leaving the terminal**: status, labels, assignee, comments, and new issues. Changes show up immediately. If Linear rejects one, it is rolled back and the reason is shown.
- **Send an issue to an agent** (`p`):
  - A template is filled with the issue's title, link, state, labels, description, and recent comments.
  - You can edit it before sending.
  - It is pasted into the agent's pane.
- **Polish from the v0.1 review**: key-change handling and other small fixes

## Later

- Prebuilt binaries with checksums on GitHub Releases, so installing doesn't need Rust. Building from source stays as the fallback.
- Report agent results back to Linear as comments and status changes
- Create a git worktree from an issue
- Korean initial-consonant (초성) search
- More edits: priority, project, cycle
- Syntax highlighting in code blocks

## Under consideration

- An English interface (the UI text is Korean for now)
- Signing in with Linear OAuth instead of a personal API key

## Not planned

- Syncing the whole workspace or running a background daemon (only what you look at is cached)
- Using several Linear workspaces at the same time
- Deleting issues, or editing and deleting comments
- Showing images inline

---

# 로드맵

## ✅ v0.1 — 빠른 조회 (2026-10-07 공개)

- 팝업 팔레트: 내 이슈 · 최근 본 · 전체 탭을 Linear 색으로 보여 줘요.
- 입력하는 대로 검색해요. 캐시에서 먼저 찾고 서버에서도 찾아요. 깊은 검색은 코멘트까지 찾아요.
- 상세 화면: markdown 본문, 코멘트, 링크, 열린 PR을 보여 줘요.
- 티켓 URL·열린 PR 링크를 복사하고, 브라우저로 열고, 마우스로 조작해요.
- herdr 연결
  - 단축키 액션
  - Linear 링크 Ctrl+클릭
  - 현재 브랜치의 이슈 고정
  - 선택한 식별자로 바로 열기
- 캐시로 오프라인에서도 봐요. CLI도 함께 있어요(`mine`, `search`, `show`).
- GitHub에 공개하고 herdr 플러그인 마켓플레이스에 올렸어요.

## ✅ v0.1.1 — v0.1 리뷰 수정

- 상세 화면에서도 `q`로 닫아요. Esc는 뒤로예요.
- 코멘트 안 링크 번호가 `u` 링크 목록 번호와 같아요.
- 목록을 위로 올릴 때 스크롤 위치가 그대로예요.
- 한도를 넘으면 상단에 남기지 않고 잠깐 알려요.
- 설정 경고가 더 오래 보이고 키 입력 화면에도 보여요.
- CLI: 출력을 파이프로 넘겨도(`herdr-linear mine | head`) 오류로 멈추지 않아요. `logout`은 두 위치의 캐시를 모두 지워요. 키를 바꾼 직후 오프라인이면 다른 워크스페이스 캐시를 보이지 않아요.

## ✅ v0.1.2 — 관계 보기

- 상세에 상위·하위·막힘·막는 중·관련 이슈를 상태 색과 함께 보여 줘요. 안 끝난 막는 이슈는 빨강으로 눈에 띄어요.
- `t` 메뉴나 줄 클릭으로 관계 이슈를 열고, Esc로 한 단계씩 돌아와요.
- `herdr-linear show`에도 같은 관계가 나오고, 오프라인에서도 볼 수 있어요.

## ✅ v0.2 — 사이드 pane

- **사이드 pane**: 키 하나(`prefix+shift+i`)를 더 두면 팔레트 화면을 작업 오른쪽 pane에 열고 닫아요. 상시로 띄워 두고, 보고 있는 것을 스스로 새로 받아요(기본 60초마다). 목록 모드에서 Esc로는 닫히지 않고, `q`·Ctrl+C나 그 키로 닫혀요.
- 목록 줄에 Linear식 우선순위 막대를 보여 줘요. 팔레트, 사이드 pane, CLI 모두 같아요.
- 누를 수 있는 곳에 마우스를 올리면 옅게 밝아져요.
- 바뀐 게 있을 때와 1초에 한 번만 다시 그려서, 가만히 둔 pane은 CPU를 거의 쓰지 않아요.

## 🔜 v0.3 — 변경 동작과 에이전트 전달 (다음)

- **터미널에서 바로 바꾸기**: 상태, 라벨, 담당자, 코멘트, 새 이슈. 바꾼 내용은 바로 보여요. Linear가 거절하면 되돌리고 이유를 보여 줘요.
- **이슈를 에이전트에게 보내기** (`p`)
  - 이슈의 제목·링크·상태·라벨·본문·최근 코멘트로 템플릿을 채워요.
  - 보내기 전에 고칠 수 있어요.
  - 에이전트 pane에 붙여 넣어요.
- **v0.1 리뷰에서 미룬 손질**: 키를 바꿀 때 상태 정리 등 작은 수정

## 나중에

- GitHub Releases에 미리 빌드한 바이너리와 체크섬을 올려서, Rust 없이도 설치되게 해요. 소스 빌드는 대체 수단으로 남겨요.
- 에이전트 작업 결과를 Linear 코멘트·상태에 반영해요.
- 이슈에서 git worktree를 만들어요.
- 초성 검색
- 더 많은 변경: 우선순위, 프로젝트, 사이클
- 코드 블록 문법 강조

## 검토 중

- 영어 화면 (지금은 화면 문구가 한국어예요)
- 개인 API 키 대신 Linear OAuth로 로그인

## 하지 않을 것

- 워크스페이스 전체 동기화나 백그라운드 데몬 (본 것만 캐시해요)
- 여러 Linear 워크스페이스를 동시에 쓰기
- 이슈 삭제, 코멘트 수정·삭제
- 이미지를 화면 안에 그대로 보여 주기
