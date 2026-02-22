use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;

use crate::cli::{DetailLevel, Format, GroupBy, SortBy};
use crate::config::Config;
use crate::context::collect_context_map;
use crate::output::print_search;
use crate::search::search_items;

use super::do_scan;
use super::filter::{apply_filters, FilterOptions};

pub struct SearchOptions {
    pub query: String,
    pub exact: bool,
    pub context: Option<usize>,
    pub author: Option<String>,
    pub tag: Vec<String>,
    pub path: Option<String>,
    pub sort: SortBy,
    pub group_by: GroupBy,
    pub detail: DetailLevel,
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

    apply_filters(
        &mut result.items,
        &FilterOptions {
            tags: opts.tag,
            author: opts.author,
            path: opts.path,
            priority: vec![],
        },
    )?;

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
    } else if opts.detail == DetailLevel::Full {
        collect_context_map(root, &result.items, 3)
    } else {
        HashMap::new()
    };

    print_search(&result, format, &opts.group_by, &context_map, &opts.detail);
    Ok(())
}
