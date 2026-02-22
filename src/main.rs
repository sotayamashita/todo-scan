mod blame;
mod cache;
mod check;
mod clean;
mod cli;
mod cmd;
mod completions;
mod config;
mod context;
mod deadline;
mod diff;
mod git;
mod init;
mod lint;
mod model;
mod output;
mod relate;
mod report;
mod scanner;
mod search;
mod stats;
mod tasks;
#[cfg(test)]
mod test_helpers;
mod watch;
mod workspace;

use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use check::CheckOverrides;
use cli::{Cli, Command, WorkspaceAction};
use cmd::*;
use config::Config;
use lint::LintOverrides;

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

    match cli.command {
        // Commands that don't need config
        Command::Init { yes } => init::cmd_init(&root, yes),
        Command::Completions { shell } => completions::cmd_completions(shell),

        // Commands that need config
        command => {
            let config = if let Some(ref config_path) = cli.config {
                let content = std::fs::read_to_string(config_path)?;
                toml::from_str(&content)?
            } else {
                Config::load(&root)?
            };
            let no_cache = cli.no_cache;

            match command {
                Command::Init { .. } | Command::Completions { .. } => unreachable!(),
                Command::List {
                    tag,
                    sort,
                    group_by,
                    priority,
                    author,
                    path,
                    limit,
                    context,
                    package,
                } => {
                    let opts = ListOptions {
                        tag,
                        sort,
                        group_by,
                        priority,
                        author,
                        path,
                        limit,
                        context,
                    };
                    let scan_root = resolve_package_root(&root, &config, package.as_deref())?;
                    cmd_list(&scan_root, &config, &cli.format, opts, no_cache)
                }
                Command::Blame {
                    sort,
                    author,
                    min_age,
                    stale_threshold,
                    tag,
                    path,
                } => {
                    let opts = BlameOptions {
                        sort,
                        author,
                        min_age,
                        stale_threshold,
                        tag,
                        path,
                    };
                    cmd_blame(&root, &config, &cli.format, opts, no_cache)
                }
                Command::Search {
                    query,
                    exact,
                    context,
                    author,
                    tag,
                    path,
                    sort,
                    group_by,
                } => {
                    let opts = SearchOptions {
                        query,
                        exact,
                        context,
                        author,
                        tag,
                        path,
                        sort,
                        group_by,
                    };
                    cmd_search(&root, &config, &cli.format, opts, no_cache)
                }
                Command::Stats { since } => cmd_stats(&root, &config, &cli.format, since, no_cache),
                Command::Diff {
                    git_ref,
                    tag,
                    context,
                    package,
                } => {
                    let scan_root = resolve_package_root(&root, &config, package.as_deref())?;
                    cmd_diff(
                        &scan_root,
                        &config,
                        &cli.format,
                        &git_ref,
                        &tag,
                        context,
                        no_cache,
                    )
                }
                Command::Check {
                    max,
                    block_tags,
                    max_new,
                    since,
                    expired,
                    package,
                    workspace: ws_mode,
                } => {
                    if ws_mode {
                        cmd_workspace_check(&root, &config, &cli.format, no_cache)
                    } else {
                        let overrides = CheckOverrides {
                            max,
                            block_tags,
                            max_new,
                            expired,
                        };
                        let scan_root = resolve_package_root(&root, &config, package.as_deref())?;
                        cmd_check(&scan_root, &config, &cli.format, overrides, since, no_cache)
                    }
                }
                Command::Context { location, context } => {
                    cmd_context(&root, &config, &cli.format, &location, context, no_cache)
                }
                Command::Clean { check, since } => {
                    cmd_clean(&root, &config, &cli.format, check, since, no_cache)
                }
                Command::Relate {
                    cluster,
                    r#for: for_item,
                    min_score,
                    proximity,
                } => {
                    let opts = RelateOptions {
                        cluster,
                        for_item,
                        min_score,
                        proximity,
                    };
                    cmd_relate(&root, &config, &cli.format, opts, no_cache)
                }
                Command::Lint {
                    no_bare_tags,
                    max_message_length,
                    require_author,
                    require_issue_ref,
                    uppercase_tag,
                    require_colon,
                } => {
                    let overrides = LintOverrides {
                        no_bare_tags,
                        max_message_length,
                        require_author,
                        require_issue_ref,
                        uppercase_tag,
                        require_colon,
                    };
                    cmd_lint(&root, &config, &cli.format, overrides, no_cache)
                }
                Command::Report {
                    output,
                    history,
                    stale_threshold,
                } => cmd_report(&root, &config, &output, history, stale_threshold, no_cache),
                Command::Tasks {
                    tag,
                    context,
                    output,
                    dry_run,
                    since,
                    priority,
                    author,
                    path,
                } => {
                    let opts = TasksOptions {
                        tag,
                        context,
                        output,
                        dry_run,
                        since,
                        priority,
                        author,
                        path,
                    };
                    cmd_tasks(&root, &config, &cli.format, opts, no_cache)
                }
                Command::Watch { tag, max, debounce } => {
                    watch::cmd_watch(&root, &config, &cli.format, &tag, max, debounce)
                }
                Command::Workspace { action } => match action {
                    WorkspaceAction::List => {
                        cmd_workspace_list(&root, &config, &cli.format, no_cache)
                    }
                },
            }
        }
    }
}
