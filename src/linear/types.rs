//! Linear 응답 타입. 숫자 필드 중 Linear 스키마에서 Float인 것은 f64로 받는다.

use serde::{Deserialize, Deserializer, Serialize};

/// Linear는 `number`, `priority`를 Float으로 준다. 정수로 바꿔 쓴다.
fn f64_as_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(f64::deserialize(d)? as i64)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub identifier: String,
    #[serde(deserialize_with = "f64_as_i64")]
    pub number: i64,
    pub title: String,
    pub description: Option<String>,
    /// 0 없음, 1 긴급, 2 높음, 3 보통, 4 낮음
    #[serde(deserialize_with = "f64_as_i64")]
    pub priority: i64,
    pub estimate: Option<f64>,
    pub url: String,
    pub branch_name: String,
    pub due_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
    #[serde(default)]
    pub trashed: Option<bool>,
    pub team: TeamRef,
    pub state: StateRef,
    pub assignee: Option<UserRef>,
    pub project: Option<NamedRef>,
    pub cycle: Option<CycleRef>,
    pub parent: Option<ParentRef>,
    pub labels: Nodes<LabelRef>,
    /// 연결된 링크(GitHub PR 등). 예전 캐시에는 없을 수 있다
    #[serde(default)]
    pub attachments: Nodes<Attachment>,
}

impl Issue {
    /// 보관됐거나 휴지통에 있는 이슈인지.
    pub fn is_gone(&self) -> bool {
        self.archived_at.is_some() || self.trashed == Some(true)
    }

    pub fn label_names(&self) -> Vec<&str> {
        self.labels.nodes.iter().map(|l| l.name.as_str()).collect()
    }

    /// 연결된 열린 PR. 여러 개면 가장 최근에 연결된 것.
    pub fn open_pr(&self) -> Option<PullRequest> {
        self.attachments
            .nodes
            .iter()
            .filter(|a| pr_number(&a.url).is_some())
            .filter(|a| {
                a.metadata
                    .as_ref()
                    .and_then(|m| m.status.as_deref())
                    .is_some_and(|s| {
                        s.eq_ignore_ascii_case("open") || s.eq_ignore_ascii_case("opened")
                    })
            })
            .max_by(|a, b| a.created_at.cmp(&b.created_at))
            .map(|a| {
                let meta = a.metadata.clone().unwrap_or_default();
                PullRequest {
                    url: a.url.clone(),
                    number: meta.number.map(|n| n as i64).or_else(|| pr_number(&a.url)),
                    repo: meta.repo_name,
                    draft: meta.draft.unwrap_or(false),
                }
            })
    }
}

/// GitHub `…/pull/<번호>`, GitLab `…/merge_requests/<번호>` 링크면 그 번호.
fn pr_number(url: &str) -> Option<i64> {
    ["/pull/", "/merge_requests/"].iter().find_map(|marker| {
        let rest = url.split_once(marker)?.1;
        rest.split(['/', '?', '#']).next()?.parse().ok()
    })
}

/// 이슈에 붙은 링크. 쓰는 필드만 받는다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub url: String,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    /// Linear의 metadata는 연동마다 모양이 달라서, 쓰는 값만 너그럽게 읽는다
    #[serde(default, deserialize_with = "lenient_meta")]
    pub metadata: Option<AttachmentMeta>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    pub status: Option<String>,
    pub number: Option<f64>,
    pub repo_name: Option<String>,
    pub draft: Option<bool>,
}

/// 모양이 다르거나 타입이 맞지 않는 값은 버리고, 이슈 전체를 읽는 데는 실패하지 않는다.
fn lenient_meta<'de, D: Deserializer<'de>>(d: D) -> Result<Option<AttachmentMeta>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    if !v.is_object() {
        return Ok(None);
    }
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Ok(Some(AttachmentMeta {
        status: text("status"),
        number: v.get("number").and_then(|x| x.as_f64()),
        repo_name: text("repoName"),
        draft: v.get("draft").and_then(|x| x.as_bool()),
    }))
}

/// 이슈에 연결된 PR.
#[derive(Debug, Clone, PartialEq)]
pub struct PullRequest {
    pub url: String,
    pub number: Option<i64>,
    pub repo: Option<String>,
    pub draft: bool,
}

