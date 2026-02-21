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

    #[command(subcommand)]
    pub command: Command,
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
    },

    Diff {
        git_ref: String,

        #[arg(long)]
        tag: Vec<String>,

        /// Number of context lines to show around each TODO
        #[arg(short = 'C', long)]
        context: Option<usize>,
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

impl PriorityFilter {
    pub fn to_priority(&self) -> model::Priority {
        match self {
            PriorityFilter::Normal => model::Priority::Normal,
            PriorityFilter::High => model::Priority::High,
            PriorityFilter::Urgent => model::Priority::Urgent,
        }
    }
}
