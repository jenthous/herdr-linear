//! 테스트용 이슈 데이터.

use serde_json::{Value, json};

use crate::linear::types::Issue;

/// 필드를 바꿔 가며 테스트 이슈를 만든다.
pub struct IssueBuilder(Value);

impl IssueBuilder {
    /// `identifier`는 `KEY-번호` 형식이어야 한다.
    pub fn new(id: &str, identifier: &str, title: &str) -> Self {
        let (key, num) = identifier.split_once('-').expect("KEY-번호 형식");
        IssueBuilder(json!({
            "id": id,
            "identifier": identifier,
            "number": num.parse::<f64>().unwrap(),
            "title": title,
            "description": null,
            "priority": 0.0,
            "estimate": null,
            "url": format!("https://linear.app/acme/issue/{identifier}"),
            "branchName": format!("me/{}", identifier.to_lowercase()),
            "dueDate": null,
            "createdAt": "2026-10-01T00:00:00.000Z",
            "updatedAt": "2026-10-01T00:00:00.000Z",
            "archivedAt": null,
            "trashed": null,
            "team": { "id": format!("team-{key}"), "key": key, "name": key },
            "state": { "id": "st-unstarted", "name": "Todo", "type": "unstarted", "color": "#e2e2e2" },
            "assignee": null,
            "project": null,
            "cycle": null,
            "parent": null,
            "labels": { "nodes": [] }
        }))
    }

    pub fn description(mut self, d: &str) -> Self {
        self.0["description"] = json!(d);
        self
    }

    pub fn state(mut self, name: &str, state_type: &str) -> Self {
        self.0["state"] = json!({ "id": format!("st-{state_type}"), "name": name, "type": state_type, "color": "#5e6ad2" });
        self
    }

    pub fn priority(mut self, p: i64) -> Self {
        self.0["priority"] = json!(p as f64);
        self
    }

    pub fn updated(mut self, ts: &str) -> Self {
        self.0["updatedAt"] = json!(ts);
        self
    }

    pub fn assignee(mut self, id: &str, name: &str) -> Self {
        self.0["assignee"] = json!({ "id": id, "name": name, "displayName": name });
        self
    }

    pub fn labels(mut self, names: &[&str]) -> Self {
        let nodes: Vec<Value> = names
            .iter()
            .map(|n| json!({ "id": format!("lb-{n}"), "name": n, "color": "#eb5757" }))
            .collect();
        self.0["labels"] = json!({ "nodes": nodes });
        self
    }

    pub fn archived(mut self) -> Self {
        self.0["archivedAt"] = json!("2026-10-02T00:00:00.000Z");
        self
    }

    pub fn json(self) -> Value {
        self.0
    }

    pub fn build(self) -> Issue {
        serde_json::from_value(self.0).expect("유효한 이슈 JSON")
    }
}
