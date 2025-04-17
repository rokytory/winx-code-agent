use crate::core::types::{AllowedItems, CodeWriterConfig, Mode, ModeType};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
