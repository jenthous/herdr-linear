//! 상세 화면의 관계 칸: 상위·막힘·막는 중·관련·하위. TUI 상세, 관계 메뉴, CLI가 함께 쓴다.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::style::{DIM, state_icon, state_style, truncate};
use crate::i18n::t;
use crate::linear::types::{IssueRelations, ParentRef, RelatedIssue, StateRef};
use crate::markdown::sanitize;

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
    pub const ALL: [RelKind; 5] = [
        RelKind::Parent,
        RelKind::BlockedBy,
        RelKind::Blocking,
        RelKind::Related,
        RelKind::Child,
    ];

    pub fn label(self) -> &'static str {
        let t = t();
        match self {
            RelKind::Parent => t.rel_parent,
            RelKind::BlockedBy => t.rel_blocked_by,
            RelKind::Blocking => t.rel_blocking,
            RelKind::Related => t.rel_related,
            RelKind::Child => t.rel_child,
        }
    }
}

/// 칸 이름 폭: 지금 언어의 칸 이름 중 가장 넓은 것 + 1칸. 한국어는 `막는 중`(7칸)이라 8칸이다.
pub fn kind_width() -> usize {
    RelKind::ALL
        .iter()
        .map(|k| k.label().width())
        .max()
        .unwrap_or(0)
        + 1
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

/// 관계 한 줄: 칸 이름(`kind_width`칸) · 상태 아이콘 · 식별자 · 제목.
/// `show_kind`가 false면 칸 이름 자리를 비운다. 제목은 `width`에 맞춰 자르고, 자리가 없으면 뺀다.
pub fn row_line(row: &RelRow, show_kind: bool, width: u16) -> Line<'static> {
    let kind_width = kind_width();
    let kind = if show_kind {
        pad(row.kind.label(), kind_width)
    } else {
        " ".repeat(kind_width)
    };
    let icon = match &row.state {
        Some(s) => Span::styled(format!("{} ", state_icon(&s.state_type)), state_style(s)),
        None => Span::raw("  "),
    };
    let id = Span::styled(format!("{:<9} ", sanitize(&row.identifier)), DIM);
    let used = kind_width + icon.width() + id.width();
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

/// `하위    4개 중 2개 남음` / `하위    4개 모두 끝남` / `하위    50개 넘음` (지금 언어로).
fn children_summary(rows: &[RelRow], relations: Option<&IssueRelations>) -> Line<'static> {
    let t = t();
    let kids: Vec<&RelRow> = rows.iter().filter(|r| r.kind == RelKind::Child).collect();
    let total = kids.len();
    let left = kids.iter().filter(|r| !r.finished()).count();
    let mut spans = vec![Span::styled(pad(RelKind::Child.label(), kind_width()), DIM)];
    if relations.is_some_and(|r| r.more_children) {
        spans.push(Span::styled((t.children_over)(total), DIM));
    } else if left == 0 {
        spans.push(Span::styled(
            (t.children_all_done)(total),
            Style::new().fg(Color::Green),
        ));
    } else {
        // 어순이 언어마다 달라서 (흐리게 그릴지, 글자) 조각을 카탈로그가 정한다
        for (dim, text) in (t.children_left)(total, left) {
            spans.push(if dim {
                Span::styled(text, DIM)
            } else {
                Span::raw(text)
            });
        }
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
            format!("{}{}", " ".repeat(kind_width()), t().more_children),
            DIM,
        )));
        map.push(None);
    }
    (out, map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Lang, with_lang};
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
        assert_eq!(
            fg(&out[1], 0),
            Some(Color::Red),
            "안 끝난 막는 이슈가 있으면 막힘은 빨강"
        );
        assert_eq!(fg(&out[3], 0), Some(Color::Yellow), "막는 중은 노랑");
        assert_eq!(fg(&out[0], 0), DIM.fg, "상위 칸 이름은 흐리게");
        assert_eq!(
            fg(&out[1], 1),
            Some(Color::Rgb(94, 106, 210)),
            "아이콘은 Linear 상태 색"
        );
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
        assert_eq!(
            fg(&out[0], 0),
            DIM.fg,
            "막는 이슈가 다 끝나면 막힘도 흐리게"
        );
        assert_eq!(text(&out)[1], "하위    1개 모두 끝남");
        assert_eq!(
            out[1].spans[1].style.fg,
            Some(Color::Green),
            "모두 끝나면 초록"
        );
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
        assert!(
            !rows(Some(&p), None)[0].finished(),
            "상태를 모르면 안 끝난 것으로 본다"
        );
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
        assert_eq!(
            rows(None, Some(&sample()))[0].filter_text(),
            "막힘 ENG-140 API 스키마 확정"
        );
    }

    #[test]
    fn kind_width_fits_every_language() {
        assert_eq!(kind_width(), 8);
        assert_eq!(with_lang(Lang::En, kind_width), "Blocked by".width() + 1);
        for lang in Lang::ALL {
            with_lang(lang, || {
                let w = kind_width();
                for k in RelKind::ALL {
                    assert!(k.label().width() < w, "{lang:?} {k:?}");
                }
            });
        }
    }

    #[test]
    fn english_relations_use_english_labels_and_summary() {
        let (out, _) = with_lang(Lang::En, || lines(Some(&parent()), Some(&sample()), 80));
        let got = text(&out);
        // "Blocked by"(10칸)가 가장 넓어서 칸 이름은 11칸이다
        assert!(got[0].starts_with("Parent     "), "{got:?}");
        assert!(got.iter().any(|l| l.starts_with("Blocked by ")), "{got:?}");
        assert!(
            got.iter().any(|l| l == "Sub-issues 2 open · 4 total"),
            "{got:?}"
        );
        let mut more = sample();
        more.more_children = true;
        let (out, _) = with_lang(Lang::En, || lines(None, Some(&more), 80));
        let got = text(&out);
        assert!(got.iter().any(|l| l == "Sub-issues over 4"), "{got:?}");
        assert_eq!(
            got.last().unwrap(),
            &format!("{}… more (o to view in the browser)", " ".repeat(11))
        );
    }
}
