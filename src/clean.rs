use std::collections::HashMap;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use crate::blame::parse_duration_days;
use crate::config::Config;
use crate::date_utils;
use crate::model::{CleanResult, CleanViolation, ScanResult, TodoItem};

static ISO8601_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})").unwrap());

static ISSUE_NUMBER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#(\d+)$").unwrap());

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueState {
    Open,
    Closed { closed_at: Option<i64> },
}

pub trait IssueChecker {
    fn check_issue(&self, issue_number: u32) -> Result<Option<IssueState>>;
}

pub struct GhIssueChecker {
    cache: std::cell::RefCell<HashMap<u32, Option<IssueState>>>,
}

impl GhIssueChecker {
    pub fn new() -> Option<Self> {
        // Check if gh CLI is available
        let output = std::process::Command::new("gh")
            .arg("--version")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(Self {
            cache: std::cell::RefCell::new(HashMap::new()),
        })
    }
}

impl IssueChecker for GhIssueChecker {
    fn check_issue(&self, issue_number: u32) -> Result<Option<IssueState>> {
        // Check cache first
        if let Some(cached) = self.cache.borrow().get(&issue_number) {
            return Ok(cached.clone());
        }

        let output = std::process::Command::new("gh")
            .args([
                "issue",
                "view",
                &issue_number.to_string(),
                "--json",
                "state,closedAt",
            ])
            .output();

        let result = match output {
            Ok(out) if out.status.success() => {
                let json: serde_json::Value =
                    serde_json::from_slice(&out.stdout).unwrap_or_default();
                let state_str = json["state"].as_str().unwrap_or("OPEN");
                if state_str == "CLOSED" {
                    let closed_at = json["closedAt"].as_str().and_then(parse_iso8601_timestamp);
                    Some(IssueState::Closed { closed_at })
                } else {
                    Some(IssueState::Open)
                }
            }
            _ => {
                // gh command failed (auth issue, network, etc.) — skip this issue
                None
            }
        };

        self.cache.borrow_mut().insert(issue_number, result.clone());
        Ok(result)
    }
}

/// Parse an ISO 8601 timestamp string into a Unix timestamp.
fn parse_iso8601_timestamp(s: &str) -> Option<i64> {
    let caps = ISO8601_RE.captures(s)?;

    let year: i64 = caps[1].parse().ok()?;
    let month: u32 = caps[2].parse().ok()?;
    let day: u32 = caps[3].parse().ok()?;
    let hour: i64 = caps[4].parse().ok()?;
    let min: i64 = caps[5].parse().ok()?;
    let sec: i64 = caps[6].parse().ok()?;

    // Simple days-from-epoch calculation
    let days = date_utils::ymd_to_days(year, month, day);
    Some(days * 86400 + hour * 3600 + min * 60 + sec)
}

