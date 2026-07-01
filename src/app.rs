use crate::input::Action;
use crate::types::Item;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    File,
    Content,
}

pub struct App {
    pub query: String,
    pub cursor: usize,
    pub results: Vec<Item>,
    pub selected: usize,
    pub scroll: usize,
    pub viewport: usize,
    pub status: String,
    pub should_quit: bool,
    pub case_sensitive: Option<bool>,
    pub accept: Option<String>,
    pub pending_open: Option<(PathBuf, Option<usize>)>,
    pub query_dirty: bool,
    pub mode: Mode,
    pub regex: bool,
    pub show_preview: bool,
    pub preview_scroll: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            results: Vec::new(),
            selected: 0,
            scroll: 0,
            viewport: 0,
            status: String::new(),
            should_quit: false,
            case_sensitive: None,
            accept: None,
            pending_open: None,
            query_dirty: false,
            mode: Mode::File,
            regex: false,
            show_preview: true,
            preview_scroll: 0,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn insert_char(&mut self, c: char) {
        let mut chars: Vec<char> = self.query.chars().collect();
        let idx = self.cursor.min(chars.len());
        chars.insert(idx, c);
        self.cursor = idx + 1;
        self.query = chars.into_iter().collect();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut chars: Vec<char> = self.query.chars().collect();
        chars.remove(self.cursor - 1);
        self.cursor -= 1;
        self.query = chars.into_iter().collect();
    }

    pub fn delete(&mut self) {
        let mut chars: Vec<char> = self.query.chars().collect();
        if self.cursor < chars.len() {
            chars.remove(self.cursor);
            self.query = chars.into_iter().collect();
        }
    }

    pub fn delete_word(&mut self) {
        let chars: Vec<char> = self.query.chars().collect();
        let mut start = self.cursor.min(chars.len());
        while start > 0 && chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        let mut next: Vec<char> = Vec::with_capacity(chars.len());
        next.extend_from_slice(&chars[..start]);
        next.extend_from_slice(&chars[self.cursor.min(chars.len())..]);
        self.cursor = start;
        self.query = next.into_iter().collect();
    }

