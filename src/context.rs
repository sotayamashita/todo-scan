use serde::Serialize;
use std::path::Path;

use anyhow::{Context, Result};

use crate::model::TodoItem;

#[derive(Debug, Clone, Serialize)]
pub struct ContextLine {
    pub line_number: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextInfo {
    pub before: Vec<ContextLine>,
    pub after: Vec<ContextLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RichContext {
    pub file: String,
    pub line: usize,
    pub before: Vec<ContextLine>,
    pub todo_line: String,
    pub after: Vec<ContextLine>,
    pub related_todos: Vec<RelatedTodo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelatedTodo {
    pub line: usize,
    pub tag: String,
    pub message: String,
}

/// Extract context lines around a target line from file content.
/// `target_line` is 1-based.
pub fn extract_context(content: &str, target_line: usize, n: usize) -> ContextInfo {
    if target_line == 0 {
        return ContextInfo {
            before: Vec::new(),
            after: Vec::new(),
        };
    }

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let idx = target_line - 1; // convert to 0-based

    if idx >= total {
        return ContextInfo {
            before: Vec::new(),
            after: Vec::new(),
        };
    }

    let before_start = idx.saturating_sub(n);
    let before: Vec<ContextLine> = (before_start..idx)
        .map(|i| ContextLine {
            line_number: i + 1,
            content: lines[i].to_string(),
        })
        .collect();

    let after_end = (idx + 1 + n).min(total);
    let after: Vec<ContextLine> = ((idx + 1)..after_end)
        .map(|i| ContextLine {
            line_number: i + 1,
            content: lines[i].to_string(),
        })
        .collect();

    ContextInfo { before, after }
}

/// Read file and extract context around the given line.
/// Returns (ContextInfo, todo_line_content).
pub fn read_file_context(
    root: &Path,
    file: &str,
    line: usize,
    n: usize,
) -> Result<(ContextInfo, String)> {
    let path = root.join(file);
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("cannot read file: {}", file))?;

    let lines: Vec<&str> = content.lines().collect();
    let todo_line = if line > 0 && line <= lines.len() {
        lines[line - 1].to_string()
    } else {
        String::new()
    };

    let ctx = extract_context(&content, line, n);
    Ok((ctx, todo_line))
}

/// Build a RichContext for the standalone `context` subcommand.
pub fn build_rich_context(
    root: &Path,
    file: &str,
    line: usize,
    n: usize,
    todos_in_file: &[&TodoItem],
) -> Result<RichContext> {
    let (ctx, todo_line) = read_file_context(root, file, line, n)?;

    let window_start = line.saturating_sub(n);
    let window_end = line + n;

    let related_todos: Vec<RelatedTodo> = todos_in_file
        .iter()
        .filter(|item| item.line != line && item.line >= window_start && item.line <= window_end)
        .map(|item| RelatedTodo {
            line: item.line,
            tag: item.tag.as_str().to_string(),
            message: item.message.clone(),
        })
        .collect();

    Ok(RichContext {
        file: file.to_string(),
        line,
        before: ctx.before,
        todo_line,
        after: ctx.after,
        related_todos,
    })
}

/// Collect context for a list of TODO items, reading each unique file once.
pub fn collect_context_map(
    root: &Path,
    items: &[TodoItem],
    n: usize,
) -> std::collections::HashMap<String, ContextInfo> {
    use std::collections::HashMap;

    let mut file_contents: HashMap<String, String> = HashMap::new();
    let mut context_map: HashMap<String, ContextInfo> = HashMap::new();

    for item in items {
        let content = file_contents.entry(item.file.clone()).or_insert_with(|| {
            let path = root.join(&item.file);
            std::fs::read_to_string(&path).unwrap_or_default()
        });

        let ctx = extract_context(content, item.line, n);
        let key = format!("{}:{}", item.file, item.line);
        context_map.insert(key, ctx);
    }

    context_map
}

/// Resolve a location that may be a stable TODO ID or a `file:line` string.
/// First tries to match against `item.id()` for all scanned items.
/// Falls back to `parse_location()` if no ID match is found.
pub fn resolve_location(location: &str, items: &[TodoItem]) -> Result<(String, usize)> {
    for item in items {
        if item.id() == location {
            return Ok((item.file.clone(), item.line));
        }
    }
    parse_location(location)
}

/// Parse a location string like "file.rs:42" into (file, line).
pub fn parse_location(location: &str) -> Result<(String, usize)> {
    let parts: Vec<&str> = location.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "invalid location format: expected FILE:LINE, got '{}'",
            location
        );
    }

