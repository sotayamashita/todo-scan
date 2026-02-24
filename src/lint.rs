use std::collections::HashMap;
use std::path::Path;

use regex::Regex;

use crate::config::Config;
use crate::model::{LintResult, LintViolation, ScanResult, TodoItem};
use crate::scanner;

pub struct LintOverrides {
    pub no_bare_tags: bool,
    pub max_message_length: Option<usize>,
    pub require_author: Vec<String>,
    pub require_issue_ref: Vec<String>,
    pub uppercase_tag: bool,
    pub require_colon: bool,
}

struct ResolvedLint {
    no_bare_tags: bool,
    max_message_length: Option<usize>,
    require_author: Vec<String>,
    require_issue_ref: Vec<String>,
    uppercase_tag: bool,
    require_colon: bool,
}

fn resolve_config(config: &Config, overrides: &LintOverrides) -> ResolvedLint {
    ResolvedLint {
        no_bare_tags: overrides.no_bare_tags || config.lint.no_bare_tags.unwrap_or(true),
        max_message_length: overrides
            .max_message_length
            .or(config.lint.max_message_length),
        require_author: if !overrides.require_author.is_empty() {
            overrides.require_author.clone()
        } else {
            config.lint.require_author.clone().unwrap_or_default()
        },
        require_issue_ref: if !overrides.require_issue_ref.is_empty() {
            overrides.require_issue_ref.clone()
        } else {
            config.lint.require_issue_ref.clone().unwrap_or_default()
        },
        uppercase_tag: overrides.uppercase_tag || config.lint.uppercase_tag.unwrap_or(true),
        require_colon: overrides.require_colon || config.lint.require_colon.unwrap_or(true),
    }
}

pub fn run_lint(
    scan: &ScanResult,
    config: &Config,
    overrides: &LintOverrides,
    root: &Path,
) -> LintResult {
    let resolved = resolve_config(config, overrides);
    let mut violations = Vec::new();

    // Phase 1: Metadata-based rules
    for item in &scan.items {
        check_metadata_rules(item, &resolved, &mut violations);
    }

    // Phase 2: Raw-text rules (uppercase_tag, require_colon)
    if resolved.uppercase_tag || resolved.require_colon {
        check_raw_text_rules(scan, config, root, &resolved, &mut violations);
    }

    // Sort by file, then line
    violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

    let violation_count = violations.len();
    LintResult {
        passed: violations.is_empty(),
        total_items: scan.items.len(),
        violation_count,
        violations,
    }
}

fn check_metadata_rules(
    item: &TodoItem,
    resolved: &ResolvedLint,
    violations: &mut Vec<LintViolation>,
) {
    // no_bare_tags
    if resolved.no_bare_tags && item.message.trim().is_empty() {
        violations.push(LintViolation {
            rule: "no_bare_tags".to_string(),
            message: format!("Empty {} message", item.tag),
            file: item.file.clone(),
            line: item.line,
            suggestion: Some(format!("{}: <description>", item.tag)),
        });
    }

    // max_message_length
    if let Some(max) = resolved.max_message_length {
        if item.message.len() > max {
            violations.push(LintViolation {
                rule: "max_message_length".to_string(),
                message: format!(
                    "Message length ({}) exceeds maximum ({})",
                    item.message.len(),
                    max
                ),
                file: item.file.clone(),
                line: item.line,
                suggestion: Some("Shorten the message or raise max_message_length".to_string()),
            });
        }
    }

    // require_author
    if !resolved.require_author.is_empty() {
        let tag_str = item.tag.as_str();
        if resolved
            .require_author
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tag_str))
            && item.author.is_none()
        {
            violations.push(LintViolation {
                rule: "require_author".to_string(),
                message: format!("Missing author for {} comment", item.tag),
                file: item.file.clone(),
                line: item.line,
                suggestion: Some(format!("{}(author): <message>", item.tag)),
            });
        }
    }

    // require_issue_ref
    if !resolved.require_issue_ref.is_empty() {
        let tag_str = item.tag.as_str();
        if resolved
            .require_issue_ref
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tag_str))
            && item.issue_ref.is_none()
        {
            violations.push(LintViolation {
                rule: "require_issue_ref".to_string(),
                message: format!("Missing issue reference for {} comment", item.tag),
                file: item.file.clone(),
                line: item.line,
                suggestion: Some("Add an issue reference (#123 or JIRA-456)".to_string()),
            });
        }
    }
}

