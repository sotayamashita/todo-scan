use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use regex::Regex;

use crate::cli::Format;
use crate::config::Config;
use crate::model::{FileUpdate, Tag, TodoItem, WatchEvent};
use crate::output::{print_initial_summary, print_watch_event};
use crate::scanner::{scan_content, scan_directory};

/// In-memory index of TODO items grouped by file path.
pub struct TodoIndex {
    items: HashMap<String, Vec<TodoItem>>,
    pattern: Regex,
    root: PathBuf,
    exclude_dirs: Vec<String>,
    exclude_regexes: Vec<Regex>,
}

impl TodoIndex {
    /// Build a new index by performing a full directory scan.
    pub fn new(root: &Path, config: &Config) -> Result<Self> {
        let pattern = Regex::new(&config.tags_pattern())?;
        let scan = scan_directory(root, config)?;

        let mut items: HashMap<String, Vec<TodoItem>> = HashMap::new();
        for item in scan.items {
            items.entry(item.file.clone()).or_default().push(item);
        }

        let exclude_regexes: Vec<Regex> = config
            .exclude_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Ok(Self {
            items,
            pattern,
            root: root.to_path_buf(),
            exclude_dirs: config.exclude_dirs.clone(),
            exclude_regexes,
        })
    }

    /// Re-scan a single file and return added/removed items.
    pub fn update_file(&mut self, relative_path: &str) -> Result<FileUpdate> {
        let abs_path = self.root.join(relative_path);
        let content = std::fs::read_to_string(&abs_path)
            .with_context(|| format!("failed to read {}", abs_path.display()))?;

        let new_items = scan_content(&content, relative_path, &self.pattern);
        let old_items = self.items.remove(relative_path).unwrap_or_default();

        let old_keys: HashMap<String, &TodoItem> =
            old_items.iter().map(|i| (i.match_key(), i)).collect();
        let new_keys: HashMap<String, &TodoItem> =
            new_items.iter().map(|i| (i.match_key(), i)).collect();

        let added: Vec<TodoItem> = new_items
            .iter()
            .filter(|i| !old_keys.contains_key(&i.match_key()))
            .cloned()
            .collect();
        let removed: Vec<TodoItem> = old_items
            .iter()
            .filter(|i| !new_keys.contains_key(&i.match_key()))
            .cloned()
            .collect();

        if !new_items.is_empty() {
            self.items.insert(relative_path.to_string(), new_items);
        }

        Ok(FileUpdate { added, removed })
    }

    /// Remove a file from the index, returning its former items.
    pub fn remove_file(&mut self, relative_path: &str) -> Vec<TodoItem> {
        self.items.remove(relative_path).unwrap_or_default()
    }

    /// Total TODO count across all files.
    pub fn total_count(&self) -> usize {
        self.items.values().map(|v| v.len()).sum()
    }

    /// Count of items per tag.
    pub fn tag_counts(&self) -> Vec<(Tag, usize)> {
        let mut counts: HashMap<Tag, usize> = HashMap::new();
        for items in self.items.values() {
            for item in items {
                *counts.entry(item.tag).or_insert(0) += 1;
            }
        }
        let mut result: Vec<(Tag, usize)> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Check if a path should be excluded based on config.
    pub fn should_exclude(&self, relative_path: &str) -> bool {
        let path = Path::new(relative_path);

        let excluded_by_dir = self.exclude_dirs.iter().any(|dir| {
            path.components()
                .any(|c| c.as_os_str().to_str().is_some_and(|s| s == dir))
        });
        if excluded_by_dir {
            return true;
        }

        self.exclude_regexes
            .iter()
            .any(|re| re.is_match(relative_path))
    }
}

/// Collect changed file paths from debounced events, converting to relative paths.
fn collect_changed_files(
    events: &[notify_debouncer_mini::DebouncedEvent],
    root: &Path,
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for event in events {
        if event.kind != DebouncedEventKind::Any {
            continue;
        }
        if let Ok(rel) = event.path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy().to_string();
            if seen.insert(rel_str.clone()) {
                result.push(rel_str);
            }
        }
    }

    result
}

