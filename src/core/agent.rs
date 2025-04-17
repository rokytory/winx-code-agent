use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{error, info};

use crate::core::state::{create_shared_state, SharedState};
use crate::core::types::Initialize;

/// The Winx agent that implements MCP functionality
pub struct WinxAgent {
    state: SharedState,
}

impl WinxAgent {
    /// Create a new Winx agent
    pub async fn new(init: Initialize) -> Result<Self> {
        info!("Creating new Winx agent with mode: {:?}", init.mode_name);

        let workspace_path = PathBuf::from(&init.any_workspace_path);
        let task_id = if init.task_id_to_resume.is_empty() {
            None
        } else {
            Some(init.task_id_to_resume)
        };

        let state = create_shared_state(
            workspace_path,
            init.mode_name,
            init.code_writer_config,
            task_id,
        )?;

        // Process initial files if any
        if !init.initial_files_to_read.is_empty() {
            info!("Reading initial files: {:?}", init.initial_files_to_read);
            // TODO: Implement file reading logic
        }

        Ok(Self { state })
    }

    /// Start the MCP server
    pub async fn start_server(&self) -> Result<()> {
        info!("Starting MCP server");

        // Register the tools provided by this agent
        // TODO: Implement tool registration

        Ok(())
    }

    /// Execute a command and return the result
    pub async fn execute_command(&self, command: &str) -> Result<String> {
        let state = self.state.lock().unwrap();

        if !state.is_command_allowed(command) {
            return Err(anyhow::anyhow!("Command not allowed: {}", command));
        }

        // Execute the command
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&state.workspace_path)
            .output()
            .await
            .context("Failed to execute command")?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !output.status.success() {
            error!("Command failed with status: {}", output.status);
            error!("Stderr: {}", stderr);
        }

        // Combine stdout and stderr
        let mut result = stdout;
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str("STDERR: ");
            result.push_str(&stderr);
        }

        Ok(result)
    }
}

// Implementação de Service será adicionada posteriormente quando
// tivermos uma compreensão melhor das necessidades específicas do MCP