    let line: usize = parts[0]
        .parse()
        .with_context(|| format!("invalid line number: '{}'", parts[0]))?;

    let file = parts[1].to_string();
    if file.is_empty() {
        anyhow::bail!("invalid location format: file path is empty");
    }

    Ok((file, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_context_basic() {
        let content = "line1\nline2\nline3\nline4\nline5\n";
        let ctx = extract_context(content, 3, 1);

        assert_eq!(ctx.before.len(), 1);
        assert_eq!(ctx.before[0].line_number, 2);
        assert_eq!(ctx.before[0].content, "line2");

        assert_eq!(ctx.after.len(), 1);
        assert_eq!(ctx.after[0].line_number, 4);
        assert_eq!(ctx.after[0].content, "line4");
    }

    #[test]
    fn test_extract_context_at_file_start() {
        let content = "first\nsecond\nthird\n";
        let ctx = extract_context(content, 1, 2);

        assert_eq!(ctx.before.len(), 0);
        assert_eq!(ctx.after.len(), 2);
        assert_eq!(ctx.after[0].content, "second");
        assert_eq!(ctx.after[1].content, "third");
    }

    #[test]
    fn test_extract_context_at_file_end() {
        let content = "first\nsecond\nthird\n";
        let ctx = extract_context(content, 3, 2);

        assert_eq!(ctx.before.len(), 2);
        assert_eq!(ctx.before[0].content, "first");
        assert_eq!(ctx.before[1].content, "second");
        assert_eq!(ctx.after.len(), 0);
    }

    #[test]
    fn test_extract_context_zero_lines() {
        let content = "line1\nline2\nline3\n";
        let ctx = extract_context(content, 2, 0);

        assert_eq!(ctx.before.len(), 0);
        assert_eq!(ctx.after.len(), 0);
    }

    #[test]
    fn test_extract_context_more_than_available() {
        let content = "only\n";
        let ctx = extract_context(content, 1, 10);

        assert_eq!(ctx.before.len(), 0);
        assert_eq!(ctx.after.len(), 0);
    }

    #[test]
    fn test_extract_context_target_beyond_file() {
        let content = "line1\nline2\n";
        let ctx = extract_context(content, 100, 2);

        assert_eq!(ctx.before.len(), 0);
        assert_eq!(ctx.after.len(), 0);
    }

    #[test]
    fn test_extract_context_target_zero() {
        let content = "line1\nline2\n";
        let ctx = extract_context(content, 0, 2);

        assert_eq!(ctx.before.len(), 0);
        assert_eq!(ctx.after.len(), 0);
    }

    #[test]
    fn test_resolve_location_matches_id() {
        let items = vec![TodoItem {
            file: "src/main.rs".to_string(),
            line: 42,
            tag: crate::model::Tag::Todo,
            message: "fix this bug".to_string(),
            author: None,
            issue_ref: None,
            priority: crate::model::Priority::Normal,
            deadline: None,
        }];
        let (file, line) = resolve_location("src/main.rs:TODO:fix this bug", &items).unwrap();
        assert_eq!(file, "src/main.rs");
        assert_eq!(line, 42);
    }

    #[test]
    fn test_resolve_location_falls_back_to_file_line() {
        let items = vec![TodoItem {
            file: "src/main.rs".to_string(),
            line: 42,
            tag: crate::model::Tag::Todo,
            message: "fix this bug".to_string(),
            author: None,
            issue_ref: None,
            priority: crate::model::Priority::Normal,
            deadline: None,
        }];
        // No ID match, falls back to parse_location
        let (file, line) = resolve_location("src/lib.rs:10", &items).unwrap();
        assert_eq!(file, "src/lib.rs");
        assert_eq!(line, 10);
    }

    #[test]
    fn test_resolve_location_id_takes_priority() {
        // If location looks like an ID and matches, use the matched item's line
        let items = vec![TodoItem {
            file: "src/main.rs".to_string(),
            line: 99,
            tag: crate::model::Tag::Fixme,
            message: "urgent problem".to_string(),
            author: None,
            issue_ref: None,
            priority: crate::model::Priority::Normal,
            deadline: None,
        }];
        let (file, line) = resolve_location("src/main.rs:FIXME:urgent problem", &items).unwrap();
        assert_eq!(file, "src/main.rs");
        assert_eq!(line, 99);
    }

    #[test]
    fn test_parse_location_valid() {
        let (file, line) = parse_location("src/main.rs:25").unwrap();
        assert_eq!(file, "src/main.rs");
        assert_eq!(line, 25);
    }

    #[test]
    fn test_parse_location_no_colon() {
        assert!(parse_location("src/main.rs").is_err());
    }

    #[test]
    fn test_parse_location_invalid_line() {
        assert!(parse_location("src/main.rs:abc").is_err());
    }

    #[test]
    fn test_parse_location_empty_file() {
        assert!(parse_location(":42").is_err());
    }

    #[test]
    fn test_parse_location_windows_path() {
        let (file, line) = parse_location("src\\main.rs:10").unwrap();
        assert_eq!(file, "src\\main.rs");
        assert_eq!(line, 10);
    }

    #[test]
    fn test_read_file_context_basic() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4\nline5\n").unwrap();

        let (ctx, todo_line) = read_file_context(dir.path(), "test.rs", 3, 1).unwrap();
        assert_eq!(todo_line, "line3");
        assert_eq!(ctx.before.len(), 1);
        assert_eq!(ctx.before[0].content, "line2");
        assert_eq!(ctx.after.len(), 1);
        assert_eq!(ctx.after[0].content, "line4");
    }

    #[test]
    fn test_read_file_context_line_beyond_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "only line\n").unwrap();

