//! Linear markdown → ratatui 줄. 폭은 표시 폭(한글 2칸) 기준.

use std::sync::LazyLock;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use regex::Regex;
use unicode_normalization::UnicodeNormalization;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Link,
    Image,
}

/// 본문에 나온 링크·이미지. `index`는 1부터, 등장 순서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkTarget {
    pub index: usize,
    pub kind: LinkKind,
    pub url: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub lines: Vec<Line<'static>>,
    pub links: Vec<LinkTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub heading: Style,
    pub rule: Style,
    pub code: Style,
    pub quote: Style,
    pub link: Style,
    pub dim: Style,
    pub identifier: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            heading: Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            rule: Style::new().fg(Color::DarkGray),
            code: Style::new().fg(Color::Yellow).bg(Color::Rgb(40, 40, 40)),
            quote: Style::new().fg(Color::DarkGray),
            link: Style::new()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            dim: Style::new().fg(Color::DarkGray),
            identifier: Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        }
    }
}

/// 터미널 제어 문자(ESC 등)를 지운다. 줄바꿈과 탭은 남긴다.
/// Linear 내용은 신뢰할 수 없는 입력이라 터미널에 그대로 쓰면 안 된다.
pub fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// markdown을 `width` 칸에 맞춰 그린다. 폭은 최소 10칸으로 본다.
/// 그리기 전에 NFC로 정규화하고 제어 문자를 지운다. 이슈 식별자 형식은 모두 강조한다.
pub fn render(md: &str, width: u16, theme: &Theme) -> Rendered {
    render_with(md, width, theme, &[])
}

/// `render`와 같되, `team_keys`가 있으면 그 팀 키의 식별자만 강조한다 (`UTF-8` 같은 오탐 방지).
pub fn render_with(md: &str, width: u16, theme: &Theme, team_keys: &[String]) -> Rendered {
    render_numbered(md, width, theme, team_keys, 1)
}

/// `render_with`와 같되 링크·이미지 번호를 `first`부터 매긴다.
/// 상세 화면처럼 본문과 코멘트를 따로 그려도 번호를 이어 가게 할 때 쓴다.
pub fn render_numbered(
    md: &str,
    width: u16,
    theme: &Theme,
    team_keys: &[String],
    first: usize,
) -> Rendered {
    let md = sanitize(&md.nfc().collect::<String>());
    let mut r = Renderer::new(usize::from(width).max(10), *theme);
    r.team_keys = team_keys.iter().map(|k| k.to_uppercase()).collect();
    r.first_link = first.max(1);
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for ev in Parser::new_ext(&md, opts) {
        r.event(ev);
    }
    r.finish()
}

/// markdown이 아닌 평문을 폭에 맞춰 줄바꿈한다 (제목처럼 긴 한 줄용).
pub fn wrap_text(text: &str, width: u16, style: Style) -> Vec<Line<'static>> {
    let text = sanitize(&text.nfc().collect::<String>());
    wrap(
        &[(text, style)],
        &[],
        &[],
        usize::from(width).max(10),
        false,
    )
}

/// 스타일 없이 텍스트만 (테스트·파이프 출력용).
pub fn to_plain(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// ANSI 이스케이프 문자열 (터미널 출력용).
pub fn to_ansi(lines: &[Line<'_>]) -> String {
    use ratatui::backend::IntoCrossterm;
    use ratatui::crossterm::style::{ContentStyle, StyledContent};
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        for span in &line.spans {
            let cs: ContentStyle = span.style.into_crossterm();
            // 색(ESC [ … m)은 여기서 붙인다. 내용에는 제어 문자가 남지 않게 한 번 더 거른다
            out.push_str(&StyledContent::new(cs, sanitize(&span.content)).to_string());
        }
    }
    out
}

/// 대문자 팀 키 + 번호 (예: ENG-123). 앞 글자가 영숫자면 제외한다.
static IDENTIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z][A-Z0-9]{0,9}-[0-9]+").expect("valid regex"));

/// `team_keys`가 비어 있지 않으면 그 키로 시작하는 식별자만 고른다.
fn find_identifiers(s: &str, team_keys: &[String]) -> Vec<(usize, usize)> {
    IDENTIFIER
        .find_iter(s)
        .filter(|m| {
            s[..m.start()]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-')
        })
        .filter(|m| {
            team_keys.is_empty()
                || m.as_str()
                    .split_once('-')
                    .is_some_and(|(key, _)| team_keys.iter().any(|k| k == key))
        })
        .map(|m| (m.start(), m.end()))
        .collect()
}

