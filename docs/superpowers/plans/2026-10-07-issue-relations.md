# 이슈 관계 보기 (v0.1.2) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 상세 화면에 상위·막힘·막는 중·관련·하위 이슈를 상태 색과 함께 보여 준다. `t` 메뉴나 클릭으로 그 이슈를 열고, Esc로 한 단계씩 돌아온다. CLI `show`에도 같은 관계 칸을 넣는다.

**Architecture:**
- 관계는 상세 쿼리 한 번에 같이 받고(`linear::queries`), 새 캐시 테이블에 둔다(`store`).
- 줄의 순서·모양·색은 새 순수 모듈 `ui::relations` 한 곳에서 정한다. TUI 상세, 관계 메뉴, CLI가 모두 이 모듈을 쓴다.
- 앱 상태(`tui::app`)에는 상세 스택과 관계 동작만 더한다. 런타임은 캐시를 읽고 쓰는 부분만 맞춘다.

**Tech Stack:** Rust 2024(최소 1.88), ratatui 0.30.2, rusqlite 0.40.2, ureq 3.4.2, serde_json. 테스트는 mockito·tempfile을 쓴다. **새 의존성은 없다.**

**Spec:** `docs/superpowers/specs/2026-10-07-issue-relations-design.md`. 바탕 스펙은 `docs/superpowers/specs/2026-10-06-herdr-linear-design.md`다.

## 시작 전에

- 브랜치 `feat/issue-relations`(스펙 커밋 9cf0ee8)에서 이어서 한다.
- 각 작업은 이 순서로 한다.
  1. 테스트를 쓴다.
  2. 실패를 확인한다(대개 컴파일 실패).
  3. 구현한다.
  4. 통과를 확인한다.
  5. 커밋한다.
- 편집 블록 읽는 법
  - "찾아 바꾼다"의 첫 블록은 그 시점의 파일에 정확히 한 번 나온다(Edit 도구의 old_string/new_string).
  - "모두 바꾼다"는 같은 글자가 나오는 곳을 모두 바꾼다(Edit 도구의 replace_all).
  - "테스트 모듈 끝에 넣는다"는 `mod tests`의 마지막 `}` 바로 앞에 넣는다.
- 작업마다 끝에서 아래 세 명령이 통과해야 한다. 시작 시점에는 256개 통과, 1개 무시다.
  - `cargo fmt --check` (코드를 넣은 뒤 `cargo fmt`로 먼저 맞춘다. 계획의 코드 블록과 줄바꿈이 달라져도 `cargo fmt` 결과를 따른다)
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`

## Global Constraints

- **의존성**: 더하거나 버전을 바꾸지 않는다. `rust-version = "1.88"`.
- **화면 문구**: 한국어, 해요체.
- **외부 문자열**: 식별자·제목·상태 이름처럼 Linear에서 온 문자열은 화면이나 터미널에 쓰기 전에 `markdown::sanitize`로 제어 문자를 지운다.
- **색**: 새 색을 만들지 않는다. 다음만 쓴다.
  - `DIM`
  - `Color::Red` (막힘)
  - `Color::Yellow` (막는 중)
  - `Color::Green` (모두 끝남)
  - `state_style` (Linear 상태 색)
  - `SELECTED_BG`
- **끝난 이슈**: 상태 타입이 completed·canceled·duplicate인 이슈.
- **줄 폭**: 칸 이름은 8칸, 식별자는 `{:<9} `(목록 줄과 같음).
- **관계 쿼리 개수**: `children`·`relations`·`inverseRelations` 각 50개다(`RELATION_PAGE_SIZE`). Task 1 실측 결과로 20까지 줄일 수 있다.
- **요청 수 맞추기**: 세는 요청(`loading`)과 그 끝맺음은 1:1이다. `OpenDetail` 하나에 `loading`을 1 올리고, 응답·실패 하나에 `done_loading()`을 한 번 부른다.
- **테스트 값**: 순번·카운터 같은 내부 값을 하드코딩하지 않는다.
- **키와 기록**: API 키는 Authorization 헤더와 credentials 파일에만 쓴다. 로그·화면·문서·커밋에 남기지 않는다. 실측 기록에도 이슈 제목·사람 이름·워크스페이스 이름을 남기지 않는다.
- **import**: 테스트 모듈에서 `use super::*;`로 이미 들어오는 이름을 다시 import하지 않는다. `-D warnings`라서 중복 import도 실패한다.

## Review Focus

1. **응답 전에 넘어가기**: 응답이 오기 전에 관계 이슈로 넘어갔다가 Esc로 돌아와도 원래 상세가 "불러오는 중…"에 멈추지 않고 응답 내용을 보여 준다. (Task 5 `response_for_a_stacked_detail_still_lands`)
2. **스크롤 뒤 클릭**: 상세를 스크롤한 뒤 관계 줄을 누르면 화면에 보이는 그 줄의 이슈가 열린다. 스크롤로 가려진 관계 줄 자리는 눌러도 아무 일도 없다. (Task 7 `relation_click_targets_follow_the_scroll`)
3. **예전 캐시**: 버전 1 파일이나 상위에 `state`가 없는 이슈 JSON으로 시작해도 최근 본 기록이 남는다. 상위 줄은 아이콘 없이 보이고 패닉하지 않는다. (Task 2 `old_parent_without_state_still_reads`, Task 3 `version_1_file_keeps_its_data_and_gains_relations`, Task 4 `parent_without_state_has_no_icon`)
4. **읽을 수 없는 노드**: 권한 없는 팀 등으로 관계 노드 하나가 null이거나 모양이 달라도 그 줄만 빠지고 상세는 정상으로 뜬다. (Task 2 `detail_reads_relations_by_direction`)
5. **좁은 화면**: 화면이 아주 좁거나 제목이 길어도 관계 줄은 폭을 넘지 않고 `…`로 잘린다. 작은 화면에서도 패닉하지 않는다. (Task 4 `narrow_width_truncates_the_title`, Task 7에서 `tiny_screens_do_not_panic`에 관계 있는 상세를 더함)

---

### Task 1: Linear API 가정 확인 (읽기 전용 실측)

스펙 6장의 가정을 실제 키로 확인한다. 결과에 따라 Task 2의 두 값을 정한다.

**Files:**
- Create: `docs/superpowers/notes/2026-10-07-relations-api-checks.md`

**Interfaces:**
- Produces: 실측 결과
  - `blocks` 방향이 스펙과 반대면 Task 2 `relations_from`에서 `blocking`과 `blocked_by`를 서로 바꾼다.
  - 상세 쿼리의 `x-complexity`가 2,000 이상이면 Task 2 `RELATION_PAGE_SIZE`를 20으로 한다.

- [ ] **Step 1: 실측 스크립트를 한 셸에서 실행한다**

출력은 jq로 참거짓·개수·숫자만 뽑는다. 제목·이름은 화면에 나오지 않는다. 키는 0600 임시 파일로만 넘기고, 끝나면 지운다.

```sh
set -eu
umask 077
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
KEY_FILE="$HOME/.config/herdr/plugins/config/jh.linear/credentials"
[ -s "$KEY_FILE" ] || KEY_FILE="$HOME/.config/herdr-linear/credentials"
if [ -n "${LINEAR_API_KEY:-}" ]; then
  printf 'Authorization: %s\n' "$LINEAR_API_KEY" > "$T/auth"
else
  printf 'Authorization: %s\n' "$(tr -d '\n' < "$KEY_FILE")" > "$T/auth"
fi
gql() {
  jq -n --arg q "$1" --argjson v "$2" '{query: $q, variables: $v}' |
    curl -sS -D "$T/headers" -H @"$T/auth" -H 'Content-Type: application/json' \
      --data @- https://api.linear.app/graphql
}
cx() { grep -i '^x-complexity:' "$T/headers" | tr -d '\r' | awk '{print $2}'; }

echo "== 1. 막힌 이슈의 inverseRelations: blocks 노드의 relatedIssue가 자기, issue가 막는 쪽인가"
Q1='query { issues(filter: { hasBlockedByRelations: { eq: true } }, first: 5) { nodes { id inverseRelations(first: 20) { nodes { type issue { id } relatedIssue { id } } } } } }'
gql "$Q1" '{}' > "$T/q1.json"
jq '{errors: ((.errors // []) | length)} + ([(.data.issues.nodes // [])[] | . as $x | (.inverseRelations.nodes // [])[] | select(.type == "blocks") | ((.relatedIssue.id == $x.id) and (.issue.id != $x.id))] | {checked: length, all_ok: all})' "$T/q1.json"

echo "== 2. 막는 이슈의 relations: blocks 노드의 issue가 자기, relatedIssue가 막히는 쪽인가"
Q2='query { issues(filter: { hasBlockingRelations: { eq: true } }, first: 5) { nodes { id relations(first: 20) { nodes { type issue { id } relatedIssue { id } } } } } }'
gql "$Q2" '{}' > "$T/q2.json"
jq '{errors: ((.errors // []) | length)} + ([(.data.issues.nodes // [])[] | . as $x | (.relations.nodes // [])[] | select(.type == "blocks") | ((.issue.id == $x.id) and (.relatedIssue.id != $x.id))] | {checked: length, all_ok: all})' "$T/q2.json"

FRAG='fragment IssueFields on Issue { id identifier number title description priority estimate url branchName dueDate createdAt updatedAt archivedAt trashed team { id key name } state { id name type color } assignee { id name displayName } project { id name } cycle { id number name } parent { id identifier title state { id name type color } } labels(first: 20) { nodes { id name color } } attachments(first: 10) { nodes { url sourceType createdAt metadata } } }'
REL='id identifier title state { id name type color }'

echo "== 3. 새 상세 쿼리 복잡도"
ID=$(jq -r '.data.issues.nodes[0].id // empty' "$T/q1.json")
if [ -z "$ID" ]; then
  gql 'query { issues(first: 1) { nodes { id } } }' '{}' > "$T/any.json"
  ID=$(jq -r '.data.issues.nodes[0].id' "$T/any.json")
fi
Q3="query Detail(\$id: String!) { issue(id: \$id) { ...IssueFields comments(first: 50) { nodes { id body createdAt editedAt user { id name displayName } } pageInfo { hasNextPage endCursor } } children(first: 50) { nodes { $REL subIssueSortOrder } pageInfo { hasNextPage } } relations(first: 50) { nodes { type relatedIssue { $REL } } } inverseRelations(first: 50) { nodes { type issue { $REL } } } } } $FRAG"
gql "$Q3" "$(jq -n --arg id "$ID" '{id: $id}')" > "$T/q3.json"
echo "detail x-complexity: $(cx)"
jq '{errors: ((.errors // []) | length), children: (.data.issue.children.nodes | length), relation_nodes: ((.data.issue.relations.nodes | length) + (.data.issue.inverseRelations.nodes | length))}' "$T/q3.json"

echo "== 4. 목록 50건 복잡도 (상위 상태 포함, 1부 실측은 16)"
Q4="query Issues { issues(first: 50, orderBy: updatedAt) { nodes { ...IssueFields } pageInfo { hasNextPage endCursor } } } $FRAG"
gql "$Q4" '{}' > "$T/q4.json"
echo "list x-complexity: $(cx)"
jq '{errors: ((.errors // []) | length), nodes: (.data.issues.nodes | length)}' "$T/q4.json"
```

Expected:
- 1·2번은 `errors: 0`이고, `checked`가 1 이상이면 `all_ok: true`다. 막힘 관계가 하나도 없는 워크스페이스면 `checked: 0`이 나온다. 이때는 방향을 "확인 못 함"으로 적는다. 스키마 설명(`issue`는 관계를 맺는 쪽, `relatedIssue`는 대상)대로 스펙 방향을 유지한다.
- 3번은 `errors: 0`과 복잡도 숫자다.
- 4번은 `errors: 0`, `nodes: 50`(이슈가 그보다 적으면 그 수)과 복잡도 숫자다.

- [ ] **Step 2: 결과를 기록한다**

`docs/superpowers/notes/2026-10-07-relations-api-checks.md`를 만든다. 결과 칸에는 Step 1 출력값을 그대로 옮긴다.

```markdown
# 관계 API 실측 (v0.1.2)

- 날짜: 2026-10-07
- 방법: 읽기 전용 GraphQL 쿼리. 키는 0600 임시 헤더 파일로 넘기고 바로 지웠다. 이 문서에는 이슈 제목·사람 이름·워크스페이스 정보를 남기지 않는다.

| 확인 항목 (관계 스펙 6장) | 결과 | 계획에 주는 영향 |
|---|---|---|
| `blocks` 방향: 막힌 이슈의 `inverseRelations` | (1번 출력: checked, all_ok) | all_ok면 스펙대로 |
| `blocks` 방향: 막는 이슈의 `relations` | (2번 출력: checked, all_ok) | all_ok면 스펙대로 |
| 새 상세 쿼리 복잡도 | (3번 x-complexity) | 2,000 미만이면 `RELATION_PAGE_SIZE` 50 유지 |
| 목록 50건 복잡도 (상위 상태 포함) | (4번 x-complexity, 1부 16) | 크게 늘지 않으면 `IssueFields`에 상위 상태 유지 |
| 보관된 하위 이슈 | 스키마 문서: `children`의 `includeArchived` 기본값 false | 그대로 둔다 |
| 볼 수 없는 팀과의 관계 | 이 키로는 재현할 수 없음 | 설계대로 두고, 읽을 수 없는 노드는 하나씩 버린다 |
```

결과가 스펙과 다르면 그 줄의 "계획에 주는 영향"에 무엇을 바꾸는지 적는다(예: "Task 2에서 blocking·blocked_by를 서로 바꾼다").

- [ ] **Step 3: 커밋**

```bash
git add docs/superpowers/notes/2026-10-07-relations-api-checks.md
git commit -m "docs: 관계 API 실측 — blocks 방향, 상세·목록 쿼리 복잡도"
```

---

### Task 2: 관계 타입과 상세 쿼리

**Files:**
- Modify: `src/linear/types.rs` (`ParentRef`, `StateRef::is_finished`, `RelatedIssue`, `IssueRelations`, `IssueDetail`, 테스트)
- Modify: `src/linear/queries.rs` (`ISSUE_SELECTION`, `RELATION_PAGE_SIZE`, `issue_detail`, `relations_from`, 테스트)
- Modify: `src/test_support.rs` (`IssueBuilder::parent`)

**Interfaces:**
- Produces:
  - `ParentRef { id: String, identifier: String, title: String, state: Option<StateRef> }`. `state`는 `#[serde(default)]`라서 예전 캐시에서는 `None`이다.
  - `StateRef::is_finished(&self) -> bool`: completed·canceled·duplicate면 true다.
  - `RelatedIssue { id: String, identifier: String, title: String, state: StateRef }`
  - `IssueRelations { children: Vec<RelatedIssue>, more_children: bool, blocked_by: Vec<RelatedIssue>, blocking: Vec<RelatedIssue>, related: Vec<RelatedIssue> }`. `Default`·`Serialize`·`Deserialize`를 derive하고 `is_empty(&self) -> bool`이 있다.
  - `IssueDetail.relations: IssueRelations`
  - `queries::RELATION_PAGE_SIZE: i64 = 50`
  - `IssueBuilder::parent(self, id: &str, identifier: &str, title: &str, state_type: &str) -> Self`

