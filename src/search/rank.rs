//! 로컬 검색: 캐시된 이슈를 즉시 거르고 순위를 매긴다.

use std::cmp::Ordering;
use std::collections::HashSet;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use unicode_normalization::UnicodeNormalization;

use super::query::{AssigneeMatch, ParsedQuery, StateMatch};
use crate::linear::types::Issue;

/// 상태 타입 정렬 순서. 작을수록 위.
pub fn state_order(state_type: &str) -> u8 {
    match state_type {
        "started" => 0,
        "unstarted" => 1,
        "backlog" => 2,
        "triage" => 3,
        "completed" => 4,
        "canceled" => 5,
        "duplicate" => 6,
        _ => 7,
    }
}

/// 우선순위 정렬 순서: 긴급(1) → 낮음(4) → 없음(0).
pub fn priority_order(priority: i64) -> i64 {
    if priority == 0 { 5 } else { priority }
}

/// "내 이슈" 정렬: 상태 → 우선순위 → 최근 수정.
pub fn sort_mine(issues: &mut [Issue]) {
    issues.sort_by(|a, b| {
        state_order(&a.state.state_type)
            .cmp(&state_order(&b.state.state_type))
            .then_with(|| priority_order(a.priority).cmp(&priority_order(b.priority)))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
}

fn norm(s: &str) -> String {
    s.nfc().collect::<String>().to_lowercase()
}

struct Entry {
    issue: Issue,
    /// "식별자 제목" (소문자, NFC)
    hay: String,
    /// 본문 (소문자, NFC)
    desc: String,
}

impl Entry {
    fn new(issue: Issue) -> Entry {
        let hay = format!("{} {}", issue.identifier.to_lowercase(), norm(&issue.title));
        let desc = norm(issue.description.as_deref().unwrap_or(""));
        Entry { issue, hay, desc }
    }
}

/// 캐시된 이슈의 검색 색인. 문자열 정규화를 미리 해 둔다.
pub struct SearchIndex {
    entries: Vec<Entry>,
}

impl SearchIndex {
    pub fn new(issues: Vec<Issue>) -> SearchIndex {
        SearchIndex {
            entries: issues.into_iter().map(Entry::new).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 검색어로 거르고 순위대로 돌려준다.
    pub fn search(&self, q: &ParsedQuery, viewer_id: Option<&str>) -> Vec<Issue> {
        rank(&self.entries, &HashSet::new(), q, viewer_id)
    }
}

/// 로컬 결과와 서버 결과를 합친다. 같은 id면 서버 쪽(최신)을 쓴다.
/// 서버 결과는 서버가 조건을 확인했으므로 로컬 텍스트 매칭에 실패해도 남긴다.
pub fn merge(
    local: &[Issue],
    server: &[Issue],
    q: &ParsedQuery,
    viewer_id: Option<&str>,
) -> Vec<Issue> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for issue in server.iter().chain(local) {
        if seen.insert(issue.id.clone()) {
            entries.push(Entry::new(issue.clone()));
        }
    }
    let keep: HashSet<String> = server.iter().map(|i| i.id.clone()).collect();
    rank(&entries, &keep, q, viewer_id)
}

#[derive(Debug, Clone, Copy)]
struct Rank {
    /// 0 식별자·번호 일치, 1 제목 퍼지, 2 본문 포함, 3 텍스트 조건 없음
    tier: u8,
    score: u32,
}

fn rank(
    entries: &[Entry],
    keep: &HashSet<String>,
    q: &ParsedQuery,
    viewer_id: Option<&str>,
) -> Vec<Issue> {
    let terms = text_terms(q);
    let pattern = (!terms.is_empty())
        .then(|| Pattern::parse(&terms.join(" "), CaseMatching::Ignore, Normalization::Smart));
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Vec::new();
    let mut scored: Vec<(Rank, &Issue)> = Vec::new();
    for e in entries {
        let kept = keep.contains(&e.issue.id);
        if !kept && !passes_filters(&e.issue, q, viewer_id) {
            continue;
        }
        let rank = if !q.has_text() {
            Rank { tier: 3, score: 0 }
        } else if exact_match(&e.issue, q) {
            Rank { tier: 0, score: 0 }
        } else if let Some(score) = pattern
            .as_ref()
            .and_then(|p| p.score(Utf32Str::new(&e.hay, &mut buf), &mut matcher))
        {
            Rank { tier: 1, score }
        } else if terms.iter().all(|t| e.desc.contains(t.as_str())) || kept {
            Rank { tier: 2, score: 0 }
        } else {
            continue;
        };
        scored.push((rank, &e.issue));
    }
    scored.sort_by(|a, b| compare(a.0, a.1, b.0, b.1));
    scored.into_iter().map(|(_, issue)| issue.clone()).collect()
}

fn text_terms(q: &ParsedQuery) -> Vec<String> {
    let mut terms: Vec<String> = q.words.iter().map(|w| norm(w)).collect();
    if let Some((key, n)) = &q.identifier {
        terms.push(format!("{}-{n}", key.to_lowercase()));
    }
    if let Some(n) = q.number {
        terms.push(n.to_string());
    }
    terms
}

fn exact_match(issue: &Issue, q: &ParsedQuery) -> bool {
    if let Some((key, n)) = &q.identifier
        && issue.team.key.eq_ignore_ascii_case(key)
        && issue.number == *n
    {
        return true;
    }
    q.number == Some(issue.number)
}

fn passes_filters(issue: &Issue, q: &ParsedQuery, viewer_id: Option<&str>) -> bool {
    if let Some(state) = &q.state {
        let ok = match state {
            StateMatch::Type(t) => issue.state.state_type == *t,
            StateMatch::Name(n) => norm(&issue.state.name).contains(&norm(n)),
        };
        if !ok {
            return false;
        }
    }
    if let Some(label) = &q.label {
        let label = norm(label);
        if !issue
            .labels
            .nodes
            .iter()
            .any(|l| norm(&l.name).contains(&label))
        {
            return false;
        }
    }
    match &q.assignee {
        Some(AssigneeMatch::Me) => {
            let mine =
                viewer_id.is_some() && issue.assignee.as_ref().map(|a| a.id.as_str()) == viewer_id;
            if !mine {
                return false;
            }
        }
        Some(AssigneeMatch::Name(name)) => {
            let name = norm(name);
            let ok = issue.assignee.as_ref().is_some_and(|a| {
                norm(&a.name).contains(&name) || norm(&a.display_name).contains(&name)
            });
            if !ok {
                return false;
            }
        }
        None => {}
    }
    if let Some(key) = &q.team
        && !issue.team.key.eq_ignore_ascii_case(key)
    {
        return false;
    }
    if let Some(p) = q.priority
        && issue.priority != p
    {
        return false;
    }
    true
}

fn compare(ra: Rank, a: &Issue, rb: Rank, b: &Issue) -> Ordering {
    ra.tier
        .cmp(&rb.tier)
        .then_with(|| rb.score.cmp(&ra.score))
        .then_with(|| state_order(&a.state.state_type).cmp(&state_order(&b.state.state_type)))
        .then_with(|| b.updated_at.cmp(&a.updated_at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::query::parse;
    use crate::test_support::IssueBuilder;

    fn ids(issues: &[Issue]) -> Vec<&str> {
        issues.iter().map(|i| i.identifier.as_str()).collect()
    }

    fn index() -> SearchIndex {
        SearchIndex::new(vec![
            IssueBuilder::new("a", "ENG-131", "로그인 버튼 비활성 버그")
                .state("In Progress", "started")
                .labels(&["bug", "frontend"])
                .assignee("me", "김민수")
                .priority(2)
                .build(),
            IssueBuilder::new("b", "ENG-140", "결제 페이지 리팩터링")
                .description("로그인 이후 결제 흐름 정리")
                .updated("2026-10-05T00:00:00.000Z")
                .build(),
            IssueBuilder::new("c", "OPS-21", "로그인 서버 모니터링")
                .state("Done", "completed")
                .assignee("u2", "이영희")
                .build(),
            IssueBuilder::new("d", "ENG-7", "다크 모드 색상 정리").build(),
        ])
    }

    #[test]
    fn identifier_and_number_come_first() {
        let r = index().search(&parse("eng-7"), None);
        assert_eq!(ids(&r)[0], "ENG-7");
        let r = index().search(&parse("131"), None);
        assert_eq!(ids(&r)[0], "ENG-131");
    }

    #[test]
    fn title_fuzzy_beats_body_and_body_matches_are_included() {
        let r = index().search(&parse("로그인"), None);
        // 제목 일치 (진행 중이 완료보다 위) → 본문 일치
        assert_eq!(ids(&r), vec!["ENG-131", "OPS-21", "ENG-140"]);
    }

    #[test]
    fn filters_apply_without_text() {
        let r = index().search(&parse("l:bug"), None);
        assert_eq!(ids(&r), vec!["ENG-131"]);
        let r = index().search(&parse("s:완료"), None);
        assert_eq!(ids(&r), vec!["OPS-21"]);
        let r = index().search(&parse("s:progress #eng"), None);
        assert_eq!(ids(&r), vec!["ENG-131"]);
        let r = index().search(&parse("p:high"), None);
        assert_eq!(ids(&r), vec!["ENG-131"]);
        let r = index().search(&parse("@영희"), None);
        assert_eq!(ids(&r), vec!["OPS-21"]);
    }

    #[test]
    fn me_needs_viewer_id() {
        assert!(index().search(&parse("@나"), None).is_empty());
        assert_eq!(
            ids(&index().search(&parse("@나"), Some("me"))),
            vec!["ENG-131"]
        );
    }

    #[test]
    fn empty_query_sorts_by_state_then_recent() {
        let r = index().search(&parse(""), None);
        assert_eq!(ids(&r), vec!["ENG-131", "ENG-140", "ENG-7", "OPS-21"]);
    }

    #[test]
    fn decomposed_titles_still_match() {
        let nfd: String = "로그인 문제".nfd().collect();
        let idx = SearchIndex::new(vec![IssueBuilder::new("x", "ENG-9", &nfd).build()]);
        assert_eq!(ids(&idx.search(&parse("로그인"), None)), vec!["ENG-9"]);
    }

    #[test]
    fn no_match_returns_nothing() {
        assert!(index().search(&parse("존재하지않는단어"), None).is_empty());
    }

    #[test]
    fn merge_prefers_server_copy_and_keeps_server_only_hits() {
        let local = vec![IssueBuilder::new("a", "ENG-131", "로그인 버튼").build()];
        let server = vec![
            IssueBuilder::new("a", "ENG-131", "로그인 버튼 (수정됨)").build(),
            // 코멘트에서만 일치한 결과: 로컬 텍스트 매칭은 실패하지만 남긴다
            IssueBuilder::new("z", "ENG-500", "알림 설정").build(),
        ];
        let r = merge(&local, &server, &parse("로그인"), None);
        assert_eq!(ids(&r), vec!["ENG-131", "ENG-500"]);
        assert_eq!(r[0].title, "로그인 버튼 (수정됨)");
    }

    #[test]
    fn sort_mine_orders_state_priority_recent() {
        let mut issues = vec![
            IssueBuilder::new("1", "ENG-1", "a").priority(0).build(),
            IssueBuilder::new("2", "ENG-2", "b")
                .state("Doing", "started")
                .priority(3)
                .build(),
            IssueBuilder::new("3", "ENG-3", "c").priority(1).build(),
            IssueBuilder::new("4", "ENG-4", "d")
                .state("Backlog", "backlog")
                .build(),
        ];
        sort_mine(&mut issues);
        assert_eq!(ids(&issues), vec!["ENG-2", "ENG-3", "ENG-1", "ENG-4"]);
    }

    /// 성능: 캐시 5천 건 기준 키 입력당 16ms 이내. `cargo test --release -- --ignored`로 실행한다.
    #[test]
    #[ignore]
    fn search_5000_under_16ms() {
        let issues: Vec<Issue> = (0..5000)
            .map(|i| {
                IssueBuilder::new(
                    &format!("id{i}"),
                    &format!("ENG-{i}"),
                    &format!("로그인 기능 {i} 개선 작업"),
                )
                .description("세션 만료 시 다시 로그인하도록 처리하고 결제 흐름을 정리한다")
                .build()
            })
            .collect();
        let idx = SearchIndex::new(issues);
        let q = parse("로그인 개선");
        let start = std::time::Instant::now();
        let r = idx.search(&q, None);
        let elapsed = start.elapsed();
        assert_eq!(r.len(), 5000);
        assert!(elapsed.as_millis() < 16, "{elapsed:?}");
    }
}
