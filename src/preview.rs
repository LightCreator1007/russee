use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use two_face::theme::EmbeddedThemeName;

use crate::types::Item;

pub const PREVIEW_CONTEXT: usize = 40;
const PREVIEW_MAX_LINE: usize = 1000;
const CACHE_CAP: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub lines: Vec<String>,
    pub start_line: usize,
    pub match_index: Option<usize>,
    pub message: Option<String>,
}

pub(crate) fn syntect_to_ratatui_color(c: syntect::highlighting::Color) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(c.r, c.g, c.b)
}

pub(crate) fn read_window(
    path: &Path,
    center: usize,
    context: usize,
    max_line_len: usize,
) -> Window {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return Window {
                lines: Vec::new(),
                start_line: 0,
                match_index: None,
                message: Some("no preview".to_string()),
            };
        }
    };

    // 1-based inclusive range of lines we want.
    let (start, end) = if center == 0 {
        (1, 1 + 2 * context)
    } else {
        (center.saturating_sub(context).max(1), center + context)
    };

    let mut reader = BufReader::new(file);
    let mut buf: Vec<u8> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut line_no = 0usize;

    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        if buf.contains(&0) {
            return Window {
                lines: Vec::new(),
                start_line: 0,
                match_index: None,
                message: Some("binary file".to_string()),
            };
        }
        line_no += 1;
        if line_no < start {
            continue;
        }
        if line_no > end {
            break;
        }
        let text = String::from_utf8_lossy(&buf);
        let text = text.trim_end_matches(['\n', '\r']);
        let truncated: String = crate::types::sanitize_line(text)
            .chars()
            .take(max_line_len)
            .collect();
        lines.push(truncated);
    }

    let match_index = if center == 0 {
        None
    } else {
        Some(center.saturating_sub(start))
    };

    Window {
        lines,
        start_line: start,
        match_index,
        message: None,
    }
}

/// The preview scroll offset that vertically centers the matched line in a pane
/// `viewport` rows tall. Returns 0 when there is no match (show the file head).
pub fn centered_scroll(match_index: Option<usize>, viewport: usize) -> usize {
    match match_index {
        Some(mi) => mi.saturating_sub(viewport / 2),
        None => 0,
    }
}

/// Resolves a theme name to a syntect theme: a custom `<name>.tmTheme` in `themes_dir`,
/// else a two-face built-in matched case-insensitively (incl. `ansi`/`base16`), else Nord.
pub fn resolve_theme(name: Option<&str>, themes_dir: Option<&Path>) -> Theme {
    let themes = two_face::theme::extra();
    let Some(name) = name else {
        return themes.get(EmbeddedThemeName::Nord).clone();
    };

    if let Some(dir) = themes_dir {
        let path = dir.join(format!("{name}.tmTheme"));
        if let Ok(theme) = ThemeSet::get_theme(&path) {
            return theme;
        }
    }

    for tn in two_face::theme::EmbeddedLazyThemeSet::theme_names() {
        if tn.as_name().eq_ignore_ascii_case(name) {
            return themes.get(*tn).clone();
        }
    }

    themes.get(EmbeddedThemeName::Nord).clone()
}

#[derive(Debug, Clone, Default)]
pub struct PreviewContent {
    pub lines: Vec<Line<'static>>,
    pub match_index: Option<usize>,
    pub message: Option<String>,
    /// Whether any line holds a non-ASCII glyph. Such glyphs can render wider
    /// than ratatui's width model and bleed into the border column, so the run
    /// loop forces a clean repaint when switching away from one of these.
    pub has_wide: bool,
}

impl PreviewContent {
    pub fn empty() -> Self {
        Self::default()
    }
}

pub struct PreviewEngine {
    syntaxes: SyntaxSet,
    theme: Theme,
    cache: HashMap<(PathBuf, usize), PreviewContent>,
    order: VecDeque<(PathBuf, usize)>,
}

impl PreviewEngine {
    pub fn new() -> Self {
        Self::with_theme(resolve_theme(None, None))
    }

