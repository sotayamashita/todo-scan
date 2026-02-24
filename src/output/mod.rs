mod github_actions;
pub mod html;
mod markdown;
mod sarif;

use std::collections::HashMap;

use colored::*;

use crate::cli::{DetailLevel, Format, GroupBy};
use crate::context::{ContextInfo, RichContext};
use crate::model::*;
use std::path::Path;

/// Apply detail-level transformations to a flat JSON item (TodoItem-shaped object).
/// - Always: inject stable `id` field
/// - Minimal: remove author, issue_ref, priority, deadline
/// - Full: inject match_key (backward compatibility)
fn apply_detail_to_json_item(item_val: &mut serde_json::Value, detail: &DetailLevel) {
    inject_id_field(item_val);

    if *detail == DetailLevel::Minimal {
        let obj = item_val.as_object_mut().unwrap();
        obj.remove("author");
        obj.remove("issue_ref");
        obj.remove("priority");
        obj.remove("deadline");
    }
    if *detail == DetailLevel::Full {
        let id = item_val["id"].as_str().unwrap_or("").to_string();
        item_val
            .as_object_mut()
            .unwrap()
            .insert("match_key".to_string(), serde_json::Value::String(id));
    }
}

fn colorize_tag(tag: &Tag) -> ColoredString {
    match tag {
        Tag::Todo => tag.as_str().yellow(),
        Tag::Fixme => tag.as_str().red(),
        Tag::Hack => tag.as_str().magenta(),
        Tag::Bug => tag.as_str().red().bold(),
        Tag::Note => tag.as_str().blue(),
        Tag::Xxx => tag.as_str().red(),
    }
}

fn group_key(item: &TodoItem, group_by: &GroupBy) -> String {
    match group_by {
        GroupBy::File => item.file.clone(),
        GroupBy::Tag => item.tag.as_str().to_string(),
        GroupBy::Priority => match item.priority {
            Priority::Urgent => "!! Urgent".to_string(),
            Priority::High => "! High".to_string(),
            Priority::Normal => "Normal".to_string(),
        },
        GroupBy::Author => item
            .author
            .clone()
            .unwrap_or_else(|| "unassigned".to_string()),
        GroupBy::Dir => Path::new(&item.file)
            .parent()
            .map(|p| {
                let s = p.to_string_lossy().to_string();
                if s.is_empty() {
                    ".".to_string()
                } else {
                    s
                }
            })
            .unwrap_or_else(|| ".".to_string()),
    }
}