        let (ctx, todo_line) = read_file_context(dir.path(), "test.rs", 100, 2).unwrap();
        assert_eq!(todo_line, "");
        assert!(ctx.before.is_empty());
        assert!(ctx.after.is_empty());
    }

    #[test]
    fn test_read_file_context_line_zero() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "line1\n").unwrap();

        let (ctx, todo_line) = read_file_context(dir.path(), "test.rs", 0, 2).unwrap();
        assert_eq!(todo_line, "");
        assert!(ctx.before.is_empty());
        assert!(ctx.after.is_empty());
    }

    #[test]
    fn test_read_file_context_file_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_file_context(dir.path(), "nonexistent.rs", 1, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_rich_context_basic() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(
            &file_path,
            "fn main() {\n    // TODO: fix this\n    println!(\"hello\");\n}\n",
        )
        .unwrap();

        let item1 = TodoItem {
            file: "test.rs".to_string(),
            line: 2,
            tag: crate::model::Tag::Todo,
            message: "fix this".to_string(),
            author: None,
            issue_ref: None,
            priority: crate::model::Priority::Normal,
            deadline: None,
        };

        let todos_in_file: Vec<&TodoItem> = vec![&item1];
        let rich = build_rich_context(dir.path(), "test.rs", 2, 1, &todos_in_file).unwrap();
        assert_eq!(rich.file, "test.rs");
        assert_eq!(rich.line, 2);
        assert!(rich.todo_line.contains("TODO"));
        assert_eq!(rich.before.len(), 1);
        assert_eq!(rich.after.len(), 1);
        // The item itself is not included in related_todos (line == target line)
        assert!(rich.related_todos.is_empty());
    }

    #[test]
    fn test_build_rich_context_with_related_todos() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(
            &file_path,
            "line1\n// TODO: first\nline3\n// FIXME: second\nline5\n",
        )
        .unwrap();

        let item1 = TodoItem {
            file: "test.rs".to_string(),
            line: 2,
            tag: crate::model::Tag::Todo,
            message: "first".to_string(),
            author: None,
            issue_ref: None,
            priority: crate::model::Priority::Normal,
            deadline: None,
        };
        let item2 = TodoItem {
            file: "test.rs".to_string(),
            line: 4,
            tag: crate::model::Tag::Fixme,
            message: "second".to_string(),
            author: None,
            issue_ref: None,
            priority: crate::model::Priority::Normal,
            deadline: None,
        };

        let todos_in_file: Vec<&TodoItem> = vec![&item1, &item2];
        let rich = build_rich_context(dir.path(), "test.rs", 2, 3, &todos_in_file).unwrap();

        // item2 at line 4 is within window (2-3=0..2+3=5), and != target line 2
        assert_eq!(rich.related_todos.len(), 1);
        assert_eq!(rich.related_todos[0].line, 4);
        assert_eq!(rich.related_todos[0].tag, "FIXME");
        assert_eq!(rich.related_todos[0].message, "second");
    }

    #[test]
    fn test_collect_context_map_basic() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

        let items = vec![TodoItem {
            file: "test.rs".to_string(),
            line: 2,
            tag: crate::model::Tag::Todo,
            message: "do something".to_string(),
            author: None,
            issue_ref: None,
            priority: crate::model::Priority::Normal,
            deadline: None,
        }];

        let map = collect_context_map(dir.path(), &items, 1);
        assert_eq!(map.len(), 1);
        let ctx = map.get("test.rs:2").unwrap();
        assert_eq!(ctx.before.len(), 1);
        assert_eq!(ctx.before[0].content, "line1");
        assert_eq!(ctx.after.len(), 1);
        assert_eq!(ctx.after[0].content, "line3");
    }

    #[test]
    fn test_collect_context_map_multiple_items_same_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "a\nb\nc\nd\ne\n").unwrap();

        let items = vec![
            TodoItem {
                file: "test.rs".to_string(),
                line: 2,
                tag: crate::model::Tag::Todo,
                message: "first".to_string(),
                author: None,
                issue_ref: None,
                priority: crate::model::Priority::Normal,
                deadline: None,
            },
            TodoItem {
                file: "test.rs".to_string(),
                line: 4,
                tag: crate::model::Tag::Fixme,
                message: "second".to_string(),
                author: None,
                issue_ref: None,
                priority: crate::model::Priority::Normal,
                deadline: None,
            },
        ];

        let map = collect_context_map(dir.path(), &items, 1);
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("test.rs:2"));
        assert!(map.contains_key("test.rs:4"));
    }

    #[test]
    fn test_collect_context_map_file_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let items = vec![TodoItem {
            file: "nonexistent.rs".to_string(),
            line: 1,
            tag: crate::model::Tag::Todo,
            message: "missing".to_string(),
            author: None,
            issue_ref: None,
            priority: crate::model::Priority::Normal,
            deadline: None,
        }];

        let map = collect_context_map(dir.path(), &items, 1);
        // Should still have an entry but with empty context
        assert_eq!(map.len(), 1);
        let ctx = map.get("nonexistent.rs:1").unwrap();
        assert!(ctx.before.is_empty());
        assert!(ctx.after.is_empty());
    }

    #[test]
    fn test_resolve_location_no_match_no_colon() {
        let items: Vec<TodoItem> = vec![];
        let result = resolve_location("invalid", &items);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_context_large_n() {
        let content = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
        let ctx = extract_context(content, 5, 100);
        assert_eq!(ctx.before.len(), 4); // lines 1-4
        assert_eq!(ctx.after.len(), 5); // lines 6-10
    }
}
