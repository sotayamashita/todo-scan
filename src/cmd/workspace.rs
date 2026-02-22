use std::path::Path;

use anyhow::Result;

use crate::cli::Format;
use crate::config::Config;
use crate::model;
use crate::output::print_workspace_list;
use crate::workspace;

use super::do_scan;

pub fn cmd_workspace_list(
    root: &Path,
    config: &Config,
    format: &Format,
    no_cache: bool,
) -> Result<()> {
    let ws = workspace::detect_workspace(root, config)?
        .ok_or_else(|| anyhow::anyhow!("no workspace detected"))?;

    let mut summaries = Vec::new();
    let mut total_todos = 0;

    for pkg in &ws.packages {
        let pkg_root = root.join(&pkg.path);
        let scan = do_scan(&pkg_root, config, no_cache)?;
        let todo_count = scan.items.len();
        total_todos += todo_count;

        let max = config.workspace.packages.get(&pkg.name).and_then(|c| c.max);

        let status = match max {
            Some(m) if todo_count > m => model::PackageStatus::Over,
            Some(_) => model::PackageStatus::Ok,
            None => model::PackageStatus::Uncapped,
        };

        summaries.push(model::PackageScanSummary {
            name: pkg.name.clone(),
            path: pkg.path.clone(),
            todo_count,
            max,
            status,
        });
    }

    let result = model::WorkspaceResult {
        total_packages: summaries.len(),
        packages: summaries,
        total_todos,
    };

    print_workspace_list(&result, format, &ws.kind);
    Ok(())
}
