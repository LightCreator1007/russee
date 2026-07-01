mod app;
mod args;
mod config;
mod editor;
mod engine;
mod input;
mod preview;
mod query;
mod types;
mod ui;

use std::io::{self, Stdout};
use std::process::Command;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::App;
use engine::content::ContentEngine;
use engine::files::FileEngine;
use input::map_key;
use preview::{PreviewContent, PreviewEngine};

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}

fn main() -> anyhow::Result<()> {
    // Restore the terminal even if a later panic unwinds through the draw loop.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    let cli = args::parse(std::env::args().skip(1));

    if let Some(query) = cli.cli_query.clone() {
        return run_headless(cli, &query);
    }

    let cfg = config::load();

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let outcome = run(&mut terminal, cli, cfg);

    restore_terminal();
    let _ = terminal.show_cursor();

    match outcome {
        // Printed AFTER the alt-screen is gone, so it lands on the real terminal / pipe.
        Ok(Some(selection)) => {
            println!("{selection}");
            std::process::exit(0);
        }
        Ok(None) => std::process::exit(1),
        Err(e) => Err(e),
    }
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    cli: args::CliArgs,
    cfg: config::Config,
) -> anyhow::Result<Option<String>> {
    let mut app = App::new();
    let mut file_engine = FileEngine::new(cli.root.clone(), cli.filters.clone())?;
    let mut content_engine = ContentEngine::new(cli.root, cli.filters);
    let theme = preview::resolve_theme(cfg.theme.as_deref(), config::themes_dir().as_deref());
    let mut preview_engine = PreviewEngine::with_theme(theme);
    let mut current_preview = PreviewContent::empty();
    let mut last_preview_key: Option<(std::path::PathBuf, usize)> = None;
    let mut last_viewport = 0usize;
    let mut pending: Option<(String, bool, Instant)> = None;
    const DEBOUNCE: Duration = Duration::from_millis(120);

    loop {
        let mut viewport = last_viewport;

        // Recompute the preview only when the selected item changes.
        let key = app.results.get(app.selected).map(|item| match item {
            types::Item::File { path } => (path.clone(), 0usize),
            types::Item::Content { path, line, .. } => (path.clone(), *line),
        });
        let mut force_redraw = false;
        if key != last_preview_key {
            // A wide glyph can render past ratatui's width model into the pane's
            // border column; the diff renderer never reclaims it, so the old
            // file's tail ghosts behind the next preview. Repaint from scratch
            // when leaving such a preview. Pure-ASCII previews can't desync, so
            // typing through ASCII results stays flicker-free.
            force_redraw = current_preview.has_wide;
            current_preview = match app.results.get(app.selected) {
                Some(item) => preview_engine.render(item),
                None => PreviewContent::empty(),
            };
            // Center the matched line in the preview pane (its height == the list viewport).
            app.preview_scroll = preview::centered_scroll(current_preview.match_index, viewport);
            last_preview_key = key;
        }

        if force_redraw {
            terminal.clear()?;
        }
        terminal.draw(|f| {
            viewport = ui::draw(f, &app, &current_preview);
        })?;
        last_viewport = viewport;
        app.set_viewport(viewport);

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            app.apply(map_key(key));
        }

        match app.mode {
            app::Mode::File => {
                if app.query_dirty {
                    file_engine.set_query(&app.query, app.case_sensitive);
                    app.query_dirty = false;
                    pending = None;
                }
                // Full matched set up to a cap keeps selection/scroll math trivial;
                // windowing is a Phase 3 optimization.
                let _running = file_engine.tick();
                let (items, matched, total) = file_engine.snapshot(0, 50_000);
                app.on_results_changed(items);
                app.status = format!("{matched}/{total}");
            }
            app::Mode::Content => {
                if app.query_dirty {
                    pending = Some((app.query.clone(), app.regex, Instant::now()));
                    app.query_dirty = false;
                }
                let fire = matches!(&pending, Some((_, _, t)) if t.elapsed() >= DEBOUNCE);
                if fire && let Some((q, regex, _)) = pending.take() {
                    content_engine.set_query(&q, regex, app.case_sensitive);
                }
                let _running = content_engine.poll();
                app.on_results_changed(content_engine.results().to_vec());
                app.status = match content_engine.error() {
                    Some(e) => format!("regex error: {e}"),
                    None => {
                        let n = content_engine.results().len();
                        if content_engine.capped() {
                            format!("{n}+ (capped — refine query)")
                        } else {
                            format!("{n} matches")
                        }
                    }
                };
            }
        }

        if let Some((path, line)) = app.pending_open.take() {
            open_in_editor(terminal, &cfg, &path, line)?;
        }
        if app.should_quit {
            return Ok(app.accept.take());
        }
    }
}

fn run_headless(cli: args::CliArgs, query: &str) -> anyhow::Result<()> {
    use std::io::Write;

    let mut engine = ContentEngine::new(cli.root, cli.filters);
    engine.set_query(query, false, cli.case_sensitive);

    // Drive to completion.
    while engine.poll() {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    if let Some(e) = engine.error() {
        anyhow::bail!("regex error: {e}");
    }

    let mut out = std::io::stdout().lock();
    for item in engine.results() {
        writeln!(out, "{}", item.list_label())?;
    }
    Ok(())
}

fn open_in_editor(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    cfg: &config::Config,
    path: &std::path::Path,
    line: Option<usize>,
) -> anyhow::Result<()> {
    let resolved = editor::resolve_editor(cfg, std::env::var("EDITOR").ok());
    let argv = editor::build_editor_argv(&resolved, &path.to_string_lossy(), line);
    let Some((prog, args)) = argv.split_first() else {
        return Ok(());
    };

    // Suspend the TUI so the editor owns the terminal.
    restore_terminal();
    let _ = Command::new(prog).args(args).status();

    // Resume the TUI.
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(())
}
