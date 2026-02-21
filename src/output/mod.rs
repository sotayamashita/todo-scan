mod github_actions;
mod markdown;
mod sarif;

use colored::*;

use crate::cli::{Format, GroupBy};
use crate::model::*;
use std::path::Path;

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

pub fn print_list(result: &ScanResult, format: &Format, group_by: &GroupBy) {
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
                    let mut line = if is_file_group {
                        format!("  L{}: [{}] {}", item.line, tag_str, item.message)
                    } else {
                        format!(
                            "  {}:{}: [{}] {}",
                            item.file, item.line, tag_str, item.message
                        )
                    };

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

                    println!("{}", line);
                }
            }

            if is_file_group {
                println!("\n{} items in {} files", result.items.len(), group_count);
            } else {
                println!("\n{} items in {} groups", result.items.len(), group_count);
            }
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
        Format::GithubActions => print!("{}", github_actions::format_list(result)),
        Format::Sarif => print!("{}", sarif::format_list(result)),
        Format::Markdown => print!("{}", markdown::format_list(result)),
    }
}

pub fn print_diff(result: &DiffResult, format: &Format) {
    match format {
        Format::Text => {
            for entry in &result.entries {
                let (prefix, color): (&str, fn(&str) -> ColoredString) = match entry.status {
                    DiffStatus::Added => ("+", |s: &str| s.green()),
                    DiffStatus::Removed => ("-", |s: &str| s.red()),
                };

                let tag_str = colorize_tag(&entry.item.tag);
                let line = format!(
                    "{} {}:{} [{}] {}",
                    prefix, entry.item.file, entry.item.line, tag_str, entry.item.message
                );
                println!("{}", color(&line));
            }

            println!(
                "\n+{} -{} (base: {})",
                result.added_count, result.removed_count, result.base_ref
            );
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
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
