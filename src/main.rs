mod check;
mod cli;
mod config;
mod deadline;
mod diff;
mod model;
mod output;
mod scanner;

use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use check::{run_check, CheckOverrides};
use cli::{Cli, Command, Format, GroupBy, PriorityFilter, SortBy};
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

    let root = match cli.root {
        Some(p) => p,
        None => std::env::current_dir().context("cannot determine current directory")?,
    };

    let config = if let Some(ref config_path) = cli.config {
        let content = std::fs::read_to_string(config_path)?;
        toml::from_str(&content)?
    } else {
        Config::load(&root)?
    };

    match cli.command {
        Command::List {
            tag,
            sort,
            group_by,
            priority,
            author,
            path,
            limit,
        } => {
            let opts = ListOptions {
                tag,
                sort,
                group_by,
                priority,
                author,
                path,
                limit,
            };
            cmd_list(&root, &config, &cli.format, opts)
        }
        Command::Diff { git_ref, tag } => cmd_diff(&root, &config, &cli.format, &git_ref, &tag),
        Command::Check {
            max,
            block_tags,
            max_new,
            since,
            expired,
        } => {
            let overrides = CheckOverrides {
                max,
                block_tags,
                max_new,
                expired,
            };
            cmd_check(&root, &config, &cli.format, overrides, since)
        }
    }
}

struct ListOptions {
    tag: Vec<String>,
    sort: SortBy,
    group_by: GroupBy,
    priority: Vec<PriorityFilter>,
    author: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
}

fn cmd_list(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    opts: ListOptions,
) -> Result<()> {
    let mut result = scan_directory(root, config)?;

    // Apply tag filter
    if !opts.tag.is_empty() {
        let filter_tags: Vec<Tag> = opts
            .tag
            .iter()
            .filter_map(|s| s.parse::<Tag>().ok())
            .collect();
        result.items.retain(|item| filter_tags.contains(&item.tag));
    }

    // Apply priority filter
    if !opts.priority.is_empty() {
        let priorities: Vec<model::Priority> =
            opts.priority.iter().map(|p| p.to_priority()).collect();
        result
            .items
            .retain(|item| priorities.contains(&item.priority));
    }

    // Apply author filter
    if let Some(ref author) = opts.author {
        result
            .items
            .retain(|item| item.author.as_deref() == Some(author.as_str()));
    }

    // Apply path filter
    if let Some(ref pattern) = opts.path {
        let glob = globset::Glob::new(pattern)
            .context("invalid glob pattern")?
            .compile_matcher();
        result.items.retain(|item| glob.is_match(&item.file));
    }

    // Apply sort
    match opts.sort {
        SortBy::File => result
            .items
            .sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line))),
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

    // Apply limit
    if let Some(n) = opts.limit {
        result.items.truncate(n);
    }

    print_list(&result, format, &opts.group_by);
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
        let filter_tags: Vec<Tag> = tag_filter
            .iter()
            .filter_map(|s| s.parse::<Tag>().ok())
            .collect();
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
    overrides: CheckOverrides,
    since: Option<String>,
) -> Result<()> {
    let scan = scan_directory(root, config)?;

    let diff = if let Some(ref base_ref) = since {
        Some(compute_diff(&scan, base_ref, root, config)?)
    } else {
        None
    };

    let today = deadline::today();
    let result = run_check(&scan, diff.as_ref(), config, &overrides, &today);
    let passed = result.passed;

    print_check(&result, format);

    if !passed {
        process::exit(1);
    }

    Ok(())
}
