use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Memory entry for storing contextual information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Name/identifier of the memory
    pub name: String,
    /// The content of the memory
    pub content: String,
    /// When the memory was created or last updated
    pub timestamp: DateTime<Utc>,
    /// Optional tags for categorization
    pub tags: Vec<String>,
}

impl Memory {
    /// Creates a new memory entry
    pub fn new(name: &str, content: &str) -> Self {
        Self {
            name: name.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
            tags: Vec::new(),
        }
    }

    /// Creates a new memory with tags
    pub fn with_tags(name: &str, content: &str, tags: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
            tags,
        }
    }
}

/// Memory store for managing agent memories
#[derive(Debug)]
pub struct MemoryStore {
    /// In-memory store of memories
    memories: HashMap<String, Memory>,
    /// Directory where memories are persisted
    storage_dir: PathBuf,
}

impl MemoryStore {
    /// Creates a new memory store with the specified storage directory
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        // Create the directory if it doesn't exist
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir).context("Failed to create memory storage directory")?;
        }

        let mut store = Self {
            memories: HashMap::new(),
            storage_dir,
        };

        // Load existing memories
        store.load_memories()?;

        Ok(store)
    }

    /// Loads all memories from the storage directory
    fn load_memories(&mut self) -> Result<()> {
        debug!("Loading memories from {}", self.storage_dir.display());
        
        // Clear existing memories
        self.memories.clear();

        // Read all JSON files in the directory
        for entry in fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(memory) = self.load_memory_from_file(&path) {
                    debug!("Loaded memory: {}", memory.name);
                    self.memories.insert(memory.name.clone(), memory);
                }
            }
        }

        info!("Loaded {} memories", self.memories.len());
        Ok(())
    }

    /// Loads a single memory from a file
    fn load_memory_from_file(&self, path: &Path) -> Result<Memory> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        
        let memory: Memory = serde_json::from_str(&content)?;
        Ok(memory)
    }

    /// Saves a memory to the storage directory
    fn save_memory_to_file(&self, memory: &Memory) -> Result<()> {
        let file_name = format!("{}.json", memory.name);
        let file_path = self.storage_dir.join(file_name);
        
        let content = serde_json::to_string_pretty(memory)?;
        let mut file = File::create(file_path)?;
        file.write_all(content.as_bytes())?;
        
        Ok(())
    }

    /// Adds or updates a memory
    pub fn write_memory(&mut self, memory: Memory) -> Result<()> {
        debug!("Writing memory: {}", memory.name);
        
        // Save to file first
        self.save_memory_to_file(&memory)?;
        
        // Update in-memory store
        self.memories.insert(memory.name.clone(), memory);
        
        Ok(())
    }

    /// Retrieves a memory by name
    pub fn read_memory(&self, name: &str) -> Option<&Memory> {
        self.memories.get(name)
    }

    /// Lists all memories
    pub fn list_memories(&self) -> Vec<&Memory> {
        self.memories.values().collect()
    }

    /// Filters memories by tags
    pub fn filter_by_tags(&self, tags: &[String]) -> Vec<&Memory> {
        if tags.is_empty() {
            return self.list_memories();
        }
        
        self.memories
            .values()
            .filter(|memory| tags.iter().all(|tag| memory.tags.contains(tag)))
            .collect()
    }

    /// Deletes a memory
    pub fn delete_memory(&mut self, name: &str) -> Result<bool> {
        if let Some(memory) = self.memories.remove(name) {
            // Delete the file
            let file_name = format!("{}.json", memory.name);
            let file_path = self.storage_dir.join(file_name);
            
            if file_path.exists() {
                fs::remove_file(file_path)?;
            }
            
            debug!("Deleted memory: {}", name);
            return Ok(true);
        }
        
        Ok(false)
    }
}

/// Thread-safe memory store
pub type SharedMemoryStore = Arc<Mutex<MemoryStore>>;

/// Creates a new shared memory store
pub fn create_shared_memory_store(storage_dir: PathBuf) -> Result<SharedMemoryStore> {
    let store = MemoryStore::new(storage_dir)?;
    Ok(Arc::new(Mutex::new(store)))
}
