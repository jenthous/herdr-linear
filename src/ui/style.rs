//! Linear의 색과 상태를 터미널 스타일로 바꾼다.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::i18n::t;
use crate::linear::types::{LabelRef, StateRef};

/// 흐린 글자 (식별자, 담당자, 보조 정보)
pub const DIM: Style = Style::new().fg(Color::DarkGray);
/// 강조 (활성 탭, 제목)
pub const ACCENT: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);

/// `#rrggbb` 또는 `#rgb`를 RGB 색으로 바꾼다. 형식이 틀리면 `None`.
pub fn hex_color(s: &str) -> Option<Color> {
    let h = s.trim().strip_prefix('#')?;
    if !h.is_ascii() {
        return None;
    }
    let byte = |i: usize, n: usize| u8::from_str_radix(&h[i..i + n], 16).ok();
    match h.len() {
        6 => Some(Color::Rgb(byte(0, 2)?, byte(2, 2)?, byte(4, 2)?)),
        3 => Some(Color::Rgb(
            byte(0, 1)? * 17,
            byte(1, 1)? * 17,
            byte(2, 1)? * 17,
        )),
        _ => None,
    }
}

/// 상태 타입별 아이콘.
pub fn state_icon(state_type: &str) -> &'static str {
    match state_type {
        "triage" => "◇",
        "backlog" => "◌",
        "unstarted" => "○",
        "started" => "◐",
        "completed" => "●",
        "canceled" | "duplicate" => "✕",
        _ => "·",
    }
}

/// 상태 아이콘 스타일: Linear에 설정된 상태 색. 색이 없거나 틀리면 타입별 기본색.
pub fn state_style(state: &StateRef) -> Style {
    let fallback = match state.state_type.as_str() {
        "started" => Color::Yellow,
        "completed" => Color::Green,
        "canceled" | "duplicate" => Color::DarkGray,
        _ => Color::Gray,
    };
    Style::new().fg(hex_color(&state.color).unwrap_or(fallback))
}

/// 라벨 스타일: Linear에 설정된 라벨 색.
pub fn label_style(label: &LabelRef) -> Style {
    Style::new().fg(hex_color(&label.color).unwrap_or(Color::Gray))
}

/// 우선순위 이름.
pub fn priority_label(p: i64) -> &'static str {
    let t = t();
    match p {
        1 => t.priority_urgent,
        2 => t.priority_high,
        3 => t.priority_medium,
        4 => t.priority_low,
        _ => t.priority_none,
    }
}

/// 우선순위 라벨(2칸). 흔히 쓰는 표기대로 숫자가 작을수록 급하다.
/// 긴급 P0, 높음 P1, 보통 P2, 낮음 P3, 없음·모르는 값은 빈칸.
fn priority_mark(p: i64) -> &'static str {
    match p {
        1 => "P0",
        2 => "P1",
        3 => "P2",
        4 => "P3",
        _ => "  ",
    }
}

/// 목록 줄의 우선순위 칸(2칸). 긴급은 빨간 굵은 P0, 높음은 주황 P1, 보통은 노랑 P2, 낮음은 회색 P3.
pub fn priority_span(p: i64) -> Span<'static> {
    let style = match p {
        1 => Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        2 => Style::new().fg(Color::Rgb(242, 153, 74)),
        3 => Style::new().fg(Color::Rgb(242, 201, 76)),
        4 => Style::new().fg(Color::Rgb(150, 150, 150)),
        _ => Style::new(),
    };
    Span::styled(priority_mark(p), style)
}

/// 색 없는 출력의 우선순위 칸(2칸).
pub fn priority_text(p: i64) -> String {
    priority_mark(p).to_string()
}

