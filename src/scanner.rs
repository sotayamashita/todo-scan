use anyhow::Result;
use ignore::WalkBuilder;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

use crate::config::Config;
use crate::model::{Priority, ScanResult, Tag, TodoItem};

static ISSUE_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:([A-Z]+-\d+)|#(\d+))").unwrap());

/// Extract an issue reference from the message text.
/// Matches patterns like #123 or JIRA-456.
fn extract_issue_ref(message: &str) -> Option<String> {
    ISSUE_REF_RE.captures(message).map(|caps| {
        caps.get(1)
            .or_else(|| caps.get(2).map(|m| m))
            .map(|m| {
                if caps.get(1).is_some() {
                    m.as_str().to_string()
                } else {
                    format!("#{}", m.as_str())
                }
            })
            .unwrap()
    })
}

/// Scan text content line by line for TODO-style comments.
///
/// Pure function: takes content, a file path label, and a compiled regex.
/// Returns a `Vec<TodoItem>` with all matches found.
pub fn scan_content(content: &str, file_path: &str, pattern: &Regex) -> Vec<TodoItem> {
    let mut items = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        if let Some(caps) = pattern.captures(line) {
            let tag_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let tag = match tag_str.parse::<Tag>() {
                Ok(t) => t,
                Err(_) => continue,
            };

            let author = caps.get(2).map(|m| m.as_str().to_string());

            let priority = match caps.get(3).map(|m| m.as_str()) {
                Some("!!") => Priority::Urgent,
                Some("!") => Priority::High,
                _ => Priority::Normal,
            };

            let message = caps
                .get(4)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            let issue_ref = extract_issue_ref(&message);

            items.push(TodoItem {
                file: file_path.to_string(),
                line: line_idx + 1,
                tag,
                message,
                author,
                issue_ref,
                priority,
            });
        }
    }

    items
}

/// Walk a directory tree and scan all files for TODO-style comments.
///
/// Respects `.gitignore` via `ignore::WalkBuilder`. Applies the exclude
/// directories and exclude patterns from `Config`. Returns a `ScanResult`
/// with every matched item and the total number of files scanned.
pub fn scan_directory(root: &Path, config: &Config) -> Result<ScanResult> {
    let pattern_str = config.tags_pattern();
    let pattern = Regex::new(&pattern_str)?;

    let exclude_regexes: Vec<Regex> = config
        .exclude_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    let mut items = Vec::new();
    let mut files_scanned: usize = 0;

    let walker = WalkBuilder::new(root).build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        // Check exclude_dirs
        let should_exclude_dir = config.exclude_dirs.iter().any(|dir| {
            path.components()
                .any(|c| c.as_os_str().to_str().map(|s| s == dir).unwrap_or(false))
        });
        if should_exclude_dir {
            continue;
        }

        // Check exclude_patterns against the path string
        let path_str = path.to_string_lossy();
        let should_exclude_pattern = exclude_regexes.iter().any(|re| re.is_match(&path_str));
        if should_exclude_pattern {
            continue;
        }

        // Read the file; skip binary or unreadable files
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let found = scan_content(&content, &relative_path, &pattern);
        items.extend(found);
        files_scanned += 1;
    }

    Ok(ScanResult {
        items,
        files_scanned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_pattern() -> Regex {
        let config = Config::default();
        Regex::new(&config.tags_pattern()).unwrap()
    }

    #[test]
    fn test_basic_todo_detection() {
        let pattern = default_pattern();
        let content = "// TODO: implement this feature\n";
        let items = scan_content(content, "test.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tag, Tag::Todo);
        assert_eq!(items[0].message, "implement this feature");
        assert_eq!(items[0].file, "test.rs");
        assert_eq!(items[0].line, 1);
        assert_eq!(items[0].priority, Priority::Normal);
        assert!(items[0].author.is_none());
    }

    #[test]
    fn test_fixme_with_author() {
        let pattern = default_pattern();
        let content = "// FIXME(alice): broken parsing logic\n";
        let items = scan_content(content, "lib.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tag, Tag::Fixme);
        assert_eq!(items[0].author.as_deref(), Some("alice"));
        assert_eq!(items[0].message, "broken parsing logic");
    }

    #[test]
    fn test_priority_high() {
        let pattern = default_pattern();
        let content = "# TODO: ! fix memory leak\n";
        let items = scan_content(content, "main.py", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].priority, Priority::High);
    }

    #[test]
    fn test_priority_urgent() {
        let pattern = default_pattern();
        let content = "// BUG: !! crashes on empty input\n";
        let items = scan_content(content, "app.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tag, Tag::Bug);
        assert_eq!(items[0].priority, Priority::Urgent);
    }

    #[test]
    fn test_issue_ref_hash() {
        let pattern = default_pattern();
        let content = "// TODO: fix layout issue #123\n";
        let items = scan_content(content, "ui.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].issue_ref.as_deref(), Some("#123"));
    }

    #[test]
    fn test_issue_ref_jira() {
        let pattern = default_pattern();
        let content = "// FIXME: address JIRA-456 regression\n";
        let items = scan_content(content, "api.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].issue_ref.as_deref(), Some("JIRA-456"));
    }

    #[test]
    fn test_case_insensitivity() {
        let pattern = default_pattern();
        let content = "// todo: lowercase tag\n// Todo: mixed case\n// TODO: uppercase\n";
        let items = scan_content(content, "test.rs", &pattern);

        assert_eq!(items.len(), 3);
        for item in &items {
            assert_eq!(item.tag, Tag::Todo);
        }
    }

    #[test]
    fn test_multiple_tags_in_content() {
        let pattern = default_pattern();
        let content = "\
// TODO: first task
fn foo() {}
// FIXME(bob): second task
// HACK: workaround for upstream bug
// NOTE: remember to update docs
";
        let items = scan_content(content, "multi.rs", &pattern);

        assert_eq!(items.len(), 4);
        assert_eq!(items[0].tag, Tag::Todo);
        assert_eq!(items[1].tag, Tag::Fixme);
        assert_eq!(items[1].author.as_deref(), Some("bob"));
        assert_eq!(items[2].tag, Tag::Hack);
        assert_eq!(items[3].tag, Tag::Note);
    }

    #[test]
    fn test_line_numbers_are_correct() {
        let pattern = default_pattern();
        let content = "\
line one
// TODO: on line two
line three
line four
// FIXME: on line five
";
        let items = scan_content(content, "lines.rs", &pattern);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].line, 2);
        assert_eq!(items[1].line, 5);
    }

    #[test]
    fn test_xxx_tag() {
        let pattern = default_pattern();
        let content = "// XXX: dangerous code path\n";
        let items = scan_content(content, "danger.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tag, Tag::Xxx);
    }

    #[test]
    fn test_no_match_on_plain_text() {
        let pattern = default_pattern();
        let content = "This is just a regular comment with no tags.\n";
        let items = scan_content(content, "plain.rs", &pattern);

        assert!(items.is_empty());
    }

    #[test]
    fn test_author_with_special_chars() {
        let pattern = default_pattern();
        let content = "// TODO(user@domain.com): email-style author\n";
        let items = scan_content(content, "test.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].author.as_deref(), Some("user@domain.com"));
    }

    #[test]
    fn test_extract_issue_ref_function() {
        assert_eq!(extract_issue_ref("fix #42"), Some("#42".to_string()));
        assert_eq!(
            extract_issue_ref("see PROJ-100"),
            Some("PROJ-100".to_string())
        );
        assert_eq!(extract_issue_ref("no reference here"), None);
    }
}
