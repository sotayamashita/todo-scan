use anyhow::Result;
use ignore::{WalkBuilder, WalkState};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use crate::cache::ScanCache;
use crate::config::Config;
use crate::deadline::{parse_deadline, Deadline};
use crate::model::{Priority, ScanResult, Tag, TodoItem};

static ISSUE_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:([A-Z]+-\d+)|#(\d+))").unwrap());

/// Extract an issue reference from the message text.
/// Matches patterns like #123 or JIRA-456.
fn extract_issue_ref(message: &str) -> Option<String> {
    ISSUE_REF_RE.captures(message).map(|caps| {
        caps.get(1)
            .or_else(|| caps.get(2))
            .map(|m| {
                if caps.get(1).is_some() {
                    m.as_str().to_string()
                } else {
                    format!("#{}", m.as_str())
                }
            })
            .unwrap()
    })
}

/// Comment prefixes that can appear anywhere before the tag on the line.
const COMMENT_PREFIXES: &[&str] = &["//", "#", "/*", "--", "<!--", ";", "(*", "{-", "%"];

/// Prefixes that only match at line start (after trimming whitespace).
const LINE_START_PREFIXES: &[&str] = &["*"];

/// Parse the parenthesized content after a tag.
/// Returns `(author, deadline)` extracted from the content.
///
/// Supported formats:
/// - `"alice"` → author only
/// - `"2025-06-01"` → deadline only
/// - `"alice, 2025-06-01"` → both author and deadline
fn parse_paren_content(s: &str) -> (Option<String>, Option<Deadline>) {
    let s = s.trim();
    if s.is_empty() {
        return (None, None);
    }

    // Check if there's a comma separating author and date
    if let Some(idx) = s.find(',') {
        let left = s[..idx].trim();
        let right = s[idx + 1..].trim();

        // Try date on the right side
        if let Some(deadline) = parse_deadline(right) {
            let author = if left.is_empty() {
                None
            } else {
                Some(left.to_string())
            };
            return (author, Some(deadline));
        }

        // Try date on the left side
        if let Some(deadline) = parse_deadline(left) {
            let author = if right.is_empty() {
                None
            } else {
                Some(right.to_string())
            };
            return (author, Some(deadline));
        }

        // Neither side is a date; treat the whole thing as the author
        return (Some(s.to_string()), None);
    }

    // No comma: try as a date first, otherwise treat as author
    if let Some(deadline) = parse_deadline(s) {
        return (None, Some(deadline));
    }

    (Some(s.to_string()), None)
}

/// Returns true if the prefix at `pos` in `text` is outside any string literal,
/// using a quote-parity heuristic (even number of `"` before the position).
fn prefix_outside_quotes(text: &str, pos: usize) -> bool {
    text[..pos].chars().filter(|&c| c == '"').count() % 2 == 0
}

/// Heuristic: does the tag at `tag_start` appear to be inside a comment?
pub(crate) fn is_in_comment(line: &str, tag_start: usize) -> bool {
    let before_tag = &line[..tag_start];
    for prefix in COMMENT_PREFIXES {
        let mut start = 0;
        while let Some(pos) = before_tag[start..].find(prefix) {
            let abs_pos = start + pos;
            if prefix_outside_quotes(before_tag, abs_pos) {
                return true;
            }
            start = abs_pos + prefix.len();
        }
    }
    let trimmed = before_tag.trim_start();
    if LINE_START_PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
        let leading_ws = before_tag.len() - trimmed.len();
        return prefix_outside_quotes(before_tag, leading_ws);
    }
    false
}

/// Result of scanning content, separating normal items from suppressed ones.
pub struct ScanContentResult {
    pub items: Vec<TodoItem>,
    pub ignored_items: Vec<TodoItem>,
}

/// The inline suppression marker for the current line.
const IGNORE_MARKER: &str = "todo-scan:ignore";

/// The inline suppression marker for the next line.
const IGNORE_NEXT_LINE_MARKER: &str = "todo-scan:ignore-next-line";

