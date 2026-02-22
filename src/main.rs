mod blame;
mod cache;
mod check;
mod clean;
mod cli;
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
mod watch;
mod workspace;

use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use blame::compute_blame;
use check::{run_check, CheckOverrides};
use cli::{BlameSortBy, Cli, Command, Format, GroupBy, PriorityFilter, SortBy, WorkspaceAction};
use config::Config;
use context::{build_rich_context, collect_context_map, parse_location};
use diff::compute_diff;
use lint::{run_lint, LintOverrides};
use model::Tag;
use output::{
    print_blame, print_check, print_clean, print_context, print_diff, print_lint, print_list,
    print_relate, print_report, print_search, print_stats, print_tasks, print_workspace_list,
};
use scanner::scan_directory;
use search::search_items;
use stats::compute_stats;

/// Perform a directory scan, optionally using cache for performance.
fn do_scan(root: &std::path::Path, config: &Config, no_cache: bool) -> Result<model::ScanResult> {
    if no_cache {
        return scan_directory(root, config);
    }

    let config_hash = cache::ScanCache::config_hash(config);

    let mut scan_cache = cache::ScanCache::load(root)
        .filter(|c| c.config_hash == config_hash)
        .unwrap_or_else(|| cache::ScanCache::new(config_hash));

    let cached_result = scanner::scan_directory_cached(root, config, &mut scan_cache)?;

    // Best-effort save; don't fail the scan if cache write fails
    let _ = scan_cache.save(root);

    Ok(cached_result.result)
}

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
                } => cmd_blame(
                    &root,
                    &config,
                    &cli.format,
                    sort,
                    author,
                    min_age,
                    stale_threshold,
                    tag,
                    path,
                    no_cache,
                ),
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
                } => cmd_relate(
                    &root,
                    &config,
                    &cli.format,
                    cluster,
                    for_item,
                    min_score,
                    proximity,
                    no_cache,
                ),
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
                } => cmd_tasks(
                    &root,
                    &config,
                    &cli.format,
                    tag,
                    context,
                    output,
                    dry_run,
                    since,
                    priority,
                    author,
                    path,
                    no_cache,
                ),
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

struct ListOptions {
    tag: Vec<String>,
    sort: SortBy,
    group_by: GroupBy,
    priority: Vec<PriorityFilter>,
    author: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
    context: Option<usize>,
}

struct SearchOptions {
    query: String,
    exact: bool,
    context: Option<usize>,
    author: Option<String>,
    tag: Vec<String>,
    path: Option<String>,
    sort: SortBy,
    group_by: GroupBy,
}

