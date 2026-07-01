use std::path::PathBuf;

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Config {
    pub editor: Option<String>,
    pub editor_cmd: Option<String>,
    pub theme: Option<String>,
}

pub fn parse(text: &str) -> Result<Config, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

fn config_dir_from(xdg: Option<String>, home: Option<String>) -> Option<PathBuf> {
    if let Some(x) = xdg.filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(x).join("russee"));
    }
    home.map(|h| PathBuf::from(h).join(".config").join("russee"))
}

pub fn config_dir() -> Option<PathBuf> {
    config_dir_from(
        std::env::var("XDG_CONFIG_HOME").ok(),
        std::env::var("HOME").ok(),
    )
}

pub fn themes_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("themes"))
}

pub fn load() -> Config {
    let Some(path) = config_dir().map(|d| d.join("config.toml")) else {
        return Config::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Config::default();
    };
    match parse(&text) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("russee: ignoring invalid config at {}: {e}", path.display());
            Config::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let c = parse("editor = \"nvim\"\neditor_cmd = \"x {file}\"\ntheme = \"Dracula\"").unwrap();
        assert_eq!(c.editor.as_deref(), Some("nvim"));
        assert_eq!(c.editor_cmd.as_deref(), Some("x {file}"));
        assert_eq!(c.theme.as_deref(), Some("Dracula"));
    }

    #[test]
    fn empty_config_is_all_none() {
        let c = parse("").unwrap();
        assert!(c.editor.is_none());
        assert!(c.editor_cmd.is_none());
        assert!(c.theme.is_none());
    }

    #[test]
    fn partial_config_keeps_others_none() {
        let c = parse("theme = \"Nord\"").unwrap();
        assert_eq!(c.theme.as_deref(), Some("Nord"));
        assert!(c.editor.is_none());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let c = parse("editor = \"vi\"\nfuture_key = 3").unwrap();
        assert_eq!(c.editor.as_deref(), Some("vi"));
    }

    #[test]
    fn invalid_toml_is_an_error() {
        assert!(parse("editor = ").is_err());
    }

    #[test]
    fn config_dir_prefers_xdg() {
        assert_eq!(
            config_dir_from(Some("/x/cfg".to_string()), Some("/home/u".to_string())),
            Some(PathBuf::from("/x/cfg/russee"))
        );
    }

    #[test]
    fn config_dir_falls_back_to_home() {
        assert_eq!(
            config_dir_from(Some(String::new()), Some("/home/u".to_string())),
            Some(PathBuf::from("/home/u/.config/russee"))
        );
        assert_eq!(config_dir_from(None, None), None);
    }
}
