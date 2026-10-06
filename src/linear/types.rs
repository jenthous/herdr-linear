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
}

impl Issue {
    /// 보관됐거나 휴지통에 있는 이슈인지.
    pub fn is_gone(&self) -> bool {
        self.archived_at.is_some() || self.trashed == Some(true)
    }

    pub fn label_names(&self) -> Vec<&str> {
        self.labels.nodes.iter().map(|l| l.name.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nodes<T> {
    pub nodes: Vec<T>,
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

/// 상세 화면용: 이슈 + 코멘트(오래된 것부터).
#[derive(Debug, Clone, PartialEq)]
pub struct IssueDetail {
    pub issue: Issue,
    pub comments: Vec<Comment>,
    /// 코멘트가 50개를 넘는지.
    pub more_comments: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;

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
}
