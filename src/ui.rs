use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;

// ui.rs
pub fn draw(f: &mut Frame, app: &App) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Search
            Constraint::Min(1),    // Results
            Constraint::Length(1), // Status
        ])
        .split(size);

    // 1. Render Search Input (this overwrites the old one fine)
    let input = Paragraph::new(app.query.as_str())
        .block(Block::default().borders(Borders::ALL).title("Search"));
    f.render_widget(input, chunks[0]);

    // 2. Render Results
    let visible_height = chunks[1].height.saturating_sub(2) as usize;

    // Map ALL results, but we only show the slice
    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .skip(app.scroll)
        .take(visible_height)
        .map(|(i, r)| {
            let real_index = app.scroll + i;
            let content = format!("{}:{} {}", r.path.display(), r.line, r.text);
            let mut item = ListItem::new(content);

            if real_index == app.selected {
                item = item.style(Style::default().add_modifier(Modifier::REVERSED));
            }
            item
        })
        .collect();

    // IMPORTANT: Even if items is empty, we MUST render the List widget.
    // This draws the "box" and clears the interior area.
    let results = List::new(items).block(Block::default().borders(Borders::ALL).title("Results"));

    f.render_widget(results, chunks[1]);

    // 3. Render Status (This also needs to be a Paragraph to clear the line)
    let status = Paragraph::new(app.status.as_str());
    f.render_widget(status, chunks[2]);
}
