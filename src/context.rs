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
}
