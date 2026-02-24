use crate::model::*;

/// Escape special characters per GitHub Actions workflow command spec.
fn escape_message(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// Escape property values (additionally escape `:` and `,`).
fn escape_property(s: &str) -> String {
    escape_message(s).replace(':', "%3A").replace(',', "%2C")
}

fn format_item_annotation(item: &TodoItem) -> String {
    let severity = Severity::from_item(item);
    let level = severity.as_github_actions_str();
    let file = escape_property(&item.file);
    let title = item.tag.as_str();
    let mut msg = escape_message(&item.message);
    if let Some(ref deadline) = item.deadline {
        msg.push_str(&format!(" (deadline: {})", deadline));
    }
    format!(
        "::{level} file={file},line={},title={title}::[{title}] {msg}",
        item.line
    )
}

pub fn format_list(result: &ScanResult) -> String {
    let mut lines: Vec<String> = result.items.iter().map(format_item_annotation).collect();
    lines.push(format!(
        "::notice::todo-scan: {} items found",
        result.items.len()
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_search(result: &SearchResult) -> String {
    let mut lines: Vec<String> = result.items.iter().map(format_item_annotation).collect();
    let query = escape_message(&result.query);
    lines.push(format!(
        "::notice::todo-scan search: {} matches (query: \"{query}\")",
        result.match_count
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_diff(result: &DiffResult) -> String {
    let mut lines: Vec<String> = Vec::new();
    for entry in &result.entries {
        match entry.status {
            DiffStatus::Added => {
                lines.push(format_item_annotation(&entry.item));
            }
            DiffStatus::Removed => {
                let file = escape_property(&entry.item.file);
                let tag = entry.item.tag.as_str();
                let msg = escape_message(&entry.item.message);
                lines.push(format!(
                    "::notice file={file},line={},title=Removed {tag}::[{tag}] {msg}",
                    entry.item.line
                ));
            }
        }
    }
    lines.push(format!(
        "::notice::todo-scan diff: +{} -{}",
        result.added_count, result.removed_count
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_blame(result: &BlameResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    for entry in &result.entries {
        let level = if entry.stale { "warning" } else { "notice" };
        let file = escape_property(&entry.item.file);
        let tag = entry.item.tag.as_str();
        let msg = escape_message(&format!(
            "[{}] {} @{} {} ({} days ago)",
            tag, entry.item.message, entry.blame.author, entry.blame.date, entry.blame.age_days,
        ));
        let title = if entry.stale {
            format!("Stale {}", tag)
        } else {
            tag.to_string()
        };
        lines.push(format!(
            "::{level} file={file},line={},title={title}::{msg}",
            entry.item.line,
        ));
    }

    lines.push(format!(
        "::notice::todo-scan blame: {} items, {} stale",
        result.total, result.stale_count,
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_lint(result: &LintResult) -> String {
    let mut lines: Vec<String> = Vec::new();
    if result.passed {
        lines.push("::notice::todo-scan lint: PASS".to_string());
    } else {
        for violation in &result.violations {
            let file = escape_property(&violation.file);
            let msg = escape_message(&violation.message);
            let rule = escape_property(&violation.rule);
            lines.push(format!(
                "::error file={file},line={},title={rule}::{msg}",
                violation.line
            ));
        }
        lines.push(format!(
            "::error::todo-scan lint: FAIL ({} violations)",
            result.violation_count
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_check(result: &CheckResult) -> String {
    let mut lines: Vec<String> = Vec::new();
    if result.passed {
        lines.push("::notice::todo-scan check: PASS".to_string());
    } else {
        for violation in &result.violations {
            let msg = escape_message(&violation.message);
            let rule = escape_property(&violation.rule);
            lines.push(format!("::error title={rule}::{msg}"));
        }
        lines.push("::error::todo-scan check: FAIL".to_string());
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_clean(result: &CleanResult) -> String {
    let mut lines: Vec<String> = Vec::new();
    if result.passed {
        lines.push("::notice::todo-scan clean: PASS".to_string());
    } else {
        for violation in &result.violations {
            let file = escape_property(&violation.file);
            let msg = escape_message(&violation.message);
            let rule = escape_property(&violation.rule);
            lines.push(format!(
                "::error file={file},line={},title={rule}::{msg}",
                violation.line
            ));
        }
        lines.push(format!(
            "::error::todo-scan clean: FAIL ({} stale, {} duplicates)",
            result.stale_count, result.duplicate_count
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(tag: Tag, message: &str) -> TodoItem {
        TodoItem {
            file: "src/main.rs".to_string(),
            line: 10,
            tag,
            message: message.to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: None,
        }
    }

    #[test]
    fn test_format_list_single_item() {
        let result = ScanResult {
            items: vec![sample_item(Tag::Todo, "implement feature")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        assert!(output
            .contains("::warning file=src/main.rs,line=10,title=TODO::[TODO] implement feature"));
        assert!(output.contains("::notice::todo-scan: 1 items found"));
    }

    #[test]
    fn test_format_list_severity_mapping() {
        let result = ScanResult {
            items: vec![
                sample_item(Tag::Bug, "critical bug"),
                sample_item(Tag::Note, "a note"),
            ],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        assert!(output.contains("::error file=src/main.rs,line=10,title=BUG::[BUG] critical bug"));
        assert!(output.contains("::notice file=src/main.rs,line=10,title=NOTE::[NOTE] a note"));
    }

    #[test]
    fn test_format_list_urgent_escalates_to_error() {
        let result = ScanResult {
            items: vec![TodoItem {
                file: "lib.rs".to_string(),
                line: 5,
                tag: Tag::Todo,
                message: "urgent task".to_string(),
                author: None,
                issue_ref: None,
                priority: Priority::Urgent,
                deadline: None,
            }],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        assert!(output.contains("::error file=lib.rs,line=5,title=TODO::[TODO] urgent task"));
    }

    #[test]
    fn test_escape_special_characters() {
        let result = ScanResult {
            items: vec![sample_item(Tag::Todo, "fix 100% of bugs\nline2")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        assert!(output.contains("fix 100%25 of bugs%0Aline2"));
    }

    #[test]
    fn test_format_diff_added_and_removed() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    status: DiffStatus::Added,
                    item: sample_item(Tag::Fixme, "new fix"),
                },
                DiffEntry {
                    status: DiffStatus::Removed,
                    item: sample_item(Tag::Todo, "old task"),
                },
            ],
            added_count: 1,
            removed_count: 1,
            base_ref: "main".to_string(),
        };
        let output = format_diff(&result);
        assert!(output.contains("::error file=src/main.rs,line=10,title=FIXME::[FIXME] new fix"));
        assert!(output
            .contains("::notice file=src/main.rs,line=10,title=Removed TODO::[TODO] old task"));
        assert!(output.contains("::notice::todo-scan diff: +1 -1"));
    }

    #[test]
    fn test_format_check_pass() {
        let result = CheckResult {
            passed: true,
            total: 5,
            violations: vec![],
        };
        let output = format_check(&result);
        assert!(output.contains("::notice::todo-scan check: PASS"));
        assert!(!output.contains("::error"));
    }

    #[test]
    fn test_format_check_fail() {
        let result = CheckResult {
            passed: false,
            total: 10,
            violations: vec![CheckViolation {
                rule: "max".to_string(),
                message: "10 exceeds max 5".to_string(),
            }],
        };
        let output = format_check(&result);
        assert!(output.contains("::error title=max::10 exceeds max 5"));
        assert!(output.contains("::error::todo-scan check: FAIL"));
    }

    #[test]
    fn test_format_search_escapes_query_with_newline() {
        let result = SearchResult {
            items: vec![],
            match_count: 0,
            file_count: 0,
            exact: false,
            query: "evil\n::error::injected annotation".to_string(),
        };
        let output = format_search(&result);
        // The newline in the query must be escaped to %0A so that
        // "::error::injected annotation" cannot appear on its own line
        // (GitHub Actions only parses workflow commands at line start)
        for line in output.lines() {
            assert!(
                !line.starts_with("::error::injected"),
                "query newline must be escaped to prevent workflow command injection on a separate line"
            );
        }
        assert!(output.contains("%0A"));
    }

    #[test]
    fn test_format_lint_escapes_rule_in_title() {
        let result = LintResult {
            passed: false,
            total_items: 1,
            violation_count: 1,
            violations: vec![LintViolation {
                file: "src/main.rs".to_string(),
                line: 1,
                rule: "rule:with:colons".to_string(),
                message: "bad".to_string(),
                suggestion: None,
            }],
        };
        let output = format_lint(&result);
        // Colons in rule name must be escaped in the title property
        assert!(
            !output.contains("title=rule:with:colons"),
            "rule name colons must be escaped in title property"
        );
        assert!(output.contains("title=rule%3Awith%3Acolons"));
    }

    #[test]
    fn test_format_lint_pass() {
        let result = LintResult {
            passed: true,
            total_items: 5,
            violation_count: 0,
            violations: vec![],
        };
        let output = format_lint(&result);
        assert!(output.contains("::notice::todo-scan lint: PASS"));
        assert!(!output.contains("::error"));
    }

    #[test]
    fn test_format_blame_basic() {
        let result = BlameResult {
            entries: vec![BlameEntry {
                item: sample_item(Tag::Todo, "old item"),
                blame: BlameInfo {
                    author: "alice".to_string(),
                    email: "alice@test.com".to_string(),
                    date: "2024-01-01".to_string(),
                    age_days: 400,
                    commit: "abc123".to_string(),
                },
                stale: true,
            }],
            total: 1,
            avg_age_days: 400,
            stale_count: 1,
            stale_threshold_days: 365,
        };
        let output = format_blame(&result);
        assert!(output.contains("::warning file=src/main.rs,line=10,title=Stale TODO::"));
        assert!(output.contains("@alice"));
        assert!(output.contains("400 days ago"));
        assert!(output.contains("::notice::todo-scan blame: 1 items, 1 stale"));
    }

    #[test]
    fn test_format_blame_not_stale() {
        let result = BlameResult {
            entries: vec![BlameEntry {
                item: sample_item(Tag::Fixme, "recent"),
                blame: BlameInfo {
                    author: "bob".to_string(),
                    email: "bob@test.com".to_string(),
                    date: "2025-01-01".to_string(),
                    age_days: 10,
                    commit: "def456".to_string(),
                },
                stale: false,
            }],
            total: 1,
            avg_age_days: 10,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let output = format_blame(&result);
        assert!(output.contains("::notice file=src/main.rs,line=10,title=FIXME::"));
    }

    #[test]
    fn test_format_clean_pass() {
        let result = CleanResult {
            passed: true,
            total_items: 3,
            stale_count: 0,
            duplicate_count: 0,
            violations: vec![],
        };
        let output = format_clean(&result);
        assert!(output.contains("::notice::todo-scan clean: PASS"));
    }

    #[test]
    fn test_format_clean_fail() {
        let result = CleanResult {
            passed: false,
            total_items: 2,
            stale_count: 1,
            duplicate_count: 1,
            violations: vec![CleanViolation {
                file: "test.rs".to_string(),
                line: 10,
                rule: "stale_issue".to_string(),
                message: "stale issue".to_string(),
                issue_ref: Some("#42".to_string()),
                duplicate_of: None,
            }],
        };
        let output = format_clean(&result);
        assert!(output.contains("::error file=test.rs,line=10,title=stale_issue::stale issue"));
        assert!(output.contains("::error::todo-scan clean: FAIL (1 stale, 1 duplicates)"));
    }

    #[test]
    fn test_format_item_annotation_with_deadline() {
        use crate::deadline::Deadline;
        let item = TodoItem {
            file: "test.rs".to_string(),
            line: 5,
            tag: Tag::Todo,
            message: "task".to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: Some(Deadline {
                year: 2025,
                month: 6,
                day: 15,
            }),
        };
        let output = format_item_annotation(&item);
        assert!(output.contains("(deadline: 2025-06-15)"));
    }

    #[test]
    fn test_format_search_with_items() {
        let result = SearchResult {
            query: "fix".to_string(),
            exact: false,
            items: vec![sample_item(Tag::Fixme, "fix this")],
            match_count: 1,
            file_count: 1,
        };
        let output = format_search(&result);
        assert!(output.contains("::error file=src/main.rs,line=10,title=FIXME::[FIXME] fix this"));
        assert!(output.contains("::notice::todo-scan search: 1 matches"));
    }

    #[test]
    fn test_escape_message_carriage_return() {
        assert_eq!(escape_message("a\rb"), "a%0Db");
    }

    #[test]
    fn test_escape_property_commas() {
        assert_eq!(escape_property("a,b"), "a%2Cb");
    }
}
