use anyhow::Result;
use ignore::{WalkBuilder, WalkState};
use regex::Regex;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use crate::config::Config;
use crate::model::{Priority, ScanResult, Tag, TodoItem};

static ISSUE_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:([A-Z]+-\d+)|#(\d+))").unwrap());

/// Extract an issue reference from the message text.
/// Matches patterns like #123 or JIRA-456.
fn extract_issue_ref(message: &str) -> Option<String> {
    ISSUE_REF_RE.captures(message).map(|caps| {
        caps.get(1)
            .or_else(|| caps.get(2).map(|m| m))
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

/// Heuristic: does the tag at `tag_start` appear to be inside a comment?
fn is_in_comment(line: &str, tag_start: usize) -> bool {
    let before_tag = &line[..tag_start];
    if COMMENT_PREFIXES.iter().any(|p| before_tag.contains(p)) {
        return true;
    }
    let trimmed = before_tag.trim_start();
    LINE_START_PREFIXES.iter().any(|p| trimmed.starts_with(p))
}

/// Scan text content line by line for TODO-style comments.
///
/// Pure function: takes content, a file path label, and a compiled regex.
/// Returns a `Vec<TodoItem>` with all matches found.
pub fn scan_content(content: &str, file_path: &str, pattern: &Regex) -> Vec<TodoItem> {
    let mut items = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        if let Some(caps) = pattern.captures(line) {
            let tag_match = caps.get(1).unwrap();
            if !is_in_comment(line, tag_match.start()) {
                continue;
            }

            let tag_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let tag = match tag_str.parse::<Tag>() {
                Ok(t) => t,
                Err(_) => continue,
            };

            let author = caps.get(2).map(|m| m.as_str().to_string());

            let priority = match caps.get(3).map(|m| m.as_str()) {
                Some("!!") => Priority::Urgent,
                Some("!") => Priority::High,
                _ => Priority::Normal,
            };

            let message = caps
                .get(4)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            let issue_ref = extract_issue_ref(&message);

            items.push(TodoItem {
                file: file_path.to_string(),
                line: line_idx + 1,
                tag,
                message,
                author,
                issue_ref,
                priority,
            });
        }
    }

    items
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
    let files_scanned = Arc::new(AtomicUsize::new(0));
    let exclude_dirs = Arc::new(config.exclude_dirs.clone());
    let exclude_regexes = Arc::new(exclude_regexes);
    let root = root.to_path_buf();

    let walker = WalkBuilder::new(&root).build_parallel();

    walker.run(|| {
        let items = Arc::clone(&items);
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

            let found = scan_content(&content, &relative_path, &pattern);
            if !found.is_empty() {
                items.lock().unwrap().extend(found);
            }
            files_scanned.fetch_add(1, Ordering::Relaxed);

            WalkState::Continue
        })
    });

    let items = Arc::try_unwrap(items)
        .expect("all walker threads should have finished")
        .into_inner()
        .unwrap();
    let files_scanned = files_scanned.load(Ordering::Relaxed);

    Ok(ScanResult {
        items,
        files_scanned,
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
        let items = scan_content(content, "test.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tag, Tag::Todo);
        assert_eq!(items[0].message, "implement this feature");
        assert_eq!(items[0].file, "test.rs");
        assert_eq!(items[0].line, 1);
        assert_eq!(items[0].priority, Priority::Normal);
        assert!(items[0].author.is_none());
    }

    #[test]
    fn test_fixme_with_author() {
        let pattern = default_pattern();
        let content = "// FIXME(alice): broken parsing logic\n";
        let items = scan_content(content, "lib.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tag, Tag::Fixme);
        assert_eq!(items[0].author.as_deref(), Some("alice"));
        assert_eq!(items[0].message, "broken parsing logic");
    }

    #[test]
    fn test_priority_high() {
        let pattern = default_pattern();
        let content = "# TODO: ! fix memory leak\n";
        let items = scan_content(content, "main.py", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].priority, Priority::High);
    }

    #[test]
    fn test_priority_urgent() {
        let pattern = default_pattern();
        let content = "// BUG: !! crashes on empty input\n";
        let items = scan_content(content, "app.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tag, Tag::Bug);
        assert_eq!(items[0].priority, Priority::Urgent);
    }

    #[test]
    fn test_issue_ref_hash() {
        let pattern = default_pattern();
        let content = "// TODO: fix layout issue #123\n";
        let items = scan_content(content, "ui.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].issue_ref.as_deref(), Some("#123"));
    }

    #[test]
    fn test_issue_ref_jira() {
        let pattern = default_pattern();
        let content = "// FIXME: address JIRA-456 regression\n";
        let items = scan_content(content, "api.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].issue_ref.as_deref(), Some("JIRA-456"));
    }

    #[test]
    fn test_case_insensitivity() {
        let pattern = default_pattern();
        let content = "// todo: lowercase tag\n// Todo: mixed case\n// TODO: uppercase\n";
        let items = scan_content(content, "test.rs", &pattern);

        assert_eq!(items.len(), 3);
        for item in &items {
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
        let items = scan_content(content, "multi.rs", &pattern);

        assert_eq!(items.len(), 4);
        assert_eq!(items[0].tag, Tag::Todo);
        assert_eq!(items[1].tag, Tag::Fixme);
        assert_eq!(items[1].author.as_deref(), Some("bob"));
        assert_eq!(items[2].tag, Tag::Hack);
        assert_eq!(items[3].tag, Tag::Note);
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
        let items = scan_content(content, "lines.rs", &pattern);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].line, 2);
        assert_eq!(items[1].line, 5);
    }

    #[test]
    fn test_xxx_tag() {
        let pattern = default_pattern();
        let content = "// XXX: dangerous code path\n";
        let items = scan_content(content, "danger.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tag, Tag::Xxx);
    }

    #[test]
    fn test_no_match_on_plain_text() {
        let pattern = default_pattern();
        let content = "This is just a regular comment with no tags.\n";
        let items = scan_content(content, "plain.rs", &pattern);

        assert!(items.is_empty());
    }

    #[test]
    fn test_author_with_special_chars() {
        let pattern = default_pattern();
        let content = "// TODO(user@domain.com): email-style author\n";
        let items = scan_content(content, "test.rs", &pattern);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].author.as_deref(), Some("user@domain.com"));
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
        let items = scan_content(content, "test.rs", &pattern);
        assert!(items.is_empty(), "should not match TODO inside identifier");
    }

    #[test]
    fn test_no_match_in_camel_case() {
        let pattern = default_pattern();
        let content = "if isTodoCompleted() { return; }\n";
        let items = scan_content(content, "test.rs", &pattern);
        assert!(items.is_empty(), "should not match Todo in camelCase");
    }

    #[test]
    fn test_no_match_in_string_literal() {
        let pattern = default_pattern();
        let content = "let msg = \"TODO: not a real comment\";\n";
        let items = scan_content(content, "test.rs", &pattern);
        assert!(
            items.is_empty(),
            "should not match TODO inside string literal"
        );
    }

    #[test]
    fn test_no_match_in_plain_code() {
        let pattern = default_pattern();
        let content = "let todo_count = get_todos().len();\n";
        let items = scan_content(content, "test.rs", &pattern);
        assert!(items.is_empty(), "should not match todo in variable name");
    }

    #[test]
    fn test_no_match_enum_variant() {
        let pattern = default_pattern();
        let content = "enum State { Todo, InProgress, Done }\n";
        let items = scan_content(content, "test.rs", &pattern);
        assert!(items.is_empty(), "should not match Todo enum variant");
    }

    #[test]
    fn test_no_match_struct_name() {
        let pattern = default_pattern();
        let content = "struct TodoItem { title: String }\n";
        let items = scan_content(content, "test.rs", &pattern);
        assert!(items.is_empty(), "should not match Todo in struct name");
    }

    // --- Comment detection tests (various languages) ---

    #[test]
    fn test_comment_double_slash() {
        let pattern = default_pattern();
        let content = "// TODO: rust/js/c++ style comment\n";
        let items = scan_content(content, "test.rs", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_comment_hash() {
        let pattern = default_pattern();
        let content = "# TODO: python/ruby/shell style comment\n";
        let items = scan_content(content, "test.py", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_comment_block_start() {
        let pattern = default_pattern();
        let content = "/* TODO: c-style block comment */\n";
        let items = scan_content(content, "test.c", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_comment_block_middle_star() {
        let pattern = default_pattern();
        let content = " * TODO: middle of block comment\n";
        let items = scan_content(content, "test.java", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_comment_double_dash() {
        let pattern = default_pattern();
        let content = "-- TODO: sql/haskell style comment\n";
        let items = scan_content(content, "test.sql", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_comment_percent() {
        let pattern = default_pattern();
        let content = "% TODO: latex/erlang style comment\n";
        let items = scan_content(content, "test.erl", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_comment_html() {
        let pattern = default_pattern();
        let content = "<!-- TODO: html comment -->\n";
        let items = scan_content(content, "test.html", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_comment_semicolon() {
        let pattern = default_pattern();
        let content = "; TODO: lisp/asm style comment\n";
        let items = scan_content(content, "test.lisp", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_comment_ocaml_paren_star() {
        let pattern = default_pattern();
        let content = "(* TODO: ocaml/pascal style comment *)\n";
        let items = scan_content(content, "test.ml", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_comment_haskell_brace_dash() {
        let pattern = default_pattern();
        let content = "{- TODO: haskell block comment -}\n";
        let items = scan_content(content, "test.hs", &pattern);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_indented_comment() {
        let pattern = default_pattern();
        let content = "    // TODO: indented with spaces\n\t# FIXME: indented with tab\n";
        let items = scan_content(content, "test.rs", &pattern);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_inline_comment() {
        let pattern = default_pattern();
        let content = "let x = 42; // TODO: fix this value\n";
        let items = scan_content(content, "test.rs", &pattern);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].message, "fix this value");
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
}
