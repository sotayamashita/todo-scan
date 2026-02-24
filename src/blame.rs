use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::date_utils;
use crate::git::git_command;
use crate::model::{BlameEntry, BlameInfo, BlameResult, ScanResult, TodoItem};

#[derive(Debug, Clone)]
pub struct RawBlameData {
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub commit: String,
}

/// Parse `git blame --porcelain` output into a map of line number -> blame data.
pub fn parse_porcelain_blame(output: &str) -> HashMap<usize, RawBlameData> {
    let mut result = HashMap::new();
    let mut current_line: Option<usize> = None;
    let mut current_commit = String::new();
    let mut current_author = String::new();
    let mut current_email = String::new();
    let mut current_timestamp: i64 = 0;

    for line in output.lines() {
        // Header line: <hash> <orig-line> <final-line> [<num-lines>]
        if line.len() >= 40 && line.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                current_commit = parts[0][..8].to_string();
                if let Ok(ln) = parts[2].parse::<usize>() {
                    current_line = Some(ln);
                }
            }
        } else if let Some(stripped) = line.strip_prefix("author ") {
            current_author = stripped.to_string();
        } else if let Some(stripped) = line.strip_prefix("author-mail ") {
            current_email = stripped.trim_matches(|c| c == '<' || c == '>').to_string();
        } else if let Some(stripped) = line.strip_prefix("author-time ") {
            current_timestamp = stripped.parse::<i64>().unwrap_or(0);
        } else if line.starts_with('\t') {
            // Content line marks the end of a blame entry
            if let Some(ln) = current_line.take() {
                result.insert(
                    ln,
                    RawBlameData {
                        author: current_author.clone(),
                        email: current_email.clone(),
                        timestamp: current_timestamp,
                        commit: current_commit.clone(),
                    },
                );
            }
        }
    }

    result
}

/// Run `git blame --porcelain` on a file and return parsed blame data.
pub fn blame_file(file_path: &str, root: &Path) -> Result<HashMap<usize, RawBlameData>> {
    let output = git_command(&["blame", "--porcelain", "--", file_path], root)?;
    Ok(parse_porcelain_blame(&output))
}