- [ ] **Step 1: 테스트를 쓴다**

`src/test_support.rs`에서 찾아 바꾼다.

```rust
    pub fn archived(mut self) -> Self {
```

```rust
    /// 상위 이슈 (`state_type` 상태와 함께).
    pub fn parent(mut self, id: &str, identifier: &str, title: &str, state_type: &str) -> Self {
        self.0["parent"] = json!({
            "id": id, "identifier": identifier, "title": title,
            "state": { "id": format!("st-{state_type}"), "name": state_type, "type": state_type, "color": "#5e6ad2" }
        });
        self
    }

    pub fn archived(mut self) -> Self {
```

`src/linear/types.rs` 테스트 모듈 끝에 넣는다.

```rust
    #[test]
    fn old_parent_without_state_still_reads() {
        let mut v = IssueBuilder::new("a", "ENG-2", "하위").json();
        v["parent"] = serde_json::json!({ "id": "p", "identifier": "ENG-1", "title": "상위" });
        let issue: Issue = serde_json::from_value(v).unwrap();
        assert_eq!(issue.parent.unwrap().state, None, "예전 캐시");
        let with_state = IssueBuilder::new("a", "ENG-2", "하위")
            .parent("p", "ENG-1", "상위", "started")
            .build();
        let back: Issue =
            serde_json::from_str(&serde_json::to_string(&with_state).unwrap()).unwrap();
        assert_eq!(back.parent.unwrap().state.unwrap().state_type, "started");
    }

    #[test]
    fn finished_states_are_completed_canceled_and_duplicate() {
        let state = |t: &str| StateRef {
            id: "s".into(),
            name: "s".into(),
            state_type: t.into(),
            color: "#000000".into(),
        };
        for t in ["completed", "canceled", "duplicate"] {
            assert!(state(t).is_finished(), "{t}");
        }
        for t in ["triage", "backlog", "unstarted", "started"] {
            assert!(!state(t).is_finished(), "{t}");
        }
    }
```

`src/linear/queries.rs`에서 찾아 바꾼다(기존 테스트에 한 줄 더함).

```rust
        assert!(d.more_comments);
    }
```

```rust
        assert!(d.more_comments);
        assert_eq!(
            d.relations,
            IssueRelations::default(),
            "관계 필드가 없는 응답은 빈 관계"
        );
    }
```

`src/linear/queries.rs` 테스트 모듈 끝에 넣는다.

```rust
    fn rel_issue(id: &str, identifier: &str, state_type: &str) -> Value {
        json!({
            "id": id, "identifier": identifier, "title": format!("{identifier} 제목"),
            "state": { "id": format!("st-{state_type}"), "name": state_type, "type": state_type, "color": "#5e6ad2" }
        })
    }

    #[test]
    fn detail_reads_relations_by_direction() {
        let mut server = mockito::Server::new();
        let mut issue = IssueBuilder::new("i1", "ENG-1", "상세").json();
        issue["comments"] =
            json!({ "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } });
        let mut first = rel_issue("c1", "ENG-11", "completed");
        first["subIssueSortOrder"] = json!(1.0);
        let mut second = rel_issue("c2", "ENG-12", "started");
        second["subIssueSortOrder"] = json!(2.0);
        let mut unordered = rel_issue("c3", "ENG-13", "unstarted");
        unordered["subIssueSortOrder"] = Value::Null;
        issue["children"] = json!({
            "nodes": [unordered, second, first],
            "pageInfo": { "hasNextPage": true }
        });
        issue["relations"] = json!({ "nodes": [
            { "type": "blocks", "relatedIssue": rel_issue("b1", "ENG-21", "unstarted") },
            { "type": "related", "relatedIssue": rel_issue("r1", "ENG-31", "started") },
            { "type": "duplicate", "relatedIssue": rel_issue("d1", "ENG-41", "canceled") },
            { "type": "similar", "relatedIssue": rel_issue("s1", "ENG-51", "backlog") },
            { "type": "blocks", "relatedIssue": null },
            { "type": "blocks", "relatedIssue": { "id": "x" } }
        ] });
        issue["inverseRelations"] = json!({ "nodes": [
            { "type": "blocks", "issue": rel_issue("k1", "ENG-61", "started") },
            { "type": "related", "issue": rel_issue("r1", "ENG-31", "started") },
            { "type": "related", "issue": rel_issue("r2", "ENG-32", "completed") }
        ] });
        server
            .mock("POST", "/graphql")
            .with_body(json!({ "data": { "issue": issue } }).to_string())
            .create();
        let d = issue_detail(&client(&server), "ENG-1").unwrap().unwrap();
        let ids = |list: &[RelatedIssue]| {
            list.iter()
                .map(|i| i.identifier.clone())
                .collect::<Vec<_>>()
        };
        let r = &d.relations;
        assert_eq!(
            ids(&r.children),
            vec!["ENG-11", "ENG-12", "ENG-13"],
            "Linear 하위 순서, 값이 없으면 뒤로"
        );
        assert!(r.more_children);
        assert_eq!(
            ids(&r.blocking),
            vec!["ENG-21"],
            "이 이슈가 막는 이슈. 읽을 수 없는 노드는 버린다"
        );
        assert_eq!(ids(&r.blocked_by), vec!["ENG-61"], "이 이슈를 막는 이슈");
        assert_eq!(
            ids(&r.related),
            vec!["ENG-31", "ENG-32"],
            "양쪽 related를 합치고 겹치는 것은 하나만"
        );
    }

    #[test]
    fn detail_query_asks_for_relations_and_parent_state() {
        let mut server = mockito::Server::new();
        let mut issue = IssueBuilder::new("i1", "ENG-1", "상세").json();
        issue["comments"] =
            json!({ "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } });
        let m = server
            .mock("POST", "/graphql")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex(r"parent \{ id identifier title state \{ id name type color \} \}".into()),
                Matcher::Regex(format!(r"children\(first: {RELATION_PAGE_SIZE}\) \{{ nodes \{{ id identifier title state")),
                Matcher::Regex("subIssueSortOrder".into()),
                Matcher::Regex(format!(r" relations\(first: {RELATION_PAGE_SIZE}\) \{{ nodes \{{ type relatedIssue")),
                Matcher::Regex(format!(r"inverseRelations\(first: {RELATION_PAGE_SIZE}\) \{{ nodes \{{ type issue")),
            ]))
            .with_body(json!({ "data": { "issue": issue } }).to_string())
            .create();
        issue_detail(&client(&server), "ENG-1").unwrap();
        m.assert();
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib linear::`
Expected: 컴파일 실패. `IssueRelations`, `RelatedIssue`, `RELATION_PAGE_SIZE`, `is_finished`가 없다.

- [ ] **Step 3: 타입을 더한다**

`src/linear/types.rs`에서 찾아 바꾼다.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
    /// 상위 이슈의 상태. 예전 캐시에는 없다
    #[serde(default)]
    pub state: Option<StateRef>,
}
```

찾아 바꾼다.

```rust
    #[serde(rename = "type")]
    pub state_type: String,
    pub color: String,
}
```

```rust
    #[serde(rename = "type")]
    pub state_type: String,
    pub color: String,
}

impl StateRef {
    /// 끝난 상태(완료·취소·중복)인지.
    pub fn is_finished(&self) -> bool {
        matches!(
            self.state_type.as_str(),
            "completed" | "canceled" | "duplicate"
        )
    }
}
```

찾아 바꾼다.

```rust
/// 상세 화면용: 이슈 + 코멘트(오래된 것부터).
#[derive(Debug, Clone, PartialEq)]
pub struct IssueDetail {
    pub issue: Issue,
    pub comments: Vec<Comment>,
    /// 코멘트가 50개를 넘는지.
    pub more_comments: bool,
}
```

```rust
/// 관계로 이어진 이슈. 상세 화면의 관계 칸에 쓰는 필드만 받는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub state: StateRef,
}

/// 상세 화면의 관계: 하위, 막힘, 막는 중, 관련. 상위는 `Issue::parent`에 있다.
/// 캐시에 JSON으로 저장한다. 빠진 칸은 빈 값으로 읽는다.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IssueRelations {
    /// Linear 하위 목록 순서
    pub children: Vec<RelatedIssue>,
    /// 하위가 더 있는지 (한 번에 받는 개수를 넘음)
    pub more_children: bool,
    /// 이 이슈를 막는 이슈
    pub blocked_by: Vec<RelatedIssue>,
    /// 이 이슈가 막는 이슈
    pub blocking: Vec<RelatedIssue>,
    pub related: Vec<RelatedIssue>,
}

impl IssueRelations {
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
            && self.blocked_by.is_empty()
            && self.blocking.is_empty()
            && self.related.is_empty()
    }
}

/// 상세 화면용: 이슈 + 코멘트(오래된 것부터) + 관계.
#[derive(Debug, Clone, PartialEq)]
pub struct IssueDetail {
    pub issue: Issue,
    pub comments: Vec<Comment>,
    /// 코멘트가 50개를 넘는지.
    pub more_comments: bool,
    pub relations: IssueRelations,
}
```

- [ ] **Step 4: 쿼리와 해석을 바꾼다**

`src/linear/queries.rs`에서 찾아 바꾼다.

```rust
use super::types::{Comment, Issue, IssueDetail, IssuePage, Nodes, PageInfo, Viewer};
```

```rust
use super::types::{
    Comment, Issue, IssueDetail, IssuePage, IssueRelations, Nodes, PageInfo, RelatedIssue, Viewer,
};
```

찾아 바꾼다.

```rust
pub const COMMENT_PAGE_SIZE: i64 = 50;
```

```rust
pub const COMMENT_PAGE_SIZE: i64 = 50;
/// 상세에서 하위·관계를 종류마다 몇 개까지 받는지.
pub const RELATION_PAGE_SIZE: i64 = 50;

/// 관계로 이어진 이슈에서 받는 필드 (`RelatedIssue`).
const RELATED_SELECTION: &str = "id identifier title state { id name type color }";
```

찾아 바꾼다(`ISSUE_SELECTION` 안).

```text
parent { id identifier title } labels(first: 20)
```

```text
parent { id identifier title state { id name type color } } labels(first: 20)
```

`issue_detail` 함수 전체를 찾아 바꾼다. 찾을 블록은 `/// 이슈와 코멘트. 이슈가 없으면 \`None\`.`부터 그 함수의 마지막 `}`까지다.

```rust
/// 이슈와 코멘트. 이슈가 없으면 `None`.
pub fn issue_detail(c: &LinearClient, id: &str) -> Result<Option<IssueDetail>, ApiError> {
    #[derive(Deserialize)]
    struct D {
        issue: DetailIssue,
    }
    #[derive(Deserialize)]
    struct DetailIssue {
        #[serde(flatten)]
        issue: Issue,
        comments: CommentPage,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CommentPage {
        nodes: Vec<Comment>,
        page_info: PageInfo,
    }
    let q = format!(
        "query Detail($id: String!, $first: Int) {{ issue(id: $id) {{ ...IssueFields comments(first: $first) {{ nodes {{ id body createdAt editedAt user {{ id name displayName }} }} pageInfo {{ hasNextPage endCursor }} }} }} }} {}",
        issue_fragment()
    );
    match c.execute::<D>(&q, json!({ "id": id, "first": COMMENT_PAGE_SIZE })) {
        Ok(d) => {
            let mut comments = d.issue.comments.nodes;
            comments.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            Ok(Some(IssueDetail {
                issue: d.issue.issue,
                comments,
                more_comments: d.issue.comments.page_info.has_next_page,
            }))
        }
        Err(ApiError::GraphQl(msg)) if is_not_found(&msg) => Ok(None),
        Err(e) => Err(e),
    }
}
```

```rust
/// 이슈, 코멘트, 하위·관계. 이슈가 없으면 `None`.
pub fn issue_detail(c: &LinearClient, id: &str) -> Result<Option<IssueDetail>, ApiError> {
    #[derive(Deserialize)]
    struct D {
        issue: DetailIssue,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DetailIssue {
        #[serde(flatten)]
        issue: Issue,
        comments: CommentPage,
        /// 관계 필드는 예전 응답 모양에서도 읽히도록 없어도 된다
        #[serde(default)]
        children: Option<ChildPage>,
        #[serde(default)]
        relations: Option<Nodes<Value>>,
        #[serde(default)]
        inverse_relations: Option<Nodes<Value>>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CommentPage {
        nodes: Vec<Comment>,
        page_info: PageInfo,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ChildPage {
        nodes: Vec<Value>,
        page_info: PageInfo,
    }
    let q = format!(
        "query Detail($id: String!, $first: Int) {{ issue(id: $id) {{ ...IssueFields \
         comments(first: $first) {{ nodes {{ id body createdAt editedAt user {{ id name displayName }} }} pageInfo {{ hasNextPage endCursor }} }} \
         children(first: {RELATION_PAGE_SIZE}) {{ nodes {{ {RELATED_SELECTION} subIssueSortOrder }} pageInfo {{ hasNextPage }} }} \
         relations(first: {RELATION_PAGE_SIZE}) {{ nodes {{ type relatedIssue {{ {RELATED_SELECTION} }} }} }} \
         inverseRelations(first: {RELATION_PAGE_SIZE}) {{ nodes {{ type issue {{ {RELATED_SELECTION} }} }} }} }} }} {}",
        issue_fragment()
    );
    match c.execute::<D>(&q, json!({ "id": id, "first": COMMENT_PAGE_SIZE })) {
        Ok(d) => {
            let mut comments = d.issue.comments.nodes;
            comments.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            let (children, more_children) = d
                .issue
                .children
                .map_or((Vec::new(), false), |p| (p.nodes, p.page_info.has_next_page));
            let relations = relations_from(
                children,
                more_children,
                d.issue.relations.map(|n| n.nodes).unwrap_or_default(),
                d.issue.inverse_relations.map(|n| n.nodes).unwrap_or_default(),
            );
            Ok(Some(IssueDetail {
                issue: d.issue.issue,
                comments,
                more_comments: d.issue.comments.page_info.has_next_page,
                relations,
            }))
        }
        Err(ApiError::GraphQl(msg)) if is_not_found(&msg) => Ok(None),
        Err(e) => Err(e),
    }
}

/// 하위 노드 하나.
#[derive(Deserialize)]
struct ChildNode {
    #[serde(flatten)]
    issue: RelatedIssue,
    #[serde(default, rename = "subIssueSortOrder")]
    sort_order: Option<f64>,
}

/// 관계 노드 하나. `relations`는 `relatedIssue`가, `inverseRelations`는 `issue`가 상대 이슈다.
#[derive(Deserialize)]
struct RelationNode {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    issue: Option<RelatedIssue>,
    #[serde(default, rename = "relatedIssue")]
    related_issue: Option<RelatedIssue>,
}

/// 상세 응답의 하위·관계 노드를 화면에 쓰는 모양으로 바꾼다.
/// `blocks`는 방향으로 나누고(`relations` = 이 이슈가 막는 이슈, `inverseRelations` = 이 이슈를 막는 이슈),
/// `related`는 양쪽을 합친다. 그 밖의 종류와 읽을 수 없는 노드는 버린다.
fn relations_from(
    children: Vec<Value>,
    more_children: bool,
    relations: Vec<Value>,
    inverse: Vec<Value>,
) -> IssueRelations {
    let mut kids: Vec<ChildNode> = children
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    // Linear 하위 목록 순서. 값이 없으면 뒤로 보내고, 같으면 받은 순서를 지킨다
    kids.sort_by(|a, b| match (a.sort_order, b.sort_order) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    let nodes = |list: Vec<Value>| -> Vec<RelationNode> {
        list.into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect()
    };
    let mut out = IssueRelations {
        children: kids.into_iter().map(|k| k.issue).collect(),
        more_children,
        ..IssueRelations::default()
    };
    for n in nodes(relations) {
        let Some(other) = n.related_issue else {
            continue;
        };
        match n.kind.as_str() {
            "blocks" => push_unique(&mut out.blocking, other),
            "related" => push_unique(&mut out.related, other),
            _ => {}
        }
    }
    for n in nodes(inverse) {
        let Some(other) = n.issue else {
            continue;
        };
        match n.kind.as_str() {
            "blocks" => push_unique(&mut out.blocked_by, other),
            "related" => push_unique(&mut out.related, other),
            _ => {}
        }
    }
    out
}

fn push_unique(list: &mut Vec<RelatedIssue>, issue: RelatedIssue) {
    if !list.iter().any(|i| i.id == issue.id) {
        list.push(issue);
    }
}
```

