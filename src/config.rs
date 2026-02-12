use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub tags: Vec<String>,
    pub exclude_dirs: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub check: CheckConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct CheckConfig {
    pub max: Option<usize>,
    pub max_new: Option<usize>,
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
        }
    }
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            max: None,
            max_new: None,
            block_tags: vec![],
        }
    }
}

impl Config {
    /// Build regex pattern from configured tags
    pub fn tags_pattern(&self) -> String {
        let tags = self.tags.join("|");
        format!(r"(?i)\b({tags})(?:\(([\w@.\-]+)\))?:?\s*(!{{1,2}})?\s*(.+)$")
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
}
