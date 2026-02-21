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
        "::notice::todox: {} items found",
        result.items.len()
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_search(result: &SearchResult) -> String {
    let mut lines: Vec<String> = result.items.iter().map(format_item_annotation).collect();
    lines.push(format!(
        "::notice::todox search: {} matches (query: \"{}\")",
        result.match_count, result.query
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
        "::notice::todox diff: +{} -{}",
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
        "::notice::todox blame: {} items, {} stale",
        result.total, result.stale_count,
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_lint(result: &LintResult) -> String {
    let mut lines: Vec<String> = Vec::new();
    if result.passed {
        lines.push("::notice::todox lint: PASS".to_string());
    } else {
        for violation in &result.violations {
            let file = escape_property(&violation.file);
            let msg = escape_message(&violation.message);
            lines.push(format!(
                "::error file={file},line={},title={}::{msg}",
                violation.line, violation.rule
            ));
        }
        lines.push(format!(
            "::error::todox lint: FAIL ({} violations)",
            result.violation_count
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn format_check(result: &CheckResult) -> String {
    let mut lines: Vec<String> = Vec::new();
    if result.passed {
        lines.push("::notice::todox check: PASS".to_string());
    } else {
        for violation in &result.violations {
            let msg = escape_message(&violation.message);
            lines.push(format!("::error title={}::{msg}", violation.rule));
        }
        lines.push("::error::todox check: FAIL".to_string());
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
        };
        let output = format_list(&result);
        assert!(output
            .contains("::warning file=src/main.rs,line=10,title=TODO::[TODO] implement feature"));
        assert!(output.contains("::notice::todox: 1 items found"));
    }

    #[test]
    fn test_format_list_severity_mapping() {
        let result = ScanResult {
            items: vec![
                sample_item(Tag::Bug, "critical bug"),
                sample_item(Tag::Note, "a note"),
            ],
            files_scanned: 1,
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
        };
        let output = format_list(&result);
        assert!(output.contains("::error file=lib.rs,line=5,title=TODO::[TODO] urgent task"));
    }

    #[test]
    fn test_escape_special_characters() {
        let result = ScanResult {
            items: vec![sample_item(Tag::Todo, "fix 100% of bugs\nline2")],
            files_scanned: 1,
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
        assert!(output.contains("::notice::todox diff: +1 -1"));
    }

    #[test]
    fn test_format_check_pass() {
        let result = CheckResult {
            passed: true,
            total: 5,
            violations: vec![],
        };
        let output = format_check(&result);
        assert!(output.contains("::notice::todox check: PASS"));
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
        assert!(output.contains("::error::todox check: FAIL"));
    }
}
