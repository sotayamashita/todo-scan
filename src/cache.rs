use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::model::TodoItem;

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheEntry {
    pub content_hash: [u8; 32],
    pub items: Vec<TodoItem>,
    #[serde(default)]
    pub ignored_items: Vec<TodoItem>,
    pub mtime_secs: u64,
    pub mtime_nanos: u32,
}

/// Result returned by cache check methods, containing both items and ignored items.
pub struct CacheCheckResult<'a> {
    pub items: &'a [TodoItem],
    pub ignored_items: &'a [TodoItem],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanCache {
    pub config_hash: [u8; 32],
    pub entries: HashMap<PathBuf, CacheEntry>,
}

impl ScanCache {
    /// Create a new empty cache with the given config hash.
    pub fn new(config_hash: [u8; 32]) -> Self {
        Self {
            config_hash,
            entries: HashMap::new(),
        }
    }

    /// Load cache from disk. Returns None if missing or corrupt.
    pub fn load(repo_root: &Path) -> Option<Self> {
        let path = cache_path(repo_root)?;
        let data = fs::read(&path).ok()?;
        bincode::deserialize(&data).ok()
    }

    /// Save cache to disk with atomic write (write tmp, then rename).
    pub fn save(&self, repo_root: &Path) -> Result<()> {
        let path = match cache_path(repo_root) {
            Some(p) => p,
            None => anyhow::bail!("cannot determine cache directory"),
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = path.with_extension("tmp");
        let data = bincode::serialize(self)?;
        fs::write(&tmp_path, &data)?;
        fs::rename(&tmp_path, &path)?;
        Ok(())
    }

    /// Compute a deterministic hash of the config fields that affect scanning.
    pub fn config_hash(config: &Config) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        for tag in &config.tags {
            hasher.update(tag.as_bytes());
            hasher.update(b"\0");
        }
        hasher.update(b"\x01");
        for dir in &config.exclude_dirs {
            hasher.update(dir.as_bytes());
            hasher.update(b"\0");
        }
        hasher.update(b"\x01");
        for pat in &config.exclude_patterns {
            hasher.update(pat.as_bytes());
            hasher.update(b"\0");
        }
        *hasher.finalize().as_bytes()
    }

    /// Check if we have a cached entry for this path with matching mtime.
    /// Returns the cached items and ignored items if mtime matches (layer 1 hit).
    pub fn check(&self, path: &Path, mtime: SystemTime) -> Option<CacheCheckResult<'_>> {
        let entry = self.entries.get(path)?;
        let (secs, nanos) = system_time_to_parts(mtime);
        if entry.mtime_secs == secs && entry.mtime_nanos == nanos {
            Some(CacheCheckResult {
                items: &entry.items,
                ignored_items: &entry.ignored_items,
            })
        } else {
            None
        }
    }

    /// Check if we have a cached entry for this path with matching content hash.
    /// Returns the cached items and ignored items if hash matches (layer 2 hit).
    pub fn check_with_content(&self, path: &Path, content: &[u8]) -> Option<CacheCheckResult<'_>> {
        let entry = self.entries.get(path)?;
        let hash = blake3::hash(content);
        if entry.content_hash == *hash.as_bytes() {
            Some(CacheCheckResult {
                items: &entry.items,
                ignored_items: &entry.ignored_items,
            })
        } else {
            None
        }
    }

    /// Insert or update a cache entry.
    pub fn insert(
        &mut self,
        path: PathBuf,
        content_hash: [u8; 32],
        items: Vec<TodoItem>,
        ignored_items: Vec<TodoItem>,
        mtime: SystemTime,
    ) {
        let (secs, nanos) = system_time_to_parts(mtime);
        self.entries.insert(
            path,
            CacheEntry {
                content_hash,
                items,
                ignored_items,
                mtime_secs: secs,
                mtime_nanos: nanos,
            },
        );
    }

    /// Remove entries for files that no longer exist.
    pub fn prune(&mut self, existing_paths: &std::collections::HashSet<PathBuf>) {
        self.entries.retain(|path, _| existing_paths.contains(path));
    }
}

/// Convert SystemTime to (secs, nanos) since UNIX_EPOCH.
fn system_time_to_parts(time: SystemTime) -> (u64, u32) {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => (d.as_secs(), d.subsec_nanos()),
        Err(_) => (0, 0),
    }
}

