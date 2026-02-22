use std::collections::HashMap;

use crate::context::ContextInfo;
use crate::model::{ClaudeTask, ClaudeTaskMetadata, Priority, Tag, TodoItem};

/// Map a tag to an imperative action verb for task subjects.
pub fn action_verb(tag: &Tag) -> &'static str {
    match tag {
        Tag::Bug | Tag::Fixme => "Fix",
        Tag::Todo => "Implement",
        Tag::Hack => "Refactor",
        Tag::Xxx => "Address",
        Tag::Note => "Review",
    }
}

/// Map a tag to a present-continuous verb for activeForm.
pub fn active_verb(tag: &Tag) -> &'static str {
    match tag {
        Tag::Bug | Tag::Fixme => "Fixing",
        Tag::Todo => "Implementing",
        Tag::Hack => "Refactoring",
        Tag::Xxx => "Addressing",
        Tag::Note => "Reviewing",
    }
}

/// Build a task subject from a TODO item.
pub fn build_subject(item: &TodoItem) -> String {
    let verb = action_verb(&item.tag);
    let msg = item.message.trim();
    if msg.is_empty() {
        format!("{} {} at {}:{}", verb, item.tag, item.file, item.line)
    } else {
        format!("{} {}", verb, msg)
    }
}

/// Build the activeForm string (present continuous) for a task.
pub fn build_active_form(item: &TodoItem) -> String {
    let verb = active_verb(&item.tag);
    let msg = item.message.trim();
    if msg.is_empty() {
        format!("{} {} at {}:{}", verb, item.tag, item.file, item.line)
    } else {
        format!("{} {}", verb, msg)
    }
}

/// Build a multi-line description for a task.
pub fn build_description(item: &TodoItem, context: Option<&ContextInfo>) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Header: tag and location
    lines.push(format!("**[{}]** `{}:{}`", item.tag, item.file, item.line));

    // Message
    let msg = item.message.trim();
    if !msg.is_empty() {
        lines.push(String::new());
        lines.push(msg.to_string());
    }

    // Priority
    let priority_str = match item.priority {
        Priority::Urgent => "Urgent (!!)",
        Priority::High => "High (!)",
        Priority::Normal => "Normal",
    };
    lines.push(String::new());
    lines.push(format!("Priority: {}", priority_str));

    // Author
    if let Some(ref author) = item.author {
        lines.push(format!("Author: `@{}`", author));
    }

    // Issue reference
    if let Some(ref issue_ref) = item.issue_ref {
        lines.push(format!("Issue: {}", issue_ref));
    }

    // Code context
    if let Some(ctx) = context {
        if !ctx.before.is_empty() || !ctx.after.is_empty() {
            lines.push(String::new());
            lines.push("```".to_string());
            for cl in &ctx.before {
                lines.push(format!("{:>4} | {}", cl.line_number, cl.content));
            }
            lines.push(format!("{:>4} > {}", item.line, item.message.trim()));
            for cl in &ctx.after {
                lines.push(format!("{:>4} | {}", cl.line_number, cl.content));
            }
            lines.push("```".to_string());
        }
    }

    lines.join("\n")
}

/// Convert a list of TodoItems into Claude Code Tasks.
pub fn build_tasks(
    items: &[TodoItem],
    context_map: &HashMap<String, ContextInfo>,
) -> Vec<ClaudeTask> {
    items
        .iter()
        .map(|item| {
            let ctx_key = format!("{}:{}", item.file, item.line);
            let context = context_map.get(&ctx_key);

            ClaudeTask {
                subject: build_subject(item),
                description: build_description(item, context),
                active_form: build_active_form(item),
                metadata: ClaudeTaskMetadata {
                    todo_scan_file: item.file.clone(),
                    todo_scan_line: item.line,
                    todo_scan_tag: item.tag.as_str().to_string(),
                    todo_scan_priority: format!("{:?}", item.priority).to_lowercase(),
                    todo_scan_author: item.author.clone(),
                    todo_scan_issue_ref: item.issue_ref.clone(),
                    todo_scan_match_key: item.match_key(),
                },
            }
        })
        .collect()
}

