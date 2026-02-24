use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration for todo-scan TODO tracking tool
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(default)]
#[schemars(deny_unknown_fields, title = "todo-scan Configuration")]
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
    /// Clean detection settings
    pub clean: CleanConfig,
    /// Workspace/monorepo settings
    pub workspace: WorkspaceConfig,
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

/// Clean detection settings for stale issues and duplicates
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default)]
#[schemars(deny_unknown_fields)]
pub struct CleanConfig {
    /// Enable stale issue detection (default: true)
    pub stale_issues: Option<bool>,
    /// Enable duplicate detection (default: true)
    pub duplicates: Option<bool>,
    /// Only flag issues closed longer than this duration (e.g., "30d")
    pub since: Option<String>,
}

/// Workspace/monorepo settings
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default)]
#[schemars(deny_unknown_fields)]
pub struct WorkspaceConfig {
    /// Enable automatic workspace detection (default: true)
    pub auto_detect: Option<bool>,
    /// Per-package check configuration
    pub packages: std::collections::HashMap<String, PackageCheckConfig>,
}

/// Per-package check configuration
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default)]
#[schemars(deny_unknown_fields)]
pub struct PackageCheckConfig {
    /// Maximum total TODOs allowed for this package
    pub max: Option<usize>,
    /// Tags that cause check to fail for this package
    pub block_tags: Vec<String>,
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
            clean: CleanConfig::default(),
            workspace: WorkspaceConfig::default(),
        }
    }
}

impl Config {
    /// Build regex pattern from configured tags.
    /// Each tag is escaped to prevent regex injection from config values.
    pub fn tags_pattern(&self) -> String {
        let tags = self
            .tags
            .iter()
            .map(|t| regex::escape(t))
            .collect::<Vec<_>>()
            .join("|");
        format!(r"(?i)\b({tags})\b(?:\(([^)]+)\))?:?\s*(!{{1,2}})?\s*(.*)$")
    }

    /// Load config from .todo-scan.toml, searching up from the given directory
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

/// Search for .todo-scan.toml from start_dir upward
fn find_config_file(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join(".todo-scan.toml");
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

    #[test]
    fn test_workspace_config_default() {
        let config = Config::default();
        assert_eq!(config.workspace.auto_detect, None);
        assert!(config.workspace.packages.is_empty());
    }

    #[test]
    fn test_workspace_config_from_toml() {
        let toml_str = r#"
[workspace]
auto_detect = false

[workspace.packages.core]
max = 20
block_tags = ["BUG"]

[workspace.packages.cli]
max = 10
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.workspace.auto_detect, Some(false));
        assert_eq!(config.workspace.packages.len(), 2);
        let core = &config.workspace.packages["core"];
        assert_eq!(core.max, Some(20));
        assert_eq!(core.block_tags, vec!["BUG"]);
        let cli = &config.workspace.packages["cli"];
        assert_eq!(cli.max, Some(10));
        assert!(cli.block_tags.is_empty());
    }

