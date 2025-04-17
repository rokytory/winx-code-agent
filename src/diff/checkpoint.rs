use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Represents a change to a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// Relative path to the file
    pub file_path: String,
    
    /// Content before the change
    pub content_before: String,
    
    /// Content after the change
    pub content_after: String,
}

/// Represents a checkpoint in the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique ID for the checkpoint
    pub id: String,
    
    /// When the checkpoint was created
    pub timestamp: DateTime<Utc>,
    
    /// Description of the checkpoint
    pub description: String,
    
    /// Changes included in the checkpoint
    pub changes: Vec<FileChange>,
}

/// Manager for project checkpoints
pub struct CheckpointManager {
    /// Directory where checkpoints are stored
    checkpoint_dir: PathBuf,
    
    /// Project root directory
    project_root: PathBuf,
    
    /// Cache of loaded checkpoints
    checkpoints: HashMap<String, Checkpoint>,
}

impl CheckpointManager {
    /// Create a new checkpoint manager
    pub async fn new(project_dir: impl AsRef<Path>) -> Result<Self> {
        let project_root = project_dir.as_ref().to_path_buf();
        let checkpoint_dir = project_root.join(".winx").join("checkpoints");
        
        // Create the checkpoints directory if it doesn't exist
        fs::create_dir_all(&checkpoint_dir).await
            .context("Failed to create checkpoints directory")?;
        
        let mut manager = Self {
            checkpoint_dir,
            project_root,
            checkpoints: HashMap::new(),
        };
        
        // Load existing checkpoints
        manager.load_checkpoints().await?;
        
        Ok(manager)
    }
    
    /// Load existing checkpoints from the checkpoints directory
    async fn load_checkpoints(&mut self) -> Result<()> {
        let mut entries = fs::read_dir(&self.checkpoint_dir).await
            .context("Failed to read checkpoints directory")?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".json") {
                        let checkpoint_id = file_name.trim_end_matches(".json");
                        match self.load_checkpoint(checkpoint_id).await {
                            Ok(checkpoint) => {
                                self.checkpoints.insert(checkpoint_id.to_string(), checkpoint);
                            },
                            Err(e) => {
                                warn!("Failed to load checkpoint {}: {}", checkpoint_id, e);
                            }
                        }
                    }
                }
            }
        }
        
        info!("Loaded {} checkpoints", self.checkpoints.len());
        Ok(())
    }
    
    /// Load a specific checkpoint by ID
    async fn load_checkpoint(&self, id: &str) -> Result<Checkpoint> {
        let file_path = self.checkpoint_dir.join(format!("{}.json", id));
        
        let content = fs::read_to_string(&file_path).await
            .context(format!("Failed to read checkpoint file: {}", id))?;
        
        let checkpoint: Checkpoint = serde_json::from_str(&content)
            .context(format!("Failed to parse checkpoint JSON: {}", id))?;
        
        Ok(checkpoint)
    }
    
    /// Save a checkpoint to disk
    async fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let file_path = self.checkpoint_dir.join(format!("{}.json", checkpoint.id));
        
        let json = serde_json::to_string_pretty(checkpoint)
            .context("Failed to serialize checkpoint to JSON")?;
        
        fs::write(&file_path, json).await
            .context(format!("Failed to write checkpoint file: {}", checkpoint.id))?;
        
        Ok(())
    }
    
    /// Create a new checkpoint with the given changes
    pub async fn create_checkpoint(
        &mut self,
        description: &str,
        changes: Vec<FileChange>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now();
        
        let checkpoint = Checkpoint {
            id: id.clone(),
            timestamp,
            description: description.to_string(),
            changes,
        };
        
        // Save to disk
        self.save_checkpoint(&checkpoint).await?;
        
        // Add to in-memory cache
        self.checkpoints.insert(id.clone(), checkpoint);
        
        info!("Created checkpoint {} - {}", id, description);
        Ok(id)
    }
    
    /// Restore a checkpoint by ID
    pub async fn restore_checkpoint(&self, id: &str) -> Result<()> {
        let checkpoint = if let Some(checkpoint) = self.checkpoints.get(id) {
            checkpoint
        } else {
            // Try to load it if not in memory
            self.load_checkpoint(id).await?
        };
        
        info!("Restoring checkpoint {} - {}", id, checkpoint.description);
        
        // Apply changes in reverse
        for change in &checkpoint.changes {
            let file_path = self.project_root.join(&change.file_path);
            
            // Create parent directories if they don't exist
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).await
                    .context(format!("Failed to create parent directory for: {}", change.file_path))?;
            }
            
            // Write the "before" content to restore
            fs::write(&file_path, &change.content_before).await
                .context(format!("Failed to restore file: {}", change.file_path))?;
                
            debug!("Restored file: {}", change.file_path);
        }
        
        info!("Checkpoint {} successfully restored", id);
        Ok(())
    }
    
    /// Get a list of all checkpoints
    pub fn list_checkpoints(&self) -> Vec<(String, String, DateTime<Utc>)> {
        self.checkpoints
            .iter()
            .map(|(id, checkpoint)| (id.clone(), checkpoint.description.clone(), checkpoint.timestamp))
            .collect()
    }
    
    /// Get details of a specific checkpoint
    pub fn get_checkpoint(&self, id: &str) -> Option<&Checkpoint> {
        self.checkpoints.get(id)
    }
    
    /// Delete a checkpoint
    pub async fn delete_checkpoint(&mut self, id: &str) -> Result<()> {
        if !self.checkpoints.contains_key(id) {
            return Err(anyhow::anyhow!("Checkpoint not found: {}", id));
        }
        
        // Remove from memory
        self.checkpoints.remove(id);
        
        // Remove from disk
        let file_path = self.checkpoint_dir.join(format!("{}.json", id));
        fs::remove_file(&file_path).await
            .context(format!("Failed to delete checkpoint file: {}", id))?;
        
        info!("Deleted checkpoint: {}", id);
        Ok(())
    }
    
    /// Create a file change record by comparing before and after content
    pub fn create_file_change(
        &self,
        relative_path: &str,
        content_before: &str,
        content_after: &str,
    ) -> FileChange {
        FileChange {
            file_path: relative_path.to_string(),
            content_before: content_before.to_string(),
            content_after: content_after.to_string(),
        }
    }
}
