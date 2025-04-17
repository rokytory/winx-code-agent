use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

use crate::code::symbol::SymbolManager;
use crate::lsp::server::LSPServer;

/// Tracks file modifications to maintain the project context
#[derive(Debug, Default)]
pub struct LinesRead {
    /// Maps file paths to sets of line ranges that have been read
    files: HashMap<String, Vec<(usize, usize)>>,
}

impl LinesRead {
    /// Add a range of lines that have been read
    pub fn add_lines_read(&mut self, relative_path: &str, start_line: usize, end_line: usize) {
        let entry = self.files.entry(relative_path.to_string()).or_default();
        entry.push((start_line, end_line));
    }

    /// Check if a range of lines has been read
    pub fn were_lines_read(&self, relative_path: &str, start_line: usize, end_line: usize) -> bool {
        if let Some(ranges) = self.files.get(relative_path) {
            for &(range_start, range_end) in ranges {
                if range_start <= start_line && range_end >= end_line {
                    return true;
                }
            }
        }
        false
    }

    /// Invalidate lines read for a file when it's modified
    pub fn invalidate_lines_read(&mut self, relative_path: &str) {
        self.files.remove(relative_path);
    }
}

/// Manager for project memories
pub struct MemoryManager {
    /// Directory where memories are stored
    memory_dir: PathBuf,
}

impl MemoryManager {
    /// Create a new memory manager
    pub fn new(project_dir: impl AsRef<Path>) -> Result<Self> {
        let memory_dir = project_dir.as_ref().join(".winx").join("memories");

        // Create the memory directory if it doesn't exist
        tokio::task::block_in_place(|| {
            std::fs::create_dir_all(&memory_dir).context("Failed to create memory directory")
        })?;

        Ok(Self { memory_dir })
    }

    /// Save a memory
    pub async fn save_memory(&self, name: &str, content: &str) -> Result<String> {
        let file_path = self.memory_dir.join(name);
        fs::write(&file_path, content)
            .await
            .context(format!("Failed to save memory file: {}", name))?;

        Ok(format!("Memory file '{}' saved successfully", name))
    }

    /// Load a memory
    pub async fn load_memory(&self, name: &str) -> Result<String> {
        let file_path = self.memory_dir.join(name);

        if !file_path.exists() {
            return Ok(format!(
                "Memory file '{}' not found. Consider creating it with the write_memory function if needed.",
                name
            ));
        }

        fs::read_to_string(&file_path)
            .await
            .context(format!("Failed to read memory file: {}", name))
    }

    /// List available memories
    pub async fn list_memories(&self) -> Result<Vec<String>> {
        let mut memories = Vec::new();

        let mut entries = fs::read_dir(&self.memory_dir)
            .await
            .context("Failed to read memory directory")?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    memories.push(name.to_string());
                }
            }
        }

        Ok(memories)
    }

    /// Delete a memory
    pub async fn delete_memory(&self, name: &str) -> Result<String> {
        let file_path = self.memory_dir.join(name);

        if !file_path.exists() {
            return Ok(format!("Memory file '{}' not found", name));
        }

        fs::remove_file(&file_path)
            .await
            .context(format!("Failed to delete memory file: {}", name))?;

        Ok(format!("Memory file '{}' deleted successfully", name))
    }
}

/// Comprehensive code manager that coordinates symbol operations, tracking, and memories
pub struct CodeManager {
    /// Symbol manager for code operations
    symbol_manager: SymbolManager,

    /// Memory manager for project context
    memory_manager: MemoryManager,

    /// Tracks file modifications
    lines_read: Mutex<LinesRead>,

    /// Project root path
    root_path: PathBuf,

    /// Modified files
    modified_files: Mutex<Vec<String>>,
}

impl CodeManager {
    /// Create a new code manager
    pub async fn new(
        lsp_server: Arc<Mutex<LSPServer>>,
        root_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let root = root_path.as_ref().to_path_buf();
        let symbol_manager = SymbolManager::new(lsp_server, &root);
        let memory_manager = MemoryManager::new(&root)?;

        Ok(Self {
            symbol_manager,
            memory_manager,
            lines_read: Mutex::new(LinesRead::default()),
            root_path: root,
            modified_files: Mutex::new(Vec::new()),
        })
    }

    /// Get the symbol manager
    pub fn symbol_manager(&self) -> &SymbolManager {
        &self.symbol_manager
    }

