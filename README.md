# herdr-linear

**Fast Linear issue lookup inside [herdr](https://herdr.dev).** A popup palette, or a side pane you keep open, that searches as you type, renders issue bodies and comments as markdown in the terminal, and copies issue and PR links.

[한국어](#한국어) · [Roadmap](ROADMAP.md) · MIT

> The interface text is in Korean for now.

## Features

- **Tabs**: your open issues · recently viewed · all issues in your teams, drawn with Linear's own state and label colors.
- **Side pane**: a second key opens the same screen in a pane to the right of your work, and closes it again. It stays open and refreshes what you're looking at every 60 seconds by default. In list mode, Esc doesn't close it; `q` does.
- **Priority**: list rows show a one-column priority mark after the issue ID: `▆` high, `▄` medium, `▂` low, and a red `!` for urgent.
- **Search as you type**: cached issues are searched instantly. When you pause for 300 ms the server is searched too and the results are merged. Pick "서버에서 검색 (코멘트 포함)" at the bottom of the list to search comments as well.
- **Issue detail**: markdown body (headings, lists, code blocks, tables), comments, and a list of links. An open PR is shown at the top.
- **Relations**: the detail view lists the parent, sub-issues, blocked by, blocking, and related issues in Linear's state colors. Press `t` for the relations menu or click a line to open an issue, and Esc to come back.
- **Copy**: `y` copies the issue URL and `Y` copies the open PR link. Copying uses OSC 52, so it reaches your clipboard even when you attach to herdr remotely.
- **Current branch**: when the focused pane is on a branch like `me/eng-123-fix-login`, that issue is pinned at the top.
- **Links**: Ctrl+click a `linear.app/…/issue/…` link in herdr to open it in the palette. Open the palette with an issue ID selected to jump straight to that issue.
- **Mouse**: wheel to move and scroll, click to select, click the selected row again to open it. Whatever you can click lights up under the pointer.
- **Cache first**: issues you have seen are stored locally, shown instantly next time and refreshed in the background. They stay readable offline.

## Requirements

- herdr 0.9.3 or later
- Rust 1.88 or later (herdr builds the plugin from source when you install it)
- macOS or Linux

## Install

```sh
herdr plugin install jenthous/herdr-linear
```

Plugins can't register keys themselves. Add a binding to `~/.config/herdr/config.toml`:

```toml
[[keys.command]]
key = "prefix+i"
type = "plugin_action"
command = "jh.linear.palette"
description = "Linear search"

[[keys.command]]
key = "prefix+shift+i"
type = "plugin_action"
command = "jh.linear.side"
description = "Linear side pane"

# Optional: a direct key that also works while a non-Latin input method is active
[[keys.command]]
key = "ctrl+alt+i"
type = "plugin_action"
command = "jh.linear.palette"
description = "Linear search"
```

You can give `jh.linear.side` a direct key the same way. The actions also show up in the command-palette plugin.

## API key

Create a key in Linear → Settings → Security & access → Personal API keys. The first time you open the palette it asks for the key: paste it and press Enter. The key is checked against Linear and saved with mode 0600.

- `LINEAR_API_KEY` in the environment takes precedence.
- The key is only sent in the `Authorization` header. It is never written to logs, shown on screen, or included in error messages.

## Keys

| Mode | Keys |
|---|---|
| Search (default) | type to search, ↑/↓ or Ctrl+P/N to move, Enter to open, Tab/Shift+Tab to switch tabs, Ctrl+K for the action menu, Esc for list mode |
| List | j/k to move, g/G for top/bottom, `/` to search, Enter to open, `y` copy issue URL, `Y` copy open PR link, `r` refresh, `o` open in browser, `q`/Esc to close (Esc doesn't close the side pane) |
| Detail | j/k to scroll, Ctrl+D/U for half a page, g/G for top/bottom, `t` for relations, `u` for links, `y` `Y` `o` `r`, Esc to go back, `q` to close |

- Copy the issue ID from the Ctrl+K menu. URL copy is at the top of that menu.
- Ctrl combinations work with any input method. In list and detail modes, Korean jamo keys act as the same Latin keys (ㅓ→j, ㅏ→k, …).
- With mouse support on, drag-selecting text inside the palette needs Shift (or Option, depending on your terminal).
- When you attach to herdr remotely, `o` opens a browser on the machine that runs herdr.

## Search syntax

Mix free text and tokens. All conditions must match. Quote values that contain spaces: `s:"In Progress"`.

| Token | Meaning | Example |
|---|---|---|
| `s:value` | state name or type (`started`, `todo`, `done`, …) | `s:started` |
| `l:value` | label | `l:bug` |
| `@value` | assignee (`@me` is you) | `@minsu` |
| `#KEY` | team | `#ENG` |
| `p:value` | priority (`urgent`/`1` … `none`/`0`) | `p:high` |

## Configuration

`~/.config/herdr/plugins/config/jh.linear/config.toml`. Every key is optional.

```toml
teams = ["ENG", "OPS"]   # scope for search and the "all" tab; empty means all your teams

[side]
refresh_seconds = 60     # how often the side pane refreshes (0 to 3600 seconds); 0 turns it off

[cache]
retention_days = 30      # issues not fetched or viewed for this long are dropped from the cache
```

## Files

| File | Location |
|---|---|
| `config.toml`, `credentials` | `~/.config/herdr/plugins/config/jh.linear/` |
| `cache.db`, `herdr-linear.log`, `side-panes.json` | `~/.local/state/herdr/plugins/jh.linear/` |

The standalone CLI uses `~/.config/herdr-linear/` and `~/.local/state/herdr-linear/`. Each side also finds a key saved by the other.

## CLI

The same binary works on its own:

```sh
herdr-linear login | logout | whoami
herdr-linear mine
herdr-linear search login l:bug @me
herdr-linear search --deep session expired
herdr-linear show ENG-123
```

## Troubleshooting

- **The palette doesn't open**: herdr refuses popups while its settings, copy mode, or another modal is open. Close it and try again.
- **The side pane doesn't open or close**: the reason shows in a herdr notification and in `herdr-linear.log`. If the side pane was closed some other way, the key opens a new one.
- **"오프라인" (offline)**: cached data is shown. It retries on the next lookup.
- **"한도를 넘었어요" (rate limited)**: a short notice at the bottom says when to try again. Automatic server search waits until then; local search keeps working.
- Errors are written to `herdr-linear.log`.

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
scripts/deploy-local.sh   # build and link a stable copy for daily use on this machine
```

---

# 한국어

[로드맵](ROADMAP.md#로드맵)

[herdr](https://herdr.dev) 안에서 Linear 이슈를 빠르게 찾고 읽는 플러그인이에요. 단축키로 팝업 팔레트를 띄우거나 사이드 pane에 상시로 띄워 두고, 입력하는 대로 찾고, 본문(markdown)과 코멘트를 터미널에서 바로 봐요. 이슈와 PR 링크도 복사해요.

## 기능

- **탭**: 내 이슈 · 최근 본 · 전체. 상태와 라벨은 Linear에 설정된 색으로 보여요.
- **사이드 pane**: 키를 하나 더 두면 같은 화면을 작업 오른쪽 pane에 열고 닫아요. 상시로 띄워 두고, 보고 있는 것을 기본 60초마다 새로 받아요. 목록 모드에서는 Esc로 닫히지 않고 `q`로 닫아요.
- **우선순위**: 목록 줄의 식별자 뒤에 우선순위를 한 칸으로 보여 줘요. `▆` 높음, `▄` 보통, `▂` 낮음, 긴급은 빨간 `!`예요.
- **입력하는 대로 검색**: 캐시에서 바로 찾아요. 입력이 0.3초 멈추면 서버에서도 찾아 합쳐요. 목록 맨 아래 "서버에서 검색 (코멘트 포함)"을 고르면 코멘트까지 찾아요.
- **상세**: markdown 본문(제목, 목록, 코드 블록, 표), 코멘트, 링크 목록을 보여 줘요. 열린 PR은 맨 위에 보여요.
- **관계**: 상세에 상위·하위·막힘·막는 중·관련 이슈를 Linear 상태 색으로 보여 줘요. `t`로 관계 메뉴를 열거나 줄을 클릭하면 그 이슈로 가고, Esc로 돌아와요.
- **복사**: `y`는 티켓 URL, `Y`는 열린 PR 링크를 복사해요. OSC 52로 복사해서 herdr에 원격으로 붙어 있어도 지금 쓰는 컴퓨터의 클립보드로 와요.
- **현재 브랜치**: 지금 pane의 브랜치(`me/eng-123-...`)에 연결된 이슈를 맨 위에 고정해요.
- **링크**: herdr에서 linear.app 이슈 링크를 Ctrl+클릭하면 팔레트에서 열려요. 이슈 식별자(`ENG-123`)를 선택한 채로 열면 그 이슈로 바로 가요.
- **마우스**: 휠로 이동·스크롤하고, 줄을 클릭해 고르고, 고른 줄을 다시 클릭해 열어요. 누를 수 있는 곳에 마우스를 올리면 옅게 밝아져요.
- **캐시 먼저**: 한 번 본 내용은 저장해 두었다가 다음에 바로 보여 주고 뒤에서 새로 받아요. 오프라인에서도 볼 수 있어요.

## 필요한 것

- herdr 0.9.3 이상
- Rust 1.88 이상 (설치할 때 herdr가 소스를 빌드해요)
- macOS 또는 Linux

## 설치

```sh
herdr plugin install jenthous/herdr-linear
```

플러그인은 키를 직접 등록할 수 없어서 `~/.config/herdr/config.toml`에 추가해요.

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

# 한글 입력 중에도 바로 쓰는 키 (선택)
[[keys.command]]
key = "ctrl+alt+i"
type = "plugin_action"
command = "jh.linear.palette"
description = "Linear 검색"
```

`jh.linear.side`에도 같은 방법으로 직접 키를 둘 수 있어요. 액션은 command-palette 플러그인에도 나타나요.

## API 키

Linear → Settings → Security & access → Personal API keys에서 키를 만들어요. 팔레트를 처음 열면 키 입력 화면이 나와요. 붙여넣고 Enter를 누르면 Linear로 확인한 뒤 권한 0600으로 저장해요.

- `LINEAR_API_KEY` 환경 변수가 있으면 그 키를 먼저 써요.
- 키는 `Authorization` 헤더에만 써요. 로그, 화면, 오류 메시지에는 남기지 않아요.

## 키

| 모드 | 키 |
|---|---|
| 검색 (기본) | 글자 = 검색어, ↑/↓·Ctrl+P/N = 이동, Enter = 상세, Tab/Shift+Tab = 탭, Ctrl+K = 동작 메뉴, Esc = 목록 모드 |
| 목록 | j/k = 이동, g/G = 처음/끝, `/` = 검색, Enter = 상세, `y` = 티켓 URL 복사, `Y` = 열린 PR 링크 복사, `r` = 새로고침, `o` = 브라우저, `q`·Esc = 닫기(사이드 pane은 Esc로 닫히지 않아요) |
| 상세 | j/k = 스크롤, Ctrl+D/U = 반 페이지, g/G = 처음/끝, `t` = 관계, `u` = 링크 목록, `y`·`Y`·`o`·`r`, Esc = 뒤로, `q` = 닫기 |

- ID 복사는 Ctrl+K 메뉴에 있어요. 메뉴 맨 위는 URL 복사예요.
- Ctrl 조합은 한글 입력 중에도 동작해요. 목록·상세에서는 한글 자모 키도 같은 영문 키로 받아요(ㅓ→j, ㅏ→k 등).
- 마우스를 쓰는 동안 팔레트 안의 글자를 드래그로 고르려면 터미널에 따라 Shift(또는 Option)를 누른 채로 드래그해요.
- herdr에 원격으로 붙어 쓰면 `o`는 herdr가 도는 기기의 브라우저를 열어요.

## 검색어

자유 텍스트와 토큰을 섞어 써요. 조건은 모두 맞아야 해요. 공백이 있는 값은 따옴표로 감싸요(`s:"In Progress"`).

| 토큰 | 뜻 | 예 |
|---|---|---|
| `s:값` | 상태 이름 또는 종류(`진행`, `할일`, `완료`, `started`…) | `s:진행` |
| `l:값` | 라벨 | `l:bug` |
| `@값` | 담당자 (`@나`, `@me`는 나) | `@민수` |
| `#KEY` | 팀 | `#ENG` |
| `p:값` | 우선순위 (`긴급`/`urgent`/`1` … `없음`/`none`/`0`) | `p:high` |

## 설정

`~/.config/herdr/plugins/config/jh.linear/config.toml` (모든 항목은 선택)

```toml
teams = ["ENG", "OPS"]   # 검색·전체 탭 범위. 비우면 내가 속한 팀 전부

[side]
refresh_seconds = 60     # 사이드 pane 새로고침 주기(0~3600초). 0이면 꺼요

[cache]
retention_days = 30      # 이 기간 동안 안 받고 안 본 이슈는 캐시에서 지워요
```

## 파일

| 파일 | 위치 |
|---|---|
| `config.toml`, `credentials` | `~/.config/herdr/plugins/config/jh.linear/` |
| `cache.db`, `herdr-linear.log`, `side-panes.json` | `~/.local/state/herdr/plugins/jh.linear/` |

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
- **사이드 pane이 안 열리거나 안 닫혀요**: 이유가 herdr 알림과 `herdr-linear.log`에 남아요. 사이드 pane을 다른 방법으로 닫았으면 키를 누를 때 새로 열려요.
- **"오프라인"**: 저장된 내용으로 계속 보여 주고, 다음 조회 때 다시 시도해요.
- **"한도를 넘었어요"**: 아래에 잠깐 뜨는 안내에 언제 다시 시도할지 나와요. 그때까지 자동 서버 검색은 멈추고, 캐시 검색은 계속 돼요.
- 자세한 오류는 `herdr-linear.log`에 남아요.

## 개발

```sh
cargo test
cargo clippy --all-targets -- -D warnings
scripts/deploy-local.sh   # 이 기기에서 매일 쓸 복사본을 빌드해 herdr에 연결
```

`scripts/deploy-local.sh`는 빌드한 플러그인을 `~/.local/share/herdr-linear/plugin`에 복사해 herdr에 연결하고, `herdr-linear` 명령을 `~/.local/bin`에 연결해요. 그 뒤로는 저장소에서 개발해도 쓰고 있는 플러그인이 바뀌지 않아요.

## 라이선스

MIT
