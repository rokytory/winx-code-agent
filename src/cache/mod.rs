use lru::LruCache;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::{WinxError, WinxResult};

#[derive(Clone)]
pub struct FileCache {
    content_cache: HashMap<PathBuf, (String, Instant)>,
    metadata_cache: HashMap<PathBuf, (fs::Metadata, Instant)>,
    cache_duration: Duration,
}

impl FileCache {
    pub fn new(cache_duration_secs: u64) -> Self {
        Self {
            content_cache: HashMap::new(),
            metadata_cache: HashMap::new(),
            cache_duration: Duration::from_secs(cache_duration_secs),
        }
    }

    pub fn get_content(&self, path: &Path) -> Option<&str> {
        if let Some((content, time)) = self.content_cache.get(path) {
            if time.elapsed() < self.cache_duration {
                return Some(content);
            }
        }
        None
    }

    pub fn store_content(&mut self, path: PathBuf, content: String) {
        self.content_cache.insert(path, (content, Instant::now()));
    }

    pub fn get_metadata(&self, path: &Path) -> Option<&fs::Metadata> {
        if let Some((metadata, time)) = self.metadata_cache.get(path) {
            if time.elapsed() < self.cache_duration {
                return Some(metadata);
            }
        }
        None
    }

    pub fn store_metadata(&mut self, path: PathBuf, metadata: fs::Metadata) {
        self.metadata_cache.insert(path, (metadata, Instant::now()));
    }

    pub fn invalidate(&mut self, path: &Path) {
        self.content_cache.remove(path);
        self.metadata_cache.remove(path);
    }

    pub fn clear(&mut self) {
        self.content_cache.clear();
        self.metadata_cache.clear();
    }

    pub fn read_file(&mut self, path: &Path) -> std::io::Result<String> {
        if let Some(content) = self.get_content(path) {
            return Ok(content.to_string());
        }

        let content = fs::read_to_string(path)?;
        self.store_content(path.to_path_buf(), content.clone());
        Ok(content)
    }

    pub fn get_file_metadata(&mut self, path: &Path) -> std::io::Result<fs::Metadata> {
        if let Some(metadata) = self.get_metadata(path) {
            return Ok(metadata.to_owned());
        }

        let metadata = fs::metadata(path)?;
        self.store_metadata(path.to_path_buf(), metadata.clone());
        Ok(metadata)
    }
}

#[derive(Clone)]
pub struct CacheEntry {
    pub content: String,
    pub metadata: fs::Metadata,
    pub timestamp: Instant,
    pub hash: String,
}

pub struct AdvancedCache {
    file_cache: Arc<Mutex<LruCache<PathBuf, CacheEntry>>>,
    plugin_cache: Arc<Mutex<LruCache<String, Vec<u8>>>>,
    query_cache: Arc<Mutex<LruCache<String, QueryResult>>>,
}

#[derive(Clone)]
pub struct QueryResult {
    pub data: serde_json::Value,
    pub timestamp: Instant,
}

