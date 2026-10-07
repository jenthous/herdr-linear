//! 팔레트 그리기.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::app::{App, Detail, Mode, Problem, Row, Tab};
use crate::linear::types::Issue;
use crate::markdown::{self, Theme, sanitize};
use crate::ui::row::issue_row;
use crate::ui::style::{
    ACCENT, DIM, ago, label_style, local_time, priority_label, state_icon, state_style, truncate,
};

/// 이 폭 이상이면 목록 옆에 미리보기를 붙인다.
pub const PREVIEW_MIN_WIDTH: u16 = 100;

const SELECTED_BG: Style = Style::new().bg(Color::Rgb(45, 45, 60));
const WARN: Style = Style::new().fg(Color::Yellow);
const ERROR: Style = Style::new().fg(Color::Red);

/// 마우스로 누를 수 있는 대상.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// 목록의 n번째 줄 (`App::rows` 기준)
    Row(usize),
    Tab(Tab),
    /// 메뉴에 보이는 n번째 항목
    MenuItem(usize),
}

/// 화면에서 누를 수 있는 곳.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub area: Rect,
    pub target: Target,
}

/// 그린 결과.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Drawn {
    /// 상세 화면이면 최대 스크롤
    pub detail_max_scroll: Option<u16>,
    /// 누를 수 있는 곳. 나중에 그린 것(메뉴)이 뒤에 온다
    pub hits: Vec<Hit>,
    /// 목록을 그렸으면 그 스크롤 위치 (다음 프레임에 이어 쓴다)
    pub list_offset: Option<usize>,
}

impl Drawn {
    /// (x, y)에 맨 위로 그려진 대상.
    pub fn target_at(&self, x: u16, y: u16) -> Option<Target> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.area.contains(Position::new(x, y)))
            .map(|h| h.target)
    }
}

/// 화면 전체를 그린다.
pub fn draw(f: &mut Frame, app: &App, now: i64) -> Drawn {
    let area = f.area();
    if app.mode == Mode::Onboarding {
        draw_onboarding(f, app, area);
        return Drawn::default();
    }
    let [header, search, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let mut drawn = Drawn::default();
    drawn.hits.extend(draw_header(f, app, header, now));
    if app.mode == Mode::Detail {
        drawn.detail_max_scroll = Some(draw_detail(f, app, search.union(body)));
    } else {
        draw_search(f, app, search);
        if body.width >= PREVIEW_MIN_WIDTH {
            let [list, preview] =
                Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .areas(body);
            draw_list(f, app, list, &mut drawn);
            draw_preview(f, app, preview);
        } else {
            draw_list(f, app, body, &mut drawn);
        }
    }
    draw_footer(f, app, footer, now);
    if app.menu.is_some() {
        drawn.hits.extend(draw_menu(f, app, area));
    }
    drawn
}

/// 탭 이름의 위치를 함께 돌려준다 (마우스로 탭 전환).
fn draw_header(f: &mut Frame, app: &App, area: Rect, now: i64) -> Vec<Hit> {
    let mut spans = vec![Span::styled(" Linear ", ACCENT)];
    let mut hits = Vec::new();
    let mut x = area.x.saturating_add(spans[0].width() as u16);
    for tab in Tab::ALL {
        spans.push(Span::raw(" "));
        x = x.saturating_add(1);
        let span = if tab == app.tab {
            Span::styled(format!("[{}]", tab.title()), ACCENT)
        } else {
            Span::styled(tab.title(), DIM)
        };
        let w = span.width() as u16;
        hits.push(Hit {
            area: Rect::new(x, area.y, w, 1).intersection(area),
            target: Target::Tab(tab),
        });
        x = x.saturating_add(w);
        spans.push(span);
    }
    let status = status_span(app, now);
    let used = Line::from(spans.clone()).width() + status.width() + 1;
    spans.push(Span::raw(
        " ".repeat(usize::from(area.width).saturating_sub(used)),
    ));
    spans.push(status);
    f.render_widget(Paragraph::new(Line::from(spans)), area);
    hits
}

/// 상단 오른쪽 상태: 갱신 중 / n분 전 갱신 / 오프라인 / 오류. 한도 초과는 하단에 잠깐 알린다.
pub fn status_span(app: &App, now: i64) -> Span<'static> {
    if app.loading > 0 {
        return Span::styled("갱신 중…", DIM);
    }
    match &app.problem {
        Some(Problem::Offline(_)) => Span::styled("오프라인", WARN),
        Some(Problem::Error(_)) => Span::styled("오류", ERROR),
        None => match app.updated_at {
            Some(at) => Span::styled(format!("{} 갱신", ago(now, at)), DIM),
            None => Span::raw(""),
        },
    }
}

fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let query = sanitize(&app.query);
    let line = if app.mode == Mode::Search {
        let mut spans = vec![Span::styled(" > ", ACCENT), Span::raw(query.clone())];
        if query.is_empty() {
            spans.push(Span::styled(
                " ID·제목·본문 검색 · l:라벨 s:상태 @담당자 #팀",
                DIM,
            ));
        }
        // 한글 IME가 조합 중인 글자를 제자리에 보이도록 실제 커서를 검색어 끝에 둔다
        let x = area.x + 3 + query.width() as u16;
        f.set_cursor_position(Position::new(x.min(area.right().saturating_sub(1)), area.y));
        Line::from(spans)
    } else if query.is_empty() {
        Line::from(Span::styled(" / 검색", DIM))
    } else {
        Line::from(vec![Span::styled(" / ", DIM), Span::raw(query)])
    };
    f.render_widget(Paragraph::new(line), area);
}

/// 목록을 그리고, 줄의 위치(마우스로 선택·열기)와 스크롤 위치를 `drawn`에 남긴다.
fn draw_list(f: &mut Frame, app: &App, area: Rect, drawn: &mut Drawn) {
    if app.rows.is_empty() {
        let text = if app.loading > 0 {
            "불러오는 중…"
        } else {
            "결과가 없어요"
        };
        f.render_widget(Paragraph::new(Span::styled(format!("  {text}"), DIM)), area);
        return;
    }
    let width = area.width.saturating_sub(2);
    let mut items = Vec::new();
    // 화면 줄마다 어느 이슈 줄인지 (머리말·구분선은 None)
    let mut item_rows: Vec<Option<usize>> = Vec::new();
    let mut selected_item = 0;
    for (i, row) in app.rows.iter().enumerate() {
        if matches!(row, Row::Pinned(_)) {
            items.push(ListItem::new(Span::styled(" 현재 브랜치", DIM)));
            item_rows.push(None);
        }
        if i > 0 && matches!(app.rows[i - 1], Row::Pinned(_)) {
            items.push(ListItem::new(Span::styled(
                format!(" {}", "─".repeat(usize::from(width).min(30))),
                DIM,
            )));
            item_rows.push(None);
        }
        if i == app.selected {
            selected_item = items.len();
        }
        let line = match row {
            Row::Pinned(issue) | Row::Issue(issue) => issue_row(issue, width),
            Row::DeepSearch => Line::from(Span::styled("⏎ 서버에서 검색 (코멘트 포함)", ACCENT)),
        };
        let marker = if i == app.selected { "▶ " } else { "  " };
        let mut spans = vec![Span::styled(marker, ACCENT)];
        spans.extend(line.spans);
        items.push(ListItem::new(Line::from(spans)));
        item_rows.push(Some(i));
    }
    // 항목은 모두 한 줄이라, 칸을 다 채울 수 있는 만큼만 이전 스크롤 위치를 이어 쓴다
    let max_offset = items.len().saturating_sub(usize::from(area.height));
    let mut state = ListState::default()
        .with_offset(app.list_offset.min(max_offset))
        .with_selected(Some(selected_item));
    f.render_stateful_widget(
        List::new(items).highlight_style(SELECTED_BG),
        area,
        &mut state,
    );
    drawn.list_offset = Some(state.offset());
    drawn.hits.extend((0..area.height).filter_map(|k| {
        let row = (*item_rows.get(state.offset() + usize::from(k))?)?;
        Some(Hit {
            area: Rect::new(area.x, area.y + k, area.width, 1),
            target: Target::Row(row),
        })
    }));
}

