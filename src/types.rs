use std::path::PathBuf;

const TAB_STOP: usize = 4;

/// Makes text safe to render in a fixed-width pane: tabs become spaces and
/// other control characters are dropped. Terminals expand `\t` to a tab stop
/// and act on control bytes, but ratatui's width model counts them as ~0
/// columns; that mismatch shoves a line's tail past the pane edge into the
/// border, where the diff renderer can't reclaim it (the ghosting bug).
pub(crate) fn sanitize_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut col = 0;
    for ch in text.chars() {
        match ch {
            '\t' => {
                let spaces = TAB_STOP - (col % TAB_STOP);
                for _ in 0..spaces {
                    out.push(' ');
                }
                col += spaces;
            }
            c if c.is_control() => {}
            c => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    File {
        path: PathBuf,
    },
    Content {
        path: PathBuf,
        line: usize,
        text: String,
    },
}

impl Item {
    /// The text `Enter` prints to stdout.
    pub fn selection_string(&self) -> String {
        match self {
            Item::File { path } => path.display().to_string(),
            Item::Content { path, line, .. } => format!("{}:{}", path.display(), line),
        }
    }

    /// (path, optional 1-based line) for opening in `$EDITOR`.
    pub fn open_target(&self) -> (PathBuf, Option<usize>) {
        match self {
            Item::File { path } => (path.clone(), None),
            Item::Content { path, line, .. } => (path.clone(), Some(*line)),
        }
    }

    /// The label shown in the results list. Sanitized so tabs and control
    /// characters in matched lines can't desync the list pane's width.
    pub fn list_label(&self) -> String {
        match self {
            Item::File { path } => sanitize_line(&path.display().to_string()),
            Item::Content { path, line, text } => {
                sanitize_line(&format!("{}:{}: {}", path.display(), line, text))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_item_selection_and_open_target() {
        let item = Item::File {
            path: PathBuf::from("src/parser.rs"),
        };
        assert_eq!(item.selection_string(), "src/parser.rs");
        assert_eq!(item.open_target(), (PathBuf::from("src/parser.rs"), None));
        assert_eq!(item.list_label(), "src/parser.rs");
    }

    #[test]
    fn content_item_selection_open_and_label() {
        let item = Item::Content {
            path: PathBuf::from("src/math.rs"),
            line: 14,
            text: "let t = a.checked_add(b);".to_string(),
        };
        assert_eq!(item.selection_string(), "src/math.rs:14");
        assert_eq!(item.open_target(), (PathBuf::from("src/math.rs"), Some(14)));
        assert_eq!(
            item.list_label(),
            "src/math.rs:14: let t = a.checked_add(b);"
        );
    }

    #[test]
    fn content_label_expands_tabs_and_drops_controls() {
        // go.mod require lines are tab-indented; the raw tab must not survive
        // into the list label (it desyncs the pane width and leaves residue).
        let item = Item::Content {
            path: PathBuf::from("go.mod"),
            line: 8,
            text: "\tgithub.com/x/y v1.2.3".to_string(),
        };
        let label = item.list_label();
        assert!(!label.contains('\t'), "tab leaked: {label:?}");
        assert!(!label.chars().any(|c| c.is_control()));
        // Tab sits at column 10 (after "go.mod:8: "), so it fills to the next
        // stop with 2 spaces.
        assert_eq!(label, "go.mod:8:   github.com/x/y v1.2.3");
    }

    #[test]
    fn sanitize_expands_tabs_to_tab_stops() {
        assert_eq!(sanitize_line("\tx"), "    x"); // tab at col 0 -> 4 spaces
        assert_eq!(sanitize_line("a\tb"), "a   b"); // tab at col 1 -> 3 spaces
        assert_eq!(sanitize_line("ab\tc"), "ab  c"); // tab at col 2 -> 2 spaces
    }

    #[test]
    fn sanitize_drops_control_chars() {
        assert_eq!(sanitize_line("a\u{07}b\u{1b}c"), "abc"); // bell + ESC removed
        assert_eq!(sanitize_line("a\rb"), "ab"); // stray carriage return removed
        assert_eq!(sanitize_line("café — déjà"), "café — déjà"); // printable multibyte kept
    }
}
