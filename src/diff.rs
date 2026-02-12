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

    let stdout =
        String::from_utf8(output.stdout).with_context(|| "git output is not valid UTF-8")?;

    Ok(stdout)
}

/// Detect which files changed between `base_ref` and the current working tree.
///
/// Uses `git diff --name-only` to find files that differ. Falls back to treating
/// all files as changed if the git diff commands fail (e.g., shallow clone).
fn detect_changed_files(
    base_ref: &str,
    root: &Path,
    base_files: &HashSet<String>,
    current: &ScanResult,
) -> HashSet<String> {
    let diff_from_ref = git_command(&["diff", "--name-only", base_ref], root);
    let diff_unstaged = git_command(&["diff", "--name-only"], root);

    // If either diff command failed, fall back to all files
    let (diff_ref_output, diff_unstaged_output) = match (diff_from_ref, diff_unstaged) {
        (Ok(a), Ok(b)) => (a, b),
        _ => {
            let mut all: HashSet<String> = base_files.clone();
            all.extend(current.items.iter().map(|i| i.file.clone()));
            return all;
        }
    };

    let mut changed_files: HashSet<String> = HashSet::new();

    // Files changed between base_ref and index + between index and working tree
    for line in diff_ref_output.lines().chain(diff_unstaged_output.lines()) {
        let path = line.trim();
        if !path.is_empty() {
            changed_files.insert(path.to_string());
        }
    }

    // Add new untracked files (in current scan but not in base)
    for item in &current.items {
        if !base_files.contains(&item.file) {
            changed_files.insert(item.file.clone());
        }
    }

    changed_files
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
    let re = Regex::new(&pattern).with_context(|| format!("Invalid tags pattern: {}", pattern))?;

    let base_files: HashSet<String> = file_list
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let changed_files = detect_changed_files(base_ref, root, &base_files, current);

    // Only scan changed files from base ref (instead of all files)
    let mut base_items: Vec<TodoItem> = Vec::new();
    for path in &changed_files {
        if !base_files.contains(path) {
            continue; // new file, not in base
        }

        let content = match git_command(&["show", &format!("{}:{}", base_ref, path)], root) {
            Ok(c) => c,
            Err(_) => continue, // skip binary or inaccessible files
        };

        let items = scan_content(&content, path, &re);
        base_items.extend(items);
    }

    // Only compare current items from changed files
    let current_changed: Vec<&TodoItem> = current
        .items
        .iter()
        .filter(|i| changed_files.contains(&i.file))
        .collect();

    let current_keys: HashSet<String> = current_changed.iter().map(|i| i.match_key()).collect();
    let base_keys: HashSet<String> = base_items.iter().map(|i| i.match_key()).collect();

    let mut entries: Vec<DiffEntry> = Vec::new();

    // Added = in current but not in base
    for item in &current_changed {
        if !base_keys.contains(&item.match_key()) {
            entries.push(DiffEntry {
                status: DiffStatus::Added,
                item: (*item).clone(),
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
