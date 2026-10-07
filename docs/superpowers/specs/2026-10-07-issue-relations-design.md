# 이슈 관계 보기 설계 (상위·하위·막힘)

- 작성일: 2026-10-07
- 상태: 구현 완료 (v0.1.2)
- 버전: v0.1.2
- 바탕 스펙: `docs/superpowers/specs/2026-10-06-herdr-linear-design.md`. 구현이 끝나면 그 문서의 4.6(상세 화면), 4.7(키), 6.2(캐시 스키마), 7.2(쿼리)를 이 문서대로 고친다.

## 1. 목적

티켓을 읽다가 상위 이슈, 하위 이슈, 이 이슈를 막는 이슈, 이 이슈가 막는 이슈를 바로 확인한다. 그 이슈로 넘어갔다가 Esc로 돌아올 수 있다. 보기 전용이다.

### 1.1 성공 기준

1. 상세 화면 머리 아래에 상위·막힘·막는 중·관련·하위가 상태 색과 함께 보인다.
2. `t` 메뉴나 마우스 클릭으로 관계 이슈를 열고, Esc로 한 단계씩 돌아온다. 돌아오면 스크롤 위치가 그대로다.
3. 한 번 본 이슈의 관계는 오프라인에서도 보인다.
4. 상세를 열 때 보내는 요청 수가 늘지 않는다.
5. CLI `show`에도 같은 관계 칸이 나온다.

### 1.2 범위 밖

- 목록 줄의 막힘 표시와 하위 진행 표시
- 관계 만들기·지우기
- 본문·코멘트에 적힌 식별자(`ENG-123`) 클릭
- 중복(duplicate)·비슷한 이슈(similar) 관계
- 사이드 pane (v0.2에서 따로 한다)

## 2. 화면

### 2.1 상세 화면의 관계 칸

```
ENG-131  로그인 버튼 비활성 버그
◐ In Progress · 우선순위 높음 · @minsu
bug frontend · 프로젝트 Auth 개편 · 사이클 12
https://linear.app/acme/issue/ENG-131/login-button

상위    ◐ ENG-120   인증 개편
막힘    ○ ENG-140   API 스키마 확정
        ● ENG-141   토큰 형식 정리
막는 중 ○ ENG-150   결제 페이지 리팩터링
관련    ● ENG-101   로그인 화면 개편
하위    4개 중 2개 남음
        ● ENG-132   토큰 갱신
        ✕ ENG-133   세션 만료 처리
        ◐ ENG-134   자동 로그인
        ○ ENG-135   로그아웃 정리

재현 방법
 …
```

- **위치**: 머리와 URL 줄(보관 안내가 있으면 그 줄) 다음에 빈 줄 하나, 관계 칸, 빈 줄 하나, 본문 순서다. 보여 줄 관계가 하나도 없으면 칸과 그 뒤 빈 줄을 그리지 않는다.
- **칸 순서**: 상위 → 막힘 → 막는 중 → 관련 → 하위.
- **줄 모양**: 칸 이름(표시 폭 8칸), 상태 아이콘과 빈칸 하나, 식별자(`{:<9} `, 목록 줄과 같음), 제목 순서다. 칸 이름은 그 칸의 첫 줄에만 쓰고, 이어지는 줄은 같은 폭만큼 비운다.
- **한 줄에 한 이슈**: 제목이 넘치면 `…`로 자른다. 폭이 모자라 제목 자리가 없으면 제목을 뺀다. 줄바꿈은 하지 않는다(클릭 위치를 줄 번호로 계산하기 때문).
- **칸 안의 순서**
  - 막힘·막는 중·관련: 안 끝난 이슈 먼저, 그다음 Linear가 준 순서(만든 순서)
  - 하위: Linear 하위 목록 순서(`subIssueSortOrder` 오름차순, 값이 없으면 뒤로)
- **끝난 이슈**: 상태 타입이 completed·canceled·duplicate인 이슈.
- **하위 요약**: 하위 칸 첫 줄에 쓰고, 하위 이슈는 그 아래 줄부터 나온다.
  - 기본은 `N개 중 M개 남음`이고, 모두 끝났으면 `N개 모두 끝남`이다.
  - 하위가 50개를 넘으면 `50개 넘음`이라고만 쓴다. 이때 마지막 줄에 `… 더 있어요 (o로 브라우저에서 보기)`를 붙인다.
