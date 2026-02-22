use std::path::Path;

use anyhow::{Context, Result};

use crate::blame::{compute_blame, parse_duration_days};
use crate::cli::{BlameSortBy, Format};
use crate::config::Config;
use crate::model::Tag;
use crate::output::print_blame;

use super::do_scan;

pub struct BlameOptions {
    pub sort: BlameSortBy,
    pub author: Option<String>,
    pub min_age: Option<String>,
    pub stale_threshold: Option<String>,
    pub tag: Vec<String>,
    pub path: Option<String>,
}

pub fn cmd_blame(
    root: &Path,
    config: &Config,
    format: &Format,
    opts: BlameOptions,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    // Resolve stale threshold: CLI > config > default (365d)
    let threshold_str = opts
        .stale_threshold
        .or_else(|| config.blame.stale_threshold.clone())
        .unwrap_or_else(|| "365d".to_string());
    let stale_threshold = parse_duration_days(&threshold_str)?;

    let mut result = compute_blame(&scan, root, stale_threshold)?;

    // Apply tag filter
    if !opts.tag.is_empty() {
        let filter_tags: Vec<Tag> = opts
            .tag
            .iter()
            .filter_map(|s| s.parse::<Tag>().ok())
            .collect();
        result.entries.retain(|e| filter_tags.contains(&e.item.tag));
    }

    // Apply author filter (substring match)
    if let Some(ref author) = opts.author {
        let lower = author.to_lowercase();
        result
            .entries
            .retain(|e| e.blame.author.to_lowercase().contains(&lower));
    }

    // Apply min-age filter
    if let Some(ref age_str) = opts.min_age {
        let min_days = parse_duration_days(age_str)?;
        result.entries.retain(|e| e.blame.age_days >= min_days);
    }

    // Apply path filter
    if let Some(ref pattern) = opts.path {
        let glob = globset::Glob::new(pattern)
            .context("invalid glob pattern")?
            .compile_matcher();
        result.entries.retain(|e| glob.is_match(&e.item.file));
    }

    // Apply sort
    match opts.sort {
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