/// Scan text content line by line for TODO-style comments.
///
/// Pure function: takes content, a file path label, and a compiled regex.
/// Returns a `ScanContentResult` with matched items and suppressed items separated.
///
/// Suppression markers:
/// - `todo-scan:ignore` on the same line as a TODO suppresses that item
/// - `todo-scan:ignore-next-line` on any line suppresses the immediately following line
pub fn scan_content(content: &str, file_path: &str, pattern: &Regex) -> ScanContentResult {
    let lines: Vec<&str> = content.lines().collect();

    // Pre-scan for todo-scan:ignore-next-line markers
    let mut suppressed_lines: HashSet<usize> = HashSet::new();
    for (idx, line) in lines.iter().enumerate() {
        if line.contains(IGNORE_NEXT_LINE_MARKER) {
            // Only suppress the immediately next line (no blank lines between)
            let next_idx = idx + 1;
            if next_idx < lines.len() && !lines[next_idx].trim().is_empty() {
                suppressed_lines.insert(next_idx);
            }
        }
    }

    let mut items = Vec::new();
    let mut ignored_items = Vec::new();

    for (line_idx, line) in lines.iter().enumerate() {
        if let Some(caps) = pattern.captures(line) {
            let tag_match = caps.get(1).unwrap();
            if !is_in_comment(line, tag_match.start()) {
                continue;
            }

            // Skip if the tag is immediately followed by a hyphen (e.g., "todo-scan:ignore")
            if line.as_bytes().get(tag_match.end()) == Some(&b'-') {
                continue;
            }

            let tag_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let tag = match tag_str.parse::<Tag>() {
                Ok(t) => t,
                Err(_) => continue,
            };

            let (author, deadline) = match caps.get(2) {
                Some(m) => parse_paren_content(m.as_str()),
                None => (None, None),
            };

            let priority = match caps.get(3).map(|m| m.as_str()) {
                Some("!!") => Priority::Urgent,
                Some("!") => Priority::High,
                _ => Priority::Normal,
            };

            let mut message = caps
                .get(4)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            // Check if this line is suppressed
            let has_inline_ignore =
                line.contains(IGNORE_MARKER) && !line.contains(IGNORE_NEXT_LINE_MARKER);
            let is_next_line_suppressed = suppressed_lines.contains(&line_idx);
            let is_suppressed = has_inline_ignore || is_next_line_suppressed;

            // Strip trailing todo-scan:ignore from message text
            if has_inline_ignore {
                if let Some(pos) = message.find(IGNORE_MARKER) {
                    message = message[..pos].trim().to_string();
                }
            }

            let issue_ref = extract_issue_ref(&message);

            let item = TodoItem {
                file: file_path.to_string(),
                line: line_idx + 1,
                tag,
                message,
                author,
                issue_ref,
                priority,
                deadline,
            };

            if is_suppressed {
                ignored_items.push(item);
            } else {
                items.push(item);
            }
        }
    }

    ScanContentResult {
        items,
        ignored_items,
    }
}

/// Walk a directory tree and scan all files for TODO-style comments.
///
/// Respects `.gitignore` via `ignore::WalkBuilder`. Applies the exclude
/// directories and exclude patterns from `Config`. Returns a `ScanResult`
/// with every matched item and the total number of files scanned.
pub fn scan_directory(root: &Path, config: &Config) -> Result<ScanResult> {
    let pattern_str = config.tags_pattern();
    let pattern = Regex::new(&pattern_str)?;

    let exclude_regexes: Vec<Regex> = config
        .exclude_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    let items = Arc::new(Mutex::new(Vec::new()));
    let ignored_items = Arc::new(Mutex::new(Vec::new()));
    let files_scanned = Arc::new(AtomicUsize::new(0));
    let exclude_dirs = Arc::new(config.exclude_dirs.clone());
    let exclude_regexes = Arc::new(exclude_regexes);
    let root = root.to_path_buf();

    let walker = WalkBuilder::new(&root).build_parallel();

    walker.run(|| {
        let items = Arc::clone(&items);
        let ignored_items = Arc::clone(&ignored_items);
        let files_scanned = Arc::clone(&files_scanned);
        let exclude_dirs = Arc::clone(&exclude_dirs);
        let exclude_regexes = Arc::clone(&exclude_regexes);
        let pattern = pattern.clone();
        let root = root.clone();

        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };

            let path = entry.path();

            if !path.is_file() {
                return WalkState::Continue;
            }

            // Check exclude_dirs
            let should_exclude_dir = exclude_dirs.iter().any(|dir| {
                path.components()
                    .any(|c| c.as_os_str().to_str().map(|s| s == dir).unwrap_or(false))
            });
            if should_exclude_dir {
                return WalkState::Continue;
            }

            // Check exclude_patterns against the path string
            let path_str = path.to_string_lossy();
            let should_exclude_pattern = exclude_regexes.iter().any(|re| re.is_match(&path_str));
            if should_exclude_pattern {
                return WalkState::Continue;
            }

            // Read the file; skip binary or unreadable files
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => return WalkState::Continue,
            };

            let relative_path = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            let result = scan_content(&content, &relative_path, &pattern);
            if !result.items.is_empty() {
                items
                    .lock()
                    .expect("scan thread panicked")
                    .extend(result.items);
            }
            if !result.ignored_items.is_empty() {
                ignored_items
                    .lock()
                    .expect("scan thread panicked")
                    .extend(result.ignored_items);
            }
            files_scanned.fetch_add(1, Ordering::Relaxed);

            WalkState::Continue
        })
    });

    let items = Arc::try_unwrap(items)
        .expect("all walker threads should have finished")
        .into_inner()
        .unwrap();
    let ignored_items = Arc::try_unwrap(ignored_items)
        .expect("all walker threads should have finished")
        .into_inner()
        .unwrap();
    let files_scanned = files_scanned.load(Ordering::Relaxed);

    Ok(ScanResult {
        items,
        ignored_items,
        files_scanned,
    })
}

