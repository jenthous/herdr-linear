//! GraphQL 문서와 호출 함수.

use serde::Deserialize;
use serde_json::{Value, json};

use super::client::{ApiError, LinearClient};
use super::types::{
    Comment, Issue, IssueDetail, IssuePage, IssueRelations, Nodes, PageInfo, RelatedIssue, Viewer,
};

pub const PAGE_SIZE: i64 = 50;
pub const DEEP_SEARCH_SIZE: i64 = 20;
pub const COMMENT_PAGE_SIZE: i64 = 50;
/// 상세에서 하위·관계를 종류마다 몇 개까지 받는지.
pub const RELATION_PAGE_SIZE: i64 = 50;

/// 관계로 이어진 이슈에서 받는 필드 (`RelatedIssue`).
const RELATED_SELECTION: &str = "id identifier title state { id name type color }";

/// 이슈 하나에 대해 가져오는 필드. `Issue`와 `IssueSearchResult`에 똑같이 쓴다.
const ISSUE_SELECTION: &str = "id identifier number title description priority estimate url branchName dueDate createdAt updatedAt archivedAt trashed team { id key name } state { id name type color } assignee { id name displayName } project { id name } cycle { id number name } parent { id identifier title state { id name type color } } labels(first: 20) { nodes { id name color } } attachments(first: 10) { nodes { url sourceType createdAt metadata } }";

pub fn issue_fragment() -> String {
    format!("fragment IssueFields on Issue {{ {ISSUE_SELECTION} }}")
}

fn search_fragment() -> String {
    format!("fragment SearchFields on IssueSearchResult {{ {ISSUE_SELECTION} }}")
}

/// 키 검증과 내 정보.
pub fn viewer(c: &LinearClient) -> Result<Viewer, ApiError> {
    #[derive(Deserialize)]
    struct D {
        viewer: Viewer,
    }
    let q = "query Viewer { viewer { id name displayName email organization { id name urlKey } teams { nodes { id key name } } } }";
    Ok(c.execute::<D>(q, json!({}))?.viewer)
}