- **머리 정리**: 머리 정보 줄의 `상위 ENG-120`은 상세 화면과 CLI `show`에서 뺀다. 관계 칸에 같은 정보가 있기 때문이다. 목록 옆 미리보기에는 관계 칸이 없으니 그대로 둔다.
- **아직 모를 때**: 관계를 아직 받지 못했으면(캐시에도 없고 응답 전) 상위만 보인다. 상위는 목록 데이터(`IssueFields`)에도 들어 있다.

### 2.2 색

새 색은 만들지 않는다. 앱에서 이미 쓰는 기본색과 Linear 색만 쓴다.

| 요소 | 스타일 |
|---|---|
| 상태 아이콘 | `state_style` (Linear 상태 색, 목록과 같음) |
| 식별자 | `DIM` |
| 제목 | 기본 글자색. 끝난 이슈면 `DIM` |
| 칸 이름 `상위`·`관련`·`하위` | `DIM` |
| 칸 이름 `막힘` | 그 줄의 이슈가 안 끝났으면 빨강(`Color::Red`), 끝났으면 `DIM` |
| 칸 이름 `막는 중` | 그 줄의 이슈가 안 끝났으면 노랑(`Color::Yellow`), 끝났으면 `DIM` |
| 하위 요약 | `N개 중`은 `DIM`, `M개 남음`은 기본 글자색, `N개 모두 끝남`은 초록(`Color::Green`), `50개 넘음`은 `DIM` |
| `… 더 있어요 …` | `DIM` |

- 칸 이름은 첫 줄에만 쓰고 안 끝난 이슈가 앞에 온다. 그래서 상세 화면의 `막힘`·`막는 중` 색은 그 칸에 안 끝난 이슈가 하나라도 있는지를 보여 준다.
- 관계 메뉴는 줄마다 칸 이름을 쓰므로 줄마다 같은 규칙을 적용한다.
- 미리보기 목업: `/tmp/herdr-linear-preview/relations_colors.py`(사용자 확인 완료, 저장소에 넣지 않음)

### 2.3 관계 메뉴 (`t`)

- **열기**: 상세 화면에서 `t`(두벌식 `ㅅ`)를 누르거나, Ctrl+K 메뉴에서 "관계 이슈"를 고른다. Ctrl+K 메뉴에서는 "링크·이미지 목록" 바로 아래에 둔다. 목록·검색 모드에서는 `t`가 아무 일도 하지 않는다.
- **제목**: 상황에 따라 다르다.

| 상황 | 제목 |
|---|---|
| 관계를 받았고 하나 이상 있음 | `관계` |
| 관계를 받았고 하나도 없음 | `관계 없음` |
| 아직 받는 중 | `관계 불러오는 중…` |
| 받지 못함(오프라인이고 캐시도 없음) | `관계를 불러오지 못했어요` |

  아직 받는 중이거나 받지 못했어도 아는 관계(상위)는 항목으로 보여 준다.
- **항목**: 관계 칸과 같은 순서다. 줄마다 칸 이름을 쓰고, 색은 2.2를 따른다. 고른 줄은 다른 메뉴처럼 배경을 칠한다(`SELECTED_BG`).
- **거르기**: `칸 이름 식별자 제목` 글자로 거른다(`134`, `토큰`, `막힘`).
- **열기**: Enter를 누르거나 항목을 클릭하면 연다(3장).

### 2.4 CLI `show`

- URL 줄 다음에 빈 줄, 관계 칸, 빈 줄을 두고 본문으로 이어진다. 줄 모양과 순서는 2.1과 같고, 폭은 `ctx.width`다.
- 색을 켠 출력이면 2.2 색을 쓰고, 파이프처럼 색을 끈 출력이면 글자만 쓴다(지금 `to_ansi`/`to_plain` 규칙).
- 오프라인이면 캐시된 관계를 쓴다.

### 2.5 하단 안내

상세 모드 안내를 ` j/k 스크롤  t 관계  u 링크  y URL 복사  Y PR 링크  ^K 메뉴  Esc 뒤로  q 닫기`로 바꾼다.

## 3. 이동

### 3.1 여는 방법

| 입력 | 동작 |
|---|---|
| `t` → 항목에서 Enter | 고른 관계 이슈를 연다 |
| 관계 메뉴 항목 클릭 | 그 이슈를 연다 |
| 상세 화면의 관계 줄 클릭 | 그 이슈를 바로 연다(한 번 클릭) |
| 하위 요약 줄, `… 더 있어요` 줄 클릭 | 아무 일도 하지 않는다 |
| 휠 | 지금처럼 상세 스크롤, 메뉴 이동 |

