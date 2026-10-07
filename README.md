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
| 목록 | j/k = 이동, g/G = 처음/끝, `/` = 검색, Enter = 상세, `y` = 티켓 URL 복사, `Y` = 열린 PR 링크 복사, `r` = 새로고침, `o` = 브라우저, `q`·Esc = 닫기 |
| 상세 | j/k = 스크롤, Ctrl+D/U = 반 페이지, g/G = 처음/끝, `u` = 본문 링크 목록, `y`·`Y`·`o`·`r`, Esc = 뒤로 |

- ID 복사는 Ctrl+K 메뉴에 있어요. 메뉴 맨 위는 URL 복사예요.
- 열린 PR이 있으면 상세 머리에 `PR #482 열림 · 저장소`로 보여요.
- 마우스도 쓸 수 있어요.
  - 휠: 목록을 움직이고 상세를 스크롤해요.
  - 줄 클릭: 선택해요. 선택된 줄을 다시 클릭하면 상세가 열려요.
  - 탭 이름·메뉴 항목 클릭: 바로 실행해요.
  - 팔레트 안에서 글자를 드래그로 고르려면 터미널에 따라 Shift(또는 Option)를 누른 채로 드래그해요.
- Ctrl 조합은 한글 입력 중에도 동작해요. 목록·상세에서는 한글 자모 키도 같은 영문 키로 받아요(ㅓ→j, ㅏ→k 등).
- herdr에 원격으로 붙어 쓰면 `o`는 herdr가 도는 기기의 브라우저를 열어요. 복사(OSC 52)는 지금 쓰는 터미널로 와요.

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
