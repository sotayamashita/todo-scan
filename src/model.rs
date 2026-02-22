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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Normal,
    High,
    Urgent,
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
}

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub items: Vec<TodoItem>,
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
    pub todox_file: String,
    pub todox_line: usize,
    pub todox_tag: String,
    pub todox_priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todox_author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todox_issue_ref: Option<String>,
    pub todox_match_key: String,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(json, "\"gowork\"");
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
