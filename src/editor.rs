use crate::config::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedEditor {
    Known(String),
    Template(String),
}

pub fn resolve_editor(cfg: &Config, env_editor: Option<String>) -> ResolvedEditor {
    if let Some(t) = &cfg.editor_cmd {
        return ResolvedEditor::Template(t.clone());
    }
    if let Some(e) = &cfg.editor {
        return ResolvedEditor::Known(e.clone());
    }
    if let Some(e) = env_editor.filter(|s| !s.is_empty()) {
        return ResolvedEditor::Known(e);
    }
    ResolvedEditor::Known("vi".to_string())
}

fn basename(cmd: &str) -> &str {
    cmd.rsplit('/').next().unwrap_or(cmd)
}

fn known_args_template(basename: &str) -> Option<&'static str> {
    match basename {
        "vi" | "vim" | "nvim" | "nano" | "emacs" | "micro" => Some("+{line} {file}"),
        "code" | "code-insiders" | "codium" => Some("-g {file}:{line}"),
        "zed" | "subl" | "hx" => Some("{file}:{line}"),
        _ => None,
    }
}

fn substitute_tokens(template: &str, file: &str, line: Option<usize>) -> Vec<String> {
    let line_s = line.unwrap_or(1).to_string();
    template
        .split_whitespace()
        .map(|tok| tok.replace("{file}", file).replace("{line}", &line_s))
        .collect()
}

pub fn build_editor_argv(
    resolved: &ResolvedEditor,
    file: &str,
    line: Option<usize>,
) -> Vec<String> {
    match resolved {
        ResolvedEditor::Template(t) => substitute_tokens(t, file, line),
        ResolvedEditor::Known(name) => {
            let mut argv = vec![name.clone()];
            match (line, known_args_template(basename(name))) {
                (Some(l), Some(tmpl)) => argv.extend(substitute_tokens(tmpl, file, Some(l))),
                _ => argv.push(file.to_string()),
            }
            argv
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(editor: Option<&str>, editor_cmd: Option<&str>) -> Config {
        Config {
            editor: editor.map(String::from),
            editor_cmd: editor_cmd.map(String::from),
            theme: None,
        }
    }

    #[test]
    fn resolve_precedence() {
        assert_eq!(
            resolve_editor(&cfg(Some("nvim"), Some("x {file}")), Some("vim".into())),
            ResolvedEditor::Template("x {file}".into())
        );
        assert_eq!(
            resolve_editor(&cfg(Some("nvim"), None), Some("vim".into())),
            ResolvedEditor::Known("nvim".into())
        );
        assert_eq!(
            resolve_editor(&cfg(None, None), Some("emacs".into())),
            ResolvedEditor::Known("emacs".into())
        );
        assert_eq!(
            resolve_editor(&cfg(None, None), None),
            ResolvedEditor::Known("vi".into())
        );
    }

    fn known(name: &str, file: &str, line: Option<usize>) -> Vec<String> {
        build_editor_argv(&ResolvedEditor::Known(name.into()), file, line)
    }

    #[test]
    fn terminal_editors_use_plus_line() {
        assert_eq!(known("vim", "a.rs", Some(42)), vec!["vim", "+42", "a.rs"]);
        assert_eq!(known("nano", "a.rs", Some(10)), vec!["nano", "+10", "a.rs"]);
        assert_eq!(known("emacs", "a.rs", Some(5)), vec!["emacs", "+5", "a.rs"]);
    }

    #[test]
    fn vscode_uses_goto() {
        assert_eq!(
            known("code", "a.rs", Some(42)),
            vec!["code", "-g", "a.rs:42"]
        );
    }

    #[test]
    fn zed_subl_hx_use_colon() {
        assert_eq!(known("zed", "a.rs", Some(7)), vec!["zed", "a.rs:7"]);
        assert_eq!(known("subl", "a.rs", Some(7)), vec!["subl", "a.rs:7"]);
        assert_eq!(known("hx", "a.rs", Some(3)), vec!["hx", "a.rs:3"]);
    }

    #[test]
    fn basename_is_used_for_lookup() {
        assert_eq!(
            known("/usr/bin/nvim", "a.rs", Some(9)),
            vec!["/usr/bin/nvim", "+9", "a.rs"]
        );
    }

    #[test]
    fn no_line_opens_file_only() {
        assert_eq!(known("vim", "a.rs", None), vec!["vim", "a.rs"]);
    }

    #[test]
    fn unknown_editor_opens_file_only() {
        assert_eq!(known("weirded", "a.rs", Some(1)), vec!["weirded", "a.rs"]);
    }

    #[test]
    fn template_substitutes_and_keeps_spaced_path_as_one_arg() {
        let r = ResolvedEditor::Template("code -g {file}:{line}".into());
        assert_eq!(
            build_editor_argv(&r, "a b.rs", Some(42)),
            vec!["code", "-g", "a b.rs:42"]
        );
    }

    #[test]
    fn template_line_defaults_to_one() {
        let r = ResolvedEditor::Template("nvim +{line} {file}".into());
        assert_eq!(
            build_editor_argv(&r, "a.rs", None),
            vec!["nvim", "+1", "a.rs"]
        );
    }
}
