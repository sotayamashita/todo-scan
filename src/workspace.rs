use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::model::{PackageInfo, WorkspaceInfo, WorkspaceKind};

/// Detect workspace configuration at the given root directory.
/// Tries detectors in order: Cargo > npm > pnpm > Nx > Go > manual config fallback.
pub fn detect_workspace(root: &Path, config: &Config) -> Result<Option<WorkspaceInfo>> {
    // If auto_detect is explicitly disabled, only use manual config
    if config.workspace.auto_detect == Some(false) {
        return detect_manual(root, config);
    }

    if let Some(ws) = detect_cargo(root)? {
        return Ok(Some(ws));
    }
    if let Some(ws) = detect_npm(root)? {
        return Ok(Some(ws));
    }
    if let Some(ws) = detect_pnpm(root)? {
        return Ok(Some(ws));
    }
    if let Some(ws) = detect_nx(root)? {
        return Ok(Some(ws));
    }
    if let Some(ws) = detect_go_work(root)? {
        return Ok(Some(ws));
    }

    detect_manual(root, config)
}

fn detect_cargo(root: &Path) -> Result<Option<WorkspaceInfo>> {
    let cargo_toml = root.join("Cargo.toml");
    if !cargo_toml.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&cargo_toml)?;
    let doc: toml::Value = toml::from_str(&content)?;

    let members = doc
        .get("workspace")
        .and_then(|ws| ws.get("members"))
        .and_then(|m| m.as_array());

    let members = match members {
        Some(m) => m,
        None => return Ok(None),
    };

    let patterns: Vec<String> = members
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let mut packages = resolve_member_paths(root, &patterns, WorkspaceKind::Cargo);
    packages.sort_by(|a, b| a.path.cmp(&b.path));

    if packages.is_empty() {
        return Ok(None);
    }

    Ok(Some(WorkspaceInfo {
        kind: WorkspaceKind::Cargo,
        packages,
    }))
}

fn detect_npm(root: &Path) -> Result<Option<WorkspaceInfo>> {
    let pkg_json = root.join("package.json");
    if !pkg_json.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&pkg_json)?;
    let doc: serde_json::Value = serde_json::from_str(&content)?;

    let workspaces = match doc.get("workspaces").and_then(|w| w.as_array()) {
        Some(w) => w,
        None => return Ok(None),
    };

    let patterns: Vec<String> = workspaces
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let mut packages = resolve_member_paths(root, &patterns, WorkspaceKind::Npm);
    packages.sort_by(|a, b| a.path.cmp(&b.path));

    if packages.is_empty() {
        return Ok(None);
    }

    Ok(Some(WorkspaceInfo {
        kind: WorkspaceKind::Npm,
        packages,
    }))
}

fn detect_pnpm(root: &Path) -> Result<Option<WorkspaceInfo>> {
    let yaml_path = root.join("pnpm-workspace.yaml");
    if !yaml_path.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&yaml_path)?;

    // Simple YAML parser: look for lines under "packages:" that start with "- "
    let mut patterns = Vec::new();
    let mut in_packages = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }
        if in_packages {
            if trimmed.starts_with("- ") {
                let value = trimmed
                    .trim_start_matches("- ")
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string();
                patterns.push(value);
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                break;
            }
        }
    }

    if patterns.is_empty() {
        return Ok(None);
    }

    let mut packages = resolve_member_paths(root, &patterns, WorkspaceKind::Pnpm);
    packages.sort_by(|a, b| a.path.cmp(&b.path));

    if packages.is_empty() {
        return Ok(None);
    }

    Ok(Some(WorkspaceInfo {
        kind: WorkspaceKind::Pnpm,
        packages,
    }))
}

