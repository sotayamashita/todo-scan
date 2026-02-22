use std::path::Path;

use anyhow::{Context, Result};

use crate::cli::{Format, PriorityFilter};
use crate::config::Config;
use crate::context::collect_context_map;
use crate::diff::compute_diff;
use crate::model;
use crate::model::Tag;
use crate::output::print_tasks;
use crate::tasks;

use super::do_scan;

pub struct TasksOptions {
    pub tag: Vec<String>,
    pub context: usize,
    pub output: Option<std::path::PathBuf>,
    pub dry_run: bool,
    pub since: Option<String>,
    pub priority: Vec<PriorityFilter>,
    pub author: Option<String>,
    pub path: Option<String>,
}

pub fn cmd_tasks(
    root: &Path,
    config: &Config,
    format: &Format,
    opts: TasksOptions,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    let mut items = if let Some(ref base_ref) = opts.since {
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
    if !opts.tag.is_empty() {
        let filter_tags: Vec<Tag> = opts
            .tag
            .iter()
            .filter_map(|s| s.parse::<Tag>().ok())
            .collect();
        items.retain(|item| filter_tags.contains(&item.tag));
    }

    // Apply priority filter
    if !opts.priority.is_empty() {
        let priorities: Vec<model::Priority> =
            opts.priority.iter().map(|p| p.to_priority()).collect();
        items.retain(|item| priorities.contains(&item.priority));
    }

    // Apply author filter
    if let Some(ref author) = opts.author {
        items.retain(|item| item.author.as_deref() == Some(author.as_str()));
    }

    // Apply path filter
    if let Some(ref pattern) = opts.path {
        let glob = globset::Glob::new(pattern)
            .context("invalid glob pattern")?
            .compile_matcher();
        items.retain(|item| glob.is_match(&item.file));
    }

    // Sort by priority
    tasks::sort_by_priority(&mut items);

    // Collect context
    let context_map = collect_context_map(root, &items, opts.context);

    // Build tasks
    let claude_tasks = tasks::build_tasks(&items, &context_map);
    let total = claude_tasks.len();

    // Output
    match opts.output {
        Some(dir) if !opts.dry_run => {
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
