use std::path::Path;

use anyhow::Result;
use regex::Regex;

use crate::blame::compute_blame;
use crate::config::Config;
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

    let generated_at = now_iso8601();

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

            count += scan_content(&content, file_path, &pattern).len();
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

/// Current UTC time as ISO-8601 string.
fn now_iso8601() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = now / 86400;
    let (year, month, day) = days_to_ymd(days as i64);
    let secs_today = now % 86400;
    let hour = secs_today / 3600;
    let minute = (secs_today % 3600) / 60;
    let second = secs_today % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// Convert days since epoch to (year, month, day).
/// Same algorithm as blame.rs.
fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
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

    #[test]
    fn test_now_iso8601_format() {
        let ts = now_iso8601();
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20);
        assert!(ts.contains('T'));
    }
}