Task 1 결과에 따라 두 가지를 바꿀 수 있다.
- **`blocks` 방향이 반대로 나왔을 때**: 두 `"blocks" =>` 줄의 대상(`out.blocking`과 `out.blocked_by`)을 서로 바꾸고 주석도 고친다. Step 1 테스트 `detail_reads_relations_by_direction`의 `blocking`과 `blocked_by` 기대값(`ENG-21`, `ENG-61`)도 서로 바꾼다.
- **상세 쿼리 복잡도가 2,000 이상이었을 때**: `RELATION_PAGE_SIZE`를 20으로 한다. 테스트는 상수를 읽으므로 고칠 것이 없다.

- [ ] **Step 5: 통과 확인**

Run: `cargo test --lib linear:: && cargo test`
Expected: 모두 통과. 새 테스트 4개가 늘어 260개 통과, 1개 무시다.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: 경고 없음. `cargo fmt`가 줄을 바꾸면 그대로 받아들인다.

- [ ] **Step 6: 커밋**

```bash
git add src/linear/types.rs src/linear/queries.rs src/test_support.rs
git commit -m "feat(linear): 상세 쿼리로 하위·막힘·막는 중·관련 이슈를 받고 상위 이슈 상태도 받기"
```

---

### Task 3: 관계 캐시

**Files:**
- Modify: `src/store/mod.rs` (`SCHEMA_VERSION` 2, `relations` 테이블, 버전 1 옮기기, 테스트)
- Modify: `src/store/cache.rs` (`set_relations`, `get_relations`, 지우는 곳 넷, 테스트)

**Interfaces:**
- Consumes: `IssueRelations`, `RelatedIssue` (Task 2)
- Produces:
  - `Store::set_relations(&self, issue_id: &str, relations: &IssueRelations, now_ms: i64) -> Result<()>`
  - `Store::get_relations(&self, issue_id: &str) -> Result<Option<(IssueRelations, i64)>>`
  - `store::SCHEMA_VERSION = 2`

- [ ] **Step 1: 테스트를 쓴다**

`src/store/cache.rs`에서 찾아 바꾼다.

```rust
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;
```

```rust
mod tests {
    use super::*;
    use crate::linear::types::RelatedIssue;
    use crate::test_support::IssueBuilder;
```

`src/store/cache.rs` 테스트 모듈 끝에 넣는다.

```rust
    fn relations() -> IssueRelations {
        let i = IssueBuilder::new("b1", "ENG-9", "막는 이슈").build();
        IssueRelations {
            blocked_by: vec![RelatedIssue {
                id: i.id,
                identifier: i.identifier,
                title: i.title,
                state: i.state,
            }],
            more_children: true,
            ..IssueRelations::default()
        }
    }

    #[test]
    fn relations_roundtrip() {
        let s = store();
        assert_eq!(s.get_relations("i1").unwrap(), None);
        s.set_relations("i1", &relations(), 500).unwrap();
        assert_eq!(s.get_relations("i1").unwrap(), Some((relations(), 500)));
        s.set_relations("i1", &IssueRelations::default(), 600).unwrap();
        assert_eq!(
            s.get_relations("i1").unwrap(),
            Some((IssueRelations::default(), 600)),
            "덮어쓴다"
        );
    }

    #[test]
    fn relations_go_away_with_their_issue() {
        let s = store();
        for (id, ident) in [("a", "ENG-1"), ("b", "ENG-2"), ("c", "ENG-3"), ("d", "ENG-4")] {
            s.upsert_issues(&[IssueBuilder::new(id, ident, "제목").build()], 100)
                .unwrap();
            s.set_relations(id, &relations(), 100).unwrap();
        }
        s.upsert_issues(
            &[IssueBuilder::new("a", "ENG-1", "제목").archived().build()],
            200,
        )
        .unwrap();
        assert_eq!(s.get_relations("a").unwrap(), None, "보관된 이슈");
        s.remove_issue("b").unwrap();
        assert_eq!(s.get_relations("b").unwrap(), None, "지운 이슈");
        s.mark_viewed("c", 1_000).unwrap();
        s.evict_older_than(500).unwrap();
        assert_eq!(s.get_relations("d").unwrap(), None, "오래된 이슈");
        assert!(s.get_relations("c").unwrap().is_some(), "최근 본 이슈는 남는다");
        s.clear_all().unwrap();
        assert_eq!(s.get_relations("c").unwrap(), None, "모두 비우기");
    }
```

`src/store/mod.rs` 테스트 모듈 끝에 넣는다.

```rust
    #[test]
    fn version_1_file_keeps_its_data_and_gains_relations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        {
            let store = Store::open(&path).unwrap();
            store.meta_set("k", "v").unwrap();
            // 버전 1 파일: 관계 테이블이 없다
            store.conn.execute_batch("DROP TABLE relations").unwrap();
            store.conn.pragma_update(None, "user_version", 1).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.meta_get("k").unwrap().as_deref(),
            Some("v"),
            "기존 캐시가 남는다"
        );
        let v: i64 = store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        store
            .set_relations("i1", &crate::linear::types::IssueRelations::default(), 1)
            .unwrap();
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib store::`
Expected: 컴파일 실패. `set_relations`·`get_relations`가 없다.

- [ ] **Step 3: 스키마와 옮기기**

`src/store/mod.rs`에서 찾아 바꾼다.

```rust
/// 스키마 버전. 다르면 캐시를 비우고 새로 만든다.
pub const SCHEMA_VERSION: i64 = 1;
```

```rust
/// 스키마 버전. 1(관계 테이블 없음)이면 테이블만 더하고, 그 밖의 다른 값이면 캐시를 비우고 새로 만든다.
pub const SCHEMA_VERSION: i64 = 2;
```

찾아 바꾼다.

```rust
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY (repo, branch)
);
";
```

```rust
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY (repo, branch)
);
CREATE TABLE IF NOT EXISTS relations (
  issue_id   TEXT PRIMARY KEY,
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
";
```

찾아 바꾼다.

```rust
        if version != SCHEMA_VERSION {
            if version != 0 {
                drop_all_tables(&tx)?;
            }
```

```rust
        if version != SCHEMA_VERSION {
            // 0은 새 파일이거나 만들다 끊긴 파일, 1은 관계 테이블만 없는 파일이다.
            // 테이블을 모두 IF NOT EXISTS로 만들므로 둘은 비우지 않고 채운다
            if !matches!(version, 0 | 1) {
                drop_all_tables(&tx)?;
            }
```

- [ ] **Step 4: 읽기·쓰기와 지우기**

`src/store/cache.rs`에서 찾아 바꾼다.

```rust
use crate::linear::types::{Comment, Issue};
```

```rust
use crate::linear::types::{Comment, Issue, IssueRelations};
```

찾아 바꾼다.

```rust
            "DELETE FROM meta; DELETE FROM issues; DELETE FROM comments; DELETE FROM view_results;
             DELETE FROM team_refs; DELETE FROM workspace_labels; DELETE FROM branch_map;",
```

```rust
            "DELETE FROM meta; DELETE FROM issues; DELETE FROM comments; DELETE FROM view_results;
             DELETE FROM team_refs; DELETE FROM workspace_labels; DELETE FROM branch_map;
             DELETE FROM relations;",
```

찾아 바꾼다(`upsert_issues`의 보관 이슈 처리).

```rust
                tx.execute(
                    "DELETE FROM comments WHERE issue_id = ?1",
                    params![issue.id],
                )?;
                removed.push(issue.id.clone());
```

```rust
                tx.execute(
                    "DELETE FROM comments WHERE issue_id = ?1",
                    params![issue.id],
                )?;
                tx.execute(
                    "DELETE FROM relations WHERE issue_id = ?1",
                    params![issue.id],
                )?;
                removed.push(issue.id.clone());
```

찾아 바꾼다(`remove_issue`).

```rust
        self.conn
            .execute("DELETE FROM comments WHERE issue_id = ?1", params![id])?;
        Ok(())
    }
```

```rust
        self.conn
            .execute("DELETE FROM comments WHERE issue_id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM relations WHERE issue_id = ?1", params![id])?;
        Ok(())
    }
```

찾아 바꾼다(`evict_older_than`).

```rust
        self.conn.execute(
            "DELETE FROM comments WHERE issue_id NOT IN (SELECT id FROM issues)",
            [],
        )?;
        Ok(n)
```

```rust
        self.conn.execute(
            "DELETE FROM comments WHERE issue_id NOT IN (SELECT id FROM issues)",
            [],
        )?;
        self.conn.execute(
            "DELETE FROM relations WHERE issue_id NOT IN (SELECT id FROM issues)",
            [],
        )?;
        Ok(n)
```

찾아 바꾼다.

```rust
    /// 상세 화면을 열었다고 기록한다 ("최근 본").
```

```rust
    /// 상세의 관계(하위·막힘·막는 중·관련)를 저장한다.
    pub fn set_relations(
        &self,
        issue_id: &str,
        relations: &IssueRelations,
        now_ms: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO relations (issue_id, data, fetched_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(issue_id) DO UPDATE SET data = excluded.data, fetched_at = excluded.fetched_at",
            params![issue_id, serde_json::to_string(relations)?, now_ms],
        )?;
        Ok(())
    }

    pub fn get_relations(&self, issue_id: &str) -> Result<Option<(IssueRelations, i64)>> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT data, fetched_at FROM relations WHERE issue_id = ?1",
                params![issue_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((data, at)) => Some((serde_json::from_str(&data)?, at)),
            None => None,
        })
    }

    /// 상세 화면을 열었다고 기록한다 ("최근 본").
```

- [ ] **Step 5: 통과 확인**

Run: `cargo test --lib store:: && cargo test`
Expected: 모두 통과(263개 통과, 1개 무시). 기존 `version_mismatch_recreates_cache`(버전 99)도 그대로 통과한다.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: 경고 없음.

- [ ] **Step 6: 커밋**

```bash
git add src/store/mod.rs src/store/cache.rs
git commit -m "feat(store): 관계 캐시 테이블 — 버전 1 캐시는 비우지 않고 테이블만 더하기"
```

---

### Task 4: 관계 칸 줄 (`ui::relations`)

TUI 상세, 관계 메뉴, CLI가 함께 쓰는 순수 모듈이다.

**Files:**
- Create: `src/ui/relations.rs`
- Modify: `src/ui/mod.rs`

**Interfaces:**
- Consumes: `IssueRelations`, `RelatedIssue`, `ParentRef`, `StateRef::is_finished` (Task 2), `ui::style::{DIM, state_icon, state_style, truncate}`, `markdown::sanitize`
- Produces:
  - `pub enum RelKind { Parent, BlockedBy, Blocking, Related, Child }`과 `RelKind::label(self) -> &'static str`
  - `pub struct RelRow { pub kind: RelKind, pub id: String, pub identifier: String, pub title: String, pub state: Option<StateRef> }`. `Debug, Clone, PartialEq, Eq`를 derive한다.
  - `RelRow::finished(&self) -> bool`: 상태를 모르면 false다.
  - `RelRow::filter_text(&self) -> String`: `"막힘 ENG-140 API 스키마 확정"` 모양이다.
  - `pub fn rows(parent: Option<&ParentRef>, relations: Option<&IssueRelations>) -> Vec<RelRow>`
  - `pub fn lines(parent: Option<&ParentRef>, relations: Option<&IssueRelations>, width: u16) -> (Vec<Line<'static>>, Vec<Option<usize>>)`
  - `pub fn row_line(row: &RelRow, show_kind: bool, width: u16) -> Line<'static>`

- [ ] **Step 1: 모듈을 등록하고 테스트를 쓴다**

`src/ui/mod.rs` 전체를 바꾼다.

```rust
//! 화면 공통: Linear 색, 상태 아이콘, 목록 한 줄, 상세의 관계 칸.

pub mod relations;
pub mod row;
pub mod style;
```

`src/ui/relations.rs`를 만든다. 먼저 테스트만 넣고, 구현은 Step 3에서 테스트 위에 넣는다.

