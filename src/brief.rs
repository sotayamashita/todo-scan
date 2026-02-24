use std::collections::HashSet;

use crate::model::*;

pub fn compute_brief(scan: &ScanResult, diff: Option<&DiffResult>) -> BriefResult {
    let total_items = scan.items.len();

    let total_files = scan
        .items
        .iter()
        .map(|i| i.file.as_str())
        .collect::<HashSet<_>>()
        .len();

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

    let top_urgent = scan
        .items
        .iter()
        .filter(|i| i.priority != Priority::Normal)
        .max_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.tag.severity().cmp(&b.tag.severity()))
        })
        .cloned();

    BriefResult {
        total_items,
        total_files,
        priority_counts: PriorityCounts {
            normal,
            high,
            urgent,
        },
        top_urgent,
        trend: diff.map(|d| TrendInfo {
            added: d.added_count,
            removed: d.removed_count,
            base_ref: d.base_ref.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::helpers::make_item;

    #[test]
    fn test_basic_counts() {
        let mut items = vec![
            make_item("a.rs", 1, Tag::Todo, "task one"),
            make_item("a.rs", 2, Tag::Todo, "task two"),
            make_item("b.rs", 1, Tag::Fixme, "fix this"),
        ];
        items[1].priority = Priority::High;
        items[2].priority = Priority::Urgent;

        let scan = ScanResult {
            items,
            files_scanned: 2,
            ignored_items: vec![],
        };

        let result = compute_brief(&scan, None);
        assert_eq!(result.total_items, 3);
        assert_eq!(result.total_files, 2);
        assert_eq!(result.priority_counts.normal, 1);
        assert_eq!(result.priority_counts.high, 1);
        assert_eq!(result.priority_counts.urgent, 1);
    }

    #[test]
    fn test_top_urgent_selected() {
        let mut items = vec![
            make_item("a.rs", 1, Tag::Todo, "normal task"),
            make_item("b.rs", 5, Tag::Bug, "urgent bug"),
            make_item("c.rs", 10, Tag::Todo, "high task"),
        ];
        items[1].priority = Priority::Urgent;
        items[2].priority = Priority::High;

        let scan = ScanResult {
            items,
            files_scanned: 3,
            ignored_items: vec![],
        };

        let result = compute_brief(&scan, None);
        let top = result.top_urgent.expect("should have a top urgent item");
        assert_eq!(top.file, "b.rs");
        assert_eq!(top.line, 5);
        assert_eq!(top.priority, Priority::Urgent);
        assert_eq!(top.tag, Tag::Bug);
    }

    #[test]
    fn test_top_urgent_none_when_all_normal() {
        let items = vec![
            make_item("a.rs", 1, Tag::Todo, "task one"),
            make_item("b.rs", 1, Tag::Note, "note"),
        ];

        let scan = ScanResult {
            items,
            files_scanned: 2,
            ignored_items: vec![],
        };

        let result = compute_brief(&scan, None);
        assert!(result.top_urgent.is_none());
    }

    #[test]
    fn test_trend_from_diff() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "task")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let diff = DiffResult {
            entries: vec![],
            added_count: 5,
            removed_count: 2,
            base_ref: "main".to_string(),
        };

        let result = compute_brief(&scan, Some(&diff));
        let trend = result.trend.expect("should have trend info");
        assert_eq!(trend.added, 5);
        assert_eq!(trend.removed, 2);
        assert_eq!(trend.base_ref, "main");
    }

    #[test]
    fn test_top_urgent_tiebreak_by_tag_severity() {
        // Two high-priority items: Bug (severity 5) should win over Todo (severity 1)
        let mut items = vec![
            make_item("a.rs", 1, Tag::Todo, "high todo"),
            make_item("b.rs", 2, Tag::Bug, "high bug"),
        ];
        items[0].priority = Priority::High;
        items[1].priority = Priority::High;

        let scan = ScanResult {
            items,
            files_scanned: 2,
            ignored_items: vec![],
        };

        let result = compute_brief(&scan, None);
        let top = result.top_urgent.expect("should have top urgent");
        assert_eq!(top.tag, Tag::Bug);
    }

    #[test]
    fn test_top_urgent_prefers_urgent_over_high() {
        let mut items = vec![
            make_item("a.rs", 1, Tag::Bug, "high bug"),
            make_item("b.rs", 2, Tag::Todo, "urgent todo"),
        ];
        items[0].priority = Priority::High;
        items[1].priority = Priority::Urgent;

        let scan = ScanResult {
            items,
            files_scanned: 2,
            ignored_items: vec![],
        };

        let result = compute_brief(&scan, None);
        let top = result.top_urgent.expect("should have top urgent");
        assert_eq!(top.priority, Priority::Urgent);
    }

    #[test]
    fn test_single_file_counts_as_one() {
        let items = vec![
            make_item("a.rs", 1, Tag::Todo, "first"),
            make_item("a.rs", 2, Tag::Fixme, "second"),
            make_item("a.rs", 3, Tag::Bug, "third"),
        ];

        let scan = ScanResult {
            items,
            files_scanned: 1,
            ignored_items: vec![],
        };

        let result = compute_brief(&scan, None);
        assert_eq!(result.total_items, 3);
        assert_eq!(result.total_files, 1);
    }

    #[test]
    fn test_all_priorities_with_no_diff() {
        let mut items = vec![
            make_item("a.rs", 1, Tag::Todo, "n1"),
            make_item("a.rs", 2, Tag::Todo, "n2"),
            make_item("b.rs", 1, Tag::Fixme, "h1"),
            make_item("c.rs", 1, Tag::Bug, "u1"),
        ];
        items[2].priority = Priority::High;
        items[3].priority = Priority::Urgent;

        let scan = ScanResult {
            items,
            files_scanned: 3,
            ignored_items: vec![],
        };

        let result = compute_brief(&scan, None);
        assert_eq!(result.priority_counts.normal, 2);
        assert_eq!(result.priority_counts.high, 1);
        assert_eq!(result.priority_counts.urgent, 1);
        assert!(result.trend.is_none());
    }

    #[test]
    fn test_empty_scan() {
        let scan = ScanResult {
            items: vec![],
            files_scanned: 0,
            ignored_items: vec![],
        };

        let result = compute_brief(&scan, None);
        assert_eq!(result.total_items, 0);
        assert_eq!(result.total_files, 0);
        assert_eq!(result.priority_counts.normal, 0);
        assert_eq!(result.priority_counts.high, 0);
        assert_eq!(result.priority_counts.urgent, 0);
        assert!(result.top_urgent.is_none());
        assert!(result.trend.is_none());
    }
}
