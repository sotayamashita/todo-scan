use std::path::Path;
use std::process;

use anyhow::Result;

use crate::check::{run_check, CheckOverrides};
use crate::cli::Format;
use crate::config::Config;
use crate::deadline;
use crate::diff::compute_diff;
use crate::model;
use crate::output::print_check;
use crate::workspace;

use super::do_scan;

pub fn cmd_check(
    root: &Path,
    config: &Config,
    format: &Format,
    overrides: CheckOverrides,
    since: Option<String>,
    no_cache: bool,
) -> Result<()> {
    let scan = do_scan(root, config, no_cache)?;

    let diff = if let Some(ref base_ref) = since {
        Some(compute_diff(&scan, base_ref, root, config)?)
    } else {
        None
    };

    let today = deadline::today();
    let result = run_check(&scan, diff.as_ref(), config, &overrides, &today);
    let passed = result.passed;

    print_check(&result, format);

    if !passed {
        process::exit(1);
    }

    Ok(())
}

pub fn cmd_workspace_check(
    root: &Path,
    config: &Config,
    format: &Format,
    no_cache: bool,
) -> Result<()> {
    let ws = workspace::detect_workspace(root, config)?
        .ok_or_else(|| anyhow::anyhow!("no workspace detected"))?;

    let mut all_passed = true;
    let mut violations = Vec::new();

    for pkg in &ws.packages {
        let pkg_root = root.join(&pkg.path);
        let scan = do_scan(&pkg_root, config, no_cache)?;
        let todo_count = scan.items.len();

        let pkg_config = config.workspace.packages.get(&pkg.name);

        if let Some(pc) = pkg_config {
            if let Some(max) = pc.max {
                if todo_count > max {
                    all_passed = false;
                    violations.push(model::CheckViolation {
                        rule: "workspace/max".to_string(),
                        message: format!(
                            "package '{}' has {} TODOs (max: {})",
                            pkg.name, todo_count, max
                        ),
                    });
                }
            }

            if !pc.block_tags.is_empty() {
                for item in &scan.items {
                    if pc
                        .block_tags
                        .iter()
                        .any(|t| t.eq_ignore_ascii_case(item.tag.as_str()))
                    {
                        all_passed = false;
                        violations.push(model::CheckViolation {
                            rule: "workspace/block-tag".to_string(),
                            message: format!(
                                "package '{}': forbidden tag {} at {}:{}",
                                pkg.name, item.tag, item.file, item.line
                            ),
                        });
                    }
                }
            }
        }
    }

    let result = model::CheckResult {
        passed: all_passed,
        total: violations.len(),
        violations,
    };

    print_check(&result, format);

    if !all_passed {
        process::exit(1);
    }

    Ok(())
}
