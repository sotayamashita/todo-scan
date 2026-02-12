use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
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

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TODO" => Some(Tag::Todo),
            "FIXME" => Some(Tag::Fixme),
            "HACK" => Some(Tag::Hack),
            "XXX" => Some(Tag::Xxx),
            "BUG" => Some(Tag::Bug),
            "NOTE" => Some(Tag::Note),
            _ => None,
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

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Normal,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize)]
pub struct TodoItem {
    pub file: String,
    pub line: usize,
    pub tag: Tag,
    pub message: String,
    pub author: Option<String>,
    pub issue_ref: Option<String>,
    pub priority: Priority,
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
