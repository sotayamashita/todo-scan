use std::collections::HashSet;

use crate::config::Config;
use crate::deadline::Deadline;
use crate::model::*;

pub struct CheckOverrides {
    pub max: Option<usize>,
    pub block_tags: Vec<String>,
    pub max_new: Option<usize>,
    pub expired: bool,
}

pub fn run_check(
    scan: &ScanResult,
    diff: Option<&DiffResult>,
    config: &Config,
    overrides: &CheckOverrides,
    today: &Deadline,
) -> CheckResult {
    let mut violations: Vec<CheckViolation> = Vec::new();

    // Step 1: block_tags check
    let blocked: HashSet<String> = overrides
        .block_tags
        .iter()
        .chain(config.check.block_tags.iter())
        .map(|t| t.to_uppercase())
        .collect();

    for item in &scan.items {
        let item_tag = item.tag.as_str().to_uppercase();
        if blocked.contains(&item_tag) {
            violations.push(CheckViolation {
                rule: "block_tags".to_string(),
                message: format!(
                    "Blocked tag {} found in {}:{}",
                    item.tag, item.file, item.line
                ),
            });
        }
    }

    // Step 2: max total check
    let max = overrides.max.or(config.check.max);
    if let Some(max) = max {
        let total = scan.items.len();
        if total > max {
            violations.push(CheckViolation {
                rule: "max".to_string(),
                message: format!("Total TODOs ({}) exceeds max ({})", total, max),
            });
        }
    }

    // Step 3: max_new check
    let max_new = overrides.max_new.or(config.check.max_new);
    if let Some(max_new) = max_new {
        if let Some(diff) = diff {
            if diff.added_count > max_new {
                violations.push(CheckViolation {
                    rule: "max_new".to_string(),
                    message: format!(
                        "New TODOs ({}) exceeds max_new ({})",
                        diff.added_count, max_new
                    ),
                });
            }
        }
    }

    // Step 4: expired deadline check
    let check_expired = overrides.expired || config.check.expired.unwrap_or(false);
    if check_expired {
        for item in &scan.items {
            if let Some(ref deadline) = item.deadline {
                if deadline.is_expired(today) {
                    violations.push(CheckViolation {
                        rule: "expired".to_string(),
                        message: format!(
                            "Expired deadline {} in {}:{}",
                            deadline, item.file, item.line
                        ),
                    });
                }
            }
        }
    }

    let passed = violations.is_empty();
    let total = scan.items.len();

    CheckResult {
        passed,
        total,
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Priority, Tag};

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

    fn default_overrides() -> CheckOverrides {
        CheckOverrides {
            max: None,
            block_tags: vec![],
            max_new: None,
            expired: false,
        }
    }

    fn test_today() -> Deadline {
        Deadline {
            year: 2025,
            month: 6,
            day: 15,
        }
    }

    #[test]
    fn test_pass_when_under_max() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "do something")],
            files_scanned: 1,
        };
        let config = Config::default();
        let overrides = CheckOverrides {
            max: Some(5),
            ..default_overrides()
        };

        let result = run_check(&scan, None, &config, &overrides, &test_today());
        assert!(result.passed);
        assert!(result.violations.is_empty());
        assert_eq!(result.total, 1);
    }

    #[test]
    fn test_fail_when_over_max() {
        let items: Vec<TodoItem> = (0..10)
            .map(|i| make_item("a.rs", i + 1, Tag::Todo, &format!("task {}", i)))
            .collect();
        let scan = ScanResult {
            items,
            files_scanned: 1,
        };
        let config = Config::default();
        let overrides = CheckOverrides {
            max: Some(5),
            ..default_overrides()
        };

        let result = run_check(&scan, None, &config, &overrides, &test_today());
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule, "max");
        assert!(result.violations[0].message.contains("10"));
        assert!(result.violations[0].message.contains("5"));
    }

    #[test]
    fn test_block_tags_detection() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Bug, "critical bug here"),
                make_item("b.rs", 5, Tag::Todo, "normal todo"),
            ],
            files_scanned: 2,
        };
        let config = Config::default();
        let overrides = CheckOverrides {
            block_tags: vec!["BUG".to_string()],
            ..default_overrides()
        };

        let result = run_check(&scan, None, &config, &overrides, &test_today());
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule, "block_tags");
        assert!(result.violations[0].message.contains("BUG"));
        assert!(result.violations[0].message.contains("a.rs:1"));
    }

    #[test]
    fn test_max_new_with_diff() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "new todo")],
            files_scanned: 1,
        };
        let diff = DiffResult {
            entries: vec![DiffEntry {
                status: DiffStatus::Added,
                item: make_item("a.rs", 1, Tag::Todo, "new todo"),
            }],
            added_count: 5,
            removed_count: 0,
            base_ref: "HEAD~1".to_string(),
        };
        let config = Config::default();
        let overrides = CheckOverrides {
            max_new: Some(3),
            ..default_overrides()
        };

        let result = run_check(&scan, Some(&diff), &config, &overrides, &test_today());
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule, "max_new");
        assert!(result.violations[0].message.contains("5"));
        assert!(result.violations[0].message.contains("3"));
    }

    #[test]
    fn test_pass_with_no_violations() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Todo, "task one"),
                make_item("b.rs", 2, Tag::Note, "just a note"),
            ],
            files_scanned: 2,
        };
        let config = Config::default();
        let overrides = default_overrides();

        let result = run_check(&scan, None, &config, &overrides, &test_today());
        assert!(result.passed);
        assert!(result.violations.is_empty());
        assert_eq!(result.total, 2);
    }

    #[test]
    fn test_expired_deadline_detected() {
        let mut item = make_item("a.rs", 1, Tag::Todo, "overdue task");
        item.deadline = Some(Deadline {
            year: 2025,
            month: 1,
            day: 1,
        });
        let scan = ScanResult {
            items: vec![item],
            files_scanned: 1,
        };
        let config = Config::default();
        let overrides = CheckOverrides {
            expired: true,
            ..default_overrides()
        };

        let result = run_check(&scan, None, &config, &overrides, &test_today());
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule, "expired");
        assert!(result.violations[0].message.contains("2025-01-01"));
    }

    #[test]
    fn test_future_deadline_passes() {
        let mut item = make_item("a.rs", 1, Tag::Todo, "future task");
        item.deadline = Some(Deadline {
            year: 2025,
            month: 12,
            day: 31,
        });
        let scan = ScanResult {
            items: vec![item],
            files_scanned: 1,
        };
        let config = Config::default();
        let overrides = CheckOverrides {
            expired: true,
            ..default_overrides()
        };

        let result = run_check(&scan, None, &config, &overrides, &test_today());
        assert!(result.passed);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_expired_flag_not_set_ignores_deadline() {
        let mut item = make_item("a.rs", 1, Tag::Todo, "overdue but ignored");
        item.deadline = Some(Deadline {
            year: 2024,
            month: 1,
            day: 1,
        });
        let scan = ScanResult {
            items: vec![item],
            files_scanned: 1,
        };
        let config = Config::default();
        let overrides = default_overrides(); // expired: false

        let result = run_check(&scan, None, &config, &overrides, &test_today());
        assert!(result.passed);
        assert!(result.violations.is_empty());
    }
}
