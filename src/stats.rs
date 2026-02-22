use std::collections::HashMap;

use crate::model::*;

pub fn compute_stats(scan: &ScanResult, diff: Option<&DiffResult>) -> StatsResult {
    let total_items = scan.items.len();

    // Unique file count
    let mut file_set: HashMap<&str, usize> = HashMap::new();
    for item in &scan.items {
        *file_set.entry(item.file.as_str()).or_insert(0) += 1;
    }
    let total_files = file_set.len();

    // Tag counts
    let mut tag_map: HashMap<Tag, usize> = HashMap::new();
    for item in &scan.items {
        *tag_map.entry(item.tag).or_insert(0) += 1;
    }
    let mut tag_counts: Vec<(Tag, usize)> = tag_map.into_iter().collect();
    tag_counts.sort_by(|a, b| b.1.cmp(&a.1));

    // Priority counts
    let mut normal = 0;
    let mut high = 0;
    let mut urgent = 0;
    for item in &scan.items {
        match item.priority {
            Priority::Normal => normal += 1,
            Priority::High => high += 1,
            Priority::Urgent => urgent += 1,
        }
    }
    let priority_counts = PriorityCounts {
        normal,
        high,
        urgent,
    };

    // Author counts
    let mut author_map: HashMap<String, usize> = HashMap::new();
    for item in &scan.items {
        let key = item
            .author
            .clone()
            .unwrap_or_else(|| "unassigned".to_string());
        *author_map.entry(key).or_insert(0) += 1;
    }
    let mut author_counts: Vec<(String, usize)> = author_map.into_iter().collect();
    author_counts.sort_by(|a, b| b.1.cmp(&a.1));

    // Hotspot files (top 5 by count)
    let mut hotspot_files: Vec<(String, usize)> = file_set
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    hotspot_files.sort_by(|a, b| b.1.cmp(&a.1));
    hotspot_files.truncate(5);

    // Trend info from diff
    let trend = diff.map(|d| TrendInfo {
        added: d.added_count,
        removed: d.removed_count,
        base_ref: d.base_ref.clone(),
    });

    StatsResult {
        total_items,
        total_files,
        tag_counts,
        priority_counts,
        author_counts,
        hotspot_files,
        trend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Priority, Tag};
    use crate::test_helpers::helpers::make_item;

    #[test]
    fn test_basic_counts() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Todo, "task one"),
                make_item("a.rs", 2, Tag::Todo, "task two"),
                make_item("b.rs", 1, Tag::Fixme, "fix this"),
            ],
            files_scanned: 2,
        };

        let result = compute_stats(&scan, None);
        assert_eq!(result.total_items, 3);
        assert_eq!(result.total_files, 2);
        assert_eq!(result.tag_counts.len(), 2);
        assert!(result.trend.is_none());
    }

    #[test]
    fn test_priority_counts() {
        let mut items = vec![
            make_item("a.rs", 1, Tag::Todo, "normal"),
            make_item("a.rs", 2, Tag::Todo, "high"),
            make_item("a.rs", 3, Tag::Todo, "urgent"),
        ];
        items[1].priority = Priority::High;
        items[2].priority = Priority::Urgent;

        let scan = ScanResult {
            items,
            files_scanned: 1,
        };

        let result = compute_stats(&scan, None);
        assert_eq!(result.priority_counts.normal, 1);
        assert_eq!(result.priority_counts.high, 1);
        assert_eq!(result.priority_counts.urgent, 1);
    }

    #[test]
    fn test_author_counts() {
        let mut items = vec![
            make_item("a.rs", 1, Tag::Todo, "alice task"),
            make_item("a.rs", 2, Tag::Todo, "bob task"),
            make_item("a.rs", 3, Tag::Todo, "no author"),
        ];
        items[0].author = Some("alice".to_string());
        items[1].author = Some("bob".to_string());

        let scan = ScanResult {
            items,
            files_scanned: 1,
        };

        let result = compute_stats(&scan, None);
        assert_eq!(result.author_counts.len(), 3);
    }

    #[test]
    fn test_hotspot_files_limited_to_5() {
        let items: Vec<TodoItem> = (0..10)
            .map(|i| make_item(&format!("file{}.rs", i), 1, Tag::Todo, "task"))
            .collect();

        let scan = ScanResult {
            items,
            files_scanned: 10,
        };

        let result = compute_stats(&scan, None);
        assert_eq!(result.hotspot_files.len(), 5);
    }

    #[test]
    fn test_trend_from_diff() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "task")],
            files_scanned: 1,
        };
        let diff = DiffResult {
            entries: vec![],
            added_count: 3,
            removed_count: 1,
            base_ref: "main".to_string(),
        };

        let result = compute_stats(&scan, Some(&diff));
        assert!(result.trend.is_some());
        let trend = result.trend.unwrap();
        assert_eq!(trend.added, 3);
        assert_eq!(trend.removed, 1);
        assert_eq!(trend.base_ref, "main");
    }

    #[test]
    fn test_empty_scan() {
        let scan = ScanResult {
            items: vec![],
            files_scanned: 0,
        };

        let result = compute_stats(&scan, None);
        assert_eq!(result.total_items, 0);
        assert_eq!(result.total_files, 0);
        assert!(result.tag_counts.is_empty());
        assert!(result.author_counts.is_empty());
        assert!(result.hotspot_files.is_empty());
    }
}
