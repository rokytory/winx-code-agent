use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::core::state::{AgentState, FileReadInfo};
use crate::core::types::Mode;

/// Task information for saving and resuming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    /// Task ID
    pub id: String,
    /// Task creation time
    pub created_at: DateTime<Utc>,
    /// Task last updated time
    pub updated_at: DateTime<Utc>,
    /// Task workspace path
    pub workspace_path: String,
    /// Task description
    pub description: String,
    /// Task mode
    pub mode: Mode,
    /// File read statistics
    pub file_reads: HashMap<PathBuf, FileReadInfo>,
    /// Task status
    pub status: TaskStatus,
    /// Relevant file paths (relative to workspace)
    pub relevant_files: Vec<String>,
}

/// Task status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    /// Task is active
    Active,
    /// Task is completed
    Completed,
    /// Task is paused
    Paused,
}

impl TaskInfo {
    /// Create a new task information
    pub fn new(
        workspace_path: impl AsRef<Path>,
        description: String,
        mode: Mode,
        file_reads: HashMap<PathBuf, FileReadInfo>,
        relevant_files: Vec<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
            workspace_path: workspace_path.as_ref().to_string_lossy().to_string(),
            description,
            mode,
            file_reads,
            status: TaskStatus::Active,
            relevant_files,
        }
    }

    /// Update task information
    pub fn update(&mut self, state: &AgentState, description: Option<String>) {
        self.updated_at = Utc::now();

        if let Some(desc) = description {
            self.description = desc;
        }

        self.file_reads = state.read_files.clone();
    }

    /// Mark task as completed
    pub fn complete(&mut self) {
        self.updated_at = Utc::now();
        self.status = TaskStatus::Completed;
    }

    /// Mark task as paused
    pub fn pause(&mut self) {
        self.updated_at = Utc::now();
        self.status = TaskStatus::Paused;
    }

    /// Resume task
    pub fn resume(&mut self) {
        self.updated_at = Utc::now();
        self.status = TaskStatus::Active;
    }

    /// Get task status as string
    pub fn status_str(&self) -> &'static str {
        match self.status {
            TaskStatus::Active => "active",
            TaskStatus::Completed => "completed",
            TaskStatus::Paused => "paused",
        }
    }
}

/// Task storage manager
pub struct TaskManager {
    /// Task storage directory
    tasks_dir: PathBuf,
}

impl TaskManager {
    /// Create a new task manager
    pub fn new() -> Result<Self> {
        let tasks_dir = Self::get_tasks_dir()?;
        Ok(Self { tasks_dir })
    }

    /// Get tasks directory
    pub fn get_tasks_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
            .join("winx")
            .join("tasks");

        fs::create_dir_all(&data_dir).context("Failed to create tasks directory")?;

