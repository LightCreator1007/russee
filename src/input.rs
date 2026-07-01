use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Insert(char),
    Backspace,
    Delete,
    DeleteWord,
    ClearLine,
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,
    SelectNext,
    SelectPrev,
    PageDown,
    PageUp,
    SwitchMode,
    ToggleRegex,
    ToggleCase,
    Accept,
    OpenInEditor,
    TogglePreview,
    PreviewUp,
    PreviewDown,
    Quit,
    None,
}

pub fn map_key(key: KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    // Ctrl/Alt chords first.
    if ctrl {
        return match key.code {
            KeyCode::Char('c') | KeyCode::Char('q') => Action::Quit,
            KeyCode::Char('o') => Action::OpenInEditor,
            KeyCode::Char('t') => Action::TogglePreview,
            KeyCode::Char('r') => Action::ToggleRegex,
            KeyCode::Char('w') => Action::DeleteWord,
            KeyCode::Char('u') => Action::ClearLine,
            KeyCode::Char('a') => Action::MoveHome,
            KeyCode::Char('e') => Action::MoveEnd,
            KeyCode::Char('p') | KeyCode::Char('k') => Action::SelectPrev,
            KeyCode::Char('n') | KeyCode::Char('j') => Action::SelectNext,
            _ => Action::None,
        };
    }
    if alt {
        return match key.code {
            // Case toggle on Alt+C to avoid the legacy Tab==Ctrl+I collision.
            KeyCode::Char('c') | KeyCode::Char('C') => Action::ToggleCase,
            KeyCode::Up => Action::PreviewUp,
            KeyCode::Down => Action::PreviewDown,
            _ => Action::None,
        };
    }

    match key.code {
        KeyCode::Esc => Action::Quit,
        KeyCode::Enter => Action::Accept,
        KeyCode::Tab => Action::SwitchMode,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Delete => Action::Delete,
        KeyCode::Left => Action::MoveLeft,
        KeyCode::Right => Action::MoveRight,
        KeyCode::Home => Action::MoveHome,
        KeyCode::End => Action::MoveEnd,
        KeyCode::Up => Action::SelectPrev,
        KeyCode::Down => Action::SelectNext,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        // Accepts SHIFT — capitals and shifted symbols insert correctly.
        KeyCode::Char(c) => Action::Insert(c),
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn plain_letter_inserts() {
        assert_eq!(
            map_key(key(KeyCode::Char('a'), KeyModifiers::NONE)),
            Action::Insert('a')
        );
    }

    #[test]
    fn shifted_capital_inserts() {
        assert_eq!(
            map_key(key(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            Action::Insert('A')
        );
    }

    #[test]
    fn shifted_symbol_inserts() {
        assert_eq!(
            map_key(key(KeyCode::Char('_'), KeyModifiers::SHIFT)),
            Action::Insert('_')
        );
        assert_eq!(
            map_key(key(KeyCode::Char('('), KeyModifiers::SHIFT)),
            Action::Insert('(')
        );
    }

    #[test]
    fn ctrl_chords_map() {
        assert_eq!(
            map_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
        assert_eq!(
            map_key(key(KeyCode::Char('o'), KeyModifiers::CONTROL)),
            Action::OpenInEditor
        );
        assert_eq!(
            map_key(key(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            Action::DeleteWord
        );
        assert_eq!(
            map_key(key(KeyCode::Char('u'), KeyModifiers::CONTROL)),
            Action::ClearLine
        );
        assert_eq!(
            map_key(key(KeyCode::Char('p'), KeyModifiers::CONTROL)),
            Action::SelectPrev
        );
        assert_eq!(
            map_key(key(KeyCode::Char('n'), KeyModifiers::CONTROL)),
            Action::SelectNext
        );
        assert_eq!(
            map_key(key(KeyCode::Char('j'), KeyModifiers::CONTROL)),
            Action::SelectNext
        );
        assert_eq!(
            map_key(key(KeyCode::Char('k'), KeyModifiers::CONTROL)),
            Action::SelectPrev
        );
    }

    #[test]
    fn alt_c_toggles_case() {
        assert_eq!(
            map_key(key(KeyCode::Char('c'), KeyModifiers::ALT)),
            Action::ToggleCase
        );
    }

    #[test]
    fn ctrl_t_toggles_preview() {
        assert_eq!(
            map_key(key(KeyCode::Char('t'), KeyModifiers::CONTROL)),
            Action::TogglePreview
        );
    }

    #[test]
    fn alt_arrows_scroll_preview() {
        assert_eq!(
            map_key(key(KeyCode::Up, KeyModifiers::ALT)),
            Action::PreviewUp
        );
        assert_eq!(
            map_key(key(KeyCode::Down, KeyModifiers::ALT)),
            Action::PreviewDown
        );
    }

    #[test]
    fn navigation_and_control_keys() {
        assert_eq!(
            map_key(key(KeyCode::Enter, KeyModifiers::NONE)),
            Action::Accept
        );
        assert_eq!(
            map_key(key(KeyCode::Tab, KeyModifiers::NONE)),
            Action::SwitchMode
        );
        assert_eq!(map_key(key(KeyCode::Esc, KeyModifiers::NONE)), Action::Quit);
        assert_eq!(
            map_key(key(KeyCode::Up, KeyModifiers::NONE)),
            Action::SelectPrev
        );
        assert_eq!(
            map_key(key(KeyCode::Down, KeyModifiers::NONE)),
            Action::SelectNext
        );
    }
}