```rust
//! 상세 화면의 관계 칸: 상위·막힘·막는 중·관련·하위. TUI 상세, 관계 메뉴, CLI가 함께 쓴다.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::to_plain;
    use crate::test_support::IssueBuilder;

    fn rel(id: &str, identifier: &str, title: &str, state_type: &str) -> RelatedIssue {
        let i = IssueBuilder::new(id, identifier, title)
            .state(state_type, state_type)
            .build();
        RelatedIssue {
            id: i.id,
            identifier: i.identifier,
            title: i.title,
            state: i.state,
        }
    }

    fn parent() -> ParentRef {
        IssueBuilder::new("x", "ENG-1", "하위")
            .parent("p", "ENG-120", "인증 개편", "started")
            .build()
            .parent
            .unwrap()
    }

    fn sample() -> IssueRelations {
        IssueRelations {
            blocked_by: vec![
                rel("b2", "ENG-141", "토큰 형식 정리", "completed"),
                rel("b1", "ENG-140", "API 스키마 확정", "unstarted"),
            ],
            blocking: vec![rel("k1", "ENG-150", "결제 페이지 리팩터링", "unstarted")],
            related: vec![rel("r1", "ENG-101", "로그인 화면 개편", "completed")],
            children: vec![
                rel("c1", "ENG-132", "토큰 갱신", "completed"),
                rel("c2", "ENG-133", "세션 만료 처리", "canceled"),
                rel("c3", "ENG-134", "자동 로그인", "started"),
                rel("c4", "ENG-135", "로그아웃 정리", "unstarted"),
            ],
            more_children: false,
        }
    }

    fn text(out: &[Line<'_>]) -> Vec<String> {
        to_plain(out)
            .lines()
            .map(|l| l.trim_end().to_string())
            .collect()
    }

    #[test]
    fn rows_follow_kind_order_with_unfinished_first() {
        let r = sample();
        let got: Vec<(RelKind, String)> = rows(Some(&parent()), Some(&r))
            .into_iter()
            .map(|x| (x.kind, x.identifier))
            .collect();
        let want = [
            (RelKind::Parent, "ENG-120"),
            (RelKind::BlockedBy, "ENG-140"),
            (RelKind::BlockedBy, "ENG-141"),
            (RelKind::Blocking, "ENG-150"),
            (RelKind::Related, "ENG-101"),
            (RelKind::Child, "ENG-132"),
            (RelKind::Child, "ENG-133"),
            (RelKind::Child, "ENG-134"),
            (RelKind::Child, "ENG-135"),
        ];
        assert_eq!(
            got,
            want.map(|(k, i)| (k, i.to_string())).to_vec(),
            "하위는 받은 순서(Linear 순서) 그대로"
        );
        assert!(rows(None, None).is_empty());
        assert_eq!(rows(Some(&parent()), None).len(), 1, "관계를 모르면 상위만");
    }

    #[test]
    fn lines_show_kind_once_and_summarize_children() {
        let r = sample();
        let (out, map) = lines(Some(&parent()), Some(&r), 80);
        assert_eq!(
            text(&out),
            vec![
                "상위    ◐ ENG-120   인증 개편",
                "막힘    ○ ENG-140   API 스키마 확정",
                "        ● ENG-141   토큰 형식 정리",
                "막는 중 ○ ENG-150   결제 페이지 리팩터링",
                "관련    ● ENG-101   로그인 화면 개편",
                "하위    4개 중 2개 남음",
                "        ● ENG-132   토큰 갱신",
                "        ✕ ENG-133   세션 만료 처리",
                "        ◐ ENG-134   자동 로그인",
                "        ○ ENG-135   로그아웃 정리",
            ]
        );
        assert_eq!(
            map,
            vec![
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                None,
                Some(5),
                Some(6),
                Some(7),
                Some(8)
            ]
        );
    }

    #[test]
    fn colors_mark_open_blockers_and_finished_issues() {
        let r = sample();
        let (out, _) = lines(Some(&parent()), Some(&r), 80);
        let fg = |line: &Line<'_>, i: usize| line.spans[i].style.fg;
        assert_eq!(fg(&out[1], 0), Some(Color::Red), "안 끝난 막는 이슈가 있으면 막힘은 빨강");
        assert_eq!(fg(&out[3], 0), Some(Color::Yellow), "막는 중은 노랑");
        assert_eq!(fg(&out[0], 0), DIM.fg, "상위 칸 이름은 흐리게");
        assert_eq!(fg(&out[1], 1), Some(Color::Rgb(94, 106, 210)), "아이콘은 Linear 상태 색");
        assert_eq!(fg(&out[1], 2), DIM.fg, "식별자는 흐리게");
        assert_eq!(fg(&out[1], 3), None, "안 끝난 이슈 제목은 기본색");
        assert_eq!(fg(&out[2], 3), DIM.fg, "끝난 이슈 제목은 흐리게");
        assert_eq!(out[5].spans[2].style.fg, None, "남은 개수는 기본색");
        let done = IssueRelations {
            blocked_by: vec![rel("b2", "ENG-141", "토큰 형식 정리", "completed")],
            children: vec![rel("c1", "ENG-132", "토큰 갱신", "completed")],
            ..IssueRelations::default()
        };
        let (out, _) = lines(None, Some(&done), 80);
        assert_eq!(fg(&out[0], 0), DIM.fg, "막는 이슈가 다 끝나면 막힘도 흐리게");
        assert_eq!(text(&out)[1], "하위    1개 모두 끝남");
        assert_eq!(out[1].spans[1].style.fg, Some(Color::Green), "모두 끝나면 초록");
    }

    #[test]
    fn more_children_says_so_and_points_to_the_browser() {
        let r = IssueRelations {
            children: vec![rel("c1", "ENG-132", "토큰 갱신", "started")],
            more_children: true,
            ..IssueRelations::default()
        };
        let (out, map) = lines(None, Some(&r), 80);
        assert_eq!(
            text(&out),
            vec![
                "하위    1개 넘음",
                "        ◐ ENG-132   토큰 갱신",
                "        … 더 있어요 (o로 브라우저에서 보기)",
            ]
        );
        assert_eq!(map, vec![None, Some(0), None]);
    }

    #[test]
    fn narrow_width_truncates_the_title() {
        let r = IssueRelations {
            blocked_by: vec![rel("b1", "ENG-140", "API 스키마 확정", "unstarted")],
            ..IssueRelations::default()
        };
        let (out, _) = lines(None, Some(&r), 24);
        assert_eq!(text(&out), vec!["막힘    ○ ENG-140   API…"]);
        assert!(out[0].width() <= 24);
        let (out, _) = lines(None, Some(&r), 10);
        assert!(!text(&out)[0].contains("API"), "제목 자리가 없으면 뺀다");
    }

    #[test]
    fn parent_without_state_has_no_icon() {
        let p = ParentRef {
            id: "p".into(),
            identifier: "ENG-120".into(),
            title: "인증 개편".into(),
            state: None,
        };
        let (out, _) = lines(Some(&p), None, 80);
        assert_eq!(text(&out), vec!["상위      ENG-120   인증 개편"]);
        assert!(!rows(Some(&p), None)[0].finished(), "상태를 모르면 안 끝난 것으로 본다");
    }

    #[test]
    fn filter_text_and_lines_have_no_control_characters() {
        let bad = RelatedIssue {
            id: "r1".into(),
            identifier: "ENG-1\u{1b}[2J".into(),
            title: "제목\u{7}".into(),
            state: rel("x", "ENG-2", "t", "started").state,
        };
        let r = IssueRelations {
            related: vec![bad],
            ..IssueRelations::default()
        };
        let (out, _) = lines(None, Some(&r), 80);
        assert!(!to_plain(&out).chars().any(|c| c.is_control()));
        let row = &rows(None, Some(&r))[0];
        assert!(!row.filter_text().chars().any(char::is_control));
        assert_eq!(rows(None, Some(&sample()))[0].filter_text(), "막힘 ENG-140 API 스키마 확정");
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib ui::relations`
Expected: 컴파일 실패. `rows`, `lines`, `RelKind` 등이 없다.

- [ ] **Step 3: 구현을 넣는다**

`src/ui/relations.rs`의 첫 줄 문서 주석 바로 아래, `#[cfg(test)]` 위에 넣는다.

```rust
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::style::{DIM, state_icon, state_style, truncate};
use crate::linear::types::{IssueRelations, ParentRef, RelatedIssue, StateRef};
use crate::markdown::sanitize;

/// 칸 이름 폭. 가장 긴 `막는 중`이 7칸이다.
const KIND_WIDTH: usize = 8;

/// 관계 종류. 화면에는 이 순서로 나온다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelKind {
    Parent,
    BlockedBy,
    Blocking,
    Related,
    Child,
}

impl RelKind {
    pub fn label(self) -> &'static str {
        match self {
            RelKind::Parent => "상위",
            RelKind::BlockedBy => "막힘",
            RelKind::Blocking => "막는 중",
            RelKind::Related => "관련",
            RelKind::Child => "하위",
        }
    }
}

/// 관계 한 줄.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelRow {
    pub kind: RelKind,
    pub id: String,
    pub identifier: String,
    pub title: String,
    /// 상위 이슈는 예전 캐시에서 상태를 모를 수 있다
    pub state: Option<StateRef>,
}

impl RelRow {
    /// 끝난 이슈(완료·취소·중복)인지. 상태를 모르면 안 끝난 것으로 본다.
    pub fn finished(&self) -> bool {
        self.state.as_ref().is_some_and(StateRef::is_finished)
    }

    /// 메뉴에서 거를 때 쓰는 글자: 칸 이름, 식별자, 제목.
    pub fn filter_text(&self) -> String {
        sanitize(&format!(
            "{} {} {}",
            self.kind.label(),
            self.identifier,
            self.title
        ))
    }
}

fn row(kind: RelKind, i: &RelatedIssue) -> RelRow {
    RelRow {
        kind,
        id: i.id.clone(),
        identifier: i.identifier.clone(),
        title: i.title.clone(),
        state: Some(i.state.clone()),
    }
}

/// 표시 순서대로 늘어놓은 관계 줄: 상위 → 막힘 → 막는 중 → 관련 → 하위.
/// 막힘·막는 중·관련은 안 끝난 이슈를 앞에 둔다. 하위는 받은 순서(Linear 하위 순서) 그대로다.
/// 앱(메뉴·클릭)과 화면이 같은 번호를 쓰도록 순서는 여기서만 정한다.
pub fn rows(parent: Option<&ParentRef>, relations: Option<&IssueRelations>) -> Vec<RelRow> {
    let mut out = Vec::new();
    if let Some(p) = parent {
        out.push(RelRow {
            kind: RelKind::Parent,
            id: p.id.clone(),
            identifier: p.identifier.clone(),
            title: p.title.clone(),
            state: p.state.clone(),
        });
    }
    let Some(r) = relations else {
        return out;
    };
    for (kind, list) in [
        (RelKind::BlockedBy, &r.blocked_by),
        (RelKind::Blocking, &r.blocking),
        (RelKind::Related, &r.related),
    ] {
        let mut group: Vec<RelRow> = list.iter().map(|i| row(kind, i)).collect();
        // 안정 정렬이라 같은 쪽끼리는 받은 순서를 지킨다
        group.sort_by_key(RelRow::finished);
        out.extend(group);
    }
    out.extend(r.children.iter().map(|i| row(RelKind::Child, i)));
    out
}

/// 칸 이름 스타일. 막힘·막는 중은 그 줄의 이슈가 안 끝났을 때만 색을 입힌다.
fn kind_style(row: &RelRow) -> Style {
    match row.kind {
        RelKind::BlockedBy if !row.finished() => Style::new().fg(Color::Red),
        RelKind::Blocking if !row.finished() => Style::new().fg(Color::Yellow),
        _ => DIM,
    }
}

fn pad(s: &str, width: usize) -> String {
    format!("{s}{}", " ".repeat(width.saturating_sub(s.width())))
}

/// 관계 한 줄: 칸 이름(8칸) · 상태 아이콘 · 식별자 · 제목.
/// `show_kind`가 false면 칸 이름 자리를 비운다. 제목은 `width`에 맞춰 자르고, 자리가 없으면 뺀다.
pub fn row_line(row: &RelRow, show_kind: bool, width: u16) -> Line<'static> {
    let kind = if show_kind {
        pad(row.kind.label(), KIND_WIDTH)
    } else {
        " ".repeat(KIND_WIDTH)
    };
    let icon = match &row.state {
        Some(s) => Span::styled(format!("{} ", state_icon(&s.state_type)), state_style(s)),
        None => Span::raw("  "),
    };
    let id = Span::styled(format!("{:<9} ", sanitize(&row.identifier)), DIM);
    let used = KIND_WIDTH + icon.width() + id.width();
    let title = truncate(
        &sanitize(&row.title),
        usize::from(width).saturating_sub(used),
    );
    let title_style = if row.finished() { DIM } else { Style::new() };
    Line::from(vec![
        Span::styled(kind, kind_style(row)),
        icon,
        id,
        Span::styled(title, title_style),
    ])
}

/// `하위    4개 중 2개 남음` / `하위    4개 모두 끝남` / `하위    50개 넘음`.
fn children_summary(rows: &[RelRow], relations: Option<&IssueRelations>) -> Line<'static> {
    let kids: Vec<&RelRow> = rows.iter().filter(|r| r.kind == RelKind::Child).collect();
    let total = kids.len();
    let left = kids.iter().filter(|r| !r.finished()).count();
    let mut spans = vec![Span::styled(pad(RelKind::Child.label(), KIND_WIDTH), DIM)];
    if relations.is_some_and(|r| r.more_children) {
        spans.push(Span::styled(format!("{total}개 넘음"), DIM));
    } else if left == 0 {
        spans.push(Span::styled(
            format!("{total}개 모두 끝남"),
            Style::new().fg(Color::Green),
        ));
    } else {
        spans.push(Span::styled(format!("{total}개 중 "), DIM));
        spans.push(Span::raw(format!("{left}개 남음")));
    }
    Line::from(spans)
}

/// 관계 칸의 줄과, 줄마다 대응하는 `rows` 번호. 요약 줄과 안내 줄은 `None`이다.
/// 칸 이름은 그 칸의 첫 줄에만 쓴다. 하위는 요약 줄을 먼저 쓰고 그 아래에 이슈를 늘어놓는다.
/// 관계가 하나도 없으면 빈 목록이다.
pub fn lines(
    parent: Option<&ParentRef>,
    relations: Option<&IssueRelations>,
    width: u16,
) -> (Vec<Line<'static>>, Vec<Option<usize>>) {
    let rows = rows(parent, relations);
    let mut out = Vec::new();
    let mut map = Vec::new();
    let mut prev: Option<RelKind> = None;
    for (i, row) in rows.iter().enumerate() {
        let first = prev != Some(row.kind);
        prev = Some(row.kind);
        if row.kind == RelKind::Child && first {
            out.push(children_summary(&rows, relations));
            map.push(None);
            out.push(row_line(row, false, width));
        } else {
            out.push(row_line(row, first, width));
        }
        map.push(Some(i));
    }
    if relations.is_some_and(|r| r.more_children) {
        out.push(Line::from(Span::styled(
            format!(
                "{}… 더 있어요 (o로 브라우저에서 보기)",
                " ".repeat(KIND_WIDTH)
            ),
            DIM,
        )));
        map.push(None);
    }
    (out, map)
}
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test --lib ui::relations && cargo test`
Expected: 모두 통과(270개 통과, 1개 무시).

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: 경고 없음.

- [ ] **Step 5: 커밋**

```bash
git add src/ui/mod.rs src/ui/relations.rs
git commit -m "feat(ui): 관계 칸 줄 — 순서·칸 이름·색·하위 요약 (TUI·CLI 공용)"
```

---

### Task 5: 앱 — 관계 메뉴, 관계 이슈 열기, 상세 스택

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/runtime.rs`: `Msg::Detail`을 만드는 두 곳(캐시는 `None`, 응답은 `Some`)과 테스트 패턴 한 곳
- Modify: `src/tui/view.rs`: 테스트에서 `Msg::Detail`을 만드는 두 곳

**Interfaces:**
- Consumes: `IssueRelations` (Task 2), `ui::relations::{rows, RelRow}`, `RelRow::filter_text` (Task 4)
- Produces:
  - `Act::Relations`: 관계 메뉴를 연다.
  - `Act::OpenRelated(RelRow)`: 관계 이슈를 연다. 지금 상세는 스택에 쌓는다.
  - `Input::ClickRelation(usize)`: 값은 `rows` 번호다.
  - `Msg::Detail { id, issue, comments, more, relations: Option<IssueRelations>, fresh }`
  - `Detail.relations: Option<IssueRelations>`: `None`이면 아직 모른다.
  - `App.detail_stack: Vec<Detail>`: 비공개다.

- [ ] **Step 1: 테스트를 쓴다**

`src/tui/app.rs`에서 찾아 바꾼다.

```rust
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;

    const T0: i64 = 1_000_000;