/// Build a WatchEvent from a file update.
fn build_watch_event(
    file: &str,
    update: &FileUpdate,
    index: &TodoIndex,
    previous_total: usize,
) -> WatchEvent {
    let total = index.total_count();
    let tag_summary: Vec<(String, usize)> = index
        .tag_counts()
        .into_iter()
        .map(|(tag, count)| (tag.as_str().to_string(), count))
        .collect();

    let now = time::OffsetDateTime::now_utc();
    let timestamp = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    );

    WatchEvent {
        timestamp,
        file: file.to_string(),
        added: update.added.clone(),
        removed: update.removed.clone(),
        tag_summary,
        total,
        total_delta: total as i64 - previous_total as i64,
    }
}

/// Main watch command entry point.
pub fn cmd_watch(
    root: &Path,
    config: &Config,
    format: &Format,
    tag_filter: &[String],
    max: Option<usize>,
    debounce_ms: u64,
) -> Result<()> {
    // Canonicalize root to match paths reported by the OS watcher
    // (e.g., macOS resolves /tmp → /private/tmp)
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    let mut index = TodoIndex::new(&root, config)?;
    let filter_tags: Vec<Tag> = tag_filter
        .iter()
        .filter_map(|s| s.parse::<Tag>().ok())
        .collect();

    print_initial_summary(&index.tag_counts(), index.total_count(), format);

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .context("failed to set Ctrl+C handler")?;

    let (tx, rx) = std::sync::mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(debounce_ms), tx)
        .context("failed to create watcher")?;

    debouncer
        .watcher()
        .watch(&root, notify::RecursiveMode::Recursive)
        .context("failed to watch directory")?;

    eprintln!("Watching for changes... (Ctrl+C to stop)");

    while running.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(events)) => {
                let files = collect_changed_files(&events, &root);
                for file in files {
                    if index.should_exclude(&file) {
                        continue;
                    }

                    let abs_path = root.join(&file);
                    let previous_total = index.total_count();

                    let update = if abs_path.is_file() {
                        match index.update_file(&file) {
                            Ok(u) => u,
                            Err(_) => continue,
                        }
                    } else {
                        let removed = index.remove_file(&file);
                        FileUpdate {
                            added: vec![],
                            removed,
                        }
                    };

                    if update.added.is_empty() && update.removed.is_empty() {
                        continue;
                    }

                    let mut event = build_watch_event(&file, &update, &index, previous_total);

                    // Apply tag filter to displayed items
                    if !filter_tags.is_empty() {
                        event.added.retain(|i| filter_tags.contains(&i.tag));
                        event.removed.retain(|i| filter_tags.contains(&i.tag));
                        if event.added.is_empty() && event.removed.is_empty() {
                            continue;
                        }
                    }

                    print_watch_event(&event, format, max);
                }
            }
            Ok(Err(_)) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    eprintln!("Watching stopped.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;
    use tempfile::TempDir;

    fn setup_index(files: &[(&str, &str)]) -> (TempDir, TodoIndex) {
        let dir = TempDir::new().unwrap();
        for (path, content) in files {
            let full_path = dir.path().join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(full_path, content).unwrap();
        }
        let config = Config::default();
        let index = TodoIndex::new(dir.path(), &config).unwrap();
        (dir, index)
    }

    #[test]
    fn test_index_new_populates_items() {
        let (_dir, index) = setup_index(&[
            ("a.rs", "// TODO: first\n// FIXME: second\n"),
            ("b.rs", "// HACK: third\n"),
        ]);

        assert_eq!(index.total_count(), 3);
    }

    #[test]
    fn test_index_tag_counts() {
        let (_dir, index) = setup_index(&[
            ("a.rs", "// TODO: one\n// TODO: two\n"),
            ("b.rs", "// FIXME: three\n"),
        ]);

        let counts = index.tag_counts();
        let todo_count = counts
            .iter()
            .find(|(t, _)| *t == Tag::Todo)
            .map(|(_, c)| *c);
        let fixme_count = counts
            .iter()
            .find(|(t, _)| *t == Tag::Fixme)
            .map(|(_, c)| *c);

        assert_eq!(todo_count, Some(2));
        assert_eq!(fixme_count, Some(1));
    }

    #[test]
    fn test_index_update_file_detects_added() {
        let (dir, mut index) = setup_index(&[("a.rs", "// TODO: original\n")]);

        assert_eq!(index.total_count(), 1);

        // Add a second TODO
        fs::write(
            dir.path().join("a.rs"),
            "// TODO: original\n// FIXME: new one\n",
        )
        .unwrap();

        let update = index.update_file("a.rs").unwrap();
        assert_eq!(update.added.len(), 1);
        assert_eq!(update.added[0].tag, Tag::Fixme);
        assert!(update.removed.is_empty());
        assert_eq!(index.total_count(), 2);
    }

    #[test]
    fn test_index_update_file_detects_removed() {
        let (dir, mut index) = setup_index(&[("a.rs", "// TODO: one\n// FIXME: two\n")]);

        assert_eq!(index.total_count(), 2);

        // Remove the FIXME
        fs::write(dir.path().join("a.rs"), "// TODO: one\n").unwrap();

        let update = index.update_file("a.rs").unwrap();
        assert!(update.added.is_empty());
        assert_eq!(update.removed.len(), 1);
        assert_eq!(update.removed[0].tag, Tag::Fixme);
        assert_eq!(index.total_count(), 1);
    }

    #[test]
    fn test_index_update_file_unchanged() {
        let (_dir, mut index) = setup_index(&[("a.rs", "// TODO: same\n")]);

        let update = index.update_file("a.rs").unwrap();
        assert!(update.added.is_empty());
        assert!(update.removed.is_empty());
    }

    #[test]
    fn test_index_remove_file() {
        let (_dir, mut index) = setup_index(&[("a.rs", "// TODO: gone\n// FIXME: also gone\n")]);

        assert_eq!(index.total_count(), 2);

        let removed = index.remove_file("a.rs");
        assert_eq!(removed.len(), 2);
        assert_eq!(index.total_count(), 0);
    }

    #[test]
    fn test_index_remove_nonexistent_file() {
        let (_dir, mut index) = setup_index(&[("a.rs", "// TODO: exists\n")]);

        let removed = index.remove_file("nonexistent.rs");
        assert!(removed.is_empty());
        assert_eq!(index.total_count(), 1);
    }

    #[test]
    fn test_should_exclude_dirs() {
        let (_dir, _index) = setup_index(&[]);
        let config = Config {
            exclude_dirs: vec!["node_modules".to_string()],
            ..Config::default()
        };
        let dir = TempDir::new().unwrap();
        let index = TodoIndex::new(dir.path(), &config).unwrap();

        assert!(index.should_exclude("node_modules/foo.js"));
        assert!(!index.should_exclude("src/main.rs"));
    }

    #[test]
    fn test_should_exclude_patterns() {
        let config = Config {
            exclude_patterns: vec![r"\.min\.js$".to_string()],
            ..Config::default()
        };
        let dir = TempDir::new().unwrap();
        let index = TodoIndex::new(dir.path(), &config).unwrap();

        assert!(index.should_exclude("dist/bundle.min.js"));
        assert!(!index.should_exclude("src/app.js"));
    }

    #[test]
    fn test_collect_changed_files_dedup() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.rs");

        let events = vec![
            notify_debouncer_mini::DebouncedEvent {
                path: path.clone(),
                kind: DebouncedEventKind::Any,
            },
            notify_debouncer_mini::DebouncedEvent {
                path: path.clone(),
                kind: DebouncedEventKind::Any,
            },
        ];

        let files = collect_changed_files(&events, dir.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], "test.rs");
    }

    #[test]
    fn test_build_watch_event_delta() {
        let (dir, mut index) = setup_index(&[("a.rs", "// TODO: one\n")]);

        let previous_total = index.total_count();

        fs::write(
            dir.path().join("a.rs"),
            "// TODO: one\n// TODO: two\n// TODO: three\n",
        )
        .unwrap();

        let update = index.update_file("a.rs").unwrap();
        let event = build_watch_event("a.rs", &update, &index, previous_total);

        assert_eq!(event.total, 3);
        assert_eq!(event.total_delta, 2);
        assert_eq!(event.added.len(), 2);
        assert!(event.removed.is_empty());
    }
}
