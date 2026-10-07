//! 키 입력 → [`Input`]. 한글 입력 중에도 단축키가 먹도록 두벌식 자모를 영문 키로 바꾼다.

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use super::app::{Act, Input, Mode};
use super::view::{Drawn, Target};

/// 두벌식 자모 → 같은 자리의 영문 키. 자모가 아니면 그대로.
pub fn jamo_to_latin(c: char) -> char {
    match c {
        'ㅂ' => 'q',
        'ㅈ' => 'w',
        'ㄷ' => 'e',
        'ㄱ' => 'r',
        'ㅅ' => 't',
        'ㅛ' => 'y',
        'ㅕ' => 'u',
        'ㅑ' => 'i',
        'ㅐ' => 'o',
        'ㅔ' => 'p',
        'ㅁ' => 'a',
        'ㄴ' => 's',
        'ㅇ' => 'd',
        'ㄹ' => 'f',
        'ㅎ' => 'g',
        'ㅗ' => 'h',
        'ㅓ' => 'j',
        'ㅏ' => 'k',
        'ㅣ' => 'l',
        'ㅋ' => 'z',
        'ㅌ' => 'x',
        'ㅊ' => 'c',
        'ㅍ' => 'v',
        'ㅠ' => 'b',
        'ㅜ' => 'n',
        'ㅡ' => 'm',
        'ㅃ' => 'Q',
        'ㅉ' => 'W',
        'ㄸ' => 'E',
        'ㄲ' => 'R',
        'ㅆ' => 'T',
        'ㅒ' => 'O',
        'ㅖ' => 'P',
        other => other,
    }
}

/// 키 하나를 해석한다. `menu_open`이면 메뉴 입력으로 본다. 쓰지 않는 키면 `None`.
pub fn translate(mode: Mode, menu_open: bool, key: KeyEvent) -> Option<Input> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        let detail = mode == Mode::Detail && !menu_open;
        return match key.code {
            KeyCode::Char('c') => Some(Input::Quit),
            KeyCode::Char('k') => Some(Input::Menu),
            KeyCode::Char('p') => Some(Input::Up),
            KeyCode::Char('n') => Some(Input::Down),
            KeyCode::Char('d') if detail => Some(Input::PageDown),
            KeyCode::Char('u') if detail => Some(Input::PageUp),
            KeyCode::Char('u') => Some(Input::ClearLine),
            _ => None,
        };
    }
    match key.code {
        KeyCode::Up => return Some(Input::Up),
        KeyCode::Down => return Some(Input::Down),
        KeyCode::PageUp => return Some(Input::PageUp),
        KeyCode::PageDown => return Some(Input::PageDown),
        KeyCode::Home => return Some(Input::Top),
        KeyCode::End => return Some(Input::Bottom),
        KeyCode::Enter => return Some(Input::Enter),
        KeyCode::Esc => return Some(Input::Esc),
        KeyCode::Backspace => return Some(Input::Backspace),
        KeyCode::Tab if !menu_open => return Some(Input::NextTab),
        KeyCode::BackTab if !menu_open => return Some(Input::PrevTab),
        _ => {}
    }
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    // 글자가 그대로 입력되는 곳: 검색창, 키 입력, 메뉴 거르기
    if menu_open || matches!(mode, Mode::Search | Mode::Onboarding) {
        return Some(Input::Char(c));
    }
    // 목록·상세: 한 글자 동작
    let act = |a| Some(Input::Act(a));
    match (mode, jamo_to_latin(c)) {
        (_, 'j') => Some(Input::Down),
        (_, 'k') => Some(Input::Up),
        (_, 'g') => Some(Input::Top),
        (_, 'G') => Some(Input::Bottom),
        (_, 'o') => act(Act::Browser),
        (_, 'y') => act(Act::CopyUrl),
        (_, 'Y') => act(Act::CopyPr),
        (_, 'r') => act(Act::Refresh),
        (_, 'q') => Some(Input::Esc),
        (Mode::List, '/') => Some(Input::Search),
        (Mode::Detail, 'u') => act(Act::Links),
        _ => None,
    }
}