```

```rust
mod tests {
    use super::*;
    use crate::linear::types::RelatedIssue;
    use crate::test_support::IssueBuilder;

    const T0: i64 = 1_000_000;
```

`src/tui/app.rs` 테스트 모듈 끝에 넣는다.

```rust
    fn rel(id: &str, identifier: &str, title: &str, state_type: &str) -> RelatedIssue {
        let i = IssueBuilder::new(id, identifier, title)
            .state(state_type, state_type)
            .build();
        RelatedIssue {
            id: i.id,
            identifier: i.identifier,
            title: i.title,
            state: i.state,
        }
    }

    /// ENG-1(상위 ENG-10): 막힘 ENG-20, 하위 ENG-30·ENG-31.
    fn related() -> (Issue, IssueRelations) {
        let issue = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .parent("p", "ENG-10", "인증 개편", "started")
            .build();
        let relations = IssueRelations {
            blocked_by: vec![rel("b", "ENG-20", "API 스키마", "unstarted")],
            children: vec![
                rel("c1", "ENG-30", "토큰 갱신", "started"),
                rel("c2", "ENG-31", "세션 만료", "completed"),
            ],
            ..IssueRelations::default()
        };
        (issue, relations)
    }

    fn detail_msg(id: &str, issue: Issue, relations: Option<IssueRelations>, fresh: bool) -> Msg {
        Msg::Detail {
            id: id.into(),
            issue,
            comments: Vec::new(),
            more: false,
            relations,
            fresh,
        }
    }

    /// 첫 이슈의 상세를 열고 관계까지 받은 앱.
    fn with_relations() -> App {
        let mut app = started();
        app.handle(Input::Enter, T0);
        let (issue, relations) = related();
        app.apply(detail_msg("a", issue, Some(relations), true), T0);
        app
    }

    fn menu_labels(app: &App) -> Vec<String> {
        app.menu
            .as_ref()
            .unwrap()
            .visible()
            .iter()
            .map(|(l, _)| l.clone())
            .collect()
    }

    #[test]
    fn relations_menu_lists_rows_in_screen_order() {
        let mut app = with_relations();
        app.handle(Input::Act(Act::Relations), T0);
        assert_eq!(app.menu.as_ref().unwrap().title, "관계");
        assert_eq!(
            menu_labels(&app),
            vec![
                "상위 ENG-10 인증 개편",
                "막힘 ENG-20 API 스키마",
                "하위 ENG-30 토큰 갱신",
                "하위 ENG-31 세션 만료",
            ]
        );
    }

    #[test]
    fn opening_a_relation_stacks_the_detail_and_esc_comes_back() {
        let mut app = with_relations();
        app.set_detail_max_scroll(10);
        app.handle(Input::Down, T0);
        app.handle(Input::Down, T0);
        app.handle(Input::Act(Act::Relations), T0);
        type_str(&mut app, "31", T0);
        let effects = app.handle(Input::Enter, T0);
        assert_eq!(effects, vec![Effect::OpenDetail("c2".into())]);
        assert_eq!(app.mode, Mode::Detail);
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.id, "c2");
        assert!(d.loading && d.issue.is_none());
        assert_eq!(app.loading, 1, "연 관계 이슈 요청 하나");
        app.handle(Input::Esc, T0);
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.issue.as_ref().unwrap().identifier, "ENG-1", "원래 이슈로");
        assert_eq!(d.scroll, 2, "스크롤 위치 그대로");
        assert_eq!(app.mode, Mode::Detail);
        app.handle(Input::Esc, T0);
        assert_eq!(app.mode, Mode::Search, "처음 이슈에서 Esc는 목록으로");
        assert!(app.detail.is_none());
    }

    #[test]
    fn clicking_a_relation_line_opens_that_issue() {
        let mut app = with_relations();
        assert_eq!(
            app.handle(Input::ClickRelation(1), T0),
            vec![Effect::OpenDetail("b".into())]
        );
        assert_eq!(app.detail.as_ref().unwrap().id, "b");
        assert!(
            app.handle(Input::ClickRelation(99), T0).is_empty(),
            "없는 줄"
        );
    }

    #[test]
    fn response_for_a_stacked_detail_still_lands() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        let (issue, relations) = related();
        // 캐시가 먼저 오고 서버 응답은 아직이다
        app.apply(
            detail_msg("a", issue.clone(), Some(relations.clone()), false),
            T0,
        );
        app.handle(Input::ClickRelation(0), T0);
        assert_eq!(app.detail.as_ref().unwrap().id, "p");
        let mut fresh = issue;
        fresh.title = "새 제목".into();
        app.apply(detail_msg("a", fresh, Some(relations), true), T0);
        assert_eq!(app.detail.as_ref().unwrap().id, "p", "지금 상세는 그대로");
        app.handle(Input::Esc, T0);
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.issue.as_ref().unwrap().title, "새 제목");
        assert!(!d.loading, "돌아와도 불러오는 중에 멈춰 있지 않다");
        assert_eq!(app.loading, 1, "상위 이슈 요청만 남았다");
    }

    #[test]
    fn failure_clears_waiting_on_stacked_details_too() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        let (issue, relations) = related();
        app.apply(detail_msg("a", issue, Some(relations), false), T0);
        app.handle(Input::ClickRelation(0), T0);
        app.apply(Msg::Failed(ApiError::Offline("x".into())), T0);
        app.handle(Input::Esc, T0);
        assert!(!app.detail.as_ref().unwrap().loading);
    }

    #[test]
    fn relations_menu_title_says_what_we_know() {
        let title = |app: &mut App| {
            app.handle(Input::Act(Act::Relations), T0);
            let t = app.menu.as_ref().unwrap().title;
            app.handle(Input::Esc, T0);
            t
        };
        let mut app = started();
        app.handle(Input::Enter, T0);
        assert_eq!(title(&mut app), "관계 불러오는 중…");
        app.apply(Msg::Failed(ApiError::Offline("x".into())), T0);
        assert_eq!(title(&mut app), "관계를 불러오지 못했어요");
        app.apply(
            detail_msg(
                "a",
                issue("a", "ENG-1", "로그인 버그"),
                Some(IssueRelations::default()),
                true,
            ),
            T0,
        );
        assert_eq!(title(&mut app), "관계 없음");
        assert!(app.menu.is_none());
    }

    #[test]
    fn detail_menu_has_relations_after_links() {
        let mut app = with_relations();
        app.handle(Input::Menu, T0);
        let labels = menu_labels(&app);
        let links = labels.iter().position(|l| l == "링크·이미지 목록").unwrap();
        assert_eq!(labels[links + 1], "관계 이슈");
        app.handle(Input::Esc, T0);
        app.handle(Input::Esc, T0);
        app.handle(Input::Menu, T0);
        assert!(
            !menu_labels(&app).contains(&"관계 이슈".to_string()),
            "목록에서는 없다"
        );
    }

    #[test]
    fn auth_failure_drops_the_detail_stack() {
        let mut app = with_relations();
        app.handle(Input::ClickRelation(0), T0);
        app.apply(Msg::AuthFailed { env: false }, T0);
        assert!(app.detail.is_none() && app.detail_stack.is_empty());
    }
```

`src/tui/app.rs`에서 모두 바꾼다(기존 테스트 다섯 곳).

```text
                more: false,
                fresh:
```

```text
                more: false,
                relations: None,
                fresh:
```

`src/tui/view.rs`에서도 모두 바꾼다(테스트 두 곳).

```text
                more: false,
                fresh:
```

```text
                more: false,
                relations: None,
                fresh:
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib tui::app`
Expected: 컴파일 실패. `Act::Relations`, `Input::ClickRelation`, `Msg::Detail`의 `relations` 필드가 없다.

- [ ] **Step 3: 타입과 상태**

`src/tui/app.rs`에서 찾아 바꾼다.

```rust
use crate::linear::types::{Comment, Issue, Viewer};
use crate::markdown::{self, Theme};
```

```rust
use crate::linear::types::{Comment, Issue, IssueRelations, Viewer};
use crate::markdown::{self, Theme};
use crate::ui::relations::{self, RelRow};
```

찾아 바꾼다(`Act` 끝).

```rust
    Quit,
    OpenUrl(String),
}
```

```rust
    Quit,
    OpenUrl(String),
    /// 상세의 관계 메뉴
    Relations,
    /// 관계 이슈를 연다. 지금 상세는 쌓아 두고 Esc로 돌아온다
    OpenRelated(RelRow),
}
```

찾아 바꾼다(`Input` 끝).

```rust
    /// 마우스로 메뉴에 보이는 n번째 항목을 눌렀다
    ClickMenu(usize),
}
```

```rust
    /// 마우스로 메뉴에 보이는 n번째 항목을 눌렀다
    ClickMenu(usize),
    /// 상세 화면의 관계 줄(`ui::relations::rows` 번호)을 눌렀다
    ClickRelation(usize),
}
```

찾아 바꾼다(`Msg::Detail`).

```rust
    Detail {
        id: String,
        issue: Issue,
        comments: Vec<Comment>,
        more: bool,
        fresh: bool,
    },
```

```rust
    Detail {
        id: String,
        issue: Issue,
        comments: Vec<Comment>,
        more: bool,
        /// 관계. 캐시에 없으면 `None`
        relations: Option<IssueRelations>,
        fresh: bool,
    },
```

찾아 바꾼다(`Detail`).

```rust
    pub comments: Vec<Comment>,
    pub more_comments: bool,
    pub scroll: u16,
```

```rust
    pub comments: Vec<Comment>,
    pub more_comments: bool,
    /// 관계. 아직 모르면 `None`
    pub relations: Option<IssueRelations>,
    pub scroll: u16,
```

찾아 바꾼다(`App`).

```rust
    pub detail: Option<Detail>,
    pub menu: Option<Menu>,
    pub key_input: String,
```

```rust
    pub detail: Option<Detail>,
    /// 관계 이슈를 열 때 쌓아 둔 상세. Esc로 하나씩 돌아간다
    detail_stack: Vec<Detail>,
    pub menu: Option<Menu>,
    pub key_input: String,
```

찾아 바꾼다(`blank`).

```rust
            detail: None,
            menu: None,
            key_input: String::new(),
```

```rust
            detail: None,
            detail_stack: Vec::new(),
            menu: None,
            key_input: String::new(),
```

찾아 바꾼다(`start`).

```rust
                more_comments: false,
                scroll: 0,
                max_scroll: 0,
                loading: true,
                gone: false,
                back: Mode::Search,
```

```rust
                more_comments: false,
                relations: None,
                scroll: 0,
                max_scroll: 0,
                loading: true,
                gone: false,
                back: Mode::Search,
```

찾아 바꾼다(`act`의 `Act::Open`, 들여쓰기 24칸).

```rust
                        more_comments: false,
                        scroll: 0,
```

```rust
                        more_comments: false,
                        relations: None,
                        scroll: 0,
```

- [ ] **Step 4: 관계 메뉴·열기·돌아오기**

찾아 바꾼다(`handle_detail`).

```rust
            Input::Esc => return self.act(Act::Back, now),
            Input::Act(a) => return self.act(a, now),
            Input::Menu => self.open_menu(),
```

```rust
            Input::Esc => return self.act(Act::Back, now),
            Input::Act(a) => return self.act(a, now),
            Input::Menu => self.open_menu(),
            Input::ClickRelation(i) => {
                if let Some(row) = self.detail_rows().get(i).cloned() {
                    return self.act(Act::OpenRelated(row), now);
                }
            }
```

찾아 바꾼다(`open_menu`).

```rust
        if self.mode == Mode::Detail {
            items.push(("링크·이미지 목록".into(), Act::Links));
        }
```

```rust
        if self.mode == Mode::Detail {
            items.push(("링크·이미지 목록".into(), Act::Links));
            items.push(("관계 이슈".into(), Act::Relations));
        }
