use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

use crate::model;

#[derive(Parser)]
#[command(
    name = "todox",
    version,
    about = "Track TODO/FIXME/HACK comments in your codebase"
)]
pub struct Cli {
    #[arg(long, global = true, value_enum, default_value = "text")]
    pub format: Format,

    #[arg(long, global = true)]
    pub root: Option<PathBuf>,

    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Disable scan result caching
    #[arg(long, global = true)]
    pub no_cache: bool,

    /// Control output detail level: minimal (compact), normal (default), full (enriched)
    #[arg(long, global = true, value_enum, default_value = "normal")]
    pub detail: DetailLevel,

    /// Show items suppressed by todox:ignore markers
    #[arg(long, global = true)]
    pub show_ignored: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum DetailLevel {
    Minimal,
    Normal,
    Full,
}

#[derive(Clone, ValueEnum)]
pub enum Format {
    Text,
    Json,
    GithubActions,
    Sarif,
    Markdown,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(alias = "ls")]
    List {
        #[arg(long)]
        tag: Vec<String>,

        #[arg(long, value_enum, default_value = "file")]
        sort: SortBy,

        #[arg(long, value_enum, default_value = "file")]
        group_by: GroupBy,

        #[arg(long, value_enum)]
        priority: Vec<PriorityFilter>,

        #[arg(long)]
        author: Option<String>,

        #[arg(long)]
        path: Option<String>,

        #[arg(long)]
        limit: Option<usize>,

        /// Number of context lines to show around each TODO
        #[arg(short = 'C', long)]
        context: Option<usize>,

        /// Scope scan to a single workspace package
        #[arg(long)]
        package: Option<String>,
    },

    Diff {
        git_ref: String,

        #[arg(long)]
        tag: Vec<String>,

        /// Number of context lines to show around each TODO
        #[arg(short = 'C', long)]
        context: Option<usize>,

        /// Scope scan to a single workspace package
        #[arg(long)]
        package: Option<String>,
    },

    /// Show code context around a TODO at FILE:LINE
    Context {
        /// Location in FILE:LINE format
        location: String,

        /// Number of context lines (default: 5)
        #[arg(short = 'C', long, default_value = "5")]
        context: usize,
    },