fn detect_nx(root: &Path) -> Result<Option<WorkspaceInfo>> {
    let ws_json = root.join("workspace.json");
    if !ws_json.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&ws_json)?;
    let doc: serde_json::Value = serde_json::from_str(&content)?;

    let projects = match doc.get("projects").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return Ok(None),
    };

    let mut packages: Vec<PackageInfo> = projects
        .iter()
        .filter_map(|(name, value)| {
            let path = value.as_str()?;
            if root.join(path).is_dir() {
                Some(PackageInfo {
                    name: name.clone(),
                    path: path.to_string(),
                    kind: WorkspaceKind::Nx,
                })
            } else {
                None
            }
        })
        .collect();

    packages.sort_by(|a, b| a.name.cmp(&b.name));

    if packages.is_empty() {
        return Ok(None);
    }

    Ok(Some(WorkspaceInfo {
        kind: WorkspaceKind::Nx,
        packages,
    }))
}

fn detect_go_work(root: &Path) -> Result<Option<WorkspaceInfo>> {
    let go_work = root.join("go.work");
    if !go_work.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&go_work)?;

    // Parse "use" directives from go.work
    // Format: use ( ./path1 \n ./path2 ) or use ./single
    let mut paths = Vec::new();
    let mut in_use_block = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("use (") || trimmed == "use (" {
            in_use_block = true;
            continue;
        }
        if in_use_block {
            if trimmed == ")" {
                in_use_block = false;
                continue;
            }
            let path = trimmed.strip_prefix("./").unwrap_or(trimmed);
            if !path.is_empty() {
                paths.push(path.to_string());
            }
        } else if trimmed.starts_with("use ") {
            let path = trimmed
                .trim_start_matches("use ")
                .trim()
                .strip_prefix("./")
                .unwrap_or(trimmed.trim_start_matches("use ").trim());
            if !path.is_empty() {
                paths.push(path.to_string());
            }
        }
    }

    let mut packages: Vec<PackageInfo> = paths
        .into_iter()
        .filter(|p| root.join(p).is_dir())
        .map(|p| PackageInfo {
            name: package_name_from_path(&p),
            path: p,
            kind: WorkspaceKind::GoWork,
        })
        .collect();

    packages.sort_by(|a, b| a.path.cmp(&b.path));

    if packages.is_empty() {
        return Ok(None);
    }

    Ok(Some(WorkspaceInfo {
        kind: WorkspaceKind::GoWork,
        packages,
    }))
}

fn detect_manual(_root: &Path, config: &Config) -> Result<Option<WorkspaceInfo>> {
    if config.workspace.packages.is_empty() {
        return Ok(None);
    }

    let mut packages: Vec<PackageInfo> = config
        .workspace
        .packages
        .keys()
        .map(|name| PackageInfo {
            name: name.clone(),
            path: name.clone(),
            kind: WorkspaceKind::Manual,
        })
        .collect();

    packages.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Some(WorkspaceInfo {
        kind: WorkspaceKind::Manual,
        packages,
    }))
}

/// Resolve a list of member patterns (direct paths or globs) to PackageInfo items.
fn resolve_member_paths(root: &Path, patterns: &[String], kind: WorkspaceKind) -> Vec<PackageInfo> {
    let mut packages = Vec::new();
    for pattern in patterns {
        // Check if it's a literal directory path (no glob chars)
        if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
            let cleaned = pattern.strip_prefix("./").unwrap_or(pattern);
            let abs = root.join(cleaned);
            if abs.is_dir() {
                packages.push(PackageInfo {
                    name: package_name_from_path(cleaned),
                    path: cleaned.to_string(),
                    kind,
                });
            }
            continue;
        }
        // It's a glob pattern
        let expanded = expand_globs(root, std::slice::from_ref(pattern));
        for path in expanded {
            packages.push(PackageInfo {
                name: package_name_from_path(&path),
                path,
                kind,
            });
        }
    }
    packages
}

/// Expand glob patterns against the filesystem to find matching directories.
/// Supports patterns like "packages/*", "crates/*", etc.
fn expand_globs(root: &Path, patterns: &[String]) -> Vec<String> {
    let mut results = Vec::new();
    for pattern in patterns {
        let glob = match globset::Glob::new(pattern) {
            Ok(g) => g.compile_matcher(),
            Err(_) => continue,
        };

        // Determine the fixed prefix before any glob characters
        let prefix = glob_prefix(pattern);
        let search_root = root.join(&prefix);

        if !search_root.is_dir() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&search_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    if glob.is_match(&rel) {
                        results.push(rel);
                    }
                }
            }
        }
    }
    results.sort();
    results.dedup();
    results
}

