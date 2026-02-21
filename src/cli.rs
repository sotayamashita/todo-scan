use clap::{Parser, Subcommand, ValueEnum};
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
    },

    Diff {
        git_ref: String,

        #[arg(long)]
        tag: Vec<String>,
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

impl PriorityFilter {
    pub fn to_priority(&self) -> model::Priority {
        match self {
            PriorityFilter::Normal => model::Priority::Normal,
            PriorityFilter::High => model::Priority::High,
            PriorityFilter::Urgent => model::Priority::Urgent,
        }
    }
}
