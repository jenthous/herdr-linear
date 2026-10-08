//! 목록 한 줄.

use ratatui::text::{Line, Span};

use super::style::{DIM, label_style, priority_span, state_icon, state_style, truncate};
use crate::linear::types::Issue;
use crate::markdown::sanitize;

/// 제목이 이보다 좁아지면 라벨과 담당자를 숨긴다.
const MIN_TITLE: usize = 12;

/// 목록 한 줄: 상태 아이콘(상태 색) · 식별자 · 우선순위 칸 · 제목 · 라벨(라벨 색) · @담당자.
/// 폭이 모자라면 라벨·담당자를 먼저 빼고, 그래도 넘치면 제목을 `…`로 줄인다. 우선순위 칸은 빼지 않는다.
pub fn issue_row(issue: &Issue, width: u16) -> Line<'static> {
    let width = usize::from(width);
    let mut spans = vec![
        Span::styled(
            format!("{} ", state_icon(&issue.state.state_type)),
            state_style(&issue.state),
        ),
        Span::styled(format!("{:<9} ", sanitize(&issue.identifier)), DIM),
    ];
    spans.push(priority_span(issue.priority));
    spans.push(Span::raw(" "));
    let fixed: usize = spans.iter().map(Span::width).sum();
    let mut tail: Vec<Span<'static>> = Vec::new();
    for (i, label) in issue.labels.nodes.iter().enumerate() {
        tail.push(Span::raw(if i == 0 { "  " } else { " " }));
        tail.push(Span::styled(sanitize(&label.name), label_style(label)));
    }
    if let Some(a) = &issue.assignee {
        tail.push(Span::styled(
            format!("  @{}", sanitize(&a.display_name)),
            DIM,
        ));
    }
    let tail_width: usize = tail.iter().map(Span::width).sum();
    let title = sanitize(&issue.title);
    let (room, tail) = if fixed + MIN_TITLE + tail_width <= width {
        (width - fixed - tail_width, tail)
    } else {
        (width.saturating_sub(fixed), Vec::new())
    };
    spans.push(Span::raw(truncate(&title, room)));
    spans.extend(tail);
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::to_plain;
    use crate::test_support::IssueBuilder;
    use ratatui::style::Color;

    fn issue() -> Issue {
        IssueBuilder::new("i1", "UP-1812", "데이터 손상 수정")
            .state("In Progress", "started")
            .labels(&["Bug", "Backend"])
            .assignee("u1", "jhhan")
            .build()
    }

    #[test]
    fn row_shows_colored_icon_labels_and_assignee() {
        let line = issue_row(&issue(), 80);
        assert_eq!(
            to_plain(std::slice::from_ref(&line)),
            "◐ UP-1812     데이터 손상 수정  Bug Backend  @jhhan"
        );
        assert_eq!(line.spans[0].style.fg, Some(Color::Rgb(94, 106, 210)));
        let bug = line.spans.iter().find(|s| s.content == "Bug").unwrap();
        assert_eq!(bug.style.fg, Some(Color::Rgb(235, 87, 87)));
    }

    #[test]
    fn narrow_row_drops_tail_then_truncates_title() {
        let line = issue_row(&issue(), 24);
        let text = to_plain(std::slice::from_ref(&line));
        assert!(!text.contains("Bug"), "{text}");
        assert!(line.width() <= 24, "{text}");
        assert!(text.ends_with('…'), "{text}");
    }

    #[test]
    fn row_strips_control_characters() {
        let i = IssueBuilder::new("i1", "UP-1", "제목\u{1b}[2J").build();
        let text = to_plain(&[issue_row(&i, 80)]);
        assert!(!text.contains('\u{1b}'), "{text:?}");
        let mut v = IssueBuilder::new("i2", "UP-2", "제목").json();
        v["identifier"] = serde_json::json!("UP-2\u{1b}]0;x\u{7}");
        let i: Issue = serde_json::from_value(v).unwrap();
        let text = to_plain(&[issue_row(&i, 80)]);
        assert!(!text.chars().any(char::is_control), "{text:?}");
    }

    #[test]
    fn priority_mark_sits_between_identifier_and_title() {
        let row = |p: i64| {
            issue_row(
                &IssueBuilder::new("i1", "ENG-1", "제목").priority(p).build(),
                80,
            )
        };
        let text = |p: i64| to_plain(&[row(p)]);
        assert_eq!(text(1), "○ ENG-1     ! 제목");
        assert_eq!(text(2), "○ ENG-1     ▆ 제목");
        assert_eq!(text(4), "○ ENG-1     ▂ 제목", "낮음은 가장 낮은 막대");
        assert_eq!(text(0), "○ ENG-1       제목");
        let urgent = row(1);
        let bang = urgent.spans.iter().find(|s| s.content == "!").unwrap();
        assert_eq!(bang.style.fg, Some(Color::Red));
    }

    #[test]
    fn narrow_row_keeps_the_priority_mark() {
        let i = IssueBuilder::new("i1", "UP-1812", "데이터 손상 수정")
            .priority(2)
            .labels(&["Bug"])
            .build();
        for width in [24, 16] {
            let line = issue_row(&i, width);
            let text = to_plain(std::slice::from_ref(&line));
            assert!(text.starts_with("○ UP-1812   ▆ "), "{text}");
            assert!(!text.contains("Bug"), "{text}");
            assert!(line.width() <= usize::from(width), "{text}");
        }
    }
}