### 3.2 상세 스택

- **열기**: 관계 이슈를 열면 지금 상세(스크롤 위치 포함)를 스택에 쌓고 새 상세를 연다. 새 상세는 지금 흐름을 그대로 탄다(캐시 먼저 → 서버, 최근 본 기록).
- **Esc**: 스택이 있으면 하나를 꺼내 그 상세로 돌아간다. 스크롤 위치도 그대로다. 스택이 비었으면 지금처럼 상세를 열었던 모드(검색·목록)로 돌아간다.
- **`q`**: 언제나 팔레트를 닫는다.
- **같은 이슈 다시 열기**: 같은 이슈가 스택에 여러 번 있어도 된다(A → B → A).
- **인증 실패**: 키 입력 화면으로 갈 때 스택도 비운다.
- **깊이 제한**: 두지 않는다. 사람이 하나씩 여는 것이라 크게 쌓이지 않는다.

### 3.3 응답 반영

- `Msg::Detail`과 `Msg::DetailGone`은 지금 상세뿐 아니라, 쌓인 상세 가운데 같은 이슈에도 반영한다. 그래서 응답 전에 다른 이슈로 넘어갔다가 돌아와도 "불러오는 중…"에 멈춰 있지 않다.
- `Msg::Failed`는 어느 요청이 실패했는지 모른다(2부 후속 N6). 그래서 지금 상세와 쌓인 상세의 대기 표시를 모두 푼다.
- 요청 회계는 그대로다. `OpenDetail` 하나에 `loading`을 1 올리고, 응답이나 실패 하나에 1 내린다.

## 4. 데이터

### 4.1 쿼리

- **공용 `IssueFields`**: 상위 이슈의 상태를 더한다.
  ```graphql
  parent { id identifier title state { id name type color } }
  ```
- **상세 쿼리(`issue_detail`)**: 아래를 더한다. 요청은 지금처럼 상세 1회다.
  ```graphql
  children(first: 50) { nodes { id identifier title subIssueSortOrder state { id name type color } } pageInfo { hasNextPage } }
  relations(first: 50) { nodes { type relatedIssue { id identifier title state { id name type color } } } }
  inverseRelations(first: 50) { nodes { type issue { id identifier title state { id name type color } } } }
  ```

### 4.2 해석

| 원본 | 결과 |
|---|---|
| `inverseRelations` 중 `type = blocks` | 막힘: 노드의 `issue` (이 이슈를 막는 이슈) |
| `relations` 중 `type = blocks` | 막는 중: 노드의 `relatedIssue` (이 이슈가 막는 이슈) |
| 양쪽의 `type = related` | 관련: 상대 이슈. 같은 id는 하나만 둔다 |
| `duplicate`, `similar`, 모르는 `type` | 버린다 |
| `children` | 하위. `subIssueSortOrder` 순으로 정렬하고, 다음 페이지가 있으면 `more_children` |
| 이슈를 읽을 수 없는 노드(null이거나 모양이 다름) | 그 노드만 버리고 나머지는 읽는다 |

하위 정렬은 해석할 때 한 번 한다. 안 끝난 것을 앞으로 보내는 정렬은 그릴 때 한다(4.4). 그래야 캐시에 있는 상태가 바뀌어도 바로 맞는 순서가 된다.

### 4.3 타입 (`linear::types`)

```rust
pub struct ParentRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
    /// 예전 캐시에는 없다
    #[serde(default)]
    pub state: Option<StateRef>,
}

pub struct RelatedIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub state: StateRef,
}

pub struct IssueRelations {
    pub children: Vec<RelatedIssue>,
    pub more_children: bool,
    pub blocked_by: Vec<RelatedIssue>,
    pub blocking: Vec<RelatedIssue>,
    pub related: Vec<RelatedIssue>,
}

pub struct IssueDetail {
    pub issue: Issue,
    pub comments: Vec<Comment>,
    pub more_comments: bool,
    /// 관계. 관계 없이 다시 받았으면 `None`(모름)
    pub relations: Option<IssueRelations>,
}
```

### 4.4 화면용 줄 (`ui::relations`, 새 파일)

TUI와 CLI가 함께 쓴다. 앱(메뉴·클릭), 상세 화면, 관계 메뉴, CLI가 모두 같은 순서를 쓰게 하려는 것이다.

