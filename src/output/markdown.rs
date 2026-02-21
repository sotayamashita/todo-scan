use crate::model::*;

/// Escape pipe characters in markdown table cells.
fn escape_cell(s: &str) -> String {
    s.replace('|', "\\|")
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
        let author = item.author.as_deref().unwrap_or("");
        let issue = item.issue_ref.as_deref().unwrap_or("");
        let deadline = item
            .deadline
            .as_ref()
            .map(|d| d.to_string())
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
            lines.push(format!("- **{}**: {}", violation.rule, violation.message));
        }
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
        };
        let output = format_list(&result);
        assert!(output.contains("| lib.rs | 42 | TODO | ! | add tests | alice | #123 |  |"));
        assert!(output.contains("**1 items found**"));
    }

    #[test]
    fn test_format_list_escapes_pipe() {
        let result = ScanResult {
            items: vec![sample_item(Tag::Todo, "a | b")],
            files_scanned: 1,
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
