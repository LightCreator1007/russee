use crate::app::{App, Mode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

/// Draws the UI and returns the number of result rows visible (the viewport).
pub fn draw(f: &mut Frame, app: &App, preview: &crate::preview::PreviewContent) -> usize {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Input line with a visible caret rendered as a reversed cell.
    let mode_label = match app.mode {
        Mode::File => "Files",
        Mode::Content => {
            if app.regex {
                "Content [regex]"
            } else {
                "Content"
            }
        }
    };
    let input = render_input_line(app);
    f.render_widget(
        Paragraph::new(input).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Search · {mode_label}")),
        ),
        chunks[0],
    );

    let body = chunks[1];
    let show_preview = app.show_preview && body.width >= 80;
    let (list_area, preview_area) = if show_preview {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(body);
        (cols[0], Some(cols[1]))
    } else {
        (body, None)
    };

    let viewport = (list_area.height as usize).saturating_sub(2);

    let rows: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .skip(app.scroll)
        .take(viewport)
        .map(|(i, item)| {
            let text = item.list_label();
            let style = if i == app.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect();

    let title = format!("Results ({})", app.results.len());
    f.render_widget(
        List::new(rows).block(Block::default().borders(Borders::ALL).title(title)),
        list_area,
    );

    if let Some(area) = preview_area {
        render_preview(f, area, preview, app.preview_scroll);
    }

    let case = match app.case_sensitive {
        None => "smart-case",
        Some(true) => "case-sensitive",
        Some(false) => "ignore-case",
    };
    let status_line = format!("{}  ·  {}", app.status, case);
    let status_style = if app.status.contains("error") {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };
    f.render_widget(Paragraph::new(status_line).style(status_style), chunks[2]);

    let help = "↑/↓ move  ·  Enter print  ·  Ctrl+O editor  ·  Alt+C case  ·  Esc quit";
    f.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        chunks[3],
    );

    viewport
}

fn render_input_line(app: &App) -> Line<'static> {
    let chars: Vec<char> = app.query.chars().collect();
    let cursor = app.cursor.min(chars.len());
    let before: String = chars[..cursor].iter().collect();
    let at: String = chars
        .get(cursor)
        .map(|c| c.to_string())
        .unwrap_or_else(|| " ".to_string());
    let after: String = chars
        .get(cursor + 1..)
        .map(|s| s.iter().collect())
        .unwrap_or_default();
    Line::from(vec![
        Span::raw(before),
        Span::styled(at, Style::default().add_modifier(Modifier::REVERSED)),
        Span::raw(after),
    ])
}

fn render_preview(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    preview: &crate::preview::PreviewContent,
    scroll: usize,
) {
    let block = Block::default().borders(Borders::ALL).title("Preview");
    let inner_h = (area.height as usize).saturating_sub(2);

    let body: Vec<Line> = if let Some(msg) = &preview.message {
        vec![Line::from(Span::styled(
            msg.clone(),
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        preview
            .lines
            .iter()
            .enumerate()
            .skip(scroll)
            .take(inner_h)
            .map(|(i, line)| {
                let marker = if preview.match_index == Some(i) {
                    Span::styled("▶ ", Style::default().fg(Color::Yellow))
                } else {
                    Span::raw("  ")
                };
                let mut spans = vec![marker];
                spans.extend(line.spans.iter().cloned());
                Line::from(spans)
            })
            .collect()
    };

    f.render_widget(Paragraph::new(body).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Item;
    use ratatui::{Terminal, backend::TestBackend};
    use std::path::PathBuf;

    fn render(app: &App, w: u16, h: u16) -> String {
        use crate::preview::PreviewContent;
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw(f, app, &PreviewContent::empty());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>()
    }

    #[test]
    fn shows_query_and_results() {
        let mut app = App::new();
        app.query = "par".to_string();
        app.cursor = 3;
        app.results = vec![
            Item::File {
                path: PathBuf::from("src/parser.rs"),
            },
            Item::File {
                path: PathBuf::from("src/lexer.rs"),
            },
        ];
        let out = render(&app, 40, 10);
        assert!(out.contains("par"));
        assert!(out.contains("parser.rs"));
        assert!(out.contains("lexer.rs"));
    }

    #[test]
    fn status_is_rendered() {
        let mut app = App::new();
        app.status = "2/2".to_string();
        let out = render(&app, 40, 10);
        assert!(out.contains("2/2"));
    }

    #[test]
    fn shows_mode_and_regex_in_title() {
        use crate::app::Mode;
        let mut app = App::new();
        app.mode = Mode::Content;
        app.regex = true;
        let out = render(&app, 60, 10);
        assert!(out.contains("Content"));
        assert!(out.contains("regex"));
    }

    #[test]
    fn shows_preview_pane_when_wide() {
        use crate::preview::PreviewContent;
        use ratatui::text::Line;
        let app = App::new();
        let preview = PreviewContent {
            lines: vec![Line::from("fn preview_marker() {}")],
            match_index: Some(0),
            message: None,
            has_wide: false,
        };
        let backend = TestBackend::new(100, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw(f, &app, &preview);
            })
            .unwrap();
        let out = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(out.contains("Preview"));
        assert!(out.contains("preview_marker"));
    }
}
