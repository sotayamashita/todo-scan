use std::path::Path;

use anyhow::Result;
use regex::Regex;

use crate::blame::compute_blame;
use crate::config::Config;
use crate::date_utils;
use crate::git::git_command;
use crate::model::*;
use crate::scanner::scan_content;
use crate::stats::compute_stats;

/// Compute the full report data from a scan result.
pub fn compute_report(
    scan: &ScanResult,
    root: &Path,
    config: &Config,
    history_count: usize,
    stale_threshold_days: u64,
) -> Result<ReportResult> {
    // Reuse stats computation
    let stats = compute_stats(scan, None);

    // Compute blame for age data
    let (age_histogram, stale_count, avg_age_days) =
        match compute_blame(scan, root, stale_threshold_days) {
            Ok(blame_result) => {
                let histogram = build_age_histogram(&blame_result);
                (
                    histogram,
                    blame_result.stale_count,
                    blame_result.avg_age_days,
                )
            }
            Err(_) => (default_age_histogram(), 0, 0),
        };

    // Compute history trend
    let history = if history_count > 0 {
        compute_history(root, config, history_count).unwrap_or_default()
    } else {
        Vec::new()
    };

    let generated_at = date_utils::now_iso8601();

    let summary = ReportSummary {
        total_items: stats.total_items,
        total_files: stats.total_files,
        files_scanned: scan.files_scanned,
        urgent_count: stats.priority_counts.urgent,
        high_count: stats.priority_counts.high,
        stale_count,
        avg_age_days,
    };

    Ok(ReportResult {
        generated_at,
        summary,
        tag_counts: stats.tag_counts,
        priority_counts: stats.priority_counts,
        author_counts: stats.author_counts,
        hotspot_files: stats.hotspot_files,
        history,
        age_histogram,
        items: scan.items.clone(),
    })
}

/// Sample N commits from git history and count tagged items at each.
pub fn compute_history(
    root: &Path,
    config: &Config,
    sample_count: usize,
) -> Result<Vec<HistoryPoint>> {
    // Get commit list (hash + date)
    let log_output = git_command(
        &[
            "log",
            "--format=%H %aI",
            "--first-parent",
            "--no-merges",
            "-n",
            "500",
        ],
        root,
    )?;

    let commits: Vec<(&str, &str)> = log_output
        .lines()
        .filter_map(|line| {
            let (hash, date) = line.split_once(' ')?;
            Some((hash, date))
        })
        .collect();

    if commits.is_empty() {
        return Ok(Vec::new());
    }

    let indices = select_sample_indices(commits.len(), sample_count);
    let pattern_str = config.tags_pattern();
    let pattern = Regex::new(&pattern_str)?;

    let mut history = Vec::new();

    for idx in indices {
        let (hash, date) = commits[idx];
        let short_hash = &hash[..hash.len().min(8)];
        let date_str = date.split('T').next().unwrap_or(date);

        // List files at this commit
        let file_list = match git_command(&["ls-tree", "-r", "--name-only", "--", hash], root) {
            Ok(output) => output,
            Err(_) => continue,
        };

        let mut count = 0;
        for file_path in file_list.lines() {
            let file_path = file_path.trim();
            if file_path.is_empty() {
                continue;
            }

            let content = match git_command(&["show", &format!("{}:{}", hash, file_path)], root) {
                Ok(c) => c,
                Err(_) => continue,
            };

            count += scan_content(&content, file_path, &pattern).items.len();
        }

        history.push(HistoryPoint {
            commit: short_hash.to_string(),
            date: date_str.to_string(),
            count,
        });
    }

    // Chronological order (oldest first)
    history.reverse();

    Ok(history)
}

/// Build age histogram from blame result.
pub fn build_age_histogram(blame_result: &BlameResult) -> Vec<AgeBucket> {
    let mut buckets = [0usize; 6];
    // Buckets: <1w, 1-4w, 1-3m, 3-6m, 6-12m, >1y

    for entry in &blame_result.entries {
        let days = entry.blame.age_days;
        let idx = if days < 7 {
            0
        } else if days < 28 {
            1
        } else if days < 90 {
            2
        } else if days < 180 {
            3
        } else if days < 365 {
            4
        } else {
            5
        };
        buckets[idx] += 1;
    }

    let labels = [
        "<1 week",
        "1-4 weeks",
        "1-3 months",
        "3-6 months",
        "6-12 months",
        ">1 year",
    ];

    labels
        .iter()
        .zip(buckets.iter())
        .map(|(label, &count)| AgeBucket {
            label: label.to_string(),
            count,
        })
        .collect()
}