fn cmd_search(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    opts: SearchOptions,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;
    let mut result = search_items(&scan, &opts.query, opts.exact);

    // Apply tag filter
    if !opts.tag.is_empty() {
        let filter_tags: Vec<Tag> = opts
            .tag
            .iter()
            .filter_map(|s| s.parse::<Tag>().ok())
            .collect();
        result.items.retain(|item| filter_tags.contains(&item.tag));
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

    // Recompute counts after filtering
    result.match_count = result.items.len();
    result.file_count = result
        .items
        .iter()
        .map(|i| &i.file)
        .collect::<std::collections::HashSet<_>>()
        .len();

    let context_map = if let Some(n) = opts.context {
        collect_context_map(root, &result.items, n)
    } else {
        std::collections::HashMap::new()
    };

    print_search(&result, format, &opts.group_by, &context_map);
    Ok(())
}

fn cmd_list(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    opts: ListOptions,
    no_cache: bool,
) -> Result<()> {
    let mut result = do_scan(root, config, no_cache)?;

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

    let context_map = if let Some(n) = opts.context {
        collect_context_map(root, &result.items, n)
    } else {
        std::collections::HashMap::new()
    };

    print_list(&result, format, &opts.group_by, &context_map);
    Ok(())
}

fn cmd_diff(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    git_ref: &str,
    tag_filter: &[String],
    context_lines: Option<usize>,
    no_cache: bool,
) -> Result<()> {
    let current = do_scan(root, config, no_cache)?;
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

    let items: Vec<_> = diff_result.entries.iter().map(|e| e.item.clone()).collect();
    let context_map = if let Some(n) = context_lines {
        collect_context_map(root, &items, n)
    } else {
        std::collections::HashMap::new()
    };

    print_diff(&diff_result, format, &context_map);
    Ok(())
}

fn cmd_context(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    location: &str,
    n: usize,
    no_cache: bool,
) -> Result<()> {
    let (file, line) = parse_location(location)?;

    // Scan to find related TODOs in the same file
    let scan = do_scan(root, config, no_cache)?;
    let todos_in_file: Vec<&model::TodoItem> =
        scan.items.iter().filter(|i| i.file == file).collect();

    let rich = build_rich_context(root, &file, line, n, &todos_in_file)?;
    print_context(&rich, format);
    Ok(())
}

fn cmd_check(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    overrides: CheckOverrides,
    since: Option<String>,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

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

fn cmd_lint(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    overrides: LintOverrides,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;
    let result = run_lint(&scan, config, &overrides, root);
    let passed = result.passed;

    print_lint(&result, format);

    if !passed {
        process::exit(1);
    }

    Ok(())
}

fn cmd_clean(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    check_mode: bool,
    since: Option<String>,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    // Try to create GhIssueChecker; warn if gh is unavailable
    let gh_checker = clean::GhIssueChecker::new();
    if gh_checker.is_none() && config.clean.stale_issues.unwrap_or(true) {
        eprintln!("warning: gh CLI not found, skipping stale issue detection");
    }

    let result = clean::run_clean(
        &scan,
        config,
        gh_checker.as_ref().map(|c| c as &dyn clean::IssueChecker),
        since.as_deref(),
    );
    let has_violations = !result.passed;

    print_clean(&result, format);

    if check_mode && has_violations {
        process::exit(1);
    }

    Ok(())
}

fn cmd_stats(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    since: Option<String>,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    let diff = if let Some(ref base_ref) = since {
        Some(compute_diff(&scan, base_ref, root, config)?)
    } else {
        None
    };

    let result = compute_stats(&scan, diff.as_ref());
    print_stats(&result, format);
    Ok(())
}

fn cmd_report(
    root: &std::path::Path,
    config: &Config,
    output_path: &str,
    history_count: usize,
    stale_threshold_cli: Option<String>,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    let threshold_str = stale_threshold_cli
        .or_else(|| config.blame.stale_threshold.clone())
        .unwrap_or_else(|| "365d".to_string());
    let stale_threshold = blame::parse_duration_days(&threshold_str)?;

    let result = report::compute_report(&scan, root, config, history_count, stale_threshold)?;
    print_report(&result, output_path)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_blame(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    sort: BlameSortBy,
    author_filter: Option<String>,
    min_age: Option<String>,
    stale_threshold_cli: Option<String>,
    tag_filter: Vec<String>,
    path_filter: Option<String>,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    // Resolve stale threshold: CLI > config > default (365d)
    let threshold_str = stale_threshold_cli
        .or_else(|| config.blame.stale_threshold.clone())
        .unwrap_or_else(|| "365d".to_string());
    let stale_threshold = blame::parse_duration_days(&threshold_str)?;

    let mut result = compute_blame(&scan, root, stale_threshold)?;

    // Apply tag filter
    if !tag_filter.is_empty() {
        let filter_tags: Vec<Tag> = tag_filter
            .iter()
            .filter_map(|s| s.parse::<Tag>().ok())
            .collect();
        result.entries.retain(|e| filter_tags.contains(&e.item.tag));
    }

    // Apply author filter (substring match)
    if let Some(ref author) = author_filter {
        let lower = author.to_lowercase();
        result
            .entries
            .retain(|e| e.blame.author.to_lowercase().contains(&lower));
    }

    // Apply min-age filter
    if let Some(ref age_str) = min_age {
        let min_days = blame::parse_duration_days(age_str)?;
        result.entries.retain(|e| e.blame.age_days >= min_days);
    }

    // Apply path filter
    if let Some(ref pattern) = path_filter {
        let glob = globset::Glob::new(pattern)
            .context("invalid glob pattern")?
            .compile_matcher();
        result.entries.retain(|e| glob.is_match(&e.item.file));
    }

    // Apply sort
    match sort {
        BlameSortBy::File => result.entries.sort_by(|a, b| {
            a.item
                .file
                .cmp(&b.item.file)
                .then(a.item.line.cmp(&b.item.line))
        }),
        BlameSortBy::Age => result
            .entries
            .sort_by(|a, b| b.blame.age_days.cmp(&a.blame.age_days)),
        BlameSortBy::Author => result
            .entries
            .sort_by(|a, b| a.blame.author.cmp(&b.blame.author)),
        BlameSortBy::Tag => result
            .entries
            .sort_by(|a, b| a.item.tag.severity().cmp(&b.item.tag.severity()).reverse()),
    }

    // Recompute summary after filtering
    result.total = result.entries.len();
    result.stale_count = result.entries.iter().filter(|e| e.stale).count();
    result.avg_age_days = if result.total > 0 {
        result.entries.iter().map(|e| e.blame.age_days).sum::<u64>() / result.total as u64
    } else {
        0
    };

    print_blame(&result, format);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_tasks(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    tag_filter: Vec<String>,
    context_lines: usize,
    output_dir: Option<std::path::PathBuf>,
    dry_run: bool,
    since: Option<String>,
    priority_filter: Vec<PriorityFilter>,
    author_filter: Option<String>,
    path_filter: Option<String>,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    let mut items = if let Some(ref base_ref) = since {
        // Only TODOs added since the git ref
        let diff = compute_diff(&scan, base_ref, root, config)?;
        diff.entries
            .into_iter()
            .filter(|e| matches!(e.status, model::DiffStatus::Added))
            .map(|e| e.item)
            .collect()
    } else {
        scan.items
    };

    // Apply tag filter
    if !tag_filter.is_empty() {
        let filter_tags: Vec<Tag> = tag_filter
            .iter()
            .filter_map(|s| s.parse::<Tag>().ok())
            .collect();
        items.retain(|item| filter_tags.contains(&item.tag));
    }

    // Apply priority filter
    if !priority_filter.is_empty() {
        let priorities: Vec<model::Priority> =
            priority_filter.iter().map(|p| p.to_priority()).collect();
        items.retain(|item| priorities.contains(&item.priority));
    }

    // Apply author filter
    if let Some(ref author) = author_filter {
        items.retain(|item| item.author.as_deref() == Some(author.as_str()));
    }

    // Apply path filter
    if let Some(ref pattern) = path_filter {
        let glob = globset::Glob::new(pattern)
            .context("invalid glob pattern")?
            .compile_matcher();
        items.retain(|item| glob.is_match(&item.file));
    }

    // Sort by priority
    tasks::sort_by_priority(&mut items);

    // Collect context
    let context_map = collect_context_map(root, &items, context_lines);

    // Build tasks
    let claude_tasks = tasks::build_tasks(&items, &context_map);
    let total = claude_tasks.len();

    // Output
    match output_dir {
        Some(dir) if !dry_run => {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("cannot create output directory: {}", dir.display()))?;

            for (i, task) in claude_tasks.iter().enumerate() {
                let filename = format!("task-{:04}.json", i + 1);
                let path = dir.join(&filename);
                let json =
                    serde_json::to_string_pretty(task).context("failed to serialize task")?;
                std::fs::write(&path, json)
                    .with_context(|| format!("cannot write task file: {}", path.display()))?;
            }

            let result = model::TasksResult {
                tasks: claude_tasks,
                total,
                output_dir: Some(dir.to_string_lossy().to_string()),
            };
            print_tasks(&result, format);
        }
        _ => {
            let result = model::TasksResult {
                tasks: claude_tasks,
                total,
                output_dir: None,
            };
            print_tasks(&result, format);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_relate(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    cluster: bool,
    for_item: Option<String>,
    min_score: f64,
    proximity: usize,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;
    let mut result = relate::compute_relations(&scan, min_score, proximity);

    if let Some(ref location) = for_item {
        let (file, line) = parse_location(location)?;
        result = relate::filter_for_item(result, &file, line);
    }

    if cluster {
        let clusters = relate::build_clusters(&result.relationships, &scan.items);
        result.clusters = Some(clusters);
    }

    print_relate(&result, format);
    Ok(())
}

/// Resolve a `--package` flag to an absolute scan root path via workspace detection.
fn resolve_package_root(
    root: &std::path::Path,
    config: &Config,
    package: Option<&str>,
) -> Result<std::path::PathBuf> {
    let pkg_name = match package {
        Some(name) => name,
        None => return Ok(root.to_path_buf()),
    };

    let ws = workspace::detect_workspace(root, config)?
        .ok_or_else(|| anyhow::anyhow!("no workspace detected"))?;

    let pkg = ws
        .packages
        .iter()
        .find(|p| p.name == pkg_name)
        .ok_or_else(|| {
            let names: Vec<_> = ws.packages.iter().map(|p| p.name.as_str()).collect();
            anyhow::anyhow!(
                "package '{}' not found in workspace. Available: {}",
                pkg_name,
                names.join(", ")
            )
        })?;

    Ok(root.join(&pkg.path))
}

fn cmd_workspace_list(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    no_cache: bool,
) -> Result<()> {
    let ws = workspace::detect_workspace(root, config)?
        .ok_or_else(|| anyhow::anyhow!("no workspace detected"))?;

    let mut summaries = Vec::new();
    let mut total_todos = 0;

    for pkg in &ws.packages {
        let pkg_root = root.join(&pkg.path);
        let scan = do_scan(&pkg_root, config, no_cache)?;
        let todo_count = scan.items.len();
        total_todos += todo_count;

        let max = config.workspace.packages.get(&pkg.name).and_then(|c| c.max);

        let status = match max {
            Some(m) if todo_count > m => model::PackageStatus::Over,
            Some(_) => model::PackageStatus::Ok,
            None => model::PackageStatus::Uncapped,
        };

        summaries.push(model::PackageScanSummary {
            name: pkg.name.clone(),
            path: pkg.path.clone(),
            todo_count,
            max,
            status,
        });
    }

    let result = model::WorkspaceResult {
        total_packages: summaries.len(),
        packages: summaries,
        total_todos,
    };

    print_workspace_list(&result, format, &ws.kind);
    Ok(())
}

fn cmd_workspace_check(
    root: &std::path::Path,
    config: &Config,
    format: &Format,
    no_cache: bool,
) -> Result<()> {
    let ws = workspace::detect_workspace(root, config)?
        .ok_or_else(|| anyhow::anyhow!("no workspace detected"))?;

    let mut all_passed = true;
    let mut violations = Vec::new();

    for pkg in &ws.packages {
        let pkg_root = root.join(&pkg.path);
        let scan = do_scan(&pkg_root, config, no_cache)?;
        let todo_count = scan.items.len();

        let pkg_config = config.workspace.packages.get(&pkg.name);

        if let Some(pc) = pkg_config {
            if let Some(max) = pc.max {
                if todo_count > max {
                    all_passed = false;
                    violations.push(model::CheckViolation {
                        rule: "workspace/max".to_string(),
                        message: format!(
                            "package '{}' has {} TODOs (max: {})",
                            pkg.name, todo_count, max
                        ),
                    });
                }
            }

            if !pc.block_tags.is_empty() {
                for item in &scan.items {
                    if pc
                        .block_tags
                        .iter()
                        .any(|t| t.eq_ignore_ascii_case(item.tag.as_str()))
                    {
                        all_passed = false;
                        violations.push(model::CheckViolation {
                            rule: "workspace/block-tag".to_string(),
                            message: format!(
                                "package '{}': forbidden tag {} at {}:{}",
                                pkg.name, item.tag, item.file, item.line
                            ),
                        });
                    }
                }
            }
        }
    }

    let result = model::CheckResult {
        passed: all_passed,
        total: violations.len(),
        violations,
    };

    print_check(&result, format);

    if !all_passed {
        process::exit(1);
    }

    Ok(())
}
