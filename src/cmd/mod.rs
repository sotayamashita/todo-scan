mod blame;
mod check;
mod clean;
mod context;
mod diff;
mod filter;
mod lint;
mod list;
mod relate;
mod report;
mod search;
mod stats;
mod tasks;
mod workspace;

pub use self::blame::{cmd_blame, BlameOptions};
pub use self::check::{cmd_check, cmd_workspace_check};
pub use self::clean::cmd_clean;
pub use self::context::cmd_context;
pub use self::diff::{cmd_diff, DiffOptions};
pub use self::lint::cmd_lint;
pub use self::list::{cmd_list, ListOptions};
pub use self::relate::{cmd_relate, RelateOptions};
pub use self::report::cmd_report;
pub use self::search::{cmd_search, SearchOptions};
pub use self::stats::cmd_stats;
pub use self::tasks::{cmd_tasks, TasksOptions};
pub use self::workspace::cmd_workspace_list;

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::cache;
use crate::config::Config;
use crate::model;
use crate::scanner;
use crate::workspace as ws;

/// Perform a directory scan, optionally using cache for performance.
pub(crate) fn do_scan(root: &Path, config: &Config, no_cache: bool) -> Result<model::ScanResult> {
    if no_cache {
        return scanner::scan_directory(root, config);
    }

    let config_hash = cache::ScanCache::config_hash(config);

    let mut scan_cache = cache::ScanCache::load(root)
        .filter(|c| c.config_hash == config_hash)
        .unwrap_or_else(|| cache::ScanCache::new(config_hash));

    let cached_result = scanner::scan_directory_cached(root, config, &mut scan_cache)?;

    // Best-effort save; don't fail the scan if cache write fails
    let _ = scan_cache.save(root);

    Ok(cached_result.result)
}

/// Resolve a `--package` flag to an absolute scan root path via workspace detection.
pub fn resolve_package_root(
    root: &Path,
    config: &Config,
    package: Option<&str>,
) -> Result<PathBuf> {
    let pkg_name = match package {
        Some(name) => name,
        None => return Ok(root.to_path_buf()),
    };

    let workspace = ws::detect_workspace(root, config)?
        .ok_or_else(|| anyhow::anyhow!("no workspace detected"))?;

    let pkg = workspace
        .packages
        .iter()
        .find(|p| p.name == pkg_name)
        .ok_or_else(|| {
            let names: Vec<_> = workspace.packages.iter().map(|p| p.name.as_str()).collect();
            anyhow::anyhow!(
                "package '{}' not found in workspace. Available: {}",
                pkg_name,
                names.join(", ")
            )
        })?;

    Ok(root.join(&pkg.path))
}
