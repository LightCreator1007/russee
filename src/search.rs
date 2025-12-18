use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::mpsc::Sender,
};

use regex::Regex;
use walkdir::WalkDir;

use crate::types::SearchResult;

pub fn search(root: PathBuf, pattern: String, tx: Sender<SearchResult>) {
    let regex = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => return,
    };

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path().to_path_buf();
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let reader = BufReader::new(file);

        for (idx, line) in reader.lines().enumerate() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            if regex.is_match(&line) {
                let _ = tx.send(SearchResult {
                    path: path.clone(),
                    line: idx + 1,
                    text: line,
                });
            }
        }
    }
}