    pub fn with_theme(theme: Theme) -> Self {
        let syntaxes = two_face::syntax::extra_newlines();
        Self {
            syntaxes,
            theme,
            cache: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn render(&mut self, item: &Item) -> PreviewContent {
        let (path, center) = match item {
            Item::File { path } => (path.clone(), 0usize),
            Item::Content { path, line, .. } => (path.clone(), *line),
        };
        let key = (path.clone(), center);
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }

        let window = read_window(&path, center, PREVIEW_CONTEXT, PREVIEW_MAX_LINE);
        let content = if let Some(message) = window.message {
            PreviewContent {
                lines: Vec::new(),
                match_index: None,
                message: Some(message),
                has_wide: false,
            }
        } else {
            let has_wide = window.lines.iter().any(|l| !l.is_ascii());
            let lines = self.highlight_window(&path, &window);
            PreviewContent {
                lines,
                match_index: window.match_index,
                message: None,
                has_wide,
            }
        };

        self.insert(key, content.clone());
        content
    }

    fn insert(&mut self, key: (PathBuf, usize), content: PreviewContent) {
        if self.cache.insert(key.clone(), content).is_none() {
            self.order.push_back(key);
            if self.order.len() > CACHE_CAP
                && let Some(old) = self.order.pop_front()
            {
                self.cache.remove(&old);
            }
        }
    }

    fn highlight_window(&self, path: &Path, window: &Window) -> Vec<Line<'static>> {
        let syntax = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|ext| self.syntaxes.find_syntax_by_extension(ext))
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());
        let mut hl = HighlightLines::new(syntax, &self.theme);

        let mut out = Vec::with_capacity(window.lines.len());
        for line in &window.lines {
            let with_nl = format!("{line}\n");
            let spans: Vec<Span<'static>> = match hl.highlight_line(&with_nl, &self.syntaxes) {
                Ok(ranges) => ranges
                    .iter()
                    .map(|(style, text)| {
                        Span::styled(
                            text.trim_end_matches('\n').to_string(),
                            Style::default().fg(syntect_to_ratatui_color(style.foreground)),
                        )
                    })
                    .collect(),
                Err(_) => vec![Span::raw(line.clone())],
            };
            out.push(Line::from(spans));
        }
        out
    }
}

impl Default for PreviewEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    use std::io::Write;

    fn write_lines(path: &Path, n: usize) {
        let mut f = std::fs::File::create(path).unwrap();
        for i in 1..=n {
            writeln!(f, "line {i}").unwrap();
        }
    }

    #[test]
    fn window_around_center() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        write_lines(&p, 100);
        let w = read_window(&p, 50, 10, 1000);
        assert_eq!(w.start_line, 40);
        assert_eq!(w.lines.len(), 21); // 40..=60
        assert_eq!(w.match_index, Some(10)); // 50 - 40
        assert_eq!(w.lines[10], "line 50");
        assert!(w.message.is_none());
    }

    #[test]
    fn head_window_when_center_zero() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        write_lines(&p, 100);
        let w = read_window(&p, 0, 10, 1000);
        assert_eq!(w.start_line, 1);
        assert_eq!(w.lines.first().map(String::as_str), Some("line 1"));
        assert!(w.match_index.is_none());
    }

    #[test]
    fn binary_file_reports_message() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bin.dat");
        std::fs::write(&p, b"abc\0def").unwrap();
        let w = read_window(&p, 0, 10, 1000);
        assert_eq!(w.message.as_deref(), Some("binary file"));
        assert!(w.lines.is_empty());
    }

    #[test]
    fn missing_file_reports_message() {
        let w = read_window(Path::new("/no/such/file.xyz"), 0, 10, 1000);
        assert_eq!(w.message.as_deref(), Some("no preview"));
    }

    #[test]
    fn long_lines_are_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, "x".repeat(5000)).unwrap();
        let w = read_window(&p, 0, 10, 100);
        assert_eq!(w.lines[0].chars().count(), 100);
    }

    #[test]
    fn read_window_lines_have_no_tabs_or_controls() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tabs.go");
        std::fs::write(&p, "func main() {\n\tprintln(\"hi\")\n}\n").unwrap();
        let w = read_window(&p, 0, 10, 1000);
        for line in &w.lines {
            assert!(!line.contains('\t'), "tab leaked: {line:?}");
            assert!(
                !line.chars().any(|c| c.is_control()),
                "control char leaked: {line:?}"
            );
        }
        // The tab-indented body line is expanded, not dropped.
        assert!(w.lines.iter().any(|l| l.contains("    println")));
    }

    #[test]
    fn color_conversion_maps_rgb() {
        use ratatui::style::Color;
        let c = syntect::highlighting::Color {
            r: 255,
            g: 128,
            b: 0,
            a: 255,
        };
        assert_eq!(syntect_to_ratatui_color(c), Color::Rgb(255, 128, 0));
    }

    #[test]
    fn centered_scroll_centers_the_match() {
        assert_eq!(centered_scroll(Some(40), 20), 30); // 40 - 20/2
        assert_eq!(centered_scroll(Some(5), 20), 0); // saturates at the top
        assert_eq!(centered_scroll(None, 20), 0); // file head
    }
}

