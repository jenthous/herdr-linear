//! README 스크린샷: 가짜 데이터로 팔레트·사이드 pane·상세 화면을 그려 언어마다 SVG로 저장한다.
//! 다시 만들기: `cargo run --example screenshots` (결과: `docs/images/<언어 코드>/*.svg`)

mod data;
mod svg;

use std::path::Path;

use herdr_linear::i18n::{self, Lang};
use herdr_linear::tui::app::{App, Effect, Input, Msg, Tab};
use herdr_linear::tui::view;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

/// 그림 크기(칸).
const W: u16 = 120;
const H: u16 = 30;
/// 사이드 pane 그림에서 왼쪽 작업 pane의 폭.
const SHELL_W: u16 = 56;

fn main() -> std::io::Result<()> {
    // 코멘트 시각이 기기의 시간대에 따라 달라지지 않게 UTC로 그린다. 다른 스레드를 만들기 전이다
    unsafe { std::env::set_var("TZ", "UTC") };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("images");
    for lang in Lang::ALL {
        i18n::set_lang(lang);
        let dir = root.join(lang.code());
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("palette.svg"),
            svg::render(&[(0, palette())], W, H, lang),
        )?;
        std::fs::write(
            dir.join("side.svg"),
            svg::render(&[(0, shell()), (SHELL_W, side())], W, H, lang),
        )?;
        std::fs::write(
            dir.join("detail.svg"),
            svg::render(&[(0, detail())], W, H, lang),
        )?;
        println!("{}", dir.display());
    }
    Ok(())
}

/// 앱을 `w`×`h` 화면에 그린 버퍼.
fn draw(app: &App, w: u16, h: u16) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).expect("test backend");
    term.draw(|f| {
        view::draw(f, app, data::NOW);
    })
    .expect("draw");
    term.backend().buffer().clone()
}

/// 키가 있고, 2분 전에 내 이슈를 받은 팔레트. 현재 브랜치의 이슈가 맨 위에 있다.
fn started() -> App {
    let loaded = data::NOW - 2 * 60_000;
    let (mut app, _) = App::start(None);
    app.apply(Msg::Viewer(data::viewer()), loaded);
    app.apply(Msg::Pinned(Some(data::pinned())), loaded);
    app.apply(
        Msg::Tab {
            tab: Tab::Mine,
            issues: data::mine(),
            fresh: true,
            has_more: false,
            append: false,
        },
        loaded,
    );
    assert_eq!(app.loading, 0, "the first load is finished");
    app
}

/// 팝업 팔레트: 검색 모드, 고른 이슈의 미리 보기.
fn palette() -> Buffer {
    draw(&started(), W, H)
}

/// 사이드 pane: 작업 옆에 목록 모드로 띄워 둔 모습.
fn side() -> Buffer {
    let mut app = started().into_side(60);
    app.handle(Input::Esc, data::NOW);
    draw(&app, W - SHELL_W, H)
}

/// 상세: 현재 브랜치의 이슈를 열고 본문·관계·코멘트를 받은 모습.
fn detail() -> Buffer {
    let mut app = started();
    let effects = app.handle(Input::Enter, data::NOW);
    let id = effects
        .iter()
        .find_map(|e| match e {
            Effect::OpenDetail(id) => Some(id.clone()),
            _ => None,
        })
        .expect("Enter opens the selected issue");
    app.apply(
        Msg::Detail {
            id,
            issue: data::pinned(),
            comments: data::comments(),
            more: false,
            relations: Some(data::relations()),
            fresh: true,
        },
        data::NOW,
    );
    draw(&app, W, H)
}

/// 사이드 pane 옆의 가짜 작업 pane: 셸 몇 줄과 오른쪽 경계선. 글은 모든 언어에서 같다.
fn shell() -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, SHELL_W, H));
    let prompt = Style::new().fg(Color::Cyan);
    let dim = Style::new().fg(Color::DarkGray);
    let ok = Style::new().fg(Color::Green);
    let lines = vec![
        Line::default(),
        Line::styled(" ~/acme/web  alex/eng-142-fix-login-redirect", prompt),
        Line::raw(" $ npm test -- auth"),
        Line::default(),
        Line::styled("  PASS  src/auth/session.test.ts", ok),
        Line::styled("  PASS  src/auth/callback.test.ts", ok),
        Line::styled("  PASS  src/auth/cookies.test.ts", ok),
        Line::default(),
        Line::raw(" Tests:  24 passed, 24 total"),
        Line::styled(" Time:   3.81 s", dim),
        Line::default(),
        Line::styled(" ~/acme/web  alex/eng-142-fix-login-redirect", prompt),
        Line::raw(" $ git push -u origin alex/eng-142-fix-login-redirect"),
        Line::default(),
        Line::styled(" ~/acme/web  alex/eng-142-fix-login-redirect", prompt),
        Line::raw(" $ "),
    ];
    Paragraph::new(lines).render(Rect::new(0, 0, SHELL_W - 1, H), &mut buf);
    let border: Vec<Line> = (0..H).map(|_| Line::styled("│", dim)).collect();
    Paragraph::new(border).render(Rect::new(SHELL_W - 1, 0, 1, H), &mut buf);
    buf
}
