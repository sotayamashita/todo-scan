use crate::model::*;

/// Escape characters that break markdown table cells.
fn escape_cell(s: &str) -> String {
    s.replace('|', "\\|")
        .replace('\n', " ")
        .replace('\r', "")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('`', "\\`")
}

fn priority_str(priority: &Priority) -> &'static str {
    match priority {
        Priority::Normal => "",
        Priority::High => "!",
        Priority::Urgent => "!!",
    }
}

pub fn format_list(result: &ScanResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines
        .push("| File | Line | Tag | Priority | Message | Author | Issue | Deadline |".to_string());
    lines
        .push("|------|------|-----|----------|---------|--------|-------|----------|".to_string());

    for item in &result.items {
        let file = escape_cell(&item.file);
        let tag = item.tag.as_str();
        let priority = priority_str(&item.priority);
        let message = escape_cell(&item.message);
        let author = item
            .author
            .as_deref()
            .map(|a| escape_cell(a))
            .unwrap_or_default();
        let issue = item
            .issue_ref
            .as_deref()
            .map(|r| escape_cell(r))
            .unwrap_or_default();
        let deadline = item
            .deadline
            .as_ref()
            .map(|d| escape_cell(&d.to_string()))
            .unwrap_or_default();
        lines.push(format!(
            "| {file} | {} | {tag} | {priority} | {message} | {author} | {issue} | {deadline} |",
            item.line
        ));
    }

    lines.push(String::new());
    lines.push(format!("**{} items found**", result.items.len()));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_search(result: &SearchResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines
        .push("| File | Line | Tag | Priority | Message | Author | Issue | Deadline |".to_string());
    lines
        .push("|------|------|-----|----------|---------|--------|-------|----------|".to_string());

    for item in &result.items {
        let file = escape_cell(&item.file);
        let tag = item.tag.as_str();
        let priority = priority_str(&item.priority);
        let message = escape_cell(&item.message);
        let author = item
            .author
            .as_deref()
            .map(|a| escape_cell(a))
            .unwrap_or_default();
        let issue = item
            .issue_ref
            .as_deref()
            .map(|r| escape_cell(r))
            .unwrap_or_default();
        let deadline = item
            .deadline
            .as_ref()
            .map(|d| escape_cell(&d.to_string()))
            .unwrap_or_default();
        lines.push(format!(
            "| {file} | {} | {tag} | {priority} | {message} | {author} | {issue} | {deadline} |",
            item.line
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "**{} matches across {} files** (query: \"{}\")",
        result.match_count, result.file_count, result.query
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_diff(result: &DiffResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push("| Status | File | Line | Tag | Message |".to_string());
    lines.push("|--------|------|------|-----|---------|".to_string());

    for entry in &result.entries {
        let status = match entry.status {
            DiffStatus::Added => "+",
            DiffStatus::Removed => "-",
        };
        let file = escape_cell(&entry.item.file);
        let tag = entry.item.tag.as_str();
        let message = escape_cell(&entry.item.message);
        lines.push(format!(
            "| {status} | {file} | {} | {tag} | {message} |",
            entry.item.line
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "**+{} -{}** (base: `{}`)",
        result.added_count, result.removed_count, result.base_ref
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_blame(result: &BlameResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push("| File | Line | Tag | Message | Author | Date | Age (days) | Stale |".to_string());
    lines.push("|------|------|-----|---------|--------|------|------------|-------|".to_string());

    for entry in &result.entries {
        let file = escape_cell(&entry.item.file);
        let tag = entry.item.tag.as_str();
        let message = escape_cell(&entry.item.message);
        let stale = if entry.stale { "Yes" } else { "" };
        let blame_author = escape_cell(&entry.blame.author);
        let blame_date = escape_cell(&entry.blame.date);
        lines.push(format!(
            "| {file} | {} | {tag} | {message} | {blame_author} | {blame_date} | {} | {stale} |",
            entry.item.line, entry.blame.age_days,
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "**{} items, avg age {} days, {} stale** (threshold: {} days)",
        result.total, result.avg_age_days, result.stale_count, result.stale_threshold_days,
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_lint(result: &LintResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    if result.passed {
        lines.push("## PASS".to_string());
        lines.push(String::new());
        lines.push(format!(
            "All lint checks passed ({} items total).",
            result.total_items
        ));
    } else {
        lines.push("## FAIL".to_string());
        lines.push(String::new());
        lines.push("| File | Line | Rule | Message | Suggestion |".to_string());
        lines.push("|------|------|------|---------|------------|".to_string());

        for v in &result.violations {
            let file = escape_cell(&v.file);
            let message = escape_cell(&v.message);
            let rule = escape_cell(&v.rule);
            let suggestion = v.suggestion.as_deref().map(escape_cell).unwrap_or_default();
            lines.push(format!(
                "| {} | {} | {} | {} | {} |",
                file, v.line, rule, message, suggestion
            ));
        }

        lines.push(String::new());
        lines.push(format!(
            "**{} violations in {} items**",
            result.violation_count, result.total_items
        ));
    }

    lines.push(String::new());
    lines.join("\n")
}

pub fn format_check(result: &CheckResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    if result.passed {
        lines.push("## PASS".to_string());
        lines.push(String::new());
        lines.push(format!("All checks passed ({} items total).", result.total));
    } else {
        lines.push("## FAIL".to_string());
        lines.push(String::new());
        for violation in &result.violations {
            lines.push(format!(
                "- **{}**: {}",
                escape_cell(&violation.rule),
                escape_cell(&violation.message)
            ));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_clean(result: &CleanResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    if result.passed {
        lines.push("## PASS".to_string());
        lines.push(String::new());
        lines.push(format!(
            "All clean checks passed ({} items total).",
            result.total_items
        ));
    } else {
        lines.push("## FAIL".to_string());
        lines.push(String::new());
        lines.push("| File | Line | Rule | Message | Detail |".to_string());
        lines.push("|------|------|------|---------|--------|".to_string());

        for v in &result.violations {
            let file = escape_cell(&v.file);
            let message = escape_cell(&v.message);
            let rule = escape_cell(&v.rule);
            let detail = if let Some(ref dup_of) = v.duplicate_of {
                escape_cell(&format!("duplicate of {}", dup_of))
            } else if let Some(ref issue_ref) = v.issue_ref {
                escape_cell(issue_ref)
            } else {
                String::new()
            };
            lines.push(format!(
                "| {} | {} | {} | {} | {} |",
                file, v.line, rule, message, detail
            ));
        }

        lines.push(String::new());
        lines.push(format!(
            "**{} violations ({} stale, {} duplicates) in {} items**",
            result.violations.len(),
            result.stale_count,
            result.duplicate_count,
            result.total_items
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
    fn test_format_list_table_headers() {
        let result = ScanResult {
            items: vec![],
            files_scanned: 0,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        assert!(output
            .contains("| File | Line | Tag | Priority | Message | Author | Issue | Deadline |"));
        assert!(output.contains("**0 items found**"));
    }

    #[test]
    fn test_format_list_with_items() {
        let result = ScanResult {
            items: vec![TodoItem {
                file: "lib.rs".to_string(),
                line: 42,
                tag: Tag::Todo,
                message: "add tests".to_string(),
                author: Some("alice".to_string()),
                issue_ref: Some("#123".to_string()),
                priority: Priority::High,
                deadline: None,
            }],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        assert!(output.contains("| lib.rs | 42 | TODO | ! | add tests | alice | #123 |  |"));
        assert!(output.contains("**1 items found**"));
    }

    #[test]
    fn test_escape_cell_replaces_newline_with_space() {
        assert_eq!(escape_cell("line1\nline2"), "line1 line2");
    }

    #[test]
    fn test_escape_cell_removes_carriage_return() {
        assert_eq!(escape_cell("line1\rline2"), "line1line2");
    }

    #[test]
    fn test_escape_cell_escapes_brackets() {
        assert_eq!(escape_cell("[link](url)"), "\\[link\\](url)");
    }

    #[test]
    fn test_escape_cell_escapes_backtick() {
        assert_eq!(escape_cell("use `code` here"), "use \\`code\\` here");
    }

    #[test]
    fn test_escape_cell_still_escapes_pipe() {
        assert_eq!(escape_cell("a | b"), "a \\| b");
    }

    #[test]
    fn test_format_list_escapes_author() {
        let result = ScanResult {
            items: vec![TodoItem {
                file: "test.rs".to_string(),
                line: 1,
                tag: Tag::Todo,
                message: "task".to_string(),
                author: Some("user\ninjected".to_string()),
                issue_ref: None,
                priority: Priority::Normal,
                deadline: None,
            }],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        assert!(output.contains("user injected"));
        assert!(!output.contains("user\ninjected"));
    }

    #[test]
    fn test_format_list_escapes_issue_ref() {
        let result = ScanResult {
            items: vec![TodoItem {
                file: "test.rs".to_string(),
                line: 1,
                tag: Tag::Todo,
                message: "task".to_string(),
                author: None,
                issue_ref: Some("[link](evil)".to_string()),
                priority: Priority::Normal,
                deadline: None,
            }],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        assert!(output.contains("\\[link\\]"));
        assert!(!output.contains("[link](evil)"));
    }

    #[test]
    fn test_format_blame_escapes_author() {
        let result = BlameResult {
            entries: vec![BlameEntry {
                item: sample_item(Tag::Todo, "task"),
                blame: BlameInfo {
                    author: "user|inject".to_string(),
                    email: "user@test.com".to_string(),
                    date: "2025-01-01".to_string(),
                    age_days: 10,
                    commit: "abc123".to_string(),
                },
                stale: false,
            }],
            total: 1,
            avg_age_days: 10,
            stale_count: 0,
            stale_threshold_days: 180,
        };
        let output = format_blame(&result);
        assert!(output.contains("user\\|inject"));
    }

    #[test]
    fn test_format_lint_escapes_rule() {
        let result = LintResult {
            passed: false,
            total_items: 1,
            violation_count: 1,
            violations: vec![LintViolation {
                file: "test.rs".to_string(),
                line: 1,
                rule: "no`bare".to_string(),
                message: "msg".to_string(),
                suggestion: Some("use [this]".to_string()),
            }],
        };
        let output = format_lint(&result);
        assert!(output.contains("no\\`bare"));
        assert!(output.contains("\\[this\\]"));
    }

    #[test]
    fn test_format_list_escapes_pipe() {
        let result = ScanResult {
            items: vec![sample_item(Tag::Todo, "a | b")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        assert!(output.contains("a \\| b"));
    }

    #[test]
    fn test_format_diff_table() {
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
        assert!(output.contains("| + | src/main.rs | 10 | FIXME | new fix |"));
        assert!(output.contains("| - | src/main.rs | 10 | TODO | old task |"));
        assert!(output.contains("**+1 -1** (base: `main`)"));
    }

    #[test]
    fn test_format_check_pass() {
        let result = CheckResult {
            passed: true,
            total: 3,
            violations: vec![],
        };
        let output = format_check(&result);
        assert!(output.contains("## PASS"));
        assert!(output.contains("All checks passed (3 items total)."));
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
        assert!(output.contains("## FAIL"));
        assert!(output.contains("- **max**: 10 exceeds max 5"));
    }
}
