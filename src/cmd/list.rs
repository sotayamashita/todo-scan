use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::cli::{Format, GroupBy, PriorityFilter, SortBy};
use crate::config::Config;
use crate::context::collect_context_map;
use crate::model;
use crate::model::Tag;
use crate::output::print_list;

use super::do_scan;

pub struct ListOptions {
    pub tag: Vec<String>,
    pub sort: SortBy,
    pub group_by: GroupBy,
    pub priority: Vec<PriorityFilter>,
    pub author: Option<String>,
    pub path: Option<String>,
    pub limit: Option<usize>,
    pub context: Option<usize>,
}

pub fn cmd_list(
    root: &Path,
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
            b.priority
                .cmp(&a.priority)
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
        HashMap::new()
    };

    print_list(&result, format, &opts.group_by, &context_map);
    Ok(())
}
