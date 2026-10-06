//! GraphQL 문서와 호출 함수.

use serde::Deserialize;
use serde_json::{Value, json};

use super::client::{ApiError, LinearClient};
use super::types::{Comment, Issue, IssueDetail, IssuePage, Nodes, PageInfo, Viewer};

pub const PAGE_SIZE: i64 = 50;
pub const DEEP_SEARCH_SIZE: i64 = 20;
pub const COMMENT_PAGE_SIZE: i64 = 50;

/// 이슈 하나에 대해 가져오는 필드. `Issue`와 `IssueSearchResult`에 똑같이 쓴다.
const ISSUE_SELECTION: &str = "id identifier number title description priority estimate url branchName dueDate createdAt updatedAt archivedAt trashed team { id key name } state { id name type color } assignee { id name displayName } project { id name } cycle { id number name } parent { id identifier title } labels(first: 20) { nodes { id name color } }";

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

/// 범위 팀의 이슈를 최근 수정 순으로 한 페이지씩.
pub fn team_issues(
    c: &LinearClient,
    team_ids: &[String],
    after: Option<&str>,
) -> Result<IssuePage, ApiError> {
    let filter = json!({ "team": { "id": { "in": team_ids } } });
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
}