type Seg = (String, Style);

#[derive(Default)]
struct Table {
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    cell: Option<String>,
    header_rows: usize,
    /// 셀 안 링크: (주소, 셀 텍스트에서 링크 글자가 시작하는 위치)
    link: Option<(String, usize)>,
    /// 셀 안 이미지: (주소, alt 텍스트)
    image: Option<(String, String)>,
}

struct Renderer {
    width: usize,
    theme: Theme,
    out: Vec<Line<'static>>,
    links: Vec<LinkTarget>,
    inline: Vec<Seg>,
    styles: Vec<Style>,
    /// 목록마다 다음 번호. `None`이면 글머리표 목록.
    lists: Vec<Option<u64>>,
    quote: usize,
    marker: Option<String>,
    code: Option<String>,
    link: Option<(String, usize)>,
    image: Option<(String, String)>,
    table: Option<Table>,
    need_blank: bool,
    /// 강조할 이슈 식별자의 팀 키 (비어 있으면 모두)
    team_keys: Vec<String>,
    /// 첫 링크·이미지 번호 (본문과 코멘트를 이어 매길 때 1보다 크다)
    first_link: usize,
}

impl Renderer {
    fn new(width: usize, theme: Theme) -> Renderer {
        Renderer {
            width,
            theme,
            out: Vec::new(),
            links: Vec::new(),
            inline: Vec::new(),
            styles: Vec::new(),
            lists: Vec::new(),
            quote: 0,
            marker: None,
            code: None,
            link: None,
            image: None,
            table: None,
            need_blank: false,
            team_keys: Vec::new(),
            first_link: 1,
        }
    }

    fn finish(mut self) -> Rendered {
        self.flush();
        while self.out.last().is_some_and(|l| l.spans.is_empty()) {
            self.out.pop();
        }
        // `&#27;` 같은 문자 참조는 파싱하면서 제어 문자로 디코드되므로, 출력 직전에 다시 거른다
        for line in &mut self.out {
            for span in &mut line.spans {
                if span.content.chars().any(char::is_control) {
                    span.content = sanitize(&span.content).into();
                }
            }
        }
        for link in &mut self.links {
            link.url = sanitize(&link.url);
            link.label = sanitize(&link.label);
        }
        Rendered {
            lines: self.out,
            links: self.links,
        }
    }