#[cfg(test)]
mod engine_tests {
    use super::*;
    use crate::types::Item;
    use std::io::Write;

    #[test]
    fn renders_nonempty_for_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.rs");
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "fn main() {{}}\nlet x = 1;").unwrap();

        let mut engine = PreviewEngine::new();
        let content = engine.render(&Item::File { path: p });
        assert!(content.message.is_none());
        assert!(!content.lines.is_empty());
    }

    #[test]
    fn content_item_marks_match_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.rs");
        let mut f = std::fs::File::create(&p).unwrap();
        for i in 1..=20 {
            writeln!(f, "line {i}").unwrap();
        }
        let mut engine = PreviewEngine::new();
        let content = engine.render(&Item::Content {
            path: p,
            line: 10,
            text: "line 10".to_string(),
        });
        assert!(content.match_index.is_some());
        assert!(!content.lines.is_empty());
    }

    #[test]
    fn has_wide_flags_non_ascii_previews() {
        let dir = tempfile::tempdir().unwrap();
        let ascii = dir.path().join("ascii.txt");
        std::fs::write(&ascii, "plain ascii line\n").unwrap();
        let wide = dir.path().join("wide.txt");
        std::fs::write(&wide, "arrow → and box ▶ glyphs\n").unwrap();

        let mut engine = PreviewEngine::new();
        assert!(!engine.render(&Item::File { path: ascii }).has_wide);
        assert!(engine.render(&Item::File { path: wide }).has_wide);
    }

    #[test]
    fn binary_file_yields_message() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bin.dat");
        std::fs::write(&p, b"abc\0def").unwrap();
        let mut engine = PreviewEngine::new();
        let content = engine.render(&Item::File { path: p });
        assert_eq!(content.message.as_deref(), Some("binary file"));
    }

    #[test]
    fn second_render_is_cached() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.rs");
        std::fs::write(&p, "fn main() {}\n").unwrap();
        let mut engine = PreviewEngine::new();
        let item = Item::File { path: p };
        let a = engine.render(&item);
        let b = engine.render(&item);
        assert_eq!(a.lines.len(), b.lines.len());
    }
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    // Minimal valid .tmTheme (plist) named "MyTest".
    const MINIMAL_TMTHEME: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>name</key><string>MyTest</string>
<key>settings</key><array>
<dict><key>settings</key><dict>
<key>background</key><string>#000000</string>
<key>foreground</key><string>#FFFFFF</string>
</dict></dict>
</array></dict></plist>
"#;

    #[test]
    fn builtin_theme_resolves_by_name() {
        let t = resolve_theme(Some("Solarized (dark)"), None);
        assert!(
            t.name
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains("solarized"),
            "got {:?}",
            t.name
        );
    }

    #[test]
    fn unknown_theme_falls_back_to_nord() {
        let t = resolve_theme(Some("definitely-not-a-theme"), None);
        assert!(
            t.name
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains("nord"),
            "got {:?}",
            t.name
        );
    }

    #[test]
    fn custom_tmtheme_loads_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("MyTest.tmTheme"), MINIMAL_TMTHEME).unwrap();
        let t = resolve_theme(Some("MyTest"), Some(dir.path()));
        assert_eq!(t.name.as_deref(), Some("MyTest"));
    }
}