fn group_items<'a>(items: &'a [TodoItem], group_by: &GroupBy) -> Vec<(String, Vec<&'a TodoItem>)> {
    let mut groups: Vec<(String, Vec<&'a TodoItem>)> = Vec::new();
    let mut key_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for item in items {
        let key = group_key(item, group_by);
        if let Some(&idx) = key_index.get(&key) {
            groups[idx].1.push(item);
        } else {
            key_index.insert(key.clone(), groups.len());
            groups.push((key, vec![item]));
        }
    }

    // Sort groups based on GroupBy variant
    match group_by {
        GroupBy::Priority => {
            let priority_order = |key: &str| -> u8 {
                match key {
                    "!! Urgent" => 0,
                    "! High" => 1,
                    "Normal" => 2,
                    _ => 3,
                }
            };
            groups.sort_by(|a, b| priority_order(&a.0).cmp(&priority_order(&b.0)));
        }
        GroupBy::Tag => {
            groups.sort_by(|a, b| {
                let sa = a.1.first().map(|i| i.tag.severity()).unwrap_or(0);
                let sb = b.1.first().map(|i| i.tag.severity()).unwrap_or(0);
                sb.cmp(&sa)
            });
        }
        _ => {
            groups.sort_by(|a, b| a.0.cmp(&b.0));
        }
    }

    groups
}

pub fn print_list(
    result: &ScanResult,
    format: &Format,
    group_by: &GroupBy,
    context_map: &HashMap<String, ContextInfo>,
    ignored_count: usize,
    show_ignored: bool,
    detail: &DetailLevel,
) {
    let has_context = !context_map.is_empty();

    match format {
        Format::Text => {
            let groups = group_items(&result.items, group_by);
            let group_count = groups.len();
            let is_file_group = matches!(group_by, GroupBy::File);

            for (key, items) in &groups {
                if is_file_group {
                    println!("{}", key.bold().underline());
                } else {
                    println!(
                        "{}",
                        format!("{} ({} items)", key, items.len())
                            .bold()
                            .underline()
                    );
                }
                for item in items {
                    let tag_str = colorize_tag(&item.tag);

                    // Print before-context lines
                    let ctx_key = format!("{}:{}", item.file, item.line);
                    if let Some(ctx) = context_map.get(&ctx_key) {
                        for cl in &ctx.before {
                            println!(
                                "    {} {}",
                                format!("{:>4}", cl.line_number).dimmed(),
                                sanitize_for_terminal(&cl.content).dimmed()
                            );
                        }
                    }

                    let msg = sanitize_for_terminal(&item.message);
                    let file = sanitize_for_terminal(&item.file);
                    let mut line = if is_file_group {
                        format!("  L{}: [{}] {}", item.line, tag_str, msg)
                    } else {
                        format!("  {}:{}: [{}] {}", file, item.line, tag_str, msg)
                    };

                    if *detail != DetailLevel::Minimal {
                        if let Some(ref author) = item.author {
                            line.push_str(&format!(" (@{})", sanitize_for_terminal(author)));
                        }
                        if let Some(ref issue) = item.issue_ref {
                            line.push_str(&format!(" ({})", sanitize_for_terminal(issue)));
                        }
                        if let Some(ref deadline) = item.deadline {
                            let today = crate::deadline::today();
                            if deadline.is_expired(&today) {
                                line.push_str(&format!(
                                    " {}",
                                    format!("[expired: {}]", deadline).red()
                                ));
                            } else {
                                line.push_str(&format!(" [deadline: {}]", deadline));
                            }
                        }
                    }

                    if has_context {
                        println!("{} {}", "  →".cyan(), line.trim_start());
                    } else {
                        println!("{}", line);
                    }

                    // Print after-context lines
                    if let Some(ctx) = context_map.get(&ctx_key) {
                        for cl in &ctx.after {
                            println!(
                                "    {} {}",
                                format!("{:>4}", cl.line_number).dimmed(),
                                sanitize_for_terminal(&cl.content).dimmed()
                            );
                        }
                        println!();
                    }
                }
            }

            // Show ignored items section
            if show_ignored && !result.ignored_items.is_empty() {
                println!();
                println!("{}", "Ignored items".bold().underline());
                let ignored_groups = group_items(&result.ignored_items, group_by);
                for (key, items) in &ignored_groups {
                    if is_file_group {
                        println!("{}", key.dimmed());
                    } else {
                        println!("{}", format!("{} ({} items)", key, items.len()).dimmed());
                    }
                    for item in items {
                        let tag_str = colorize_tag(&item.tag);
                        let msg = sanitize_for_terminal(&item.message);
                        let file = sanitize_for_terminal(&item.file);
                        let line = if is_file_group {
                            format!("  L{}: [{}] {}", item.line, tag_str, msg)
                        } else {
                            format!("  {}:{}: [{}] {}", file, item.line, tag_str, msg)
                        };
                        println!("{}", line.dimmed());
                    }
                }
            }

            // Summary line
            let ignored_suffix = if ignored_count > 0 {
                format!(" ({} ignored)", ignored_count)
            } else {
                String::new()
            };

            if is_file_group {
                println!(
                    "{} items in {} files{}",
                    result.items.len(),
                    group_count,
                    ignored_suffix
                );
            } else {
                println!(
                    "{} items in {} groups{}",
                    result.items.len(),
                    group_count,
                    ignored_suffix
                );
            }
        }
        Format::Json => {
            let mut value: serde_json::Value =
                serde_json::to_value(result).expect("failed to serialize");
            if let Some(items) = value.get_mut("items").and_then(|v| v.as_array_mut()) {
                for item_val in items.iter_mut() {
                    let file = item_val
                        .get("file")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let line = item_val.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                    let key = format!("{}:{}", file, line);
                    if has_context {
                        if let Some(ctx) = context_map.get(&key) {
                            let ctx_value =
                                serde_json::to_value(ctx).expect("failed to serialize context");
                            item_val
                                .as_object_mut()
                                .unwrap()
                                .insert("context".to_string(), ctx_value);
                        }
                    }
                    apply_detail_to_json_item(item_val, detail);
                }
            }
            let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
            println!("{}", json);
        }
        Format::GithubActions => print!("{}", github_actions::format_list(result)),
        Format::Sarif => print!("{}", sarif::format_list(result)),
        Format::Markdown => print!("{}", markdown::format_list(result)),
    }
}

pub fn print_search(
    result: &SearchResult,
    format: &Format,
    group_by: &GroupBy,
    context_map: &HashMap<String, ContextInfo>,
    detail: &DetailLevel,
) {
    let has_context = !context_map.is_empty();

    match format {
        Format::Text => {
            let groups = group_items(&result.items, group_by);
            let group_count = groups.len();
            let is_file_group = matches!(group_by, GroupBy::File);

            for (key, items) in &groups {
                if is_file_group {
                    println!("{}", key.bold().underline());
                } else {
                    println!(
                        "{}",
                        format!("{} ({} items)", key, items.len())
                            .bold()
                            .underline()
                    );
                }
                for item in items {
                    let tag_str = colorize_tag(&item.tag);

                    // Print before-context lines
                    let ctx_key = format!("{}:{}", item.file, item.line);
                    if let Some(ctx) = context_map.get(&ctx_key) {
                        for cl in &ctx.before {
                            println!(
                                "    {} {}",
                                format!("{:>4}", cl.line_number).dimmed(),
                                sanitize_for_terminal(&cl.content).dimmed()
                            );
                        }
                    }

                    let msg = sanitize_for_terminal(&item.message);
                    let file = sanitize_for_terminal(&item.file);
                    let mut line = if is_file_group {
                        format!("  L{}: [{}] {}", item.line, tag_str, msg)
                    } else {
                        format!("  {}:{}: [{}] {}", file, item.line, tag_str, msg)
                    };

                    if *detail != DetailLevel::Minimal {
                        if let Some(ref author) = item.author {
                            line.push_str(&format!(" (@{})", sanitize_for_terminal(author)));
                        }
                        if let Some(ref issue) = item.issue_ref {
                            line.push_str(&format!(" ({})", sanitize_for_terminal(issue)));
                        }
                        if let Some(ref deadline) = item.deadline {
                            let today = crate::deadline::today();
                            if deadline.is_expired(&today) {
                                line.push_str(&format!(
                                    " {}",
                                    format!("[expired: {}]", deadline).red()
                                ));
                            } else {
                                line.push_str(&format!(" [deadline: {}]", deadline));
                            }
                        }
                    }

                    if has_context {
                        println!("{} {}", "  →".cyan(), line.trim_start());
                    } else {
                        println!("{}", line);
                    }

                    // Print after-context lines
                    if let Some(ctx) = context_map.get(&ctx_key) {
                        for cl in &ctx.after {
                            println!(
                                "    {} {}",
                                format!("{:>4}", cl.line_number).dimmed(),
                                sanitize_for_terminal(&cl.content).dimmed()
                            );
                        }
                        println!();
                    }
                }
            }

            if is_file_group {
                println!(
                    "{} matches across {} files (query: \"{}\")",
                    result.match_count,
                    result.file_count,
                    sanitize_for_terminal(&result.query)
                );
            } else {
                println!(
                    "{} matches across {} groups (query: \"{}\")",
                    result.match_count,
                    group_count,
                    sanitize_for_terminal(&result.query)
                );
            }
        }
        Format::Json => {
            let mut value: serde_json::Value =
                serde_json::to_value(result).expect("failed to serialize");
            if let Some(items) = value.get_mut("items").and_then(|v| v.as_array_mut()) {
                for item_val in items.iter_mut() {
                    let file = item_val
                        .get("file")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let line = item_val.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                    let key = format!("{}:{}", file, line);
                    if has_context {
                        if let Some(ctx) = context_map.get(&key) {
                            let ctx_value =
                                serde_json::to_value(ctx).expect("failed to serialize context");
                            item_val
                                .as_object_mut()
                                .unwrap()
                                .insert("context".to_string(), ctx_value);
                        }
                    }
                    apply_detail_to_json_item(item_val, detail);
                }
            }
            let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
            println!("{}", json);
        }
        Format::GithubActions => print!("{}", github_actions::format_search(result)),
        Format::Sarif => print!("{}", sarif::format_search(result)),
        Format::Markdown => print!("{}", markdown::format_search(result)),
    }
}

pub fn print_diff(
    result: &DiffResult,
    format: &Format,
    context_map: &HashMap<String, ContextInfo>,
    detail: &DetailLevel,
) {
    let has_context = !context_map.is_empty();

    match format {
        Format::Text => {
            for entry in &result.entries {
                let (prefix, color): (&str, fn(&str) -> ColoredString) = match entry.status {
                    DiffStatus::Added => ("+", |s: &str| s.green()),
                    DiffStatus::Removed => ("-", |s: &str| s.red()),
                };

                // Print before-context
                let ctx_key = format!("{}:{}", entry.item.file, entry.item.line);
                if let Some(ctx) = context_map.get(&ctx_key) {
                    for cl in &ctx.before {
                        println!(
                            "    {} {}",
                            format!("{:>4}", cl.line_number).dimmed(),
                            sanitize_for_terminal(&cl.content).dimmed()
                        );
                    }
                }

                let tag_str = colorize_tag(&entry.item.tag);
                let line = format!(
                    "{} {}:{} [{}] {}",
                    prefix,
                    sanitize_for_terminal(&entry.item.file),
                    entry.item.line,
                    tag_str,
                    sanitize_for_terminal(&entry.item.message)
                );
                println!("{}", color(&line));

                // Print after-context
                if let Some(ctx) = context_map.get(&ctx_key) {
                    for cl in &ctx.after {
                        println!(
                            "    {} {}",
                            format!("{:>4}", cl.line_number).dimmed(),
                            sanitize_for_terminal(&cl.content).dimmed()
                        );
                    }
                    println!();
                }
            }

            println!(
                "\n+{} -{} (base: {})",
                result.added_count, result.removed_count, result.base_ref
            );
        }
        Format::Json => {
            let mut value: serde_json::Value =
                serde_json::to_value(result).expect("failed to serialize");
            if let Some(entries) = value.get_mut("entries").and_then(|v| v.as_array_mut()) {
                for entry_val in entries.iter_mut() {
                    // Read context key from item before mutation
                    let ctx_key = entry_val.get("item").map(|item_val| {
                        let file = item_val.get("file").and_then(|v| v.as_str()).unwrap_or("");
                        let line = item_val.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                        format!("{}:{}", file, line)
                    });

                    if let Some(ref key) = ctx_key {
                        if has_context {
                            if let Some(ctx) = context_map.get(key) {
                                let ctx_value =
                                    serde_json::to_value(ctx).expect("failed to serialize context");
                                entry_val
                                    .as_object_mut()
                                    .unwrap()
                                    .insert("context".to_string(), ctx_value);
                            }
                        }
                    }

                    if let Some(item_val) = entry_val.get_mut("item") {
                        apply_detail_to_json_item(item_val, detail);
                    }
                }
            }
            let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
            println!("{}", json);
        }
        Format::GithubActions => print!("{}", github_actions::format_diff(result)),
        Format::Sarif => print!("{}", sarif::format_diff(result)),
        Format::Markdown => print!("{}", markdown::format_diff(result)),
    }
}

fn bar(count: usize, max: usize, width: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let filled = (count * width).div_ceil(max);
    "\u{2588}".repeat(filled)
}

pub fn print_brief(result: &BriefResult, format: &Format, budget: Option<usize>) {
    match format {
        Format::Text => {
            let mut lines: Vec<String> = Vec::new();

            // Line 1: summary
            let pc = &result.priority_counts;
            let mut priority_parts: Vec<String> = Vec::new();
            if pc.urgent > 0 {
                priority_parts.push(format!("{} urgent", pc.urgent));
            }
            if pc.high > 0 {
                priority_parts.push(format!("{} high", pc.high));
            }

            let summary = if priority_parts.is_empty() {
                format!(
                    "{} TODOs across {} files",
                    result.total_items, result.total_files
                )
            } else {
                format!(
                    "{} TODOs across {} files ({})",
                    result.total_items,
                    result.total_files,
                    priority_parts.join(", ")
                )
            };
            lines.push(summary);

            // Line 2: top urgent (if any)
            if let Some(ref item) = result.top_urgent {
                let priority_marker = match item.priority {
                    Priority::Urgent => "!!",
                    Priority::High => "!",
                    Priority::Normal => "",
                };
                let issue_suffix = item
                    .issue_ref
                    .as_ref()
                    .map(|r| format!(" ({})", sanitize_for_terminal(r)))
                    .unwrap_or_default();
                lines.push(format!(
                    "Top urgent: {}:{} {}{} {}{}",
                    sanitize_for_terminal(&item.file),
                    item.line,
                    item.tag.as_str(),
                    priority_marker,
                    sanitize_for_terminal(&item.message),
                    issue_suffix
                ));
            }

            // Line 3: trend (if available)
            if let Some(ref trend) = result.trend {
                lines.push(format!(
                    "Trends vs {}: +{} added, -{} removed",
                    trend.base_ref, trend.added, trend.removed
                ));
            }

            let max_lines = budget.unwrap_or(lines.len());
            for line in lines.iter().take(max_lines) {
                println!("{}", line);
            }
        }
        _ => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
    }
}

pub fn print_stats(result: &StatsResult, format: &Format) {
    match format {
        Format::Text => {
            // Tag breakdown
            println!("{}", "Tags".bold().underline());
            let tag_max = result.tag_counts.first().map(|(_, c)| *c).unwrap_or(0);
            for (tag, count) in &result.tag_counts {
                let tag_str = colorize_tag(tag);
                println!(
                    "  {:6} {:>4}  {}",
                    tag_str,
                    count,
                    bar(*count, tag_max, 20).dimmed()
                );
            }

            // Priority summary
            println!(
                "\n{} normal: {} | high: {} | urgent: {}",
                "Priority".bold().underline(),
                result.priority_counts.normal,
                result.priority_counts.high,
                result.priority_counts.urgent,
            );

            // Author breakdown
            if !result.author_counts.is_empty() {
                println!("\n{}", "Authors".bold().underline());
                let author_max = result.author_counts.first().map(|(_, c)| *c).unwrap_or(0);
                for (author, count) in &result.author_counts {
                    println!(
                        "  {:20} {:>4}  {}",
                        sanitize_for_terminal(author),
                        count,
                        bar(*count, author_max, 20).dimmed()
                    );
                }
            }

            // Hotspot files
            if !result.hotspot_files.is_empty() {
                println!("\n{}", "Hotspots".bold().underline());
                for (file, count) in &result.hotspot_files {
                    println!("  {} ({})", sanitize_for_terminal(file), count);
                }
            }

            // Total summary
            println!(
                "\n{} items across {} files",
                result.total_items, result.total_files
            );

            // Trend
            if let Some(ref trend) = result.trend {
                let net: i64 = trend.added as i64 - trend.removed as i64;
                let sign = if net > 0 { "+" } else { "" };
                println!(
                    "Trend since {}: {} added, {} removed ({}{})",
                    trend.base_ref, trend.added, trend.removed, sign, net
                );
            }
        }
        _ => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
    }
}

pub fn print_lint(result: &LintResult, format: &Format) {
    match format {
        Format::Text => {
            if result.passed {
                println!("{}", "PASS".green().bold());
                println!("{} items checked, no violations", result.total_items);
            } else {
                println!("{}", "FAIL".red().bold());

                // Group violations by file
                let mut groups: Vec<(String, Vec<&LintViolation>)> = Vec::new();
                let mut key_index: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();

                for v in &result.violations {
                    let key = v.file.clone();
                    if let Some(&idx) = key_index.get(&key) {
                        groups[idx].1.push(v);
                    } else {
                        key_index.insert(key.clone(), groups.len());
                        groups.push((key, vec![v]));
                    }
                }

                for (file, violations) in &groups {
                    println!("{}", sanitize_for_terminal(file).bold().underline());
                    for v in violations {
                        println!(
                            "  L{}: {} - {}",
                            v.line,
                            sanitize_for_terminal(&v.rule).yellow(),
                            sanitize_for_terminal(&v.message)
                        );
                        if let Some(ref suggestion) = v.suggestion {
                            println!(
                                "    {} {}",
                                "suggestion:".dimmed(),
                                sanitize_for_terminal(suggestion).dimmed()
                            );
                        }
                    }
                }

                println!(
                    "\n{} violations in {} items",
                    result.violation_count, result.total_items
                );
            }
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
        Format::GithubActions => print!("{}", github_actions::format_lint(result)),
        Format::Sarif => print!("{}", sarif::format_lint(result)),
        Format::Markdown => print!("{}", markdown::format_lint(result)),
    }
}

pub fn print_clean(result: &CleanResult, format: &Format) {
    match format {
        Format::Text => {
            if result.passed {
                println!("{}", "PASS".green().bold());
                println!("{} items checked, no violations", result.total_items);
            } else {
                println!("{}", "FAIL".red().bold());

                // Group violations by file
                let mut groups: Vec<(String, Vec<&CleanViolation>)> = Vec::new();
                let mut key_index: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();

                for v in &result.violations {
                    let key = v.file.clone();
                    if let Some(&idx) = key_index.get(&key) {
                        groups[idx].1.push(v);
                    } else {
                        key_index.insert(key.clone(), groups.len());
                        groups.push((key, vec![v]));
                    }
                }

                for (file, violations) in &groups {
                    println!("{}", sanitize_for_terminal(file).bold().underline());
                    for v in violations {
                        let mut line = format!(
                            "  L{}: {} - {}",
                            v.line,
                            sanitize_for_terminal(&v.rule).yellow(),
                            sanitize_for_terminal(&v.message)
                        );
                        if let Some(ref dup_of) = v.duplicate_of {
                            line.push_str(&format!(
                                " (duplicate of {})",
                                sanitize_for_terminal(dup_of)
                            ));
                        }
                        println!("{}", line);
                    }
                }

                let violation_count = result.violations.len();
                println!(
                    "\n{} violations ({} stale, {} duplicates) in {} items",
                    violation_count, result.stale_count, result.duplicate_count, result.total_items
                );
            }
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
        Format::GithubActions => print!("{}", github_actions::format_clean(result)),
        Format::Sarif => print!("{}", sarif::format_clean(result)),
        Format::Markdown => print!("{}", markdown::format_clean(result)),
    }
}

pub fn print_check(result: &CheckResult, format: &Format) {
    match format {
        Format::Text => {
            if result.passed {
                println!("{}", "PASS".green().bold());
            } else {
                println!("{}", "FAIL".red().bold());
                for violation in &result.violations {
                    println!(
                        "  {}: {}",
                        sanitize_for_terminal(&violation.rule).yellow(),
                        sanitize_for_terminal(&violation.message)
                    );
                }
            }
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
        Format::GithubActions => print!("{}", github_actions::format_check(result)),
        Format::Sarif => print!("{}", sarif::format_check(result)),
        Format::Markdown => print!("{}", markdown::format_check(result)),
    }
}

pub fn print_blame(result: &BlameResult, format: &Format) {
    match format {
        Format::Text => {
            // Group by file
            let mut groups: Vec<(String, Vec<&BlameEntry>)> = Vec::new();
            let mut key_index: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();

            for entry in &result.entries {
                let key = entry.item.file.clone();
                if let Some(&idx) = key_index.get(&key) {
                    groups[idx].1.push(entry);
                } else {
                    key_index.insert(key.clone(), groups.len());
                    groups.push((key, vec![entry]));
                }
            }

            for (file, entries) in &groups {
                println!("{}", sanitize_for_terminal(file).bold().underline());
                for entry in entries {
                    let tag_str = colorize_tag(&entry.item.tag);
                    let stale_marker = if entry.stale {
                        " [STALE]".red().to_string()
                    } else {
                        String::new()
                    };
                    println!(
                        "  L{}: [{}] {} @{} {} ({} days ago){}",
                        entry.item.line,
                        tag_str,
                        sanitize_for_terminal(&entry.item.message),
                        sanitize_for_terminal(&entry.blame.author),
                        sanitize_for_terminal(&entry.blame.date),
                        entry.blame.age_days,
                        stale_marker,
                    );
                }
            }

            println!(
                "\n{} items, avg age {} days, {} stale (threshold: {} days)",
                result.total, result.avg_age_days, result.stale_count, result.stale_threshold_days,
            );
        }
        Format::Json => {
            let mut value: serde_json::Value =
                serde_json::to_value(result).expect("failed to serialize");
            if let Some(entries) = value.get_mut("entries").and_then(|v| v.as_array_mut()) {
                for entry_val in entries.iter_mut() {
                    inject_id_field(entry_val);
                }
            }
            let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
            println!("{}", json);
        }
        Format::GithubActions => print!("{}", github_actions::format_blame(result)),
        Format::Sarif => print!("{}", sarif::format_blame(result)),
        Format::Markdown => print!("{}", markdown::format_blame(result)),
    }
}

/// Inject a stable `id` field into a JSON object that has flattened TodoItem fields.
fn inject_id_field(val: &mut serde_json::Value) {
    let file = val
        .get("file")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let tag = val
        .get("tag")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let message = val
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let id = format!("{}:{}:{}", file, tag, message.trim().to_lowercase());
    val.as_object_mut()
        .unwrap()
        .insert("id".to_string(), serde_json::Value::String(id));
}

pub fn print_context(rich: &RichContext, format: &Format) {
    match format {
        Format::Text => {
            println!(
                "{}",
                format!("{}:{}", rich.file, rich.line).bold().underline()
            );
            println!();

            for cl in &rich.before {
                println!(
                    "  {} {}",
                    format!("{:>4}", cl.line_number).dimmed(),
                    sanitize_for_terminal(&cl.content).dimmed()
                );
            }

            println!(
                "  {} {}",
                format!("{:>4}", rich.line).cyan(),
                sanitize_for_terminal(&rich.todo_line)
            );

            for cl in &rich.after {
                println!(
                    "  {} {}",
                    format!("{:>4}", cl.line_number).dimmed(),
                    sanitize_for_terminal(&cl.content).dimmed()
                );
            }

            if !rich.related_todos.is_empty() {
                println!();
                println!("{}", "Related TODOs:".bold());
                for rt in &rich.related_todos {
                    println!(
                        "  L{}: [{}] {}",
                        rt.line,
                        rt.tag,
                        sanitize_for_terminal(&rt.message)
                    );
                }
            }
        }
        _ => {
            let json = serde_json::to_string_pretty(rich).expect("failed to serialize");
            println!("{}", json);
        }
    }
}

pub fn print_initial_summary(tag_counts: &[(Tag, usize)], total: usize, format: &Format) {
    match format {
        Format::Text => {
            println!("{}", "Initial scan".bold().underline());
            for (tag, count) in tag_counts {
                println!("  {:6} {}", colorize_tag(tag), count);
            }
            println!("{} items total", total);
            println!();
        }
        _ => {
            let summary: serde_json::Value = serde_json::json!({
                "type": "initial_scan",
                "total": total,
                "tags": tag_counts.iter().map(|(tag, count)| {
                    serde_json::json!({ "tag": tag.as_str(), "count": count })
                }).collect::<Vec<_>>(),
            });
            println!(
                "{}",
                serde_json::to_string(&summary).expect("failed to serialize")
            );
        }
    }
}

pub fn print_watch_event(event: &WatchEvent, format: &Format, max: Option<usize>) {
    match format {
        Format::Text => {
            println!(
                "{} {}",
                event.timestamp.dimmed(),
                sanitize_for_terminal(&event.file).bold()
            );

            for item in &event.added {
                let tag_str = colorize_tag(&item.tag);
                println!(
                    "  {} L{}: [{}] {}",
                    "+".green(),
                    item.line,
                    tag_str,
                    sanitize_for_terminal(&item.message)
                );
            }

            for item in &event.removed {
                let tag_str = colorize_tag(&item.tag);
                println!(
                    "  {} L{}: [{}] {}",
                    "-".red(),
                    item.line,
                    tag_str,
                    sanitize_for_terminal(&item.message)
                );
            }

            let delta_str = if event.total_delta > 0 {
                format!("+{}", event.total_delta).green().to_string()
            } else if event.total_delta < 0 {
                format!("{}", event.total_delta).red().to_string()
            } else {
                "±0".to_string()
            };
            println!("  {} total ({})", event.total, delta_str);

            if let Some(threshold) = max {
                if event.total >= threshold {
                    println!(
                        "  {}",
                        format!(
                            "Warning: total {} reached --max threshold {}",
                            event.total, threshold
                        )
                        .yellow()
                    );
                }
            }

            println!();
        }
        _ => {
            let json = serde_json::to_string(&event).expect("failed to serialize");
            println!("{}", json);
        }
    }
}

pub fn print_tasks(result: &TasksResult, format: &Format) {
    match format {
        Format::Text => {
            if result.tasks.is_empty() {
                println!("No tasks to export.");
                return;
            }

            for task in &result.tasks {
                let priority_marker = match task.metadata.todo_scan_priority.as_str() {
                    "urgent" => "!!",
                    "high" => "!",
                    _ => " ",
                };

                println!(
                    "  {:>2} {:6} {}:{} {}",
                    priority_marker,
                    sanitize_for_terminal(&task.metadata.todo_scan_tag),
                    sanitize_for_terminal(&task.metadata.todo_scan_file),
                    task.metadata.todo_scan_line,
                    sanitize_for_terminal(&task.subject),
                );
            }

            println!("\n{} tasks exported", result.total);
            if let Some(ref dir) = result.output_dir {
                println!("Output: {}", sanitize_for_terminal(dir));
            }
        }
        _ => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
    }
}

pub fn print_relate(result: &RelateResult, format: &Format) {
    match format {
        Format::Text => {
            if result.relationships.is_empty() {
                println!("No relationships found (min_score: {})", result.min_score);
                return;
            }

            if let Some(ref target) = result.target {
                println!(
                    "{}",
                    format!("Relationships for {}", sanitize_for_terminal(target))
                        .bold()
                        .underline()
                );
            }

            if let Some(ref clusters) = result.clusters {
                for cluster in clusters {
                    println!(
                        "\n{}",
                        format!(
                            "Cluster {} — {}",
                            cluster.id,
                            sanitize_for_terminal(&cluster.theme)
                        )
                        .bold()
                        .underline()
                    );
                    println!("  Items (suggested order):");
                    for loc in &cluster.suggested_order {
                        println!("    {}", sanitize_for_terminal(loc));
                    }
                    if !cluster.relationships.is_empty() {
                        println!("  Relationships:");
                        for rel in &cluster.relationships {
                            println!(
                                "    {} ↔ {} (score: {:.2}, {})",
                                sanitize_for_terminal(&rel.from),
                                sanitize_for_terminal(&rel.to),
                                rel.score,
                                sanitize_for_terminal(&rel.reason)
                            );
                        }
                    }
                }
            } else {
                for rel in &result.relationships {
                    println!(
                        "  {} ↔ {} (score: {:.2}, {})",
                        sanitize_for_terminal(&rel.from),
                        sanitize_for_terminal(&rel.to),
                        rel.score,
                        sanitize_for_terminal(&rel.reason)
                    );
                }
            }

            println!(
                "\n{} relationships across {} items",
                result.total_relationships, result.total_items
            );
        }
        _ => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
    }
}

/// Strip terminal control characters from user-controlled strings to prevent
/// ANSI escape injection. Removes 0x00–0x1f (except tab 0x09) and 0x7f.
fn sanitize_for_terminal(s: &str) -> String {
    s.chars()
        .filter(|c| {
            let code = *c as u32;
            *c == '\t' || (code >= 0x20 && code != 0x7f)
        })
        .collect()
}

pub fn print_report(report: &ReportResult, output_path: &str) -> std::io::Result<()> {
    let content = html::render_html(report);
    std::fs::write(output_path, content)?;
    println!("Report written to {}", sanitize_for_terminal(output_path));
    Ok(())
}

pub fn print_workspace_list(
    result: &WorkspaceResult,
    format: &Format,
    kind: &crate::model::WorkspaceKind,
) {
    match format {
        Format::Text => {
            println!("{}", format!("Workspace ({kind})").bold().underline());
            println!(
                "  {:<20} {:<30} {:>6}  {:>6}  Status",
                "Package", "Path", "TODOs", "Max"
            );
            println!("  {}", "-".repeat(78));

            for pkg in &result.packages {
                let max_str = match pkg.max {
                    Some(m) => m.to_string(),
                    None => "-".to_string(),
                };
                let status_str = match pkg.status {
                    PackageStatus::Ok => "ok".green().to_string(),
                    PackageStatus::Over => "OVER".red().bold().to_string(),
                    PackageStatus::Uncapped => "-".dimmed().to_string(),
                };
                println!(
                    "  {:<20} {:<30} {:>6}  {:>6}  {}",
                    sanitize_for_terminal(&pkg.name),
                    sanitize_for_terminal(&pkg.path),
                    pkg.todo_count,
                    max_str,
                    status_str
                );
            }

            println!(
                "\n{} packages, {} TODOs total",
                result.total_packages, result.total_todos
            );
        }
        _ => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_strips_ansi_escape() {
        assert_eq!(
            sanitize_for_terminal("hello\x1b[31mworld"),
            "hello[31mworld"
        );
    }

    #[test]
    fn test_sanitize_strips_null_bytes() {
        assert_eq!(sanitize_for_terminal("hello\x00world"), "helloworld");
    }

    #[test]
    fn test_sanitize_strips_cr_lf() {
        assert_eq!(sanitize_for_terminal("hello\r\nworld"), "helloworld");
    }

    #[test]
    fn test_sanitize_preserves_tab() {
        assert_eq!(sanitize_for_terminal("hello\tworld"), "hello\tworld");
    }

    #[test]
    fn test_sanitize_preserves_normal_ascii() {
        assert_eq!(
            sanitize_for_terminal("normal text 123!@#"),
            "normal text 123!@#"
        );
    }

    #[test]
    fn test_sanitize_strips_bell() {
        assert_eq!(sanitize_for_terminal("hello\x07world"), "helloworld");
    }

    #[test]
    fn test_sanitize_preserves_unicode_emoji() {
        assert_eq!(sanitize_for_terminal("hello 🌍 café"), "hello 🌍 café");
    }

    // --- sanitize_for_terminal additional edge cases ---

    #[test]
    fn test_sanitize_empty_string() {
        assert_eq!(sanitize_for_terminal(""), "");
    }

    #[test]
    fn test_sanitize_strips_delete_0x7f() {
        assert_eq!(sanitize_for_terminal("abc\x7fdef"), "abcdef");
    }

    #[test]
    fn test_sanitize_strips_all_low_control_except_tab() {
        // 0x01 through 0x08, 0x0a through 0x1f should all be stripped
        let input = "a\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1fb";
        // Only \x09 (tab) survives
        assert_eq!(sanitize_for_terminal(input), "a\tb");
    }

    #[test]
    fn test_sanitize_mixed_valid_invalid() {
        assert_eq!(
            sanitize_for_terminal("TODO\x1b[0m(alice): fix \x00this"),
            "TODO[0m(alice): fix this"
        );
    }

    // --- Helper to build TodoItem ---

    fn make_item(file: &str, line: usize, tag: Tag, msg: &str, priority: Priority) -> TodoItem {
        TodoItem {
            file: file.to_string(),
            line,
            tag,
            message: msg.to_string(),
            author: None,
            issue_ref: None,
            priority,
            deadline: None,
        }
    }

    fn make_item_with_author(
        file: &str,
        line: usize,
        tag: Tag,
        msg: &str,
        priority: Priority,
        author: Option<&str>,
    ) -> TodoItem {
        TodoItem {
            file: file.to_string(),
            line,
            tag,
            message: msg.to_string(),
            author: author.map(|a| a.to_string()),
            issue_ref: None,
            priority,
            deadline: None,
        }
    }

    // --- inject_id_field tests ---

    #[test]
    fn test_inject_id_field_basic() {
        let mut val = serde_json::json!({
            "file": "src/main.rs",
            "tag": "TODO",
            "message": "Fix this bug"
        });
        inject_id_field(&mut val);
        assert_eq!(val["id"].as_str().unwrap(), "src/main.rs:TODO:fix this bug");
    }

    #[test]
    fn test_inject_id_field_lowercases_message() {
        let mut val = serde_json::json!({
            "file": "lib.rs",
            "tag": "FIXME",
            "message": "Uppercase Message HERE"
        });
        inject_id_field(&mut val);
        assert_eq!(
            val["id"].as_str().unwrap(),
            "lib.rs:FIXME:uppercase message here"
        );
    }

    #[test]
    fn test_inject_id_field_trims_message() {
        let mut val = serde_json::json!({
            "file": "lib.rs",
            "tag": "TODO",
            "message": "  spaces around  "
        });
        inject_id_field(&mut val);
        assert_eq!(val["id"].as_str().unwrap(), "lib.rs:TODO:spaces around");
    }

    #[test]
    fn test_inject_id_field_missing_fields_uses_defaults() {
        let mut val = serde_json::json!({});
        inject_id_field(&mut val);
        assert_eq!(val["id"].as_str().unwrap(), "::");
    }

    #[test]
    fn test_inject_id_field_overwrites_existing_id() {
        let mut val = serde_json::json!({
            "file": "a.rs",
            "tag": "BUG",
            "message": "crash",
            "id": "old-id"
        });
        inject_id_field(&mut val);
        assert_eq!(val["id"].as_str().unwrap(), "a.rs:BUG:crash");
    }

    // --- apply_detail_to_json_item tests ---

    #[test]
    fn test_apply_detail_normal_injects_id_and_keeps_fields() {
        let item = make_item_with_author(
            "src/main.rs",
            10,
            Tag::Todo,
            "do it",
            Priority::High,
            Some("alice"),
        );
        let mut val = serde_json::to_value(&item).unwrap();
        val.as_object_mut()
            .unwrap()
            .insert("issue_ref".to_string(), serde_json::json!("#42"));

        apply_detail_to_json_item(&mut val, &DetailLevel::Normal);

        // id should be injected
        assert!(val.get("id").is_some());
        // author and issue_ref should remain
        assert!(val.get("author").is_some());
        assert!(val.get("issue_ref").is_some());
        assert!(val.get("priority").is_some());
        // match_key should NOT be present in Normal mode
        assert!(val.get("match_key").is_none());
    }

    #[test]
    fn test_apply_detail_minimal_removes_fields() {
        let mut val = serde_json::json!({
            "file": "src/lib.rs",
            "line": 5,
            "tag": "TODO",
            "message": "implement this",
            "author": "bob",
            "issue_ref": "#99",
            "priority": "normal",
            "deadline": "2025-01-01"
        });

        apply_detail_to_json_item(&mut val, &DetailLevel::Minimal);

        // id should be injected
        assert!(val.get("id").is_some());
        // These fields should be removed
        assert!(val.get("author").is_none());
        assert!(val.get("issue_ref").is_none());
        assert!(val.get("priority").is_none());
        assert!(val.get("deadline").is_none());
        // file, line, tag, message should remain
        assert_eq!(val["file"].as_str().unwrap(), "src/lib.rs");
        assert_eq!(val["line"].as_u64().unwrap(), 5);
        assert_eq!(val["tag"].as_str().unwrap(), "TODO");
        assert_eq!(val["message"].as_str().unwrap(), "implement this");
    }

    #[test]
    fn test_apply_detail_full_adds_match_key() {
        let mut val = serde_json::json!({
            "file": "src/app.rs",
            "tag": "FIXME",
            "message": "Memory leak"
        });

        apply_detail_to_json_item(&mut val, &DetailLevel::Full);

        // id should be injected
        let id = val["id"].as_str().unwrap();
        assert_eq!(id, "src/app.rs:FIXME:memory leak");
        // match_key should equal the id
        let match_key = val["match_key"].as_str().unwrap();
        assert_eq!(match_key, id);
    }

    #[test]
    fn test_apply_detail_minimal_no_crash_if_fields_absent() {
        let mut val = serde_json::json!({
            "file": "x.rs",
            "tag": "TODO",
            "message": "test"
        });
        // Removing fields that don't exist should not panic
        apply_detail_to_json_item(&mut val, &DetailLevel::Minimal);
        assert!(val.get("id").is_some());
    }

    // --- colorize_tag tests ---

    #[test]
    fn test_colorize_tag_returns_correct_text_for_all_tags() {
        // We verify the underlying text is correct for each tag variant.
        // Colored strings deref to the original text.
        assert_eq!(colorize_tag(&Tag::Todo).to_string().contains("TODO"), true);
        assert_eq!(
            colorize_tag(&Tag::Fixme).to_string().contains("FIXME"),
            true
        );
        assert_eq!(colorize_tag(&Tag::Hack).to_string().contains("HACK"), true);
        assert_eq!(colorize_tag(&Tag::Bug).to_string().contains("BUG"), true);
        assert_eq!(colorize_tag(&Tag::Note).to_string().contains("NOTE"), true);
        assert_eq!(colorize_tag(&Tag::Xxx).to_string().contains("XXX"), true);
    }

    #[test]
    fn test_colorize_tag_todo_is_yellow() {
        // Disable coloring to test the underlying string
        colored::control::set_override(false);
        let result = colorize_tag(&Tag::Todo);
        assert_eq!(&*result, "TODO");
        colored::control::unset_override();
    }

    #[test]
    fn test_colorize_tag_fixme_is_red() {
        colored::control::set_override(false);
        let result = colorize_tag(&Tag::Fixme);
        assert_eq!(&*result, "FIXME");
        colored::control::unset_override();
    }

    #[test]
    fn test_colorize_tag_hack_is_magenta() {
        colored::control::set_override(false);
        let result = colorize_tag(&Tag::Hack);
        assert_eq!(&*result, "HACK");
        colored::control::unset_override();
    }

    #[test]
    fn test_colorize_tag_bug_is_red_bold() {
        colored::control::set_override(false);
        let result = colorize_tag(&Tag::Bug);
        assert_eq!(&*result, "BUG");
        colored::control::unset_override();
    }

    #[test]
    fn test_colorize_tag_note_is_blue() {
        colored::control::set_override(false);
        let result = colorize_tag(&Tag::Note);
        assert_eq!(&*result, "NOTE");
        colored::control::unset_override();
    }

    #[test]
    fn test_colorize_tag_xxx_is_red() {
        colored::control::set_override(false);
        let result = colorize_tag(&Tag::Xxx);
        assert_eq!(&*result, "XXX");
        colored::control::unset_override();
    }

    // --- group_key tests ---

    #[test]
    fn test_group_key_file() {
        let item = make_item("src/main.rs", 10, Tag::Todo, "test", Priority::Normal);
        assert_eq!(group_key(&item, &GroupBy::File), "src/main.rs");
    }

    #[test]
    fn test_group_key_tag() {
        let item = make_item("src/main.rs", 10, Tag::Fixme, "test", Priority::Normal);
        assert_eq!(group_key(&item, &GroupBy::Tag), "FIXME");
    }

    #[test]
    fn test_group_key_tag_all_variants() {
        for (tag, expected) in [
            (Tag::Todo, "TODO"),
            (Tag::Fixme, "FIXME"),
            (Tag::Hack, "HACK"),
            (Tag::Bug, "BUG"),
            (Tag::Note, "NOTE"),
            (Tag::Xxx, "XXX"),
        ] {
            let item = make_item("f.rs", 1, tag, "msg", Priority::Normal);
            assert_eq!(group_key(&item, &GroupBy::Tag), expected);
        }
    }

    #[test]
    fn test_group_key_priority_urgent() {
        let item = make_item("f.rs", 1, Tag::Todo, "msg", Priority::Urgent);
        assert_eq!(group_key(&item, &GroupBy::Priority), "!! Urgent");
    }

    #[test]
    fn test_group_key_priority_high() {
        let item = make_item("f.rs", 1, Tag::Todo, "msg", Priority::High);
        assert_eq!(group_key(&item, &GroupBy::Priority), "! High");
    }

    #[test]
    fn test_group_key_priority_normal() {
        let item = make_item("f.rs", 1, Tag::Todo, "msg", Priority::Normal);
        assert_eq!(group_key(&item, &GroupBy::Priority), "Normal");
    }

    #[test]
    fn test_group_key_author_with_author() {
        let item =
            make_item_with_author("f.rs", 1, Tag::Todo, "msg", Priority::Normal, Some("alice"));
        assert_eq!(group_key(&item, &GroupBy::Author), "alice");
    }

    #[test]
    fn test_group_key_author_without_author() {
        let item = make_item("f.rs", 1, Tag::Todo, "msg", Priority::Normal);
        assert_eq!(group_key(&item, &GroupBy::Author), "unassigned");
    }

    #[test]
    fn test_group_key_dir_with_subdirectory() {
        let item = make_item(
            "src/utils/helpers.rs",
            1,
            Tag::Todo,
            "msg",
            Priority::Normal,
        );
        assert_eq!(group_key(&item, &GroupBy::Dir), "src/utils");
    }

    #[test]
    fn test_group_key_dir_top_level_file() {
        let item = make_item("main.rs", 1, Tag::Todo, "msg", Priority::Normal);
        assert_eq!(group_key(&item, &GroupBy::Dir), ".");
    }

    #[test]
    fn test_group_key_dir_single_level() {
        let item = make_item("src/lib.rs", 1, Tag::Todo, "msg", Priority::Normal);
        assert_eq!(group_key(&item, &GroupBy::Dir), "src");
    }

    #[test]
    fn test_group_key_dir_deeply_nested() {
        let item = make_item("a/b/c/d/e.rs", 1, Tag::Todo, "msg", Priority::Normal);
        assert_eq!(group_key(&item, &GroupBy::Dir), "a/b/c/d");
    }

    // --- group_items tests ---

    #[test]
    fn test_group_items_by_file_groups_correctly() {
        let items = vec![
            make_item("a.rs", 1, Tag::Todo, "first", Priority::Normal),
            make_item("b.rs", 5, Tag::Fixme, "second", Priority::High),
            make_item("a.rs", 10, Tag::Bug, "third", Priority::Urgent),
        ];

        let groups = group_items(&items, &GroupBy::File);

        assert_eq!(groups.len(), 2);
        // Sorted alphabetically by filename
        assert_eq!(groups[0].0, "a.rs");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "b.rs");
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn test_group_items_by_tag_sorted_by_severity_descending() {
        let items = vec![
            make_item("a.rs", 1, Tag::Note, "low", Priority::Normal), // severity 0
            make_item("b.rs", 2, Tag::Bug, "high", Priority::Normal), // severity 5
            make_item("c.rs", 3, Tag::Todo, "medium", Priority::Normal), // severity 1
            make_item("d.rs", 4, Tag::Fixme, "high2", Priority::Normal), // severity 4
        ];

        let groups = group_items(&items, &GroupBy::Tag);

        // Should be ordered: BUG(5), FIXME(4), TODO(1), NOTE(0) — descending severity
        assert_eq!(groups.len(), 4);
        assert_eq!(groups[0].0, "BUG");
        assert_eq!(groups[1].0, "FIXME");
        assert_eq!(groups[2].0, "TODO");
        assert_eq!(groups[3].0, "NOTE");
    }

    #[test]
    fn test_group_items_by_priority_sorted_urgency_first() {
        let items = vec![
            make_item("a.rs", 1, Tag::Todo, "normal", Priority::Normal),
            make_item("b.rs", 2, Tag::Todo, "urgent", Priority::Urgent),
            make_item("c.rs", 3, Tag::Todo, "high", Priority::High),
        ];

        let groups = group_items(&items, &GroupBy::Priority);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, "!! Urgent");
        assert_eq!(groups[1].0, "! High");
        assert_eq!(groups[2].0, "Normal");
    }

    #[test]
    fn test_group_items_by_author_sorted_alphabetically() {
        let items = vec![
            make_item_with_author(
                "a.rs",
                1,
                Tag::Todo,
                "msg1",
                Priority::Normal,
                Some("charlie"),
            ),
            make_item_with_author(
                "b.rs",
                2,
                Tag::Todo,
                "msg2",
                Priority::Normal,
                Some("alice"),
            ),
            make_item_with_author("c.rs", 3, Tag::Todo, "msg3", Priority::Normal, None),
        ];

        let groups = group_items(&items, &GroupBy::Author);

        // Alphabetical: alice, charlie, unassigned
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, "alice");
        assert_eq!(groups[1].0, "charlie");
        assert_eq!(groups[2].0, "unassigned");
    }

    #[test]
    fn test_group_items_by_dir_sorted_alphabetically() {
        let items = vec![
            make_item("src/utils/a.rs", 1, Tag::Todo, "msg", Priority::Normal),
            make_item("lib/b.rs", 2, Tag::Todo, "msg", Priority::Normal),
            make_item("src/core/c.rs", 3, Tag::Todo, "msg", Priority::Normal),
        ];

        let groups = group_items(&items, &GroupBy::Dir);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, "lib");
        assert_eq!(groups[1].0, "src/core");
        assert_eq!(groups[2].0, "src/utils");
    }

    #[test]
    fn test_group_items_empty_input() {
        let items: Vec<TodoItem> = vec![];
        let groups = group_items(&items, &GroupBy::File);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_group_items_single_item() {
        let items = vec![make_item("a.rs", 1, Tag::Todo, "only", Priority::Normal)];
        let groups = group_items(&items, &GroupBy::File);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "a.rs");
        assert_eq!(groups[0].1.len(), 1);
    }

    #[test]
    fn test_group_items_by_tag_multiple_items_same_tag() {
        let items = vec![
            make_item("a.rs", 1, Tag::Todo, "first", Priority::Normal),
            make_item("b.rs", 2, Tag::Todo, "second", Priority::Normal),
            make_item("c.rs", 3, Tag::Bug, "bug1", Priority::Normal),
        ];

        let groups = group_items(&items, &GroupBy::Tag);

        assert_eq!(groups.len(), 2);
        // BUG has higher severity (5) than TODO (1)
        assert_eq!(groups[0].0, "BUG");
        assert_eq!(groups[0].1.len(), 1);
        assert_eq!(groups[1].0, "TODO");
        assert_eq!(groups[1].1.len(), 2);
    }

    #[test]
    fn test_group_items_by_priority_all_same_priority() {
        let items = vec![
            make_item("a.rs", 1, Tag::Todo, "msg1", Priority::High),
            make_item("b.rs", 2, Tag::Bug, "msg2", Priority::High),
        ];

        let groups = group_items(&items, &GroupBy::Priority);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "! High");
        assert_eq!(groups[0].1.len(), 2);
    }

    #[test]
    fn test_group_items_preserves_insertion_order_within_group() {
        let items = vec![
            make_item("a.rs", 10, Tag::Todo, "first", Priority::Normal),
            make_item("a.rs", 20, Tag::Todo, "second", Priority::Normal),
            make_item("a.rs", 5, Tag::Todo, "third", Priority::Normal),
        ];

        let groups = group_items(&items, &GroupBy::File);
        assert_eq!(groups.len(), 1);
        // Within the group, items should appear in the original order
        assert_eq!(groups[0].1[0].line, 10);
        assert_eq!(groups[0].1[1].line, 20);
        assert_eq!(groups[0].1[2].line, 5);
    }

    // --- bar() tests ---

    #[test]
    fn test_bar_max_zero_returns_empty() {
        assert_eq!(bar(5, 0, 20), "");
    }

    #[test]
    fn test_bar_count_zero_returns_empty() {
        // 0 * 20 / 10 = 0, div_ceil(0, 10) = 0
        assert_eq!(bar(0, 10, 20), "");
    }

    #[test]
    fn test_bar_full_width() {
        let result = bar(10, 10, 20);
        // 10 * 20 / 10 = 20 blocks
        assert_eq!(result.chars().count(), 20);
        assert!(result.chars().all(|c| c == '\u{2588}'));
    }

    #[test]
    fn test_bar_half_width() {
        let result = bar(5, 10, 20);
        // (5 * 20).div_ceil(10) = 100.div_ceil(10) = 10
        assert_eq!(result.chars().count(), 10);
    }

    #[test]
    fn test_bar_small_fraction_rounds_up() {
        let result = bar(1, 10, 20);
        // (1 * 20).div_ceil(10) = 20.div_ceil(10) = 2
        assert_eq!(result.chars().count(), 2);
    }

    #[test]
    fn test_bar_width_one() {
        let result = bar(3, 10, 1);
        // (3 * 1).div_ceil(10) = 3.div_ceil(10) = 1
        assert_eq!(result.chars().count(), 1);
    }

    #[test]
    fn test_bar_count_equals_max() {
        let result = bar(7, 7, 15);
        // (7 * 15).div_ceil(7) = 105.div_ceil(7) = 15
        assert_eq!(result.chars().count(), 15);
    }

    #[test]
    fn test_bar_uses_block_character() {
        let result = bar(5, 10, 4);
        // All characters should be the full block character U+2588
        for c in result.chars() {
            assert_eq!(c, '\u{2588}');
        }
    }

    #[test]
    fn test_bar_width_zero() {
        // (count * 0).div_ceil(max) = 0
        let result = bar(5, 10, 0);
        assert_eq!(result, "");
    }

    // --- group_items with tag severity sorting (additional) ---

    #[test]
    fn test_group_items_by_tag_all_six_tags_sorted() {
        let items = vec![
            make_item("a.rs", 1, Tag::Note, "note", Priority::Normal), // severity 0
            make_item("b.rs", 2, Tag::Todo, "todo", Priority::Normal), // severity 1
            make_item("c.rs", 3, Tag::Hack, "hack", Priority::Normal), // severity 2
            make_item("d.rs", 4, Tag::Xxx, "xxx", Priority::Normal),   // severity 3
            make_item("e.rs", 5, Tag::Fixme, "fixme", Priority::Normal), // severity 4
            make_item("f.rs", 6, Tag::Bug, "bug", Priority::Normal),   // severity 5
        ];

        let groups = group_items(&items, &GroupBy::Tag);

        assert_eq!(groups.len(), 6);
        assert_eq!(groups[0].0, "BUG"); // 5
        assert_eq!(groups[1].0, "FIXME"); // 4
        assert_eq!(groups[2].0, "XXX"); // 3
        assert_eq!(groups[3].0, "HACK"); // 2
        assert_eq!(groups[4].0, "TODO"); // 1
        assert_eq!(groups[5].0, "NOTE"); // 0
    }

    // --- apply_detail_to_json_item with serialized TodoItem ---

    #[test]
    fn test_apply_detail_full_on_serialized_item() {
        let item = TodoItem {
            file: "src/scanner.rs".to_string(),
            line: 42,
            tag: Tag::Hack,
            message: "Workaround for bug #123".to_string(),
            author: Some("dev".to_string()),
            issue_ref: Some("#123".to_string()),
            priority: Priority::High,
            deadline: None,
        };
        let mut val = serde_json::to_value(&item).unwrap();
        apply_detail_to_json_item(&mut val, &DetailLevel::Full);

        let id = val["id"].as_str().unwrap();
        let match_key = val["match_key"].as_str().unwrap();
        assert_eq!(id, match_key);
        assert_eq!(id, "src/scanner.rs:HACK:workaround for bug #123");
        // author and issue_ref should still be present in Full mode
        assert_eq!(val["author"].as_str().unwrap(), "dev");
        assert_eq!(val["issue_ref"].as_str().unwrap(), "#123");
    }

    #[test]
    fn test_apply_detail_minimal_on_serialized_item() {
        let item = TodoItem {
            file: "src/lib.rs".to_string(),
            line: 1,
            tag: Tag::Todo,
            message: "clean up".to_string(),
            author: Some("bob".to_string()),
            issue_ref: Some("JIRA-456".to_string()),
            priority: Priority::Urgent,
            deadline: None,
        };
        let mut val = serde_json::to_value(&item).unwrap();
        apply_detail_to_json_item(&mut val, &DetailLevel::Minimal);

        // id should be present
        assert!(val.get("id").is_some());
        // These should be removed
        assert!(val.get("author").is_none());
        assert!(val.get("issue_ref").is_none());
        assert!(val.get("priority").is_none());
        assert!(val.get("deadline").is_none());
        // Core fields remain
        assert_eq!(val["file"].as_str().unwrap(), "src/lib.rs");
        assert_eq!(val["line"].as_u64().unwrap(), 1);
        assert_eq!(val["tag"].as_str().unwrap(), "TODO");
        assert_eq!(val["message"].as_str().unwrap(), "clean up");
    }

    // --- inject_id_field with nested item (like BlameEntry) ---

    #[test]
    fn test_inject_id_field_with_partial_fields() {
        // Only file and tag present, no message
        let mut val = serde_json::json!({
            "file": "test.rs",
            "tag": "BUG"
        });
        inject_id_field(&mut val);
        assert_eq!(val["id"].as_str().unwrap(), "test.rs:BUG:");
    }

    #[test]
    fn test_inject_id_field_preserves_existing_fields() {
        let mut val = serde_json::json!({
            "file": "a.rs",
            "tag": "NOTE",
            "message": "remember this",
            "line": 99,
            "extra": "data"
        });
        inject_id_field(&mut val);
        assert_eq!(val["id"].as_str().unwrap(), "a.rs:NOTE:remember this");
        // Other fields are untouched
        assert_eq!(val["line"].as_u64().unwrap(), 99);
        assert_eq!(val["extra"].as_str().unwrap(), "data");
    }

    // --- group_key with Dir edge cases ---

    #[test]
    fn test_group_key_dir_windows_style_path() {
        // std::path::Path handles this: on Unix, backslashes are part of the filename
        // On Linux, "src\\main.rs" has no parent directory separator
        let item = make_item("file.txt", 1, Tag::Todo, "msg", Priority::Normal);
        assert_eq!(group_key(&item, &GroupBy::Dir), ".");
    }

    // --- group_items by priority with only urgent items ---

    #[test]
    fn test_group_items_by_priority_only_urgent() {
        let items = vec![
            make_item("a.rs", 1, Tag::Bug, "critical", Priority::Urgent),
            make_item("b.rs", 2, Tag::Fixme, "also critical", Priority::Urgent),
        ];

        let groups = group_items(&items, &GroupBy::Priority);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "!! Urgent");
        assert_eq!(groups[0].1.len(), 2);
    }

    // --- group_items by author mixed assigned and unassigned ---

    #[test]
    fn test_group_items_by_author_mixed() {
        let items = vec![
            make_item("a.rs", 1, Tag::Todo, "msg1", Priority::Normal),
            make_item_with_author("b.rs", 2, Tag::Todo, "msg2", Priority::Normal, Some("zoe")),
            make_item("c.rs", 3, Tag::Todo, "msg3", Priority::Normal),
            make_item_with_author("d.rs", 4, Tag::Todo, "msg4", Priority::Normal, Some("adam")),
        ];

        let groups = group_items(&items, &GroupBy::Author);
        assert_eq!(groups.len(), 3);
        // Alphabetically sorted
        assert_eq!(groups[0].0, "adam");
        assert_eq!(groups[0].1.len(), 1);
        assert_eq!(groups[1].0, "unassigned");
        assert_eq!(groups[1].1.len(), 2);
        assert_eq!(groups[2].0, "zoe");
        assert_eq!(groups[2].1.len(), 1);
    }

    // --- bar edge cases ---

    #[test]
    fn test_bar_count_greater_than_max_still_works() {
        // This could happen with stale data; should produce width or more blocks
        let result = bar(20, 10, 10);
        // (20 * 10).div_ceil(10) = 200.div_ceil(10) = 20
        assert_eq!(result.chars().count(), 20);
    }

    #[test]
    fn test_bar_tiny_fraction() {
        let result = bar(1, 100, 10);
        // (1 * 10).div_ceil(100) = 10.div_ceil(100) = 1
        assert_eq!(result.chars().count(), 1);
    }

    #[test]
    fn test_bar_exact_division() {
        let result = bar(4, 8, 16);
        // (4 * 16).div_ceil(8) = 64.div_ceil(8) = 8
        assert_eq!(result.chars().count(), 8);
    }

    // --- sanitize_for_terminal additional edge cases ---

    #[test]
    fn test_sanitize_strips_backspace() {
        assert_eq!(sanitize_for_terminal("abc\x08def"), "abcdef");
    }

    #[test]
    fn test_sanitize_strips_form_feed() {
        assert_eq!(sanitize_for_terminal("page1\x0cpage2"), "page1page2");
    }

    #[test]
    fn test_sanitize_strips_full_ansi_escape_sequence() {
        // A typical ANSI color reset: ESC[0m
        assert_eq!(
            sanitize_for_terminal("before\x1b[0mafter"),
            "before[0mafter"
        );
    }

    #[test]
    fn test_sanitize_strips_ansi_color_code() {
        // ESC[31m (red) ... ESC[0m (reset)
        assert_eq!(
            sanitize_for_terminal("\x1b[31mred text\x1b[0m"),
            "[31mred text[0m"
        );
    }

    #[test]
    fn test_sanitize_long_string_with_control_chars() {
        let base = "a".repeat(1000);
        let input = format!("{}\x00\x01\x02{}", base, base);
        let expected = format!("{}{}", base, base);
        assert_eq!(sanitize_for_terminal(&input), expected);
    }

    #[test]
    fn test_sanitize_only_control_chars() {
        assert_eq!(
            sanitize_for_terminal("\x00\x01\x02\x03\x04\x05\x06\x07\x08"),
            ""
        );
    }

    #[test]
    fn test_sanitize_multiple_escape_sequences() {
        assert_eq!(
            sanitize_for_terminal("\x1b[1m\x1b[31mbold red\x1b[0m"),
            "[1m[31mbold red[0m"
        );
    }

    #[test]
    fn test_sanitize_tab_preserved_among_stripped_chars() {
        assert_eq!(sanitize_for_terminal("\x01\t\x02\t\x03"), "\t\t");
    }

    // --- group_key Dir edge case: bare filename with no directory ---

    #[test]
    fn test_group_key_dir_bare_filename_no_slash() {
        // A file with no path separator at all, like "Makefile"
        let item = make_item("Makefile", 1, Tag::Todo, "msg", Priority::Normal);
        // Path::new("Makefile").parent() returns Some(""), which is mapped to "."
        assert_eq!(group_key(&item, &GroupBy::Dir), ".");
    }

    #[test]
    fn test_group_key_dir_dotfile() {
        let item = make_item(".gitignore", 1, Tag::Todo, "msg", Priority::Normal);
        assert_eq!(group_key(&item, &GroupBy::Dir), ".");
    }

    // --- group_items: multiple items in same group, verify insertion order ---

    #[test]
    fn test_group_items_multiple_same_group_insertion_order_by_tag() {
        let items = vec![
            make_item("x.rs", 30, Tag::Todo, "third", Priority::Normal),
            make_item("y.rs", 10, Tag::Todo, "first", Priority::Normal),
            make_item("z.rs", 20, Tag::Todo, "second", Priority::Normal),
        ];

        let groups = group_items(&items, &GroupBy::Tag);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "TODO");
        // Items within a group maintain insertion order
        assert_eq!(groups[0].1[0].message, "third");
        assert_eq!(groups[0].1[1].message, "first");
        assert_eq!(groups[0].1[2].message, "second");
    }

    #[test]
    fn test_group_items_multiple_groups_insertion_order_preserved() {
        let items = vec![
            make_item("a.rs", 1, Tag::Todo, "a-todo-1", Priority::Normal),
            make_item("a.rs", 2, Tag::Bug, "a-bug-1", Priority::Normal),
            make_item("a.rs", 3, Tag::Todo, "a-todo-2", Priority::Normal),
            make_item("a.rs", 4, Tag::Bug, "a-bug-2", Priority::Normal),
        ];

        let groups = group_items(&items, &GroupBy::Tag);
        // BUG (severity 5) before TODO (severity 1)
        assert_eq!(groups[0].0, "BUG");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[0].1[0].message, "a-bug-1");
        assert_eq!(groups[0].1[1].message, "a-bug-2");

        assert_eq!(groups[1].0, "TODO");
        assert_eq!(groups[1].1.len(), 2);
        assert_eq!(groups[1].1[0].message, "a-todo-1");
        assert_eq!(groups[1].1[1].message, "a-todo-2");
    }

    // --- inject_id_field with special characters ---

    #[test]
    fn test_inject_id_field_special_chars_in_file() {
        let mut val = serde_json::json!({
            "file": "path/to/my file (1).rs",
            "tag": "TODO",
            "message": "fix it"
        });
        inject_id_field(&mut val);
        assert_eq!(
            val["id"].as_str().unwrap(),
            "path/to/my file (1).rs:TODO:fix it"
        );
    }

    #[test]
    fn test_inject_id_field_special_chars_in_message() {
        let mut val = serde_json::json!({
            "file": "src/main.rs",
            "tag": "FIXME",
            "message": "Handle <script>alert('xss')</script>"
        });
        inject_id_field(&mut val);
        assert_eq!(
            val["id"].as_str().unwrap(),
            "src/main.rs:FIXME:handle <script>alert('xss')</script>"
        );
    }

    #[test]
    fn test_inject_id_field_unicode_in_message() {
        let mut val = serde_json::json!({
            "file": "src/i18n.rs",
            "tag": "TODO",
            "message": "Support \u{00e9}l\u{00e8}ve names"
        });
        inject_id_field(&mut val);
        let id = val["id"].as_str().unwrap();
        assert!(id.starts_with("src/i18n.rs:TODO:"));
        assert!(id.contains("support"));
    }

    #[test]
    fn test_inject_id_field_colons_in_fields() {
        let mut val = serde_json::json!({
            "file": "C:\\Users\\dev\\file.rs",
            "tag": "BUG",
            "message": "error: something: else"
        });
        inject_id_field(&mut val);
        assert_eq!(
            val["id"].as_str().unwrap(),
            "C:\\Users\\dev\\file.rs:BUG:error: something: else"
        );
    }

    #[test]
    fn test_inject_id_field_empty_message() {
        let mut val = serde_json::json!({
            "file": "x.rs",
            "tag": "HACK",
            "message": ""
        });
        inject_id_field(&mut val);
        assert_eq!(val["id"].as_str().unwrap(), "x.rs:HACK:");
    }

    #[test]
    fn test_inject_id_field_message_with_whitespace_only() {
        let mut val = serde_json::json!({
            "file": "x.rs",
            "tag": "TODO",
            "message": "   \t  "
        });
        inject_id_field(&mut val);
        // trim() on whitespace-only gives empty string; lowercase of empty is empty
        assert_eq!(val["id"].as_str().unwrap(), "x.rs:TODO:");
    }

    // --- apply_detail_to_json_item Minimal on item with no optional fields ---

    #[test]
    fn test_apply_detail_minimal_on_item_without_optional_fields() {
        // An item that has no author, issue_ref, priority, deadline already
        let mut val = serde_json::json!({
            "file": "bare.rs",
            "line": 1,
            "tag": "NOTE",
            "message": "just a note"
        });
        apply_detail_to_json_item(&mut val, &DetailLevel::Minimal);

        // id should be injected
        assert_eq!(val["id"].as_str().unwrap(), "bare.rs:NOTE:just a note");
        // Fields that didn't exist shouldn't cause issues
        assert!(val.get("author").is_none());
        assert!(val.get("issue_ref").is_none());
        assert!(val.get("priority").is_none());
        assert!(val.get("deadline").is_none());
        // Core fields remain
        assert_eq!(val["file"].as_str().unwrap(), "bare.rs");
        assert_eq!(val["line"].as_u64().unwrap(), 1);
    }

    // --- bar() additional edge case ---

    #[test]
    fn test_bar_both_max_and_width_zero() {
        // max == 0 returns early with empty string, width doesn't matter
        assert_eq!(bar(5, 0, 0), "");
    }

    #[test]
    fn test_bar_all_zeros() {
        assert_eq!(bar(0, 0, 0), "");
    }

    #[test]
    fn test_bar_large_values() {
        let result = bar(1000, 1000, 100);
        // (1000 * 100).div_ceil(1000) = 100
        assert_eq!(result.chars().count(), 100);
    }

    // ================================================================
    // JSON serialization path tests for print_* functions
    // ================================================================
    // These test the same JSON serialization logic that print_list,
    // print_diff, print_check, print_blame, print_lint, and print_clean
    // use in their Format::Json branches, without requiring stdout capture.

    #[test]
    fn test_print_list_json_serialization_path() {
        let result = ScanResult {
            items: vec![
                make_item(
                    "src/main.rs",
                    10,
                    Tag::Todo,
                    "do something",
                    Priority::Normal,
                ),
                make_item_with_author(
                    "src/lib.rs",
                    20,
                    Tag::Fixme,
                    "fix this",
                    Priority::High,
                    Some("alice"),
                ),
            ],
            ignored_items: vec![],
            files_scanned: 2,
        };

        // Replicate the JSON branch of print_list
        let mut value: serde_json::Value =
            serde_json::to_value(&result).expect("failed to serialize");
        let detail = DetailLevel::Normal;

        if let Some(items) = value.get_mut("items").and_then(|v| v.as_array_mut()) {
            for item_val in items.iter_mut() {
                apply_detail_to_json_item(item_val, &detail);
            }
        }

        let json = serde_json::to_string_pretty(&value).expect("failed to serialize");

        // Verify structure
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let items = parsed["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);

        // First item should have an id
        assert_eq!(
            items[0]["id"].as_str().unwrap(),
            "src/main.rs:TODO:do something"
        );
        assert_eq!(items[0]["file"].as_str().unwrap(), "src/main.rs");
        assert_eq!(items[0]["line"].as_u64().unwrap(), 10);
        assert_eq!(items[0]["tag"].as_str().unwrap(), "TODO");

        // Second item should have author preserved (Normal mode)
        assert_eq!(
            items[1]["id"].as_str().unwrap(),
            "src/lib.rs:FIXME:fix this"
        );
        assert_eq!(items[1]["author"].as_str().unwrap(), "alice");

        // files_scanned should be present
        assert_eq!(parsed["files_scanned"].as_u64().unwrap(), 2);
    }

    #[test]
    fn test_print_list_json_serialization_path_minimal() {
        let result = ScanResult {
            items: vec![TodoItem {
                file: "src/main.rs".to_string(),
                line: 10,
                tag: Tag::Todo,
                message: "do something".to_string(),
                author: Some("bob".to_string()),
                issue_ref: Some("#42".to_string()),
                priority: Priority::Urgent,
                deadline: None,
            }],
            ignored_items: vec![],
            files_scanned: 1,
        };

        let mut value: serde_json::Value =
            serde_json::to_value(&result).expect("failed to serialize");
        let detail = DetailLevel::Minimal;

        if let Some(items) = value.get_mut("items").and_then(|v| v.as_array_mut()) {
            for item_val in items.iter_mut() {
                apply_detail_to_json_item(item_val, &detail);
            }
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&serde_json::to_string_pretty(&value).unwrap()).unwrap();
        let items = parsed["items"].as_array().unwrap();

        // In Minimal mode, author/issue_ref/priority/deadline are removed
        assert!(items[0].get("author").is_none());
        assert!(items[0].get("issue_ref").is_none());
        assert!(items[0].get("priority").is_none());
        assert!(items[0].get("deadline").is_none());
        // But id, file, line, tag, message remain
        assert!(items[0].get("id").is_some());
        assert_eq!(items[0]["file"].as_str().unwrap(), "src/main.rs");
    }

    #[test]
    fn test_print_list_json_serialization_path_full() {
        let result = ScanResult {
            items: vec![make_item(
                "src/main.rs",
                10,
                Tag::Todo,
                "do something",
                Priority::Normal,
            )],
            ignored_items: vec![],
            files_scanned: 1,
        };

        let mut value: serde_json::Value =
            serde_json::to_value(&result).expect("failed to serialize");
        let detail = DetailLevel::Full;

        if let Some(items) = value.get_mut("items").and_then(|v| v.as_array_mut()) {
            for item_val in items.iter_mut() {
                apply_detail_to_json_item(item_val, &detail);
            }
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&serde_json::to_string_pretty(&value).unwrap()).unwrap();
        let items = parsed["items"].as_array().unwrap();

        // In Full mode, match_key is added and equals id
        let id = items[0]["id"].as_str().unwrap();
        let match_key = items[0]["match_key"].as_str().unwrap();
        assert_eq!(id, match_key);
        assert_eq!(id, "src/main.rs:TODO:do something");
    }

    #[test]
    fn test_print_diff_json_serialization_path() {
        let diff_result = DiffResult {
            entries: vec![
                DiffEntry {
                    status: DiffStatus::Added,
                    item: make_item("src/new.rs", 5, Tag::Todo, "new item", Priority::Normal),
                },
                DiffEntry {
                    status: DiffStatus::Removed,
                    item: make_item("src/old.rs", 15, Tag::Fixme, "old item", Priority::High),
                },
            ],
            added_count: 1,
            removed_count: 1,
            base_ref: "main".to_string(),
        };

        // Replicate the JSON branch of print_diff
        let mut value: serde_json::Value =
            serde_json::to_value(&diff_result).expect("failed to serialize");
        let detail = DetailLevel::Normal;

        if let Some(entries) = value.get_mut("entries").and_then(|v| v.as_array_mut()) {
            for entry_val in entries.iter_mut() {
                if let Some(item_val) = entry_val.get_mut("item") {
                    apply_detail_to_json_item(item_val, &detail);
                }
            }
        }

        let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let entries = parsed["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);

        // First entry: added
        assert_eq!(entries[0]["status"].as_str().unwrap(), "added");
        assert_eq!(
            entries[0]["item"]["id"].as_str().unwrap(),
            "src/new.rs:TODO:new item"
        );

        // Second entry: removed
        assert_eq!(entries[1]["status"].as_str().unwrap(), "removed");
        assert_eq!(
            entries[1]["item"]["id"].as_str().unwrap(),
            "src/old.rs:FIXME:old item"
        );

        // Top-level fields
        assert_eq!(parsed["added_count"].as_u64().unwrap(), 1);
        assert_eq!(parsed["removed_count"].as_u64().unwrap(), 1);
        assert_eq!(parsed["base_ref"].as_str().unwrap(), "main");
    }

    #[test]
    fn test_print_diff_json_serialization_path_full() {
        let diff_result = DiffResult {
            entries: vec![DiffEntry {
                status: DiffStatus::Added,
                item: make_item("src/a.rs", 1, Tag::Bug, "crash", Priority::Urgent),
            }],
            added_count: 1,
            removed_count: 0,
            base_ref: "develop".to_string(),
        };

        let mut value: serde_json::Value =
            serde_json::to_value(&diff_result).expect("failed to serialize");
        let detail = DetailLevel::Full;

        if let Some(entries) = value.get_mut("entries").and_then(|v| v.as_array_mut()) {
            for entry_val in entries.iter_mut() {
                if let Some(item_val) = entry_val.get_mut("item") {
                    apply_detail_to_json_item(item_val, &detail);
                }
            }
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&serde_json::to_string_pretty(&value).unwrap()).unwrap();
        let entries = parsed["entries"].as_array().unwrap();

        // Full mode adds match_key
        let item = &entries[0]["item"];
        assert_eq!(
            item["id"].as_str().unwrap(),
            item["match_key"].as_str().unwrap()
        );
    }

    #[test]
    fn test_print_check_json_serialization_path() {
        let check_result = CheckResult {
            passed: false,
            total: 15,
            violations: vec![
                CheckViolation {
                    rule: "max_count".to_string(),
                    message: "Total 15 exceeds max 10".to_string(),
                },
                CheckViolation {
                    rule: "blocked_tag".to_string(),
                    message: "Tag HACK is blocked".to_string(),
                },
            ],
        };

        // Replicate the JSON branch of print_check
        let json = serde_json::to_string_pretty(&check_result).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["passed"].as_bool().unwrap(), false);
        assert_eq!(parsed["total"].as_u64().unwrap(), 15);

        let violations = parsed["violations"].as_array().unwrap();
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0]["rule"].as_str().unwrap(), "max_count");
        assert_eq!(
            violations[0]["message"].as_str().unwrap(),
            "Total 15 exceeds max 10"
        );
        assert_eq!(violations[1]["rule"].as_str().unwrap(), "blocked_tag");
    }

    #[test]
    fn test_print_check_json_serialization_path_passed() {
        let check_result = CheckResult {
            passed: true,
            total: 5,
            violations: vec![],
        };

        let json = serde_json::to_string_pretty(&check_result).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["passed"].as_bool().unwrap(), true);
        assert_eq!(parsed["total"].as_u64().unwrap(), 5);
        assert_eq!(parsed["violations"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_print_blame_json_serialization_path() {
        let blame_result = BlameResult {
            entries: vec![
                BlameEntry {
                    item: make_item("src/main.rs", 10, Tag::Todo, "fix later", Priority::Normal),
                    blame: BlameInfo {
                        author: "alice".to_string(),
                        email: "alice@example.com".to_string(),
                        date: "2025-01-15".to_string(),
                        age_days: 90,
                        commit: "abc1234".to_string(),
                    },
                    stale: false,
                },
                BlameEntry {
                    item: make_item("src/lib.rs", 20, Tag::Fixme, "urgent fix", Priority::Urgent),
                    blame: BlameInfo {
                        author: "bob".to_string(),
                        email: "bob@example.com".to_string(),
                        date: "2024-06-01".to_string(),
                        age_days: 365,
                        commit: "def5678".to_string(),
                    },
                    stale: true,
                },
            ],
            total: 2,
            avg_age_days: 227,
            stale_count: 1,
            stale_threshold_days: 180,
        };

        // Replicate the JSON branch of print_blame
        let mut value: serde_json::Value =
            serde_json::to_value(&blame_result).expect("failed to serialize");
        if let Some(entries) = value.get_mut("entries").and_then(|v| v.as_array_mut()) {
            for entry_val in entries.iter_mut() {
                inject_id_field(entry_val);
            }
        }

        let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let entries = parsed["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);

        // BlameEntry uses #[serde(flatten)] on item, so fields are at top level
        assert_eq!(
            entries[0]["id"].as_str().unwrap(),
            "src/main.rs:TODO:fix later"
        );
        assert_eq!(entries[0]["file"].as_str().unwrap(), "src/main.rs");
        assert_eq!(entries[0]["blame"]["author"].as_str().unwrap(), "alice");
        assert_eq!(entries[0]["stale"].as_bool().unwrap(), false);

        assert_eq!(
            entries[1]["id"].as_str().unwrap(),
            "src/lib.rs:FIXME:urgent fix"
        );
        assert_eq!(entries[1]["stale"].as_bool().unwrap(), true);
        assert_eq!(entries[1]["blame"]["age_days"].as_u64().unwrap(), 365);

        // Top-level fields
        assert_eq!(parsed["total"].as_u64().unwrap(), 2);
        assert_eq!(parsed["avg_age_days"].as_u64().unwrap(), 227);
        assert_eq!(parsed["stale_count"].as_u64().unwrap(), 1);
    }

    #[test]
    fn test_print_lint_json_serialization_path() {
        let lint_result = LintResult {
            passed: false,
            total_items: 10,
            violation_count: 2,
            violations: vec![
                LintViolation {
                    rule: "missing_author".to_string(),
                    message: "TODO has no author".to_string(),
                    file: "src/main.rs".to_string(),
                    line: 5,
                    suggestion: Some("Add (author) after tag".to_string()),
                },
                LintViolation {
                    rule: "vague_message".to_string(),
                    message: "Message is too vague".to_string(),
                    file: "src/lib.rs".to_string(),
                    line: 12,
                    suggestion: None,
                },
            ],
        };

        // Replicate the JSON branch of print_lint
        let json = serde_json::to_string_pretty(&lint_result).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["passed"].as_bool().unwrap(), false);
        assert_eq!(parsed["total_items"].as_u64().unwrap(), 10);
        assert_eq!(parsed["violation_count"].as_u64().unwrap(), 2);

        let violations = parsed["violations"].as_array().unwrap();
        assert_eq!(violations.len(), 2);

        assert_eq!(violations[0]["rule"].as_str().unwrap(), "missing_author");
        assert_eq!(violations[0]["file"].as_str().unwrap(), "src/main.rs");
        assert_eq!(violations[0]["line"].as_u64().unwrap(), 5);
        assert_eq!(
            violations[0]["suggestion"].as_str().unwrap(),
            "Add (author) after tag"
        );

        assert_eq!(violations[1]["rule"].as_str().unwrap(), "vague_message");
        assert!(violations[1].get("suggestion").unwrap().is_null());
    }

    #[test]
    fn test_print_lint_json_serialization_path_passed() {
        let lint_result = LintResult {
            passed: true,
            total_items: 5,
            violation_count: 0,
            violations: vec![],
        };

        let json = serde_json::to_string_pretty(&lint_result).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["passed"].as_bool().unwrap(), true);
        assert_eq!(parsed["violation_count"].as_u64().unwrap(), 0);
        assert_eq!(parsed["violations"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_print_clean_json_serialization_path() {
        let clean_result = CleanResult {
            passed: false,
            total_items: 8,
            stale_count: 1,
            duplicate_count: 1,
            violations: vec![
                CleanViolation {
                    rule: "stale".to_string(),
                    message: "TODO is stale (180+ days old)".to_string(),
                    file: "src/main.rs".to_string(),
                    line: 10,
                    issue_ref: Some("#42".to_string()),
                    duplicate_of: None,
                },
                CleanViolation {
                    rule: "duplicate".to_string(),
                    message: "Duplicate TODO found".to_string(),
                    file: "src/lib.rs".to_string(),
                    line: 20,
                    issue_ref: None,
                    duplicate_of: Some("src/main.rs:10".to_string()),
                },
            ],
        };

        // Replicate the JSON branch of print_clean
        let json = serde_json::to_string_pretty(&clean_result).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["passed"].as_bool().unwrap(), false);
        assert_eq!(parsed["total_items"].as_u64().unwrap(), 8);
        assert_eq!(parsed["stale_count"].as_u64().unwrap(), 1);
        assert_eq!(parsed["duplicate_count"].as_u64().unwrap(), 1);

        let violations = parsed["violations"].as_array().unwrap();
        assert_eq!(violations.len(), 2);

        assert_eq!(violations[0]["rule"].as_str().unwrap(), "stale");
        assert_eq!(violations[0]["file"].as_str().unwrap(), "src/main.rs");
        assert_eq!(violations[0]["issue_ref"].as_str().unwrap(), "#42");
        assert!(violations[0].get("duplicate_of").unwrap().is_null());

        assert_eq!(violations[1]["rule"].as_str().unwrap(), "duplicate");
        assert_eq!(
            violations[1]["duplicate_of"].as_str().unwrap(),
            "src/main.rs:10"
        );
        assert!(violations[1].get("issue_ref").unwrap().is_null());
    }

    #[test]
    fn test_print_clean_json_serialization_path_passed() {
        let clean_result = CleanResult {
            passed: true,
            total_items: 5,
            stale_count: 0,
            duplicate_count: 0,
            violations: vec![],
        };

        let json = serde_json::to_string_pretty(&clean_result).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["passed"].as_bool().unwrap(), true);
        assert_eq!(parsed["stale_count"].as_u64().unwrap(), 0);
        assert_eq!(parsed["duplicate_count"].as_u64().unwrap(), 0);
        assert_eq!(parsed["violations"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_print_list_json_with_empty_items() {
        let result = ScanResult {
            items: vec![],
            ignored_items: vec![],
            files_scanned: 0,
        };

        let mut value: serde_json::Value =
            serde_json::to_value(&result).expect("failed to serialize");
        let detail = DetailLevel::Normal;

        if let Some(items) = value.get_mut("items").and_then(|v| v.as_array_mut()) {
            for item_val in items.iter_mut() {
                apply_detail_to_json_item(item_val, &detail);
            }
        }

        let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["items"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["files_scanned"].as_u64().unwrap(), 0);
    }

    #[test]
    fn test_print_diff_json_with_empty_entries() {
        let diff_result = DiffResult {
            entries: vec![],
            added_count: 0,
            removed_count: 0,
            base_ref: "HEAD~1".to_string(),
        };

        let mut value: serde_json::Value =
            serde_json::to_value(&diff_result).expect("failed to serialize");

        if let Some(entries) = value.get_mut("entries").and_then(|v| v.as_array_mut()) {
            for entry_val in entries.iter_mut() {
                if let Some(item_val) = entry_val.get_mut("item") {
                    apply_detail_to_json_item(item_val, &DetailLevel::Normal);
                }
            }
        }

        let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["entries"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["base_ref"].as_str().unwrap(), "HEAD~1");
    }

    #[test]
    fn test_print_search_json_serialization_path() {
        let search_result = SearchResult {
            query: "memory".to_string(),
            exact: false,
            items: vec![
                make_item(
                    "src/alloc.rs",
                    10,
                    Tag::Todo,
                    "fix memory leak",
                    Priority::High,
                ),
                make_item(
                    "src/cache.rs",
                    25,
                    Tag::Fixme,
                    "memory usage too high",
                    Priority::Normal,
                ),
            ],
            match_count: 2,
            file_count: 2,
        };

        // Replicate the JSON branch of print_search
        let mut value: serde_json::Value =
            serde_json::to_value(&search_result).expect("failed to serialize");
        let context_map: HashMap<String, crate::context::ContextInfo> = HashMap::new();
        let has_context = !context_map.is_empty();
        let detail = DetailLevel::Normal;

        if let Some(items) = value.get_mut("items").and_then(|v| v.as_array_mut()) {
            for item_val in items.iter_mut() {
                let file = item_val
                    .get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let line = item_val.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                let key = format!("{}:{}", file, line);
                if has_context {
                    if let Some(ctx) = context_map.get(&key) {
                        let ctx_value =
                            serde_json::to_value(ctx).expect("failed to serialize context");
                        item_val
                            .as_object_mut()
                            .unwrap()
                            .insert("context".to_string(), ctx_value);
                    }
                }
                apply_detail_to_json_item(item_val, &detail);
            }
        }

        let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["query"].as_str().unwrap(), "memory");
        assert_eq!(parsed["exact"].as_bool().unwrap(), false);
        assert_eq!(parsed["match_count"].as_u64().unwrap(), 2);
        assert_eq!(parsed["file_count"].as_u64().unwrap(), 2);

        let items = parsed["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0]["id"].as_str().unwrap(),
            "src/alloc.rs:TODO:fix memory leak"
        );
        assert_eq!(
            items[1]["id"].as_str().unwrap(),
            "src/cache.rs:FIXME:memory usage too high"
        );
        // No context should be injected since context_map is empty
        assert!(items[0].get("context").is_none());
    }

    #[test]
    fn test_print_list_json_with_context() {
        use crate::context::{ContextInfo, ContextLine};

        let result = ScanResult {
            items: vec![make_item(
                "src/main.rs",
                10,
                Tag::Todo,
                "fix this",
                Priority::Normal,
            )],
            ignored_items: vec![],
            files_scanned: 1,
        };

        let mut context_map: HashMap<String, ContextInfo> = HashMap::new();
        context_map.insert(
            "src/main.rs:10".to_string(),
            ContextInfo {
                before: vec![ContextLine {
                    line_number: 9,
                    content: "fn main() {".to_string(),
                }],
                after: vec![ContextLine {
                    line_number: 11,
                    content: "}".to_string(),
                }],
            },
        );

        let has_context = !context_map.is_empty();
        let detail = DetailLevel::Normal;

        let mut value: serde_json::Value =
            serde_json::to_value(&result).expect("failed to serialize");

        if let Some(items) = value.get_mut("items").and_then(|v| v.as_array_mut()) {
            for item_val in items.iter_mut() {
                let file = item_val
                    .get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let line = item_val.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                let key = format!("{}:{}", file, line);
                if has_context {
                    if let Some(ctx) = context_map.get(&key) {
                        let ctx_value =
                            serde_json::to_value(ctx).expect("failed to serialize context");
                        item_val
                            .as_object_mut()
                            .unwrap()
                            .insert("context".to_string(), ctx_value);
                    }
                }
                apply_detail_to_json_item(item_val, &detail);
            }
        }

        let json = serde_json::to_string_pretty(&value).expect("failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let items = parsed["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);

        // Context should be injected
        let ctx = &items[0]["context"];
        assert_eq!(ctx["before"][0]["line_number"].as_u64().unwrap(), 9);
        assert_eq!(ctx["before"][0]["content"].as_str().unwrap(), "fn main() {");
        assert_eq!(ctx["after"][0]["line_number"].as_u64().unwrap(), 11);
        assert_eq!(ctx["after"][0]["content"].as_str().unwrap(), "}");

        // id should still be present
        assert!(items[0].get("id").is_some());
    }
}
