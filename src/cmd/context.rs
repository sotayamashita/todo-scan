use std::path::Path;

use anyhow::Result;

use crate::cli::Format;
use crate::config::Config;
use crate::context::{build_rich_context, parse_location};
use crate::model;
use crate::output::print_context;

use super::do_scan;

pub fn cmd_context(
    root: &Path,
    config: &Config,
    format: &Format,
    location: &str,
    n: usize,
    no_cache: bool,
) -> Result<()> {
    let (file, line) = parse_location(location)?;

    // Scan to find related TODOs in the same file
    let scan = do_scan(root, config, no_cache)?;
    let todos_in_file: Vec<&model::TodoItem> =
        scan.items.iter().filter(|i| i.file == file).collect();

    let rich = build_rich_context(root, &file, line, n, &todos_in_file)?;
    print_context(&rich, format);
    Ok(())
}
