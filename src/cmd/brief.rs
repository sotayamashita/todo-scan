use std::path::Path;

use anyhow::Result;

use crate::brief::compute_brief;
use crate::cli::Format;
use crate::config::Config;
use crate::diff::compute_diff;
use crate::output::print_brief;

use super::do_scan;

pub fn cmd_brief(
    root: &Path,
    config: &Config,
    format: &Format,
    since: Option<String>,
    budget: Option<usize>,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    let diff = if let Some(ref base_ref) = since {
        Some(compute_diff(&scan, base_ref, root, config)?)
    } else {
        None
    };

    let result = compute_brief(&scan, diff.as_ref());
    print_brief(&result, format, budget);
    Ok(())
}