/// 이슈 머리: 식별자·제목 / 상태·우선순위·담당자 / 라벨·프로젝트·사이클·상위·예상·마감.
pub fn issue_header(issue: &Issue, width: u16) -> Vec<Line<'static>> {
    let mut lines = markdown::wrap_text(
        &format!("{}  {}", issue.identifier, issue.title),
        width,
        Style::new().add_modifier(Modifier::BOLD),
    );
    if let Some(first) = lines.first_mut()
        && let Some(span) = first.spans.first_mut()
        && span.content.starts_with(issue.identifier.as_str())
    {
        // 식별자만 강조색으로
        let rest = span.content[issue.identifier.len()..].to_string();
        let id = Span::styled(issue.identifier.clone(), ACCENT);
        *span = Span::styled(rest, span.style);
        first.spans.insert(0, id);
    }
    let mut meta = vec![
        Span::styled(
            format!(
                "{} {}",
                state_icon(&issue.state.state_type),
                sanitize(&issue.state.name)
            ),
            state_style(&issue.state),
        ),
        Span::styled(
            format!(" · 우선순위 {}", priority_label(issue.priority)),
            DIM,
        ),
    ];
    match &issue.assignee {
        Some(a) => meta.push(Span::styled(
            format!(" · @{}", sanitize(&a.display_name)),
            DIM,
        )),
        None => meta.push(Span::styled(" · 담당자 없음", DIM)),
    }
    lines.push(Line::from(meta));
    if let Some(pr) = issue.open_pr() {
        let mut text = format!("{} 열림", pr.label());
        if pr.draft {
            text.push_str(" (초안)");
        }
        if let Some(repo) = &pr.repo {
            text.push_str(&format!(" · {}", sanitize(repo)));
        }
        lines.push(Line::from(Span::styled(
            text,
            Style::new().fg(Color::Green),
        )));
    }
    let mut extra: Vec<Span<'static>> = Vec::new();
    for label in &issue.labels.nodes {
        if !extra.is_empty() {
            extra.push(Span::raw(" "));
        }
        extra.push(Span::styled(sanitize(&label.name), label_style(label)));
    }
    let mut info = Vec::new();
    if let Some(p) = &issue.project {
        info.push(format!("프로젝트 {}", sanitize(&p.name)));
    }
    if let Some(c) = &issue.cycle {
        let name = c
            .name
            .clone()
            .unwrap_or_else(|| (c.number as i64).to_string());
        info.push(format!("사이클 {}", sanitize(&name)));
    }
    if let Some(p) = &issue.parent {
        info.push(format!("상위 {}", p.identifier));
    }
    if let Some(e) = issue.estimate {
        info.push(format!("예상 {e}"));
    }
    if let Some(d) = &issue.due_date {
        info.push(format!("마감 {}", sanitize(d)));
    }
    if !info.is_empty() {
        let sep = if extra.is_empty() { "" } else { " · " };
        extra.push(Span::styled(format!("{sep}{}", info.join(" · ")), DIM));
    }
    if !extra.is_empty() {
        lines.push(Line::from(extra));
    }
    lines
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::LEFT).border_style(DIM);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let Some(issue) = app.selected_issue() else {
        return;
    };
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(1),
        ..inner
    };
    let mut lines = issue_header(issue, inner.width);
    lines.push(Line::default());
    match issue.description.as_deref().map(str::trim) {
        Some(body) if !body.is_empty() => lines.extend(
            markdown::render_with(body, inner.width, &Theme::default(), &app.team_keys()).lines,
        ),
        _ => lines.push(Line::from(Span::styled("(본문 없음)", DIM))),
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// 상세 화면을 그리고 최대 스크롤을 돌려준다.
fn draw_detail(f: &mut Frame, app: &App, area: Rect) -> u16 {
    let Some(d) = &app.detail else {
        return 0;
    };
    let area = Rect {
        x: area.x + 1,
        width: area.width.saturating_sub(2),
        ..area
    };
    let lines = detail_lines(d, area.width, &app.team_keys());
    let total = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let max = total.saturating_sub(area.height);
    let scroll = d.scroll.min(max);
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
    max
}

/// 상세 화면의 줄. 링크·이미지 번호는 본문에서 코멘트로 이어 매긴다 (`u` 목록과 같은 번호).
fn detail_lines(d: &Detail, width: u16, keys: &[String]) -> Vec<Line<'static>> {
    let theme = Theme::default();
    let mut lines: Vec<Line<'static>> = Vec::new();
    match &d.issue {
        None => {
            let id = sanitize(&d.id);
            let (text, style) = if d.gone {
                (format!("{id}: 찾을 수 없거나 보관·삭제된 이슈예요"), WARN)
            } else if d.loading {
                (format!("{id} 불러오는 중…"), DIM)
            } else {
                (
                    format!("{id}: 불러오지 못했어요. r로 다시 시도하세요"),
                    WARN,
                )
            };
            lines.push(Line::from(Span::styled(text, style)));
        }
        Some(issue) => {
            lines.extend(issue_header(issue, width));
            lines.push(Line::from(Span::styled(issue.url.clone(), DIM)));
            if d.gone {
                lines.push(Line::from(Span::styled(
                    "보관되었거나 삭제된 이슈예요",
                    WARN,
                )));
            }
            lines.push(Line::default());
            let mut next_link = 1;
            match issue.description.as_deref().map(str::trim) {
                Some(body) if !body.is_empty() => {
                    let r = markdown::render_numbered(body, width, &theme, keys, next_link);
                    next_link += r.links.len();
                    lines.extend(r.lines);
                }
                _ => lines.push(Line::from(Span::styled("(본문 없음)", DIM))),
            }
            if !d.comments.is_empty() {
                lines.push(Line::default());
                lines.push(Line::from(Span::styled(
                    format!(
                        "── 코멘트 {}{} ──",
                        d.comments.len(),
                        if d.more_comments { "+" } else { "" }
                    ),
                    DIM,
                )));
                for c in &d.comments {
                    let who = c
                        .user
                        .as_ref()
                        .map_or("알 수 없음".to_string(), |u| sanitize(&u.display_name));
                    lines.push(Line::default());
                    lines.push(Line::from(vec![
                        Span::styled(who, Style::new().add_modifier(Modifier::BOLD)),
                        Span::styled(format!(" · {}", local_time(&c.created_at)), DIM),
                    ]));
                    let r = markdown::render_numbered(&c.body, width, &theme, keys, next_link);
                    next_link += r.links.len();
                    lines.extend(r.lines);
                }
                if d.more_comments {
                    lines.push(Line::default());
                    lines.push(Line::from(Span::styled(
                        "더 오래된 코멘트가 있어요. 브라우저에서 보세요 (o)",
                        DIM,
                    )));
                }
            } else if d.loading {
                lines.push(Line::default());
                lines.push(Line::from(Span::styled("코멘트 불러오는 중…", DIM)));
            }
        }
    }
    lines
}