/// Extract the fixed directory prefix from a glob pattern.
/// e.g., "packages/*" -> "packages", "crates/*/src" -> "crates"
fn glob_prefix(pattern: &str) -> String {
    let mut parts = Vec::new();
    for part in pattern.split('/') {
        if part.contains('*') || part.contains('?') || part.contains('[') {
            break;
        }
        parts.push(part);
    }
    parts.join("/")
}

/// Derive a package name from a directory path.
/// Uses the last component of the path.
fn package_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn no_workspace_detected_in_empty_dir() {
        let dir = TempDir::new().unwrap();
        let config = Config::default();
        let result = detect_workspace(dir.path(), &config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_cargo_workspace() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/core", "crates/cli"]
"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("crates/core")).unwrap();
        std::fs::create_dir_all(dir.path().join("crates/cli")).unwrap();

        let config = Config::default();
        let result = detect_workspace(dir.path(), &config).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::Cargo);
        assert_eq!(ws.packages.len(), 2);
        assert_eq!(ws.packages[0].name, "cli");
        assert_eq!(ws.packages[0].path, "crates/cli");
        assert_eq!(ws.packages[1].name, "core");
        assert_eq!(ws.packages[1].path, "crates/core");
    }

    #[test]
    fn detect_npm_workspace() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"workspaces": ["packages/*"]}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("packages/alpha")).unwrap();
        std::fs::create_dir_all(dir.path().join("packages/beta")).unwrap();

        let config = Config::default();
        let result = detect_workspace(dir.path(), &config).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::Npm);
        assert_eq!(ws.packages.len(), 2);
        assert_eq!(ws.packages[0].name, "alpha");
        assert_eq!(ws.packages[1].name, "beta");
    }

    #[test]
    fn detect_pnpm_workspace() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*'\n  - 'libs/*'\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("apps/web")).unwrap();
        std::fs::create_dir_all(dir.path().join("libs/shared")).unwrap();

        let config = Config::default();
        let result = detect_workspace(dir.path(), &config).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::Pnpm);
        assert_eq!(ws.packages.len(), 2);
    }

    #[test]
    fn detect_go_workspace() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("go.work"),
            "go 1.21\n\nuse (\n\t./cmd/server\n\t./pkg/lib\n)\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("cmd/server")).unwrap();
        std::fs::create_dir_all(dir.path().join("pkg/lib")).unwrap();

        let config = Config::default();
        let result = detect_workspace(dir.path(), &config).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::GoWork);
        assert_eq!(ws.packages.len(), 2);
    }

    #[test]
    fn detect_nx_workspace() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("workspace.json"),
            r#"{"projects": {"app": "apps/app", "lib": "libs/lib"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("apps/app")).unwrap();
        std::fs::create_dir_all(dir.path().join("libs/lib")).unwrap();

        let config = Config::default();
        let result = detect_workspace(dir.path(), &config).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::Nx);
        assert_eq!(ws.packages.len(), 2);
    }

    #[test]
    fn detect_manual_workspace() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("svc/auth")).unwrap();
        std::fs::create_dir_all(dir.path().join("svc/api")).unwrap();

        let mut config = Config::default();
        config.workspace.auto_detect = Some(false);
        config.workspace.packages.insert(
            "auth".to_string(),
            crate::config::PackageCheckConfig {
                max: Some(10),
                block_tags: vec![],
            },
        );
        config.workspace.packages.insert(
            "api".to_string(),
            crate::config::PackageCheckConfig {
                max: Some(20),
                block_tags: vec![],
            },
        );

        // Manual mode with no actual workspace files
        let result = detect_workspace(dir.path(), &config).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::Manual);
    }

    #[test]
    fn auto_detect_disabled_skips_cargo() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/core"]
"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("crates/core")).unwrap();

        let mut config = Config::default();
        config.workspace.auto_detect = Some(false);
        // No manual packages configured, so should return None
        let result = detect_workspace(dir.path(), &config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn cargo_glob_members() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/*"]
"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("crates/alpha")).unwrap();
        std::fs::create_dir_all(dir.path().join("crates/beta")).unwrap();

        let config = Config::default();
        let result = detect_workspace(dir.path(), &config).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::Cargo);
        assert_eq!(ws.packages.len(), 2);
    }

    // --- glob_prefix tests ---

    #[test]
    fn glob_prefix_packages_star() {
        assert_eq!(glob_prefix("packages/*"), "packages");
    }

    #[test]
    fn glob_prefix_nested_glob() {
        assert_eq!(glob_prefix("crates/*/src"), "crates");
    }

    #[test]
    fn glob_prefix_star_only() {
        assert_eq!(glob_prefix("*"), "");
    }

    #[test]
    fn glob_prefix_empty_pattern() {
        assert_eq!(glob_prefix(""), "");
    }

    #[test]
    fn glob_prefix_no_glob_chars() {
        assert_eq!(glob_prefix("some/literal/path"), "some/literal/path");
    }

    #[test]
    fn glob_prefix_question_mark() {
        assert_eq!(glob_prefix("dir/?"), "dir");
    }

    #[test]
    fn glob_prefix_bracket_pattern() {
        assert_eq!(glob_prefix("dir/[abc]"), "dir");
    }

    // --- package_name_from_path tests ---

    #[test]
    fn package_name_from_path_nested() {
        assert_eq!(package_name_from_path("crates/core"), "core");
    }

    #[test]
    fn package_name_from_path_single() {
        assert_eq!(package_name_from_path("single"), "single");
    }

    #[test]
    fn package_name_from_path_empty() {
        // Empty string: Path::new("").file_name() returns None, so fallback to the string itself
        assert_eq!(package_name_from_path(""), "");
    }

    #[test]
    fn package_name_from_path_trailing_slash() {
        // Path::new("foo/bar/") still yields file_name() == "bar"
        assert_eq!(package_name_from_path("foo/bar/"), "bar");
    }

    #[test]
    fn package_name_from_path_deeply_nested() {
        assert_eq!(package_name_from_path("a/b/c/d"), "d");
    }

    // --- detect_pnpm with comments and non-package section ---

    #[test]
    fn detect_pnpm_yaml_with_comments_between_entries() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*'\n  # this is a comment\n  - 'libs/*'\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("apps/web")).unwrap();
        std::fs::create_dir_all(dir.path().join("libs/shared")).unwrap();

        let result = detect_pnpm(dir.path()).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::Pnpm);
        assert_eq!(ws.packages.len(), 2);
    }

    #[test]
    fn detect_pnpm_yaml_non_package_section_breaks_parse() {
        let dir = TempDir::new().unwrap();
        // After the packages list entries, a non-package YAML key appears
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*'\nsomething_else:\n  - unrelated\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("apps/web")).unwrap();

        let result = detect_pnpm(dir.path()).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::Pnpm);
        // Should only pick up the one pattern before the break
        assert_eq!(ws.packages.len(), 1);
        assert_eq!(ws.packages[0].name, "web");
    }

    #[test]
    fn detect_pnpm_empty_packages_section() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\nother:\n  - foo\n",
        )
        .unwrap();

        let result = detect_pnpm(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "empty packages section should return None"
        );
    }

    // --- detect_go_work with single-line use ---

    #[test]
    fn detect_go_work_single_line_use() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("go.work"), "go 1.21\n\nuse ./mymodule\n").unwrap();
        std::fs::create_dir_all(dir.path().join("mymodule")).unwrap();

        let result = detect_go_work(dir.path()).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.kind, WorkspaceKind::GoWork);
        assert_eq!(ws.packages.len(), 1);
        assert_eq!(ws.packages[0].name, "mymodule");
        assert_eq!(ws.packages[0].path, "mymodule");
    }

    #[test]
    fn detect_go_work_single_line_use_without_dot_slash() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("go.work"), "go 1.21\n\nuse mylib\n").unwrap();
        std::fs::create_dir_all(dir.path().join("mylib")).unwrap();

        let result = detect_go_work(dir.path()).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.packages.len(), 1);
        assert_eq!(ws.packages[0].name, "mylib");
    }

    #[test]
    fn detect_go_work_mixed_block_and_single_line() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("go.work"),
            "go 1.21\n\nuse (\n\t./cmd/server\n)\n\nuse ./pkg/lib\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("cmd/server")).unwrap();
        std::fs::create_dir_all(dir.path().join("pkg/lib")).unwrap();

        let result = detect_go_work(dir.path()).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.packages.len(), 2);
    }

    // --- detect_cargo with no [workspace] section ---

    #[test]
    fn detect_cargo_no_workspace_section() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[package]
name = "my-app"
version = "0.1.0"
"#,
        )
        .unwrap();

        let result = detect_cargo(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "Cargo.toml without [workspace] should return None"
        );
    }

    #[test]
    fn detect_cargo_workspace_without_members() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[workspace]
resolver = "2"
"#,
        )
        .unwrap();

        let result = detect_cargo(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "workspace without members key should return None"
        );
    }

    // --- detect_npm with no workspaces field ---

    #[test]
    fn detect_npm_no_workspaces_field() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-app", "version": "1.0.0"}"#,
        )
        .unwrap();

        let result = detect_npm(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "package.json without workspaces should return None"
        );
    }

    #[test]
    fn detect_npm_workspaces_empty_array() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-app", "workspaces": []}"#,
        )
        .unwrap();

        let result = detect_npm(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "empty workspaces array should return None"
        );
    }

    // --- resolve_member_paths with non-existent directory ---

    #[test]
    fn resolve_member_paths_nonexistent_dir() {
        let dir = TempDir::new().unwrap();
        let patterns = vec!["nonexistent/path".to_string()];
        let result = resolve_member_paths(dir.path(), &patterns, WorkspaceKind::Cargo);
        assert!(
            result.is_empty(),
            "non-existent directory should produce empty result"
        );
    }

    #[test]
    fn resolve_member_paths_glob_nonexistent_dir() {
        let dir = TempDir::new().unwrap();
        let patterns = vec!["nonexistent/*".to_string()];
        let result = resolve_member_paths(dir.path(), &patterns, WorkspaceKind::Npm);
        assert!(
            result.is_empty(),
            "glob with non-existent base dir should produce empty result"
        );
    }

    #[test]
    fn resolve_member_paths_mixed_existing_and_not() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("exists")).unwrap();
        let patterns = vec!["exists".to_string(), "does-not-exist".to_string()];
        let result = resolve_member_paths(dir.path(), &patterns, WorkspaceKind::Cargo);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "exists");
    }

    #[test]
    fn resolve_member_paths_with_dot_slash_prefix() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("mypackage")).unwrap();
        let patterns = vec!["./mypackage".to_string()];
        let result = resolve_member_paths(dir.path(), &patterns, WorkspaceKind::Npm);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "mypackage");
        assert_eq!(result[0].path, "mypackage");
    }

    // --- detect_nx with non-existent project dirs ---

    #[test]
    fn detect_nx_projects_point_to_nonexistent_dirs() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("workspace.json"),
            r#"{"projects": {"app": "apps/app", "lib": "libs/lib"}}"#,
        )
        .unwrap();
        // Do NOT create the actual directories

        let result = detect_nx(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "Nx with non-existent project dirs should return None"
        );
    }

    #[test]
    fn detect_nx_some_projects_exist_some_not() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("workspace.json"),
            r#"{"projects": {"app": "apps/app", "missing": "libs/missing"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("apps/app")).unwrap();
        // libs/missing is not created

        let result = detect_nx(dir.path()).unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        assert_eq!(ws.packages.len(), 1);
        assert_eq!(ws.packages[0].name, "app");
    }

    #[test]
    fn detect_nx_no_projects_key() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("workspace.json"), r#"{"version": 2}"#).unwrap();

        let result = detect_nx(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "workspace.json without projects key should return None"
        );
    }
}