fn check_raw_text_rules(
    scan: &ScanResult,
    config: &Config,
    root: &Path,
    resolved: &ResolvedLint,
    violations: &mut Vec<LintViolation>,
) {
    // Group items by file
    let mut file_items: HashMap<&str, Vec<&TodoItem>> = HashMap::new();
    for item in &scan.items {
        file_items.entry(item.file.as_str()).or_default().push(item);
    }

    // Build regex for raw-text analysis
    let tags = config.tags.join("|");
    let raw_re = Regex::new(&format!(r"(?i)\b({})(?:\([^)]*\))?(:)?", tags))
        .expect("invalid raw lint regex");

    for (file_path, items) in &file_items {
        let full_path = root.join(file_path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();

        for item in items {
            let line_idx = item.line.saturating_sub(1);
            if line_idx >= lines.len() {
                continue;
            }
            let line = lines[line_idx];

            // Find the tag occurrence that is inside a comment
            for caps in raw_re.captures_iter(line) {
                let tag_match = caps.get(1).unwrap();
                if !scanner::is_in_comment(line, tag_match.start()) {
                    continue;
                }

                // uppercase_tag
                if resolved.uppercase_tag {
                    let raw_tag = tag_match.as_str();
                    let expected = item.tag.as_str();
                    if raw_tag != expected {
                        violations.push(LintViolation {
                            rule: "uppercase_tag".to_string(),
                            message: format!(
                                "Tag '{}' should be uppercase '{}'",
                                raw_tag, expected
                            ),
                            file: item.file.clone(),
                            line: item.line,
                            suggestion: Some(format!("Change '{}' to '{}'", raw_tag, expected)),
                        });
                    }
                }

                // require_colon
                if resolved.require_colon && caps.get(2).is_none() {
                    violations.push(LintViolation {
                        rule: "require_colon".to_string(),
                        message: format!("Missing colon after {} tag", item.tag),
                        file: item.file.clone(),
                        line: item.line,
                        suggestion: Some(format!("{}: <message>", item.tag)),
                    });
                }

                break; // Only check the first in-comment match
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Tag;
    use crate::test_helpers::helpers::make_item;

    fn default_overrides() -> LintOverrides {
        LintOverrides {
            no_bare_tags: false,
            max_message_length: None,
            require_author: vec![],
            require_issue_ref: vec![],
            uppercase_tag: false,
            require_colon: false,
        }
    }

    #[test]
    fn test_no_bare_tags_detects_empty_message() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let config = Config::default();
        let overrides = default_overrides();
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule, "no_bare_tags");
    }

    #[test]
    fn test_no_bare_tags_allows_non_empty_message() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "real message")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = default_overrides();
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(result.passed);
    }

    #[test]
    fn test_max_message_length() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "this is a long message")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            max_message_length: Some(10),
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations[0].rule, "max_message_length");
    }

    #[test]
    fn test_require_author_missing() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "no author")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            require_author: vec!["TODO".to_string()],
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations[0].rule, "require_author");
    }

    #[test]
    fn test_require_author_present() {
        let mut item = make_item("a.rs", 1, Tag::Todo, "has author");
        item.author = Some("alice".to_string());
        let scan = ScanResult {
            items: vec![item],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            require_author: vec!["TODO".to_string()],
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(result.passed);
    }

    #[test]
    fn test_require_issue_ref_missing() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Bug, "no issue ref")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            require_issue_ref: vec!["BUG".to_string()],
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations[0].rule, "require_issue_ref");
    }

    #[test]
    fn test_require_issue_ref_present() {
        let mut item = make_item("a.rs", 1, Tag::Bug, "fix #123");
        item.issue_ref = Some("#123".to_string());
        let scan = ScanResult {
            items: vec![item],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            require_issue_ref: vec!["BUG".to_string()],
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(result.passed);
    }

    #[test]
    fn test_require_author_ignores_unmatched_tags() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Note, "just a note")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            require_author: vec!["TODO".to_string()],
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(result.passed);
    }

    #[test]
    fn test_uppercase_tag_with_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "// todo: lowercase tag\n").unwrap();

        let scan = ScanResult {
            items: vec![make_item("test.rs", 1, Tag::Todo, "lowercase tag")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            uppercase_tag: true,
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, dir.path());
        assert!(!result.passed);
        assert!(result.violations.iter().any(|v| v.rule == "uppercase_tag"));
    }

    #[test]
    fn test_uppercase_tag_passes_for_uppercase() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "// TODO: uppercase tag\n").unwrap();

        let scan = ScanResult {
            items: vec![make_item("test.rs", 1, Tag::Todo, "uppercase tag")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            uppercase_tag: true,
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, dir.path());
        assert!(result.passed);
    }

    #[test]
    fn test_require_colon_missing() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "// TODO fix without colon\n").unwrap();

        let scan = ScanResult {
            items: vec![make_item("test.rs", 1, Tag::Todo, "fix without colon")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        let overrides = LintOverrides {
            require_colon: true,
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, dir.path());
        assert!(!result.passed);
        assert!(result.violations.iter().any(|v| v.rule == "require_colon"));
    }

    #[test]
    fn test_require_colon_present() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "// TODO: fix with colon\n").unwrap();

        let scan = ScanResult {
            items: vec![make_item("test.rs", 1, Tag::Todo, "fix with colon")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        let overrides = LintOverrides {
            require_colon: true,
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, dir.path());
        assert!(result.passed);
    }

    #[test]
    fn test_config_overrides_defaults() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "valid message")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        // Disable all default-true rules via config
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = default_overrides();
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(result.passed);
    }

    #[test]
    fn test_cli_overrides_config() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false); // Config disables
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            no_bare_tags: true, // CLI enables
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations[0].rule, "no_bare_tags");
    }

    #[test]
    fn test_multiple_violations() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Todo, ""),
                make_item("a.rs", 2, Tag::Bug, "no issue ref"),
            ],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            require_issue_ref: vec!["BUG".to_string()],
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violation_count, 2);
    }

    #[test]
    fn test_config_require_author_used_when_override_empty() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Fixme, "missing author")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        config.lint.require_author = Some(vec!["FIXME".to_string()]);
        let overrides = default_overrides();
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations[0].rule, "require_author");
    }

    #[test]
    fn test_config_require_issue_ref_used_when_override_empty() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "no ref")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        config.lint.require_issue_ref = Some(vec!["TODO".to_string()]);
        let overrides = default_overrides();
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations[0].rule, "require_issue_ref");
    }

    #[test]
    fn test_config_max_message_length_from_config() {
        let scan = ScanResult {
            items: vec![make_item(
                "a.rs",
                1,
                Tag::Todo,
                "a fairly long message here",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        config.lint.max_message_length = Some(5);
        let overrides = default_overrides();
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations[0].rule, "max_message_length");
    }

    #[test]
    fn test_max_message_length_at_boundary() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "12345")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            max_message_length: Some(5), // exactly equal
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(result.passed); // len == max, not > max
    }

    #[test]
    fn test_require_author_case_insensitive_match() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "no author")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        // Use lowercase "todo" in config
        let overrides = LintOverrides {
            require_author: vec!["todo".to_string()],
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations[0].rule, "require_author");
    }

    #[test]
    fn test_no_bare_tags_whitespace_only_message() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "   ")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            no_bare_tags: true,
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(!result.passed);
        assert_eq!(result.violations[0].rule, "no_bare_tags");
    }

    #[test]
    fn test_violations_sorted_by_file_then_line() {
        let scan = ScanResult {
            items: vec![
                make_item("b.rs", 5, Tag::Todo, ""),
                make_item("a.rs", 10, Tag::Bug, ""),
                make_item("a.rs", 2, Tag::Fixme, ""),
            ],
            files_scanned: 2,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            no_bare_tags: true,
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert_eq!(result.violations.len(), 3);
        // Should be sorted: a.rs:2, a.rs:10, b.rs:5
        assert_eq!(result.violations[0].file, "a.rs");
        assert_eq!(result.violations[0].line, 2);
        assert_eq!(result.violations[1].file, "a.rs");
        assert_eq!(result.violations[1].line, 10);
        assert_eq!(result.violations[2].file, "b.rs");
        assert_eq!(result.violations[2].line, 5);
    }

    #[test]
    fn test_empty_scan_passes() {
        let scan = ScanResult {
            items: vec![],
            files_scanned: 0,
            ignored_items: vec![],
        };
        let config = Config::default();
        let overrides = LintOverrides {
            no_bare_tags: true,
            require_author: vec!["TODO".to_string()],
            require_issue_ref: vec!["BUG".to_string()],
            max_message_length: Some(10),
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        assert!(result.passed);
        assert_eq!(result.violation_count, 0);
        assert_eq!(result.total_items, 0);
    }

    #[test]
    fn test_file_not_found_skips_raw_text_rules() {
        let scan = ScanResult {
            items: vec![make_item("nonexistent.rs", 1, Tag::Todo, "msg")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.no_bare_tags = Some(false);
        let overrides = LintOverrides {
            uppercase_tag: true,
            require_colon: true,
            ..default_overrides()
        };
        // File doesn't exist, so raw text checks should be silently skipped
        let result = run_lint(&scan, &config, &overrides, Path::new("/nonexistent"));
        assert!(result.passed);
    }

    #[test]
    fn test_no_bare_tags_suggestion_text() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Fixme, "")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let mut config = Config::default();
        config.lint.uppercase_tag = Some(false);
        config.lint.require_colon = Some(false);
        let overrides = LintOverrides {
            no_bare_tags: true,
            ..default_overrides()
        };
        let result = run_lint(&scan, &config, &overrides, Path::new("/tmp"));
        let suggestion = result.violations[0].suggestion.as_ref().unwrap();
        assert!(suggestion.contains("FIXME"));
        assert!(suggestion.contains("<description>"));
    }
}
