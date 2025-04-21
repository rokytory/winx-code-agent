use anyhow::Result;
use glob::glob;
use rmcp::{model::CallToolResult, model::ErrorCode, schemars, tool, Error as McpError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::NamedTempFile;

use crate::tools::initialize::{Action, Initialize};

#[derive(Debug, Clone)]
pub struct ContextSave {
    // State would be stored here in a full implementation
}

impl ContextSave {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ContextSave {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ContextSaveParams {
    #[schemars(description = "Task ID")]
    pub id: String,

    #[schemars(description = "Project root path")]
    pub project_root_path: String,

    #[schemars(description = "Task description")]
    pub description: String,

    #[schemars(description = "Relevant file globs")]
    pub relevant_file_globs: Vec<String>,
}

#[tool(tool_box)]
impl ContextSave {
    #[tool(description = "Save context for future reference")]
    pub async fn context_save(
        &self,
        #[tool(aggr)] params: ContextSaveParams,
    ) -> Result<CallToolResult, McpError> {
        // Check if initialization has been done
        crate::ensure_initialized!("You must call 'initialize' before saving context.");

        // Check permission (ContextSave is allowed in all modes)
        // Convert WinxError to McpError for permissions
        if let Err(e) = Initialize::check_permission(Action::SaveContext, None) {
            return Err(e.to_mcp_error());
        }

        // Create a temporary file to store the context
        let mut temp_file = NamedTempFile::new().map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to create temporary file: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;
        let mut content = String::new();

        // Add project root
        content.push_str(&format!("# PROJECT ROOT = {}\n", params.project_root_path));

        // Add description
        content.push_str(&params.description);
        content.push_str("\n\n");

        // Add relevant file globs
        content.push_str("# Relevant file paths\n");
        content.push_str(&params.relevant_file_globs.join(", "));
        content.push_str("\n\n");

        // Add relevant files
        content.push_str("# Relevant Files:\n");

        let mut all_files = Vec::new();
        for glob_pattern in &params.relevant_file_globs {
            let pattern = if glob_pattern.starts_with("/") {
                glob_pattern.to_string()
            } else {
                format!("{}/{}", params.project_root_path, glob_pattern)
            };

            match glob(&pattern) {
                Ok(paths) => {
                    for path in paths.flatten() {
                        if path.is_file() {
                            all_files.push(path);
                        }
                    }
                }
                Err(e) => {
                    content.push_str(&format!(
                        "Warning: Invalid glob pattern {}: {}\n",
                        pattern, e
                    ));
                }
            }
        }

        // Read and add file contents
        for file_path in all_files.iter().take(10) {
            // Limiting to 10 files for example
            if let Ok(file_content) = fs::read_to_string(file_path) {
                content.push_str(&format!(
                    "\n# File: {}\n```\n{}\n```\n",
                    file_path.display(),
                    file_content
                ));
            }
        }

        // Write the content to the temporary file
        use std::io::Write;
        write!(temp_file, "{}", content).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to write to temporary file: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        // Create a directory to store the context
        let app_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("winx-code-agent")
            .join("memory");
        fs::create_dir_all(&app_dir).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to create directory: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        // Create an index file if it doesn't exist
        let index_file = app_dir.join("index.json");
        let mut index = if index_file.exists() {
            match fs::read_to_string(&index_file) {
                Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(json) => json,
                    Err(_) => json!({ "tasks": [] }),
                },
                Err(_) => json!({ "tasks": [] }),
            }
        } else {
            json!({ "tasks": [] })
        };

        // Add task to index
        let task_entry = json!({
            "id": params.id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "description": params.description.lines().next().unwrap_or(""),
            "project_path": params.project_root_path,
            "file_count": all_files.len()
        });

        // Update the index
        if let Some(tasks) = index["tasks"].as_array_mut() {
            tasks.push(task_entry);
        }

        // Write the index file
        fs::write(
            &index_file,
            serde_json::to_string_pretty(&index).unwrap_or_default(),
        )
        .map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to write index file: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        // Move the temporary file to the final location
        let context_file = app_dir.join(format!("{}.txt", params.id));
        fs::copy(temp_file.path(), &context_file).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to copy file: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        let resume_instructions = format!(
            "To resume this task in a new conversation, use:\n```\nResume task: {}\n```",
            params.id
        );

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            format!(
                "Context saved to {}\n\n{}\n\nTask ID: {}",
                context_file.display(),
                resume_instructions,
                params.id
            ),
        )]))
    }
}