        Ok(data_dir)
    }

    /// Save task information
    pub fn save_task(&self, task: &TaskInfo) -> Result<()> {
        let task_path = self.tasks_dir.join(format!("{}.json", task.id));

        let task_json = serde_json::to_string_pretty(task).context("Failed to serialize task")?;

        fs::write(&task_path, task_json)
            .with_context(|| format!("Failed to write task file: {}", task_path.display()))?;

        debug!("Saved task {} to {}", task.id, task_path.display());
        Ok(())
    }

    /// Load task information
    pub fn load_task(&self, task_id: &str) -> Result<TaskInfo> {
        let task_path = self.tasks_dir.join(format!("{}.json", task_id));

        if !task_path.exists() {
            return Err(anyhow::anyhow!("Task not found: {}", task_id));
        }

        let task_json = fs::read_to_string(&task_path)
            .with_context(|| format!("Failed to read task file: {}", task_path.display()))?;

        let task: TaskInfo =
            serde_json::from_str(&task_json).context("Failed to deserialize task")?;

        debug!("Loaded task {} from {}", task.id, task_path.display());
        Ok(task)
    }

    /// List all tasks
    pub fn list_tasks(&self) -> Result<Vec<TaskInfo>> {
        let mut tasks = Vec::new();

        for entry in fs::read_dir(&self.tasks_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().map_or(false, |e| e == "json") {
                if let Ok(task_json) = fs::read_to_string(&path) {
                    if let Ok(task) = serde_json::from_str::<TaskInfo>(&task_json) {
                        tasks.push(task);
                    } else {
                        warn!("Failed to parse task file: {}", path.display());
                    }
                }
            }
        }

        // Sort by updated time, newest first
        tasks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(tasks)
    }

    /// Delete task
    pub fn delete_task(&self, task_id: &str) -> Result<()> {
        let task_path = self.tasks_dir.join(format!("{}.json", task_id));

        if !task_path.exists() {
            return Err(anyhow::anyhow!("Task not found: {}", task_id));
        }

        fs::remove_file(&task_path)
            .with_context(|| format!("Failed to delete task file: {}", task_path.display()))?;

        debug!("Deleted task {} at {}", task_id, task_path.display());
        Ok(())
    }

    /// Format task description with Markdown
    pub fn format_task_description(
        task: &TaskInfo,
        include_file_contents: bool,
        max_file_count: usize,
        max_chars_per_file: usize,
    ) -> Result<String> {
        let mut output = String::new();

        // Basic task information
        output.push_str(&format!("# Task: {}\n\n", task.id));
        output.push_str(&format!("- **Status**: {}\n", task.status_str()));
        output.push_str(&format!(
            "- **Created**: {}\n",
            task.created_at.format("%Y-%m-%d %H:%M:%S")
        ));
        output.push_str(&format!(
            "- **Last Updated**: {}\n",
            task.updated_at.format("%Y-%m-%d %H:%M:%S")
        ));
        output.push_str(&format!("- **Workspace**: {}\n\n", task.workspace_path));

        // Task description
        output.push_str("## Description\n\n");
        output.push_str(&task.description);
        output.push_str("\n\n");

        // Relevant files
        output.push_str("## Relevant Files\n\n");

        if task.relevant_files.is_empty() {
            output.push_str("No relevant files specified.\n\n");
        } else {
            for (i, file) in task.relevant_files.iter().enumerate() {
                if i >= max_file_count {
                    output.push_str(&format!(
                        "... and {} more files\n\n",
                        task.relevant_files.len() - i
                    ));
                    break;
                }

                output.push_str(&format!("- `{}`\n", file));

                // Add file contents if requested
                if include_file_contents {
                    let full_path = Path::new(&task.workspace_path).join(file);

                    if full_path.exists() && full_path.is_file() {
                        if let Ok(content) = fs::read_to_string(&full_path) {
                            let content = if content.len() > max_chars_per_file {
                                let mut truncated =
                                    content.chars().take(max_chars_per_file).collect::<String>();
                                truncated.push_str("\n... (truncated)");
                                truncated
                            } else {
                                content
                            };

                            // Determine file type for syntax highlighting
                            let file_type = match full_path.extension().and_then(|e| e.to_str()) {
                                Some("rs") => "rust",
                                Some("js") => "javascript",
                                Some("py") => "python",
                                Some("ts") => "typescript",
                                Some("json") => "json",
                                Some("toml") => "toml",
                                Some("md") => "markdown",
                                Some("html") => "html",
                                Some("css") => "css",
                                Some("c") | Some("cpp") | Some("h") => "cpp",
                                _ => "",
                            };

                            output.push_str(&format!("```{}\n{}\n```\n\n", file_type, content));
                        }
                    }
                }
            }
        }

        // File read statistics
        output.push_str("## File Activity\n\n");

        if task.file_reads.is_empty() {
            output.push_str("No file activity recorded.\n");
        } else {
            let mut file_reads = task
                .file_reads
                .iter()
                .map(|(path, info)| {
                    let read_percentage = info.percentage_read();
                    let rel_path = if path.starts_with(&task.workspace_path) {
                        path.strip_prefix(&task.workspace_path)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string()
                    } else {
                        path.to_string_lossy().to_string()
                    };

                    (rel_path, read_percentage, info.line_ranges.len())
                })
                .collect::<Vec<_>>();

            // Sort by read percentage, highest first
            file_reads.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            output.push_str("| File | Read % | Read Operations |\n");
            output.push_str("|------|--------|----------------|\n");

            for (i, (path, percentage, ops)) in file_reads.iter().enumerate() {
                if i >= max_file_count {
                    output.push_str(&format!(
                        "| ... and {} more files | | |\n",
                        file_reads.len() - i
                    ));
                    break;
                }

                output.push_str(&format!("| `{}` | {:.1}% | {} |\n", path, percentage, ops));
            }

            output.push_str("\n");
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_task_creation_and_serialization() {
        let temp_dir = tempdir().unwrap();
        let workspace_path = temp_dir.path();

        // Create file structure for testing
        fs::create_dir_all(workspace_path.join("src")).unwrap();
        fs::write(workspace_path.join("README.md"), "Test project").unwrap();
        fs::write(workspace_path.join("src/main.rs"), "fn main() {}").unwrap();

        // Create task info
        let task = TaskInfo::new(
            workspace_path,
            "Test task description".to_string(),
            Mode::Wcgw,
            HashMap::new(),
            vec!["README.md".to_string(), "src/main.rs".to_string()],
        );

        // Test serialization
        let task_json = serde_json::to_string_pretty(&task).unwrap();

        // Test deserialization
        let deserialized_task: TaskInfo = serde_json::from_str(&task_json).unwrap();

        assert_eq!(task.id, deserialized_task.id);
        assert_eq!(task.description, deserialized_task.description);
        assert_eq!(task.workspace_path, deserialized_task.workspace_path);
        assert_eq!(task.relevant_files, deserialized_task.relevant_files);
    }
}