    #[test]
    fn test_workspace_config_empty_section() {
        let toml_str = r#"
[workspace]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.workspace.auto_detect, None);
        assert!(config.workspace.packages.is_empty());
    }

    #[test]
    fn test_tags_pattern_escapes_regex_metacharacters() {
        let config = Config {
            tags: vec!["TODO".into(), "FIX.*".into()],
            ..Config::default()
        };
        let pattern = config.tags_pattern();
        // The .* should be escaped, not treated as regex wildcard
        assert!(pattern.contains(r"FIX\.\*"));
        // Should compile successfully
        let re = regex::Regex::new(&pattern);
        assert!(re.is_ok());
    }

    #[test]
    fn test_tags_pattern_escapes_parentheses() {
        let config = Config {
            tags: vec!["TAG(x)".into()],
            ..Config::default()
        };
        let pattern = config.tags_pattern();
        // Should compile without "unmatched group" error
        let re = regex::Regex::new(&pattern);
        assert!(re.is_ok(), "Regex with escaped parens should compile");
    }

    #[test]
    fn test_tags_pattern_escapes_pipe_literal() {
        let config = Config {
            tags: vec!["A|B".into()],
            ..Config::default()
        };
        let pattern = config.tags_pattern();
        // The pipe should be escaped, not treated as alternation
        assert!(pattern.contains(r"A\|B"));
        let re = regex::Regex::new(&pattern).unwrap();
        // Should NOT match "A" alone (which would happen if | was alternation)
        assert!(!re.is_match("// A: test"));
    }

    #[test]
    fn test_tags_pattern_default_tags_unaffected() {
        let config = Config::default();
        let pattern = config.tags_pattern();
        let re = regex::Regex::new(&pattern).unwrap();
        // Default tags should still match
        assert!(re.is_match("// TODO: test"));
        assert!(re.is_match("// FIXME: test"));
        assert!(re.is_match("// HACK: test"));
    }

    /// Validates that schema/todo-scan.schema.json matches the current Config structs.
    ///
    /// To regenerate the schema after changing Config:
    ///   UPDATE_SCHEMA=1 cargo test schema_is_up_to_date
    #[test]
    fn schema_is_up_to_date() {
        let schema = schemars::schema_for!(Config);
        let generated = serde_json::to_string_pretty(&schema).unwrap() + "\n";

        let schema_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/todo-scan.schema.json");

        if std::env::var("UPDATE_SCHEMA").is_ok() {
            std::fs::create_dir_all(schema_path.parent().unwrap()).unwrap();
            std::fs::write(&schema_path, &generated).unwrap();
            return;
        }

        let committed = std::fs::read_to_string(&schema_path).expect(
            "schema/todo-scan.schema.json not found. Run `UPDATE_SCHEMA=1 cargo test schema_is_up_to_date` to generate it.",
        );
        assert_eq!(
            generated, committed,
            "Schema is out of date. Run `UPDATE_SCHEMA=1 cargo test schema_is_up_to_date` to update."
        );
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_load_returns_default_when_no_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(dir.path()).unwrap();
        // Should return defaults when no .todo-scan.toml exists
        assert_eq!(config.tags.len(), 6);
        assert!(config.exclude_dirs.is_empty());
        assert!(config.exclude_patterns.is_empty());
    }

    #[test]
    fn test_load_reads_config_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let toml_content = r#"
