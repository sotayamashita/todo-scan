use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::cli::{Format, GroupBy, PriorityFilter, SortBy};
use crate::config::Config;
use crate::context::collect_context_map;
use crate::output::print_list;

use super::do_scan;
use super::filter::{apply_filters, FilterOptions};

pub struct ListOptions {
    pub tag: Vec<String>,
    pub sort: SortBy,
    pub group_by: GroupBy,
    pub priority: Vec<PriorityFilter>,
    pub author: Option<String>,
    pub path: Option<String>,
    pub limit: Option<usize>,
    pub context: Option<usize>,
    pub show_ignored: bool,
}

pub fn cmd_list(
    root: &Path,
    config: &Config,
    format: &Format,
    opts: ListOptions,
    no_cache: bool,
) -> Result<()> {
    let mut result = do_scan(root, config, no_cache)?;

    let ignored_count = result.ignored_items.len();

    apply_filters(
        &mut result.items,
        &FilterOptions {
            tags: opts.tag,
            author: opts.author,
            path: opts.path,
            priority: opts.priority,
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

    // Apply limit
    if let Some(n) = opts.limit {
        result.items.truncate(n);
    }

    let context_map = if let Some(n) = opts.context {
        collect_context_map(root, &result.items, n)
    } else {
        HashMap::new()
    };

    print_list(
        &result,
        format,
        &opts.group_by,
        &context_map,
        ignored_count,
        opts.show_ignored,
    );
    Ok(())
}