/// "방금", "5분 전", "3시간 전", "2일 전" (지금 언어로).
pub fn ago(now_ms: i64, then_ms: i64) -> String {
    let t = t();
    let mins = (now_ms - then_ms).max(0) / 60_000;
    match mins {
        0 => t.just_now.to_string(),
        m if m < 60 => (t.minutes_ago)(m),
        m if m < 60 * 24 => (t.hours_ago)(m / 60),
        m => (t.days_ago)(m / (60 * 24)),
    }
}

/// RFC 3339 시각을 로컬 시간 "YYYY-MM-DD HH:MM"으로. 해석하지 못하면 그대로.
pub fn local_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| rfc3339.to_string())
}

/// 표시 폭 `width`에 맞게 자른다. 넘치면 끝을 `…`로 바꾼다.
pub fn truncate(s: &str, width: usize) -> String {
    if s.width() <= width {
        return s.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    let mut out = out.trim_end().to_string();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Lang, with_lang};
    use crate::test_support::IssueBuilder;

    #[test]
    fn hex_colors_parse() {
        assert_eq!(hex_color("#5e6ad2"), Some(Color::Rgb(94, 106, 210)));
        assert_eq!(hex_color(" #ABC "), Some(Color::Rgb(170, 187, 204)));
        for bad in ["5e6ad2", "#zzzzzz", "#12", "#가나다", "", "#1234567"] {
            assert_eq!(hex_color(bad), None, "{bad}");
        }
    }

    #[test]
    fn state_style_uses_linear_color_with_fallback() {
        let issue = IssueBuilder::new("i", "ENG-1", "t")
            .state("Doing", "started")
            .build();
        assert_eq!(state_style(&issue.state).fg, Some(Color::Rgb(94, 106, 210)));
        let mut broken = issue.state.clone();
        broken.color = "blue".into();
        assert_eq!(state_style(&broken).fg, Some(Color::Yellow));
    }

    #[test]
    fn truncate_by_display_width() {
        assert_eq!(truncate("로그인 버튼", 20), "로그인 버튼");
        assert_eq!(truncate("로그인 버튼 비활성", 8), "로그인…");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 0), "");
        assert!(truncate("가나다라마바", 7).width() <= 7);
    }

    #[test]
    fn priority_label_puts_the_most_urgent_first() {
        assert_eq!(
            priority_span(1),
            Span::styled(
                "P0",
                Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
            )
        );
        assert_eq!(
            priority_span(2),
            Span::styled("P1", Style::new().fg(Color::Rgb(242, 153, 74)))
        );
        assert_eq!(
            priority_span(3),
            Span::styled("P2", Style::new().fg(Color::Rgb(242, 201, 76)))
        );
        assert_eq!(
            priority_span(4),
            Span::styled("P3", Style::new().fg(Color::Rgb(150, 150, 150)))
        );
        for p in [0, 5, -1] {
            assert_eq!(priority_span(p), Span::raw("  "), "없음은 빈칸: {p}");
        }
        for (p, label) in [
            (1, "P0"),
            (2, "P1"),
            (3, "P2"),
            (4, "P3"),
            (0, "  "),
            (-1, "  "),
        ] {
            assert_eq!(priority_text(p), label, "{p}");
        }
        for p in -1..=5 {
            assert_eq!(priority_span(p).width(), 2, "{p}");
            assert_eq!(priority_text(p).width(), 2, "{p}");
        }
    }

    #[test]
    fn ago_and_priority_follow_the_language() {
        let now = 10 * 24 * 60 * 60_000;
        assert_eq!(ago(now, now), "방금");
        assert_eq!(priority_label(1), "긴급");
        with_lang(Lang::En, || {
            assert_eq!(ago(now, now - 30_000), "just now");
            assert_eq!(ago(now, now - 5 * 60_000), "5m ago");
            assert_eq!(ago(now, now - 3 * 60 * 60_000), "3h ago");
            assert_eq!(ago(now, now - 2 * 24 * 60 * 60_000), "2d ago");
            assert_eq!(priority_label(1), "Urgent");
            assert_eq!(priority_label(0), "None");
        });
    }
}
