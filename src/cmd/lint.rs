use std::path::Path;
use std::process;

use anyhow::Result;

use crate::cli::Format;
use crate::config::Config;
use crate::lint::{run_lint, LintOverrides};
use crate::output::print_lint;

use super::do_scan;

pub fn cmd_lint(
    root: &Path,
    config: &Config,
    format: &Format,
    overrides: LintOverrides,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;
    let result = run_lint(&scan, config, &overrides, root);
    let passed = result.passed;

    print_lint(&result, format);

    if !passed {
        process::exit(1);
    }

    Ok(())
}
