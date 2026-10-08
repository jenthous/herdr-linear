//! Linear의 색과 상태를 터미널 스타일로 바꾼다.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
    match p {
        1 => "긴급",
        2 => "높음",
        3 => "보통",
        4 => "낮음",
        _ => "없음",
    }
}

/// 우선순위 표시(1칸). 막대가 높을수록 우선순위가 높다. 긴급은 `!`, 없음·모르는 값은 빈칸.
fn priority_mark(p: i64) -> &'static str {
    match p {
        1 => "!",
        2 => "▆",
        3 => "▄",
        4 => "▂",
        _ => " ",
    }
}

/// 목록 줄의 우선순위 칸(1칸). 긴급은 빨간 굵은 `!`.
pub fn priority_span(p: i64) -> Span<'static> {
    let mark = priority_mark(p);
    if p == 1 {
        Span::styled(
            mark,
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw(mark)
    }
}

/// 색 없는 출력의 우선순위 칸(1칸).
pub fn priority_text(p: i64) -> String {
    priority_mark(p).to_string()
}

/// "방금", "5분 전", "3시간 전", "2일 전".
pub fn ago(now_ms: i64, then_ms: i64) -> String {
    let mins = (now_ms - then_ms).max(0) / 60_000;
    match mins {
        0 => "방금".to_string(),
        m if m < 60 => format!("{m}분 전"),
        m if m < 60 * 24 => format!("{}시간 전", m / 60),
        m => format!("{}일 전", m / (60 * 24)),
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
    fn priority_mark_is_one_column_and_taller_for_higher_priority() {
        let red = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
        assert_eq!(priority_span(1), Span::styled("!", red));
        assert_eq!(priority_text(1), "!");
        for (p, mark) in [(2, "▆"), (3, "▄"), (4, "▂"), (0, " "), (5, " "), (-1, " ")] {
            assert_eq!(priority_span(p), Span::raw(mark), "{p}");
            assert_eq!(priority_text(p), mark, "{p}");
        }
        for p in -1..=5 {
            assert_eq!(priority_span(p).width(), 1, "{p}");
            assert_eq!(priority_text(p).width(), 1, "{p}");
        }
    }
}
