use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::{io, time::Duration};

use crate::types::SearchResult;

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{Terminal, backend::CrosstermBackend};

mod app;
mod events;
mod search;
mod types;
mod ui;

use app::App;

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    let mut app = App::new();

    let mut last_query = String::new();
    let mut rx: Option<mpsc::Receiver<SearchResult>> = None;

    loop {
        terminal.draw(|f| {
            ui::draw(f, &app);
        })?;

        let terminal_size = terminal.size()?;
        let chrome_height = 8;
        let visible_height = terminal_size.height.saturating_sub(chrome_height) as usize;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                events::handle_key_event(key, &mut app, visible_height);
            }
        }

        if app.query != last_query {
            last_query = app.query.clone();
            app.clear_results();
            terminal.clear()?;
            let (tx, new_rx) = mpsc::channel();
            rx = Some(new_rx);

            let pattern = app.query.clone();
            let root = PathBuf::from(".");

            if !pattern.is_empty() {
                thread::spawn(move || {
                    search::search(root, pattern, tx);
                });
            }
        }

        if let Some(receiver) = &rx {
            for result in receiver.try_iter() {
                app.results.push(result);
                app.status = format!("{} matches", app.results.len());
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
