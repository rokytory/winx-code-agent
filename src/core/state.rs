use crate::core::types::{AllowedItems, CodeWriterConfig, Mode, ModeType};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

/// Information about a file that has been read
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileReadInfo {
    /// File hash for change detection
    pub file_hash: String,
    /// Line ranges that have been read (inclusive start, inclusive end)
    pub line_ranges: Vec<(usize, usize)>,
    /// Total number of lines in the file
    pub total_lines: usize,
    /// Last read timestamp
    pub last_read: chrono::DateTime<chrono::Utc>,
}

impl FileReadInfo {
    /// Create a new file read info
    pub fn new(path: &Path) -> Self {
        // Calculate hash and total lines
        let hash = match std::fs::read(path) {
            Ok(content) => {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(&content);
                format!("{:x}", hasher.finalize())
            },
            Err(_) => String::new(),
        };
        
        // Count total lines
        let total_lines = match std::fs::read_to_string(path) {
            Ok(content) => content.lines().count(),
            Err(_) => 0,
        };
        
        Self {
            file_hash: hash,
            line_ranges: Vec::new(),
            total_lines,
            last_read: chrono::Utc::now(),
        }
    }
    
    /// Add a line range that has been read
    pub fn add_range(&mut self, start: usize, end: usize) {
        // Normalize range (ensure start <= end)
        let start = start.min(end);
        let end = end.max(start);
        
        // Check for overlaps and merge ranges
        let mut merged = false;
        for range in &mut self.line_ranges {
            // Check if ranges overlap or are adjacent
            if (start <= range.1 + 1) && (end + 1 >= range.0) {
                // Merge ranges
                range.0 = range.0.min(start);
                range.1 = range.1.max(end);
                merged = true;
                break;
            }
        }
        
        if !merged {
            self.line_ranges.push((start, end));
        }
        
        // Sort ranges
        self.line_ranges.sort_by_key(|r| r.0);
        
        // Merge any overlapping ranges after sorting
        let mut i = 0;
        while i < self.line_ranges.len() - 1 {
            if self.line_ranges[i].1 + 1 >= self.line_ranges[i + 1].0 {
                // Merge these ranges
                let merged_end = self.line_ranges[i + 1].1.max(self.line_ranges[i].1);
                self.line_ranges[i].1 = merged_end;
                self.line_ranges.remove(i + 1);
            } else {
                i += 1;
            }
        }
        
        // Update last read time
        self.last_read = chrono::Utc::now();
    }
    
    /// Check if a file has changed since it was read
    pub fn has_changed(&self, path: &Path) -> Result<bool> {
        let current_hash = match std::fs::read(path) {
            Ok(content) => {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(&content);
                format!("{:x}", hasher.finalize())
            },
            Err(e) => return Err(anyhow::anyhow!("Failed to read file: {}", e)),
        };
        
        Ok(current_hash != self.file_hash)
    }
    
    /// Calculate the percentage of the file that has been read
    pub fn percentage_read(&self) -> f64 {
        if self.total_lines == 0 {
            return 100.0;
        }
        
        let mut read_lines = 0;
        for &(start, end) in &self.line_ranges {
            read_lines += end - start + 1;
        }
        
        (read_lines as f64 / self.total_lines as f64) * 100.0
    }
    
    /// Get ranges of the file that haven't been read yet
    pub fn get_unread_ranges(&self) -> Vec<(usize, usize)> {
        if self.total_lines == 0 || self.line_ranges.is_empty() {
            return if self.total_lines > 0 {
                vec![(1, self.total_lines)]
            } else {
                Vec::new()
            };
        }
        
        let mut unread = Vec::new();
        let mut current_line = 1;
        
        for &(start, end) in &self.line_ranges {
            if current_line < start {
                unread.push((current_line, start - 1));
            }
            current_line = end + 1;
        }
        
        if current_line <= self.total_lines {
            unread.push((current_line, self.total_lines));
        }
        
        unread
    }
}

/// Task state for storage and resumption
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskState {
    /// Workspace path
    pub workspace_path: String,
    /// Mode configuration
    pub mode: Mode,
    /// File read tracking
    pub read_files: HashMap<PathBuf, FileReadInfo>,
    /// Background processes
    pub background_processes: Vec<String>,
}

/// Represents the current state of the Winx agent
#[derive(Debug, Clone)]
pub struct AgentState {
    /// Current working directory
    pub workspace_path: PathBuf,
    /// Current operating mode
    pub mode: Mode,
    /// Task ID if resuming
    pub task_id: Option<String>,
    /// Process status
    pub process_running: bool,
    /// Process exit code
    pub last_exit_code: Option<i32>,
    /// Files that have been read with their tracking info
    pub read_files: HashMap<PathBuf, FileReadInfo>,
    /// Active terminal session ID
    pub terminal_session_id: Option<String>,
    /// Running background processes
    pub background_processes: Vec<String>,
}

impl AgentState {
    /// Create a new agent state
    pub fn new(
        workspace_path: impl AsRef<Path>,
        mode_type: ModeType,
        config: Option<CodeWriterConfig>,
        task_id: Option<String>,
    ) -> Result<Self> {
        let workspace = PathBuf::from(workspace_path.as_ref())
            .canonicalize()
            .context("Failed to canonicalize workspace path")?;

        let mode = match mode_type {
            ModeType::Wcgw => Mode::Wcgw,
            ModeType::Architect => Mode::Architect,
            ModeType::CodeWriter => {
                let config = config.ok_or_else(|| anyhow::anyhow!("CodeWriter mode requires configuration"))?;
                Mode::CodeWriter(config)
            }
        };

        Ok(Self {
            workspace_path: workspace,
            mode,
            task_id,
            process_running: false,
            last_exit_code: None,
            read_files: HashMap::new(),
            terminal_session_id: None,
            background_processes: Vec::new(),
        })
    }