impl PullRequest {
    /// `PR #482`, 번호를 모르면 `PR`.
    pub fn label(&self) -> String {
        match self.number {
            Some(n) => format!("PR #{n}"),
            None => "PR".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nodes<T> {
    pub nodes: Vec<T>,
}

impl<T> Default for Nodes<T> {
    fn default() -> Self {
        Nodes { nodes: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRef {
    pub id: String,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateRef {
    pub id: String,
    pub name: String,
    /// triage, backlog, unstarted, started, completed, canceled, duplicate
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRef {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CycleRef {
    pub id: String,
    pub number: f64,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
    /// 상위 이슈의 상태. 예전 캐시에는 없다
    #[serde(default)]
    pub state: Option<StateRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelRef {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub body: String,
    pub created_at: String,
    pub edited_at: Option<String>,
    pub user: Option<UserRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewer {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub email: String,
    pub organization: Organization,
    pub teams: Nodes<TeamRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub url_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuePage {
    pub nodes: Vec<Issue>,
    pub page_info: PageInfo,
}

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
    /// 관계. 관계 없이 다시 받았으면 `None`(모름)
    pub relations: Option<IssueRelations>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;

    #[test]
    fn open_pr_picks_the_latest_open_pull_request() {
        let issue = IssueBuilder::new("a", "ENG-1", "로그인")
            .pr(
                "https://github.com/acme/web/pull/10",
                "merged",
                10,
                "2026-10-01T00:00:00.000Z",
            )
            .pr(
                "https://github.com/acme/web/pull/12",
                "open",
                12,
                "2026-10-02T00:00:00.000Z",
            )
            .pr(
                "https://github.com/acme/web/issues/3",
                "open",
                3,
                "2026-10-06T00:00:00.000Z",
            )
            .pr(
                "https://github.com/acme/web/pull/15",
                "open",
                15,
                "2026-10-05T00:00:00.000Z",
            )
            .pr(
                "https://github.com/acme/web/pull/9",
                "closed",
                9,
                "2026-10-07T00:00:00.000Z",
            )
            .build();
        let pr = issue.open_pr().unwrap();
        assert_eq!(pr.url, "https://github.com/acme/web/pull/15");
        assert_eq!(pr.label(), "PR #15");
        assert_eq!(pr.repo.as_deref(), Some("web"));
        assert!(
            IssueBuilder::new("b", "ENG-2", "없음")
                .build()
                .open_pr()
                .is_none()
        );
    }

    #[test]
    fn odd_attachment_metadata_does_not_break_the_issue() {
        let mut v = IssueBuilder::new("a", "ENG-1", "로그인").json();
        v["attachments"] = serde_json::json!({ "nodes": [
            { "url": "https://github.com/acme/web/pull/7", "sourceType": "github", "metadata": { "status": 123, "number": "x" } },
            { "url": "https://figma.com/file/abc", "sourceType": "figma", "metadata": null },
            { "url": "https://github.com/acme/web/pull/8", "sourceType": "github", "metadata": { "status": "open" } }
        ] });
        let issue: Issue = serde_json::from_value(v).unwrap();
        let pr = issue.open_pr().unwrap();
        assert_eq!(pr.url, "https://github.com/acme/web/pull/8");
        assert_eq!(pr.label(), "PR #8", "번호가 없으면 URL에서 읽는다");
        // 캐시에 저장했다 다시 읽어도 같다
        let back: Issue = serde_json::from_str(&serde_json::to_string(&issue).unwrap()).unwrap();
        assert_eq!(back.open_pr(), issue.open_pr());
    }

    #[test]
    fn float_numbers_become_integers() {
        let mut v = IssueBuilder::new("i1", "ENG-131", "로그인 버그").json();
        v["number"] = serde_json::json!(131.0);
        v["priority"] = serde_json::json!(2.0);
        let issue: Issue = serde_json::from_value(v).unwrap();
        assert_eq!(issue.number, 131);
        assert_eq!(issue.priority, 2);
    }

    #[test]
    fn roundtrip_through_json_keeps_fields() {
        let issue = IssueBuilder::new("i1", "ENG-131", "로그인 버그")
            .labels(&["bug", "frontend"])
            .assignee("u1", "김민수")
            .build();
        let back: Issue = serde_json::from_str(&serde_json::to_string(&issue).unwrap()).unwrap();
        assert_eq!(back, issue);
        assert_eq!(back.label_names(), vec!["bug", "frontend"]);
    }

    #[test]
    fn archived_or_trashed_is_gone() {
        assert!(!IssueBuilder::new("i1", "ENG-1", "a").build().is_gone());
        assert!(
            IssueBuilder::new("i1", "ENG-1", "a")
                .archived()
                .build()
                .is_gone()
        );
        let mut v = IssueBuilder::new("i1", "ENG-1", "a").json();
        v["trashed"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Issue>(v).unwrap().is_gone());
    }

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
}