/// Return default (empty) age histogram when blame is unavailable.
fn default_age_histogram() -> Vec<AgeBucket> {
    let labels = [
        "<1 week",
        "1-4 weeks",
        "1-3 months",
        "3-6 months",
        "6-12 months",
        ">1 year",
    ];
    labels
        .iter()
        .map(|label| AgeBucket {
            label: label.to_string(),
            count: 0,
        })
        .collect()
}

/// Select evenly-spaced sample indices from a range.
/// Pure function for testability.
pub fn select_sample_indices(total: usize, sample_count: usize) -> Vec<usize> {
    if total == 0 || sample_count == 0 {
        return Vec::new();
    }
    if sample_count >= total {
        return (0..total).collect();
    }

    let step = (total - 1) as f64 / (sample_count - 1) as f64;
    (0..sample_count)
        .map(|i| (i as f64 * step).round() as usize)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_sample_indices_basic() {
        let indices = select_sample_indices(10, 3);
        assert_eq!(indices, vec![0, 5, 9]);
    }

    #[test]
    fn test_select_sample_indices_all() {
        let indices = select_sample_indices(5, 10);
        assert_eq!(indices, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_select_sample_indices_one() {
        let indices = select_sample_indices(10, 1);
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn test_select_sample_indices_empty() {
        assert!(select_sample_indices(0, 5).is_empty());
        assert!(select_sample_indices(5, 0).is_empty());
    }

    #[test]
    fn test_select_sample_indices_equal() {
        let indices = select_sample_indices(3, 3);
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_build_age_histogram_empty() {
        let blame = BlameResult {
            entries: vec![],
            total: 0,
            avg_age_days: 0,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram.len(), 6);
        for bucket in &histogram {
            assert_eq!(bucket.count, 0);
        }
    }

    #[test]
    fn test_build_age_histogram_single_bucket() {
        let entry = BlameEntry {
            item: TodoItem {
                file: "test.rs".to_string(),
                line: 1,
                tag: Tag::Todo,
                message: "test".to_string(),
                author: None,
                issue_ref: None,
                priority: Priority::Normal,
                deadline: None,
            },
            blame: BlameInfo {
                author: "test".to_string(),
                email: "test@test.com".to_string(),
                date: "2024-01-01".to_string(),
                age_days: 3,
                commit: "abc12345".to_string(),
            },
            stale: false,
        };
        let blame = BlameResult {
            entries: vec![entry],
            total: 1,
            avg_age_days: 3,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[0].count, 1); // <1 week
        for bucket in &histogram[1..] {
            assert_eq!(bucket.count, 0);
        }
    }

    #[test]
    fn test_build_age_histogram_all_buckets() {
        let ages = [3, 14, 60, 120, 250, 400];
        let entries: Vec<BlameEntry> = ages
            .iter()
            .map(|&age| BlameEntry {
                item: TodoItem {
                    file: "test.rs".to_string(),
                    line: 1,
                    tag: Tag::Todo,
                    message: "test".to_string(),
                    author: None,
                    issue_ref: None,
                    priority: Priority::Normal,
                    deadline: None,
                },
                blame: BlameInfo {
                    author: "test".to_string(),
                    email: "test@test.com".to_string(),
                    date: "2024-01-01".to_string(),
                    age_days: age,
                    commit: "abc12345".to_string(),
                },
                stale: age >= 365,
            })
            .collect();

        let blame = BlameResult {
            entries,
            total: 6,
            avg_age_days: 141,
            stale_count: 1,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        for bucket in &histogram {
            assert_eq!(bucket.count, 1);
        }
    }

    // ── Helper to reduce boilerplate ──────────────────────────────────
    fn make_blame_entry(age_days: u64) -> BlameEntry {
        BlameEntry {
            item: TodoItem {
                file: "test.rs".to_string(),
                line: 1,
                tag: Tag::Todo,
                message: "test".to_string(),
                author: None,
                issue_ref: None,
                priority: Priority::Normal,
                deadline: None,
            },
            blame: BlameInfo {
                author: "tester".to_string(),
                email: "tester@test.com".to_string(),
                date: "2024-01-01".to_string(),
                age_days,
                commit: "abc12345".to_string(),
            },
            stale: false,
        }
    }

    // ── default_age_histogram tests ───────────────────────────────────
    #[test]
    fn test_default_age_histogram_returns_six_buckets() {
        let histogram = default_age_histogram();
        assert_eq!(histogram.len(), 6);
    }

    #[test]
    fn test_default_age_histogram_all_zero() {
        let histogram = default_age_histogram();
        for bucket in &histogram {
            assert_eq!(bucket.count, 0, "bucket '{}' should be 0", bucket.label);
        }
    }

    #[test]
    fn test_default_age_histogram_labels() {
        let histogram = default_age_histogram();
        let expected_labels = [
            "<1 week",
            "1-4 weeks",
            "1-3 months",
            "3-6 months",
            "6-12 months",
            ">1 year",
        ];
        for (bucket, expected) in histogram.iter().zip(expected_labels.iter()) {
            assert_eq!(bucket.label, *expected);
        }
    }

    // ── build_age_histogram boundary value tests ──────────────────────
    #[test]
    fn test_build_age_histogram_boundary_day_6_is_first_bucket() {
        // 6 days should be in "<1 week" (index 0)
        let blame = BlameResult {
            entries: vec![make_blame_entry(6)],
            total: 1,
            avg_age_days: 6,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[0].count, 1, "6 days should be in <1 week bucket");
        assert_eq!(histogram[1].count, 0);
    }

    #[test]
    fn test_build_age_histogram_boundary_exactly_7_days() {
        // Exactly 7 days should be in "1-4 weeks" (index 1), since condition is days < 7
        let blame = BlameResult {
            entries: vec![make_blame_entry(7)],
            total: 1,
            avg_age_days: 7,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[0].count, 0, "7 days should NOT be in <1 week");
        assert_eq!(histogram[1].count, 1, "7 days should be in 1-4 weeks");
    }

    #[test]
    fn test_build_age_histogram_boundary_exactly_28_days() {
        // Exactly 28 days: condition is days < 28 for bucket 1, so 28 goes to bucket 2
        let blame = BlameResult {
            entries: vec![make_blame_entry(28)],
            total: 1,
            avg_age_days: 28,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[1].count, 0, "28 days should NOT be in 1-4 weeks");
        assert_eq!(histogram[2].count, 1, "28 days should be in 1-3 months");
    }

    #[test]
    fn test_build_age_histogram_boundary_day_27_in_second_bucket() {
        let blame = BlameResult {
            entries: vec![make_blame_entry(27)],
            total: 1,
            avg_age_days: 27,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[1].count, 1, "27 days should be in 1-4 weeks");
    }

    #[test]
    fn test_build_age_histogram_boundary_exactly_90_days() {
        // 90 days: condition is days < 90 for bucket 2, so 90 goes to bucket 3
        let blame = BlameResult {
            entries: vec![make_blame_entry(90)],
            total: 1,
            avg_age_days: 90,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[2].count, 0, "90 days should NOT be in 1-3 months");
        assert_eq!(histogram[3].count, 1, "90 days should be in 3-6 months");
    }

    #[test]
    fn test_build_age_histogram_boundary_day_89_in_third_bucket() {
        let blame = BlameResult {
            entries: vec![make_blame_entry(89)],
            total: 1,
            avg_age_days: 89,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[2].count, 1, "89 days should be in 1-3 months");
    }

    #[test]
    fn test_build_age_histogram_boundary_exactly_180_days() {
        // 180 days: condition is days < 180 for bucket 3, so 180 goes to bucket 4
        let blame = BlameResult {
            entries: vec![make_blame_entry(180)],
            total: 1,
            avg_age_days: 180,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(
            histogram[3].count, 0,
            "180 days should NOT be in 3-6 months"
        );
        assert_eq!(histogram[4].count, 1, "180 days should be in 6-12 months");
    }

    #[test]
    fn test_build_age_histogram_boundary_day_179_in_fourth_bucket() {
        let blame = BlameResult {
            entries: vec![make_blame_entry(179)],
            total: 1,
            avg_age_days: 179,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[3].count, 1, "179 days should be in 3-6 months");
    }

    #[test]
    fn test_build_age_histogram_boundary_exactly_365_days() {
        // 365 days: condition is days < 365 for bucket 4, so 365 goes to bucket 5
        let blame = BlameResult {
            entries: vec![make_blame_entry(365)],
            total: 1,
            avg_age_days: 365,
            stale_count: 1,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(
            histogram[4].count, 0,
            "365 days should NOT be in 6-12 months"
        );
        assert_eq!(histogram[5].count, 1, "365 days should be in >1 year");
    }

    #[test]
    fn test_build_age_histogram_boundary_day_364_in_fifth_bucket() {
        let blame = BlameResult {
            entries: vec![make_blame_entry(364)],
            total: 1,
            avg_age_days: 364,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[4].count, 1, "364 days should be in 6-12 months");
    }

    #[test]
    fn test_build_age_histogram_day_zero() {
        let blame = BlameResult {
            entries: vec![make_blame_entry(0)],
            total: 1,
            avg_age_days: 0,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[0].count, 1, "0 days should be in <1 week");
    }

    #[test]
    fn test_build_age_histogram_very_old_item() {
        let blame = BlameResult {
            entries: vec![make_blame_entry(3650)],
            total: 1,
            avg_age_days: 3650,
            stale_count: 1,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[5].count, 1, "3650 days should be in >1 year");
        for bucket in &histogram[..5] {
            assert_eq!(bucket.count, 0);
        }
    }

    #[test]
    fn test_build_age_histogram_multiple_in_same_bucket() {
        let entries = vec![
            make_blame_entry(1),
            make_blame_entry(2),
            make_blame_entry(5),
        ];
        let blame = BlameResult {
            entries,
            total: 3,
            avg_age_days: 2,
            stale_count: 0,
            stale_threshold_days: 365,
        };
        let histogram = build_age_histogram(&blame);
        assert_eq!(histogram[0].count, 3, "all 3 should be in <1 week");
        for bucket in &histogram[1..] {
            assert_eq!(bucket.count, 0);
        }
    }

    // ── select_sample_indices edge case tests ─────────────────────────
    #[test]
    fn test_select_sample_indices_total_one_sample_one() {
        let indices = select_sample_indices(1, 1);
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn test_select_sample_indices_total_two_sample_one() {
        let indices = select_sample_indices(2, 1);
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn test_select_sample_indices_total_two_sample_two() {
        let indices = select_sample_indices(2, 2);
        assert_eq!(indices, vec![0, 1]);
    }

    #[test]
    fn test_select_sample_indices_large_values() {
        let indices = select_sample_indices(1000, 5);
        assert_eq!(indices.len(), 5);
        // First should be 0, last should be 999
        assert_eq!(indices[0], 0);
        assert_eq!(indices[4], 999);
        // All indices must be within range
        for &idx in &indices {
            assert!(idx < 1000);
        }
        // Indices should be sorted (ascending)
        for window in indices.windows(2) {
            assert!(
                window[0] < window[1],
                "indices should be strictly increasing"
            );
        }
    }

    #[test]
    fn test_select_sample_indices_large_sample_exceeds_total() {
        let indices = select_sample_indices(3, 100);
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_select_sample_indices_both_zero() {
        assert!(select_sample_indices(0, 0).is_empty());
    }

    #[test]
    fn test_select_sample_indices_total_one_sample_zero() {
        assert!(select_sample_indices(1, 0).is_empty());
    }

    #[test]
    fn test_select_sample_indices_total_zero_sample_zero() {
        assert!(select_sample_indices(0, 0).is_empty());
    }

    #[test]
    fn test_select_sample_indices_two_from_ten() {
        let indices = select_sample_indices(10, 2);
        assert_eq!(indices, vec![0, 9]);
    }

    #[test]
    fn test_select_sample_indices_four_from_ten() {
        let indices = select_sample_indices(10, 4);
        assert_eq!(indices.len(), 4);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[3], 9);
        // Evenly spaced: step = 9/3 = 3.0 → 0, 3, 6, 9
        assert_eq!(indices, vec![0, 3, 6, 9]);
    }

    // ── compute_report fallback path tests ────────────────────────────
    #[test]
    fn test_compute_report_empty_scan_no_history() {
        // Use a temp dir (not a git repo) so blame fails and exercises the
        // Err(_) => (default_age_histogram(), 0, 0) fallback on line 36.
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let scan = ScanResult {
            items: vec![],
            files_scanned: 0,
            ignored_items: vec![],
        };

        let result = compute_report(&scan, tmp.path(), &config, 0, 365).unwrap();

        // Summary should be all zeros
        assert_eq!(result.summary.total_items, 0);
        assert_eq!(result.summary.total_files, 0);
        assert_eq!(result.summary.files_scanned, 0);
        assert_eq!(result.summary.urgent_count, 0);
        assert_eq!(result.summary.high_count, 0);
        assert_eq!(result.summary.stale_count, 0);
        assert_eq!(result.summary.avg_age_days, 0);

        // History should be empty (history_count=0 bypasses it)
        assert!(result.history.is_empty());

        // Age histogram should be the default (6 buckets, all 0)
        assert_eq!(result.age_histogram.len(), 6);
        for bucket in &result.age_histogram {
            assert_eq!(bucket.count, 0);
        }

        // Items should be empty
        assert!(result.items.is_empty());

        // generated_at should be non-empty ISO 8601 string
        assert!(!result.generated_at.is_empty());
    }

    #[test]
    fn test_compute_report_with_items_blame_fails() {
        // Non-git dir with items: blame fails, fallback values used
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let scan = ScanResult {
            items: vec![
                TodoItem {
                    file: "foo.rs".to_string(),
                    line: 10,
                    tag: Tag::Todo,
                    message: "implement this".to_string(),
                    author: Some("alice".to_string()),
                    issue_ref: None,
                    priority: Priority::Normal,
                    deadline: None,
                },
                TodoItem {
                    file: "bar.rs".to_string(),
                    line: 20,
                    tag: Tag::Fixme,
                    message: "urgent fix".to_string(),
                    author: None,
                    issue_ref: Some("#123".to_string()),
                    priority: Priority::Urgent,
                    deadline: None,
                },
                TodoItem {
                    file: "foo.rs".to_string(),
                    line: 30,
                    tag: Tag::Hack,
                    message: "workaround".to_string(),
                    author: None,
                    issue_ref: None,
                    priority: Priority::High,
                    deadline: None,
                },
            ],
            files_scanned: 5,
            ignored_items: vec![],
        };

        let result = compute_report(&scan, tmp.path(), &config, 0, 365).unwrap();

        // Stats should reflect the items
        assert_eq!(result.summary.total_items, 3);
        assert_eq!(result.summary.total_files, 2);
        assert_eq!(result.summary.files_scanned, 5);
        assert_eq!(result.summary.urgent_count, 1);
        assert_eq!(result.summary.high_count, 1);

        // Blame-derived values should be fallback zeros
        assert_eq!(result.summary.stale_count, 0);
        assert_eq!(result.summary.avg_age_days, 0);

        // Age histogram should be default (all zeros)
        assert_eq!(result.age_histogram.len(), 6);
        for bucket in &result.age_histogram {
            assert_eq!(bucket.count, 0);
        }

        // Items should be passed through
        assert_eq!(result.items.len(), 3);

        // Tag counts should be present
        assert!(!result.tag_counts.is_empty());

        // History is empty since history_count=0
        assert!(result.history.is_empty());
    }

    #[test]
    fn test_compute_report_history_count_positive_non_git() {
        // With history_count > 0 in a non-git dir, compute_history should
        // return an error that gets unwrap_or_default'd to empty vec.
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let scan = ScanResult {
            items: vec![],
            files_scanned: 0,
            ignored_items: vec![],
        };

        let result = compute_report(&scan, tmp.path(), &config, 5, 365).unwrap();

        // History should be empty because git commands fail in non-git dir
        assert!(result.history.is_empty());
    }

    #[test]
    fn test_compute_history_non_git_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let result = compute_history(tmp.path(), &config, 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_history_empty_repo_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        let config = Config::default();
        let result = compute_history(dir.path(), &config, 5);
        // Either an error or empty vec (no commits)
        assert!(result.is_err() || result.unwrap().is_empty());
    }
}