/// 마우스 이벤트를 `Input`으로 바꾼다. 누른 곳은 마지막으로 그린 화면(`Drawn`)에서 찾는다.
pub fn mouse(drawn: &Drawn, ev: MouseEvent) -> Option<Input> {
    match ev.kind {
        MouseEventKind::ScrollUp => Some(Input::ScrollUp),
        MouseEventKind::ScrollDown => Some(Input::ScrollDown),
        MouseEventKind::Down(MouseButton::Left) => {
            drawn
                .target_at(ev.column, ev.row)
                .map(|target| match target {
                    Target::Row(i) => Input::ClickRow(i),
                    Target::Tab(tab) => Input::ClickTab(tab),
                    Target::MenuItem(i) => Input::ClickMenu(i),
                })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn ctrl_keys_work_in_every_mode() {
        for mode in [Mode::Search, Mode::List, Mode::Detail, Mode::Onboarding] {
            assert_eq!(translate(mode, false, ctrl('c')), Some(Input::Quit));
            assert_eq!(translate(mode, false, ctrl('k')), Some(Input::Menu));
            assert_eq!(translate(mode, false, ctrl('n')), Some(Input::Down));
        }
        assert_eq!(
            translate(Mode::Search, false, ctrl('u')),
            Some(Input::ClearLine)
        );
        assert_eq!(
            translate(Mode::Detail, false, ctrl('u')),
            Some(Input::PageUp)
        );
        assert_eq!(
            translate(Mode::Detail, false, ctrl('d')),
            Some(Input::PageDown)
        );
    }

    #[test]
    fn search_mode_types_letters_including_korean() {
        assert_eq!(
            translate(Mode::Search, false, key(KeyCode::Char('j'))),
            Some(Input::Char('j'))
        );
        assert_eq!(
            translate(Mode::Search, false, key(KeyCode::Char('로'))),
            Some(Input::Char('로'))
        );
        assert_eq!(
            translate(Mode::Search, false, key(KeyCode::Tab)),
            Some(Input::NextTab)
        );
    }

    #[test]
    fn list_mode_letters_are_actions_and_jamo_maps() {
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('j'))),
            Some(Input::Down)
        );
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('ㅓ'))),
            Some(Input::Down)
        );
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('ㅛ'))),
            Some(Input::Act(Act::CopyUrl))
        );
        assert_eq!(
            translate(
                Mode::List,
                false,
                KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT)
            ),
            Some(Input::Act(Act::CopyPr))
        );
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('/'))),
            Some(Input::Search)
        );
        assert_eq!(
            translate(Mode::List, false, key(KeyCode::Char('ㅂ'))),
            Some(Input::Esc)
        );
        assert_eq!(translate(Mode::List, false, key(KeyCode::Char('u'))), None);
    }

    #[test]
    fn detail_mode_has_links_key() {
        assert_eq!(
            translate(Mode::Detail, false, key(KeyCode::Char('u'))),
            Some(Input::Act(Act::Links))
        );
        assert_eq!(
            translate(Mode::Detail, false, key(KeyCode::Char('/'))),
            None
        );
    }

    #[test]
    fn menu_takes_letters_as_filter() {
        assert_eq!(
            translate(Mode::List, true, key(KeyCode::Char('j'))),
            Some(Input::Char('j'))
        );
        assert_eq!(translate(Mode::List, true, key(KeyCode::Tab)), None);
    }

    #[test]
    fn mouse_wheel_and_clicks_become_inputs() {
        use crate::tui::app::Tab;
        use crate::tui::view::{Drawn, Hit, Target};
        use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;
        let drawn = Drawn {
            hits: vec![
                Hit {
                    area: Rect::new(0, 2, 40, 1),
                    target: Target::Row(0),
                },
                Hit {
                    area: Rect::new(9, 0, 9, 1),
                    target: Target::Tab(Tab::Mine),
                },
                // 나중에 그린 메뉴가 위에 있다
                Hit {
                    area: Rect::new(10, 2, 20, 1),
                    target: Target::MenuItem(0),
                },
            ],
            ..Drawn::default()
        };
        let ev = |kind, column, row| MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        let left = MouseEventKind::Down(MouseButton::Left);
        assert_eq!(
            mouse(&drawn, ev(MouseEventKind::ScrollDown, 5, 5)),
            Some(Input::ScrollDown)
        );
        assert_eq!(
            mouse(&drawn, ev(MouseEventKind::ScrollUp, 5, 5)),
            Some(Input::ScrollUp)
        );
        assert_eq!(mouse(&drawn, ev(left, 3, 2)), Some(Input::ClickRow(0)));
        assert_eq!(mouse(&drawn, ev(left, 12, 2)), Some(Input::ClickMenu(0)));
        assert_eq!(
            mouse(&drawn, ev(left, 10, 0)),
            Some(Input::ClickTab(Tab::Mine))
        );
        assert_eq!(mouse(&drawn, ev(left, 3, 9)), None, "빈 곳");
        assert_eq!(
            mouse(&drawn, ev(MouseEventKind::Down(MouseButton::Right), 3, 2)),
            None
        );
        assert_eq!(mouse(&drawn, ev(MouseEventKind::Moved, 3, 2)), None);
    }

    #[test]
    fn key_release_is_ignored() {
        let mut ev = key(KeyCode::Char('j'));
        ev.kind = KeyEventKind::Release;
        assert_eq!(translate(Mode::List, false, ev), None);
    }
}
