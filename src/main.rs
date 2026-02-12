mod check;
mod cli;
mod config;
mod diff;
mod model;
mod output;
mod scanner;

use std::process;

use anyhow::Result;
use clap::Parser;

use check::{CheckOverrides, run_check};
use cli::{Cli, Command, Format, SortBy};
use config::Config;
use diff::compute_diff;
use model::Tag;
use output::{print_check, print_diff, print_list};
use scanner::scan_directory;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {:#}", e);
        process::exit(2);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let root = cli
        .root
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine current directory"));

    let config = if let Some(ref config_path) = cli.config {
        let content = std::fs::read_to_string(config_path)?;
        toml::from_str(&content)?
    } else {
        Config::load(&root)?
    };

    match cli.command {
        Command::List { tag, sort } => cmd_list(&root, &config, &cli.format, &tag, &sort),
        Command::Diff { git_ref, tag } => cmd_diff(&root, &config, &cli.format, &git_ref, &tag),
        Command::Check {
            max,
            block_tags,
            max_new,
            since,
        } => cmd_check(&root, &config, &cli.format, max, block_tags, max_new, since),
    }
}

fn cmd_list(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    tag_filter: &[String],
    sort: &SortBy,
) -> Result<()> {
    let mut result = scan_directory(root, config)?;

    // Apply tag filter
    if !tag_filter.is_empty() {
        let filter_tags: Vec<Tag> = tag_filter.iter().filter_map(|s| Tag::from_str(s)).collect();
        result.items.retain(|item| filter_tags.contains(&item.tag));
    }

    // Apply sort
    match sort {
        SortBy::File => result.items.sort_by(|a, b| {
            a.file.cmp(&b.file).then(a.line.cmp(&b.line))
        }),
        SortBy::Tag => result.items.sort_by(|a, b| {
            a.tag
                .severity()
                .cmp(&b.tag.severity())
                .reverse()
                .then(a.file.cmp(&b.file))
                .then(a.line.cmp(&b.line))
        }),
        SortBy::Priority => result.items.sort_by(|a, b| {
            let pa = match a.priority {
                model::Priority::Urgent => 2,
                model::Priority::High => 1,
                model::Priority::Normal => 0,
            };
            let pb = match b.priority {
                model::Priority::Urgent => 2,
                model::Priority::High => 1,
                model::Priority::Normal => 0,
            };
            pb.cmp(&pa)
                .then(a.file.cmp(&b.file))
                .then(a.line.cmp(&b.line))
        }),
    }

    print_list(&result, format);
    Ok(())
}

fn cmd_diff(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    git_ref: &str,
    tag_filter: &[String],
) -> Result<()> {
    let current = scan_directory(root, config)?;
    let mut diff_result = compute_diff(&current, git_ref, root, config)?;

    // Apply tag filter
    if !tag_filter.is_empty() {
        let filter_tags: Vec<Tag> = tag_filter.iter().filter_map(|s| Tag::from_str(s)).collect();
        diff_result
            .entries
            .retain(|entry| filter_tags.contains(&entry.item.tag));
        diff_result.added_count = diff_result
            .entries
            .iter()
            .filter(|e| matches!(e.status, model::DiffStatus::Added))
            .count();
        diff_result.removed_count = diff_result
            .entries
            .iter()
            .filter(|e| matches!(e.status, model::DiffStatus::Removed))
            .count();
    }

    print_diff(&diff_result, format);
    Ok(())
}

fn cmd_check(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    max: Option<usize>,
    block_tags: Vec<String>,
    max_new: Option<usize>,
    since: Option<String>,
) -> Result<()> {
    let scan = scan_directory(root, config)?;

    let diff = if let Some(ref base_ref) = since {
        Some(compute_diff(&scan, base_ref, root, config)?)
    } else {
        None
    };

    let overrides = CheckOverrides {
        max,
        block_tags,
        max_new,
    };

    let result = run_check(&scan, diff.as_ref(), config, &overrides);
    let passed = result.passed;

    print_check(&result, format);

    if !passed {
        process::exit(1);
    }

    Ok(())
}
