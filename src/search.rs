use std::collections::HashSet;

use crate::model::{ScanResult, SearchResult, TodoItem};

fn matches_query(item: &TodoItem, query: &str, exact: bool) -> bool {
    if exact {
        item.message.contains(query)
            || item
                .issue_ref
                .as_deref()
                .map_or(false, |r| r.contains(query))
    } else {
        let lower_query = query.to_lowercase();
        item.message.to_lowercase().contains(&lower_query)
            || item
                .issue_ref
                .as_deref()
                .map_or(false, |r| r.to_lowercase().contains(&lower_query))
    }
}

pub fn search_items(scan: &ScanResult, query: &str, exact: bool) -> SearchResult {
    let items: Vec<TodoItem> = scan
        .items
        .iter()
        .filter(|item| matches_query(item, query, exact))
        .cloned()
        .collect();

    let file_count = items.iter().map(|i| &i.file).collect::<HashSet<_>>().len();
    let match_count = items.len();

    SearchResult {
        query: query.to_string(),
        exact,
        items,
        match_count,
        file_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Priority, ScanResult, Tag, TodoItem};

    fn make_item(file: &str, message: &str, issue_ref: Option<&str>) -> TodoItem {
        TodoItem {
            file: file.to_string(),
            line: 1,
            tag: Tag::Todo,
            message: message.to_string(),
            author: None,
            issue_ref: issue_ref.map(|s| s.to_string()),
            priority: Priority::Normal,
            deadline: None,
        }
    }

    fn make_scan(items: Vec<TodoItem>) -> ScanResult {
        ScanResult {
            files_scanned: 1,
            items,
        }
    }

    #[test]
    fn test_case_insensitive_match() {
        let scan = make_scan(vec![make_item("a.rs", "Fix the BUG", None)]);
        let result = search_items(&scan, "fix the bug", false);
        assert_eq!(result.match_count, 1);
    }

    #[test]
    fn test_exact_match_case_sensitive() {
        let scan = make_scan(vec![make_item("a.rs", "Fix the BUG", None)]);

        let result = search_items(&scan, "Fix the BUG", true);
        assert_eq!(result.match_count, 1);

        let result = search_items(&scan, "fix the bug", true);
        assert_eq!(result.match_count, 0);
    }

    #[test]
    fn test_issue_ref_match() {
        let scan = make_scan(vec![make_item("a.rs", "some task", Some("#123"))]);
        let result = search_items(&scan, "#123", false);
        assert_eq!(result.match_count, 1);
    }

    #[test]
    fn test_no_match_empty_result() {
        let scan = make_scan(vec![make_item("a.rs", "something", None)]);
        let result = search_items(&scan, "nonexistent", false);
        assert_eq!(result.match_count, 0);
        assert_eq!(result.file_count, 0);
        assert!(result.items.is_empty());
    }

    #[test]
    fn test_file_count_deduplication() {
        let scan = make_scan(vec![
            make_item("a.rs", "fix foo", None),
            make_item("a.rs", "fix bar", None),
            make_item("b.rs", "fix baz", None),
        ]);
        let result = search_items(&scan, "fix", false);
        assert_eq!(result.match_count, 3);
        assert_eq!(result.file_count, 2);
    }
}