impl AdvancedCache {
    pub fn new(
        max_file_entries: usize,
        max_plugin_entries: usize,
        max_query_entries: usize,
    ) -> Self {
        Self {
            file_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(max_file_entries)
                    .unwrap_or_else(|| NonZeroUsize::new(1000).unwrap()),
            ))),
            plugin_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(max_plugin_entries)
                    .unwrap_or_else(|| NonZeroUsize::new(100).unwrap()),
            ))),
            query_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(max_query_entries)
                    .unwrap_or_else(|| NonZeroUsize::new(500).unwrap()),
            ))),
        }
    }

    pub fn get_or_compute<F, T>(&self, key: &str, compute: F) -> WinxResult<T>
    where
        F: FnOnce() -> WinxResult<T>,
        T: Clone + serde::Serialize + serde::de::DeserializeOwned,
    {
        let mut cache = self
            .query_cache
            .lock()
            .map_err(|e| WinxError::lock_error(format!("Failed to acquire cache lock: {}", e)))?;

        // Check if the result is in cache and not expired
        if let Some(result) = cache.get(key) {
            // Cache hit - check if expired (e.g., 5 minutes TTL)
            if result.timestamp.elapsed() < Duration::from_secs(300) {
                // Deserialize and return
                return serde_json::from_value(result.data.clone()).map_err(|e| {
                    WinxError::parse_error(format!("Cache deserialization error: {}", e))
                });
            }
        }

        // Cache miss or expired - compute the result
        let computed = compute()?;

        // Serialize and store in cache
        let serialized = serde_json::to_value(&computed)
            .map_err(|e| WinxError::parse_error(format!("Cache serialization error: {}", e)))?;

        cache.put(
            key.to_string(),
            QueryResult {
                data: serialized,
                timestamp: Instant::now(),
            },
        );

        Ok(computed)
    }

    pub fn get_file(&self, path: &Path) -> Option<CacheEntry> {
        let cache = self.file_cache.lock().ok()?;
        cache.peek(path).cloned()
    }

    pub fn store_file(
        &self,
        path: PathBuf,
        content: String,
        metadata: fs::Metadata,
    ) -> WinxResult<()> {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let entry = CacheEntry {
            content,
            metadata,
            timestamp: Instant::now(),
            hash,
        };

        let mut cache = self
            .file_cache
            .lock()
            .map_err(|e| WinxError::lock_error(format!("Failed to acquire cache lock: {}", e)))?;
        cache.put(path, entry);
        Ok(())
    }

    pub fn invalidate_file(&self, path: &Path) -> WinxResult<()> {
        let mut cache = self
            .file_cache
            .lock()
            .map_err(|e| WinxError::lock_error(format!("Failed to acquire cache lock: {}", e)))?;
        cache.pop(path);
        Ok(())
    }

    pub fn get_plugin(&self, key: &str) -> Option<Vec<u8>> {
        let cache = self.plugin_cache.lock().ok()?;
        cache.peek(key).cloned()
    }

    pub fn store_plugin(&self, key: String, data: Vec<u8>) -> WinxResult<()> {
        let mut cache = self
            .plugin_cache
            .lock()
            .map_err(|e| WinxError::lock_error(format!("Failed to acquire cache lock: {}", e)))?;
        cache.put(key, data);
        Ok(())
    }

    pub fn clear_all(&self) -> WinxResult<()> {
        let mut file_cache = self.file_cache.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire file cache lock: {}", e))
        })?;
        let mut plugin_cache = self.plugin_cache.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire plugin cache lock: {}", e))
        })?;
        let mut query_cache = self.query_cache.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire query cache lock: {}", e))
        })?;

        file_cache.clear();
        plugin_cache.clear();
        query_cache.clear();
        Ok(())
    }
}

lazy_static::lazy_static! {
    static ref GLOBAL_FILE_CACHE: Arc<Mutex<FileCache>> =
        Arc::new(Mutex::new(FileCache::new(30))); // 30 second cache duration

    static ref GLOBAL_ADVANCED_CACHE: Arc<AdvancedCache> =
        Arc::new(AdvancedCache::new(1000, 100, 500));
}

pub fn get_file_cache() -> Arc<Mutex<FileCache>> {
    GLOBAL_FILE_CACHE.clone()
}

pub fn get_advanced_cache() -> Arc<AdvancedCache> {
    GLOBAL_ADVANCED_CACHE.clone()
}

pub fn cached_read_file(path: &Path) -> std::io::Result<String> {
    let mut cache = match GLOBAL_FILE_CACHE.lock() {
        Ok(cache) => cache,
        Err(_) => return fs::read_to_string(path), // Fallback to direct read if lock fails
    };
    cache.read_file(path)
}

pub fn cached_metadata(path: &Path) -> std::io::Result<fs::Metadata> {
    let mut cache = match GLOBAL_FILE_CACHE.lock() {
        Ok(cache) => cache,
        Err(_) => return fs::metadata(path), // Fallback to direct read if lock fails
    };
    cache.get_file_metadata(path)
}

pub fn invalidate_cached_file(path: &Path) {
    if let Ok(mut cache) = GLOBAL_FILE_CACHE.lock() {
        cache.invalidate(path);
    }
    if let Ok(()) = GLOBAL_ADVANCED_CACHE.invalidate_file(path) {
        // File invalidated in advanced cache
    }
}
