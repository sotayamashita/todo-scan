use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

use crate::config::Config;
use crate::git::git_command;
use crate::model::*;
use crate::scanner::scan_content;

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
    let diff_from_ref = git_command(&["diff", "--name-only", "--", base_ref], root);
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
    anyhow::ensure!(
        !base_ref.starts_with('-'),
        "invalid git ref '{}': must not start with '-'",
        base_ref
    );

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

        let result = scan_content(&content, path, &re);
        base_items.extend(result.items);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    // ---- Helper functions ----

    /// Create a temporary git repo, populate it with initial files, and commit.
    /// Returns the TempDir (which keeps the directory alive while in scope).
    fn setup_git_repo(initial_files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();

        Command::new("git")
            .args(["init"])
            .current_dir(cwd)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(cwd)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(cwd)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "commit.gpgsign", "false"])
            .current_dir(cwd)
            .output()
            .unwrap();

        for (path, content) in initial_files {
            let full_path = cwd.join(path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(full_path, content).unwrap();
        }

        Command::new("git")
            .args(["add", "."])
            .current_dir(cwd)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(cwd)
            .output()
            .unwrap();

        dir
    }

    /// Helper to build a TodoItem quickly for constructing ScanResults.
    fn make_item(file: &str, line: usize, tag: Tag, message: &str) -> TodoItem {
        TodoItem {
            file: file.to_string(),
            line,
            tag,
            message: message.to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: None,
        }
    }

    // ---- Existing test ----

    #[test]
    fn test_compute_diff_rejects_ref_starting_with_dash() {
        let current = ScanResult {
            items: vec![],
            files_scanned: 0,
            ignored_items: vec![],
        };
        let config = Config::default();
        let root = Path::new(".");
        let result = compute_diff(&current, "--output=/tmp/leak", root, &config);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("must not start with '-'"),
            "expected rejection of dash-prefixed ref, got: {err_msg}"
        );
    }

    // ---- Tests for compute_diff with real git repos ----

    #[test]
    fn test_compute_diff_added_todos() {
        // Start with a file that has no TODOs, then add TODOs in the working tree
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        // Add TODOs after commit
        std::fs::write(
            cwd.join("main.rs"),
            "// TODO: new feature\n// FIXME: urgent fix\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 2);
        assert_eq!(result.removed_count, 0);
        assert_eq!(result.base_ref, "HEAD");
        assert_eq!(result.entries.len(), 2);

        // All entries should be Added
        for entry in &result.entries {
            assert!(matches!(entry.status, DiffStatus::Added));
        }

        // Verify the specific messages are found
        let messages: Vec<&str> = result
            .entries
            .iter()
            .map(|e| e.item.message.as_str())
            .collect();
        assert!(messages.contains(&"new feature"));
        assert!(messages.contains(&"urgent fix"));
    }

    #[test]
    fn test_compute_diff_removed_todos() {
        // Start with TODOs in the committed file, then remove them
        let dir = setup_git_repo(&[(
            "main.rs",
            "// TODO: old task\n// FIXME: old fix\nfn main() {}\n",
        )]);
        let cwd = dir.path();

        // Remove all TODOs
        std::fs::write(cwd.join("main.rs"), "fn main() {}\n").unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 0);
        assert_eq!(result.removed_count, 2);
        assert_eq!(result.entries.len(), 2);

        for entry in &result.entries {
            assert!(matches!(entry.status, DiffStatus::Removed));
        }

        let messages: Vec<&str> = result
            .entries
            .iter()
            .map(|e| e.item.message.as_str())
            .collect();
        assert!(messages.contains(&"old task"));
        assert!(messages.contains(&"old fix"));
    }

    #[test]
    fn test_compute_diff_no_changes() {
        // No modifications to files after commit - diff should be empty
        let dir = setup_git_repo(&[("main.rs", "// TODO: existing task\nfn main() {}\n")]);
        let cwd = dir.path();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 0);
        assert_eq!(result.removed_count, 0);
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_compute_diff_empty_scan_results() {
        // Repo with no TODO items at all, and no changes
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        let current = ScanResult {
            items: vec![],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let config = Config::default();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 0);
        assert_eq!(result.removed_count, 0);
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_compute_diff_with_invalid_git_ref() {
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        let current = ScanResult {
            items: vec![],
            files_scanned: 0,
            ignored_items: vec![],
        };
        let config = Config::default();
        let result = compute_diff(&current, "nonexistent-ref-abc123", cwd, &config);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Failed to list files at ref nonexistent-ref-abc123"),
            "expected error about invalid ref, got: {err_msg}"
        );
    }

    #[test]
    fn test_compute_diff_mixed_added_and_removed() {
        // Start with some TODOs, then add new ones and remove old ones
        let dir = setup_git_repo(&[(
            "main.rs",
            "// TODO: will be removed\n// FIXME: stays\nfn main() {}\n",
        )]);
        let cwd = dir.path();

        // Replace "will be removed" with "newly added", keep "stays"
        std::fs::write(
            cwd.join("main.rs"),
            "// FIXME: stays\n// HACK: newly added\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 1);
        assert_eq!(result.removed_count, 1);
        assert_eq!(result.entries.len(), 2);

        let added: Vec<&DiffEntry> = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Added))
            .collect();
        let removed: Vec<&DiffEntry> = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Removed))
            .collect();

        assert_eq!(added.len(), 1);
        assert_eq!(added[0].item.message, "newly added");
        assert_eq!(added[0].item.tag, Tag::Hack);

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].item.message, "will be removed");
        assert_eq!(removed[0].item.tag, Tag::Todo);
    }

    #[test]
    fn test_compute_diff_new_untracked_file() {
        // Commit a file, then add a new untracked file with TODOs
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        // Create a brand new file not in the base commit
        std::fs::write(
            cwd.join("newfile.rs"),
            "// TODO: brand new task\n// BUG: found a bug\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 2);
        assert_eq!(result.removed_count, 0);

        let messages: Vec<&str> = result
            .entries
            .iter()
            .map(|e| e.item.message.as_str())
            .collect();
        assert!(messages.contains(&"brand new task"));
        assert!(messages.contains(&"found a bug"));
    }

    #[test]
    fn test_compute_diff_deleted_file() {
        // Commit a file with TODOs, then delete it
        let dir = setup_git_repo(&[("removeme.rs", "// TODO: this will be gone\nfn old() {}\n")]);
        let cwd = dir.path();

        std::fs::remove_file(cwd.join("removeme.rs")).unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 0);
        assert_eq!(result.removed_count, 1);
        assert_eq!(result.entries[0].item.message, "this will be gone");
        assert!(matches!(result.entries[0].status, DiffStatus::Removed));
    }

    #[test]
    fn test_compute_diff_multiple_files_only_one_changed() {
        // Commit multiple files, only change one - unchanged file TODOs should not appear
        let dir = setup_git_repo(&[
            ("a.rs", "// TODO: task in a\nfn a() {}\n"),
            ("b.rs", "// FIXME: task in b\nfn b() {}\n"),
        ]);
        let cwd = dir.path();

        // Only modify a.rs
        std::fs::write(
            cwd.join("a.rs"),
            "// TODO: task in a\n// HACK: new hack in a\nfn a() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 1);
        assert_eq!(result.removed_count, 0);
        assert_eq!(result.entries[0].item.message, "new hack in a");

        // b.rs should not appear in diff at all
        for entry in &result.entries {
            assert_ne!(
                entry.item.file, "b.rs",
                "unchanged file b.rs should not appear in diff"
            );
        }
    }

    #[test]
    fn test_compute_diff_preserves_base_ref_in_result() {
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.base_ref, "HEAD");
    }

    #[test]
    fn test_compute_diff_with_named_branch_ref() {
        // Create a branch, make changes, and diff against the branch
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        // Create a branch at current HEAD
        Command::new("git")
            .args(["branch", "baseline"])
            .current_dir(cwd)
            .output()
            .unwrap();

        // Add a TODO in working tree
        std::fs::write(cwd.join("main.rs"), "// TODO: after branch\nfn main() {}\n").unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "baseline", cwd, &config).unwrap();

        assert_eq!(result.added_count, 1);
        assert_eq!(result.base_ref, "baseline");
        assert_eq!(result.entries[0].item.message, "after branch");
    }

    #[test]
    fn test_compute_diff_todo_message_change_is_added_and_removed() {
        // Changing a TODO message means the old one is "removed" and new one is "added"
        // because match_key includes the message
        let dir = setup_git_repo(&[("main.rs", "// TODO: original message\nfn main() {}\n")]);
        let cwd = dir.path();

        std::fs::write(
            cwd.join("main.rs"),
            "// TODO: updated message\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 1);
        assert_eq!(result.removed_count, 1);

        let added: Vec<&DiffEntry> = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Added))
            .collect();
        let removed: Vec<&DiffEntry> = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Removed))
            .collect();

        assert_eq!(added[0].item.message, "updated message");
        assert_eq!(removed[0].item.message, "original message");
    }

    #[test]
    fn test_compute_diff_tag_change_is_added_and_removed() {
        // Changing a tag (e.g., TODO -> FIXME) with same message is add+remove
        // because match_key includes the tag
        let dir = setup_git_repo(&[("main.rs", "// TODO: fix something\nfn main() {}\n")]);
        let cwd = dir.path();

        std::fs::write(
            cwd.join("main.rs"),
            "// FIXME: fix something\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 1);
        assert_eq!(result.removed_count, 1);

        let added: Vec<&DiffEntry> = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Added))
            .collect();
        let removed: Vec<&DiffEntry> = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Removed))
            .collect();

        assert_eq!(added[0].item.tag, Tag::Fixme);
        assert_eq!(removed[0].item.tag, Tag::Todo);
    }

    #[test]
    fn test_compute_diff_line_number_change_only_is_not_a_diff() {
        // Moving a TODO to a different line but keeping same content should NOT
        // cause a diff, because match_key() excludes line numbers
        let dir = setup_git_repo(&[("main.rs", "// TODO: stable task\nfn main() {}\n")]);
        let cwd = dir.path();

        // Add blank lines above to shift the TODO down
        std::fs::write(
            cwd.join("main.rs"),
            "\n\n\n// TODO: stable task\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 0);
        assert_eq!(result.removed_count, 0);
        assert!(
            result.entries.is_empty(),
            "line number change only should not produce diff entries"
        );
    }

    #[test]
    fn test_compute_diff_with_author_and_priority() {
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        std::fs::write(
            cwd.join("main.rs"),
            "// TODO(alice): ! high priority task\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 1);
        let item = &result.entries[0].item;
        assert_eq!(item.author.as_deref(), Some("alice"));
        assert_eq!(item.priority, Priority::High);
    }

    #[test]
    fn test_compute_diff_with_subdirectory_files() {
        let dir = setup_git_repo(&[("src/lib.rs", "// TODO: lib task\nfn lib() {}\n")]);
        let cwd = dir.path();

        // Add a new TODO in the subdirectory file
        std::fs::write(
            cwd.join("src/lib.rs"),
            "// TODO: lib task\n// HACK: new hack\nfn lib() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 1);
        assert_eq!(result.entries[0].item.file, "src/lib.rs");
        assert_eq!(result.entries[0].item.message, "new hack");
    }

    #[test]
    fn test_compute_diff_all_tags() {
        // Test that all six tag types work correctly in diffs
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        std::fs::write(
            cwd.join("main.rs"),
            "// TODO: todo item\n// FIXME: fixme item\n// HACK: hack item\n// XXX: xxx item\n// BUG: bug item\n// NOTE: note item\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 6);
        let tags: Vec<Tag> = result.entries.iter().map(|e| e.item.tag).collect();
        assert!(tags.contains(&Tag::Todo));
        assert!(tags.contains(&Tag::Fixme));
        assert!(tags.contains(&Tag::Hack));
        assert!(tags.contains(&Tag::Xxx));
        assert!(tags.contains(&Tag::Bug));
        assert!(tags.contains(&Tag::Note));
    }

    // ---- Tests for detect_changed_files ----

    #[test]
    fn test_detect_changed_files_with_modified_file() {
        let dir = setup_git_repo(&[("a.rs", "fn a() {}\n"), ("b.rs", "fn b() {}\n")]);
        let cwd = dir.path();

        // Modify only a.rs
        std::fs::write(cwd.join("a.rs"), "// changed\nfn a() {}\n").unwrap();

        let base_files: HashSet<String> = ["a.rs".to_string(), "b.rs".to_string()]
            .into_iter()
            .collect();

        let current = ScanResult {
            items: vec![],
            files_scanned: 2,
            ignored_items: vec![],
        };

        let changed = detect_changed_files("HEAD", cwd, &base_files, &current);

        assert!(
            changed.contains("a.rs"),
            "modified file a.rs should be in changed set"
        );
        // b.rs is unchanged, so it should NOT be in changed
        assert!(
            !changed.contains("b.rs"),
            "unchanged file b.rs should not be in changed set"
        );
    }

    #[test]
    fn test_detect_changed_files_new_file_not_in_base() {
        // A file that's in the current scan but not in base_files
        // should be added to changed_files
        let dir = setup_git_repo(&[("a.rs", "fn a() {}\n")]);
        let cwd = dir.path();

        // Create a new file not in base
        std::fs::write(cwd.join("newfile.rs"), "// TODO: new\n").unwrap();

        let base_files: HashSet<String> = ["a.rs".to_string()].into_iter().collect();

        let current = ScanResult {
            items: vec![make_item("newfile.rs", 1, Tag::Todo, "new")],
            files_scanned: 2,
            ignored_items: vec![],
        };

        let changed = detect_changed_files("HEAD", cwd, &base_files, &current);

        assert!(
            changed.contains("newfile.rs"),
            "new file should be in changed set"
        );
    }

    #[test]
    fn test_detect_changed_files_fallback_on_invalid_ref() {
        // When git diff commands fail, detect_changed_files should fall back
        // to returning all files (base_files + current item files)
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();

        // No git repo initialized - git commands will fail
        let base_files: HashSet<String> = ["base.rs".to_string()].into_iter().collect();

        let current = ScanResult {
            items: vec![make_item("current.rs", 1, Tag::Todo, "task")],
            files_scanned: 1,
            ignored_items: vec![],
        };

        let changed = detect_changed_files("HEAD", cwd, &base_files, &current);

        // Fallback: should include both base_files and current item files
        assert!(
            changed.contains("base.rs"),
            "fallback should include base files"
        );
        assert!(
            changed.contains("current.rs"),
            "fallback should include current item files"
        );
    }

    #[test]
    fn test_detect_changed_files_fallback_includes_all_base_and_current() {
        // Verify the fallback path returns the union of base_files and current item files
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();

        let base_files: HashSet<String> = ["base1.rs".to_string(), "base2.rs".to_string()]
            .into_iter()
            .collect();

        let current = ScanResult {
            items: vec![
                make_item("current1.rs", 1, Tag::Todo, "t1"),
                make_item("current2.rs", 2, Tag::Fixme, "t2"),
                make_item("base1.rs", 3, Tag::Hack, "t3"), // overlaps with base
            ],
            files_scanned: 3,
            ignored_items: vec![],
        };

        let changed = detect_changed_files("HEAD", cwd, &base_files, &current);

        assert!(changed.contains("base1.rs"));
        assert!(changed.contains("base2.rs"));
        assert!(changed.contains("current1.rs"));
        assert!(changed.contains("current2.rs"));
        assert_eq!(changed.len(), 4);
    }

    #[test]
    fn test_detect_changed_files_empty_inputs() {
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        let base_files: HashSet<String> = HashSet::new();
        let current = ScanResult {
            items: vec![],
            files_scanned: 0,
            ignored_items: vec![],
        };

        let changed = detect_changed_files("HEAD", cwd, &base_files, &current);

        // No files changed, no new files
        assert!(changed.is_empty());
    }

    #[test]
    fn test_detect_changed_files_deleted_file() {
        let dir = setup_git_repo(&[("a.rs", "fn a() {}\n"), ("b.rs", "fn b() {}\n")]);
        let cwd = dir.path();

        // Delete b.rs
        std::fs::remove_file(cwd.join("b.rs")).unwrap();

        let base_files: HashSet<String> = ["a.rs".to_string(), "b.rs".to_string()]
            .into_iter()
            .collect();

        let current = ScanResult {
            items: vec![],
            files_scanned: 1,
            ignored_items: vec![],
        };

        let changed = detect_changed_files("HEAD", cwd, &base_files, &current);

        assert!(
            changed.contains("b.rs"),
            "deleted file b.rs should appear in changed set"
        );
    }

    // ---- compute_diff edge cases ----

    #[test]
    fn test_compute_diff_not_a_git_repo() {
        // Running compute_diff in a non-git directory should error
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();

        std::fs::write(cwd.join("main.rs"), "// TODO: task\n").unwrap();

        let current = ScanResult {
            items: vec![make_item("main.rs", 1, Tag::Todo, "task")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let config = Config::default();

        let result = compute_diff(&current, "HEAD", cwd, &config);
        assert!(result.is_err(), "should fail outside a git repo");
    }

    #[test]
    fn test_compute_diff_multiple_todos_same_file() {
        let dir = setup_git_repo(&[(
            "main.rs",
            "// TODO: first\n// TODO: second\n// FIXME: third\nfn main() {}\n",
        )]);
        let cwd = dir.path();

        // Remove second, add fourth
        std::fs::write(
            cwd.join("main.rs"),
            "// TODO: first\n// FIXME: third\n// HACK: fourth\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 1);
        assert_eq!(result.removed_count, 1);

        let added: Vec<&DiffEntry> = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Added))
            .collect();
        let removed: Vec<&DiffEntry> = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Removed))
            .collect();

        assert_eq!(added[0].item.message, "fourth");
        assert_eq!(removed[0].item.message, "second");
    }

    #[test]
    fn test_compute_diff_empty_repo_with_empty_current() {
        // Repo with an empty file, current scan is empty
        let dir = setup_git_repo(&[("main.rs", "\n")]);
        let cwd = dir.path();

        let current = ScanResult {
            items: vec![],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let config = Config::default();

        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();
        assert_eq!(result.added_count, 0);
        assert_eq!(result.removed_count, 0);
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_compute_diff_binary_file_in_base_is_skipped() {
        // If a file in the base ref is binary or unreadable via git show,
        // it should be silently skipped (not cause an error)
        let dir = setup_git_repo(&[("data.bin", "binary\x00content\n")]);
        let cwd = dir.path();

        // The file has null bytes but git may or may not consider it binary.
        // What matters is that compute_diff doesn't crash.
        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config);

        // Should succeed regardless
        assert!(result.is_ok());
    }

    #[test]
    fn test_compute_diff_with_new_file_in_subdirectory() {
        let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
        let cwd = dir.path();

        // Create a new file in a subdirectory
        std::fs::create_dir_all(cwd.join("src/utils")).unwrap();
        std::fs::write(
            cwd.join("src/utils/helper.rs"),
            "// TODO: implement helper\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        assert_eq!(result.added_count, 1);
        assert_eq!(result.entries[0].item.file, "src/utils/helper.rs");
        assert_eq!(result.entries[0].item.message, "implement helper");
    }

    #[test]
    fn test_compute_diff_diff_against_earlier_commit() {
        // Make two commits, then diff against the first commit
        let dir = setup_git_repo(&[("main.rs", "// TODO: first commit task\nfn main() {}\n")]);
        let cwd = dir.path();

        // Get the first commit hash
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(cwd)
            .output()
            .unwrap();
        let first_commit = String::from_utf8(output.stdout).unwrap().trim().to_string();

        // Make a second commit with more TODOs
        std::fs::write(
            cwd.join("main.rs"),
            "// TODO: first commit task\n// FIXME: second commit fix\nfn main() {}\n",
        )
        .unwrap();

        Command::new("git")
            .args(["add", "."])
            .current_dir(cwd)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "second"])
            .current_dir(cwd)
            .output()
            .unwrap();

        // Now add more in working tree
        std::fs::write(
            cwd.join("main.rs"),
            "// TODO: first commit task\n// FIXME: second commit fix\n// HACK: working tree hack\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();

        // Diff against the first commit - should show both second commit and working tree additions
        let result = compute_diff(&current, &first_commit, cwd, &config).unwrap();

        assert_eq!(result.added_count, 2);
        let messages: Vec<&str> = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Added))
            .map(|e| e.item.message.as_str())
            .collect();
        assert!(messages.contains(&"second commit fix"));
        assert!(messages.contains(&"working tree hack"));
    }

    #[test]
    fn test_compute_diff_counts_match_entries() {
        // Verify that added_count and removed_count match the actual entry counts
        let dir = setup_git_repo(&[(
            "main.rs",
            "// TODO: a\n// TODO: b\n// FIXME: c\nfn main() {}\n",
        )]);
        let cwd = dir.path();

        std::fs::write(
            cwd.join("main.rs"),
            "// TODO: a\n// HACK: d\n// BUG: e\nfn main() {}\n",
        )
        .unwrap();

        let config = Config::default();
        let current = crate::scanner::scan_directory(cwd, &config).unwrap();
        let result = compute_diff(&current, "HEAD", cwd, &config).unwrap();

        let actual_added = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Added))
            .count();
        let actual_removed = result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Removed))
            .count();

        assert_eq!(result.added_count, actual_added);
        assert_eq!(result.removed_count, actual_removed);
        assert_eq!(result.entries.len(), actual_added + actual_removed);
    }
}
