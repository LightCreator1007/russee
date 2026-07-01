use std::path::{Path, PathBuf};
use std::sync::Arc;

use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use ignore::types::TypesBuilder;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};

use crate::engine::Filters;
use crate::types::Item;

pub struct FileEngine {
    nuc: Nucleo<PathBuf>,
}

impl FileEngine {
    pub fn new(root: PathBuf, filters: Filters) -> anyhow::Result<Self> {
        // One column: the path string we fuzzy-match against.
        let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let nuc = Nucleo::<PathBuf>::new(Config::DEFAULT, notify, None, 1);
        let injector = nuc.injector();

        // Build the type matcher (empty `types` => all files pass).
        let mut tb = TypesBuilder::new();
        tb.add_defaults();
        for t in &filters.types {
            tb.select(t);
        }
        let types = tb.build()?;

        // Build glob overrides.
        let mut ob = OverrideBuilder::new(&root);
        for g in &filters.globs {
            ob.add(g)?;
        }
        let overrides = ob.build()?;

        let walker = WalkBuilder::new(&root)
            .types(types)
            .overrides(overrides)
            .hidden(true)
            .require_git(false) // honor .gitignore even outside a git repo
            .build_parallel();

        // Walk on background threads, streaming paths into nucleo's injector.
        std::thread::spawn(move || {
            walker.run(|| {
                let injector = injector.clone();
                Box::new(move |entry| {
                    if let Ok(entry) = entry
                        && entry.file_type().is_some_and(|ft| ft.is_file())
                    {
                        let path = files_display_path(entry.path());
                        injector.push(path, |p, columns| {
                            columns[0] = p.to_string_lossy().into();
                        });
                    }
                    ignore::WalkState::Continue
                })
            });
        });

        Ok(Self { nuc })
    }

    pub fn set_query(&mut self, query: &str, case_sensitive: Option<bool>) {
        let case = match case_sensitive {
            None => CaseMatching::Smart,
            Some(true) => CaseMatching::Respect,
            Some(false) => CaseMatching::Ignore,
        };
        self.nuc
            .pattern
            .reparse(0, query, case, Normalization::Smart, false);
    }

    /// Advances the matcher; returns true while threads are still working.
    pub fn tick(&mut self) -> bool {
        self.nuc.tick(10).running
    }

    /// Returns (items in `[offset, offset+limit)`, total matched, total items).
    pub fn snapshot(&self, offset: usize, limit: usize) -> (Vec<Item>, usize, usize) {
        let snap = self.nuc.snapshot();
        let matched = snap.matched_item_count() as usize;
        let total = snap.item_count() as usize;
        let end = offset.saturating_add(limit).min(matched);
        let items = if offset >= end {
            Vec::new()
        } else {
            snap.matched_items(offset as u32..end as u32)
                .map(|it| Item::File {
                    path: it.data.clone(),
                })
                .collect()
        };
        (items, matched, total)
    }
}

/// Strips a leading `./` so paths display cleanly and `^`-anchored queries work,
/// while staying relative to the current directory (composable in the shell).
pub(crate) fn files_display_path(path: &Path) -> PathBuf {
    path.strip_prefix(".").unwrap_or(path).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn files_display_path_strips_leading_dot() {
        assert_eq!(
            files_display_path(Path::new("./src/main.rs")),
            PathBuf::from("src/main.rs")
        );
        assert_eq!(
            files_display_path(Path::new("src/parser.rs")),
            PathBuf::from("src/parser.rs")
        );
        assert_eq!(
            files_display_path(Path::new("/abs/x.rs")),
            PathBuf::from("/abs/x.rs")
        );
    }

    fn touch(dir: &std::path::Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    /// Drive the engine until the match counts stabilize, then collect paths.
    fn matched_paths(engine: &mut FileEngine) -> Vec<String> {
        let mut last = (usize::MAX, usize::MAX);
        let mut stable = 0;
        for _ in 0..500 {
            engine.tick();
            let (_i, matched, total) = engine.snapshot(0, 0);
            if (matched, total) == last {
                stable += 1;
                if stable >= 10 {
                    break;
                }
            } else {
                stable = 0;
                last = (matched, total);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let (items, matched, _total) = engine.snapshot(0, 10_000);
        assert_eq!(items.len(), matched.min(10_000));
        items
            .iter()
            .map(|i| i.selection_string().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn finds_files_fuzzy_and_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "src/parser.rs", "fn parse() {}");
        touch(dir.path(), "src/lexer.rs", "fn lex() {}");
        touch(dir.path(), ".gitignore", "ignored/\n");
        touch(dir.path(), "ignored/secret.rs", "secret");

        let mut engine = FileEngine::new(dir.path().to_path_buf(), Filters::default()).unwrap();
        engine.set_query("parser", None);
        let paths = matched_paths(&mut engine);

        assert!(
            paths.iter().any(|p| p.ends_with("src/parser.rs")),
            "got {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains("ignored/secret.rs")),
            "gitignored file must be excluded: {paths:?}"
        );
    }

    #[test]
    fn type_filter_restricts_extensions() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "a.rs", "x");
        touch(dir.path(), "b.txt", "x");

        let filters = Filters {
            types: vec!["rust".to_string()],
            globs: vec![],
        };
        let mut engine = FileEngine::new(dir.path().to_path_buf(), filters).unwrap();
        engine.set_query("", None);
        let paths = matched_paths(&mut engine);

        assert!(paths.iter().any(|p| p.ends_with("a.rs")), "got {paths:?}");
        assert!(
            !paths.iter().any(|p| p.ends_with("b.txt")),
            "txt filtered out: {paths:?}"
        );
    }
}