fn hints(mode: Mode) -> &'static str {
    match mode {
        Mode::Search => " ⏎ 상세  Tab 보기  ↑↓ 이동  ^K 메뉴  Esc 목록 모드",
        Mode::List => " j/k 이동  / 검색  ⏎ 상세  y URL 복사  Y PR 링크  ^K 메뉴  q 닫기",
        Mode::Detail => " j/k 스크롤  u 링크  y URL 복사  Y PR 링크  ^K 메뉴  Esc 뒤로  q 닫기",
        Mode::Onboarding => " ⏎ 확인  Esc 닫기",
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect, now: i64) {
    let span = if let Some(text) = app.flash_text(now) {
        Span::styled(
            format!(" {}", sanitize(text)),
            Style::new().fg(Color::Green),
        )
    } else {
        match &app.problem {
            Some(Problem::Offline(m)) => Span::styled(
                format!(" 오프라인이라 저장된 내용을 보여줘요 ({})", sanitize(m)),
                WARN,
            ),
            Some(Problem::Error(m)) => Span::styled(format!(" 오류: {}", sanitize(m)), ERROR),
            None => Span::styled(hints(app.mode), DIM),
        }
    };
    let text = truncate(&span.content, usize::from(area.width));
    f.render_widget(Paragraph::new(Span::styled(text, span.style)), area);
}

/// 가운데에 `w`×`h` 사각형.
fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

/// 메뉴 항목의 위치를 함께 돌려준다 (마우스로 실행).
fn draw_menu(f: &mut Frame, app: &App, area: Rect) -> Vec<Hit> {
    let Some(menu) = &app.menu else {
        return Vec::new();
    };
    let items = menu.visible();
    let w = (area.width * 6 / 10).max(30);
    let h = u16::try_from(items.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4);
    let rect = centered(area, w, h);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(ACCENT)
        .title(format!(" {} ", menu.title));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let [filter, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    let text = sanitize(&menu.filter);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", ACCENT),
            Span::raw(text.clone()),
        ])),
        filter,
    );
    f.set_cursor_position(Position::new(
        (filter.x + 2 + text.width() as u16).min(filter.right().saturating_sub(1)),
        filter.y,
    ));
    let label_width = usize::from(list.width.saturating_sub(2));
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|(label, _)| ListItem::new(truncate(&sanitize(label), label_width)))
        .collect();
    let mut state = ListState::default().with_selected(Some(menu.selected));
    f.render_stateful_widget(
        List::new(list_items)
            .highlight_style(SELECTED_BG)
            .highlight_symbol("▶ "),
        list,
        &mut state,
    );
    (0..list.height)
        .filter_map(|k| {
            let i = state.offset() + usize::from(k);
            (i < items.len()).then(|| Hit {
                area: Rect::new(list.x, list.y + k, list.width, 1),
                target: Target::MenuItem(i),
            })
        })
        .collect()
}