    fn cur_style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, style: Style) {
        let s = self.cur_style().patch(style);
        self.styles.push(s);
    }

    fn push_seg(&mut self, text: String, style: Style) {
        if !text.is_empty() {
            self.inline.push((text, style));
        }
    }

    fn push_text(&mut self, s: &str) {
        let style = self.cur_style();
        if self.link.is_some() {
            self.push_seg(s.to_string(), style);
            return;
        }
        let mut last = 0;
        for (start, end) in find_identifiers(s, &self.team_keys) {
            self.push_seg(s[last..start].to_string(), style);
            self.push_seg(
                s[start..end].to_string(),
                style.patch(self.theme.identifier),
            );
            last = end;
        }
        self.push_seg(s[last..].to_string(), style);
    }

    fn blank_if_needed(&mut self) {
        if self.need_blank && !self.out.is_empty() {
            self.out.push(Line::default());
        }
        self.need_blank = false;
    }

    fn event(&mut self, ev: Event<'_>) {
        if let Some(mut t) = self.table.take() {
            if !self.table_event(&mut t, ev) {
                self.table = Some(t);
            }
            return;
        }
        if let Some(code) = self.code.as_mut() {
            match ev {
                Event::Text(s) => code.push_str(&s),
                Event::End(TagEnd::CodeBlock) => {
                    let c = self.code.take().unwrap_or_default();
                    self.render_code(&c);
                }
                _ => {}
            }
            return;
        }
        if let Some((_, alt)) = self.image.as_mut() {
            match ev {
                Event::Text(s) | Event::Code(s) => alt.push_str(&s),
                Event::End(TagEnd::Image) => {
                    if let Some((url, alt)) = self.image.take() {
                        self.push_image(url, alt);
                    }
                }
                _ => {}
            }
            return;
        }
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(s) => self.push_text(&s),
            Event::Code(s) => {
                let st = self.cur_style().patch(self.theme.code);
                self.push_seg(s.to_string(), st);
            }
            Event::Html(s) | Event::InlineHtml(s) => {
                let st = self.theme.dim;
                self.push_seg(s.trim_end_matches('\n').to_string(), st);
            }
            Event::SoftBreak => {
                let st = self.cur_style();
                self.push_seg(" ".to_string(), st);
            }
            Event::HardBreak => {
                let st = self.cur_style();
                self.push_seg("\n".to_string(), st);
            }
            Event::Rule => {
                self.flush();
                self.blank_if_needed();
                self.out.push(Line::from(Span::styled(
                    "─".repeat(self.width),
                    self.theme.rule,
                )));
                self.need_blank = true;
            }
            Event::TaskListMarker(done) => {
                self.marker = Some(if done { "☑ " } else { "☐ " }.to_string());
            }
            Event::FootnoteReference(s) => {
                let st = self.theme.dim;
                self.push_seg(format!("[^{s}]"), st);
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush();
                let style = match level {
                    HeadingLevel::H1 | HeadingLevel::H2 | HeadingLevel::H3 => self.theme.heading,
                    _ => Style::new().add_modifier(Modifier::BOLD),
                };
                self.push_style(style);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.quote += 1;
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.code = Some(String::new());
            }
            Tag::List(start) => {
                self.flush();
                self.lists.push(start);
            }
            Tag::Item => {
                self.flush();
                let marker = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "• ".to_string(),
                };
                self.marker = Some(marker);
            }
            Tag::Emphasis => self.push_style(Style::new().add_modifier(Modifier::ITALIC)),
            Tag::Strong => self.push_style(Style::new().add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => self.push_style(Style::new().add_modifier(Modifier::CROSSED_OUT)),
            Tag::Link { dest_url, .. } => {
                self.link = Some((dest_url.to_string(), self.inline.len()));
                self.push_style(self.theme.link);
            }
            Tag::Image { dest_url, .. } => {
                self.image = Some((dest_url.to_string(), String::new()));
            }
            Tag::Table(_) => {
                self.flush();
                self.table = Some(Table::default());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::HtmlBlock => {
                self.flush();
                self.need_blank = true;
            }
            TagEnd::Heading(level) => {
                self.flush();
                self.styles.pop();
                if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
                    self.out.push(Line::from(Span::styled(
                        "─".repeat(self.width),
                        self.theme.rule,
                    )));
                }
                self.need_blank = true;
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote = self.quote.saturating_sub(1);
                self.need_blank = true;
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
                if self.lists.is_empty() {
                    self.need_blank = true;
                }
            }
            TagEnd::Item => self.flush(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.styles.pop();
            }
            TagEnd::Link => {
                self.styles.pop();
                if let Some((url, start)) = self.link.take() {
                    let label: String = self.inline[start..]
                        .iter()
                        .map(|(s, _)| s.as_str())
                        .collect();
                    let index = self.add_link(url, label.trim().to_string());
                    let st = self.theme.dim;
                    self.push_seg(format!(" [{index}]"), st);
                }
            }
            _ => {}
        }
    }

    fn push_image(&mut self, url: String, alt: String) {
        let text = self.add_image(url, alt);
        let st = self.theme.dim;
        self.push_seg(text, st);
    }

    /// 링크를 번호 목록에 넣고 번호를 돌려준다.
    fn add_link(&mut self, url: String, label: String) -> usize {
        let index = self.first_link + self.links.len();
        self.links.push(LinkTarget {
            index,
            kind: LinkKind::Link,
            url,
            label,
        });
        index
    }

    /// 이미지를 번호 목록에 넣고 본문에 보일 자리 표시 문구를 돌려준다.
    fn add_image(&mut self, url: String, alt: String) -> String {
        let index = self.first_link + self.links.len();
        let label = if alt.trim().is_empty() {
            file_name(&url)
        } else {
            alt.trim().to_string()
        };
        self.links.push(LinkTarget {
            index,
            kind: LinkKind::Image,
            url,
            label: label.clone(),
        });
        format!("[이미지 {index}: {label}]")
    }

    /// 표 안의 이벤트. 셀 텍스트를 모으고, 링크·이미지에도 번호를 매긴다. 표를 다 그렸으면 true.
    fn table_event(&mut self, t: &mut Table, ev: Event<'_>) -> bool {
        if let Some((_, alt)) = t.image.as_mut() {
            match ev {
                Event::Text(s) | Event::Code(s) => alt.push_str(&s),
                Event::End(TagEnd::Image) => {
                    if let Some((url, alt)) = t.image.take() {
                        let text = self.add_image(url, alt);
                        if let Some(c) = t.cell.as_mut() {
                            c.push_str(&text);
                        }
                    }
                }
                _ => {}
            }
            return false;
        }
        match ev {
            Event::Start(Tag::TableCell) => t.cell = Some(String::new()),
            Event::End(TagEnd::TableCell) => {
                let c = t.cell.take().unwrap_or_default();
                t.row.push(c.trim().to_string());
            }
            Event::End(TagEnd::TableHead) => {
                let row = std::mem::take(&mut t.row);
                t.rows.push(row);
                t.header_rows = 1;
            }
            Event::End(TagEnd::TableRow) => {
                let row = std::mem::take(&mut t.row);
                t.rows.push(row);
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let start = t.cell.as_ref().map_or(0, String::len);
                t.link = Some((dest_url.to_string(), start));
            }
            Event::End(TagEnd::Link) => {
                if let Some((url, start)) = t.link.take()
                    && let Some(c) = t.cell.as_mut()
                {
                    let label = c.get(start..).unwrap_or("").trim().to_string();
                    let index = self.add_link(url, label);
                    c.push_str(&format!(" [{index}]"));
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                t.image = Some((dest_url.to_string(), String::new()));
            }
            Event::Text(s) | Event::Code(s) | Event::Html(s) | Event::InlineHtml(s) => {
                if let Some(c) = t.cell.as_mut() {
                    c.push_str(&s);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(c) = t.cell.as_mut() {
                    c.push(' ');
                }
            }
            Event::End(TagEnd::Table) => {
                let table = std::mem::take(t);
                self.render_table(table);
                return true;
            }
            _ => {}
        }
        false
    }

    /// 목록·인용 접두사. (첫 줄, 이어지는 줄)
    fn prefixes(&mut self) -> (Vec<Seg>, Vec<Seg>) {
        let mut base: Vec<Seg> = Vec::new();
        for _ in 0..self.quote {
            base.push(("▌ ".to_string(), self.theme.quote));
        }
        let depth = self.lists.len();
        if depth > 1 {
            base.push(("  ".repeat(depth - 1), Style::default()));
        }
        match self.marker.take() {
            Some(m) => {
                let w = m.width();
                let mut first = base.clone();
                first.push((m, Style::default()));
                let mut rest = base;
                rest.push((" ".repeat(w), Style::default()));
                (first, rest)
            }
            None => {
                if depth > 0 {
                    base.push(("  ".to_string(), Style::default()));
                }
                (base.clone(), base)
            }
        }
    }

    fn flush(&mut self) {
        if self.inline.is_empty() && self.marker.is_none() {
            return;
        }
        let mut segs = std::mem::take(&mut self.inline);
        if self.quote > 0 {
            for (_, st) in segs.iter_mut() {
                *st = st.add_modifier(Modifier::DIM);
            }
        }
        self.blank_if_needed();
        let (first, rest) = self.prefixes();
        let lines = wrap(&segs, &first, &rest, self.width, false);
        self.out.extend(lines);
    }

    fn render_code(&mut self, code: &str) {
        self.blank_if_needed();
        let (mut first, mut rest) = self.prefixes();
        first.push(("  ".to_string(), self.theme.code));
        rest.push(("  ".to_string(), self.theme.code));
        for raw in code.trim_end_matches('\n').split('\n') {
            let text = raw.replace('\t', "    ");
            let segs = vec![(text, self.theme.code)];
            for mut line in wrap(&segs, &first, &rest, self.width, true) {
                // 코드 블록 배경을 폭 끝까지 칠한다
                let w = line.width();
                if w < self.width {
                    line.spans
                        .push(Span::styled(" ".repeat(self.width - w), self.theme.code));
                }
                self.out.push(line);
            }
        }
        self.need_blank = true;
    }

    fn render_table(&mut self, t: Table) {
        let cols = t.rows.iter().map(Vec::len).max().unwrap_or(0);
        if cols == 0 {
            return;
        }
        self.blank_if_needed();
        let mut widths = vec![1usize; cols];
        for row in &t.rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.width());
            }
        }
        let total = |w: &[usize]| 1 + w.iter().map(|x| x + 3).sum::<usize>();
        while total(&widths) > self.width {
            let (i, max) = widths
                .iter()
                .copied()
                .enumerate()
                .max_by_key(|(_, w)| *w)
                .unwrap_or((0, 0));
            if max <= 3 {
                break;
            }
            widths[i] -= 1;
        }
        let rule = self.theme.rule;
        self.out.push(border(&widths, "┌", "┬", "┐", rule));
        for (ri, row) in t.rows.iter().enumerate() {
            let style = if ri < t.header_rows {
                Style::new().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let mut spans = vec![Span::styled("│", rule)];
            for (i, w) in widths.iter().enumerate() {
                let cell = row.get(i).map(String::as_str).unwrap_or("");
                spans.push(Span::styled(format!(" {} ", fit(cell, *w)), style));
                spans.push(Span::styled("│", rule));
            }
            self.out.push(Line::from(spans));
            if ri + 1 == t.header_rows && t.rows.len() > t.header_rows {
                self.out.push(border(&widths, "├", "┼", "┤", rule));
            }
        }
        self.out.push(border(&widths, "└", "┴", "┘", rule));
        self.need_blank = true;
    }
}

