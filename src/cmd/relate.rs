use std::path::Path;

use anyhow::Result;

use crate::cli::Format;
use crate::config::Config;
use crate::context::parse_location;
use crate::output::print_relate;
use crate::relate;

use super::do_scan;

pub struct RelateOptions {
    pub cluster: bool,
    pub for_item: Option<String>,
    pub min_score: f64,
    pub proximity: usize,
}

pub fn cmd_relate(
    root: &Path,
    config: &Config,
    format: &Format,
    opts: RelateOptions,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;
    let mut result = relate::compute_relations(&scan, opts.min_score, opts.proximity);

    if let Some(ref location) = opts.for_item {
        let (file, line) = parse_location(location)?;
        result = relate::filter_for_item(result, &file, line);
    }

    if opts.cluster {
        let clusters = relate::build_clusters(&result.relationships, &scan.items);
        result.clusters = Some(clusters);
    }

    print_relate(&result, format);
    Ok(())
}