```

찾아 바꾼다.

```rust
    fn switch_tab(&mut self, tab: Tab, now: i64) -> Vec<Effect> {
```

```rust
    /// 지금 상세의 관계 줄. 화면의 관계 칸과 같은 순서라 메뉴·클릭 번호가 화면과 맞는다.
    fn detail_rows(&self) -> Vec<RelRow> {
        self.detail.as_ref().map_or_else(Vec::new, |d| {
            relations::rows(
                d.issue.as_ref().and_then(|i| i.parent.as_ref()),
                d.relations.as_ref(),
            )
        })
    }

    /// 관계 메뉴. 제목으로 관계를 아직 모르는지, 받지 못했는지, 없는지 알린다.
    fn open_relations(&mut self) {
        let Some(d) = self.detail.as_ref() else {
            return;
        };
        let (known, loading) = (d.relations.is_some(), d.loading);
        let rows = self.detail_rows();
        let title = match (known, loading) {
            (false, true) => "관계 불러오는 중…",
            (false, false) => "관계를 불러오지 못했어요",
            (true, _) if rows.is_empty() => "관계 없음",
            (true, _) => "관계",
        };
        self.menu = Some(Menu {
            title,
            items: rows
                .into_iter()
                .map(|r| (r.filter_text(), Act::OpenRelated(r)))
                .collect(),
            filter: String::new(),
            selected: 0,
        });
    }

    /// 관계 이슈를 연다. 지금 상세는 스크롤 위치째 쌓아 둔다.
    fn open_related(&mut self, row: RelRow) -> Vec<Effect> {
        let Some(current) = self.detail.take() else {
            return Vec::new();
        };
        let back = current.back;
        self.detail_stack.push(current);
        self.detail = Some(Detail {
            id: row.id.clone(),
            issue: None,
            comments: Vec::new(),
            more_comments: false,
            relations: None,
            scroll: 0,
            max_scroll: 0,
            loading: true,
            gone: false,
            back,
        });
        self.mode = Mode::Detail;
        self.loading += 1;
        vec![Effect::OpenDetail(row.id)]
    }

    /// 지금 상세와 쌓아 둔 상세.
    fn details_mut(&mut self) -> impl Iterator<Item = &mut Detail> {
        self.detail.iter_mut().chain(self.detail_stack.iter_mut())
    }

    fn switch_tab(&mut self, tab: Tab, now: i64) -> Vec<Effect> {
```

찾아 바꾼다(`act`의 `Links`·`Back`).

```rust
            Act::Links => {
                self.open_links();
                Vec::new()
            }
            Act::Back => {
                let back = self.detail.take().map_or(Mode::List, |d| d.back);
                self.mode = back;
                Vec::new()
            }
```

```rust
            Act::Links => {
                self.open_links();
                Vec::new()
            }
            Act::Relations => {
                self.open_relations();
                Vec::new()
            }
            Act::OpenRelated(row) => self.open_related(row),
            Act::Back => {
                // 관계 이슈에서 왔으면 앞 상세로 돌아간다 (스크롤 위치째)
                if let Some(prev) = self.detail_stack.pop() {
                    self.detail = Some(prev);
                    self.mode = Mode::Detail;
                    return Vec::new();
                }
                let back = self.detail.take().map_or(Mode::List, |d| d.back);
                self.mode = back;
                Vec::new()
            }
```

- [ ] **Step 5: 응답을 쌓인 상세에도 반영**

찾아 바꾼다(`apply`의 `Msg::Detail`).

```rust
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
```

```rust
            Msg::Detail {
                id,
                issue,
                comments,
                more,
                relations,
                fresh,
            } => {
                if fresh {
                    self.done_loading();
                    self.succeeded(now);
                    self.index.upsert(std::slice::from_ref(&issue));
                }
                // 쌓아 둔 상세도 같은 이슈면 바꾼다 (응답 전에 관계 이슈로 넘어간 경우)
                for d in self.details_mut() {
                    if d.id == id
                        || d.id == issue.id
                        || d.id.eq_ignore_ascii_case(&issue.identifier)
                    {
                        d.id = issue.id.clone();
                        d.issue = Some(issue.clone());
                        d.comments = comments.clone();
                        d.more_comments = more;
                        if let Some(r) = &relations {
                            d.relations = Some(r.clone());
                        }
                        if fresh {
                            d.loading = false;
                        }
                    }
                }
            }
```

찾아 바꾼다(`Msg::DetailGone`).

```rust
                let same = |i: &Issue| i.id == id || i.identifier.eq_ignore_ascii_case(&id);
                if let Some(d) = self.detail.as_mut()
                    && (d.id.eq_ignore_ascii_case(&id) || d.issue.as_ref().is_some_and(same))
                {
                    d.gone = true;
                    d.loading = false;
                }
```

```rust
                let same = |i: &Issue| i.id == id || i.identifier.eq_ignore_ascii_case(&id);
                for d in self.details_mut() {
                    if d.id.eq_ignore_ascii_case(&id) || d.issue.as_ref().is_some_and(same) {
                        d.gone = true;
                        d.loading = false;
                    }
                }
```

찾아 바꾼다(`Msg::AuthFailed`).

```rust
                self.menu = None;
                self.detail = None;
                self.loading = 0;
```

```rust
                self.menu = None;
                self.detail = None;
                self.detail_stack.clear();
                self.loading = 0;
```

찾아 바꾼다(`Msg::Failed`).

```rust
                self.done_loading();
                if let Some(d) = self.detail.as_mut() {
                    d.loading = false;
                }
```

```rust
                self.done_loading();
                // 어느 요청의 실패인지 모르니 쌓아 둔 상세의 대기도 푼다
                for d in self.details_mut() {
                    d.loading = false;
                }
```

- [ ] **Step 6: 런타임이 컴파일되게 맞춘다**

`src/tui/runtime.rs`에서 찾아 바꾼다(서버 응답).

```rust
                    comments: d.comments,
                    more: d.more_comments,
                    fresh: true,
```

```rust
                    comments: d.comments,
                    more: d.more_comments,
                    relations: Some(d.relations),
                    fresh: true,
```

찾아 바꾼다(캐시, `open_detail`). 캐시에서 읽는 것은 Task 6에서 한다.

```rust
                comments,
                more: false,
                fresh: false,
            });
```

```rust
                comments,
                more: false,
                relations: None,
                fresh: false,
            });
```

찾아 바꾼다(테스트 `detail_sends_cache_then_fresh_and_marks_viewed`의 패턴).

```rust
            Msg::Detail {
                id,
                issue,
                comments,
                more,
                fresh,
            } => {
```

```rust
            Msg::Detail {
                id,
                issue,
                comments,
                more,
                fresh,
                ..
            } => {
```

- [ ] **Step 7: 통과 확인**

Run: `cargo test --lib tui::app && cargo test`
Expected: 모두 통과(278개 통과, 1개 무시).

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: 경고 없음.

- [ ] **Step 8: 커밋**

```bash
git add src/tui/app.rs src/tui/runtime.rs src/tui/view.rs
git commit -m "feat(tui): 관계 동작 — 관계 메뉴, 관계 이슈 열기, 상세 스택과 Esc로 돌아오기"
```

---

### Task 6: 런타임 — 관계 캐시 읽기·쓰기

**Files:**
- Modify: `src/tui/runtime.rs` (`open_detail`, `absorb`의 상세 응답, 테스트)

**Interfaces:**
- Consumes: `Store::set_relations`, `Store::get_relations` (Task 3), `Msg::Detail.relations` (Task 5)

- [ ] **Step 1: 테스트를 쓴다**

`src/tui/runtime.rs`에서 찾아 바꾼다.

```rust
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;
    use mockito::Matcher;
```

```rust
mod tests {
    use super::*;
    use crate::linear::types::{IssueRelations, RelatedIssue};
    use crate::test_support::IssueBuilder;
    use mockito::Matcher;
```

`src/tui/runtime.rs` 테스트 모듈 끝에 넣는다.

```rust
    #[test]
    fn detail_sends_cached_relations_then_saves_fresh_ones() {
        let mut fx = fixture(Some(KeySource::File), true, Origin::default());
        fx.rt
            .store
            .upsert_issues(&[issue("a", "ENG-1", "제목")], T0)
            .unwrap();
        let blocker = issue("b1", "ENG-5", "막는 이슈");
        let cached = IssueRelations {
            blocked_by: vec![RelatedIssue {
                id: blocker.id,
                identifier: blocker.identifier,
                title: blocker.title,
                state: blocker.state,
            }],
            ..IssueRelations::default()
        };
        fx.rt.store.set_relations("a", &cached, T0).unwrap();
        let mut detail = IssueBuilder::new("a", "ENG-1", "제목").json();
        detail["comments"] =
            json!({ "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } });
        detail["inverseRelations"] = json!({ "nodes": [
            { "type": "blocks", "issue": IssueBuilder::new("b2", "ENG-6", "새로 막는 이슈").json() }
        ] });
        mock(&mut fx.server, "Detail", json!({ "issue": detail }));
        let first = fx.rt.execute(Effect::OpenDetail("ENG-1".into()), T0);
        assert!(
            matches!(&first[0], Msg::Detail { fresh: false, relations: Some(r), .. } if *r == cached),
            "{first:?}"
        );
        let fresh = settle(&mut fx.rt, T0);
        let Msg::Detail {
            relations: Some(r),
            fresh: true,
            ..
        } = &fresh[0]
        else {
            panic!("{fresh:?}");
        };
        assert_eq!(r.blocked_by[0].identifier, "ENG-6");
        assert_eq!(fx.rt.store.get_relations("a").unwrap().unwrap().0, r.clone());
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib tui::runtime::tests::detail_sends_cached_relations_then_saves_fresh_ones`
Expected: FAIL. 첫 메시지의 `relations`가 `None`이다.

- [ ] **Step 3: 구현**

찾아 바꾼다(`open_detail`).

```rust
            self.note(self.store.mark_viewed(&issue.id, now));
            msgs.push(Msg::Detail {
                id: id.clone(),
                issue,
                comments,
                more: false,
                relations: None,
                fresh: false,
            });
```

```rust
            let relations = self
                .note(self.store.get_relations(&issue.id))
                .flatten()
                .map(|(r, _)| r);
            self.note(self.store.mark_viewed(&issue.id, now));
            msgs.push(Msg::Detail {
                id: id.clone(),
                issue,
                comments,
                more: false,
                relations,
                fresh: false,
            });
```

찾아 바꾼다(`absorb`의 상세 응답).

```rust
                self.note(self.store.set_comments(&d.issue.id, &d.comments, now));
                self.note(self.store.mark_viewed(&d.issue.id, now));
```

```rust
                self.note(self.store.set_comments(&d.issue.id, &d.comments, now));
                self.note(self.store.set_relations(&d.issue.id, &d.relations, now));
                self.note(self.store.mark_viewed(&d.issue.id, now));
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test --lib tui::runtime && cargo test`
Expected: 모두 통과(279개 통과, 1개 무시).

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: 경고 없음.

- [ ] **Step 5: 커밋**

```bash
git add src/tui/runtime.rs
git commit -m "feat(tui): 관계를 캐시에서 먼저 보이고 서버 응답은 저장하기"
```

---

### Task 7: 화면과 키 — 관계 칸, 클릭, 관계 메뉴 색, `t`

**Files:**
- Modify: `src/tui/view.rs` (`Target::Relation`, `issue_header`, `draw_detail`, `detail_lines`, `hints`, `draw_menu`, 테스트)
- Modify: `src/tui/keys.rs` (`t`, `Target::Relation` → `Input::ClickRelation`, 테스트)

**Interfaces:**
- Consumes: `ui::relations::{lines, row_line}` (Task 4), `Act::Relations`, `Act::OpenRelated`, `Input::ClickRelation` (Task 5)
- Produces:
  - `Target::Relation(usize)`
  - `pub fn issue_header(issue: &Issue, width: u16, show_parent: bool) -> Vec<Line<'static>>`
  - `fn detail_lines(d: &Detail, width: u16, keys: &[String]) -> (Vec<Line<'static>>, Vec<(usize, usize)>)`. 둘째 값은 (줄 번호, `rows` 번호)다.

- [ ] **Step 1: 테스트를 쓴다**

`src/tui/view.rs`에서 찾아 바꾼다. `Act`는 Step 3에서 본문 모듈이 import하므로 `use super::*;`로 들어온다.

```rust
    use super::*;
    use crate::linear::types::Viewer;
    use crate::test_support::IssueBuilder;
    use crate::tui::app::{Act, Input, Msg};
```

```rust
    use super::*;
    use crate::linear::types::{IssueRelations, RelatedIssue, Viewer};
    use crate::test_support::IssueBuilder;
    use crate::tui::app::{Input, Msg};
```

`src/tui/view.rs` 테스트 모듈 끝에 넣는다.

```rust
    fn rel(id: &str, identifier: &str, title: &str, state_type: &str) -> RelatedIssue {
        let i = IssueBuilder::new(id, identifier, title)
            .state(state_type, state_type)
            .build();
        RelatedIssue {
            id: i.id,
            identifier: i.identifier,
            title: i.title,
            state: i.state,
        }
    }

    /// ENG-1(상위 ENG-10): 막힘 ENG-20, 하위 ENG-30(진행)·ENG-31(완료). 본문은 `body`.
    fn related_detail(body: &str) -> App {
        let mut a = app();
        a.handle(Input::Enter, T0);
        let issue = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .parent("p", "ENG-10", "인증 개편", "started")
            .description(body)
            .build();
        let relations = IssueRelations {
            blocked_by: vec![rel("b", "ENG-20", "API 스키마", "unstarted")],
            children: vec![
                rel("c1", "ENG-30", "토큰 갱신", "started"),
                rel("c2", "ENG-31", "세션 만료", "completed"),
            ],
            ..IssueRelations::default()
        };
        a.apply(
            Msg::Detail {
                id: "a".into(),
                issue,
                comments: Vec::new(),
                more: false,
                relations: Some(relations),
                fresh: true,
            },
            T0,
        );
        a
    }

    #[test]
    fn detail_shows_relations_between_header_and_body() {
        let a = related_detail("본문 첫 줄");
        let (rows, _, _) = screen(&a, 80, 20);
        let url = rows
            .iter()
            .position(|r| r.contains("https://linear.app/acme/issue/ENG-1"))
            .unwrap();
        assert_eq!(rows[url + 1], "", "URL 다음 빈 줄");
        assert_eq!(rows[url + 2], " 상위    ◐ ENG-10    인증 개편");
        assert_eq!(rows[url + 3], " 막힘    ○ ENG-20    API 스키마");
        assert_eq!(rows[url + 4], " 하위    2개 중 1개 남음");
        assert_eq!(rows[url + 5], "         ◐ ENG-30    토큰 갱신");
        assert_eq!(rows[url + 6], "         ● ENG-31    세션 만료");
        assert_eq!(rows[url + 7], "", "관계 칸 다음 빈 줄");
        assert_eq!(rows[url + 8], " 본문 첫 줄");
        assert!(
            !rows[..url].iter().any(|r| r.contains("상위 ENG-10")),
            "머리에서는 상위를 뺀다"
        );
    }

    #[test]
    fn preview_header_keeps_the_parent() {
        let issue = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .parent("p", "ENG-10", "인증 개편", "started")
            .build();
        assert!(markdown::to_plain(&issue_header(&issue, 80, true)).contains("상위 ENG-10"));
        assert!(!markdown::to_plain(&issue_header(&issue, 80, false)).contains("상위"));
    }

    #[test]
    fn relation_click_targets_follow_the_scroll() {
        let body: String = (1..=40).map(|i| format!("{i}번째 문단\n\n")).collect();
        let mut a = related_detail(&body);
        let (rows, drawn, _) = screen(&a, 80, 12);
        let y = rows.iter().position(|r| r.contains("ENG-20")).unwrap() as u16;
        assert_eq!(
            drawn.target_at(5, y),
            Some(Target::Relation(1)),
            "막힘 줄은 관계 1번"
        );
        let summary = rows
            .iter()
            .position(|r| r.contains("2개 중 1개 남음"))
            .unwrap() as u16;
        assert_eq!(drawn.target_at(5, summary), None, "요약 줄은 누를 수 없다");
        a.set_detail_max_scroll(drawn.detail_max_scroll.unwrap());
        a.handle(Input::Down, T0);
        let (rows, drawn, _) = screen(&a, 80, 12);
        let y2 = rows.iter().position(|r| r.contains("ENG-20")).unwrap() as u16;
        assert_eq!(y2 + 1, y, "한 줄 올라갔다");
        assert_eq!(drawn.target_at(5, y2), Some(Target::Relation(1)));
        for _ in 0..20 {
            a.handle(Input::Down, T0);
        }
        let (rows, drawn, _) = screen(&a, 80, 12);
        assert!(!rows.iter().any(|r| r.contains("ENG-20")));
        assert!(
            !drawn
                .hits
                .iter()
                .any(|h| matches!(h.target, Target::Relation(_))),
            "가려진 관계 줄은 누를 수 없다"
        );
    }

    #[test]
    fn relation_menu_draws_colored_rows() {
        let mut a = related_detail("본문");
        a.handle(Input::Act(Act::Relations), T0);
        let (rows, drawn, term) = screen(&a, 100, 20);
        let item = drawn
            .hits
            .iter()
            .find(|h| h.target == Target::MenuItem(1))
            .expect("막힘 항목");
        let y = item.area.y;
        assert!(rows[usize::from(y)].contains("막힘    ○ ENG-20    API 스키마"), "{rows:?}");
        let buf = term.backend().buffer();
        let x = (item.area.x..item.area.right())
            .find(|&x| buf[(x, y)].symbol() == "막")
            .expect("막힘 글자");
        assert_eq!(buf[(x, y)].fg, Color::Red, "안 끝난 막는 이슈");
    }
```

찾아 바꾼다(`tiny_screens_do_not_panic`).

```rust
        let onboarding = App::onboarding(false);
        for app in [&app(), &detail, &menu, &onboarding] {
```

```rust
        let onboarding = App::onboarding(false);
        let related = related_detail("본문");
        let mut related_menu = related_detail("본문");
        related_menu.handle(Input::Act(Act::Relations), T0);
        for app in [
            &app(),
            &detail,
            &menu,
            &onboarding,
            &related,
            &related_menu,
        ] {
```

찾아 바꾼다(`detail_shows_body_comments_and_max_scroll`).

```rust
        assert!(rows[11].ends_with("Esc 뒤로  q 닫기"), "{}", rows[11]);
```

```rust
        assert!(rows[11].ends_with("Esc 뒤로  q 닫기"), "{}", rows[11]);
        assert!(rows[11].contains("t 관계"), "{}", rows[11]);
```

`src/tui/view.rs` 테스트에서 아래 네 가지를 모두 바꾼다.

| 찾기 | 바꾸기 |
|---|---|
| `issue_header(&issue, 80)` | `issue_header(&issue, 80, true)` |
| `issue_header(&open, 80)` | `issue_header(&open, 80, true)` |
| `issue_header(&merged, 80)` | `issue_header(&merged, 80, true)` |
| `detail_lines(a.detail.as_ref().unwrap(), 80, &[])` | `detail_lines(a.detail.as_ref().unwrap(), 80, &[]).0` |

`src/tui/keys.rs` 테스트 모듈 끝에 넣는다.

```rust
    #[test]
    fn detail_mode_has_relations_key_even_in_korean_input() {
        for c in ['t', 'ㅅ'] {
            assert_eq!(
                translate(Mode::Detail, false, key(KeyCode::Char(c))),
                Some(Input::Act(Act::Relations)),
                "{c}"
            );
        }
        assert_eq!(translate(Mode::List, false, key(KeyCode::Char('t'))), None);
        assert_eq!(
            translate(Mode::Search, false, key(KeyCode::Char('t'))),
            Some(Input::Char('t'))
        );
        assert_eq!(
            translate(Mode::Detail, true, key(KeyCode::Char('t'))),
            Some(Input::Char('t')),
            "메뉴에서는 거르기 글자"
        );
    }

    #[test]
    fn clicking_a_relation_line_becomes_click_relation() {
        use crate::tui::view::{Drawn, Hit, Target};
        use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;
        let drawn = Drawn {
            hits: vec![Hit {
                area: Rect::new(1, 5, 60, 1),
                target: Target::Relation(2),
            }],
            ..Drawn::default()
        };
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(mouse(&drawn, ev), Some(Input::ClickRelation(2)));
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib tui::`
Expected: 컴파일 실패. `Target::Relation`이 없고, `issue_header`의 인자 수가 다르다.

- [ ] **Step 3: 화면 구현**

`src/tui/view.rs`에서 찾아 바꾼다.

```rust
use super::app::{App, Detail, Mode, Problem, Row, Tab};
use crate::linear::types::Issue;
use crate::markdown::{self, Theme, sanitize};
use crate::ui::row::issue_row;
```

```rust
use super::app::{Act, App, Detail, Mode, Problem, Row, Tab};
use crate::linear::types::Issue;
use crate::markdown::{self, Theme, sanitize};
use crate::ui::relations;
use crate::ui::row::issue_row;
```

찾아 바꾼다.

```rust
    /// 메뉴에 보이는 n번째 항목
    MenuItem(usize),
}
```

```rust
    /// 메뉴에 보이는 n번째 항목
    MenuItem(usize),
    /// 상세 화면의 관계 줄 (`ui::relations::rows` 번호)
    Relation(usize),
}
```

찾아 바꾼다(`draw`).

```rust
    if app.mode == Mode::Detail {
        drawn.detail_max_scroll = Some(draw_detail(f, app, search.union(body)));
    } else {
```

```rust
    if app.mode == Mode::Detail {
        let max = draw_detail(f, app, search.union(body), &mut drawn);
        drawn.detail_max_scroll = Some(max);
    } else {
```

찾아 바꾼다.

```rust
/// 이슈 머리: 식별자·제목 / 상태·우선순위·담당자 / 라벨·프로젝트·사이클·상위·예상·마감.
pub fn issue_header(issue: &Issue, width: u16) -> Vec<Line<'static>> {
```

```rust
/// 이슈 머리: 식별자·제목 / 상태·우선순위·담당자 / 라벨·프로젝트·사이클·상위·예상·마감.
/// 상세 화면은 관계 칸에 상위를 보여서 `show_parent`를 끈다.
pub fn issue_header(issue: &Issue, width: u16, show_parent: bool) -> Vec<Line<'static>> {
```

찾아 바꾼다.

```rust
    if let Some(p) = &issue.parent {
        info.push(format!("상위 {}", sanitize(&p.identifier)));
    }
```

```rust
    if show_parent && let Some(p) = &issue.parent {
        info.push(format!("상위 {}", sanitize(&p.identifier)));
    }
```

찾아 바꾼다(`draw_preview`).

```rust
    let mut lines = issue_header(issue, inner.width);
```

```rust
    let mut lines = issue_header(issue, inner.width, true);
```

`draw_detail` 함수 전체를 찾아 바꾼다.

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
```

```rust
/// 상세 화면을 그리고 최대 스크롤을 돌려준다. 보이는 관계 줄은 누를 수 있게 `drawn`에 남긴다.
fn draw_detail(f: &mut Frame, app: &App, area: Rect, drawn: &mut Drawn) -> u16 {
    let Some(d) = &app.detail else {
        return 0;
    };
    let area = Rect {
        x: area.x + 1,
        width: area.width.saturating_sub(2),
        ..area
    };
    let (lines, relation_lines) = detail_lines(d, area.width, &app.team_keys());
    let total = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let max = total.saturating_sub(area.height);
    let scroll = d.scroll.min(max);
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
    // 줄바꿈 없이 한 줄씩 그리므로 화면 줄 = 줄 번호 - 스크롤
    for (line, row) in relation_lines {
        let Some(y) = u16::try_from(line).ok().and_then(|l| l.checked_sub(scroll)) else {
            continue;
        };
        if y < area.height {
            drawn.hits.push(Hit {
                area: Rect::new(area.x, area.y + y, area.width, 1),
                target: Target::Relation(row),
            });
        }
    }
    max
}
```

찾아 바꾼다(`detail_lines` 머리).

```rust
/// 상세 화면의 줄. 링크·이미지 번호는 본문에서 코멘트로 이어 매긴다 (`u` 목록과 같은 번호).
fn detail_lines(d: &Detail, width: u16, keys: &[String]) -> Vec<Line<'static>> {
    let theme = Theme::default();
    let mut lines: Vec<Line<'static>> = Vec::new();
```

```rust
/// 상세 화면의 줄과, 관계 줄의 (줄 번호, `ui::relations::rows` 번호).
/// 링크·이미지 번호는 본문에서 코멘트로 이어 매긴다 (`u` 목록과 같은 번호).
fn detail_lines(
    d: &Detail,
    width: u16,
    keys: &[String],
) -> (Vec<Line<'static>>, Vec<(usize, usize)>) {
    let theme = Theme::default();
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut relation_lines = Vec::new();
```

찾아 바꾼다(머리 다음, 본문 앞).

```rust
            lines.extend(issue_header(issue, width));
            lines.push(Line::from(Span::styled(sanitize(&issue.url), DIM)));
            if d.gone {
                lines.push(Line::from(Span::styled(
                    "보관되었거나 삭제된 이슈예요",
                    WARN,
                )));
            }
            lines.push(Line::default());
```

```rust
            lines.extend(issue_header(issue, width, false));
            lines.push(Line::from(Span::styled(sanitize(&issue.url), DIM)));
            if d.gone {
                lines.push(Line::from(Span::styled(
                    "보관되었거나 삭제된 이슈예요",
                    WARN,
                )));
            }
            lines.push(Line::default());
            let (rel, rows) =
                relations::lines(issue.parent.as_ref(), d.relations.as_ref(), width);
            if !rel.is_empty() {
                let start = lines.len();
                relation_lines.extend(
                    rows.into_iter()
                        .enumerate()
                        .filter_map(|(k, row)| row.map(|r| (start + k, r))),
                );
                lines.extend(rel);
                lines.push(Line::default());
            }
```

찾아 바꾼다(`detail_lines` 끝).

```rust
            } else if d.loading {
                lines.push(Line::default());
                lines.push(Line::from(Span::styled("코멘트 불러오는 중…", DIM)));
            }
        }
    }
    lines
}
```

```rust
            } else if d.loading {
                lines.push(Line::default());
                lines.push(Line::from(Span::styled("코멘트 불러오는 중…", DIM)));
            }
        }
    }
    (lines, relation_lines)
}
```

찾아 바꾼다(`hints`).

```rust
        Mode::Detail => " j/k 스크롤  u 링크  y URL 복사  Y PR 링크  ^K 메뉴  Esc 뒤로  q 닫기",
