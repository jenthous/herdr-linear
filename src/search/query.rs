//! 검색어 해석: 자유 텍스트 + 필터 토큰(s: l: @ # p:).

use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateMatch {
    /// 상태 타입 (triage, backlog, unstarted, started, completed, canceled)
    Type(&'static str),
    /// 상태 이름 일부
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssigneeMatch {
    Me,
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedQuery {
    /// 자유 텍스트 단어 (따옴표로 묶으면 공백 포함 한 단어)
    pub words: Vec<String>,
    /// 숫자만 있는 단어 (이슈 번호)
    pub number: Option<i64>,
    /// `ABC-123` 형식 (팀 키는 대문자)
    pub identifier: Option<(String, i64)>,
    pub state: Option<StateMatch>,
    pub label: Option<String>,
    pub assignee: Option<AssigneeMatch>,
    /// 팀 키 (대문자)
    pub team: Option<String>,
    /// 0 없음 … 4 낮음
    pub priority: Option<i64>,
}

impl ParsedQuery {
    /// 텍스트 조건(단어·번호·식별자)이 있는지.
    pub fn has_text(&self) -> bool {
        !self.words.is_empty() || self.number.is_some() || self.identifier.is_some()
    }

    pub fn is_empty(&self) -> bool {
        *self == ParsedQuery::default()
    }

    /// 깊은 검색에 보낼 텍스트.
    pub fn text(&self) -> String {
        let mut parts = self.words.clone();
        if let Some((key, n)) = &self.identifier {
            parts.push(format!("{key}-{n}"));
        }
        if let Some(n) = self.number {
            parts.push(n.to_string());
        }
        parts.join(" ")
    }
}

/// 검색어를 해석한다. 같은 종류의 토큰이 여러 번 나오면 마지막 것을 쓴다.
pub fn parse(input: &str) -> ParsedQuery {
    let normalized: String = input.nfc().collect();
    let mut q = ParsedQuery::default();
    for token in tokenize(&normalized) {
        if let Some(v) = strip_prefix_ci(&token, "s:") {
            if let Some(t) = state_type_keyword(v) {
                q.state = Some(StateMatch::Type(t));
            } else if !v.is_empty() {
                q.state = Some(StateMatch::Name(v.to_string()));
            }
        } else if let Some(v) = strip_prefix_ci(&token, "l:") {
            if !v.is_empty() {
                q.label = Some(v.to_string());
            }
        } else if let Some(v) = strip_prefix_ci(&token, "p:") {
            // 알 수 없는 우선순위 값은 무시한다
            if let Some(p) = priority_keyword(v) {
                q.priority = Some(p);
            }
        } else if let Some(v) = token.strip_prefix('@').filter(|v| !v.is_empty()) {
            let lower = v.to_lowercase();
            q.assignee = Some(if lower == "나" || lower == "me" {
                AssigneeMatch::Me
            } else {
                AssigneeMatch::Name(v.to_string())
            });
        } else if let Some(v) = token.strip_prefix('#').filter(|v| !v.is_empty()) {
            q.team = Some(v.to_uppercase());
        } else if let Some(id) = parse_identifier(&token) {
            q.identifier = Some(id);
        } else if token.chars().all(|c| c.is_ascii_digit()) {
            match token.parse::<i64>() {
                Ok(n) => q.number = Some(n),
                Err(_) => q.words.push(token),
            }
        } else {
            q.words.push(token);
        }
    }
    q
}

/// 공백으로 나누되 큰따옴표 안의 공백은 유지한다. 따옴표 자체는 지운다.
fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in s.chars() {
        match c {
            '"' => in_quote = !in_quote,
            c if c.is_whitespace() && !in_quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let n = prefix.len();
    (s.len() >= n && s.is_char_boundary(n) && s[..n].eq_ignore_ascii_case(prefix)).then(|| &s[n..])
}

/// `ABC-123` 형식이면 (팀 키 대문자, 번호).
pub fn parse_identifier(s: &str) -> Option<(String, i64)> {
    let (key, num) = s.split_once('-')?;
    let mut chars = key.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() || !chars.all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((key.to_uppercase(), num.parse().ok()?))
}

/// 상태 타입 키워드 (한글 포함).
pub fn state_type_keyword(v: &str) -> Option<&'static str> {
    match v.to_lowercase().as_str() {
        "triage" | "분류" => Some("triage"),
        "backlog" | "백로그" => Some("backlog"),
        "unstarted" | "todo" | "할일" => Some("unstarted"),
        "started" | "progress" | "진행" => Some("started"),
        "completed" | "done" | "완료" => Some("completed"),
        "canceled" | "취소" => Some("canceled"),
        _ => None,
    }
}

/// 우선순위 키워드 → 0~4.
pub fn priority_keyword(v: &str) -> Option<i64> {
    match v.to_lowercase().as_str() {
        "none" | "없음" | "0" => Some(0),
        "urgent" | "긴급" | "1" => Some(1),
        "high" | "높음" | "2" => Some(2),
        "medium" | "보통" | "3" => Some(3),
        "low" | "낮음" | "4" => Some(4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_text_words() {
        let q = parse("로그인  버그");
        assert_eq!(q.words, vec!["로그인", "버그"]);
        assert!(q.has_text());
    }

    #[test]
    fn all_tokens_together() {
        let q = parse("로그인 l:bug s:진행 @나 #eng p:high");
        assert_eq!(q.words, vec!["로그인"]);
        assert_eq!(q.label.as_deref(), Some("bug"));
        assert_eq!(q.state, Some(StateMatch::Type("started")));
        assert_eq!(q.assignee, Some(AssigneeMatch::Me));
        assert_eq!(q.team.as_deref(), Some("ENG"));
        assert_eq!(q.priority, Some(2));
    }

    #[test]
    fn quoted_values_keep_spaces() {
        let q = parse("s:\"In Review\" \"로그인 버튼\"");
        assert_eq!(q.state, Some(StateMatch::Name("In Review".into())));
        assert_eq!(q.words, vec!["로그인 버튼"]);
    }

    #[test]
    fn identifier_and_number() {
        let q = parse("eng-131 42");
        assert_eq!(q.identifier, Some(("ENG".into(), 131)));
        assert_eq!(q.number, Some(42));
        assert_eq!(q.text(), "ENG-131 42");
    }

    #[test]
    fn prefixes_are_case_insensitive_and_names_kept() {
        let q = parse("S:done L:Frontend @민수 P:긴급");
        assert_eq!(q.state, Some(StateMatch::Type("completed")));
        assert_eq!(q.label.as_deref(), Some("Frontend"));
        assert_eq!(q.assignee, Some(AssigneeMatch::Name("민수".into())));
        assert_eq!(q.priority, Some(1));
    }

    #[test]
    fn unknown_priority_and_empty_tokens_are_ignored() {
        let q = parse("p:soon l: s: @ #");
        assert_eq!(q.priority, None);
        assert_eq!(q.label, None);
        assert_eq!(q.state, None);
        assert_eq!(q.words, vec!["@", "#"]);
    }

    #[test]
    fn decomposed_korean_is_normalized() {
        let nfd: String = "로그인".nfd().collect();
        assert_eq!(parse(&nfd).words, vec!["로그인"]);
    }

    #[test]
    fn empty_query() {
        assert!(parse("   ").is_empty());
        assert!(!parse("l:bug").has_text());
    }

    #[test]
    fn identifier_rules() {
        assert_eq!(parse_identifier("ABC-12"), Some(("ABC".into(), 12)));
        assert_eq!(parse_identifier("a1-3"), Some(("A1".into(), 3)));
        assert_eq!(parse_identifier("1A-3"), None);
        assert_eq!(parse_identifier("ABC-"), None);
        assert_eq!(parse_identifier("ABC-1x"), None);
        assert_eq!(parse_identifier("로그-1"), None);
    }
}
