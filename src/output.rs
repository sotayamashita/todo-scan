use colored::*;

use crate::cli::Format;
use crate::model::*;
use std::collections::BTreeMap;

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

pub fn print_list(result: &ScanResult, format: &Format) {
    match format {
        Format::Text => {
            let mut grouped: BTreeMap<&str, Vec<&TodoItem>> = BTreeMap::new();
            for item in &result.items {
                grouped.entry(&item.file).or_default().push(item);
            }

            let file_count = grouped.len();

            for (file, items) in &grouped {
                println!("{}", file.bold().underline());
                for item in items {
                    let tag_str = colorize_tag(&item.tag);
                    let mut line = format!("  L{}: [{}] {}", item.line, tag_str, item.message);

                    if let Some(ref author) = item.author {
                        line.push_str(&format!(" (@{})", author));
                    }
                    if let Some(ref issue) = item.issue_ref {
                        line.push_str(&format!(" (#{})", issue));
                    }

                    println!("{}", line);
                }
            }

            println!(
                "\n{} items in {} files",
                result.items.len(),
                file_count
            );
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(result).expect("failed to serialize");
            println!("{}", json);
        }
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
    }
}
