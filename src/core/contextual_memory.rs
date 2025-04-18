use anyhow::{Context as AnyhowContext, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Status of a task being tracked in memory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is in planning phase
    Planning,
    /// Task is in progress
    InProgress,
    /// Task is being tested
    Testing,
    /// Task has been completed
    Completed,
    /// Task has failed or been abandoned
    Failed,
}

/// A decision made during task execution with reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// Description of the decision
    pub description: String,
    /// Rationale for the decision
    pub rationale: String,
    /// Alternative options considered
    pub alternatives_considered: Vec<String>,
    /// When the decision was made
    pub timestamp: DateTime<Utc>,
}

/// A symbol with its context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolContext {
    /// Name of the symbol
    pub name: String,
    /// Kind of symbol (function, class, etc.)
    pub kind: String,
    /// File containing the symbol
    pub file_path: PathBuf,
    /// Relevance to the current task
    pub relevance: String,
    /// Additional notes about the symbol
    pub notes: String,
}

/// A file with its purpose and context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    /// Path to the file
    pub path: PathBuf,
    /// Purpose of this file
    pub purpose: String,
    /// Files this one depends on
    pub dependencies: Vec<PathBuf>,
    /// Additional notes about the file
    pub notes: String,
}

/// Memory about the current context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualMemory {
    /// Unique ID for this memory
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Task this memory is associated with
    pub task_description: String,
    /// Status of the task
    pub task_status: TaskStatus,
    /// Symbols relevant to the task
    pub relevant_symbols: Vec<SymbolContext>,
    /// Files relevant to the task
    pub relevant_files: Vec<FileContext>,
    /// Decisions made during task execution
    pub decisions: Vec<Decision>,
    /// Progress notes
    pub progress_notes: Vec<String>,
    /// Completion percentage (0-100)
    pub completion_percentage: f32,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl ContextualMemory {
    /// Create a new contextual memory
    pub fn new(name: &str, task_description: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            task_description: task_description.to_string(),
            task_status: TaskStatus::Planning,
            relevant_symbols: Vec::new(),
            relevant_files: Vec::new(),
            decisions: Vec::new(),
            progress_notes: Vec::new(),
            completion_percentage: 0.0,
            tags: Vec::new(),
        }
    }

    /// Add a decision to the memory
    pub fn add_decision(&mut self, description: &str, rationale: &str, alternatives: Vec<String>) {
        self.decisions.push(Decision {
            description: description.to_string(),
            rationale: rationale.to_string(),
            alternatives_considered: alternatives,
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Add a progress note
    pub fn add_progress_note(&mut self, note: &str) {
        self.progress_notes.push(note.to_string());
        self.updated_at = Utc::now();
    }

    /// Update task status
    pub fn update_status(&mut self, status: TaskStatus) {
        self.task_status = status;
        self.updated_at = Utc::now();
    }

    /// Update completion percentage
    pub fn update_completion(&mut self, percentage: f32) {
        self.completion_percentage = percentage.clamp(0.0, 100.0);
        self.updated_at = Utc::now();
    }

    /// Add a relevant symbol
    pub fn add_symbol(
        &mut self,
        name: &str,
        kind: &str,
        file_path: &Path,
        relevance: &str,
        notes: &str,
    ) {
        self.relevant_symbols.push(SymbolContext {
            name: name.to_string(),
            kind: kind.to_string(),
            file_path: file_path.to_path_buf(),
            relevance: relevance.to_string(),
            notes: notes.to_string(),
        });
        self.updated_at = Utc::now();
    }

    /// Add a relevant file
    pub fn add_file(
        &mut self,
        path: &Path,
        purpose: &str,
        dependencies: Vec<PathBuf>,
        notes: &str,
    ) {
        self.relevant_files.push(FileContext {
            path: path.to_path_buf(),
            purpose: purpose.to_string(),
            dependencies,
            notes: notes.to_string(),
        });
        self.updated_at = Utc::now();
    }

    /// Get a summary of the memory
    pub fn get_summary(&self) -> String {
        let mut summary = String::new();

        summary.push_str(&format!("# Task: {}\n\n", self.name));
        summary.push_str(&format!("* Status: {:?}\n", self.task_status));
        summary.push_str(&format!(
            "* Completion: {:.1}%\n",
            self.completion_percentage
        ));
        summary.push_str(&format!("* Last updated: {}\n\n", self.updated_at));

        if !self.progress_notes.is_empty() {
            summary.push_str("## Recent Progress\n\n");
            for (i, note) in self.progress_notes.iter().rev().take(3).enumerate() {
                summary.push_str(&format!("{}. {}\n", i + 1, note));
            }
            summary.push('\n');
        }

        if !self.decisions.is_empty() {
            summary.push_str("## Key Decisions\n\n");
            for (i, decision) in self.decisions.iter().rev().take(3).enumerate() {
                summary.push_str(&format!(
                    "{}. {} - {}\n",
                    i + 1,
                    decision.description,
                    decision.rationale
                ));
            }
            summary.push('\n');
        }

        if !self.relevant_files.is_empty() {
            summary.push_str("## Key Files\n\n");
            for (i, file) in self.relevant_files.iter().take(5).enumerate() {
                summary.push_str(&format!(
                    "{}. {} - {}\n",
                    i + 1,
                    file.path.display(),
                    file.purpose
                ));
            }
        }

        summary
    }
}

/// Store for contextual memories
pub struct ContextualMemoryStore {
    /// Directory where memories are stored
    memory_dir: PathBuf,
    /// In-memory cache of loaded memories
    memories: HashMap<String, ContextualMemory>,
}

impl ContextualMemoryStore {
    /// Create a new memory store
    pub fn new(memory_dir: impl AsRef<Path>) -> Result<Self> {
        let memory_dir = memory_dir.as_ref().to_path_buf();

        // Ensure directory exists
        fs::create_dir_all(&memory_dir).context("Failed to create memory directory")?;

        Ok(Self {
            memory_dir,
            memories: HashMap::new(),
        })
    }

    /// Load all memories from disk
    pub fn load_all_memories(&mut self) -> Result<()> {
        info!("Loading all memories from {}", self.memory_dir.display());

        let entries = fs::read_dir(&self.memory_dir).context("Failed to read memory directory")?;

        let mut loaded = 0;
        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                match self.load_memory_from_file(&path) {
                    Ok(memory) => {
                        self.memories.insert(memory.id.clone(), memory);
                        loaded += 1;
                    }
                    Err(e) => {
                        warn!("Failed to load memory from {}: {}", path.display(), e);
                    }
                }
            }
        }

        info!("Loaded {} memories", loaded);
        Ok(())
    }

    /// Load a memory from a file
    fn load_memory_from_file(&self, path: &Path) -> Result<ContextualMemory> {
        let content = fs::read_to_string(path).context("Failed to read memory file")?;

        let memory: ContextualMemory =
            serde_json::from_str(&content).context("Failed to parse memory JSON")?;

        Ok(memory)
    }

    /// Save a memory to disk
    pub fn save_memory(&mut self, memory: &ContextualMemory) -> Result<()> {
        let json = serde_json::to_string_pretty(memory).context("Failed to serialize memory")?;

        let file_path = self.memory_dir.join(format!("{}.json", memory.id));

        fs::write(&file_path, json).context("Failed to write memory file")?;

        // Update cache
        self.memories.insert(memory.id.clone(), memory.clone());

        debug!("Saved memory {} to {}", memory.id, file_path.display());
        Ok(())
    }

    /// Get a memory by ID
    pub fn get_memory(&self, id: &str) -> Option<ContextualMemory> {
        self.memories.get(id).cloned()
    }

    /// Get all memories
    pub fn get_all_memories(&self) -> Vec<ContextualMemory> {
        self.memories.values().cloned().collect()
    }

    /// Create a new memory
    pub fn create_memory(
        &mut self,
        name: &str,
        task_description: &str,
    ) -> Result<ContextualMemory> {
        let memory = ContextualMemory::new(name, task_description);

        self.save_memory(&memory)?;

        Ok(memory)
    }

    /// Update an existing memory
    pub fn update_memory(&mut self, memory: ContextualMemory) -> Result<()> {
        self.save_memory(&memory)
    }

    /// Delete a memory
    pub fn delete_memory(&mut self, id: &str) -> Result<()> {
        if let Some(memory) = self.memories.remove(id) {
            let file_path = self.memory_dir.join(format!("{}.json", memory.id));

            if file_path.exists() {
                fs::remove_file(&file_path).context("Failed to delete memory file")?;
            }

            debug!("Deleted memory {}", id);
        }

        Ok(())
    }

    /// Search memories by tag
    pub fn search_by_tag(&self, tag: &str) -> Vec<ContextualMemory> {
        self.memories
            .values()
            .filter(|m| {
                m.tags
                    .iter()
                    .any(|t| t.to_lowercase() == tag.to_lowercase())
            })
            .cloned()
            .collect()
    }

    /// Search memories by text
    pub fn search_by_text(&self, query: &str) -> Vec<ContextualMemory> {
        let query = query.to_lowercase();

        self.memories
            .values()
            .filter(|m| {
                m.name.to_lowercase().contains(&query)
                    || m.task_description.to_lowercase().contains(&query)
                    || m.progress_notes
                        .iter()
                        .any(|n| n.to_lowercase().contains(&query))
                    || m.decisions
                        .iter()
                        .any(|d| d.description.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }
}

/// Thread-safe store for contextual memories
pub type SharedContextualMemoryStore = Arc<Mutex<ContextualMemoryStore>>;

/// Create a shared memory store
pub fn create_shared_contextual_memory_store(
    memory_dir: impl AsRef<Path>,
) -> Result<SharedContextualMemoryStore> {
    let mut store = ContextualMemoryStore::new(memory_dir)?;

    // Load existing memories
    let _ = store.load_all_memories();

    Ok(Arc::new(Mutex::new(store)))
}

/// Get a default memory directory
pub fn get_contextual_memory_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

    let memory_dir = data_dir.join("winx").join("contextual_memories");

    Ok(memory_dir)
}