fn draw_onboarding(f: &mut Frame, app: &App, area: Rect) {
    let rect = centered(area, 72, 11);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(ACCENT)
        .title(" Linear 연결 ");
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    if app.env_key_invalid {
        lines.push(Line::from(Span::styled(
            "LINEAR_API_KEY 환경 변수의 키가 유효하지 않아요",
            ERROR,
        )));
        lines.push(Line::from("환경 변수를 고치거나 지운 뒤 다시 열어주세요."));
        lines.push(Line::default());
        lines.push(Line::from(Span::styled("Esc 닫기", DIM)));
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }
    lines.push(Line::from("Linear Personal API 키를 붙여넣으세요."));
    lines.push(Line::from(Span::styled(
        "Linear → Settings → Security & access → Personal API keys",
        DIM,
    )));
    lines.push(Line::default());
    let masked = "•".repeat(app.key_input.chars().count());
    let shown = if app.key_checking {
        "확인 중…".to_string()
    } else {
        masked.clone()
    };
    lines.push(Line::from(vec![
        Span::styled("키: ", ACCENT),
        Span::raw(shown),
    ]));
    if let Some(err) = &app.key_error {
        lines.push(Line::from(Span::styled(sanitize(err), ERROR)));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("⏎ 확인 · Esc 닫기", DIM)));
    if !app.key_checking {
        f.set_cursor_position(Position::new(
            (inner.x + 4 + masked.width() as u16).min(inner.right().saturating_sub(1)),
            inner.y + 3,
        ));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::types::Viewer;
    use crate::test_support::IssueBuilder;
    use crate::tui::app::{Act, Input, Msg};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const T0: i64 = 1_000_000;

    fn viewer() -> Viewer {
        serde_json::from_value(serde_json::json!({
            "id": "me", "name": "김민수", "displayName": "minsu", "email": "m@acme.dev",
            "organization": { "id": "org1", "name": "Acme", "urlKey": "acme" },
            "teams": { "nodes": [ { "id": "team-ENG", "key": "ENG", "name": "Eng" } ] }
        }))
        .unwrap()
    }

    fn app() -> App {
        let (mut app, _) = App::start(None);
        app.apply(Msg::Viewer(viewer()), T0);
        app.apply(
            Msg::Tab {
                tab: Tab::Mine,
                issues: vec![
                    IssueBuilder::new("a", "ENG-1", "로그인 버그")
                        .state("In Progress", "started")
                        .labels(&["bug"])
                        .description("## 재현\n- 로그인 후 대기")
                        .build(),
                    IssueBuilder::new("b", "ENG-2", "결제 화면").build(),
                ],
                fresh: true,
                has_more: false,
                append: false,
            },
            T0,
        );
        app
    }

    /// 화면 각 줄의 글자 (넓은 글자 뒤 칸은 건너뛴다).
    fn screen(app: &App, w: u16, h: u16) -> (Vec<String>, Drawn, Terminal<TestBackend>) {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut drawn = Drawn::default();
        term.draw(|f| drawn = draw(f, app, T0)).unwrap();
        let buf = term.backend().buffer().clone();
        let rows = (0..h)
            .map(|y| {
                let mut s = String::new();
                let mut x = 0;
                while x < w {
                    let sym = buf[(x, y)].symbol();
                    s.push_str(sym);
                    x += if sym.width() == 2 { 2 } else { 1 };
                }
                s.trim_end().to_string()
            })
            .collect();
        (rows, drawn, term)
    }

    #[test]
    fn tiny_screens_do_not_panic() {
        let mut detail = app();
        detail.handle(Input::Enter, T0);
        let mut menu = app();
        menu.handle(Input::Menu, T0);
        let onboarding = App::onboarding(false);
        for app in [&app(), &detail, &menu, &onboarding] {
            for (w, h) in [(1, 1), (5, 3), (12, 4), (20, 5), (30, 8)] {
                screen(app, w, h);
            }
        }
    }

    #[test]
    fn palette_shows_tabs_status_rows_and_preview() {
        let (rows, _, _) = screen(&app(), 120, 16);
        assert!(
            rows[0].contains("[내 이슈]") && rows[0].contains("최근 본"),
            "{}",
            rows[0]
        );
        assert!(rows[0].ends_with("방금 갱신"), "{}", rows[0]);
        assert!(
            rows[2].contains("▶ ◐ ENG-1") && rows[2].contains("로그인 버그"),
            "{}",
            rows[2]
        );
        let all = rows.join("\n");
        assert!(all.contains("우선순위 없음"), "미리보기 머리\n{all}");
        assert!(all.contains("• 로그인 후 대기"), "미리보기 본문\n{all}");
    }

    #[test]
    fn narrow_palette_hides_preview() {
        let (rows, _, _) = screen(&app(), 80, 12);
        assert!(!rows.join("\n").contains("우선순위"));
    }

    #[test]
    fn search_mode_puts_cursor_after_query() {
        let mut a = app();
        a.handle(Input::Char('로'), T0);
        let (rows, _, mut term) = screen(&a, 80, 12);
        assert!(rows[1].starts_with(" > 로"), "{}", rows[1]);
        term.backend_mut().assert_cursor_position((5, 1));
    }

    #[test]
    fn pinned_section_has_label() {
        let mut a = app();
        a.apply(
            Msg::Pinned(Some(IssueBuilder::new("b", "ENG-2", "결제 화면").build())),
            T0,
        );
        let (rows, _, _) = screen(&a, 80, 12);
        assert_eq!(rows[2], " 현재 브랜치");
        assert!(rows[3].contains("ENG-2"), "{}", rows[3]);
        assert!(rows[4].contains('─'));
    }

    #[test]
    fn detail_shows_body_comments_and_max_scroll() {
        let mut a = app();
        a.handle(Input::Enter, T0);
        let long = (1..=40)
            // 줄바꿈 하나는 같은 문단이라 빈 줄로 문단을 나눈다
            .map(|i| format!("{i}번째 문단\n\n"))
            .collect::<String>();
        a.apply(
            Msg::Detail {
                id: "a".into(),
                issue: IssueBuilder::new("a", "ENG-1", "로그인 버그")
                    .description(&long)
                    .build(),
                comments: Vec::new(),
                more: false,
                fresh: true,
            },
            T0,
        );
        let (rows, drawn, _) = screen(&a, 80, 12);
        assert!(
            rows[1].contains("ENG-1") && rows[1].contains("로그인 버그"),
            "{rows:?}"
        );
        assert!(drawn.detail_max_scroll.unwrap() > 0);
        assert!(rows[11].ends_with("Esc 뒤로  q 닫기"), "{}", rows[11]);
    }

    #[test]
    fn menu_overlay_lists_actions() {
        let mut a = app();
        a.handle(Input::Menu, T0);
        let (rows, _, _) = screen(&a, 80, 16);
        let all = rows.join("\n");
        assert!(
            all.contains("동작") && all.contains("브라우저에서 열기"),
            "{all}"
        );
    }

    #[test]
    fn onboarding_masks_key() {
        let mut a = App::onboarding(false);
        a.handle(Input::Paste("lin_api_secret".into()), T0);
        let (rows, _, _) = screen(&a, 80, 14);
        let all = rows.join("\n");
        assert!(!all.contains("secret"), "{all}");
        assert!(all.contains("••••••••••••••"), "{all}");
    }

    #[test]
    fn rate_limit_shows_briefly_and_status_keeps_refresh_time() {
        let mut a = app();
        a.apply(
            Msg::Failed(crate::linear::client::ApiError::Offline("x".into())),
            T0,
        );
        assert_eq!(status_span(&a, T0).content, "오프라인");
        a.apply(
            Msg::Failed(crate::linear::client::ApiError::RateLimited {
                reset_at_ms: Some(T0 + 120_000),
            }),
            T0,
        );
        assert_eq!(status_span(&a, T0).content, "방금 갱신");
        let (rows, _, _) = screen(&a, 80, 12);
        assert_eq!(
            rows[11],
            " Linear API 한도를 넘었어요. 2분 후 다시 시도하세요"
        );
    }

    #[test]
    fn server_text_cannot_inject_terminal_codes() {
        let mut a = app();
        a.apply(
            Msg::Failed(crate::linear::client::ApiError::GraphQl(
                "나쁜\u{1b}]52;c;eA==\u{7}응답".into(),
            )),
            T0,
        );
        let (rows, _, _) = screen(&a, 100, 12);
        assert!(
            rows.iter()
                .all(|r| !r.contains('\u{1b}') && !r.contains('\u{7}'))
        );
        assert!(rows.last().unwrap().contains("오류: 나쁜"), "{rows:?}");
    }

    #[test]
    fn detail_without_cache_shows_gone_or_failure() {
        let (mut gone, _) = App::start(Some("UTF-8".into()));
        gone.apply(Msg::DetailGone("UTF-8".into()), T0);
        let text = screen(&gone, 80, 12).0.join("\n");
        assert!(
            text.contains("찾을 수 없") && !text.contains("불러오는 중"),
            "{text}"
        );
        let (mut failed, _) = App::start(Some("ENG-9".into()));
        failed.apply(
            Msg::Failed(crate::linear::client::ApiError::Offline("x".into())),
            T0,
        );
        let text = screen(&failed, 80, 12).0.join("\n");
        assert!(
            text.contains("불러오지 못했어요") && !text.contains("불러오는 중"),
            "{text}"
        );
    }

    #[test]
    fn header_shows_estimate_and_due_date() {
        let mut v = IssueBuilder::new("a", "ENG-1", "로그인 버그").json();
        v["estimate"] = serde_json::json!(3.0);
        v["dueDate"] = serde_json::json!("2026-10-31");
        let issue: Issue = serde_json::from_value(v).unwrap();
        let text = markdown::to_plain(&issue_header(&issue, 80));
        assert!(
            text.contains("예상 3") && text.contains("마감 2026-10-31"),
            "{text}"
        );
    }

    #[test]
    fn header_shows_open_pr_only() {
        let open = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .pr(
                "https://github.com/acme/web/pull/15",
                "open",
                15,
                "2026-10-05T00:00:00.000Z",
            )
            .build();
        let text = markdown::to_plain(&issue_header(&open, 80));
        assert!(text.contains("PR #15 열림 · web"), "{text}");
        let merged = IssueBuilder::new("b", "ENG-2", "결제")
            .pr(
                "https://github.com/acme/web/pull/9",
                "merged",
                9,
                "2026-10-01T00:00:00.000Z",
            )
            .build();
        assert!(!markdown::to_plain(&issue_header(&merged, 80)).contains("PR #"));
    }

    #[test]
    fn drawn_reports_click_targets() {
        let (_, drawn, _) = screen(&app(), 120, 16);
        // 목록은 머리·검색 줄 아래 셋째 줄부터
        assert_eq!(drawn.target_at(3, 2), Some(Target::Row(0)));
        assert_eq!(drawn.target_at(3, 3), Some(Target::Row(1)));
        assert_eq!(drawn.target_at(3, 10), None, "줄이 없는 곳");
        let recent = drawn
            .hits
            .iter()
            .find(|h| h.target == Target::Tab(Tab::Recent))
            .expect("최근 본 탭");
        assert_eq!(recent.area.y, 0);
        assert_eq!(
            drawn.target_at(recent.area.x + 1, 0),
            Some(Target::Tab(Tab::Recent))
        );
        // 메뉴가 열리면 메뉴 항목이 목록보다 위에 있다
        let mut a = app();
        a.handle(Input::Menu, T0);
        let (_, drawn, _) = screen(&a, 120, 16);
        let first = drawn
            .hits
            .iter()
            .find(|h| h.target == Target::MenuItem(0))
            .expect("메뉴 첫 항목");
        assert_eq!(
            drawn.target_at(first.area.x + 2, first.area.y),
            Some(Target::MenuItem(0))
        );
    }

    #[test]
    fn flash_replaces_hints() {
        let mut a = app();
        a.apply(Msg::Flash("복사됨: ENG-1".into()), T0);
        let (rows, _, _) = screen(&a, 80, 12);
        assert_eq!(rows[11], " 복사됨: ENG-1");
        a.handle(Input::Act(Act::Back), T0);
    }

    fn numbered(n: usize) -> Vec<Issue> {
        (1..=n)
            .map(|k| {
                IssueBuilder::new(&format!("i{k}"), &format!("ENG-{k}"), &format!("이슈 {k}"))
                    .build()
            })
            .collect()
    }

    /// 이슈 20개. 80×12 화면이면 목록은 셋째 줄부터 9줄이다.
    fn twenty() -> App {
        let (mut a, _) = App::start(None);
        a.apply(
            Msg::Tab {
                tab: Tab::Mine,
                issues: numbered(20),
                fresh: true,
                has_more: false,
                append: false,
            },
            T0,
        );
        a
    }

    /// 한 프레임을 그리고 스크롤 위치를 넘겨준 뒤, 선택 표시(▶)가 있는 화면 줄을 돌려준다.
    fn frame(a: &mut App) -> usize {
        let (rows, drawn, _) = screen(a, 80, 12);
        if let Some(offset) = drawn.list_offset {
            a.list_offset = offset;
        }
        rows.iter()
            .position(|r| r.starts_with('▶'))
            .expect("선택 줄")
    }

    #[test]
    fn list_keeps_its_scroll_position_when_moving_back_up() {
        let mut a = twenty();
        for _ in 0..12 {
            a.handle(Input::Down, T0);
            frame(&mut a);
        }
        assert_eq!(frame(&mut a), 10, "맨 아래 줄");
        a.handle(Input::Up, T0);
        assert_eq!(frame(&mut a), 9, "한 줄 올라간다 (아래에 붙지 않는다)");
    }

    #[test]
    fn shorter_list_after_scrolling_starts_from_the_top() {
        let mut a = twenty();
        a.list_offset = 10;
        a.selected = 15;
        a.apply(
            Msg::Tab {
                tab: Tab::Mine,
                issues: numbered(3),
                fresh: true,
                has_more: false,
                append: false,
            },
            T0,
        );
        let (rows, drawn, _) = screen(&a, 80, 12);
        assert_eq!(drawn.list_offset, Some(0));
        assert!(rows[2].contains("ENG-1"), "{rows:?}");
    }

    fn comment(id: &str, body: &str) -> crate::linear::types::Comment {
        serde_json::from_value(serde_json::json!({
            "id": id, "body": body, "createdAt": "2026-10-02T00:00:00.000Z",
            "editedAt": null, "user": null
        }))
        .unwrap()
    }

    /// 첫 이슈(id "a")의 상세를 열고 서버 응답을 반영한다.
    fn detail_of(issue: Issue, comments: Vec<crate::linear::types::Comment>) -> App {
        let mut a = app();
        a.handle(Input::Enter, T0);
        a.apply(
            Msg::Detail {
                id: "a".into(),
                issue,
                comments,
                more: false,
                fresh: true,
            },
            T0,
        );
        a
    }

    fn link_menu(a: &mut App) -> Vec<String> {
        a.handle(Input::Act(Act::Links), T0);
        a.menu
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|(l, _)| l.clone())
            .collect()
    }

    #[test]
    fn comment_link_numbers_continue_from_the_body() {
        let issue = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .description("[문서](https://x.dev/doc)")
            .build();
        let mut a = detail_of(issue, vec![comment("c1", "[로그](https://x.dev/log)")]);
        let text = markdown::to_plain(&detail_lines(a.detail.as_ref().unwrap(), 80, &[]));
        assert!(
            text.contains("문서 [1]") && text.contains("로그 [2]"),
            "{text}"
        );
        assert_eq!(
            link_menu(&mut a),
            vec![
                "[1] 문서 — https://x.dev/doc",
                "[2] 로그 — https://x.dev/log"
            ]
        );
    }

    #[test]
    fn links_in_comments_only_start_at_one() {
        let issue = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .description("  \n")
            .build();
        let mut a = detail_of(
            issue,
            vec![
                comment("c1", "링크 없음"),
                comment("c2", "![](https://x.dev/a.png)"),
            ],
        );
        let text = markdown::to_plain(&detail_lines(a.detail.as_ref().unwrap(), 80, &[]));
        assert!(text.contains("[이미지 1: a.png]"), "{text}");
        assert_eq!(link_menu(&mut a), vec!["[1] a.png — https://x.dev/a.png"]);
    }
}
