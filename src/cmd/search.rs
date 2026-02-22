use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};

use crate::cli::{Format, GroupBy, SortBy};
use crate::config::Config;
use crate::context::collect_context_map;
use crate::model::Tag;
use crate::output::print_search;
use crate::search::search_items;

use super::do_scan;

pub struct SearchOptions {
    pub query: String,
    pub exact: bool,
    pub context: Option<usize>,
    pub author: Option<String>,
    pub tag: Vec<String>,
    pub path: Option<String>,
    pub sort: SortBy,
    pub group_by: GroupBy,
}

pub fn cmd_search(
    root: &Path,
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
            b.priority
                .cmp(&a.priority)
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
        .collect::<HashSet<_>>()
        .len();

    let context_map = if let Some(n) = opts.context {
        collect_context_map(root, &result.items, n)
    } else {
        HashMap::new()
    };

    print_search(&result, format, &opts.group_by, &context_map);
    Ok(())
}
