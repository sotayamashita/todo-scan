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
    let config_path = root.join(".todo-scan.toml");

    // Check for existing config
    if config_path.exists() {
        if non_interactive {
            bail!(".todo-scan.toml already exists. Use interactive mode to overwrite.");
        }
        let overwrite = Confirm::new()
            .with_prompt(".todo-scan.toml already exists. Overwrite?")
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

    eprintln!("Created .todo-scan.toml");
    eprintln!("Try it out: todo-scan list");
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

    #[test]
    fn test_cmd_init_non_interactive_creates_config() {
        let dir = tempfile::TempDir::new().unwrap();
        let result = cmd_init(dir.path(), true);
        assert!(result.is_ok(), "cmd_init should succeed: {:?}", result);
        let config_path = dir.path().join(".todo-scan.toml");
        assert!(config_path.exists(), ".todo-scan.toml should be created");
        let content = std::fs::read_to_string(&config_path).unwrap();
        // Non-interactive mode includes all tags
        for tag in ALL_TAGS {
            assert!(content.contains(tag), "config should contain tag {}", tag);
        }
        // Should be parseable
        let parsed: crate::config::Config = toml::from_str(&content).unwrap();
        assert_eq!(parsed.tags.len(), ALL_TAGS.len());
    }

    #[test]
    fn test_cmd_init_non_interactive_fails_if_exists() {
        let dir = tempfile::TempDir::new().unwrap();
        let config_path = dir.path().join(".todo-scan.toml");
        std::fs::write(&config_path, "tags = [\"TODO\"]").unwrap();
        let result = cmd_init(dir.path(), true);
        assert!(result.is_err(), "cmd_init should fail when config exists");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("already exists"),
            "error should mention 'already exists', got: {}",
            err_msg
        );
    }

    #[test]
    fn test_detect_python_requirements_txt() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "flask==2.0").unwrap();
        let hints = detect_projects(dir.path());
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].name, "Python");
        assert!(hints[0].exclude_dirs.contains(&".venv"));
        assert!(hints[0].exclude_dirs.contains(&"__pycache__"));
        assert!(hints[0].exclude_dirs.contains(&".tox"));
    }

    #[test]
    fn test_detect_multiple_project_types() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        let hints = detect_projects(dir.path());
        assert_eq!(hints.len(), 2);
        let names: Vec<&str> = hints.iter().map(|h| h.name).collect();
        assert!(names.contains(&"Rust"), "should detect Rust");
        assert!(
            names.contains(&"JavaScript/TypeScript"),
            "should detect JS/TS"
        );
    }

    #[test]
    fn test_collect_suggested_dirs_empty() {
        let hints: Vec<ProjectHint> = vec![];
        let dirs = collect_suggested_dirs(&hints);
        assert!(dirs.is_empty(), "empty hints should produce empty dirs");
    }

    #[test]
    fn test_build_config_toml_empty() {
        let tags: Vec<String> = vec![];
        let dirs: Vec<String> = vec![];
        let content = build_config_toml(&tags, &dirs, None);
        // Should still be valid TOML with empty arrays
        assert!(content.contains("tags"));
        assert!(content.contains("exclude_dirs"));
        assert!(!content.contains("[check]"));
        let parsed: DocumentMut = content.parse().expect("should be valid TOML");
        let tag_arr = parsed["tags"].as_array().unwrap();
        assert_eq!(tag_arr.len(), 0);
        let dir_arr = parsed["exclude_dirs"].as_array().unwrap();
        assert_eq!(dir_arr.len(), 0);
    }

    #[test]
    fn test_build_config_toml_all_tags() {
        let tags: Vec<String> = ALL_TAGS.iter().map(|s| s.to_string()).collect();
        let dirs = vec!["target".to_string(), "node_modules".to_string()];
        let content = build_config_toml(&tags, &dirs, None);
        for tag in ALL_TAGS {
            assert!(content.contains(tag), "config should contain tag {}", tag);
        }
        let parsed: DocumentMut = content.parse().expect("should be valid TOML");
        let tag_arr = parsed["tags"].as_array().unwrap();
        assert_eq!(tag_arr.len(), ALL_TAGS.len());
    }

    // --- cmd_init non-interactive with project-type detection ---

    #[test]
    fn test_cmd_init_non_interactive_rust_project_includes_target() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let result = cmd_init(dir.path(), true);
        assert!(result.is_ok(), "cmd_init should succeed: {:?}", result);
        let content = std::fs::read_to_string(dir.path().join(".todo-scan.toml")).unwrap();
        let parsed: crate::config::Config = toml::from_str(&content).unwrap();
        assert!(
            parsed.exclude_dirs.contains(&"target".to_string()),
            "Rust project should include 'target' in exclude_dirs, got: {:?}",
            parsed.exclude_dirs
        );
    }

    #[test]
    fn test_cmd_init_non_interactive_node_project_includes_node_modules() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        let result = cmd_init(dir.path(), true);
        assert!(result.is_ok(), "cmd_init should succeed: {:?}", result);
        let content = std::fs::read_to_string(dir.path().join(".todo-scan.toml")).unwrap();
        let parsed: crate::config::Config = toml::from_str(&content).unwrap();
        assert!(
            parsed.exclude_dirs.contains(&"node_modules".to_string()),
            "Node.js project should include 'node_modules' in exclude_dirs, got: {:?}",
            parsed.exclude_dirs
        );
    }

    #[test]
    fn test_cmd_init_non_interactive_go_project_includes_vendor() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module example").unwrap();
        let result = cmd_init(dir.path(), true);
        assert!(result.is_ok(), "cmd_init should succeed: {:?}", result);
        let content = std::fs::read_to_string(dir.path().join(".todo-scan.toml")).unwrap();
        let parsed: crate::config::Config = toml::from_str(&content).unwrap();
        assert!(
            parsed.exclude_dirs.contains(&"vendor".to_string()),
            "Go project should include 'vendor' in exclude_dirs, got: {:?}",
            parsed.exclude_dirs
        );
    }

    // --- collect_suggested_dirs edge cases ---

    #[test]
    fn test_collect_suggested_dirs_single_hint() {
        let hints = vec![ProjectHint {
            name: "Rust",
            exclude_dirs: vec!["target"],
        }];
        let dirs = collect_suggested_dirs(&hints);
        assert_eq!(dirs, vec!["target"]);
    }

    #[test]
    fn test_collect_suggested_dirs_multiple_hints_overlapping() {
        let hints = vec![
            ProjectHint {
                name: "Rust",
                exclude_dirs: vec!["target"],
            },
            ProjectHint {
                name: "JavaScript/TypeScript",
                exclude_dirs: vec!["node_modules", "dist", ".next"],
            },
            ProjectHint {
                name: "Go",
                exclude_dirs: vec!["vendor"],
            },
        ];
        let dirs = collect_suggested_dirs(&hints);
        assert_eq!(
            dirs,
            vec!["target", "node_modules", "dist", ".next", "vendor"]
        );
    }

    #[test]
    fn test_collect_suggested_dirs_full_overlap_deduplicates() {
        // Both hints share the exact same dir
        let hints = vec![
            ProjectHint {
                name: "A",
                exclude_dirs: vec!["shared_dir", "only_a"],
            },
            ProjectHint {
                name: "B",
                exclude_dirs: vec!["shared_dir", "only_b"],
            },
        ];
        let dirs = collect_suggested_dirs(&hints);
        assert_eq!(dirs, vec!["shared_dir", "only_a", "only_b"]);
        // Verify no duplicates
        let unique: std::collections::HashSet<&&str> = dirs.iter().collect();
        assert_eq!(unique.len(), dirs.len(), "should have no duplicates");
    }
}
