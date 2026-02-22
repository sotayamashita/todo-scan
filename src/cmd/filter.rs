use anyhow::{Context, Result};

use crate::cli::PriorityFilter;
use crate::model::{self, Tag, TodoItem};

pub struct FilterOptions {
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub path: Option<String>,
    pub priority: Vec<PriorityFilter>,
}

pub fn apply_filters(items: &mut Vec<TodoItem>, filters: &FilterOptions) -> Result<()> {
    // Apply tag filter
    if !filters.tags.is_empty() {
        let filter_tags: Vec<Tag> = filters
            .tags
            .iter()
            .filter_map(|s| s.parse::<Tag>().ok())
            .collect();
        items.retain(|item| filter_tags.contains(&item.tag));
    }

    // Apply priority filter
    if !filters.priority.is_empty() {
        let priorities: Vec<model::Priority> =
            filters.priority.iter().map(|p| p.to_priority()).collect();
        items.retain(|item| priorities.contains(&item.priority));
    }

    // Apply author filter
    if let Some(ref author) = filters.author {
        items.retain(|item| item.author.as_deref() == Some(author.as_str()));
    }

    // Apply path filter
    if let Some(ref pattern) = filters.path {
        let glob = globset::Glob::new(pattern)
            .context("invalid glob pattern")?
            .compile_matcher();
        items.retain(|item| glob.is_match(&item.file));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Priority, Tag, TodoItem};
    use crate::test_helpers::helpers::make_item;

    fn make_filter_item(
        file: &str,
        tag: Tag,
        priority: Priority,
        author: Option<&str>,
    ) -> TodoItem {
        let mut item = make_item(file, 1, tag, "test");
        item.priority = priority;
        item.author = author.map(|a| a.to_string());
        item
    }

    #[test]
    fn filter_by_tag() {
        let mut items = vec![
            make_filter_item("a.rs", Tag::Todo, Priority::Normal, None),
            make_filter_item("b.rs", Tag::Fixme, Priority::Normal, None),
            make_filter_item("c.rs", Tag::Hack, Priority::Normal, None),
        ];
        let filters = FilterOptions {
            tags: vec!["TODO".to_string()],
            author: None,
            path: None,
            priority: vec![],
        };
        apply_filters(&mut items, &filters).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tag, Tag::Todo);
    }

    #[test]
    fn filter_by_multiple_tags() {
        let mut items = vec![
            make_filter_item("a.rs", Tag::Todo, Priority::Normal, None),
            make_filter_item("b.rs", Tag::Fixme, Priority::Normal, None),
            make_filter_item("c.rs", Tag::Hack, Priority::Normal, None),
        ];
        let filters = FilterOptions {
            tags: vec!["TODO".to_string(), "HACK".to_string()],
            author: None,
            path: None,
            priority: vec![],
        };
        apply_filters(&mut items, &filters).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].tag, Tag::Todo);
        assert_eq!(items[1].tag, Tag::Hack);
    }

    #[test]
    fn filter_by_priority() {
        let mut items = vec![
            make_filter_item("a.rs", Tag::Todo, Priority::Normal, None),
            make_filter_item("b.rs", Tag::Todo, Priority::High, None),
            make_filter_item("c.rs", Tag::Todo, Priority::Urgent, None),
        ];
        let filters = FilterOptions {
            tags: vec![],
            author: None,
            path: None,
            priority: vec![PriorityFilter::High],
        };
        apply_filters(&mut items, &filters).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].priority, Priority::High);
    }

    #[test]
    fn filter_by_author() {
        let mut items = vec![
            make_filter_item("a.rs", Tag::Todo, Priority::Normal, Some("alice")),
            make_filter_item("b.rs", Tag::Todo, Priority::Normal, Some("bob")),
            make_filter_item("c.rs", Tag::Todo, Priority::Normal, None),
        ];
        let filters = FilterOptions {
            tags: vec![],
            author: Some("alice".to_string()),
            path: None,
            priority: vec![],
        };
        apply_filters(&mut items, &filters).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].author.as_deref(), Some("alice"));
    }

    #[test]
    fn filter_by_path() {
        let mut items = vec![
            make_filter_item("src/main.rs", Tag::Todo, Priority::Normal, None),
            make_filter_item("src/lib.rs", Tag::Todo, Priority::Normal, None),
            make_filter_item("tests/test.rs", Tag::Todo, Priority::Normal, None),
        ];
        let filters = FilterOptions {
            tags: vec![],
            author: None,
            path: Some("src/*.rs".to_string()),
            priority: vec![],
        };
        apply_filters(&mut items, &filters).unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.file.starts_with("src/")));
    }

    #[test]
    fn filter_combined() {
        let mut items = vec![
            make_filter_item("src/main.rs", Tag::Todo, Priority::High, Some("alice")),
            make_filter_item("src/lib.rs", Tag::Fixme, Priority::Normal, Some("alice")),
            make_filter_item("tests/test.rs", Tag::Todo, Priority::High, Some("bob")),
            make_filter_item("src/util.rs", Tag::Todo, Priority::Normal, Some("alice")),
        ];
        let filters = FilterOptions {
            tags: vec!["TODO".to_string()],
            author: Some("alice".to_string()),
            path: Some("src/**".to_string()),
            priority: vec![PriorityFilter::High],
        };
        apply_filters(&mut items, &filters).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file, "src/main.rs");
    }

    #[test]
    fn empty_filters_retain_all() {
        let mut items = vec![
            make_filter_item("a.rs", Tag::Todo, Priority::Normal, None),
            make_filter_item("b.rs", Tag::Fixme, Priority::High, Some("alice")),
        ];
        let filters = FilterOptions {
            tags: vec![],
            author: None,
            path: None,
            priority: vec![],
        };
        apply_filters(&mut items, &filters).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn invalid_glob_returns_error() {
        let mut items = vec![make_filter_item("a.rs", Tag::Todo, Priority::Normal, None)];
        let filters = FilterOptions {
            tags: vec![],
            author: None,
            path: Some("[invalid".to_string()),
            priority: vec![],
        };
        assert!(apply_filters(&mut items, &filters).is_err());
    }
}