/// Normalize a TODO message for duplicate comparison.
fn normalize_message(msg: &str) -> String {
    msg.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract GitHub issue number from an issue ref like "#123".
fn extract_issue_number(issue_ref: &str) -> Option<u32> {
    let caps = ISSUE_NUMBER_RE.captures(issue_ref.trim())?;
    caps[1].parse().ok()
}

/// Run clean analysis on scan results.
pub fn run_clean(
    scan: &ScanResult,
    config: &Config,
    issue_checker: Option<&dyn IssueChecker>,
    since_cli: Option<&str>,
) -> CleanResult {
    let mut violations = Vec::new();

    let enable_stale = config.clean.stale_issues.unwrap_or(true);
    let enable_duplicates = config.clean.duplicates.unwrap_or(true);

    // Resolve since: CLI > config
    let since_str = since_cli.or(config.clean.since.as_deref());
    let since_days = since_str.and_then(|s| parse_duration_days(s).ok());

    // Phase 1: Stale issue detection
    if enable_stale {
        if let Some(checker) = issue_checker {
            detect_stale_issues(&scan.items, checker, since_days, &mut violations);
        }
    }

    // Phase 2: Duplicate detection
    if enable_duplicates {
        detect_duplicates(&scan.items, &mut violations);
    }

    // Sort by file, then line
    violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

    let stale_count = violations
        .iter()
        .filter(|v| v.rule == "stale_issue")
        .count();
    let duplicate_count = violations.iter().filter(|v| v.rule == "duplicate").count();

    CleanResult {
        passed: violations.is_empty(),
        total_items: scan.items.len(),
        stale_count,
        duplicate_count,
        violations,
    }
}

fn detect_stale_issues(
    items: &[TodoItem],
    checker: &dyn IssueChecker,
    since_days: Option<u64>,
    violations: &mut Vec<CleanViolation>,
) {
    // Collect unique issue numbers first
    let mut issue_items: Vec<(&TodoItem, u32)> = Vec::new();
    for item in items {
        if let Some(ref issue_ref) = item.issue_ref {
            if let Some(num) = extract_issue_number(issue_ref) {
                issue_items.push((item, num));
            }
            // Skip JIRA-style refs — gh can't query them
        }
    }

    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for (item, issue_num) in &issue_items {
        let state = match checker.check_issue(*issue_num) {
            Ok(Some(state)) => state,
            _ => continue, // Skip if we can't determine state
        };

        if let IssueState::Closed { closed_at } = state {
            // Apply since filter if configured
            if let Some(days) = since_days {
                if let Some(closed_ts) = closed_at {
                    let age_secs = now_ts - closed_ts;
                    let age_days = if age_secs > 0 {
                        age_secs as u64 / 86400
                    } else {
                        0
                    };
                    if age_days < days {
                        continue; // Issue was closed recently, skip
                    }
                }
                // If no closed_at timestamp, still flag it
            }

            violations.push(CleanViolation {
                rule: "stale_issue".to_string(),
                message: format!("Issue #{} is closed", issue_num),
                file: item.file.clone(),
                line: item.line,
                issue_ref: item.issue_ref.clone(),
                duplicate_of: None,
            });
        }
    }
}

fn detect_duplicates(items: &[TodoItem], violations: &mut Vec<CleanViolation>) {
    // Group by normalized message
    let mut groups: HashMap<String, Vec<&TodoItem>> = HashMap::new();

    for item in items {
        let normalized = normalize_message(&item.message);
        if normalized.is_empty() {
            continue; // Skip empty messages
        }
        groups.entry(normalized).or_default().push(item);
    }

    for group in groups.values() {
        if group.len() < 2 {
            continue;
        }

        // The first occurrence is the "original", rest are duplicates
        let original = group[0];
        let original_loc = format!("{}:{}", original.file, original.line);

        for dup in &group[1..] {
            violations.push(CleanViolation {
                rule: "duplicate".to_string(),
                message: format!("Duplicate TODO: \"{}\"", dup.message.trim()),
                file: dup.file.clone(),
                line: dup.line,
                issue_ref: None,
                duplicate_of: Some(original_loc.clone()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Priority, Tag};

    struct MockIssueChecker {
        issues: HashMap<u32, Option<IssueState>>,
    }

    impl MockIssueChecker {
        fn new(issues: Vec<(u32, Option<IssueState>)>) -> Self {
            Self {
                issues: issues.into_iter().collect(),
            }
        }
    }

    impl IssueChecker for MockIssueChecker {
        fn check_issue(&self, issue_number: u32) -> Result<Option<IssueState>> {
            Ok(self.issues.get(&issue_number).cloned().unwrap_or(None))
        }
    }

    use crate::test_helpers::helpers::make_item;

    fn make_item_with_issue(
        file: &str,
        line: usize,
        tag: Tag,
        message: &str,
        issue_ref: &str,
    ) -> TodoItem {
        TodoItem {
            file: file.to_string(),
            line,
            tag,
            message: message.to_string(),
            author: None,
            issue_ref: Some(issue_ref.to_string()),
            priority: Priority::Normal,
            deadline: None,
        }
    }

    fn default_config() -> Config {
        Config::default()
    }

    // --- Stale issue detection ---

    #[test]
    fn test_stale_closed_issue_detected() {
        let scan = ScanResult {
            items: vec![make_item_with_issue(
                "a.rs",
                1,
                Tag::Todo,
                "fix bug #42",
                "#42",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker =
            MockIssueChecker::new(vec![(42, Some(IssueState::Closed { closed_at: None }))]);
        let result = run_clean(&scan, &default_config(), Some(&checker), None);
        assert!(!result.passed);
        assert_eq!(result.stale_count, 1);
        assert_eq!(result.violations[0].rule, "stale_issue");
        assert!(result.violations[0].message.contains("#42"));
    }

    #[test]
    fn test_open_issue_not_flagged() {
        let scan = ScanResult {
            items: vec![make_item_with_issue(
                "a.rs",
                1,
                Tag::Todo,
                "fix bug #42",
                "#42",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker = MockIssueChecker::new(vec![(42, Some(IssueState::Open))]);
        let result = run_clean(&scan, &default_config(), Some(&checker), None);
        assert!(result.passed);
        assert_eq!(result.stale_count, 0);
    }

    #[test]
    fn test_since_filter_skips_recently_closed() {
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Closed 5 days ago
        let closed_ts = now_ts - (5 * 86400);

        let scan = ScanResult {
            items: vec![make_item_with_issue(
                "a.rs",
                1,
                Tag::Todo,
                "fix bug #42",
                "#42",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker = MockIssueChecker::new(vec![(
            42,
            Some(IssueState::Closed {
                closed_at: Some(closed_ts),
            }),
        )]);

        // Since 30 days — closed 5 days ago should NOT be flagged
        let result = run_clean(&scan, &default_config(), Some(&checker), Some("30d"));
        assert!(result.passed);
        assert_eq!(result.stale_count, 0);
    }

    #[test]
    fn test_since_filter_flags_old_closed() {
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Closed 60 days ago
        let closed_ts = now_ts - (60 * 86400);

        let scan = ScanResult {
            items: vec![make_item_with_issue(
                "a.rs",
                1,
                Tag::Todo,
                "fix bug #42",
                "#42",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker = MockIssueChecker::new(vec![(
            42,
            Some(IssueState::Closed {
                closed_at: Some(closed_ts),
            }),
        )]);

        // Since 30 days — closed 60 days ago SHOULD be flagged
        let result = run_clean(&scan, &default_config(), Some(&checker), Some("30d"));
        assert!(!result.passed);
        assert_eq!(result.stale_count, 1);
    }

    #[test]
    fn test_jira_refs_not_checked() {
        let mut item = make_item("a.rs", 1, Tag::Todo, "fix JIRA-456");
        item.issue_ref = Some("JIRA-456".to_string());

        let scan = ScanResult {
            items: vec![item],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker = MockIssueChecker::new(vec![]);
        let result = run_clean(&scan, &default_config(), Some(&checker), None);
        assert!(result.passed);
        assert_eq!(result.stale_count, 0);
    }

    #[test]
    fn test_no_issue_checker_skips_stale() {
        let scan = ScanResult {
            items: vec![make_item_with_issue(
                "a.rs",
                1,
                Tag::Todo,
                "fix bug #42",
                "#42",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let result = run_clean(&scan, &default_config(), None, None);
        assert!(result.passed);
        assert_eq!(result.stale_count, 0);
    }

    // --- Duplicate detection ---

    #[test]
    fn test_duplicate_exact_match() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Todo, "implement feature"),
                make_item("b.rs", 5, Tag::Todo, "implement feature"),
            ],
            files_scanned: 2,
            ignored_items: vec![],
        };
        let result = run_clean(&scan, &default_config(), None, None);
        assert!(!result.passed);
        assert_eq!(result.duplicate_count, 1);
        assert_eq!(result.violations[0].rule, "duplicate");
    }

    #[test]
    fn test_duplicate_case_insensitive() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Todo, "Implement Feature"),
                make_item("b.rs", 5, Tag::Todo, "implement feature"),
            ],
            files_scanned: 2,
            ignored_items: vec![],
        };
        let result = run_clean(&scan, &default_config(), None, None);
        assert!(!result.passed);
        assert_eq!(result.duplicate_count, 1);
    }

    #[test]
    fn test_duplicate_whitespace_normalization() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Todo, "implement   feature"),
                make_item("b.rs", 5, Tag::Todo, "implement feature"),
            ],
            files_scanned: 2,
            ignored_items: vec![],
        };
        let result = run_clean(&scan, &default_config(), None, None);
        assert!(!result.passed);
        assert_eq!(result.duplicate_count, 1);
    }

    #[test]
    fn test_different_messages_no_duplicate() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Todo, "implement feature A"),
                make_item("b.rs", 5, Tag::Todo, "implement feature B"),
            ],
            files_scanned: 2,
            ignored_items: vec![],
        };
        let result = run_clean(&scan, &default_config(), None, None);
        assert!(result.passed);
        assert_eq!(result.duplicate_count, 0);
    }

    #[test]
    fn test_empty_messages_not_duplicated() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Todo, ""),
                make_item("b.rs", 5, Tag::Todo, ""),
            ],
            files_scanned: 2,
            ignored_items: vec![],
        };
        let result = run_clean(&scan, &default_config(), None, None);
        assert!(result.passed);
        assert_eq!(result.duplicate_count, 0);
    }

    #[test]
    fn test_passed_when_no_violations() {
        let scan = ScanResult {
            items: vec![make_item("a.rs", 1, Tag::Todo, "unique message")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let result = run_clean(&scan, &default_config(), None, None);
        assert!(result.passed);
        assert_eq!(result.stale_count, 0);
        assert_eq!(result.duplicate_count, 0);
    }

    // --- Helper tests ---

    #[test]
    fn test_normalize_message() {
        assert_eq!(normalize_message("  Hello  World  "), "hello world");
        assert_eq!(normalize_message("UPPER"), "upper");
        assert_eq!(normalize_message("a\tb"), "a b");
    }

    #[test]
    fn test_extract_issue_number() {
        assert_eq!(extract_issue_number("#123"), Some(123));
        assert_eq!(extract_issue_number("#1"), Some(1));
        assert_eq!(extract_issue_number("JIRA-456"), None);
        assert_eq!(extract_issue_number("no ref"), None);
    }

    #[test]
    fn test_parse_iso8601_timestamp() {
        let ts = parse_iso8601_timestamp("2024-01-01T00:00:00Z");
        assert!(ts.is_some());
        // 2024-01-01 00:00:00 UTC = 1704067200
        assert_eq!(ts.unwrap(), 1704067200);
    }

    #[test]
    fn test_config_disables_stale() {
        let scan = ScanResult {
            items: vec![make_item_with_issue("a.rs", 1, Tag::Todo, "fix #42", "#42")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker =
            MockIssueChecker::new(vec![(42, Some(IssueState::Closed { closed_at: None }))]);
        let mut config = default_config();
        config.clean.stale_issues = Some(false);
        let result = run_clean(&scan, &config, Some(&checker), None);
        assert!(result.passed);
    }

    #[test]
    fn test_config_disables_duplicates() {
        let scan = ScanResult {
            items: vec![
                make_item("a.rs", 1, Tag::Todo, "same message"),
                make_item("b.rs", 2, Tag::Todo, "same message"),
            ],
            files_scanned: 2,
            ignored_items: vec![],
        };
        let mut config = default_config();
        config.clean.duplicates = Some(false);
        let result = run_clean(&scan, &config, None, None);
        assert!(result.passed);
    }

    // --- parse_iso8601_timestamp edge cases ---

    #[test]
    fn test_parse_iso8601_timestamp_empty_string() {
        assert_eq!(parse_iso8601_timestamp(""), None);
    }

    #[test]
    fn test_parse_iso8601_timestamp_no_match() {
        assert_eq!(parse_iso8601_timestamp("not a date at all"), None);
    }

    #[test]
    fn test_parse_iso8601_timestamp_partial_date() {
        // Only date portion, missing time components
        assert_eq!(parse_iso8601_timestamp("2024-01-15"), None);
    }

    // --- since_days with closed_at=None ---

    #[test]
    fn test_since_filter_flags_closed_with_no_timestamp() {
        // When since_days is set but closed_at is None, the issue should
        // still be flagged (line 206: "If no closed_at timestamp, still flag it")
        let scan = ScanResult {
            items: vec![make_item_with_issue(
                "a.rs",
                1,
                Tag::Todo,
                "fix bug #42",
                "#42",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker =
            MockIssueChecker::new(vec![(42, Some(IssueState::Closed { closed_at: None }))]);

        // Since 30 days — but closed_at is None, so it should still be flagged
        let result = run_clean(&scan, &default_config(), Some(&checker), Some("30d"));
        assert!(!result.passed);
        assert_eq!(result.stale_count, 1);
        assert!(result.violations[0].message.contains("#42"));
    }

    // --- run_clean with since from config ---

    #[test]
    fn test_run_clean_since_from_config() {
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Closed 5 days ago
        let closed_ts = now_ts - (5 * 86400);

        let scan = ScanResult {
            items: vec![make_item_with_issue(
                "a.rs",
                1,
                Tag::Todo,
                "fix bug #42",
                "#42",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker = MockIssueChecker::new(vec![(
            42,
            Some(IssueState::Closed {
                closed_at: Some(closed_ts),
            }),
        )]);

        // Set since in config (not CLI), 30 days — closed 5 days ago should NOT be flagged
        let mut config = default_config();
        config.clean.since = Some("30d".to_string());
        let result = run_clean(&scan, &config, Some(&checker), None);
        assert!(result.passed);
        assert_eq!(result.stale_count, 0);
    }

    #[test]
    fn test_run_clean_cli_since_overrides_config() {
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Closed 60 days ago
        let closed_ts = now_ts - (60 * 86400);

        let scan = ScanResult {
            items: vec![make_item_with_issue(
                "a.rs",
                1,
                Tag::Todo,
                "fix bug #42",
                "#42",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker = MockIssueChecker::new(vec![(
            42,
            Some(IssueState::Closed {
                closed_at: Some(closed_ts),
            }),
        )]);

        // Config says 90d (would skip), CLI says 30d (should flag)
        let mut config = default_config();
        config.clean.since = Some("90d".to_string());
        let result = run_clean(&scan, &config, Some(&checker), Some("30d"));
        assert!(!result.passed);
        assert_eq!(result.stale_count, 1);
    }

    // --- Multiple items referencing same issue ---

    #[test]
    fn test_multiple_items_same_issue_number() {
        let scan = ScanResult {
            items: vec![
                make_item_with_issue("a.rs", 1, Tag::Todo, "first ref to #42", "#42"),
                make_item_with_issue("b.rs", 10, Tag::Todo, "second ref to #42", "#42"),
            ],
            files_scanned: 2,
            ignored_items: vec![],
        };
        let checker =
            MockIssueChecker::new(vec![(42, Some(IssueState::Closed { closed_at: None }))]);

        let result = run_clean(&scan, &default_config(), Some(&checker), None);
        assert!(!result.passed);
        // Both items should be flagged as stale
        assert_eq!(result.stale_count, 2);
    }

    // --- IssueChecker returning Err ---

    struct ErrorIssueChecker;

    impl IssueChecker for ErrorIssueChecker {
        fn check_issue(&self, _issue_number: u32) -> Result<Option<IssueState>> {
            Err(anyhow::anyhow!("network error"))
        }
    }

    #[test]
    fn test_issue_checker_error_skips_issue() {
        let scan = ScanResult {
            items: vec![make_item_with_issue(
                "a.rs",
                1,
                Tag::Todo,
                "fix bug #42",
                "#42",
            )],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let checker = ErrorIssueChecker;

        // When the checker returns Err, the issue should be skipped (not flagged)
        let result = run_clean(&scan, &default_config(), Some(&checker), None);
        assert!(result.passed);
        assert_eq!(result.stale_count, 0);
    }
}