    pub fn clear_line(&mut self) {
        self.query.clear();
        self.cursor = 0;
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        let len = self.query.chars().count();
        if self.cursor < len {
            self.cursor += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.query.chars().count();
    }

    pub fn set_viewport(&mut self, height: usize) {
        self.viewport = height;
        self.ensure_visible();
    }

    pub fn reset_selection(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn select_next(&mut self) {
        if self.results.is_empty() {
            return;
        }
        if self.selected + 1 < self.results.len() {
            self.selected += 1;
        }
        self.ensure_visible();
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.ensure_visible();
    }

    pub fn page_down(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let step = self.viewport.max(1);
        self.selected = (self.selected + step).min(self.results.len() - 1);
        self.ensure_visible();
    }

    pub fn page_up(&mut self) {
        let step = self.viewport.max(1);
        self.selected = self.selected.saturating_sub(step);
        self.ensure_visible();
    }

    pub fn on_results_changed(&mut self, new_results: Vec<Item>) {
        self.results = new_results;
        if self.results.is_empty() {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        if self.selected >= self.results.len() {
            self.selected = self.results.len() - 1;
        }
        self.ensure_visible();
    }

    /// Single source of truth for keeping `selected` inside the scroll window.
    fn ensure_visible(&mut self) {
        if self.viewport == 0 {
            self.scroll = 0;
            return;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + self.viewport {
            self.scroll = self.selected + 1 - self.viewport;
        }
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Insert(c) => {
                self.insert_char(c);
                self.mark_query_changed();
            }
            Action::Backspace => {
                self.backspace();
                self.mark_query_changed();
            }
            Action::Delete => {
                self.delete();
                self.mark_query_changed();
            }
            Action::DeleteWord => {
                self.delete_word();
                self.mark_query_changed();
            }
            Action::ClearLine => {
                self.clear_line();
                self.mark_query_changed();
            }
            Action::MoveLeft => self.move_left(),
            Action::MoveRight => self.move_right(),
            Action::MoveHome => self.move_home(),
            Action::MoveEnd => self.move_end(),
            Action::SelectNext => self.select_next(),
            Action::SelectPrev => self.select_prev(),
            Action::PageDown => self.page_down(),
            Action::PageUp => self.page_up(),
            Action::ToggleCase => {
                self.case_sensitive = match self.case_sensitive {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
                self.query_dirty = true;
            }
            Action::Accept => {
                if let Some(item) = self.results.get(self.selected) {
                    self.accept = Some(item.selection_string());
                    self.should_quit = true;
                }
            }
            Action::OpenInEditor => {
                if let Some(item) = self.results.get(self.selected) {
                    self.pending_open = Some(item.open_target());
                }
            }
            Action::Quit => self.should_quit = true,
            Action::SwitchMode => {
                self.mode = match self.mode {
                    Mode::File => Mode::Content,
                    Mode::Content => Mode::File,
                };
                self.query_dirty = true;
                self.reset_selection();
            }
            Action::ToggleRegex => {
                self.regex = !self.regex;
                self.query_dirty = true;
            }
            Action::TogglePreview => self.show_preview = !self.show_preview,
            Action::PreviewUp => self.preview_scroll = self.preview_scroll.saturating_sub(1),
            Action::PreviewDown => self.preview_scroll = self.preview_scroll.saturating_add(1),
            Action::None => {}
        }
    }

    fn mark_query_changed(&mut self) {
        self.query_dirty = true;
        self.reset_selection();
    }
}

#[cfg(test)]
mod apply_tests {
    use super::*;
    use crate::input::Action;
    use crate::types::Item;
    use std::path::PathBuf;

    fn app_with_results() -> App {
        let mut a = App::new();
        a.results = vec![
            Item::File {
                path: PathBuf::from("a.rs"),
            },
            Item::File {
                path: PathBuf::from("b.rs"),
            },
        ];
        a.set_viewport(10);
        a
    }

    #[test]
    fn insert_marks_dirty_and_resets_selection() {
        let mut a = app_with_results();
        a.selected = 1;
        a.apply(Action::Insert('x'));
        assert_eq!(a.query, "x");
        assert!(a.query_dirty);
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn accept_prints_selected() {
        let mut a = app_with_results();
        a.selected = 1;
        a.apply(Action::Accept);
        assert_eq!(a.accept.as_deref(), Some("b.rs"));
        assert!(a.should_quit);
    }

    #[test]
    fn accept_with_no_results_does_nothing() {
        let mut a = App::new();
        a.apply(Action::Accept);
        assert!(a.accept.is_none());
        assert!(!a.should_quit);
    }

    #[test]
    fn open_sets_pending_open() {
        let mut a = app_with_results();
        a.selected = 0;
        a.apply(Action::OpenInEditor);
        assert_eq!(a.pending_open, Some((PathBuf::from("a.rs"), None)));
        assert!(!a.should_quit, "opening must not quit the app");
    }

    #[test]
    fn toggle_case_cycles() {
        let mut a = App::new();
        assert_eq!(a.case_sensitive, None);
        a.apply(Action::ToggleCase);
        assert_eq!(a.case_sensitive, Some(true));
        a.apply(Action::ToggleCase);
        assert_eq!(a.case_sensitive, Some(false));
        a.apply(Action::ToggleCase);
        assert_eq!(a.case_sensitive, None);
    }

    #[test]
    fn nav_does_not_mark_dirty() {
        let mut a = app_with_results();
        a.apply(Action::SelectNext);
        assert_eq!(a.selected, 1);
        assert!(!a.query_dirty);
    }
}

#[cfg(test)]
mod mode_tests {
    use super::*;
    use crate::input::Action;

    #[test]
    fn switch_mode_toggles_and_marks_dirty() {
        let mut a = App::new();
        assert_eq!(a.mode, Mode::File);
        a.apply(Action::SwitchMode);
        assert_eq!(a.mode, Mode::Content);
        assert!(a.query_dirty);
        a.query_dirty = false;
        a.apply(Action::SwitchMode);
        assert_eq!(a.mode, Mode::File);
        assert!(a.query_dirty);
    }

    #[test]
    fn toggle_regex_flips_and_marks_dirty() {
        let mut a = App::new();
        assert!(!a.regex);
        a.apply(Action::ToggleRegex);
        assert!(a.regex);
        assert!(a.query_dirty);
    }

    #[test]
    fn toggle_preview_flips() {
        let mut a = App::new();
        assert!(a.show_preview);
        a.apply(Action::TogglePreview);
        assert!(!a.show_preview);
    }

    #[test]
    fn preview_scroll_saturates_at_zero() {
        let mut a = App::new();
        a.apply(Action::PreviewUp);
        assert_eq!(a.preview_scroll, 0);
        a.apply(Action::PreviewDown);
        assert_eq!(a.preview_scroll, 1);
    }
}

#[cfg(test)]
mod scroll_tests {
    use super::*;
    use crate::types::Item;
    use std::path::PathBuf;

    fn items(n: usize) -> Vec<Item> {
        (0..n)
            .map(|i| Item::File {
                path: PathBuf::from(format!("f{i}")),
            })
            .collect()
    }

    fn app_with(n: usize, viewport: usize) -> App {
        let mut a = App::new();
        a.results = items(n);
        a.set_viewport(viewport);
        a
    }

    #[test]
    fn next_clamps_at_bottom_no_wrap() {
        let mut a = app_with(3, 10);
        a.selected = 2;
        a.select_next();
        assert_eq!(a.selected, 2, "must not wrap to 0");
    }

    #[test]
    fn prev_clamps_at_top_no_wrap() {
        let mut a = app_with(3, 10);
        a.selected = 0;
        a.select_prev();
        assert_eq!(a.selected, 0, "must not wrap to last");
    }

    #[test]
    fn scroll_follows_selection_down() {
        let mut a = app_with(100, 5);
        for _ in 0..5 {
            a.select_next();
        }
        assert_eq!(a.selected, 5);
        assert!(a.scroll <= a.selected && a.selected < a.scroll + a.viewport);
    }

    #[test]
    fn scroll_follows_selection_up() {
        let mut a = app_with(100, 5);
        a.selected = 50;
        a.scroll = 46;
        a.select_prev();
        for _ in 0..10 {
            a.select_prev();
        }
        assert!(a.scroll <= a.selected);
    }

    #[test]
    fn page_down_and_up_stay_in_bounds() {
        let mut a = app_with(20, 5);
        a.page_down();
        assert!(a.selected < a.results.len());
        assert!(a.scroll <= a.selected && a.selected < a.scroll + a.viewport);
        a.page_up();
        assert!(a.scroll <= a.selected);
    }

    #[test]
    fn results_change_clamps_selection() {
        let mut a = app_with(100, 5);
        a.selected = 80;
        a.scroll = 76;
        a.on_results_changed(items(10));
        assert!(a.selected < 10);
        assert!(a.scroll <= a.selected && a.selected < a.scroll + a.viewport);
    }

    #[test]
    fn empty_results_reset_to_zero() {
        let mut a = app_with(5, 5);
        a.selected = 3;
        a.on_results_changed(Vec::new());
        assert_eq!(a.selected, 0);
        assert_eq!(a.scroll, 0);
    }

    #[test]
    fn resize_smaller_keeps_selection_visible() {
        let mut a = app_with(100, 20);
        a.selected = 18;
        a.set_viewport(20);
        a.set_viewport(5);
        assert!(a.scroll <= a.selected && a.selected < a.scroll + a.viewport);
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    fn app_with(query: &str, cursor: usize) -> App {
        let mut a = App::new();
        a.query = query.to_string();
        a.cursor = cursor;
        a
    }

    #[test]
    fn insert_at_cursor_advances() {
        let mut a = app_with("ac", 1);
        a.insert_char('b');
        assert_eq!(a.query, "abc");
        assert_eq!(a.cursor, 2);
    }

    #[test]
    fn insert_accepts_uppercase_and_symbols() {
        let mut a = App::new();
        for c in "Foo_(x)".chars() {
            a.insert_char(c);
        }
        assert_eq!(a.query, "Foo_(x)");
        assert_eq!(a.cursor, 7);
    }

    #[test]
    fn backspace_removes_char_before_cursor() {
        let mut a = app_with("abc", 2);
        a.backspace();
        assert_eq!(a.query, "ac");
        assert_eq!(a.cursor, 1);
    }

    #[test]
    fn backspace_at_start_is_noop() {
        let mut a = app_with("abc", 0);
        a.backspace();
        assert_eq!(a.query, "abc");
        assert_eq!(a.cursor, 0);
    }

    #[test]
    fn delete_removes_char_at_cursor() {
        let mut a = app_with("abc", 1);
        a.delete();
        assert_eq!(a.query, "ac");
        assert_eq!(a.cursor, 1);
    }

    #[test]
    fn delete_word_removes_trailing_spaces_and_word() {
        let mut a = app_with("foo bar ", 8);
        a.delete_word();
        assert_eq!(a.query, "foo ");
        assert_eq!(a.cursor, 4);
    }

    #[test]
    fn clear_line_empties_query() {
        let mut a = app_with("abc", 3);
        a.clear_line();
        assert_eq!(a.query, "");
        assert_eq!(a.cursor, 0);
    }

    #[test]
    fn cursor_moves_clamp_at_ends() {
        let mut a = app_with("ab", 1);
        a.move_left();
        assert_eq!(a.cursor, 0);
        a.move_left();
        assert_eq!(a.cursor, 0);
        a.move_end();
        assert_eq!(a.cursor, 2);
        a.move_right();
        assert_eq!(a.cursor, 2);
        a.move_home();
        assert_eq!(a.cursor, 0);
    }

    #[test]
    fn editing_is_char_safe_for_multibyte() {
        let mut a = app_with("áé", 2);
        a.backspace();
        assert_eq!(a.query, "á");
        assert_eq!(a.cursor, 1);
    }
}
