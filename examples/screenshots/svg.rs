//! ratatui 화면 버퍼를 SVG로 바꾼다. 글자마다 x 좌표를 줘서 글꼴 폭과 상관없이 칸이 맞는다.

use std::collections::BTreeMap;
use std::fmt::Write;

use herdr_linear::i18n::Lang;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use unicode_width::UnicodeWidthStr;

/// 한 칸의 너비·높이(px)와 글자 기준선.
const CW: u32 = 9;
const CH: u32 = 20;
const BASELINE: u32 = 15;
const FONT_SIZE: u32 = 15;
/// 넓은 글자(한중일)는 두 칸을 채우도록 크게 그린다. 칸 폭은 글자 크기의 0.6배인데
/// 한중일 글꼴의 글자 폭은 1배라서, 그대로 두면 글자 사이가 벌어진다.
const WIDE_FONT_SIZE: u32 = 17;
const WIDE_BASELINE: u32 = 16;
/// 터미널 기본 배경과 글자색.
const BG: &str = "#1e1e2e";
const FG: &str = "#cdd6f4";
/// 목록에서 고른 줄을 가리키는 표시. 글꼴에 맡기지 않고 삼각형으로 그린다.
const MARKER: &str = "▶";

/// 그릴 칸 하나. 넓은 글자의 둘째 칸은 `sym`이 비어 있다.
#[derive(Clone)]
struct Cell {
    sym: String,
    fg: String,
    bg: Option<String>,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    dim: bool,
}

impl Cell {
    fn blank() -> Cell {
        Cell {
            sym: " ".to_string(),
            fg: FG.to_string(),
            bg: None,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            dim: false,
        }
    }

    /// 글자 스타일이 같아 한 `<text>`로 묶을 수 있는지.
    fn same_text_style(&self, other: &Cell) -> bool {
        self.fg == other.fg
            && self.bold == other.bold
            && self.italic == other.italic
            && self.underline == other.underline
            && self.strike == other.strike
            && self.dim == other.dim
    }

    /// 묶음을 시작할 수 있는 글자인지: 빈칸·넓은 글자의 둘째 칸·상자 글자·고른 줄 표시가 아니다.
    fn starts_text(&self) -> bool {
        !self.sym.is_empty()
            && self.sym != " "
            && self.sym != MARKER
            && box_arms(&self.sym).is_none()
    }
}

/// 버퍼들을 (가로 칸 위치, 버퍼)로 나란히 놓아 `w`×`h` 칸 SVG 하나로 만든다.
pub fn render(parts: &[(u16, Buffer)], w: u16, h: u16, lang: Lang) -> String {
    let rows = compose(parts, w, h);
    let (pw, ph) = (u32::from(w) * CW, u32::from(h) * CH);
    let mut s = String::new();
    // `<text>` 안에는 빈칸이 이어지지 않아서(`texts`) 빈칸 처리 설정은 필요 없다
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xml:lang="{}" width="{pw}" height="{ph}" viewBox="0 0 {pw} {ph}" font-family="{}" font-size="{FONT_SIZE}">"#,
        lang.code(),
        fonts(lang)
    );
    let _ = writeln!(
        s,
        r#"<rect width="{pw}" height="{ph}" rx="8" fill="{BG}"/>"#
    );
    backgrounds(&mut s, &rows);
    box_lines(&mut s, &rows);
    markers(&mut s, &rows);
    texts(&mut s, &rows);
    s.push_str("</svg>\n");
    s
}

/// 고정폭 글꼴 다음에 그 언어의 한중일 글꼴을 둔다 (한자 모양이 언어마다 다르다).
fn fonts(lang: Lang) -> String {
    let cjk = match lang {
        Lang::Ko => "'Apple SD Gothic Neo', 'Noto Sans CJK KR', 'Noto Sans KR', ",
        Lang::Ja => "'Hiragino Sans', 'Noto Sans CJK JP', 'Noto Sans JP', ",
        Lang::ZhCn => "'PingFang SC', 'Noto Sans CJK SC', 'Noto Sans SC', ",
        Lang::En | Lang::De => "",
    };
    format!("ui-monospace, 'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', {cjk}monospace")
}

/// 버퍼들을 한 화면(`h`줄 × `w`칸)으로 모은다. 넓은 글자의 둘째 칸은 첫 칸의 배경을 이어받는다
/// (ratatui는 둘째 칸의 스타일을 지운다).
fn compose(parts: &[(u16, Buffer)], w: u16, h: u16) -> Vec<Vec<Cell>> {
    let mut rows = vec![vec![Cell::blank(); usize::from(w)]; usize::from(h)];
    for (dx, buf) in parts {
        let area = buf.area;
        for y in 0..area.height.min(h) {
            let mut x = 0;
            while x < area.width {
                let src = &buf[(area.x + x, area.y + y)];
                let width = src.symbol().width().clamp(1, 2) as u16;
                let cell = convert(src);
                let col = usize::from(dx + x);
                let row = &mut rows[usize::from(y)];
                if col + 1 < row.len() && width == 2 {
                    row[col + 1] = Cell {
                        sym: String::new(),
                        bg: cell.bg.clone(),
                        ..Cell::blank()
                    };
                }
                if col < row.len() {
                    row[col] = cell;
                }
                x += width;
            }
        }
    }
    rows
}