/// Result of a cached scan, wrapping ScanResult with cache statistics.
pub struct CachedScanResult {
    pub result: ScanResult,
    #[allow(dead_code)]
    pub cache_hits: usize,
    #[allow(dead_code)]
    pub cache_misses: usize,
}

/// Scan a directory using a two-layer cache (mtime + content hash).
///
/// Uses sequential walk because the cache requires `&mut ScanCache`.
/// When cache is warm, most files are skipped via mtime check (no file I/O),
/// so parallelism provides diminishing returns.
pub fn scan_directory_cached(
    root: &Path,
    config: &Config,
    cache: &mut ScanCache,
) -> Result<CachedScanResult> {
    let pattern_str = config.tags_pattern();
    let pattern = Regex::new(&pattern_str)?;

    let exclude_regexes: Vec<Regex> = config
        .exclude_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    let mut items = Vec::new();
    let mut ignored_items = Vec::new();
    let mut files_scanned: usize = 0;
    let mut cache_hits: usize = 0;
    let mut cache_misses: usize = 0;
    let mut seen_paths = HashSet::new();

    let walker = WalkBuilder::new(root).build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        // Check exclude_dirs
        let should_exclude_dir = config.exclude_dirs.iter().any(|dir| {
            path.components()
                .any(|c| c.as_os_str().to_str().map(|s| s == dir).unwrap_or(false))
        });
        if should_exclude_dir {
            continue;
        }

        // Check exclude_patterns
        let path_str = path.to_string_lossy();
        let should_exclude_pattern = exclude_regexes.iter().any(|re| re.is_match(&path_str));
        if should_exclude_pattern {
            continue;
        }

        let relative_path = path.strip_prefix(root).unwrap_or(path).to_path_buf();

        seen_paths.insert(relative_path.clone());

        // Layer 1: mtime check
        let metadata = match path.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = metadata
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        if let Some(cached) = cache.check(&relative_path, mtime) {
            items.extend(cached.items.iter().cloned());
            ignored_items.extend(cached.ignored_items.iter().cloned());
            files_scanned += 1;
            cache_hits += 1;
            continue;
        }

        // Read file content
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => {
                files_scanned += 1;
                continue;
            }
        };

        // Layer 2: content hash check
        let content_bytes = content.as_bytes();
        if let Some(cached) = cache.check_with_content(&relative_path, content_bytes) {
            // Content unchanged (mtime was different, e.g. touched file)
            // Clone first to release the immutable borrow on cache
            let cloned_items: Vec<TodoItem> = cached.items.to_vec();
            let cloned_ignored: Vec<TodoItem> = cached.ignored_items.to_vec();
            // Update mtime in cache so next time layer 1 hits
            let content_hash = *blake3::hash(content_bytes).as_bytes();
            cache.insert(
                relative_path.clone(),
                content_hash,
                cloned_items.clone(),
                cloned_ignored.clone(),
                mtime,
            );
            items.extend(cloned_items);
            ignored_items.extend(cloned_ignored);
            files_scanned += 1;
            cache_hits += 1;
            continue;
        }

        // Cache miss: full scan
        let relative_str = relative_path.to_string_lossy().to_string();
        let result = scan_content(&content, &relative_str, &pattern);
        let content_hash = *blake3::hash(content_bytes).as_bytes();
        cache.insert(
            relative_path,
            content_hash,
            result.items.clone(),
            result.ignored_items.clone(),
            mtime,
        );
        items.extend(result.items);
        ignored_items.extend(result.ignored_items);
        files_scanned += 1;
        cache_misses += 1;
    }

    // Prune deleted files
    cache.prune(&seen_paths);

    Ok(CachedScanResult {
        result: ScanResult {
            items,
            ignored_items,
            files_scanned,
        },
        cache_hits,
        cache_misses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_pattern() -> Regex {
        let config = Config::default();
        Regex::new(&config.tags_pattern()).unwrap()
    }

    #[test]
    fn test_basic_todo_detection() {
        let pattern = default_pattern();
        let content = "// TODO: implement this feature\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].tag, Tag::Todo);
        assert_eq!(result.items[0].message, "implement this feature");
        assert_eq!(result.items[0].file, "test.rs");
        assert_eq!(result.items[0].line, 1);
        assert_eq!(result.items[0].priority, Priority::Normal);
        assert!(result.items[0].author.is_none());
    }

    #[test]
    fn test_fixme_with_author() {
        let pattern = default_pattern();
        let content = "// FIXME(alice): broken parsing logic\n";
        let result = scan_content(content, "lib.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].tag, Tag::Fixme);
        assert_eq!(result.items[0].author.as_deref(), Some("alice"));
        assert_eq!(result.items[0].message, "broken parsing logic");
    }

    #[test]
    fn test_priority_high() {
        let pattern = default_pattern();
        let content = "# TODO: ! fix memory leak\n";
        let result = scan_content(content, "main.py", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].priority, Priority::High);
    }

    #[test]
    fn test_priority_urgent() {
        let pattern = default_pattern();
        let content = "// BUG: !! crashes on empty input\n";
        let result = scan_content(content, "app.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].tag, Tag::Bug);
        assert_eq!(result.items[0].priority, Priority::Urgent);
    }

    #[test]
    fn test_issue_ref_hash() {
        let pattern = default_pattern();
        let content = "// TODO: fix layout issue #123\n";
        let result = scan_content(content, "ui.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].issue_ref.as_deref(), Some("#123"));
    }

    #[test]
    fn test_issue_ref_jira() {
        let pattern = default_pattern();
        let content = "// FIXME: address JIRA-456 regression\n";
        let result = scan_content(content, "api.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].issue_ref.as_deref(), Some("JIRA-456"));
    }

    #[test]
    fn test_case_insensitivity() {
        let pattern = default_pattern();
        let content = "// todo: lowercase tag\n// Todo: mixed case\n// TODO: uppercase\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 3);
        for item in &result.items {
            assert_eq!(item.tag, Tag::Todo);
        }
    }

    #[test]
    fn test_multiple_tags_in_content() {
        let pattern = default_pattern();
        let content = "\
// TODO: first task
fn foo() {}
// FIXME(bob): second task
// HACK: workaround for upstream bug
// NOTE: remember to update docs
";
        let result = scan_content(content, "multi.rs", &pattern);

        assert_eq!(result.items.len(), 4);
        assert_eq!(result.items[0].tag, Tag::Todo);
        assert_eq!(result.items[1].tag, Tag::Fixme);
        assert_eq!(result.items[1].author.as_deref(), Some("bob"));
        assert_eq!(result.items[2].tag, Tag::Hack);
        assert_eq!(result.items[3].tag, Tag::Note);
    }

    #[test]
    fn test_line_numbers_are_correct() {
        let pattern = default_pattern();
        let content = "\
line one
// TODO: on line two
line three
line four
// FIXME: on line five
";
        let result = scan_content(content, "lines.rs", &pattern);

        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].line, 2);
        assert_eq!(result.items[1].line, 5);
    }

    #[test]
    fn test_xxx_tag() {
        let pattern = default_pattern();
        let content = "// XXX: dangerous code path\n";
        let result = scan_content(content, "danger.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].tag, Tag::Xxx);
    }

    #[test]
    fn test_no_match_on_plain_text() {
        let pattern = default_pattern();
        let content = "This is just a regular comment with no tags.\n";
        let result = scan_content(content, "plain.rs", &pattern);

        assert!(result.items.is_empty());
    }

    #[test]
    fn test_author_with_special_chars() {
        let pattern = default_pattern();
        let content = "// TODO(user@domain.com): email-style author\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].author.as_deref(), Some("user@domain.com"));
    }

    #[test]
    fn test_extract_issue_ref_function() {
        assert_eq!(extract_issue_ref("fix #42"), Some("#42".to_string()));
        assert_eq!(
            extract_issue_ref("see PROJ-100"),
            Some("PROJ-100".to_string())
        );
        assert_eq!(extract_issue_ref("no reference here"), None);
    }

    // --- False-positive rejection tests ---

    #[test]
    fn test_no_match_in_identifier() {
        let pattern = default_pattern();
        let content = "let service = TodoService::new();\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match TODO inside identifier"
        );
    }

    #[test]
    fn test_no_match_in_camel_case() {
        let pattern = default_pattern();
        let content = "if isTodoCompleted() { return; }\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match Todo in camelCase"
        );
    }

    #[test]
    fn test_no_match_in_string_literal() {
        let pattern = default_pattern();
        let content = "let msg = \"TODO: not a real comment\";\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match TODO inside string literal"
        );
    }

    #[test]
    fn test_no_match_in_plain_code() {
        let pattern = default_pattern();
        let content = "let todo_count = get_todos().len();\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match todo in variable name"
        );
    }

    #[test]
    fn test_no_match_enum_variant() {
        let pattern = default_pattern();
        let content = "enum State { Todo, InProgress, Done }\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match Todo enum variant"
        );
    }

    #[test]
    fn test_no_match_struct_name() {
        let pattern = default_pattern();
        let content = "struct TodoItem { title: String }\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match Todo in struct name"
        );
    }

    #[test]
    fn test_no_match_comment_prefix_in_string_literal() {
        let pattern = default_pattern();
        let content = r#"let s = "// TODO: not real";"#;
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match TODO when // is inside a string literal"
        );
    }

    #[test]
    fn test_no_match_hash_prefix_in_string_literal() {
        let pattern = default_pattern();
        let content = r##"let s = "# TODO: not real";"##;
        let result = scan_content(content, "test.py", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match TODO when # is inside a string literal"
        );
    }

    #[test]
    fn test_match_real_comment_after_quoted_prefix() {
        let pattern = default_pattern();
        let content = r#""//"; // TODO: fix this"#;
        let result = scan_content(content, "test.rs", &pattern);
        assert_eq!(
            result.items.len(),
            1,
            "should match the real comment after quoted prefix"
        );
        assert_eq!(result.items[0].message, "fix this");
    }

    // --- Comment detection tests (various languages) ---

    #[test]
    fn test_comment_double_slash() {
        let pattern = default_pattern();
        let content = "// TODO: rust/js/c++ style comment\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_comment_hash() {
        let pattern = default_pattern();
        let content = "# TODO: python/ruby/shell style comment\n";
        let result = scan_content(content, "test.py", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_comment_block_start() {
        let pattern = default_pattern();
        let content = "/* TODO: c-style block comment */\n";
        let result = scan_content(content, "test.c", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_comment_block_middle_star() {
        let pattern = default_pattern();
        let content = " * TODO: middle of block comment\n";
        let result = scan_content(content, "test.java", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_comment_double_dash() {
        let pattern = default_pattern();
        let content = "-- TODO: sql/haskell style comment\n";
        let result = scan_content(content, "test.sql", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_comment_percent() {
        let pattern = default_pattern();
        let content = "% TODO: latex/erlang style comment\n";
        let result = scan_content(content, "test.erl", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_comment_html() {
        let pattern = default_pattern();
        let content = "<!-- TODO: html comment -->\n";
        let result = scan_content(content, "test.html", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_comment_semicolon() {
        let pattern = default_pattern();
        let content = "; TODO: lisp/asm style comment\n";
        let result = scan_content(content, "test.lisp", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_comment_ocaml_paren_star() {
        let pattern = default_pattern();
        let content = "(* TODO: ocaml/pascal style comment *)\n";
        let result = scan_content(content, "test.ml", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_comment_haskell_brace_dash() {
        let pattern = default_pattern();
        let content = "{- TODO: haskell block comment -}\n";
        let result = scan_content(content, "test.hs", &pattern);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_indented_comment() {
        let pattern = default_pattern();
        let content = "    // TODO: indented with spaces\n\t# FIXME: indented with tab\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert_eq!(result.items.len(), 2);
    }

    #[test]
    fn test_inline_comment() {
        let pattern = default_pattern();
        let content = "let x = 42; // TODO: fix this value\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].message, "fix this value");
    }

    // --- is_in_comment() direct tests ---

    #[test]
    fn test_is_in_comment_double_slash() {
        assert!(is_in_comment("// TODO: test", 3));
    }

    #[test]
    fn test_is_in_comment_hash() {
        assert!(is_in_comment("# TODO: test", 2));
    }

    #[test]
    fn test_is_in_comment_block_start() {
        assert!(is_in_comment("/* TODO: test */", 3));
    }

    #[test]
    fn test_is_in_comment_star_line_start() {
        assert!(is_in_comment(" * TODO: test", 3));
    }

    #[test]
    fn test_is_in_comment_html() {
        assert!(is_in_comment("<!-- TODO: test -->", 5));
    }

    #[test]
    fn test_is_in_comment_inline() {
        assert!(is_in_comment("let x = 1; // TODO: fix", 15));
    }

    #[test]
    fn test_is_in_comment_false_for_code() {
        assert!(!is_in_comment("let todo_count = 0;", 4));
    }

    #[test]
    fn test_is_in_comment_false_for_string() {
        assert!(!is_in_comment("let s = \"TODO: test\";", 9));
    }

    #[test]
    fn test_is_in_comment_false_for_identifier() {
        assert!(!is_in_comment("TodoService::new()", 0));
    }

    #[test]
    fn test_is_in_comment_false_for_string_with_comment_prefix() {
        // "// TODO" inside a string literal should not be detected as a comment
        assert!(!is_in_comment(r#"let s = "// TODO: test";"#, 12));
    }

    #[test]
    fn test_is_in_comment_inline_after_code() {
        // Real inline comment after code → should match
        assert!(is_in_comment("let x = 1; // TODO: fix this", 15));
    }

    #[test]
    fn test_is_in_comment_false_for_string_with_hash_prefix() {
        // "# TODO" inside a string literal should not be detected as a comment
        assert!(!is_in_comment(r##"let s = "# TODO: test";"##, 11));
    }

    #[test]
    fn test_is_in_comment_quoted_prefix_then_real_comment() {
        // "//"; // TODO — quoted prefix then real comment → should match
        assert!(is_in_comment(r#""//"; // TODO: fix"#, 10));
    }

    #[test]
    fn test_is_in_comment_false_for_string_with_block_comment_prefix() {
        // "/* TODO" inside a string literal should not be detected as a comment
        assert!(!is_in_comment(r#"let s = "/* TODO: test";"#, 12));
    }

    // --- scan_directory() tests ---

    #[test]
    fn test_scan_directory_basic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.rs"), "// TODO: basic test\n").unwrap();

        let config = Config::default();
        let result = scan_directory(dir.path(), &config).unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].tag, Tag::Todo);
        assert_eq!(result.items[0].message, "basic test");
        assert_eq!(result.files_scanned, 1);
    }

    #[test]
    fn test_scan_directory_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..10 {
            std::fs::write(
                dir.path().join(format!("file_{i}.rs")),
                format!("// TODO: task {i}\n"),
            )
            .unwrap();
        }

        let config = Config::default();
        let result = scan_directory(dir.path(), &config).unwrap();

        assert_eq!(result.items.len(), 10);
        assert_eq!(result.files_scanned, 10);
    }

    #[test]
    fn test_scan_directory_exclude_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.rs"), "// TODO: keep this\n").unwrap();
        let vendor = dir.path().join("vendor");
        std::fs::create_dir(&vendor).unwrap();
        std::fs::write(vendor.join("skip.rs"), "// TODO: skip this\n").unwrap();

        let config = Config {
            exclude_dirs: vec!["vendor".to_string()],
            ..Config::default()
        };
        let result = scan_directory(dir.path(), &config).unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].message, "keep this");
    }

    #[test]
    fn test_scan_directory_files_scanned_count() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("with_todo.rs"), "// TODO: has todo\n").unwrap();
        std::fs::write(dir.path().join("no_todo.rs"), "fn main() {}\n").unwrap();

        let config = Config::default();
        let result = scan_directory(dir.path(), &config).unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.files_scanned, 2);
    }

    // --- parse_paren_content tests ---

    #[test]
    fn test_parse_paren_author_only() {
        let (author, deadline) = parse_paren_content("alice");
        assert_eq!(author.as_deref(), Some("alice"));
        assert!(deadline.is_none());
    }

    #[test]
    fn test_parse_paren_date_only() {
        let (author, deadline) = parse_paren_content("2025-06-01");
        assert!(author.is_none());
        let d = deadline.unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 6);
        assert_eq!(d.day, 1);
    }

    #[test]
    fn test_parse_paren_author_and_date() {
        let (author, deadline) = parse_paren_content("alice, 2025-06-01");
        assert_eq!(author.as_deref(), Some("alice"));
        let d = deadline.unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 6);
        assert_eq!(d.day, 1);
    }

    #[test]
    fn test_parse_paren_quarter_format() {
        let (author, deadline) = parse_paren_content("2025-Q2");
        assert!(author.is_none());
        let d = deadline.unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 6);
        assert_eq!(d.day, 30);
    }

    #[test]
    fn test_parse_paren_author_and_quarter() {
        let (author, deadline) = parse_paren_content("bob, 2025-Q3");
        assert_eq!(author.as_deref(), Some("bob"));
        let d = deadline.unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 9);
        assert_eq!(d.day, 30);
    }

    #[test]
    fn test_parse_paren_empty() {
        let (author, deadline) = parse_paren_content("");
        assert!(author.is_none());
        assert!(deadline.is_none());
    }

    // --- Scanning TODOs with dates ---

    #[test]
    fn test_scan_todo_with_date() {
        let pattern = default_pattern();
        let content = "// TODO(2025-06-01): finish this by June\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].tag, Tag::Todo);
        assert!(result.items[0].author.is_none());
        let d = result.items[0].deadline.unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 6);
        assert_eq!(d.day, 1);
        assert_eq!(result.items[0].message, "finish this by June");
    }

    #[test]
    fn test_scan_todo_with_author_and_date() {
        let pattern = default_pattern();
        let content = "// TODO(alice, 2025-06-01): finish this\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].author.as_deref(), Some("alice"));
        let d = result.items[0].deadline.unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 6);
        assert_eq!(d.day, 1);
    }

    #[test]
    fn test_scan_todo_with_quarter() {
        let pattern = default_pattern();
        let content = "// TODO(2025-Q4): year-end cleanup\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].author.is_none());
        let d = result.items[0].deadline.unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 12);
        assert_eq!(d.day, 31);
    }

    #[test]
    fn test_scan_todo_author_only_still_works() {
        let pattern = default_pattern();
        let content = "// TODO(bob): no date here\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].author.as_deref(), Some("bob"));
        assert!(result.items[0].deadline.is_none());
    }

    #[test]
    fn test_scan_todo_no_parens_no_deadline() {
        let pattern = default_pattern();
        let content = "// TODO: plain task\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].author.is_none());
        assert!(result.items[0].deadline.is_none());
    }

    // --- Word boundary after tag: false-positive rejection ---

    #[test]
    fn test_no_match_todox_in_comment() {
        let pattern = default_pattern();
        let content = "// todox report generates HTML\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match TODO as prefix of 'todox'"
        );
    }

    #[test]
    fn test_no_match_todos_in_comment() {
        let pattern = default_pattern();
        let content = "// TODOS remaining in the backlog\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match TODO as prefix of 'TODOS'"
        );
    }

    #[test]
    fn test_no_match_noted_in_comment() {
        let pattern = default_pattern();
        let content = "# NOTEd this for future reference\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match NOTE as prefix of 'NOTEd'"
        );
    }

    #[test]
    fn test_no_match_fixme_suffix_in_comment() {
        let pattern = default_pattern();
        let content = "// FIXMEd the issue yesterday\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert!(
            result.items.is_empty(),
            "should not match FIXME as prefix of 'FIXMEd'"
        );
    }

    // --- Word boundary after tag: legitimate patterns still match ---

    #[test]
    fn test_still_matches_todo_colon() {
        let pattern = default_pattern();
        let content = "// TODO: fix this\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert_eq!(result.items.len(), 1, "TODO: should still match");
    }

    #[test]
    fn test_still_matches_todo_paren() {
        let pattern = default_pattern();
        let content = "// TODO(alice): fix this\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert_eq!(result.items.len(), 1, "TODO(author) should still match");
    }

    #[test]
    fn test_still_matches_todo_space() {
        let pattern = default_pattern();
        let content = "// TODO fix this\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert_eq!(
            result.items.len(),
            1,
            "TODO followed by space should still match"
        );
    }

    #[test]
    fn test_still_matches_todo_bang() {
        let pattern = default_pattern();
        let content = "// TODO! fix this\n";
        let result = scan_content(content, "test.rs", &pattern);
        assert_eq!(result.items.len(), 1, "TODO! should still match");
    }

    // --- scan_directory_cached tests ---

    #[test]
    fn test_cached_scan_first_run_all_misses() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "// TODO: task a\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "// FIXME: task b\n").unwrap();

        let config = Config::default();
        let config_hash = ScanCache::config_hash(&config);
        let mut cache = ScanCache::new(config_hash);

        let result = scan_directory_cached(dir.path(), &config, &mut cache).unwrap();

        assert_eq!(result.result.items.len(), 2);
        assert_eq!(result.cache_hits, 0);
        assert_eq!(result.cache_misses, 2);
        assert_eq!(cache.entries.len(), 2);
    }

    #[test]
    fn test_cached_scan_second_run_all_hits() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "// TODO: task a\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "// FIXME: task b\n").unwrap();

        let config = Config::default();
        let config_hash = ScanCache::config_hash(&config);
        let mut cache = ScanCache::new(config_hash);

        // First run
        scan_directory_cached(dir.path(), &config, &mut cache).unwrap();

        // Second run - should be all hits
        let result = scan_directory_cached(dir.path(), &config, &mut cache).unwrap();

        assert_eq!(result.result.items.len(), 2);
        assert_eq!(result.cache_hits, 2);
        assert_eq!(result.cache_misses, 0);
    }

    #[test]
    fn test_cached_scan_modified_file_detected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "// TODO: original\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "// FIXME: unchanged\n").unwrap();

        let config = Config::default();
        let config_hash = ScanCache::config_hash(&config);
        let mut cache = ScanCache::new(config_hash);

        // First run
        scan_directory_cached(dir.path(), &config, &mut cache).unwrap();

        // Modify one file (ensure mtime changes)
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(
            dir.path().join("a.rs"),
            "// TODO: original\n// HACK: new item\n",
        )
        .unwrap();

        // Second run
        let result = scan_directory_cached(dir.path(), &config, &mut cache).unwrap();

        assert_eq!(result.result.items.len(), 3);
        assert_eq!(result.cache_hits, 1); // b.rs hit
        assert_eq!(result.cache_misses, 1); // a.rs miss
    }

    #[test]
    fn test_cached_scan_deleted_file_pruned() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "// TODO: keep\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "// FIXME: remove\n").unwrap();

        let config = Config::default();
        let config_hash = ScanCache::config_hash(&config);
        let mut cache = ScanCache::new(config_hash);

        // First run
        scan_directory_cached(dir.path(), &config, &mut cache).unwrap();
        assert_eq!(cache.entries.len(), 2);

        // Delete one file
        std::fs::remove_file(dir.path().join("b.rs")).unwrap();

        // Second run
        let result = scan_directory_cached(dir.path(), &config, &mut cache).unwrap();

        assert_eq!(result.result.items.len(), 1);
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn test_cached_scan_matches_uncached_results() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "// TODO: task one\n").unwrap();
        std::fs::write(
            dir.path().join("b.rs"),
            "// FIXME(alice): task two\n// BUG: !! urgent\n",
        )
        .unwrap();

        let config = Config::default();

        // Uncached scan
        let uncached = scan_directory(dir.path(), &config).unwrap();

        // Cached scan
        let config_hash = ScanCache::config_hash(&config);
        let mut cache = ScanCache::new(config_hash);
        let cached = scan_directory_cached(dir.path(), &config, &mut cache).unwrap();

        // Sort both results for comparison
        let mut uncached_items = uncached.items;
        let mut cached_items = cached.result.items;

        uncached_items.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        cached_items.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

        assert_eq!(uncached_items.len(), cached_items.len());
        for (u, c) in uncached_items.iter().zip(cached_items.iter()) {
            assert_eq!(u.file, c.file);
            assert_eq!(u.line, c.line);
            assert_eq!(u.tag, c.tag);
            assert_eq!(u.message, c.message);
            assert_eq!(u.author, c.author);
            assert_eq!(u.issue_ref, c.issue_ref);
            assert_eq!(u.priority, c.priority);
        }
    }

    // --- todo-scan:ignore suppression tests ---

    #[test]
    fn test_ignore_inline_suppresses_item() {
        let pattern = default_pattern();
        let content = "// TODO: keep this\n// TODO: suppress this todo-scan:ignore\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].message, "keep this");
        assert_eq!(result.ignored_items.len(), 1);
        assert_eq!(result.ignored_items[0].message, "suppress this");
    }

    #[test]
    fn test_ignore_next_line_suppresses_following_item() {
        let pattern = default_pattern();
        let content = "// todo-scan:ignore-next-line\n// TODO: suppressed by next-line\n// TODO: not suppressed\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].message, "not suppressed");
        assert_eq!(result.ignored_items.len(), 1);
        assert_eq!(result.ignored_items[0].message, "suppressed by next-line");
    }

    #[test]
    fn test_ignore_next_line_only_affects_immediate_next() {
        let pattern = default_pattern();
        let content =
            "// todo-scan:ignore-next-line\n// TODO: suppressed\n// TODO: not suppressed\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].message, "not suppressed");
        assert_eq!(result.ignored_items.len(), 1);
    }

    #[test]
    fn test_ignore_next_line_blank_line_between_does_not_suppress() {
        let pattern = default_pattern();
        let content = "// todo-scan:ignore-next-line\n\n// TODO: should not be suppressed\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].message, "should not be suppressed");
        assert!(result.ignored_items.is_empty());
    }

    #[test]
    fn test_ignore_mixed_items() {
        let pattern = default_pattern();
        let content = "\
// TODO: normal item
// todo-scan:ignore-next-line
// FIXME: suppressed fixme
// HACK: normal hack
// BUG: suppressed bug todo-scan:ignore
";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].message, "normal item");
        assert_eq!(result.items[1].message, "normal hack");

        assert_eq!(result.ignored_items.len(), 2);
        assert_eq!(result.ignored_items[0].message, "suppressed fixme");
        assert_eq!(result.ignored_items[1].message, "suppressed bug");
    }

    #[test]
    fn test_ignore_no_items_affected_when_no_markers() {
        let pattern = default_pattern();
        let content = "// TODO: first\n// FIXME: second\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.items.len(), 2);
        assert!(result.ignored_items.is_empty());
    }

    #[test]
    fn test_ignore_strips_marker_from_message() {
        let pattern = default_pattern();
        let content = "// TODO: fix this todo-scan:ignore\n";
        let result = scan_content(content, "test.rs", &pattern);

        assert_eq!(result.ignored_items.len(), 1);
        assert_eq!(result.ignored_items[0].message, "fix this");
    }

    #[test]
    fn test_ignore_directory_scan_separates_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("test.rs"),
            "// TODO: visible\n// TODO: hidden todo-scan:ignore\n",
        )
        .unwrap();

        let config = Config::default();
        let result = scan_directory(dir.path(), &config).unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].message, "visible");
        assert_eq!(result.ignored_items.len(), 1);
        assert_eq!(result.ignored_items[0].message, "hidden");
    }
}
