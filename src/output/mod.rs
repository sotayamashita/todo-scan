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
                                cl.content.dimmed()
                            );
                        }
                    }

                    let mut line = if is_file_group {
                        format!("  L{}: [{}] {}", item.line, tag_str, item.message)
                    } else {
                        format!(
                            "  {}:{}: [{}] {}",
                            item.file, item.line, tag_str, item.message
                        )
                    };

                    if *detail != DetailLevel::Minimal {
                        if let Some(ref author) = item.author {
                            line.push_str(&format!(" (@{})", author));
                        }
                        if let Some(ref issue) = item.issue_ref {
                            line.push_str(&format!(" ({})", issue));
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
                                cl.content.dimmed()
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
                        let line = if is_file_group {
                            format!("  L{}: [{}] {}", item.line, tag_str, item.message)
                        } else {
                            format!(
                                "  {}:{}: [{}] {}",
                                item.file, item.line, tag_str, item.message
                            )
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
                                cl.content.dimmed()
                            );
                        }
                    }

                    let mut line = if is_file_group {
                        format!("  L{}: [{}] {}", item.line, tag_str, item.message)
                    } else {
                        format!(
                            "  {}:{}: [{}] {}",
                            item.file, item.line, tag_str, item.message
                        )
                    };

                    if *detail != DetailLevel::Minimal {
                        if let Some(ref author) = item.author {
                            line.push_str(&format!(" (@{})", author));
                        }
                        if let Some(ref issue) = item.issue_ref {
                            line.push_str(&format!(" ({})", issue));
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
                                cl.content.dimmed()
                            );
                        }
                        println!();
                    }
                }
            }

            if is_file_group {
                println!(
                    "{} matches across {} files (query: \"{}\")",
                    result.match_count, result.file_count, result.query
                );
            } else {
                println!(
                    "{} matches across {} groups (query: \"{}\")",
                    result.match_count, group_count, result.query
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
                            cl.content.dimmed()
                        );
                    }
                }

                let tag_str = colorize_tag(&entry.item.tag);
                let line = format!(
                    "{} {}:{} [{}] {}",
                    prefix, entry.item.file, entry.item.line, tag_str, entry.item.message
                );
                println!("{}", color(&line));

                // Print after-context
                if let Some(ctx) = context_map.get(&ctx_key) {
                    for cl in &ctx.after {
                        println!(
                            "    {} {}",
                            format!("{:>4}", cl.line_number).dimmed(),
                            cl.content.dimmed()
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
                        author,
                        count,
                        bar(*count, author_max, 20).dimmed()
                    );
                }
            }

            // Hotspot files
            if !result.hotspot_files.is_empty() {
                println!("\n{}", "Hotspots".bold().underline());
                for (file, count) in &result.hotspot_files {
                    println!("  {} ({})", file, count);
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
                    println!("{}", file.bold().underline());
                    for v in violations {
                        println!("  L{}: {} - {}", v.line, v.rule.yellow(), v.message);
                        if let Some(ref suggestion) = v.suggestion {
                            println!("    {} {}", "suggestion:".dimmed(), suggestion.dimmed());
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
                    println!("{}", file.bold().underline());
                    for v in violations {
                        let mut line =
                            format!("  L{}: {} - {}", v.line, v.rule.yellow(), v.message);
                        if let Some(ref dup_of) = v.duplicate_of {
                            line.push_str(&format!(" (duplicate of {})", dup_of));
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
                    println!("  {}: {}", violation.rule.yellow(), violation.message);
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
                println!("{}", file.bold().underline());
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
                        entry.item.message,
                        entry.blame.author,
                        entry.blame.date,
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
                    cl.content.dimmed()
                );
            }

            println!(
                "  {} {}",
                format!("{:>4}", rich.line).cyan(),
                rich.todo_line
            );

            for cl in &rich.after {
                println!(
                    "  {} {}",
                    format!("{:>4}", cl.line_number).dimmed(),
                    cl.content.dimmed()
                );
            }

            if !rich.related_todos.is_empty() {
                println!();
                println!("{}", "Related TODOs:".bold());
                for rt in &rich.related_todos {
                    println!("  L{}: [{}] {}", rt.line, rt.tag, rt.message);
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
            println!("{} {}", event.timestamp.dimmed(), event.file.bold());

            for item in &event.added {
                let tag_str = colorize_tag(&item.tag);
                println!(
                    "  {} L{}: [{}] {}",
                    "+".green(),
                    item.line,
                    tag_str,
                    item.message
                );
            }

            for item in &event.removed {
                let tag_str = colorize_tag(&item.tag);
                println!(
                    "  {} L{}: [{}] {}",
                    "-".red(),
                    item.line,
                    tag_str,
                    item.message
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
                    task.metadata.todo_scan_tag,
                    task.metadata.todo_scan_file,
                    task.metadata.todo_scan_line,
                    task.subject,
                );
            }

            println!("\n{} tasks exported", result.total);
            if let Some(ref dir) = result.output_dir {
                println!("Output: {}", dir);
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
                    format!("Relationships for {}", target).bold().underline()
                );
            }

            if let Some(ref clusters) = result.clusters {
                for cluster in clusters {
                    println!(
                        "\n{}",
                        format!("Cluster {} — {}", cluster.id, cluster.theme)
                            .bold()
                            .underline()
                    );
                    println!("  Items (suggested order):");
                    for loc in &cluster.suggested_order {
                        println!("    {}", loc);
                    }
                    if !cluster.relationships.is_empty() {
                        println!("  Relationships:");
                        for rel in &cluster.relationships {
                            println!(
                                "    {} ↔ {} (score: {:.2}, {})",
                                rel.from, rel.to, rel.score, rel.reason
                            );
                        }
                    }
                }
            } else {
                for rel in &result.relationships {
                    println!(
                        "  {} ↔ {} (score: {:.2}, {})",
                        rel.from, rel.to, rel.score, rel.reason
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

pub fn print_report(report: &ReportResult, output_path: &str) -> std::io::Result<()> {
    let content = html::render_html(report);
    std::fs::write(output_path, content)?;
    println!("Report written to {}", output_path);
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
                    pkg.name, pkg.path, pkg.todo_count, max_str, status_str
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