/// Convert a unix timestamp to a "YYYY-MM-DD" date string.
pub fn timestamp_to_date_string(timestamp: i64) -> String {
    // Manual conversion without external date library
    let days_since_epoch = timestamp / 86400;
    let (year, month, day) = date_utils::days_to_ymd(days_since_epoch);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Compute age in days from a unix timestamp to now.
pub fn compute_age_days(timestamp: i64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let diff = now - timestamp;
    if diff < 0 {
        return 0;
    }
    (diff / 86400) as u64
}

/// Parse a duration string like "90d" or "365" into days.
pub fn parse_duration_days(s: &str) -> Result<u64> {
    let s = s.trim();
    let numeric = s.strip_suffix('d').unwrap_or(s);
    numeric
        .parse::<u64>()
        .with_context(|| format!("invalid duration: {}", s))
}

/// Build blame entries for all TODO items in a scan result.
pub fn compute_blame(
    scan: &ScanResult,
    root: &Path,
    stale_threshold_days: u64,
) -> Result<BlameResult> {
    // Group items by file
    let mut by_file: HashMap<&str, Vec<&TodoItem>> = HashMap::new();
    for item in &scan.items {
        by_file.entry(&item.file).or_default().push(item);
    }

    let mut entries: Vec<BlameEntry> = Vec::new();

    for (file, items) in &by_file {
        let blame_data = match blame_file(file, root) {
            Ok(data) => data,
            Err(_) => continue, // Skip files not tracked by git
        };

        for item in items {
            let raw = blame_data.get(&item.line);
            let blame_info = match raw {
                Some(raw) => {
                    let age_days = compute_age_days(raw.timestamp);
                    BlameInfo {
                        author: raw.author.clone(),
                        email: raw.email.clone(),
                        date: timestamp_to_date_string(raw.timestamp),
                        age_days,
                        commit: raw.commit.clone(),
                    }
                }
                None => BlameInfo {
                    author: "Unknown".to_string(),
                    email: String::new(),
                    date: String::new(),
                    age_days: 0,
                    commit: String::new(),
                },
            };

            let stale = blame_info.age_days >= stale_threshold_days;

            entries.push(BlameEntry {
                item: (*item).clone(),
                blame: blame_info,
                stale,
            });
        }
    }

    // Sort by file/line by default
    entries.sort_by(|a, b| {
        a.item
            .file
            .cmp(&b.item.file)
            .then(a.item.line.cmp(&b.item.line))
    });

    let total = entries.len();
    let stale_count = entries.iter().filter(|e| e.stale).count();
    let avg_age_days = if total > 0 {
        entries.iter().map(|e| e.blame.age_days).sum::<u64>() / total as u64
    } else {
        0
    };

    Ok(BlameResult {
        entries,
        total,
        avg_age_days,
        stale_count,
        stale_threshold_days,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_days_with_suffix() {
        assert_eq!(parse_duration_days("90d").unwrap(), 90);
    }

    #[test]
    fn test_parse_duration_days_bare_number() {
        assert_eq!(parse_duration_days("365").unwrap(), 365);
    }

    #[test]
    fn test_parse_duration_days_invalid() {
        assert!(parse_duration_days("abc").is_err());
    }

    #[test]
    fn test_timestamp_to_date_string() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        assert_eq!(timestamp_to_date_string(1704067200), "2024-01-01");
    }

    #[test]
    fn test_compute_age_days() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        // 30 days ago
        let thirty_days_ago = now - (30 * 86400);
        let age = compute_age_days(thirty_days_ago);
        assert!((29..=31).contains(&age));
    }

    #[test]
    fn test_parse_porcelain_blame_single_line() {
        let output = "\
abc1234567890123456789012345678901234567 1 1 1
author John Doe
author-mail <john@example.com>
author-time 1704067200
author-tz +0000
committer John Doe
committer-mail <john@example.com>
committer-time 1704067200
committer-tz +0000
summary initial commit
filename test.rs
\t// TODO: test line
";
        let result = parse_porcelain_blame(output);
        assert_eq!(result.len(), 1);
        let data = result.get(&1).unwrap();
        assert_eq!(data.author, "John Doe");
        assert_eq!(data.email, "john@example.com");
        assert_eq!(data.timestamp, 1704067200);
        assert_eq!(data.commit, "abc12345");
    }

    #[test]
    fn test_parse_porcelain_blame_multiple_lines() {
        let output = "\
abc1234567890123456789012345678901234567 1 1 2
author Alice
author-mail <alice@test.com>
author-time 1704067200
author-tz +0000
committer Alice
committer-mail <alice@test.com>
committer-time 1704067200
committer-tz +0000
summary first commit
filename test.rs
\tline one
def4567890123456789012345678901234567890 2 2
author Bob
author-mail <bob@test.com>
author-time 1704153600
author-tz +0000
committer Bob
committer-mail <bob@test.com>
committer-time 1704153600
committer-tz +0000
summary second commit
filename test.rs
\tline two
";
        let result = parse_porcelain_blame(output);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(&1).unwrap().author, "Alice");
        assert_eq!(result.get(&2).unwrap().author, "Bob");
    }

    #[test]
    fn test_parse_porcelain_blame_empty() {
        let result = parse_porcelain_blame("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_porcelain_blame_uncommitted() {
        let output = "\
0000000000000000000000000000000000000000 1 1 1
author Not Committed Yet
author-mail <not.committed.yet>
author-time 1704067200
author-tz +0000
committer Not Committed Yet
committer-mail <not.committed.yet>
committer-time 1704067200
committer-tz +0000
summary
filename test.rs
\t// TODO: uncommitted line
";
        let result = parse_porcelain_blame(output);
        assert_eq!(result.len(), 1);
        let data = result.get(&1).unwrap();
        assert_eq!(data.author, "Not Committed Yet");
    }

    #[test]
    fn test_compute_age_days_future_timestamp() {
        // A timestamp far in the future should return 0
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let future_ts = now + (365 * 86400); // 1 year in the future
        let age = compute_age_days(future_ts);
        assert_eq!(age, 0);
    }

    #[test]
    fn test_parse_porcelain_blame_short_header() {
        // A 40-char hex line with fewer than 3 space-separated parts
        // should be skipped (parts.len() >= 3 check)
        let output = "\
abc1234567890123456789012345678901234567 1
author Alice
author-mail <alice@test.com>
author-time 1704067200
author-tz +0000
committer Alice
committer-mail <alice@test.com>
committer-time 1704067200
committer-tz +0000
summary commit
filename test.rs
\tsome content
";
        let result = parse_porcelain_blame(output);
        // The header only has 2 parts so current_line is never set,
        // meaning no entry is inserted despite the tab content line.
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_porcelain_blame_non_parseable_line_number() {
        // The third part is not a valid number, so parse::<usize>() fails
        let output = "\
abc1234567890123456789012345678901234567 1 notanumber 1
author Bob
author-mail <bob@test.com>
author-time 1704067200
author-tz +0000
committer Bob
committer-mail <bob@test.com>
committer-time 1704067200
committer-tz +0000
summary commit
filename test.rs
\tsome content
";
        let result = parse_porcelain_blame(output);
        // parts[2] can't be parsed as usize, so current_line stays None
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_porcelain_blame_unparseable_author_time() {
        // author-time has a non-numeric value -> unwrap_or(0)
        let output = "\
abc1234567890123456789012345678901234567 1 1 1
author Carol
author-mail <carol@test.com>
author-time not_a_timestamp
author-tz +0000
committer Carol
committer-mail <carol@test.com>
committer-time 1704067200
committer-tz +0000
summary commit
filename test.rs
\tsome line
";
        let result = parse_porcelain_blame(output);
        assert_eq!(result.len(), 1);
        let data = result.get(&1).unwrap();
        assert_eq!(data.author, "Carol");
        assert_eq!(data.timestamp, 0); // Falls back to 0
    }

    #[test]
    fn test_timestamp_to_date_string_epoch() {
        // Unix epoch: timestamp 0 should produce 1970-01-01
        assert_eq!(timestamp_to_date_string(0), "1970-01-01");
    }
}
