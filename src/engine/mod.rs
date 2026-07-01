pub mod content;
pub mod files;

/// Walk-level filters shared by all engines.
#[derive(Debug, Clone, Default)]
pub struct Filters {
    /// `ignore` type names to include (e.g. "rust", "py"). Empty = all types.
    pub types: Vec<String>,
    /// Glob overrides (e.g. "!*.lock", "src/**"). Empty = none.
    pub globs: Vec<String>,
}
