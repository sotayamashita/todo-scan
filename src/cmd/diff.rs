use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::cli::Format;
use crate::config::Config;
use crate::context::collect_context_map;
use crate::diff::compute_diff;
use crate::model::{DiffStatus, Tag};
use crate::output::print_diff;

use super::do_scan;

pub fn cmd_diff(
    root: &Path,
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
            .filter(|e| matches!(e.status, DiffStatus::Added))
            .count();
        diff_result.removed_count = diff_result
            .entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Removed))
            .count();
    }

    let items: Vec<_> = diff_result.entries.iter().map(|e| e.item.clone()).collect();
    let context_map = if let Some(n) = context_lines {
        collect_context_map(root, &items, n)
    } else {
        HashMap::new()
    };

    print_diff(&diff_result, format, &context_map);
    Ok(())
}
