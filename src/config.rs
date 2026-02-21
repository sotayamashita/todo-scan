use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration for todox TODO tracking tool
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(default)]
#[schemars(deny_unknown_fields, title = "todox Configuration")]
pub struct Config {
    /// Tags to scan for (e.g., TODO, FIXME, HACK)
    pub tags: Vec<String>,
    /// Directory names to skip during scanning
    pub exclude_dirs: Vec<String>,
    /// Regex patterns; matching file paths are excluded
    pub exclude_patterns: Vec<String>,
    /// CI gate check settings
    pub check: CheckConfig,
    /// Git blame analysis settings
    pub blame: BlameConfig,
    /// Lint rule settings
    pub lint: LintConfig,
}

/// CI gate check settings
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default)]
#[schemars(deny_unknown_fields)]
pub struct CheckConfig {
    /// Maximum total TODOs allowed
    pub max: Option<usize>,
    /// Maximum new TODOs allowed (requires --since)
    pub max_new: Option<usize>,
    /// Tags that cause check to fail immediately
    pub block_tags: Vec<String>,
    /// Fail if any TODOs have expired deadlines
    pub expired: Option<bool>,
}

/// Git blame analysis settings
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default)]
#[schemars(deny_unknown_fields)]
pub struct BlameConfig {
    /// Duration threshold for marking TODOs as stale (e.g., "180d")
    pub stale_threshold: Option<String>,
}

/// Lint rule settings for TODO comment formatting
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default)]
#[schemars(deny_unknown_fields)]
pub struct LintConfig {
    /// Reject TODOs with empty message (default: true)
    pub no_bare_tags: Option<bool>,
    /// Enforce max message character count
    pub max_message_length: Option<usize>,
    /// Require (author) for specified tags
    pub require_author: Option<Vec<String>>,
    /// Require issue ref (#N) for specified tags
    pub require_issue_ref: Option<Vec<String>>,
    /// Enforce uppercase tag names (default: true)
    pub uppercase_tag: Option<bool>,
    /// Enforce colon after tag (default: true)
    pub require_colon: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tags: vec![
                "TODO".into(),
                "FIXME".into(),
                "HACK".into(),
                "XXX".into(),
                "BUG".into(),
                "NOTE".into(),
            ],
            exclude_dirs: vec![],
            exclude_patterns: vec![],
            check: CheckConfig::default(),
            blame: BlameConfig::default(),
            lint: LintConfig::default(),
        }
    }
}

impl Config {
    /// Build regex pattern from configured tags
    pub fn tags_pattern(&self) -> String {
        let tags = self.tags.join("|");
        format!(r"(?i)\b({tags})(?:\(([^)]+)\))?:?\s*(!{{1,2}})?\s*(.*)$")
    }

    /// Load config from .todox.toml, searching up from the given directory
    pub fn load(start_dir: &Path) -> Result<Self> {
        if let Some(path) = find_config_file(start_dir) {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config: {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config: {}", path.display()))?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }
}

/// Search for .todox.toml from start_dir upward
fn find_config_file(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join(".todox.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_tags_pattern() {
        let config = Config::default();
        let pattern = config.tags_pattern();
        assert!(pattern.contains("TODO"));
        assert!(pattern.contains("FIXME"));
        assert!(pattern.contains("HACK"));
    }

    #[test]
    fn test_config_from_toml() {
        let toml_str = r#"
tags = ["TODO", "FIXME"]

[check]
max = 50
block_tags = ["BUG"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.tags.len(), 2);
        assert_eq!(config.check.max, Some(50));
        assert_eq!(config.check.block_tags, vec!["BUG"]);
    }

    /// Validates that schema/todox.schema.json matches the current Config structs.
    ///
    /// To regenerate the schema after changing Config:
    ///   UPDATE_SCHEMA=1 cargo test schema_is_up_to_date
    #[test]
    fn schema_is_up_to_date() {
        let schema = schemars::schema_for!(Config);
        let generated = serde_json::to_string_pretty(&schema).unwrap() + "\n";

        let schema_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/todox.schema.json");

        if std::env::var("UPDATE_SCHEMA").is_ok() {
            std::fs::create_dir_all(schema_path.parent().unwrap()).unwrap();
            std::fs::write(&schema_path, &generated).unwrap();
            return;
        }

        let committed = std::fs::read_to_string(&schema_path).expect(
            "schema/todox.schema.json not found. Run `UPDATE_SCHEMA=1 cargo test schema_is_up_to_date` to generate it.",
        );
        assert_eq!(
            generated, committed,
            "Schema is out of date. Run `UPDATE_SCHEMA=1 cargo test schema_is_up_to_date` to update."
        );
    }
}
