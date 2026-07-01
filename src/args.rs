use crate::engine::Filters;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub root: PathBuf,
    pub filters: Filters,
    pub cli_query: Option<String>,
    pub case_sensitive: Option<bool>,
}

pub fn parse<I: IntoIterator<Item = String>>(args: I) -> CliArgs {
    let mut root: Option<PathBuf> = None;
    let mut filters = Filters::default();
    let mut cli = false;
    let mut cli_query: Option<String> = None;
    let mut case_sensitive: Option<bool> = None;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-t" | "--type" => {
                if let Some(v) = it.next() {
                    filters.types.push(v);
                }
            }
            "-g" | "--glob" => {
                if let Some(v) = it.next() {
                    filters.globs.push(v);
                }
            }
            "--cli" | "--no-tui" => cli = true,
            "-i" | "--ignore-case" => case_sensitive = Some(false),
            "-s" | "--case-sensitive" => case_sensitive = Some(true),
            s if s.starts_with('-') => { /* unknown flag: ignore */ }
            s => {
                if cli && cli_query.is_none() {
                    cli_query = Some(s.to_string());
                } else if root.is_none() {
                    root = Some(PathBuf::from(s));
                }
            }
        }
    }

    CliArgs {
        root: root.unwrap_or_else(|| PathBuf::from(".")),
        filters,
        cli_query: if cli {
            cli_query.or(Some(String::new()))
        } else {
            None
        },
        case_sensitive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_vec(args: &[&str]) -> CliArgs {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_to_current_dir_no_filters() {
        let a = parse_vec(&[]);
        assert_eq!(a.root, PathBuf::from("."));
        assert!(a.filters.types.is_empty());
        assert!(a.filters.globs.is_empty());
        assert!(a.cli_query.is_none());
    }

    #[test]
    fn positional_root_is_captured() {
        let a = parse_vec(&["src"]);
        assert_eq!(a.root, PathBuf::from("src"));
    }

    #[test]
    fn type_and_glob_flags_accumulate() {
        let a = parse_vec(&[
            "-t", "rust", "--type", "toml", "-g", "!*.lock", "-g", "src/**",
        ]);
        assert_eq!(
            a.filters.types,
            vec!["rust".to_string(), "toml".to_string()]
        );
        assert_eq!(
            a.filters.globs,
            vec!["!*.lock".to_string(), "src/**".to_string()]
        );
        assert_eq!(a.root, PathBuf::from("."));
    }

    #[test]
    fn cli_flag_captures_query() {
        let a = parse_vec(&["--cli", "checked_add"]);
        assert_eq!(a.cli_query.as_deref(), Some("checked_add"));
        assert_eq!(a.root, PathBuf::from("."));
    }

    #[test]
    fn ignore_case_flag() {
        let a = parse_vec(&["--cli", "Foo", "-i"]);
        assert_eq!(a.case_sensitive, Some(false));
    }

    #[test]
    fn no_cli_means_none() {
        let a = parse_vec(&["src"]);
        assert!(a.cli_query.is_none());
    }
}