    /// Check if a path is within the allowed workspace
    pub fn is_path_allowed(&self, path: impl AsRef<Path>) -> bool {
        let path = PathBuf::from(path.as_ref());

        // Check if the path is absolute and within the workspace
        if path.is_absolute() {
            path.starts_with(&self.workspace_path)
        } else {
            // Relative paths are allowed, as they'll be resolved relative to workspace
            true
        }
    }

    /// Check if a command is allowed to be executed
    pub fn is_command_allowed(&self, command: &str) -> bool {
        match &self.mode {
            Mode::Wcgw | Mode::Architect => true, // All commands allowed
            Mode::CodeWriter(config) => {
                match &config.allowed_commands {
                    AllowedItems::All(s) if s == "all" => true,
                    AllowedItems::Specific(commands) => {
                        // Simple check - the command starts with an allowed command
                        commands.iter().any(|allowed| command.starts_with(allowed))
                    }
                    _ => false,
                }
            }
        }
    }

    /// Set process status
    pub fn set_process_status(&mut self, running: bool, exit_code: Option<i32>) {
        self.process_running = running;
        self.last_exit_code = exit_code;
    }
    
    /// Record file read with line ranges
    pub fn record_file_read(&mut self, path: impl AsRef<Path>, ranges: &[(usize, usize)]) -> Result<()> {
        let path = path.as_ref();
        
        // Ensure it's an absolute path
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_path.join(path)
        };
        
        // Get or create file info
        let file_info = self.read_files
            .entry(path.clone())
            .or_insert_with(|| FileReadInfo::new(&path));
            
        // Record ranges
        for &(start, end) in ranges {
            file_info.add_range(start, end);
        }
        
        Ok(())
    }
    
    /// Check if a file can be edited based on read history
    pub fn can_edit_file(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = path.as_ref();
        
        // Ensure it's an absolute path
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_path.join(path)
        };
        
        // If file doesn't exist yet, it can be created
        if !path.exists() {
            return Ok(true);
        }
        
        // Check read history
        if let Some(file_info) = self.read_files.get(&path) {
            // Check if file has changed
            if file_info.has_changed(&path)? {
                return Ok(false);
            }
            
            // Check read percentage
            let percentage = file_info.percentage_read();
            Ok(percentage >= 95.0)
        } else {
            // File hasn't been read
            Ok(false)
        }
    }
    
    /// Get unread ranges for a file
    pub fn get_unread_ranges(&self, path: impl AsRef<Path>) -> Result<Vec<(usize, usize)>> {
        let path = path.as_ref();
        
        // Ensure it's an absolute path
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_path.join(path)
        };
        
        if let Some(file_info) = self.read_files.get(&path) {
            Ok(file_info.get_unread_ranges())
        } else {
            // If file exists, return the entire file as unread
            if path.exists() {
                let total_lines = std::fs::read_to_string(&path)?
                    .lines()
                    .count();
                
                if total_lines > 0 {
                    Ok(vec![(1, total_lines)])
                } else {
                    Ok(Vec::new())
                }
            } else {
                Ok(Vec::new())
            }
        }
    }
    
    /// Register a terminal session
    pub fn set_terminal_session(&mut self, session_id: String) {
        self.terminal_session_id = Some(session_id);
    }
    
    /// Register a background process
    pub fn add_background_process(&mut self, process_id: String) {
        self.background_processes.push(process_id);
    }
    
    /// Get active terminal session ID
    pub fn get_terminal_session(&self) -> Option<String> {
        self.terminal_session_id.clone()
    }
    
    /// Get active background processes
    pub fn get_background_processes(&self) -> Vec<String> {
        self.background_processes.clone()
    }
    
    /// Save state to a task
    pub fn save_to_task(&self, task_id: &str) -> Result<()> {
        // Create task state
        let task_state = TaskState {
            workspace_path: self.workspace_path.to_string_lossy().to_string(),
            mode: self.mode.clone(),
            read_files: self.read_files.clone(),
            background_processes: self.background_processes.clone(),
        };
        
        // Save to file in task directory
        let task_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
            .join("winx")
            .join("tasks");
            
        std::fs::create_dir_all(&task_dir)?;
        
        let task_path = task_dir.join(format!("{}.json", task_id));
        let task_json = serde_json::to_string_pretty(&task_state)?;
        
        std::fs::write(task_path, task_json)?;
        
        Ok(())
    }
    
    /// Load state from a task
    pub fn load_from_task(task_id: &str) -> Result<Self> {
        let task_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
            .join("winx")
            .join("tasks");
            
        let task_path = task_dir.join(format!("{}.json", task_id));
        
        if !task_path.exists() {
            return Err(anyhow::anyhow!("Task not found: {}", task_id));
        }
        
        let task_json = std::fs::read_to_string(task_path)?;
        let task_state: TaskState = serde_json::from_str(&task_json)?;
        
        Ok(Self {
            workspace_path: PathBuf::from(task_state.workspace_path),
            mode: task_state.mode,
            task_id: Some(task_id.to_string()),
            process_running: false,
            last_exit_code: None,
            read_files: task_state.read_files,
            terminal_session_id: None,
            background_processes: task_state.background_processes,
        })
    }
}

/// Thread-safe agent state container
pub type SharedState = Arc<Mutex<AgentState>>;

/// Create a new shared state
pub fn create_shared_state(
    workspace_path: impl AsRef<Path>,
    mode_type: ModeType,
    config: Option<CodeWriterConfig>,
    task_id: Option<String>,
) -> Result<SharedState> {
    let state = AgentState::new(workspace_path, mode_type, config, task_id)?;
    Ok(Arc::new(Mutex::new(state)))
}
