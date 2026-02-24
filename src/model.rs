use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::deadline::Deadline;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Tag {
    Todo,
    Fixme,
    Hack,
    Xxx,
    Bug,
    Note,
}

impl Tag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tag::Todo => "TODO",
            Tag::Fixme => "FIXME",
            Tag::Hack => "HACK",
            Tag::Xxx => "XXX",
            Tag::Bug => "BUG",
            Tag::Note => "NOTE",
        }
    }

    pub fn severity(&self) -> u8 {
        match self {
            Tag::Note => 0,
            Tag::Todo => 1,
            Tag::Hack => 2,
            Tag::Xxx => 3,
            Tag::Fixme => 4,
            Tag::Bug => 5,
        }
    }
}

impl FromStr for Tag {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TODO" => Ok(Tag::Todo),
            "FIXME" => Ok(Tag::Fixme),
            "HACK" => Ok(Tag::Hack),
            "XXX" => Ok(Tag::Xxx),
            "BUG" => Ok(Tag::Bug),
            "NOTE" => Ok(Tag::Note),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Normal,
    High,
    Urgent,
}

impl Priority {
    pub fn numeric_order(&self) -> u8 {
        match self {
            Priority::Urgent => 2,
            Priority::High => 1,
            Priority::Normal => 0,
        }
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.numeric_order().cmp(&other.numeric_order())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub file: String,
    pub line: usize,
    pub tag: Tag,
    pub message: String,
    pub author: Option<String>,
    pub issue_ref: Option<String>,
    pub priority: Priority,
    pub deadline: Option<Deadline>,
}

impl TodoItem {
    /// Matching key for diff comparison (excludes line number)
    pub fn match_key(&self) -> String {
        let normalized = self.message.trim().to_lowercase();
        format!("{}:{}:{}", self.file, self.tag, normalized)
    }

