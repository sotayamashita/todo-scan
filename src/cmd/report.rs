use std::path::Path;

use anyhow::Result;

use crate::blame;
use crate::config::Config;
use crate::output::print_report;
use crate::report;

use super::do_scan;

pub fn cmd_report(
    root: &Path,
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
