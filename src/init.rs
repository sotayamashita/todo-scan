use std::path::Path;

use anyhow::{bail, Result};
use dialoguer::{Confirm, Input, MultiSelect};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

const ALL_TAGS: &[&str] = &["TODO", "FIXME", "HACK", "XXX", "BUG", "NOTE"];

struct ProjectHint {
    name: &'static str,
    exclude_dirs: Vec<&'static str>,
}

fn detect_projects(root: &Path) -> Vec<ProjectHint> {
    let mut hints = Vec::new();

    if root.join("Cargo.toml").exists() {
        hints.push(ProjectHint {
            name: "Rust",
            exclude_dirs: vec!["target"],
        });
    }
    if root.join("package.json").exists() {
        hints.push(ProjectHint {
            name: "JavaScript/TypeScript",
            exclude_dirs: vec!["node_modules", "dist", ".next"],
        });
    }
    if root.join("go.mod").exists() {
        hints.push(ProjectHint {
            name: "Go",
            exclude_dirs: vec!["vendor"],
        });
    }
    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        hints.push(ProjectHint {
            name: "Python",
            exclude_dirs: vec![".venv", "__pycache__", ".tox"],
        });
    }

    hints
}

fn collect_suggested_dirs(hints: &[ProjectHint]) -> Vec<&'static str> {
    let mut dirs = Vec::new();
    for hint in hints {
        for dir in &hint.exclude_dirs {
            if !dirs.contains(dir) {
                dirs.push(dir);
            }
        }
    }
    dirs
}

pub fn cmd_init(root: &Path, non_interactive: bool) -> Result<()> {
    let config_path = root.join(".todox.toml");

    // Check for existing config
    if config_path.exists() {
        if non_interactive {
            bail!(".todox.toml already exists. Use interactive mode to overwrite.");
        }
        let overwrite = Confirm::new()
            .with_prompt(".todox.toml already exists. Overwrite?")
            .default(false)
            .interact()?;
        if !overwrite {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    // Detect project types
    let hints = detect_projects(root);
    if !hints.is_empty() {
        let names: Vec<&str> = hints.iter().map(|h| h.name).collect();
        eprintln!("Detected project type(s): {}", names.join(", "));
    }

    // Select tags
    let selected_tags: Vec<String> = if non_interactive {
        ALL_TAGS.iter().map(|s| s.to_string()).collect()
    } else {
        let defaults: Vec<bool> = vec![true; ALL_TAGS.len()];
        let indices = MultiSelect::new()
            .with_prompt("Select tags to track")
            .items(ALL_TAGS)
            .defaults(&defaults)
            .interact()?;
        indices.iter().map(|&i| ALL_TAGS[i].to_string()).collect()
    };

    // Select exclude dirs
    let suggested_dirs = collect_suggested_dirs(&hints);
    let selected_dirs: Vec<String> = if non_interactive {
        suggested_dirs.iter().map(|s| s.to_string()).collect()
    } else if suggested_dirs.is_empty() {
        Vec::new()
    } else {
        let defaults: Vec<bool> = vec![true; suggested_dirs.len()];
        let indices = MultiSelect::new()
            .with_prompt("Select directories to exclude")
            .items(&suggested_dirs)
            .defaults(&defaults)
            .interact()?;
        indices
            .iter()
            .map(|&i| suggested_dirs[i].to_string())
            .collect()
    };

    // CI check max
    let check_max: Option<usize> = if non_interactive {
        None
    } else {
        let want_max = Confirm::new()
            .with_prompt("Set a maximum TODO count for CI checks?")
            .default(false)
            .interact()?;
        if want_max {
            let max: usize = Input::new()
                .with_prompt("Maximum TODO count")
                .default(100)
                .interact()?;
            Some(max)
        } else {
            None
        }
    };

    // Generate TOML
    let content = build_config_toml(&selected_tags, &selected_dirs, check_max);
    std::fs::write(&config_path, content)?;

    eprintln!("Created .todox.toml");
    eprintln!("Try it out: todox list");
    Ok(())
}

fn build_config_toml(tags: &[String], exclude_dirs: &[String], check_max: Option<usize>) -> String {
    let mut doc = DocumentMut::new();

    // tags
    let mut tag_array = Array::new();
    for tag in tags {
        tag_array.push(tag.as_str());
    }
    doc["tags"] = Item::Value(Value::Array(tag_array));

    // exclude_dirs
    let mut dir_array = Array::new();
    for dir in exclude_dirs {
        dir_array.push(dir.as_str());
    }
    doc["exclude_dirs"] = Item::Value(Value::Array(dir_array));

    // [check] section
    if let Some(max) = check_max {
        let mut check_table = Table::new();
        check_table["max"] = Item::Value(Value::from(max as i64));
        doc["check"] = Item::Table(check_table);
    }

    doc.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust_project() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let hints = detect_projects(dir.path());
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].name, "Rust");
        assert!(hints[0].exclude_dirs.contains(&"target"));
    }

    #[test]
    fn test_detect_node_project() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        let hints = detect_projects(dir.path());
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].name, "JavaScript/TypeScript");
        assert!(hints[0].exclude_dirs.contains(&"node_modules"));
    }

    #[test]
    fn test_detect_go_project() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module example").unwrap();
        let hints = detect_projects(dir.path());
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].name, "Go");
    }

    #[test]
    fn test_detect_python_project() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("pyproject.toml"), "[tool]").unwrap();
        let hints = detect_projects(dir.path());
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].name, "Python");
    }

    #[test]
    fn test_detect_no_project() {
        let dir = tempfile::TempDir::new().unwrap();
        let hints = detect_projects(dir.path());
        assert!(hints.is_empty());
    }

    #[test]
    fn test_collect_suggested_dirs_deduplicates() {
        let hints = vec![
            ProjectHint {
                name: "A",
                exclude_dirs: vec!["dist"],
            },
            ProjectHint {
                name: "B",
                exclude_dirs: vec!["dist", "vendor"],
            },
        ];
        let dirs = collect_suggested_dirs(&hints);
        assert_eq!(dirs, vec!["dist", "vendor"]);
    }

    #[test]
    fn test_build_config_toml_basic() {
        let tags = vec!["TODO".to_string(), "FIXME".to_string()];
        let dirs = vec!["target".to_string()];
        let content = build_config_toml(&tags, &dirs, None);
        assert!(content.contains("TODO"));
        assert!(content.contains("FIXME"));
        assert!(content.contains("target"));
        assert!(!content.contains("[check]"));
    }

    #[test]
    fn test_build_config_toml_with_check_max() {
        let tags = vec!["TODO".to_string()];
        let dirs = vec![];
        let content = build_config_toml(&tags, &dirs, Some(50));
        assert!(content.contains("[check]"));
        assert!(content.contains("max = 50"));
    }

    #[test]
    fn test_build_config_toml_parseable() {
        let tags = vec!["TODO".to_string(), "FIXME".to_string()];
        let dirs = vec!["target".to_string(), "node_modules".to_string()];
        let content = build_config_toml(&tags, &dirs, Some(100));
        let parsed: crate::config::Config = toml::from_str(&content).unwrap();
        assert_eq!(parsed.tags, vec!["TODO", "FIXME"]);
        assert_eq!(parsed.exclude_dirs, vec!["target", "node_modules"]);
        assert_eq!(parsed.check.max, Some(100));
    }
}