/// 나에게 할당된 열린 이슈 (완료·취소·중복 제외).
pub fn my_issues(c: &LinearClient) -> Result<Vec<Issue>, ApiError> {
    let filter = json!({
        "assignee": { "isMe": { "eq": true } },
        "state": { "type": { "nin": ["completed", "canceled", "duplicate"] } }
    });
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

/// 필터 검색 (첫 50건).
pub fn filter_issues(c: &LinearClient, filter: &Value) -> Result<Vec<Issue>, ApiError> {
    Ok(issues_page(c, filter, None)?.nodes)
}

fn issues_page(
    c: &LinearClient,
    filter: &Value,
    after: Option<&str>,
) -> Result<IssuePage, ApiError> {
    #[derive(Deserialize)]
    struct D {
        issues: IssuePage,
    }
    let q = format!(
        "query Issues($filter: IssueFilter, $after: String, $first: Int) {{ issues(filter: $filter, after: $after, first: $first, orderBy: updatedAt) {{ nodes {{ ...IssueFields }} pageInfo {{ hasNextPage endCursor }} }} }} {}",
        issue_fragment()
    );
    let vars = json!({ "filter": filter, "after": after, "first": PAGE_SIZE });
    Ok(c.execute::<D>(&q, vars)?.issues)
}

/// 서버 깊은 검색 (코멘트 포함). 분당 30회 제한이 있으니 사용자가 고를 때만 부른다.
pub fn deep_search(
    c: &LinearClient,
    term: &str,
    filter: Option<&Value>,
) -> Result<Vec<Issue>, ApiError> {
    #[derive(Deserialize)]
    struct D {
        #[serde(rename = "searchIssues")]
        search: Nodes<Issue>,
    }
    let q = format!(
        "query Deep($term: String!, $filter: IssueFilter, $first: Int) {{ searchIssues(term: $term, filter: $filter, first: $first, includeComments: true) {{ nodes {{ ...SearchFields }} }} }} {}",
        search_fragment()
    );
    let vars = json!({ "term": term, "filter": filter, "first": DEEP_SEARCH_SIZE });
    Ok(c.execute::<D>(&q, vars)?.search.nodes)
}

/// 이슈, 코멘트, 하위·관계. 이슈가 없으면 `None`.
/// 관계 때문에 요청이 실패하면 관계 없이 한 번 더 받고, 그때 `relations`는 `None`(모름)이다.
pub fn issue_detail(c: &LinearClient, id: &str) -> Result<Option<IssueDetail>, ApiError> {
    let result = match detail_query(c, id, true) {
        // 볼 수 없는 팀의 이슈와 맺은 관계처럼 관계 필드 때문에 응답 전체가 오류가 될 수 있다.
        // 관계 없이 한 번 더 받아 상세는 보이게 하고, 없는 이슈인지도 그 응답으로 판단한다
        Err(ApiError::GraphQl(_) | ApiError::Decode(_)) => detail_query(c, id, false),
        other => other,
    };
    match result {
        Ok(d) => Ok(Some(d)),
        Err(ApiError::GraphQl(msg)) if is_not_found(&msg) => Ok(None),
        Err(e) => Err(e),
    }
}

/// 상세 요청 한 번. `with_relations`가 거짓이면 하위·관계 필드를 빼고 받고, `relations`는 `None`이다.
fn detail_query(c: &LinearClient, id: &str, with_relations: bool) -> Result<IssueDetail, ApiError> {
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
    // 관계 필드는 코멘트 블록 바로 뒤에 붙는다. 빼면 이슈 선택이 거기서 닫힌다
    let relation_fields = if with_relations {
        format!(
            " children(first: {RELATION_PAGE_SIZE}) {{ nodes {{ {RELATED_SELECTION} subIssueSortOrder }} pageInfo {{ hasNextPage }} }} \
             relations(first: {RELATION_PAGE_SIZE}) {{ nodes {{ type relatedIssue {{ {RELATED_SELECTION} }} }} }} \
             inverseRelations(first: {RELATION_PAGE_SIZE}) {{ nodes {{ type issue {{ {RELATED_SELECTION} }} }} }}"
        )
    } else {
        String::new()
    };
    let q = format!(
        "query Detail($id: String!, $first: Int) {{ issue(id: $id) {{ ...IssueFields \
         comments(first: $first) {{ nodes {{ id body createdAt editedAt user {{ id name displayName }} }} pageInfo {{ hasNextPage endCursor }} }}{relation_fields} }} }} {}",
        issue_fragment()
    );
    let d = c.execute::<D>(&q, json!({ "id": id, "first": COMMENT_PAGE_SIZE }))?;
    let mut comments = d.issue.comments.nodes;
    comments.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    // 관계 필드가 없는 응답도 관계를 요청했다면 빈 관계다
    let relations = with_relations.then(|| {
        let (children, more_children) = d.issue.children.map_or((Vec::new(), false), |p| {
            (p.nodes, p.page_info.has_next_page)
        });
        relations_from(
            children,
            more_children,
            d.issue.relations.map(|n| n.nodes).unwrap_or_default(),
            d.issue
                .inverse_relations
                .map(|n| n.nodes)
                .unwrap_or_default(),
        )
    });
    Ok(IssueDetail {
        issue: d.issue.issue,
        comments,
        more_comments: d.issue.comments.page_info.has_next_page,
        relations,
    })
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

fn is_not_found(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("not found") || m.contains("could not find")
}

/// 브랜치명으로 연결된 이슈를 찾는다.
pub fn branch_issue(c: &LinearClient, branch: &str) -> Result<Option<Issue>, ApiError> {
    #[derive(Deserialize)]
    struct D {
        #[serde(rename = "issueVcsBranchSearch")]
        issue: Option<Issue>,
    }
    let q = format!(
        "query Branch($branch: String!) {{ issueVcsBranchSearch(branchName: $branch) {{ ...IssueFields }} }} {}",
        issue_fragment()
    );
    Ok(c.execute::<D>(&q, json!({ "branch": branch }))?.issue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;
    use mockito::Matcher;

    fn client(server: &mockito::Server) -> LinearClient {
        LinearClient::with_endpoint("lin_api_test", format!("{}/graphql", server.url()))
    }

    #[test]
    fn viewer_parses_org_and_teams() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Viewer".into()))
            .with_body(
                json!({ "data": { "viewer": {
                    "id": "u1", "name": "김민수", "displayName": "minsu", "email": "m@acme.dev",
                    "organization": { "id": "org1", "name": "Acme", "urlKey": "acme" },
                    "teams": { "nodes": [ { "id": "team-ENG", "key": "ENG", "name": "Engineering" } ] }
                } } })
                .to_string(),
            )
            .create();
        let v = viewer(&client(&server)).unwrap();
        assert_eq!(v.organization.name, "Acme");
        assert_eq!(v.teams.nodes[0].key, "ENG");
    }

    #[test]
    fn issue_queries_ask_for_attachments() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex(
                r"attachments\(first: 10\) \{ nodes \{ url sourceType createdAt metadata \} \}".into(),
            ))
            .with_body(
                json!({ "data": { "issues": {
                    "nodes": [ IssueBuilder::new("i1", "ENG-1", "첫 이슈")
                        .pr("https://github.com/acme/web/pull/3", "open", 3, "2026-10-01T00:00:00.000Z")
                        .json() ],
                    "pageInfo": { "hasNextPage": false, "endCursor": null }
                } } })
                .to_string(),
            )
            .create();
        let issues = my_issues(&client(&server)).unwrap();
        assert_eq!(issues[0].open_pr().unwrap().label(), "PR #3");
    }

    #[test]
    fn my_issues_sends_filter_and_fragment() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/graphql")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("fragment IssueFields on Issue".into()),
                Matcher::Regex("orderBy: updatedAt".into()),
                Matcher::PartialJson(json!({ "variables": {
                    "first": 50,
                    "filter": {
                        "assignee": { "isMe": { "eq": true } },
                        "state": { "type": { "nin": ["completed", "canceled", "duplicate"] } }
                    }
                } })),
            ]))
            .with_body(
                json!({ "data": { "issues": {
                    "nodes": [ IssueBuilder::new("i1", "ENG-1", "첫 이슈").json() ],
                    "pageInfo": { "hasNextPage": false, "endCursor": null }
                } } })
                .to_string(),
            )
            .create();
        let issues = my_issues(&client(&server)).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].identifier, "ENG-1");
        m.assert();
    }

    #[test]
    fn team_issues_pages_with_cursor() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::PartialJson(json!({ "variables": {
                "after": "cur1",
                "filter": { "team": { "id": { "in": ["team-ENG"] } } }
            } })))
            .with_body(
                json!({ "data": { "issues": {
                    "nodes": [ IssueBuilder::new("i2", "ENG-2", "둘째").json() ],
                    "pageInfo": { "hasNextPage": true, "endCursor": "cur2" }
                } } })
                .to_string(),
            )
            .create();
        let page = team_issues(&client(&server), &["team-ENG".to_string()], Some("cur1")).unwrap();
        assert!(page.page_info.has_next_page);
        assert_eq!(page.page_info.end_cursor.as_deref(), Some("cur2"));
    }

    #[test]
    fn deep_search_uses_search_fragment() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("fragment SearchFields on IssueSearchResult".into()),
                Matcher::Regex("includeComments: true".into()),
                Matcher::PartialJson(json!({ "variables": { "term": "세션", "first": 20 } })),
            ]))
            .with_body(
                json!({ "data": { "searchIssues": { "nodes": [ IssueBuilder::new("i3", "ENG-3", "세션 만료").json() ] } } })
                    .to_string(),
            )
            .create();
        let found = deep_search(&client(&server), "세션", None).unwrap();
        assert_eq!(found[0].identifier, "ENG-3");
    }

    #[test]
    fn detail_sorts_comments_oldest_first() {
        let mut server = mockito::Server::new();
        let mut issue = IssueBuilder::new("i1", "ENG-1", "상세").json();
        issue["comments"] = json!({
            "nodes": [
                { "id": "c2", "body": "두 번째", "createdAt": "2026-10-02T00:00:00.000Z", "editedAt": null, "user": null },
                { "id": "c1", "body": "첫 번째", "createdAt": "2026-10-01T00:00:00.000Z", "editedAt": null,
                  "user": { "id": "u1", "name": "김민수", "displayName": "minsu" } }
            ],
            "pageInfo": { "hasNextPage": true, "endCursor": "x" }
        });
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::PartialJson(
                json!({ "variables": { "id": "ENG-1", "first": 50 } }),
            ))
            .with_body(json!({ "data": { "issue": issue } }).to_string())
            .create();
        let d = issue_detail(&client(&server), "ENG-1").unwrap().unwrap();
        assert_eq!(d.issue.identifier, "ENG-1");
        assert_eq!(
            d.comments.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["c1", "c2"]
        );
        assert!(d.more_comments);
        assert_eq!(
            d.relations,
            Some(IssueRelations::default()),
            "관계 필드가 없는 응답은 빈 관계"
        );
    }

    #[test]
    fn detail_not_found_is_none() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"code":"INVALID_INPUT"}}]}"#)
            .create();
        assert_eq!(issue_detail(&client(&server), "ENG-404").unwrap(), None);
    }

    #[test]
    fn branch_issue_can_be_missing() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::PartialJson(
                json!({ "variables": { "branch": "main" } }),
            ))
            .with_body(r#"{"data":{"issueVcsBranchSearch":null}}"#)
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
        let r = d.relations.as_ref().unwrap();
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

    /// 관계 필드를 넣은 상세 요청의 본문.
    fn relation_query_body() -> Matcher {
        Matcher::Regex("inverseRelations".into())
    }

    /// 관계 필드를 뺀 상세 요청의 본문. 이슈 선택이 코멘트 블록 바로 뒤에서 닫힌다.
    fn plain_query_body() -> Matcher {
        Matcher::Regex(r"endCursor \} \} \} \}".into())
    }

    /// 코멘트가 없는 상세 응답의 이슈.
    fn plain_detail_issue() -> Value {
        let mut issue = IssueBuilder::new("i1", "ENG-1", "상세").json();
        issue["comments"] =
            json!({ "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } });
        issue
    }

    #[test]
    fn relation_error_falls_back_to_detail_without_relations() {
        // 볼 수 없는 팀의 이슈와 맺은 관계처럼 관계 필드 때문에 Linear가 응답 전체를 오류로 줄 때.
        // "찾을 수 없음"처럼 보여도 있는 이슈를 없는 이슈로 다루면 안 된다
        let cases = [
            (400, "Entity not found: Issue"),
            (200, "Entity not found: Issue"),
            (200, "Something went wrong"),
        ];
        for (status, message) in cases {
            let mut server = mockito::Server::new();
            let relation_request = server
                .mock("POST", "/graphql")
                .match_body(relation_query_body())
                .with_status(status)
                .with_body(json!({ "errors": [{ "message": message }] }).to_string())
                .expect(1)
                .create();
            let mut issue = plain_detail_issue();
            issue["comments"]["nodes"] = json!([
                { "id": "c1", "body": "첫 번째", "createdAt": "2026-10-01T00:00:00.000Z", "editedAt": null, "user": null }
            ]);
            let plain_request = server
                .mock("POST", "/graphql")
                .match_body(plain_query_body())
                .with_body(json!({ "data": { "issue": issue } }).to_string())
                .expect(1)
                .create();
            let found = issue_detail(&client(&server), "ENG-1").unwrap();
            assert!(
                found.is_some(),
                "{status} {message}: 있는 이슈를 없는 이슈로 다뤘어요"
            );
            let d = found.unwrap();
            assert_eq!(d.issue.identifier, "ENG-1", "{status} {message}");
            assert_eq!(d.comments.len(), 1, "{status} {message}: 코멘트는 그대로");
            assert_eq!(d.relations, None, "{status} {message}: 관계는 모름");
            relation_request.assert();
            plain_request.assert();
        }
    }

    #[test]
    fn undecodable_relations_fall_back_to_detail_without_relations() {
        // 하위 목록에 pageInfo가 없는 모양처럼 관계 필드를 읽지 못할 때
        let mut server = mockito::Server::new();
        let mut broken = plain_detail_issue();
        broken["children"] = json!({ "nodes": [] });
        let relation_request = server
            .mock("POST", "/graphql")
            .match_body(relation_query_body())
            .with_body(json!({ "data": { "issue": broken } }).to_string())
            .expect(1)
            .create();
        let plain_request = server
            .mock("POST", "/graphql")
            .match_body(plain_query_body())
            .with_body(json!({ "data": { "issue": plain_detail_issue() } }).to_string())
            .expect(1)
            .create();
        let d = issue_detail(&client(&server), "ENG-1").unwrap().unwrap();
        assert_eq!(d.issue.identifier, "ENG-1");
        assert_eq!(d.relations, None);
        relation_request.assert();
        plain_request.assert();
    }

    #[test]
    fn auth_rate_limit_and_offline_errors_are_not_retried() {
        let auth = r#"{"errors":[{"message":"Authentication required","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#;
        let limited =
            r#"{"errors":[{"message":"Rate limit exceeded","extensions":{"code":"RATELIMITED"}}]}"#;
        // 오류 종류만 비교한다 (안의 값은 보지 않는다)
        let cases = [
            (400, auth, ApiError::Auth),
            (400, limited, ApiError::RateLimited { reset_at_ms: None }),
            (503, "busy", ApiError::Offline(String::new())),
        ];
        for (status, body, expected) in cases {
            let mut server = mockito::Server::new();
            let relation_request = server
                .mock("POST", "/graphql")
                .match_body(relation_query_body())
                .with_status(status)
                .with_body(body)
                .expect(1)
                .create();
            // 관계 없는 요청이 가면 안 된다
            let plain_request = server
                .mock("POST", "/graphql")
                .match_body(plain_query_body())
                .with_body(json!({ "data": { "issue": plain_detail_issue() } }).to_string())
                .expect(0)
                .create();
            let err = issue_detail(&client(&server), "ENG-1").unwrap_err();
            assert_eq!(
                std::mem::discriminant(&err),
                std::mem::discriminant(&expected),
                "{status} {body}: {err:?}"
            );
            relation_request.assert();
            plain_request.assert();
        }
    }
}
