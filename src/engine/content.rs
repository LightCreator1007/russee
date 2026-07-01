use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};

use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use ignore::types::TypesBuilder;

use crate::engine::Filters;
use crate::engine::files::files_display_path;
use crate::query::{ParsedQuery, line_matches, parse_content};
use crate::types::Item;

#[derive(Debug)]
pub enum ContentMatcher {
    Extended(ParsedQuery),
    Regex(regex::Regex),
}

impl ContentMatcher {
    pub fn matches(&self, line: &str) -> bool {
        match self {
            ContentMatcher::Extended(q) => line_matches(q, line),
            ContentMatcher::Regex(re) => re.is_match(line),
        }
    }
}

/// Builds a matcher. `regex=false` => extended literal atoms; `regex=true` => one regex
/// over the whole query (smart-case via the uppercase heuristic unless `case_sensitive`
/// overrides). Returns a readable error string when the regex fails to compile.
pub fn build_matcher(
    query: &str,
    regex: bool,
    case_sensitive: Option<bool>,
) -> Result<ContentMatcher, String> {
    if !regex {
        return Ok(ContentMatcher::Extended(parse_content(
            query,
            case_sensitive,
        )));
    }
    let cs = case_sensitive.unwrap_or_else(|| query.chars().any(|c| c.is_uppercase()));
    regex::RegexBuilder::new(query)
        .case_insensitive(!cs)
        .build()
        .map(ContentMatcher::Regex)
        .map_err(|e| e.to_string())
}

pub const CONTENT_RESULT_CAP: usize = 50_000;
const BATCH: usize = 256;

pub struct ContentEngine {
    root: PathBuf,
    filters: Filters,
    generation: Arc<AtomicU64>,
    rx: Option<Receiver<Vec<Item>>>,
    results: Vec<Item>,
    error: Option<String>,
    counter: Arc<AtomicUsize>,
}

impl ContentEngine {
    pub fn new(root: PathBuf, filters: Filters) -> Self {
        Self {
            root,
            filters,
            generation: Arc::new(AtomicU64::new(0)),
            rx: None,
            results: Vec::new(),
            error: None,
            counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn results(&self) -> &[Item] {
        &self.results
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn capped(&self) -> bool {
        self.counter.load(Ordering::Relaxed) >= CONTENT_RESULT_CAP
    }

    pub fn set_query(&mut self, query: &str, regex: bool, case_sensitive: Option<bool>) {
        // Bump generation => any in-flight search abandons itself.
        let gen_id = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.results.clear();
        self.rx = None;
        self.error = None;
        self.counter.store(0, Ordering::SeqCst);

        if query.trim().is_empty() {
            return; // empty query => no search, no results
        }

        let matcher = match build_matcher(query, regex, case_sensitive) {
            Ok(m) => Arc::new(m),
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };

        let walker = match build_walker(&self.root, &self.filters) {
            Ok(w) => w,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };

        let (tx, rx) = sync_channel::<Vec<Item>>(64);
        self.rx = Some(rx);

        let generation = self.generation.clone();
        let counter = self.counter.clone();

        std::thread::spawn(move || {
            walker.run(|| {
                let tx = tx.clone();
                let matcher = matcher.clone();
                let generation = generation.clone();
                let counter = counter.clone();
                Box::new(move |entry| {
                    // Cancel if a newer query superseded us, or the cap is hit.
                    if generation.load(Ordering::SeqCst) != gen_id
                        || counter.load(Ordering::SeqCst) >= CONTENT_RESULT_CAP
                    {
                        return ignore::WalkState::Quit;
                    }
                    if let Ok(entry) = entry
                        && entry.file_type().is_some_and(|ft| ft.is_file())
                    {
                        let path = files_display_path(entry.path());
                        search_file(&path, &matcher, &tx, &generation, gen_id, &counter);
                    }
                    ignore::WalkState::Continue
                })
            });
        });
    }

    /// Drains pending batches into `results`; returns true while still running.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else {
            return false;
        };
        loop {
            match rx.try_recv() {
                Ok(batch) => self.results.extend(batch),
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    return false;
                }
            }
        }
    }
}

fn build_walker(root: &PathBuf, filters: &Filters) -> anyhow::Result<ignore::WalkParallel> {
    let mut tb = TypesBuilder::new();
    tb.add_defaults();
    for t in &filters.types {
        tb.select(t);
    }
    let types = tb.build()?;

    let mut ob = OverrideBuilder::new(root);
    for g in &filters.globs {
        ob.add(g)?;
    }
    let overrides = ob.build()?;

    Ok(WalkBuilder::new(root)
        .types(types)
        .overrides(overrides)
        .hidden(true)
        .require_git(false)
        .build_parallel())
}