/// Compute the cache file path for a given repo root.
/// Returns `~/.cache/todox/<repo-hash>/scan-cache.bin` (or platform equivalent).
fn cache_path(repo_root: &Path) -> Option<PathBuf> {
    let cache_dir = dirs::cache_dir()?;
    let repo_hash = blake3::hash(repo_root.to_string_lossy().as_bytes());
    let hex = format!("{}", repo_hash.to_hex());
    Some(
        cache_dir
            .join("todox")
            .join(&hex[..16])
            .join("scan-cache.bin"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deadline::Deadline;
    use crate::model::{Priority, Tag};
    use crate::test_helpers::helpers::make_item;

    fn make_item_with_deadline(file: &str, msg: &str) -> TodoItem {
        TodoItem {
            file: file.to_string(),
            line: 1,
            tag: Tag::Todo,
            message: msg.to_string(),
            author: Some("alice".to_string()),
            issue_ref: Some("#42".to_string()),
            priority: Priority::High,
            deadline: Some(Deadline {
                year: 2025,
                month: 6,
                day: 1,
            }),
        }
    }

    #[test]
    fn test_config_hash_deterministic() {
        let config = Config::default();
        let h1 = ScanCache::config_hash(&config);
        let h2 = ScanCache::config_hash(&config);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_config_hash_changes_with_tags() {
        let config1 = Config::default();
        let mut config2 = Config::default();
        config2.tags.push("CUSTOM".to_string());
        assert_ne!(
            ScanCache::config_hash(&config1),
            ScanCache::config_hash(&config2)
        );
    }

    #[test]
    fn test_config_hash_changes_with_exclude_dirs() {
        let config1 = Config::default();
        let mut config2 = Config::default();
        config2.exclude_dirs.push("vendor".to_string());
        assert_ne!(
            ScanCache::config_hash(&config1),
            ScanCache::config_hash(&config2)
        );
    }

    #[test]
    fn test_config_hash_changes_with_exclude_patterns() {
        let config1 = Config::default();
        let mut config2 = Config::default();
        config2.exclude_patterns.push(r"\.min\.js$".to_string());
        assert_ne!(
            ScanCache::config_hash(&config1),
            ScanCache::config_hash(&config2)
        );
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path();

        let config = Config::default();
        let config_hash = ScanCache::config_hash(&config);
        let mut cache = ScanCache::new(config_hash);

        let mtime = SystemTime::UNIX_EPOCH + std::time::Duration::new(1700000000, 123456789);
        let hash = blake3::hash(b"test content");
        cache.insert(
            PathBuf::from("src/main.rs"),
            *hash.as_bytes(),
            vec![make_item("src/main.rs", 1, Tag::Todo, "test task")],
            vec![],
            mtime,
        );

        cache.save(repo_root).unwrap();
        let loaded = ScanCache::load(repo_root).unwrap();

        assert_eq!(loaded.config_hash, config_hash);
        assert_eq!(loaded.entries.len(), 1);
        let entry = loaded.entries.get(Path::new("src/main.rs")).unwrap();
        assert_eq!(entry.items.len(), 1);
        assert_eq!(entry.items[0].message, "test task");
        assert_eq!(entry.content_hash, *hash.as_bytes());
    }

    #[test]
    fn test_save_load_roundtrip_with_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path();

        let config_hash = ScanCache::config_hash(&Config::default());
        let mut cache = ScanCache::new(config_hash);

        let mtime = SystemTime::UNIX_EPOCH + std::time::Duration::new(1700000000, 0);
        let hash = blake3::hash(b"content");
        cache.insert(
            PathBuf::from("src/lib.rs"),
            *hash.as_bytes(),
            vec![make_item_with_deadline("src/lib.rs", "deadline task")],
            vec![],
            mtime,
        );

        cache.save(repo_root).unwrap();
        let loaded = ScanCache::load(repo_root).unwrap();

        let entry = loaded.entries.get(Path::new("src/lib.rs")).unwrap();
        assert_eq!(entry.items[0].author.as_deref(), Some("alice"));
        assert_eq!(entry.items[0].priority, Priority::High);
        let d = entry.items[0].deadline.unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 6);
        assert_eq!(d.day, 1);
    }

    #[test]
    fn test_load_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ScanCache::load(dir.path()).is_none());
    }

    #[test]
    fn test_load_corrupt_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path();

        // Save a valid cache first to find the path
        let config_hash = ScanCache::config_hash(&Config::default());
        let cache = ScanCache::new(config_hash);
        cache.save(repo_root).unwrap();

        // Now corrupt it
        let path = cache_path(repo_root).unwrap();
        fs::write(&path, b"not valid bincode data").unwrap();

        assert!(ScanCache::load(repo_root).is_none());
    }

    #[test]
    fn test_mtime_hit_returns_cached_items() {
        let config_hash = ScanCache::config_hash(&Config::default());
        let mut cache = ScanCache::new(config_hash);

        let mtime = SystemTime::UNIX_EPOCH + std::time::Duration::new(1700000000, 500);
        let hash = blake3::hash(b"content");
        let path = PathBuf::from("test.rs");
        cache.insert(
            path.clone(),
            *hash.as_bytes(),
            vec![make_item("test.rs", 1, Tag::Todo, "cached")],
            vec![],
            mtime,
        );

        let result = cache.check(&path, mtime);
        assert!(result.is_some());
        let cached = result.unwrap();
        assert_eq!(cached.items.len(), 1);
        assert_eq!(cached.items[0].message, "cached");
    }

    #[test]
    fn test_mtime_miss_returns_none() {
        let config_hash = ScanCache::config_hash(&Config::default());
        let mut cache = ScanCache::new(config_hash);

        let mtime = SystemTime::UNIX_EPOCH + std::time::Duration::new(1700000000, 0);
        let different_mtime = SystemTime::UNIX_EPOCH + std::time::Duration::new(1700000001, 0);
        let hash = blake3::hash(b"content");
        let path = PathBuf::from("test.rs");
        cache.insert(
            path.clone(),
            *hash.as_bytes(),
            vec![make_item("test.rs", 1, Tag::Todo, "cached")],
            vec![],
            mtime,
        );

        assert!(cache.check(&path, different_mtime).is_none());
    }

    #[test]
    fn test_content_hash_hit_returns_cached_items() {
        let config_hash = ScanCache::config_hash(&Config::default());
        let mut cache = ScanCache::new(config_hash);

        let content = b"// TODO: test content";
        let hash = blake3::hash(content);
        let path = PathBuf::from("test.rs");
        let mtime = SystemTime::UNIX_EPOCH;
        cache.insert(
            path.clone(),
            *hash.as_bytes(),
            vec![make_item("test.rs", 1, Tag::Todo, "test content")],
            vec![],
            mtime,
        );

        let result = cache.check_with_content(&path, content);
        assert!(result.is_some());
        assert_eq!(result.unwrap().items[0].message, "test content");
    }

    #[test]
    fn test_content_hash_miss_returns_none() {
        let config_hash = ScanCache::config_hash(&Config::default());
        let mut cache = ScanCache::new(config_hash);

        let content = b"// TODO: original";
        let hash = blake3::hash(content);
        let path = PathBuf::from("test.rs");
        let mtime = SystemTime::UNIX_EPOCH;
        cache.insert(
            path.clone(),
            *hash.as_bytes(),
            vec![make_item("test.rs", 1, Tag::Todo, "original")],
            vec![],
            mtime,
        );

        let different_content = b"// TODO: modified";
        assert!(cache.check_with_content(&path, different_content).is_none());
    }

    #[test]
    fn test_prune_removes_deleted_files() {
        let config_hash = ScanCache::config_hash(&Config::default());
        let mut cache = ScanCache::new(config_hash);

        let mtime = SystemTime::UNIX_EPOCH;
        let hash = blake3::hash(b"content");

        cache.insert(
            PathBuf::from("keep.rs"),
            *hash.as_bytes(),
            vec![make_item("keep.rs", 1, Tag::Todo, "keep")],
            vec![],
            mtime,
        );
        cache.insert(
            PathBuf::from("delete.rs"),
            *hash.as_bytes(),
            vec![make_item("delete.rs", 1, Tag::Todo, "delete")],
            vec![],
            mtime,
        );

        let mut existing = std::collections::HashSet::new();
        existing.insert(PathBuf::from("keep.rs"));
        cache.prune(&existing);

        assert_eq!(cache.entries.len(), 1);
        assert!(cache.entries.contains_key(Path::new("keep.rs")));
        assert!(!cache.entries.contains_key(Path::new("delete.rs")));
    }
}
