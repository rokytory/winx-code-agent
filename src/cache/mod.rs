use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::fs;

/// Caches file content and metadata to reduce disk access
pub struct FileCache {
    content_cache: HashMap<PathBuf, (String, Instant)>,
    metadata_cache: HashMap<PathBuf, (fs::Metadata, Instant)>,
    cache_duration: Duration,
}

impl FileCache {
    /// Create a new FileCache with specified cache duration in seconds
    pub fn new(cache_duration_secs: u64) -> Self {
        Self {
            content_cache: HashMap::new(),
            metadata_cache: HashMap::new(),
            cache_duration: Duration::from_secs(cache_duration_secs),
        }
    }
    
    /// Get cached file content if available and not expired
    pub fn get_content(&self, path: &Path) -> Option<&str> {
        if let Some((content, time)) = self.content_cache.get(path) {
            if time.elapsed() < self.cache_duration {
                return Some(content);
            }
        }
        None
    }
    
    /// Store file content in cache
    pub fn store_content(&mut self, path: PathBuf, content: String) {
        self.content_cache.insert(path, (content, Instant::now()));
    }
    
    /// Get cached file metadata if available and not expired
    pub fn get_metadata(&self, path: &Path) -> Option<&fs::Metadata> {
        if let Some((metadata, time)) = self.metadata_cache.get(path) {
            if time.elapsed() < self.cache_duration {
                return Some(metadata);
            }
        }
        None
    }
    
    /// Store file metadata in cache
    pub fn store_metadata(&mut self, path: PathBuf, metadata: fs::Metadata) {
        self.metadata_cache.insert(path, (metadata, Instant::now()));
    }
    
    /// Remove an entry from cache
    pub fn invalidate(&mut self, path: &Path) {
        self.content_cache.remove(path);
        self.metadata_cache.remove(path);
    }
    
    /// Clear all cached items
    pub fn clear(&mut self) {
        self.content_cache.clear();
        self.metadata_cache.clear();
    }
    
    /// Read file content, using cache if available
    pub fn read_file(&mut self, path: &Path) -> std::io::Result<String> {
        // Check cache first
        if let Some(content) = self.get_content(path) {
            return Ok(content.to_string());
        }
        
        // Read from disk if not in cache
        let content = fs::read_to_string(path)?;
        self.store_content(path.to_path_buf(), content.clone());
        Ok(content)
    }
    
    /// Get file metadata, using cache if available
    pub fn get_file_metadata(&mut self, path: &Path) -> std::io::Result<fs::Metadata> {
        // Check cache first
        if let Some(metadata) = self.get_metadata(path) {
            return Ok(metadata.to_owned());
        }
        
        // Read from disk if not in cache
        let metadata = fs::metadata(path)?;
        self.store_metadata(path.to_path_buf(), metadata.clone());
        Ok(metadata)
    }
}

// Global file cache instance
lazy_static::lazy_static! {
    static ref GLOBAL_FILE_CACHE: Arc<Mutex<FileCache>> = 
        Arc::new(Mutex::new(FileCache::new(30))); // 30 second cache duration
}

/// Get access to the global file cache
pub fn get_file_cache() -> Arc<Mutex<FileCache>> {
    GLOBAL_FILE_CACHE.clone()
}

/// Read a file using the global cache
pub fn cached_read_file(path: &Path) -> std::io::Result<String> {
    let mut cache = match GLOBAL_FILE_CACHE.lock() {
        Ok(cache) => cache,
        Err(_) => return fs::read_to_string(path), // Fallback to direct read if lock fails
    };
    
    cache.read_file(path)
}

/// Get file metadata using the global cache
pub fn cached_metadata(path: &Path) -> std::io::Result<fs::Metadata> {
    let mut cache = match GLOBAL_FILE_CACHE.lock() {
        Ok(cache) => cache,
        Err(_) => return fs::metadata(path), // Fallback to direct read if lock fails
    };
    
    cache.get_file_metadata(path)
}

/// Invalidate a cached file
pub fn invalidate_cached_file(path: &Path) {
    if let Ok(mut cache) = GLOBAL_FILE_CACHE.lock() {
        cache.invalidate(path);
    }
}
