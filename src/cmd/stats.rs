use std::path::Path;

use anyhow::Result;

use crate::cli::Format;
use crate::config::Config;
use crate::diff::compute_diff;
use crate::output::print_stats;
use crate::stats::compute_stats;

use super::do_scan;

pub fn cmd_stats(
    root: &Path,
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
