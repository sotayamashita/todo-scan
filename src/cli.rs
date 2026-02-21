use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

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