fn convert(c: &ratatui::buffer::Cell) -> Cell {
    let m = c.modifier;
    let mut fg = color(c.fg).unwrap_or_else(|| FG.to_string());
    let mut bg = color(c.bg);
    if m.contains(Modifier::REVERSED) {
        let old_fg = fg;
        fg = bg.unwrap_or_else(|| BG.to_string());
        bg = Some(old_fg);
    }
    Cell {
        sym: c.symbol().to_string(),
        fg,
        bg,
        bold: m.contains(Modifier::BOLD),
        italic: m.contains(Modifier::ITALIC),
        underline: m.contains(Modifier::UNDERLINED),
        strike: m.contains(Modifier::CROSSED_OUT),
        dim: m.contains(Modifier::DIM),
    }
}

/// 터미널 색을 SVG 색으로. 기본색(`Reset`)과 256색 번호는 `None`(기본색을 쓴다).
fn color(c: Color) -> Option<String> {
    let hex = match c {
        Color::Reset | Color::Indexed(_) => return None,
        Color::Rgb(r, g, b) => return Some(format!("#{r:02x}{g:02x}{b:02x}")),
        Color::Black => "#45475a",
        Color::Red | Color::LightRed => "#f38ba8",
        Color::Green | Color::LightGreen => "#a6e3a1",
        Color::Yellow | Color::LightYellow => "#f9e2af",
        Color::Blue | Color::LightBlue => "#89b4fa",
        Color::Magenta | Color::LightMagenta => "#f5c2e7",
        Color::Cyan | Color::LightCyan => "#94e2d5",
        Color::Gray => "#bac2de",
        Color::DarkGray => "#6c7086",
        Color::White => "#ffffff",
    };
    Some(hex.to_string())
}

/// 배경색이 있는 칸을 가로로 이어 사각형으로 칠한다.
fn backgrounds(s: &mut String, rows: &[Vec<Cell>]) {
    for (y, row) in rows.iter().enumerate() {
        let mut x = 0;
        while x < row.len() {
            let Some(bg) = row[x].bg.clone() else {
                x += 1;
                continue;
            };
            let start = x;
            while x < row.len() && row[x].bg.as_deref() == Some(bg.as_str()) {
                x += 1;
            }
            let _ = writeln!(
                s,
                r#"<rect x="{}" y="{}" width="{}" height="{CH}" fill="{bg}"/>"#,
                start as u32 * CW,
                y as u32 * CH,
                (x - start) as u32 * CW
            );
        }
    }
}

/// 상자 그리기 글자가 칸 가운데에서 (왼쪽, 오른쪽, 위, 아래)로 뻗는지.
fn box_arms(sym: &str) -> Option<(bool, bool, bool, bool)> {
    Some(match sym {
        "─" => (true, true, false, false),
        "│" => (false, false, true, true),
        "┌" | "╭" => (false, true, false, true),
        "┐" | "╮" => (true, false, false, true),
        "└" | "╰" => (false, true, true, false),
        "┘" | "╯" => (true, false, true, false),
        "├" => (false, true, true, true),
        "┤" => (true, false, true, true),
        "┬" => (true, true, false, true),
        "┴" => (true, true, true, false),
        "┼" => (true, true, true, true),
        _ => return None,
    })
}

/// 상자 그리기 글자를 선으로 그린다. 글꼴로 그리면 줄 사이가 끊긴다. 색마다 `<path>` 하나다.
fn box_lines(s: &mut String, rows: &[Vec<Cell>]) {
    let mut paths: BTreeMap<String, String> = BTreeMap::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            let Some((left, right, up, down)) = box_arms(&cell.sym) else {
                continue;
            };
            let (x0, y0) = (x as u32 * CW, y as u32 * CH);
            let (cx, cy) = (x0 + CW / 2, y0 + CH / 2);
            let d = paths.entry(cell.fg.clone()).or_default();
            if left {
                let _ = write!(d, "M{x0} {cy}H{cx}");
            }
            if right {
                let _ = write!(d, "M{cx} {cy}H{}", x0 + CW);
            }
            if up {
                let _ = write!(d, "M{cx} {y0}V{cy}");
            }
            if down {
                let _ = write!(d, "M{cx} {cy}V{}", y0 + CH);
            }
        }
    }
    for (stroke, d) in paths {
        let _ = writeln!(
            s,
            r#"<path d="{d}" stroke="{stroke}" stroke-width="1" fill="none" shape-rendering="crispEdges"/>"#
        );
    }
}

