use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use crate::config::Config;
use crate::model::*;
use crate::scanner::scan_content;

fn git_command(args: &[&str], cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("Failed to execute git {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }

    let stdout = String::from_utf8(output.stdout)
        .with_context(|| "git output is not valid UTF-8")?;

    Ok(stdout)
}

pub fn compute_diff(
    current: &ScanResult,
    base_ref: &str,
    root: &Path,
    config: &Config,
) -> Result<DiffResult> {
    let file_list = git_command(&["ls-tree", "-r", "--name-only", "--", base_ref], root)
        .with_context(|| format!("Failed to list files at ref {}", base_ref))?;

    let pattern = config.tags_pattern();
    let re = Regex::new(&pattern)
        .with_context(|| format!("Invalid tags pattern: {}", pattern))?;

    let mut base_items: Vec<TodoItem> = Vec::new();
    for path in file_list.lines() {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }

        let content = match git_command(&["show", &format!("{}:{}", base_ref, path)], root) {
            Ok(c) => c,
            Err(_) => continue, // skip binary or inaccessible files
        };

        let items = scan_content(&content, path, &re);
        base_items.extend(items);
    }

    let current_keys: HashSet<String> = current.items.iter().map(|i| i.match_key()).collect();
    let base_keys: HashSet<String> = base_items.iter().map(|i| i.match_key()).collect();

    let mut entries: Vec<DiffEntry> = Vec::new();

    // Added = in current but not in base
    for item in &current.items {
        if !base_keys.contains(&item.match_key()) {
            entries.push(DiffEntry {
                status: DiffStatus::Added,
                item: item.clone(),
            });
        }
    }

    // Removed = in base but not in current
    for item in &base_items {
        if !current_keys.contains(&item.match_key()) {
            entries.push(DiffEntry {
                status: DiffStatus::Removed,
                item: item.clone(),
            });
        }
    }

    let added_count = entries
        .iter()
        .filter(|e| matches!(e.status, DiffStatus::Added))
        .count();
    let removed_count = entries
        .iter()
        .filter(|e| matches!(e.status, DiffStatus::Removed))
        .count();

    Ok(DiffResult {
        entries,
        added_count,
        removed_count,
        base_ref: base_ref.to_string(),
    })
}