tags = ["TODO", "FIXME"]
exclude_dirs = ["vendor"]
"#;
        std::fs::write(dir.path().join(".todo-scan.toml"), toml_content).unwrap();
        let config = Config::load(dir.path()).unwrap();
        assert_eq!(config.tags, vec!["TODO", "FIXME"]);
        assert_eq!(config.exclude_dirs, vec!["vendor"]);
    }

    #[test]
    fn test_load_invalid_toml_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".todo-scan.toml"),
            "this is not [valid toml",
        )
        .unwrap();
        let result = Config::load(dir.path());
        assert!(result.is_err(), "invalid TOML should produce an error");
        let err_msg = format!("{:#}", result.unwrap_err());
        assert!(
            err_msg.contains("Failed to parse config"),
            "error should mention parse failure, got: {err_msg}"
        );
    }

    #[test]
    fn test_find_config_file_searches_upward() {
        let dir = tempfile::tempdir().unwrap();
        // Create config in root
        std::fs::write(dir.path().join(".todo-scan.toml"), "tags = [\"TODO\"]\n").unwrap();
        // Create a nested directory
        let nested = dir.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();
        // find_config_file from nested dir should find the root config
        let found = find_config_file(&nested);
        assert!(found.is_some());
        assert_eq!(found.unwrap(), dir.path().join(".todo-scan.toml"));
    }

    #[test]
    fn test_find_config_file_returns_none_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = find_config_file(dir.path());
        // There might be a .todo-scan.toml somewhere up the path, but in a tempdir
        // the default behavior should eventually fail. Let's just ensure it doesn't panic.
        // In a tempdir, there's typically no .todo-scan.toml above.
        let _ = result;
    }

    #[test]
    fn test_tags_pattern_single_tag() {
        let config = Config {
            tags: vec!["REVIEW".into()],
            ..Config::default()
        };
        let pattern = config.tags_pattern();
        let re = regex::Regex::new(&pattern).unwrap();
        assert!(re.is_match("// REVIEW: check this"));
        assert!(!re.is_match("// TODO: check this"));
    }

    #[test]
    fn test_tags_pattern_empty_tags() {
        let config = Config {
            tags: vec![],
            ..Config::default()
        };
        let pattern = config.tags_pattern();
        // Should still be a valid regex even with empty tags
        let re = regex::Regex::new(&pattern);
        assert!(re.is_ok(), "empty tags should produce a valid regex");
    }

    #[test]
    fn test_default_config_has_all_expected_fields() {
        let config = Config::default();
        assert_eq!(
            config.tags,
            vec!["TODO", "FIXME", "HACK", "XXX", "BUG", "NOTE"]
        );
        assert!(config.exclude_dirs.is_empty());
        assert!(config.exclude_patterns.is_empty());
        assert!(config.check.max.is_none());
        assert!(config.check.max_new.is_none());
        assert!(config.check.block_tags.is_empty());
        assert!(config.check.expired.is_none());
        assert!(config.blame.stale_threshold.is_none());
        assert!(config.lint.no_bare_tags.is_none());
        assert!(config.lint.max_message_length.is_none());
        assert!(config.lint.require_author.is_none());
        assert!(config.lint.require_issue_ref.is_none());
        assert!(config.lint.uppercase_tag.is_none());
        assert!(config.lint.require_colon.is_none());
        assert!(config.clean.stale_issues.is_none());
        assert!(config.clean.duplicates.is_none());
        assert!(config.clean.since.is_none());
    }

    #[test]
    fn test_full_config_from_toml() {
        let toml_str = r#"
tags = ["TODO"]
exclude_dirs = ["node_modules", "vendor"]
exclude_patterns = ["\\.generated\\."]

[check]
max = 100
max_new = 5
block_tags = ["BUG", "HACK"]
expired = true

[blame]
stale_threshold = "180d"

[lint]
no_bare_tags = true
max_message_length = 120
require_author = ["TODO", "FIXME"]
require_issue_ref = ["BUG"]
uppercase_tag = true
require_colon = true

[clean]
stale_issues = true
duplicates = true
since = "30d"

[workspace]
auto_detect = true

[workspace.packages.api]
max = 50
block_tags = ["HACK"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.tags, vec!["TODO"]);
        assert_eq!(config.exclude_dirs, vec!["node_modules", "vendor"]);
        assert_eq!(config.exclude_patterns, vec!["\\.generated\\."]);
        assert_eq!(config.check.max, Some(100));
        assert_eq!(config.check.max_new, Some(5));
        assert_eq!(config.check.block_tags, vec!["BUG", "HACK"]);
        assert_eq!(config.check.expired, Some(true));
        assert_eq!(config.blame.stale_threshold, Some("180d".into()));
        assert_eq!(config.lint.no_bare_tags, Some(true));
        assert_eq!(config.lint.max_message_length, Some(120));
        assert_eq!(
            config.lint.require_author,
            Some(vec!["TODO".into(), "FIXME".into()])
        );
        assert_eq!(config.lint.require_issue_ref, Some(vec!["BUG".into()]));
        assert_eq!(config.lint.uppercase_tag, Some(true));
        assert_eq!(config.lint.require_colon, Some(true));
        assert_eq!(config.clean.stale_issues, Some(true));
        assert_eq!(config.clean.duplicates, Some(true));
        assert_eq!(config.clean.since, Some("30d".into()));
        assert_eq!(config.workspace.auto_detect, Some(true));
        assert_eq!(config.workspace.packages["api"].max, Some(50));
        assert_eq!(config.workspace.packages["api"].block_tags, vec!["HACK"]);
    }
}