fn search_file(
    path: &std::path::Path,
    matcher: &ContentMatcher,
    tx: &SyncSender<Vec<Item>>,
    generation: &AtomicU64,
    gen_id: u64,
    counter: &AtomicUsize,
) {
    let Ok(file) = File::open(path) else {
        return;
    };
    let mut reader = BufReader::new(file);
    let mut buf: Vec<u8> = Vec::new();
    let mut batch: Vec<Item> = Vec::new();
    let mut line_no = 0usize;

    loop {
        if generation.load(Ordering::SeqCst) != gen_id {
            return;
        }
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        // Binary heuristic: a NUL byte => stop scanning this file.
        if buf.contains(&0) {
            return;
        }
        line_no += 1;
        let line = String::from_utf8_lossy(&buf);
        let line = line.trim_end_matches(['\n', '\r']);
        if matcher.matches(line) {
            if counter.fetch_add(1, Ordering::SeqCst) >= CONTENT_RESULT_CAP {
                break;
            }
            batch.push(Item::Content {
                path: path.to_path_buf(),
                line: line_no,
                text: line.to_string(),
            });
            if batch.len() >= BATCH {
                let chunk = std::mem::take(&mut batch);
                if tx.send(chunk).is_err() {
                    return;
                }
            }
        }
    }
    if !batch.is_empty() {
        let _ = tx.send(batch);
    }
}

#[cfg(test)]
mod matcher_tests {
    use super::*;

    #[test]
    fn extended_matcher_uses_atoms() {
        let m = build_matcher("checked !mut", false, None).unwrap();
        assert!(m.matches("a.checked_add(b)"));
        assert!(!m.matches("let mut x = checked(y)"));
    }

    #[test]
    fn regex_matcher_matches_pattern() {
        let m = build_matcher("checked_(add|sub)", true, None).unwrap();
        assert!(m.matches("a.checked_add(b)"));
        assert!(m.matches("a.checked_sub(b)"));
        assert!(!m.matches("a.checked_mul(b)"));
    }

    #[test]
    fn regex_smart_case() {
        assert!(
            build_matcher("checked", true, None)
                .unwrap()
                .matches("CHECKED")
        );
        assert!(
            !build_matcher("Checked", true, None)
                .unwrap()
                .matches("checked")
        );
    }

    #[test]
    fn invalid_regex_is_an_error() {
        let err = build_matcher("a(b", true, None).unwrap_err();
        assert!(!err.is_empty());
    }
}

#[cfg(test)]
mod engine_tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn touch(dir: &std::path::Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    fn drain(engine: &mut ContentEngine) -> Vec<String> {
        for _ in 0..500 {
            let running = engine.poll();
            if !running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        engine
            .results()
            .iter()
            .map(|i| i.list_label().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn finds_matching_lines_and_skips_binary_and_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        touch(
            dir.path(),
            "src/a.rs",
            "fn one() {}\nlet x = checked_add(b);\n",
        );
        touch(dir.path(), "src/b.rs", "no match here\n");
        touch(dir.path(), ".gitignore", "skip/\n");
        touch(dir.path(), "skip/c.rs", "checked_add ignored\n");
        touch(dir.path(), "bin.dat", "checked_add\0\0binary");

        let mut engine = ContentEngine::new(dir.path().to_path_buf(), Filters::default());
        engine.set_query("checked_add", false, None);
        let rows = drain(&mut engine);

        assert!(
            rows.iter()
                .any(|r| r.contains("src/a.rs:2: ") && r.contains("checked_add")),
            "got {rows:?}"
        );
        assert!(
            !rows.iter().any(|r| r.contains("skip/c.rs")),
            "gitignored: {rows:?}"
        );
        assert!(
            !rows.iter().any(|r| r.contains("bin.dat")),
            "binary skipped: {rows:?}"
        );
        assert!(engine.error().is_none());
    }

    #[test]
    fn invalid_regex_sets_error_and_no_results() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "a.rs", "anything");
        let mut engine = ContentEngine::new(dir.path().to_path_buf(), Filters::default());
        engine.set_query("a(b", true, None);
        let rows = drain(&mut engine);
        assert!(rows.is_empty());
        assert!(engine.error().is_some());
    }
}