- **`rows(parent, relations) -> Vec<RelRow>`**: 표시 순서대로 늘어놓은 관계 줄. 막힘·막는 중·관련 안에서는 안 끝난 것을 먼저 놓는다(안정 정렬).
- **`RelRow { kind: RelKind, id, identifier, title, state: Option<StateRef> }`**: 상위의 상태는 예전 캐시에서 비어 있을 수 있어서 `Option`이다. 상태가 없으면 아이콘 자리(2칸)를 빈칸으로 두고, 안 끝난 이슈로 본다.
- **`RelKind`**: `Parent`, `BlockedBy`, `Blocking`, `Related`, `Child`.
- **`lines(parent, relations, width) -> (Vec<Line>, Vec<Option<usize>>)`**: 관계 칸의 줄과, 줄마다 대응하는 `rows` 번호를 돌려준다. 요약 줄과 `… 더 있어요` 줄은 `None`이다. 클릭 위치를 계산할 때 쓴다.
- **`row_line(row, show_kind, width) -> Line`**: 줄 하나를 그린다. 관계 메뉴도 이 함수로 그린다.

### 4.5 캐시

```sql
CREATE TABLE relations (
  issue_id   TEXT PRIMARY KEY,
  data       TEXT NOT NULL,      -- IssueRelations JSON
  fetched_at INTEGER NOT NULL
);
```

- **버전 올리기**: `SCHEMA_VERSION`을 2로 올린다.
  - 버전 1 파일은 이 테이블만 만들고 2로 올린다. 기존 이슈·코멘트·최근 본 기록은 그대로 남는다.
  - 그 밖의 버전은 지금처럼 비우고 새로 만든다.
  - 버전 1을 2로 옮기는 일도 버전 확인과 같은 쓰기 트랜잭션 안에서 한다(지금 `init`과 같음).
- **함께 지우기**: `remove_issue`, `evict_older_than`, `clear_all`이 `relations`도 지운다.
- **읽기 흐름**: 상세를 열 때 캐시된 관계를 이슈·코멘트와 함께 먼저 보낸다(`Msg::Detail.relations`). 서버 응답이 오면 저장하고 바꾼다.
- **옛 버전과 함께 쓸 때**: 옛 바이너리가 버전 2 파일을 열면 지금 규칙대로 캐시를 새로 만든다. 그다음 새 바이너리는 그 파일을 다시 2로 만든다(캐시를 한 번 잃을 뿐 오류는 없다).

### 4.6 앱 상태 (`tui::app`)

| 항목 | 내용 |
|---|---|
| `Detail.relations: Option<IssueRelations>` | `None`이면 아직 모른다 |
| `App.detail_stack: Vec<Detail>` | 3.2의 스택 |
| `Act::Relations` | 관계 메뉴 열기 |
| `Act::OpenRelated(RelRow)` | 관계 이슈 열기(스택에 쌓기) |
| `Input::ClickRelation(usize)` | 상세 화면의 관계 줄 클릭. 값은 `rows` 번호 |
| `Target::Relation(usize)` | 상세 화면을 그릴 때 보이는 관계 줄마다 등록한다(스크롤 반영) |
| `Msg::Detail { relations: Option<IssueRelations>, .. }` | 캐시면 저장된 관계, 서버 응답이면 받은 관계 |

메뉴를 그릴 때 항목의 동작이 `Act::OpenRelated`면 `ui::relations::row_line`으로 색을 입혀 그린다. 다른 메뉴는 지금 그대로 그린다.

## 5. 오류

| 상황 | 동작 |
|---|---|
| 오프라인 | 캐시된 관계를 보여 준다. 캐시에 없으면 상위만 보인다 |
| 한도 초과, 인증 실패, 그 밖의 오류 | 지금 상세 처리 그대로다. 관계는 상세와 같은 응답에 들어 있다 |
| 읽을 수 없는 관계 노드 | 그 줄만 건너뛴다 |
| 관계 때문에 상세 응답 전체가 오류(GraphQL·해석 오류) | 관계 없이 한 번 더 받아 상세를 보여 준다. 관계는 모름으로 두고(메뉴 제목 "관계를 불러오지 못했어요"), 저장된 관계는 지우지 않는다. 없는 이슈인지는 두 번째 응답으로 판단한다 |
| 연 관계 이슈가 없음·보관·삭제 | 지금처럼 "찾을 수 없거나 보관·삭제된 이슈예요"를 보여 준다. Esc로 돌아온다 |
| 하위 50개 초과 | 50개만 보여 주고 `… 더 있어요 (o로 브라우저에서 보기)`를 붙인다 |