    /// Generate a .todox.toml configuration file
    Init {
        /// Accept defaults without interactive prompts
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Show git blame metadata for TODO comments
    Blame {
        #[arg(long, value_enum, default_value = "file")]
        sort: BlameSortBy,

        #[arg(long)]
        author: Option<String>,

        #[arg(long)]
        min_age: Option<String>,

        /// Days threshold for marking TODOs as stale (default: 365)
        #[arg(long)]
        stale_threshold: Option<String>,

        #[arg(long)]
        tag: Vec<String>,

        #[arg(long)]
        path: Option<String>,
    },

    Stats {
        #[arg(long)]
        since: Option<String>,
    },

    /// Search TODO comments by message text or issue reference
    #[command(alias = "s")]
    Search {
        /// Search query string
        query: String,

        /// Exact case-sensitive substring match (default: case-insensitive)
        #[arg(long)]
        exact: bool,

        /// Number of context lines to show around each match
        #[arg(short = 'C', long)]
        context: Option<usize>,

        #[arg(long)]
        author: Option<String>,

        #[arg(long)]
        tag: Vec<String>,

        #[arg(long)]
        path: Option<String>,

        #[arg(long, value_enum, default_value = "file")]
        sort: SortBy,

        #[arg(long, value_enum, default_value = "file")]
        group_by: GroupBy,
    },

    Check {
        #[arg(long)]
        max: Option<usize>,

        #[arg(long, value_delimiter = ',')]
        block_tags: Vec<String>,

        #[arg(long)]
        max_new: Option<usize>,

        #[arg(long)]
        since: Option<String>,

        #[arg(long)]
        expired: bool,

        /// Scope scan to a single workspace package
        #[arg(long)]
        package: Option<String>,

        /// Run check across all workspace packages with per-package thresholds
        #[arg(long)]
        workspace: bool,
    },

    /// Watch filesystem for TODO changes in real-time
    #[command(alias = "w")]
    Watch {
        #[arg(long)]
        tag: Vec<String>,

        #[arg(long)]
        max: Option<usize>,

        /// Debounce interval in milliseconds
        #[arg(long, default_value = "300")]
        debounce: u64,
    },

    /// Find stale issue references and duplicate TODOs
    Clean {
        /// Exit with code 1 if any violations found (CI gate mode)
        #[arg(long)]
        check: bool,

        /// Only flag issues closed longer than this duration (e.g., "30d")
        #[arg(long)]
        since: Option<String>,
    },

    /// Generate an HTML technical debt dashboard report
    Report {
        /// Output file path (default: todox-report.html)
        #[arg(long, default_value = "todox-report.html")]
        output: String,

        /// Number of historical commits to sample for trend chart
        #[arg(long, default_value = "10")]
        history: usize,

        /// Days threshold for marking TODOs as stale (default: 365)
        #[arg(long)]
        stale_threshold: Option<String>,
    },

    /// Export TODOs as Claude Code Tasks (Claude Code-specific; not compatible with other coding agents)
    Tasks {
        /// Filter by tag (repeatable)
        #[arg(long)]
        tag: Vec<String>,

        /// Number of context lines in description
        #[arg(short = 'C', long, default_value = "3")]
        context: usize,

        /// Output directory path for task JSON files
        #[arg(long)]
        output: Option<std::path::PathBuf>,

        /// Preview to stdout only (no file writes)
        #[arg(long)]
        dry_run: bool,

        /// Only TODOs added since this git ref
        #[arg(long)]
        since: Option<String>,

        /// Filter by priority (repeatable)
        #[arg(long, value_enum)]
        priority: Vec<PriorityFilter>,

        /// Filter by author
        #[arg(long)]
        author: Option<String>,

        /// Filter by file glob
        #[arg(long)]
        path: Option<String>,
    },

    /// Manage and inspect workspace packages
    #[command(alias = "ws")]
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },

    /// Discover relationships between TODO comments
    Relate {
        /// Group related TODOs into clusters
        #[arg(long)]
        cluster: bool,

        /// Show TODOs related to a specific item (FILE:LINE)
        #[arg(long, value_name = "LOCATION")]
        r#for: Option<String>,

        /// Minimum relationship score (0.0-1.0)
        #[arg(long, default_value = "0.3")]
        min_score: f64,

        /// Line proximity threshold for same-file detection
        #[arg(long, default_value = "10")]
        proximity: usize,
    },

    /// Lint TODO comment formatting against configurable rules
    Lint {
        /// Reject TODOs with empty message
        #[arg(long)]
        no_bare_tags: bool,

        /// Enforce max message character count
        #[arg(long)]
        max_message_length: Option<usize>,

        /// Require (author) for specified tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        require_author: Vec<String>,

        /// Require issue ref (#N) for specified tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        require_issue_ref: Vec<String>,

        /// Enforce uppercase tag names
        #[arg(long)]
        uppercase_tag: bool,

        /// Enforce colon after tag
        #[arg(long)]
        require_colon: bool,
    },
}

#[derive(Clone, ValueEnum)]
pub enum SortBy {
    File,
    Tag,
    Priority,
}

#[derive(Clone, ValueEnum)]
pub enum GroupBy {
    File,
    Tag,
    Priority,
    Author,
    Dir,
}

#[derive(Clone, ValueEnum)]
pub enum PriorityFilter {
    Normal,
    High,
    Urgent,
}

#[derive(Clone, ValueEnum)]
pub enum BlameSortBy {
    File,
    Age,
    Author,
    Tag,
}

#[derive(Subcommand)]
pub enum WorkspaceAction {
    /// List detected workspace packages and their TODO counts
    #[command(alias = "ls")]
    List,
}

impl PriorityFilter {
    pub fn to_priority(&self) -> model::Priority {
        match self {
            PriorityFilter::Normal => model::Priority::Normal,
            PriorityFilter::High => model::Priority::High,
            PriorityFilter::Urgent => model::Priority::Urgent,
        }
    }
}