fn border(widths: &[usize], left: &str, mid: &str, right: &str, style: Style) -> Line<'static> {
    let inner: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
    Line::from(Span::styled(
        format!("{left}{}{right}", inner.join(mid)),
        style,
    ))
}

/// 표시 폭 `w`에 맞춰 자르고(…) 오른쪽을 공백으로 채운다.
fn fit(s: &str, w: usize) -> String {
    let sw = s.width();
    if sw <= w {
        return format!("{s}{}", " ".repeat(w - sw));
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if used + cw + 1 > w {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    used += 1;
    out.push_str(&" ".repeat(w.saturating_sub(used)));
    out
}

fn file_name(url: &str) -> String {
    url.split(['?', '#'])
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("image")
        .to_string()
}

fn width_of(chars: &[(char, Style)]) -> usize {
    chars.iter().map(|(c, _)| c.width().unwrap_or(0)).sum()
}

fn make_line(prefix: &[Seg], chars: &[(char, Style)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = prefix
        .iter()
        .map(|(t, s)| Span::styled(t.clone(), *s))
        .collect();
    let mut buf = String::new();
    let mut style: Option<Style> = None;
    for (c, st) in chars {
        if style != Some(*st) {
            if let Some(prev) = style
                && !buf.is_empty()
            {
                spans.push(Span::styled(std::mem::take(&mut buf), prev));
            }
            style = Some(*st);
        }
        buf.push(*c);
    }
    if let Some(st) = style
        && !buf.is_empty()
    {
        spans.push(Span::styled(buf, st));
    }
    Line::from(spans)
}

/// 표시 폭 기준 줄바꿈. 보통은 마지막 공백에서 끊고, 공백이 없으면 글자 단위로 끊는다.
/// `code`이면 글자 단위로만 끊고 줄 앞 공백을 지우지 않는다.
fn wrap(segs: &[Seg], first: &[Seg], rest: &[Seg], width: usize, code: bool) -> Vec<Line<'static>> {
    let prefix_w = |p: &[Seg]| p.iter().map(|(t, _)| t.width()).sum::<usize>();
    let avail_first = width.saturating_sub(prefix_w(first)).max(1);
    let avail_rest = width.saturating_sub(prefix_w(rest)).max(1);
    let mut lines = Vec::new();
    let mut cur: Vec<(char, Style)> = Vec::new();
    let mut on_first = true;
    for (text, style) in segs {
        for c in text.chars() {
            if c == '\n' {
                lines.push(make_line(if on_first { first } else { rest }, &cur));
                cur.clear();
                on_first = false;
                continue;
            }
            let cw = c.width().unwrap_or(0);
            loop {
                let avail = if on_first { avail_first } else { avail_rest };
                if cur.is_empty() || width_of(&cur) + cw <= avail {
                    break;
                }
                let prefix = if on_first { first } else { rest };
                let space = if code {
                    None
                } else {
                    cur.iter()
                        .rposition(|(ch, _)| *ch == ' ')
                        .filter(|&i| i > 0)
                };
                match space {
                    Some(i) => {
                        let tail = cur.split_off(i + 1);
                        cur.pop();
                        lines.push(make_line(prefix, &cur));
                        cur = tail;
                    }
                    None => {
                        lines.push(make_line(prefix, &cur));
                        cur.clear();
                    }
                }
                on_first = false;
            }
            if c == ' ' && cur.is_empty() && !on_first && !code {
                continue;
            }
            cur.push((c, *style));
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(make_line(if on_first { first } else { rest }, &cur));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(md: &str, width: u16) -> Vec<String> {
        let r = render(md, width, &Theme::default());
        to_plain(&r.lines).split('\n').map(str::to_string).collect()
    }

    #[test]
    fn heading_rule_and_paragraph() {
        assert_eq!(
            plain("# 제목\n\n본문 문단입니다.", 20),
            vec![
                "제목".to_string(),
                "─".repeat(20),
                String::new(),
                "본문 문단입니다.".to_string()
            ]
        );
    }

    #[test]
    fn small_headings_have_no_rule() {
        assert_eq!(
            plain("### 작은 제목\n#### 더 작은", 20),
            vec!["작은 제목", "", "더 작은"]
        );
    }

    #[test]
    fn heading_styles() {
        let theme = Theme::default();
        let r = render("## 둘\n\n#### 넷", 20, &theme);
        assert_eq!(r.lines[0].spans[0].style, theme.heading);
        let h4 = r
            .lines
            .iter()
            .find(|l| to_plain(std::slice::from_ref(l)) == "넷")
            .unwrap();
        assert_eq!(h4.spans[0].style, Style::new().add_modifier(Modifier::BOLD));
    }

    #[test]
    fn paragraphs_are_separated_and_breaks_work() {
        assert_eq!(
            plain("첫 문단\n\n둘째 문단", 20),
            vec!["첫 문단", "", "둘째 문단"]
        );
        assert_eq!(plain("줄1\n줄2", 20), vec!["줄1 줄2"]);
        assert_eq!(plain("줄1  \n줄2", 20), vec!["줄1", "줄2"]);
    }

    #[test]
    fn nested_bullets_and_numbers() {
        assert_eq!(
            plain("- 하나\n- 둘\n  - 셋\n", 20),
            vec!["• 하나", "• 둘", "  • 셋"]
        );
        assert_eq!(plain("1. 첫째\n2. 둘째\n", 20), vec!["1. 첫째", "2. 둘째"]);
    }

    #[test]
    fn task_list_boxes() {
        assert_eq!(
            plain("- [x] 끝\n- [ ] 할 일\n", 20),
            vec!["☑ 끝", "☐ 할 일"]
        );
    }

    #[test]
    fn list_item_wraps_under_its_text() {
        assert_eq!(
            plain("- 가나다 라마바 사아자", 12),
            vec!["• 가나다", "  라마바", "  사아자"]
        );
    }

    #[test]
    fn code_block_keeps_indent_and_hard_wraps() {
        let trimmed = |md: &str| -> Vec<String> {
            plain(md, 20)
                .into_iter()
                .map(|l| l.trim_end().to_string())
                .collect()
        };
        assert_eq!(
            trimmed("```rust\nfn main() {}\n```"),
            vec!["  fn main() {}"]
        );
        assert_eq!(
            trimmed("```\n0123456789012345678901234\n```"),
            vec!["  012345678901234567", "  8901234"]
        );
    }

    #[test]
    fn code_block_uses_code_style() {
        let theme = Theme::default();
        let r = render("```\nx\n```", 20, &theme);
        assert!(r.lines[0].spans.iter().all(|s| s.style == theme.code));
    }

    #[test]
    fn quote_has_bar_and_dim_text() {
        let r = render("> 인용문", 20, &Theme::default());
        assert_eq!(to_plain(&r.lines), "▌ 인용문");
        assert!(
            r.lines[0].spans[1]
                .style
                .add_modifier
                .contains(Modifier::DIM)
        );
    }

    #[test]
    fn table_aligns_korean_width() {
        assert_eq!(
            plain("| 이름 | 값 |\n|---|---|\n| 로그인 | 1 |\n", 40),
            vec![
                "┌────────┬────┐",
                "│ 이름   │ 값 │",
                "├────────┼────┤",
                "│ 로그인 │ 1  │",
                "└────────┴────┘",
            ]
        );
    }

    #[test]
    fn wide_table_cells_are_truncated() {
        assert_eq!(
            plain("| 설명 |\n|---|\n| 아주 긴 설명 텍스트입니다 |\n", 12),
            vec![
                "┌──────────┐",
                "│ 설명     │",
                "├──────────┤",
                "│ 아주 긴… │",
                "└──────────┘"
            ]
        );
    }

    #[test]
    fn links_and_images_are_numbered() {
        let r = render(
            "자세한 건 [문서](https://x.dev/doc)와 ![스크린샷](https://x.dev/a/shot.png) 참고, ![](https://x.dev/b/c.png?v=1)",
            80,
            &Theme::default(),
        );
        assert_eq!(
            to_plain(&r.lines),
            "자세한 건 문서 [1]와 [이미지 2: 스크린샷] 참고, [이미지 3: c.png]"
        );
        assert_eq!(
            r.links,
            vec![
                LinkTarget {
                    index: 1,
                    kind: LinkKind::Link,
                    url: "https://x.dev/doc".into(),
                    label: "문서".into()
                },
                LinkTarget {
                    index: 2,
                    kind: LinkKind::Image,
                    url: "https://x.dev/a/shot.png".into(),
                    label: "스크린샷".into()
                },
                LinkTarget {
                    index: 3,
                    kind: LinkKind::Image,
                    url: "https://x.dev/b/c.png?v=1".into(),
                    label: "c.png".into()
                },
            ]
        );
    }

    #[test]
    fn identifiers_are_highlighted() {
        let theme = Theme::default();
        let r = render("ENG-123을 보세요. abcENG-12는 아님", 80, &theme);
        let hits: Vec<&str> = r.lines[0]
            .spans
            .iter()
            .filter(|s| s.style == theme.identifier)
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(hits, vec!["ENG-123"]);
    }

    #[test]
    fn korean_wraps_at_spaces_then_chars() {
        assert_eq!(
            plain("가나다 라마바 사아자", 10),
            vec!["가나다", "라마바", "사아자"]
        );
        assert_eq!(plain("가나다라마바사", 10), vec!["가나다라마", "바사"]);
    }

    #[test]
    fn html_is_shown_as_text() {
        assert_eq!(
            plain("<details>숨김</details>", 40),
            vec!["<details>숨김</details>"]
        );
    }

    #[test]
    fn rule_and_inline_code() {
        assert_eq!(
            plain("위\n\n---\n\n`코드` 아래", 10),
            vec![
                "위".to_string(),
                String::new(),
                "─".repeat(10),
                String::new(),
                "코드 아래".to_string()
            ]
        );
    }

    #[test]
    fn ansi_output_has_escape_codes() {
        let r = render("**굵게**", 20, &Theme::default());
        let ansi = to_ansi(&r.lines);
        assert!(ansi.contains("\u{1b}["), "{ansi:?}");
        assert!(ansi.contains("굵게"));
    }

    #[test]
    fn empty_input_is_empty() {
        let r = render("", 20, &Theme::default());
        assert!(r.lines.is_empty());
    }

    #[test]
    fn control_characters_are_removed() {
        // ESC 시퀀스(색 바꾸기, OSC 52 클립보드 쓰기)가 터미널에 그대로 가면 안 된다
        assert_eq!(
            plain("a\u{1b}[31mb \u{1b}]52;c;ZXZpbA==\u{7}c", 40),
            vec!["a[31mb ]52;c;ZXZpbA==c"]
        );
        assert_eq!(sanitize("줄1\n\t줄2\u{0}"), "줄1\n\t줄2");
    }

    #[test]
    fn decomposed_hangul_is_normalized_before_wrapping() {
        let nfd: String = "가나다 라마바".nfd().collect();
        assert_eq!(plain(&nfd, 10), vec!["가나다", "라마바"]);
    }

    #[test]
    fn tiny_width_and_wide_tables_do_not_panic() {
        let r = render("# 제목\n\n본문 텍스트입니다", 3, &Theme::default());
        assert!(
            r.lines.iter().all(|l| l.width() <= 10),
            "{:?}",
            to_plain(&r.lines)
        );
        let table = "| a | b | c | d | e |\n|---|---|---|---|---|\n| 1 | 2 | 3 | 4 | 5 |\n";
        let r = render(table, 10, &Theme::default());
        assert_eq!(r.lines.len(), 5);
    }

    fn has_control(s: &str) -> bool {
        s.chars().any(|c| c.is_control() && c != '\n')
    }

    #[test]
    fn entity_encoded_control_characters_are_removed() {
        // pulldown-cmark는 숫자 문자 참조를 디코드하므로, 파싱 뒤에도 걸러야 한다
        let r = render(
            "a &#27;]52;c;ZXZpbA==&#7; b &#x1b;[2J c &#x9b;31m d",
            80,
            &Theme::default(),
        );
        let text = to_plain(&r.lines);
        assert!(!has_control(&text), "{text:?}");
        assert_eq!(text, "a ]52;c;ZXZpbA== b [2J c 31m d");
    }

    #[test]
    fn entity_encoded_control_characters_in_links_and_tables_are_removed() {
        let r = render(
            "[링크&#7;](https://e.com/&#27;[2J)\n\n| a |\n|---|\n| &#27;]0;t&#7; |\n",
            80,
            &Theme::default(),
        );
        assert_eq!(r.links.len(), 1);
        assert!(!has_control(&r.links[0].url), "{:?}", r.links[0].url);
        assert!(!has_control(&r.links[0].label), "{:?}", r.links[0].label);
        let text = to_plain(&r.lines);
        assert!(!has_control(&text), "{text:?}");
    }

    #[test]
    fn code_block_background_fills_width() {
        let theme = Theme::default();
        let r = render("```\nx\n```", 20, &theme);
        assert_eq!(r.lines[0].width(), 20);
        assert!(r.lines[0].spans.iter().all(|s| s.style == theme.code));
    }

    #[test]
    fn links_and_images_inside_tables_are_numbered() {
        let r = render(
            "| 문서 | 그림 |\n|---|---|\n| [가이드](https://x.dev/g) | ![](https://x.dev/a.png) |\n",
            60,
            &Theme::default(),
        );
        assert_eq!(r.links.len(), 2, "{:?}", r.links);
        assert_eq!(r.links[0].url, "https://x.dev/g");
        assert_eq!(r.links[0].label, "가이드");
        assert_eq!(r.links[1].kind, LinkKind::Image);
        let text = to_plain(&r.lines);
        assert!(text.contains("가이드 [1]"), "{text}");
        assert!(text.contains("[이미지 2: a.png]"), "{text}");
    }

    #[test]
    fn identifier_highlight_can_be_limited_to_team_keys() {
        let theme = Theme::default();
        let highlighted = |r: &Rendered| -> Vec<String> {
            r.lines[0]
                .spans
                .iter()
                .filter(|s| s.style == theme.identifier)
                .map(|s| s.content.to_string())
                .collect()
        };
        let md = "UTF-8 문서와 ENG-12, SHA-256";
        assert_eq!(
            highlighted(&render(md, 80, &theme)),
            vec!["UTF-8", "ENG-12", "SHA-256"]
        );
        assert_eq!(
            highlighted(&render_with(md, 80, &theme, &["eng".to_string()])),
            vec!["ENG-12"]
        );
    }

    #[test]
    fn link_numbers_can_start_later() {
        let r = render_numbered(
            "[a](https://a.dev) ![](https://b.dev/x.png)",
            40,
            &Theme::default(),
            &[],
            3,
        );
        let numbers: Vec<usize> = r.links.iter().map(|l| l.index).collect();
        assert_eq!(numbers, vec![3, 4]);
        let text = to_plain(&r.lines);
        assert!(
            text.contains("a [3]") && text.contains("[이미지 4: x.png]"),
            "{text}"
        );
    }

    #[test]
    fn ansi_output_never_carries_control_characters_from_content() {
        let line = Line::from(Span::styled(
            "a\u{1b}]52;c;eA==\u{7}b",
            Style::new().fg(Color::Red),
        ));
        let out = to_ansi(&[line]);
        assert!(
            !out.contains("\u{1b}]") && !out.contains('\u{7}'),
            "{out:?}"
        );
        assert!(out.contains("a]52;c;eA==b"), "{out:?}");
    }
}