```

```rust
        Mode::Detail => {
            " j/k 스크롤  t 관계  u 링크  y URL 복사  Y PR 링크  ^K 메뉴  Esc 뒤로  q 닫기"
        }
```

찾아 바꾼다(`draw_menu`).

```rust
    let label_width = usize::from(list.width.saturating_sub(2));
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|(label, _)| ListItem::new(truncate(&sanitize(label), label_width)))
        .collect();
```

```rust
    let label_width = list.width.saturating_sub(2);
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|(label, act)| match act {
            // 관계 메뉴는 상세의 관계 칸과 같은 색으로 그린다
            Act::OpenRelated(row) => ListItem::new(relations::row_line(row, true, label_width)),
            _ => ListItem::new(truncate(&sanitize(label), usize::from(label_width))),
        })
        .collect();
```

- [ ] **Step 4: 키 구현**

`src/tui/keys.rs`에서 찾아 바꾼다.

```rust
        (Mode::Detail, 'u') => act(Act::Links),
        _ => None,
```

```rust
        (Mode::Detail, 'u') => act(Act::Links),
        (Mode::Detail, 't') => act(Act::Relations),
        _ => None,
```

찾아 바꾼다.

```rust
                    Target::MenuItem(i) => Input::ClickMenu(i),
                })
```

```rust
                    Target::MenuItem(i) => Input::ClickMenu(i),
                    Target::Relation(i) => Input::ClickRelation(i),
                })
```

- [ ] **Step 5: 통과 확인**

Run: `cargo test --lib tui:: && cargo test`
Expected: 모두 통과(285개 통과, 1개 무시).

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: 경고 없음. `cargo fmt`가 `hints`의 줄을 한 줄로 합치면 그대로 받아들인다.

- [ ] **Step 6: 커밋**

```bash
git add src/tui/view.rs src/tui/keys.rs
git commit -m "feat(tui): 상세에 관계 칸 — t 메뉴와 줄 클릭으로 열기, 색 입힌 관계 메뉴"
```

---

### Task 8: CLI `show`의 관계 칸

**Files:**
- Modify: `src/cli.rs` (`show`, `format_detail`, 테스트)

**Interfaces:**
- Consumes: `ui::relations::lines` (Task 4), `Store::{set_relations, get_relations}` (Task 3), `IssueDetail.relations` (Task 2)
- Produces: `fn format_detail(ctx: &Ctx, issue: &Issue, comments: &[Comment], more: bool, relations: Option<&IssueRelations>, banner: Option<&str>, team_keys: &[String]) -> String`

- [ ] **Step 1: 테스트를 쓴다**

`src/cli.rs`에서 찾아 바꾼다.

```rust
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;
    use mockito::Matcher;
```

```rust
mod tests {
    use super::*;
    use crate::linear::types::RelatedIssue;
    use crate::test_support::IssueBuilder;
    use mockito::Matcher;
```

`src/cli.rs` 테스트 모듈 끝에 넣는다.

```rust
    fn rel_json(id: &str, identifier: &str, title: &str, state_type: &str) -> Value {
        IssueBuilder::new(id, identifier, title)
            .state(state_type, state_type)
            .json()
    }

