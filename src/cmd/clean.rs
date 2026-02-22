use std::path::Path;
use std::process;

use anyhow::Result;

use crate::clean;
use crate::cli::Format;
use crate::config::Config;
use crate::output::print_clean;

use super::do_scan;

pub fn cmd_clean(
    root: &Path,
    config: &Config,
    format: &Format,
    check_mode: bool,
    since: Option<String>,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    // Try to create GhIssueChecker; warn if gh is unavailable
    let gh_checker = clean::GhIssueChecker::new();
    if gh_checker.is_none() && config.clean.stale_issues.unwrap_or(true) {
        eprintln!("warning: gh CLI not found, skipping stale issue detection");
    }

    let result = clean::run_clean(
        &scan,
        config,
        gh_checker.as_ref().map(|c| c as &dyn clean::IssueChecker),
        since.as_deref(),
    );
    let has_violations = !result.passed;

    print_clean(&result, format);

    if check_mode && has_violations {
        process::exit(1);
    }

    Ok(())
}