    /// Track file modification
    pub async fn mark_file_modified(&self, relative_path: &str) {
        let mut modified = self.modified_files.lock().await;

        if !modified.contains(&relative_path.to_string()) {
            modified.push(relative_path.to_string());
        }

        // Also invalidate lines read
        let mut lines_read = self.lines_read.lock().await;
        lines_read.invalidate_lines_read(relative_path);
    }

    /// Get list of modified files
    pub async fn get_modified_files(&self) -> Vec<String> {
        let modified = self.modified_files.lock().await;
        modified.clone()
    }

    /// Register lines read in a file
    pub async fn add_lines_read(&self, relative_path: &str, start_line: usize, end_line: usize) {
        let mut lines_read = self.lines_read.lock().await;
        lines_read.add_lines_read(relative_path, start_line, end_line);
    }

    /// Check if lines have been read
    pub async fn were_lines_read(
        &self,
        relative_path: &str,
        start_line: usize,
        end_line: usize,
    ) -> bool {
        let lines_read = self.lines_read.lock().await;
        lines_read.were_lines_read(relative_path, start_line, end_line)
    }

    /// Save a memory
    pub async fn save_memory(&self, name: &str, content: &str) -> Result<String> {
        self.memory_manager.save_memory(name, content).await
    }

    /// Load a memory
    pub async fn load_memory(&self, name: &str) -> Result<String> {
        self.memory_manager.load_memory(name).await
    }

    /// List available memories
    pub async fn list_memories(&self) -> Result<Vec<String>> {
        self.memory_manager.list_memories().await
    }

    /// Delete a memory
    pub async fn delete_memory(&self, name: &str) -> Result<String> {
        self.memory_manager.delete_memory(name).await
    }

    /// Reads a file while tracking the lines read
    pub async fn read_file(
        &self,
        relative_path: &str,
        start_line: usize,
        end_line: Option<usize>,
    ) -> Result<String> {
        let file_path = self.root_path.join(relative_path);

        if !file_path.exists() {
            return Err(anyhow::anyhow!("File not found: {}", relative_path));
        }

        let content = fs::read_to_string(&file_path)
            .await
            .context(format!("Failed to read file: {}", relative_path))?;

        let lines: Vec<&str> = content.lines().collect();

        let end = end_line.unwrap_or(lines.len() - 1).min(lines.len() - 1);
        let start = start_line.min(end);

        // Register the lines as read
        self.add_lines_read(relative_path, start, end).await;

        // Extract the requested lines
        let result = lines[start..=end].join("\n");

        Ok(result)
    }

    /// Create a new file or completely replace an existing one
    pub async fn create_file(&self, relative_path: &str, content: &str) -> Result<()> {
        let file_path = self.root_path.join(relative_path);

        // Create parent directories if they don't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.context(format!(
                "Failed to create parent directory for: {}",
                relative_path
            ))?;
        }

        fs::write(&file_path, content)
            .await
            .context(format!("Failed to write file: {}", relative_path))?;

        self.mark_file_modified(relative_path).await;

        Ok(())
    }

    /// Replace lines in a file
    pub async fn replace_lines(
        &self,
        relative_path: &str,
        start_line: usize,
        end_line: usize,
        new_content: &str,
    ) -> Result<()> {
        // First check if the lines were read to verify the operation
        if !self
            .were_lines_read(relative_path, start_line, end_line)
            .await
        {
            return Err(anyhow::anyhow!(
                "Cannot replace lines that were not previously read. Please read the lines first to verify the content."
            ));
        }

        let file_path = self.root_path.join(relative_path);

        if !file_path.exists() {
            return Err(anyhow::anyhow!("File not found: {}", relative_path));
        }

        let content = fs::read_to_string(&file_path)
            .await
            .context(format!("Failed to read file: {}", relative_path))?;

        let lines: Vec<&str> = content.lines().collect();

        if start_line >= lines.len() || end_line >= lines.len() {
            return Err(anyhow::anyhow!(
                "Line range {}-{} is out of bounds for file with {} lines",
                start_line,
                end_line,
                lines.len()
            ));
        }

        // Replace the specified lines
        let mut new_lines = Vec::new();
        new_lines.extend_from_slice(&lines[0..start_line]);
        new_lines.extend(new_content.lines());
        if end_line + 1 < lines.len() {
            new_lines.extend_from_slice(&lines[end_line + 1..]);
        }

        // Write the modified content back to the file
        let new_content = new_lines.join("\n");
        fs::write(&file_path, new_content)
            .await
            .context(format!("Failed to write file: {}", relative_path))?;

        self.mark_file_modified(relative_path).await;

        Ok(())
    }
}