    #[test]
    fn show_lists_relations_after_the_url_and_saves_them() {
        let mut server = mockito::Server::new();
        let mut issue = IssueBuilder::new("i1", "ENG-1", "로그인 버그")
            .parent("p", "ENG-10", "인증 개편", "started")
            .description("본문")
            .json();
        issue["comments"] =
            json!({ "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } });
        issue["children"] = json!({
            "nodes": [ rel_json("c1", "ENG-30", "토큰 갱신", "completed") ],
            "pageInfo": { "hasNextPage": false }
        });
        issue["inverseRelations"] = json!({ "nodes": [
            { "type": "blocks", "issue": rel_json("b1", "ENG-20", "API 스키마", "unstarted") }
        ] });
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Detail".into()))
            .with_body(json!({ "data": { "issue": issue } }).to_string())
            .create();
        let (_d, mut ctx) = test_ctx(url(&server));
        let out = show(&ctx, "ENG-1").unwrap();
        assert!(
            out.contains(
                "https://linear.app/acme/issue/ENG-1\n\n\
                 상위    ◐ ENG-10    인증 개편\n\
                 막힘    ○ ENG-20    API 스키마\n\
                 하위    1개 모두 끝남\n        \
                 ● ENG-30    토큰 갱신\n\n본문"
            ),
            "{out}"
        );
        assert!(!out.contains("상위 ENG-10"), "머리 줄에서는 뺀다\n{out}");
        assert!(!out.contains('\u{1b}'), "색을 끄면 이스케이프가 없다");
        assert!(ctx.store.get_relations("i1").unwrap().is_some());
        ctx.color = true;
        let colored = show(&ctx, "ENG-1").unwrap();
        assert!(colored.contains("m막힘"), "색을 켜면 칸 이름에 색\n{colored:?}");
    }

    #[test]
    fn show_offline_uses_cached_relations() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        trust_cache(&ctx);
        ctx.store
            .upsert_issues(&[IssueBuilder::new("i1", "ENG-1", "저장된 상세").build()], NOW)
            .unwrap();
        let blocker = IssueBuilder::new("b1", "ENG-20", "API 스키마").build();
        let relations = IssueRelations {
            blocked_by: vec![RelatedIssue {
                id: blocker.id,
                identifier: blocker.identifier,
                title: blocker.title,
                state: blocker.state,
            }],
            ..IssueRelations::default()
        };
        ctx.store.set_relations("i1", &relations, NOW).unwrap();
        let out = show(&ctx, "ENG-1").unwrap();
        assert!(out.starts_with("(오프라인: 저장된 내용"), "{out}");
        assert!(out.contains("막힘    ○ ENG-20    API 스키마"), "{out}");
    }
```

문자열 끝의 `\`는 줄바꿈과 다음 줄 앞 공백을 건너뛴다. `\n        \`는 `\n` 다음 여덟 칸 공백을 남기고, 다음 줄 공백은 건너뛴다. 그래서 기대 문자열의 하위 이슈 줄은 `        ● ENG-30    토큰 갱신`이다.

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib cli::tests::show_`
Expected: 새 두 테스트가 FAIL한다. 출력에 관계 칸이 없다.

- [ ] **Step 3: 구현**

`src/cli.rs`에서 찾아 바꾼다.

```rust
use crate::linear::types::{Comment, Issue, TeamRef, Viewer};
```

```rust
use crate::linear::types::{Comment, Issue, IssueRelations, TeamRef, Viewer};
```

찾아 바꾼다.

```rust
use crate::store::{Store, remove_db_files};
use crate::ui::row::issue_row;
```

```rust
use crate::store::{Store, remove_db_files};
use crate::ui::relations;
use crate::ui::row::issue_row;
```

찾아 바꾼다(`show`의 온라인 경로).

```rust
            ctx.store
                .set_comments(&d.issue.id, &d.comments, ctx.now_ms)?;
            ctx.store.mark_viewed(&d.issue.id, ctx.now_ms)?;
            Ok(format_detail(
                ctx,
                &d.issue,
                &d.comments,
                d.more_comments,
                None,
                &keys,
            ))
```

```rust
            ctx.store
                .set_comments(&d.issue.id, &d.comments, ctx.now_ms)?;
            ctx.store
                .set_relations(&d.issue.id, &d.relations, ctx.now_ms)?;
            ctx.store.mark_viewed(&d.issue.id, ctx.now_ms)?;
            Ok(format_detail(
                ctx,
                &d.issue,
                &d.comments,
                d.more_comments,
                Some(&d.relations),
                None,
                &keys,
            ))
```

찾아 바꾼다(`show`의 오프라인 경로).

```rust
                .map(|(c, _)| c)
                .unwrap_or_default();
            Ok(format_detail(
                ctx,
                &issue,
                &comments,
                false,
                Some(&format!("오프라인: 저장된 내용 · {msg}")),
                &keys,
            ))
```

```rust
                .map(|(c, _)| c)
                .unwrap_or_default();
            let relations = ctx.store.get_relations(&issue.id)?.map(|(r, _)| r);
            Ok(format_detail(
                ctx,
                &issue,
                &comments,
                false,
                relations.as_ref(),
                Some(&format!("오프라인: 저장된 내용 · {msg}")),
                &keys,
            ))
```

찾아 바꾼다(`format_detail` 머리).

```rust
fn format_detail(
    ctx: &Ctx,
    issue: &Issue,
    comments: &[Comment],
    more: bool,
    banner: Option<&str>,
    team_keys: &[String],
) -> String {
```

```rust
fn format_detail(
    ctx: &Ctx,
    issue: &Issue,
    comments: &[Comment],
    more: bool,
    relations: Option<&IssueRelations>,
    banner: Option<&str>,
    team_keys: &[String],
) -> String {
```

찾아서 지운다. 머리 줄의 상위는 관계 칸으로 옮긴다.

```rust
    if let Some(p) = &issue.parent {
        extra.push(format!("상위 {}", p.identifier));
    }
```

찾아 바꾼다(URL 다음에 관계 칸).

```rust
    // 여기까지는 Linear 값이 그대로 들어간 평문이라 제어 문자를 지운다
    let mut out: Vec<String> = out.iter().map(|l| markdown::sanitize(l)).collect();
    out.push(String::new());
```

```rust
    // 여기까지는 Linear 값이 그대로 들어간 평문이라 제어 문자를 지운다
    let mut out: Vec<String> = out.iter().map(|l| markdown::sanitize(l)).collect();
    // 관계 칸은 TUI 상세와 같은 줄을 쓴다 (제어 문자는 줄을 만들 때 지운다)
    let (rel, _) = relations::lines(issue.parent.as_ref(), relations, ctx.width);
    if !rel.is_empty() {
        out.push(String::new());
        out.push(if ctx.color {
            markdown::to_ansi(&rel)
        } else {
            markdown::to_plain(&rel)
        });
    }
    out.push(String::new());
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test --lib cli:: && cargo test`
Expected: 모두 통과(287개 통과, 1개 무시). 기존 `show_renders_detail_and_marks_viewed`도 그대로 통과한다(관계가 없으면 칸이 없다).

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: 경고 없음.

- [ ] **Step 5: 커밋**

```bash
git add src/cli.rs
git commit -m "feat(cli): show에 관계 칸 — 오프라인이면 캐시된 관계"
```

---

### Task 9: 문서와 버전 0.1.2

**Files:**
- Modify: `Cargo.toml`, `herdr-plugin.toml`(version 0.1.2), `Cargo.lock`(빌드로 갱신)
- Modify: `docs/superpowers/specs/2026-10-06-herdr-linear-design.md` (4.6, 4.7, 6.2, 7.2)
- Modify: `docs/superpowers/specs/2026-10-07-issue-relations-design.md` (상태)
- Modify: `README.md` (기능, 키: 영어·한국어)
- Modify: `ROADMAP.md` (v0.1.2: 영어·한국어)

- [ ] **Step 1: 버전**

`Cargo.toml`과 `herdr-plugin.toml`의 `version = "0.1.1"`을 `"0.1.2"`로 바꾼다. 그다음 `cargo build`를 돌려 `Cargo.lock`을 갱신한다.

- [ ] **Step 2: 바탕 스펙**

`docs/superpowers/specs/2026-10-06-herdr-linear-design.md`에서 아래를 고친다.

4.6 머리 줄을 찾아 바꾼다.

```markdown
- **머리**: 식별자, 제목, 상태, 우선순위, 담당자, 열린 PR(번호·저장소), 라벨, 프로젝트, 사이클, 부모 이슈, 예상치, 마감일
```

```markdown
- **머리**: 식별자, 제목, 상태, 우선순위, 담당자, 열린 PR(번호·저장소), 라벨, 프로젝트, 사이클, 예상치, 마감일. 목록 옆 미리보기에서는 부모 이슈도 머리에 보인다.
- **관계**: 머리 아래에 상위·막힘·막는 중·관련·하위를 상태 색과 함께 보여 준다. `t` 메뉴나 관계 줄 클릭으로 열고, Esc로 한 단계씩 돌아온다. 자세한 규칙은 `2026-10-07-issue-relations-design.md`를 따른다.
```

4.7 상세 화면 줄을 찾아 바꾼다.

```markdown
| 상세 화면 | j/k = 스크롤, Ctrl+D/Ctrl+U = 반 페이지, g/G = 처음/끝, `u` = 본문 링크·이미지 목록, `r` = 새로고침, 목록 모드와 같은 동작 키, Esc = 뒤로, `q` = 닫기 |
```

```markdown
| 상세 화면 | j/k = 스크롤, Ctrl+D/Ctrl+U = 반 페이지, g/G = 처음/끝, `t` = 관계 메뉴, `u` = 본문 링크·이미지 목록, `r` = 새로고침, 목록 모드와 같은 동작 키, Esc = 뒤로(관계 이슈에서 왔으면 앞 이슈로), `q` = 닫기 |
```

4.7 마우스 줄을 찾아 바꾼다.

```markdown
| 마우스 | 휠 = 목록 이동·상세 스크롤·메뉴 이동, 줄 클릭 = 선택(선택된 줄을 다시 클릭하면 상세), 탭 이름 클릭 = 보기 전환, 메뉴 항목 클릭 = 실행 |
```

```markdown
| 마우스 | 휠 = 목록 이동·상세 스크롤·메뉴 이동, 줄 클릭 = 선택(선택된 줄을 다시 클릭하면 상세), 탭 이름 클릭 = 보기 전환, 메뉴 항목 클릭 = 실행, 상세의 관계 줄 클릭 = 그 이슈 열기 |
```

6.2 첫 주석 줄을 찾아 바꾼다.

```sql
-- PRAGMA user_version = 1. 값이 다르면 파일을 지우고 새로 만든다.
```

```sql
-- PRAGMA user_version = 2. 1이면 relations 테이블만 더하고, 그 밖의 다른 값이면 파일을 지우고 새로 만든다.
```

6.2 SQL 블록 끝을 찾아 바꾼다(`branch_map` 정의 뒤에 `relations`를 더한다).

````markdown
  PRIMARY KEY (repo, branch)
);
```
````

````markdown
  PRIMARY KEY (repo, branch)
);
CREATE TABLE relations (
  issue_id   TEXT PRIMARY KEY,
  data       TEXT NOT NULL,      -- 하위·막힘·막는 중·관련 JSON
  fetched_at INTEGER NOT NULL
);
```
````

7.2 상세 줄을 찾아 바꾼다.

```markdown
| 상세 | `issue(id: <식별자>) { ...IssueFields comments(first: 50) { nodes { id body createdAt editedAt user { id name displayName } } pageInfo { hasNextPage } } }` |
```

```markdown
| 상세 | `issue(id: <식별자>) { ...IssueFields comments(first: 50) { nodes { id body createdAt editedAt user { id name displayName } } pageInfo { hasNextPage } } children(first: 50) { … } relations(first: 50) { … } inverseRelations(first: 50) { … } }`. 관계 필드는 `2026-10-07-issue-relations-design.md` 4.1을 따른다 |
```

7.2 `IssueFields` 블록에서 찾아 바꾼다.

```graphql
  parent { id identifier title }
```

```graphql
  parent { id identifier title state { id name type color } }
```

- [ ] **Step 3: 관계 스펙 상태**

`docs/superpowers/specs/2026-10-07-issue-relations-design.md`의 `- 상태: 사용자 리뷰 대기`를 `- 상태: 구현 완료 (v0.1.2)`로 바꾼다.

- [ ] **Step 4: README** (영어·한국어 모두)

영어 기능 목록에서 `- **Issue detail**: …` 줄 바로 아래에 더한다.

```markdown
- **Relations**: the detail view lists the parent, sub-issues, blocked by, blocking, and related issues in Linear's state colors. Press `t` or click a line to open one, and Esc to come back.
```

영어 키 표의 Detail 줄을 찾아 바꾼다.

```markdown
| Detail | j/k to scroll, Ctrl+D/U for half a page, g/G for top/bottom, `u` for links, `y` `Y` `o` `r`, Esc to go back, `q` to close |
```

```markdown
| Detail | j/k to scroll, Ctrl+D/U for half a page, g/G for top/bottom, `t` for relations, `u` for links, `y` `Y` `o` `r`, Esc to go back, `q` to close |
```

한국어 기능 목록에서 `- **상세**: …` 줄 바로 아래에 더한다.

```markdown
- **관계**: 상세에 상위·하위·막힘·막는 중·관련 이슈를 Linear 상태 색으로 보여 줘요. `t`를 누르거나 줄을 클릭하면 그 이슈로 가고, Esc로 돌아와요.
```

한국어 키 표의 상세 줄을 찾아 바꾼다.

```markdown
| 상세 | j/k = 스크롤, Ctrl+D/U = 반 페이지, g/G = 처음/끝, `u` = 링크 목록, `y`·`Y`·`o`·`r`, Esc = 뒤로, `q` = 닫기 |
```

```markdown
| 상세 | j/k = 스크롤, Ctrl+D/U = 반 페이지, g/G = 처음/끝, `t` = 관계, `u` = 링크 목록, `y`·`Y`·`o`·`r`, Esc = 뒤로, `q` = 닫기 |
```

- [ ] **Step 5: ROADMAP** (영어·한국어 모두)

영어 `## 🔜 v0.2 — side panel and actions (next)` 바로 앞에 더한다.

```markdown
## ✅ v0.1.2 — issue relations

- The detail view shows the parent, sub-issues, blocked by, blocking, and related issues, colored by state. Open blockers stand out in red.
- Press `t` or click a line to open a related issue. Esc walks back one issue at a time.
- `herdr-linear show` prints the same relations, and they stay readable offline.

```

한국어 `## 🔜 v0.2 — 사이드 패널과 변경 동작 (다음)` 바로 앞에 더한다.

```markdown
## ✅ v0.1.2 — 관계 보기

- 상세에 상위·하위·막힘·막는 중·관련 이슈를 상태 색과 함께 보여 줘요. 안 끝난 막는 이슈는 빨강으로 눈에 띄어요.
- `t`를 누르거나 줄을 클릭하면 그 이슈로 가고, Esc로 한 단계씩 돌아와요.
- `herdr-linear show`에도 같은 관계가 나오고, 오프라인에서도 볼 수 있어요.

```

- [ ] **Step 6: 확인과 커밋**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && cargo build --release`
Expected: 모두 통과(287개 통과, 1개 무시). release 빌드도 성공한다.

```bash
git add Cargo.toml Cargo.lock herdr-plugin.toml README.md ROADMAP.md docs/superpowers/specs/
git commit -m "docs: v0.1.2 — 관계 보기 스펙·README·로드맵 반영, 버전 0.1.2"
```

---

### Task 10: herdr에서 직접 확인

**Files:**
- Create: `docs/superpowers/notes/2026-10-07-relations-herdr-checks.md`

- [ ] **Step 1: 배포 (사용자 확인 후)**

`scripts/deploy-local.sh`는 이 기기에서 매일 쓰는 플러그인을 바꾼다. 그래서 실행하기 전에 사용자에게 배포해도 되는지 묻는다. 허락을 받으면 실행한다.

Run: `scripts/deploy-local.sh`
Expected: "배포했어요: …"와 `jh.linear (…)` 줄이 나온다.

- [ ] **Step 2: 사용자와 함께 체크리스트를 돈다**

상위·하위·막힘이 있는 실제 이슈로 아래를 확인한다.

| # | 항목 | 기대 |
|---|---|---|
| 1 | 그런 이슈의 상세 | 머리·URL 아래에 관계 칸이 있다. 막힘은 빨강, 막는 중은 노랑, 끝난 줄은 흐리게, 하위 요약이 보인다 |
| 2 | `t` | 관계 메뉴가 같은 색으로 뜬다. 글자로 걸러지고 Enter로 열린다 |
| 3 | Esc 여러 번 | 한 단계씩 돌아가고, 스크롤 위치가 그대로다. 마지막에는 목록으로 간다 |
| 4 | 관계 줄 클릭(스크롤한 뒤에도) | 누른 줄의 이슈가 열린다 |
| 5 | 한글 입력 상태에서 `ㅅ` | `t`와 같다 |
| 6 | 관계 없는 이슈 | 관계 칸이 없고, `t`를 누르면 "관계 없음"이 뜬다 |
| 7 | `herdr-linear show <식별자>` | URL 다음에 관계 칸이 나오고 색이 맞는다 |
| 8 | `HTTPS_PROXY=http://127.0.0.1:9 herdr-linear show <식별자>` | 오프라인 안내와 함께 캐시된 관계가 나온다 |

- [ ] **Step 3: 기록과 정리**

결과를 `docs/superpowers/notes/2026-10-07-relations-herdr-checks.md`에 표로 남긴다. 이슈 제목·사람 이름은 쓰지 않는다. 문제가 나오면 고칠 작업으로 따로 적는다. 브레인스토밍 때 만든 임시 파일도 지운다: `rm -rf /tmp/herdr-linear-preview /tmp/linear-schema-dl`.

```bash
git add docs/superpowers/notes/2026-10-07-relations-herdr-checks.md
git commit -m "docs: v0.1.2 관계 보기 herdr 확인 결과"
```