/// 고른 줄 표시(`▶`)를 삼각형으로 그린다. 글자로 두면 글꼴에 따라 이모지(색 있는 그림)로 바뀐다.
/// 색마다 `<path>` 하나다.
fn markers(s: &mut String, rows: &[Vec<Cell>]) {
    let mut paths: BTreeMap<String, String> = BTreeMap::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            if cell.sym != MARKER {
                continue;
            }
            let (x0, y0) = (x as u32 * CW, y as u32 * CH);
            let d = paths.entry(cell.fg.clone()).or_default();
            let _ = write!(
                d,
                "M{} {}L{} {}L{} {}Z",
                x0 + 1,
                y0 + 5,
                x0 + CW - 1,
                y0 + CH / 2,
                x0 + 1,
                y0 + CH - 5
            );
        }
    }
    for (fill, d) in paths {
        let _ = writeln!(s, r#"<path d="{d}" fill="{fill}"/>"#);
    }
}

/// 글자를 그린다. 스타일이 같은 한 칸 글자는 한 `<text>`로 묶고 글자마다 x를 준다.
/// 묶음 안의 빈칸은 한 칸까지만 넣는다. 빈칸이 두 칸 이상이면 묶음을 나눈다.
/// 넓은 글자(한중일)와 여러 코드 포인트로 된 글자는 따로 그린다: librsvg 같은 그림 도구는 첫 x만 따르고
/// 나머지는 글꼴의 글자 폭대로 이어 그려서, 폭이 칸과 다른 글자 뒤의 글이 밀리기 때문이다.
fn texts(s: &mut String, rows: &[Vec<Cell>]) {
    for (y, row) in rows.iter().enumerate() {
        let mut x = 0;
        while x < row.len() {
            let first = &row[x];
            if !first.starts_text() {
                x += 1;
                continue;
            }
            let mut glyphs: Vec<(usize, &str)> = vec![(x, first.sym.as_str())];
            let mut k = x + 1;
            if flows(row, x) {
                // 같은 스타일의 한 칸 글자가 바로, 또는 빈칸 한 칸을 건너 이어지면 묶는다
                loop {
                    let gap = usize::from(row.get(k).is_some_and(|c| c.sym == " "));
                    let next = k + gap;
                    if next >= row.len() || !flows(row, next) || !row[next].same_text_style(first) {
                        break;
                    }
                    if gap == 1 {
                        glyphs.push((k, " "));
                    }
                    glyphs.push((next, row[next].sym.as_str()));
                    k = next + 1;
                }
            }
            let xs: Vec<String> = glyphs
                .iter()
                .map(|(col, _)| (*col as u32 * CW).to_string())
                .collect();
            let text: String = glyphs.iter().map(|(_, g)| escape(g)).collect();
            let (size, baseline) = if is_wide(row, x) {
                (format!(r#" font-size="{WIDE_FONT_SIZE}""#), WIDE_BASELINE)
            } else {
                (String::new(), BASELINE)
            };
            let _ = writeln!(
                s,
                r#"<text x="{}" y="{}"{size}{}>{text}</text>"#,
                xs.join(" "),
                y as u32 * CH + baseline,
                attrs(first)
            );
            x = k;
        }
    }
}

/// `col`의 글자가 두 칸을 차지하는지 (바로 다음 칸이 둘째 칸이다).
fn is_wide(row: &[Cell], col: usize) -> bool {
    row.get(col + 1).is_some_and(|next| next.sym.is_empty())
}

/// `col`의 글자가 이어 그릴 수 있는 글자인지: 한 칸 폭이고 코드 포인트가 하나다.
fn flows(row: &[Cell], col: usize) -> bool {
    row[col].starts_text() && row[col].sym.chars().count() == 1 && !is_wide(row, col)
}

fn attrs(c: &Cell) -> String {
    let mut a = format!(r#" fill="{}""#, c.fg);
    if c.bold {
        a.push_str(r#" font-weight="bold""#);
    }
    if c.italic {
        a.push_str(r#" font-style="italic""#);
    }
    match (c.underline, c.strike) {
        (true, true) => a.push_str(r#" text-decoration="underline line-through""#),
        (true, false) => a.push_str(r#" text-decoration="underline""#),
        (false, true) => a.push_str(r#" text-decoration="line-through""#),
        (false, false) => {}
    }
    if c.dim {
        a.push_str(r#" opacity="0.6""#);
    }
    a
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
