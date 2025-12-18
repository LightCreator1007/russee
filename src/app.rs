use crate::types::SearchResult;

pub struct App {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub status: String,
    pub scroll: usize,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            status: "Type to search, Press ctrl+c to quit".to_string(),
            should_quit: false,
            scroll: 0,
        }
    }

    pub fn clear_results(&mut self) {
        self.results.clear();
        self.selected = 0
    }

    pub fn select_next(&mut self, visible_height: usize) {
        if self.results.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.results.len();
        if self.selected >= self.scroll + visible_height {
            self.scroll += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.results.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.results.len() - 1;
        } else {
            self.selected -= 1;
        }
        if self.selected < self.scroll {
            self.scroll = self.scroll.saturating_sub(1);
        }
    }
}
