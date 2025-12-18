use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key_event(key: KeyEvent, app: &mut App, visible_height: usize) {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }

        (KeyCode::Char(c), KeyModifiers::NONE) => {
            app.query.push(c);
            app.clear_results();
            app.status = "Searching...".to_string();
        }

        (KeyCode::Backspace, KeyModifiers::NONE) => {
            app.query.pop();
            app.clear_results();
            app.status = "Searching...".to_string();
        }

        (KeyCode::Up, KeyModifiers::NONE) => {
            app.select_prev();
        }

        (KeyCode::Down, KeyModifiers::NONE) => {
            app.select_next(visible_height);
        }

        _ => {}
    }
}