## 6. 구현 초기에 확인할 가정

첫 작업에서 실제 키로 읽기 전용 쿼리를 보내 확인한다. 1부 실측처럼 키는 0600 임시 파일로 넘기고, 이슈 제목·사람 이름은 기록하지 않는다.

| 가정 | 확인 방법 | 다를 때 |
|---|---|---|
| `relations`의 `blocks`는 노드의 `issue`가 `relatedIssue`를 막는다는 뜻이다 | 막힘 관계가 있는 이슈 두 개를 조회 | 방향을 바꾼다 |
| 상세 쿼리 복잡도가 한도(10,000점)보다 충분히 작다 | `X-Complexity` 실측 | 각 `first`를 20으로 줄인다 |
| 목록 쿼리에 상위 상태를 더해도 복잡도가 거의 늘지 않는다 | 50건 페이지 실측 | 상위 상태는 상세 쿼리로만 받는다 |
| 볼 수 없는 팀의 이슈와 맺은 관계 때문에 쿼리 전체가 실패하지는 않는다 | 가능하면 실제로 확인하고, 안 되면 Linear 문서를 본다 | 관계를 따로 쿼리해서, 실패해도 상세는 뜨게 한다. 확인할 수 없으면 지금 설계대로 두고 메모에 남긴다. 실측으로 재현하지 못해, 대안(관계 없이 다시 받기)을 미리 넣었다. |
| `children`은 보관된 하위 이슈를 기본으로 빼고 준다 | 보관된 하위가 있는 이슈 조회 | `includeArchived: false`를 명시한다 |

## 7. 테스트

| 대상 | 내용 |
|---|---|
| `linear::queries` | 응답 JSON → `IssueRelations`: blocks 방향, related 양쪽 합치기와 중복 제거, duplicate·similar 버리기, 하위 순서와 `more_children`, 읽을 수 없는 노드 건너뛰기. `parent.state`가 없는 예전 JSON 읽기 |
| `store` | relations 저장·읽기, 버전 1 파일을 2로 옮기기(이슈·코멘트·최근 본 유지), `remove_issue`·`evict_older_than`·`clear_all` |
| `ui::relations` | 칸 순서와 안 끝난 것 먼저, 칸 이름은 첫 줄에만, 색(빨강·노랑·흐리게·초록), 하위 요약 세 가지, 좁은 폭 자르기, 줄 → `rows` 번호, 상위 상태가 없을 때 |
| `tui::app` | 관계 메뉴(순서, 제목 네 가지), Enter·클릭으로 열기 → 스택, Esc로 스크롤 그대로 돌아오기, 스택 끝에서 Esc → 목록, `q` 닫기, 쌓인 상세에 응답 반영, `Failed`가 모든 대기 풀기, 요청 회계, Ctrl+K "관계 이슈" |
| `tui::view` | 상세 스냅샷(관계 칸 위치, 머리에서 `상위` 빠짐, 미리보기에는 남음), 스크롤한 상태에서 클릭 위치 → `Target::Relation`, 관계 메뉴 색, 하단 안내 |
| `tui::keys` | 상세에서 `t`·`ㅅ` → `Act::Relations`, 목록·검색에서는 아님, 관계 줄 클릭 → `Input::ClickRelation` |
| `tui::runtime` | 캐시된 관계를 먼저 보내고, 응답을 저장한 뒤 보낸다 |
| `cli` | `show` 출력의 관계 칸(색 끔), 머리에서 `상위` 빠짐, 오프라인에서 캐시된 관계 |

수동 확인은 herdr에서 실제 키로 한다. 상위·하위·막힘이 있는 이슈를 열고 아래를 확인한다.
- `t` 메뉴, 관계 줄 클릭, Esc로 여러 단계 돌아오기
- 색
- 오프라인(`HTTPS_PROXY=http://127.0.0.1:9`)

## 8. 문서·버전

- 바탕 스펙의 4.6·4.7·6.2·7.2를 고친다.
- README(영어·한국어)의 기능 목록과 키 표에 관계 칸과 `t`를 더한다.
- ROADMAP에 "v0.1.2 — 관계 보기"를 더한다. v0.2(사이드 pane과 변경 동작)는 그대로 다음이다.
- `Cargo.toml`과 `herdr-plugin.toml`의 버전을 0.1.2로 올린다.