/// Sort items by priority (Urgent > High > Normal), then tag severity, then file/line.
pub fn sort_by_priority(items: &mut [TodoItem]) {
    items.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then(b.tag.severity().cmp(&a.tag.severity()))
            .then(a.file.cmp(&b.file))
            .then(a.line.cmp(&b.line))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextLine;
    use crate::model::Priority;

    use crate::test_helpers::helpers::make_item;

    #[test]
    fn test_action_verb_mapping() {
        assert_eq!(action_verb(&Tag::Bug), "Fix");
        assert_eq!(action_verb(&Tag::Fixme), "Fix");
        assert_eq!(action_verb(&Tag::Todo), "Implement");
        assert_eq!(action_verb(&Tag::Hack), "Refactor");
        assert_eq!(action_verb(&Tag::Xxx), "Address");
        assert_eq!(action_verb(&Tag::Note), "Review");
    }

    #[test]
    fn test_active_verb_mapping() {
        assert_eq!(active_verb(&Tag::Bug), "Fixing");
        assert_eq!(active_verb(&Tag::Fixme), "Fixing");
        assert_eq!(active_verb(&Tag::Todo), "Implementing");
        assert_eq!(active_verb(&Tag::Hack), "Refactoring");
        assert_eq!(active_verb(&Tag::Xxx), "Addressing");
        assert_eq!(active_verb(&Tag::Note), "Reviewing");
    }

    #[test]
    fn test_build_subject_with_message() {
        let item = make_item("src/main.rs", 10, Tag::Todo, "add user validation");
        assert_eq!(build_subject(&item), "Implement add user validation");
    }

    #[test]
    fn test_build_subject_empty_message() {
        let item = make_item("src/main.rs", 10, Tag::Bug, "");
        assert_eq!(build_subject(&item), "Fix BUG at src/main.rs:10");
    }

    #[test]
    fn test_build_active_form_with_message() {
        let item = make_item("src/main.rs", 10, Tag::Hack, "remove workaround");
        assert_eq!(build_active_form(&item), "Refactoring remove workaround");
    }

    #[test]
    fn test_build_active_form_empty_message() {
        let item = make_item("src/main.rs", 10, Tag::Fixme, "");
        assert_eq!(build_active_form(&item), "Fixing FIXME at src/main.rs:10");
    }

    #[test]
    fn test_build_description_basic() {
        let item = make_item("src/main.rs", 10, Tag::Todo, "implement feature");
        let desc = build_description(&item, None);
        assert!(desc.contains("**[TODO]** `src/main.rs:10`"));
        assert!(desc.contains("implement feature"));
        assert!(desc.contains("Priority: Normal"));
    }

    #[test]
    fn test_build_description_with_author_and_issue() {
        let mut item = make_item("src/main.rs", 10, Tag::Bug, "critical crash");
        item.author = Some("alice".to_string());
        item.issue_ref = Some("#42".to_string());
        item.priority = Priority::Urgent;

        let desc = build_description(&item, None);
        assert!(desc.contains("Author: `@alice`"));
        assert!(desc.contains("Issue: #42"));
        assert!(desc.contains("Priority: Urgent (!!)"));
    }

    #[test]
    fn test_build_description_with_context() {
        let item = make_item("src/main.rs", 10, Tag::Todo, "fix this");
        let ctx = ContextInfo {
            before: vec![ContextLine {
                line_number: 9,
                content: "let x = 1;".to_string(),
            }],
            after: vec![ContextLine {
                line_number: 11,
                content: "let y = 2;".to_string(),
            }],
        };

        let desc = build_description(&item, Some(&ctx));
        assert!(desc.contains("```"));
        assert!(desc.contains("let x = 1;"));
        assert!(desc.contains("let y = 2;"));
    }

    #[test]
    fn test_build_tasks_metadata() {
        let mut item = make_item("src/main.rs", 10, Tag::Bug, "fix crash");
        item.author = Some("bob".to_string());
        item.issue_ref = Some("#99".to_string());

        let tasks = build_tasks(&[item], &HashMap::new());
        assert_eq!(tasks.len(), 1);

        let task = &tasks[0];
        assert_eq!(task.subject, "Fix fix crash");
        assert_eq!(task.metadata.todo_scan_file, "src/main.rs");
        assert_eq!(task.metadata.todo_scan_line, 10);
        assert_eq!(task.metadata.todo_scan_tag, "BUG");
        assert_eq!(task.metadata.todo_scan_priority, "normal");
        assert_eq!(task.metadata.todo_scan_author, Some("bob".to_string()));
        assert_eq!(task.metadata.todo_scan_issue_ref, Some("#99".to_string()));
    }

    #[test]
    fn test_sort_by_priority_ordering() {
        let mut items = vec![
            {
                let mut i = make_item("src/main.rs", 10, Tag::Note, "low");
                i.priority = Priority::Normal;
                i
            },
            {
                let mut i = make_item("src/main.rs", 10, Tag::Bug, "critical");
                i.priority = Priority::Urgent;
                i
            },
            {
                let mut i = make_item("src/main.rs", 10, Tag::Todo, "medium");
                i.priority = Priority::High;
                i
            },
        ];

        sort_by_priority(&mut items);

        assert_eq!(items[0].priority, Priority::Urgent);
        assert_eq!(items[1].priority, Priority::High);
        assert_eq!(items[2].priority, Priority::Normal);
    }

    #[test]
    fn test_sort_by_priority_same_priority_uses_tag_severity() {
        let mut items = vec![
            make_item("src/main.rs", 10, Tag::Note, "note item"),
            make_item("src/main.rs", 10, Tag::Bug, "bug item"),
            make_item("src/main.rs", 10, Tag::Todo, "todo item"),
        ];

        sort_by_priority(&mut items);

        assert_eq!(items[0].tag, Tag::Bug);
        assert_eq!(items[1].tag, Tag::Todo);
        assert_eq!(items[2].tag, Tag::Note);
    }
}