    /// Stable, content-based identifier for this TODO item.
    /// Unlike `file:line`, this ID survives line-number changes from edits.
    pub fn id(&self) -> String {
        self.match_key()
    }
}

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub items: Vec<TodoItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ignored_items: Vec<TodoItem>,
    pub files_scanned: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffStatus {
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffEntry {
    pub status: DiffStatus,
    pub item: TodoItem,
}

#[derive(Debug, Serialize)]
pub struct DiffResult {
    pub entries: Vec<DiffEntry>,
    pub added_count: usize,
    pub removed_count: usize,
    pub base_ref: String,
}

#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub passed: bool,
    pub total: usize,
    pub violations: Vec<CheckViolation>,
}

#[derive(Debug, Serialize)]
pub struct CheckViolation {
    pub rule: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct StatsResult {
    pub total_items: usize,
    pub total_files: usize,
    pub tag_counts: Vec<(Tag, usize)>,
    pub priority_counts: PriorityCounts,
    pub author_counts: Vec<(String, usize)>,
    pub hotspot_files: Vec<(String, usize)>,
    pub trend: Option<TrendInfo>,
}

#[derive(Debug, Serialize)]
pub struct PriorityCounts {
    pub normal: usize,
    pub high: usize,
    pub urgent: usize,
}

#[derive(Debug, Serialize)]
pub struct TrendInfo {
    pub added: usize,
    pub removed: usize,
    pub base_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlameInfo {
    pub author: String,
    pub email: String,
    pub date: String,
    pub age_days: u64,
    pub commit: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlameEntry {
    #[serde(flatten)]
    pub item: TodoItem,
    pub blame: BlameInfo,
    pub stale: bool,
}

#[derive(Debug, Serialize)]
pub struct BlameResult {
    pub entries: Vec<BlameEntry>,
    pub total: usize,
    pub avg_age_days: u64,
    pub stale_count: usize,
    pub stale_threshold_days: u64,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub query: String,
    pub exact: bool,
    pub items: Vec<TodoItem>,
    pub match_count: usize,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LintViolation {
    pub rule: String,
    pub message: String,
    pub file: String,
    pub line: usize,
    pub suggestion: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LintResult {
    pub passed: bool,
    pub total_items: usize,
    pub violation_count: usize,
    pub violations: Vec<LintViolation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileUpdate {
    pub added: Vec<TodoItem>,
    pub removed: Vec<TodoItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WatchEvent {
    pub timestamp: String,
    pub file: String,
    pub added: Vec<TodoItem>,
    pub removed: Vec<TodoItem>,
    pub tag_summary: Vec<(String, usize)>,
    pub total: usize,
    pub total_delta: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanViolation {
    pub rule: String,
    pub message: String,
    pub file: String,
    pub line: usize,
    pub issue_ref: Option<String>,
    pub duplicate_of: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CleanResult {
    pub passed: bool,
    pub total_items: usize,
    pub stale_count: usize,
    pub duplicate_count: usize,
    pub violations: Vec<CleanViolation>,
}

#[derive(Debug, Serialize)]
pub struct ReportResult {
    pub generated_at: String,
    pub summary: ReportSummary,
    pub tag_counts: Vec<(Tag, usize)>,
    pub priority_counts: PriorityCounts,
    pub author_counts: Vec<(String, usize)>,
    pub hotspot_files: Vec<(String, usize)>,
    pub history: Vec<HistoryPoint>,
    pub age_histogram: Vec<AgeBucket>,
    pub items: Vec<TodoItem>,
}

#[derive(Debug, Serialize)]
pub struct ReportSummary {
    pub total_items: usize,
    pub total_files: usize,
    pub files_scanned: usize,
    pub urgent_count: usize,
    pub high_count: usize,
    pub stale_count: usize,
    pub avg_age_days: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryPoint {
    pub commit: String,
    pub date: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgeBucket {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaudeTask {
    pub subject: String,
    pub description: String,
    #[serde(rename = "activeForm")]
    pub active_form: String,
    pub metadata: ClaudeTaskMetadata,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaudeTaskMetadata {
    pub todo_scan_file: String,
    pub todo_scan_line: usize,
    pub todo_scan_tag: String,
    pub todo_scan_priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todo_scan_author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todo_scan_issue_ref: Option<String>,
    pub todo_scan_match_key: String,
}

#[derive(Debug, Serialize)]
pub struct TasksResult {
    pub tasks: Vec<ClaudeTask>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Notice,
}

impl Severity {
    pub fn from_item(item: &TodoItem) -> Self {
        if item.priority == Priority::Urgent {
            return Severity::Error;
        }
        match item.tag {
            Tag::Bug | Tag::Fixme => Severity::Error,
            Tag::Todo | Tag::Hack | Tag::Xxx => Severity::Warning,
            Tag::Note => Severity::Notice,
        }
    }

    pub fn as_github_actions_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Notice => "notice",
        }
    }

    pub fn as_sarif_level(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Notice => "note",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceKind {
    Cargo,
    Npm,
    Pnpm,
    Nx,
    #[serde(rename = "go")]
    GoWork,
    Manual,
}

impl fmt::Display for WorkspaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceKind::Cargo => write!(f, "cargo"),
            WorkspaceKind::Npm => write!(f, "npm"),
            WorkspaceKind::Pnpm => write!(f, "pnpm"),
            WorkspaceKind::Nx => write!(f, "nx"),
            WorkspaceKind::GoWork => write!(f, "go"),
            WorkspaceKind::Manual => write!(f, "manual"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageInfo {
    pub name: String,
    pub path: String,
    pub kind: WorkspaceKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceInfo {
    pub kind: WorkspaceKind,
    pub packages: Vec<PackageInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageStatus {
    Ok,
    Over,
    Uncapped,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageScanSummary {
    pub name: String,
    pub path: String,
    pub todo_count: usize,
    pub max: Option<usize>,
    pub status: PackageStatus,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceResult {
    pub packages: Vec<PackageScanSummary>,
    pub total_todos: usize,
    pub total_packages: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Cluster {
    pub id: usize,
    pub theme: String,
    pub items: Vec<String>,
    pub suggested_order: Vec<String>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Serialize)]
pub struct RelateResult {
    pub relationships: Vec<Relationship>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clusters: Option<Vec<Cluster>>,
    pub total_relationships: usize,
    pub total_items: usize,
    pub min_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BriefResult {
    pub total_items: usize,
    pub total_files: usize,
    pub priority_counts: PriorityCounts,
    pub top_urgent: Option<TodoItem>,
    pub trend: Option<TrendInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_returns_same_value_as_match_key() {
        let item = TodoItem {
            file: "src/main.rs".to_string(),
            line: 42,
            tag: Tag::Todo,
            message: "fix this bug".to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: None,
        };
        assert_eq!(item.id(), item.match_key());
    }

    #[test]
    fn priority_numeric_order_values() {
        assert_eq!(Priority::Normal.numeric_order(), 0);
        assert_eq!(Priority::High.numeric_order(), 1);
        assert_eq!(Priority::Urgent.numeric_order(), 2);
    }

    #[test]
    fn priority_ord_urgent_gt_high_gt_normal() {
        assert!(Priority::Urgent > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Urgent > Priority::Normal);
    }

    #[test]
    fn priority_ord_equality() {
        assert_eq!(Priority::Normal, Priority::Normal);
        assert_eq!(Priority::High, Priority::High);
        assert_eq!(Priority::Urgent, Priority::Urgent);
    }

    #[test]
    fn priority_ord_sorting() {
        let mut priorities = vec![Priority::Normal, Priority::Urgent, Priority::High];
        priorities.sort();
        assert_eq!(
            priorities,
            vec![Priority::Normal, Priority::High, Priority::Urgent]
        );
    }

    #[test]
    fn workspace_kind_display() {
        assert_eq!(WorkspaceKind::Cargo.to_string(), "cargo");
        assert_eq!(WorkspaceKind::Npm.to_string(), "npm");
        assert_eq!(WorkspaceKind::Pnpm.to_string(), "pnpm");
        assert_eq!(WorkspaceKind::Nx.to_string(), "nx");
        assert_eq!(WorkspaceKind::GoWork.to_string(), "go");
        assert_eq!(WorkspaceKind::Manual.to_string(), "manual");
    }

    #[test]
    fn workspace_kind_serializes_lowercase() {
        let json = serde_json::to_string(&WorkspaceKind::Cargo).unwrap();
        assert_eq!(json, "\"cargo\"");
        let json = serde_json::to_string(&WorkspaceKind::GoWork).unwrap();
        assert_eq!(json, "\"go\"");
    }

    #[test]
    fn package_status_serializes() {
        assert_eq!(serde_json::to_string(&PackageStatus::Ok).unwrap(), "\"ok\"");
        assert_eq!(
            serde_json::to_string(&PackageStatus::Over).unwrap(),
            "\"over\""
        );
        assert_eq!(
            serde_json::to_string(&PackageStatus::Uncapped).unwrap(),
            "\"uncapped\""
        );
    }

    #[test]
    fn relationship_serializes() {
        let rel = Relationship {
            from: "src/main.rs:10".to_string(),
            to: "src/main.rs:15".to_string(),
            score: 0.65,
            reason: "proximity".to_string(),
        };
        let json = serde_json::to_string_pretty(&rel).unwrap();
        assert!(json.contains("\"from\": \"src/main.rs:10\""));
        assert!(json.contains("\"to\": \"src/main.rs:15\""));
        assert!(json.contains("\"score\": 0.65"));
        assert!(json.contains("\"reason\": \"proximity\""));
    }

    #[test]
    fn relate_result_serializes_without_optionals() {
        let result = RelateResult {
            relationships: vec![],
            clusters: None,
            total_relationships: 0,
            total_items: 0,
            min_score: 0.3,
            target: None,
        };
        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains("\"total_relationships\": 0"));
        assert!(json.contains("\"min_score\": 0.3"));
        assert!(!json.contains("\"clusters\""));
        assert!(!json.contains("\"target\""));
    }

    #[test]
    fn relate_result_serializes_with_clusters() {
        let result = RelateResult {
            relationships: vec![],
            clusters: Some(vec![Cluster {
                id: 1,
                theme: "authentication".to_string(),
                items: vec!["src/auth.rs:10".to_string()],
                suggested_order: vec!["src/auth.rs:10".to_string()],
                relationships: vec![],
            }]),
            total_relationships: 0,
            total_items: 1,
            min_score: 0.3,
            target: Some("src/auth.rs:10".to_string()),
        };
        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains("\"clusters\""));
        assert!(json.contains("\"theme\": \"authentication\""));
        assert!(json.contains("\"target\": \"src/auth.rs:10\""));
    }

    #[test]
    fn tag_from_str_valid_cases() {
        assert_eq!("TODO".parse::<Tag>(), Ok(Tag::Todo));
        assert_eq!("fixme".parse::<Tag>(), Ok(Tag::Fixme));
        assert_eq!("Hack".parse::<Tag>(), Ok(Tag::Hack));
        assert_eq!("xxx".parse::<Tag>(), Ok(Tag::Xxx));
        assert_eq!("BUG".parse::<Tag>(), Ok(Tag::Bug));
        assert_eq!("Note".parse::<Tag>(), Ok(Tag::Note));
    }

    #[test]
    fn tag_from_str_invalid() {
        assert_eq!("INVALID".parse::<Tag>(), Err(()));
        assert_eq!("".parse::<Tag>(), Err(()));
        assert_eq!("TODOS".parse::<Tag>(), Err(()));
    }

    #[test]
    fn tag_display() {
        assert_eq!(Tag::Todo.to_string(), "TODO");
        assert_eq!(Tag::Fixme.to_string(), "FIXME");
        assert_eq!(Tag::Bug.to_string(), "BUG");
    }

    #[test]
    fn severity_from_item_urgent_always_error() {
        let mut item = TodoItem {
            file: "a.rs".to_string(),
            line: 1,
            tag: Tag::Note, // Note normally => Notice
            message: "test".to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Urgent,
            deadline: None,
        };
        // Urgent overrides to Error regardless of tag
        assert_eq!(Severity::from_item(&item), Severity::Error);

        item.tag = Tag::Todo;
        assert_eq!(Severity::from_item(&item), Severity::Error);
    }

    #[test]
    fn severity_from_item_by_tag() {
        let make = |tag: Tag| TodoItem {
            file: "a.rs".to_string(),
            line: 1,
            tag,
            message: "test".to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: None,
        };
        assert_eq!(Severity::from_item(&make(Tag::Bug)), Severity::Error);
        assert_eq!(Severity::from_item(&make(Tag::Fixme)), Severity::Error);
        assert_eq!(Severity::from_item(&make(Tag::Todo)), Severity::Warning);
        assert_eq!(Severity::from_item(&make(Tag::Hack)), Severity::Warning);
        assert_eq!(Severity::from_item(&make(Tag::Xxx)), Severity::Warning);
        assert_eq!(Severity::from_item(&make(Tag::Note)), Severity::Notice);
    }

    #[test]
    fn severity_github_actions_str() {
        assert_eq!(Severity::Error.as_github_actions_str(), "error");
        assert_eq!(Severity::Warning.as_github_actions_str(), "warning");
        assert_eq!(Severity::Notice.as_github_actions_str(), "notice");
    }

    #[test]
    fn severity_sarif_level() {
        assert_eq!(Severity::Error.as_sarif_level(), "error");
        assert_eq!(Severity::Warning.as_sarif_level(), "warning");
        assert_eq!(Severity::Notice.as_sarif_level(), "note");
    }

    #[test]
    fn workspace_result_serializes() {
        let result = WorkspaceResult {
            packages: vec![PackageScanSummary {
                name: "core".to_string(),
                path: "packages/core".to_string(),
                todo_count: 5,
                max: Some(10),
                status: PackageStatus::Ok,
            }],
            total_todos: 5,
            total_packages: 1,
        };
        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains("\"name\": \"core\""));
        assert!(json.contains("\"todo_count\": 5"));
        assert!(json.contains("\"status\": \"ok\""));
    }
}
