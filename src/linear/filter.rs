//! 검색어 → Linear `IssueFilter` JSON.

use serde_json::{Value, json};

use crate::search::query::{AssigneeMatch, ParsedQuery, StateMatch};

/// 서버 필터 검색용. `scope_team_ids`는 `#KEY` 토큰이 없을 때만 적용한다.
pub fn build_issue_filter(q: &ParsedQuery, scope_team_ids: &[String]) -> Value {
    let mut parts = text_parts(q);
    parts.extend(token_parts(q));
    if q.team.is_none() && !scope_team_ids.is_empty() {
        parts.push(json!({ "team": { "id": { "in": scope_team_ids } } }));
    }
    combine(parts)
}

/// 깊은 검색(`searchIssues`)용: 토큰 조건만. 없으면 `None`.
pub fn token_filter(q: &ParsedQuery) -> Option<Value> {
    let parts = token_parts(q);
    (!parts.is_empty()).then(|| combine(parts))
}

fn combine(parts: Vec<Value>) -> Value {
    match parts.len() {
        0 => json!({}),
        1 => parts.into_iter().next().unwrap(),
        _ => json!({ "and": parts }),
    }
}

/// 제목 또는 본문에 포함.
fn contains_word(w: &str) -> Vec<Value> {
    vec![
        json!({ "title": { "containsIgnoreCase": w } }),
        json!({ "description": { "containsIgnoreCase": w } }),
    ]
}

fn text_parts(q: &ParsedQuery) -> Vec<Value> {
    let mut parts: Vec<Value> = q
        .words
        .iter()
        .map(|w| json!({ "or": contains_word(w) }))
        .collect();
    if let Some(n) = q.number {
        let mut alts = contains_word(&n.to_string());
        alts.push(json!({ "number": { "eq": n } }));
        parts.push(json!({ "or": alts }));
    }
    if let Some((key, n)) = &q.identifier {
        // 팀 키 + 번호로 찾고, 그런 팀이 없을 때를 위해 텍스트로도 찾는다 (예: covid-19)
        let mut alts =
            vec![json!({ "team": { "key": { "eqIgnoreCase": key } }, "number": { "eq": n } })];
        alts.extend(contains_word(&format!("{key}-{n}")));
        parts.push(json!({ "or": alts }));
    }
    parts
}

fn token_parts(q: &ParsedQuery) -> Vec<Value> {
    let mut parts = Vec::new();
    match &q.state {
        Some(StateMatch::Type(t)) => parts.push(json!({ "state": { "type": { "eq": t } } })),
        Some(StateMatch::Name(n)) => {
            parts.push(json!({ "state": { "name": { "containsIgnoreCase": n } } }))
        }
        None => {}
    }
    if let Some(l) = &q.label {
        parts.push(json!({ "labels": { "some": { "name": { "containsIgnoreCase": l } } } }));
    }
    match &q.assignee {
        Some(AssigneeMatch::Me) => parts.push(json!({ "assignee": { "isMe": { "eq": true } } })),
        Some(AssigneeMatch::Name(n)) => parts.push(json!({ "assignee": { "or": [
            { "name": { "containsIgnoreCase": n } },
            { "displayName": { "containsIgnoreCase": n } }
        ] } })),
        None => {}
    }
    if let Some(k) = &q.team {
        parts.push(json!({ "team": { "key": { "eqIgnoreCase": k } } }));
    }
    if let Some(p) = q.priority {
        parts.push(json!({ "priority": { "eq": p } }));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::query::parse;

    #[test]
    fn empty_query_without_scope_is_empty_filter() {
        assert_eq!(build_issue_filter(&parse(""), &[]), json!({}));
    }

    #[test]
    fn scope_only() {
        assert_eq!(
            build_issue_filter(&parse(""), &["t1".to_string()]),
            json!({ "team": { "id": { "in": ["t1"] } } })
        );
    }

    #[test]
    fn each_word_matches_title_or_description() {
        assert_eq!(
            build_issue_filter(&parse("로그인 버그"), &[]),
            json!({ "and": [
                { "or": [ { "title": { "containsIgnoreCase": "로그인" } }, { "description": { "containsIgnoreCase": "로그인" } } ] },
                { "or": [ { "title": { "containsIgnoreCase": "버그" } }, { "description": { "containsIgnoreCase": "버그" } } ] }
            ] })
        );
    }

    #[test]
    fn number_also_matches_issue_number() {
        assert_eq!(
            build_issue_filter(&parse("131"), &[]),
            json!({ "or": [
                { "title": { "containsIgnoreCase": "131" } },
                { "description": { "containsIgnoreCase": "131" } },
                { "number": { "eq": 131 } }
            ] })
        );
    }

    #[test]
    fn identifier_matches_team_and_number_or_text() {
        assert_eq!(
            build_issue_filter(&parse("eng-7"), &[]),
            json!({ "or": [
                { "team": { "key": { "eqIgnoreCase": "ENG" } }, "number": { "eq": 7 } },
                { "title": { "containsIgnoreCase": "ENG-7" } },
                { "description": { "containsIgnoreCase": "ENG-7" } }
            ] })
        );
    }

    #[test]
    fn tokens_map_to_filters_and_team_overrides_scope() {
        assert_eq!(
            build_issue_filter(
                &parse("s:진행 l:bug @나 #OPS p:urgent"),
                &["t1".to_string()]
            ),
            json!({ "and": [
                { "state": { "type": { "eq": "started" } } },
                { "labels": { "some": { "name": { "containsIgnoreCase": "bug" } } } },
                { "assignee": { "isMe": { "eq": true } } },
                { "team": { "key": { "eqIgnoreCase": "OPS" } } },
                { "priority": { "eq": 1 } }
            ] })
        );
    }

    #[test]
    fn state_name_and_assignee_name() {
        assert_eq!(
            build_issue_filter(&parse("s:\"In Review\" @민수"), &[]),
            json!({ "and": [
                { "state": { "name": { "containsIgnoreCase": "In Review" } } },
                { "assignee": { "or": [
                    { "name": { "containsIgnoreCase": "민수" } },
                    { "displayName": { "containsIgnoreCase": "민수" } }
                ] } }
            ] })
        );
    }

    #[test]
    fn token_filter_ignores_text() {
        assert_eq!(token_filter(&parse("세션 만료")), None);
        assert_eq!(
            token_filter(&parse("세션 l:bug")),
            Some(json!({ "labels": { "some": { "name": { "containsIgnoreCase": "bug" } } } }))
        );
    }
}
