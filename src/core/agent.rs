use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::code::manager::CodeManager;
use crate::code::symbol::SymbolManager;
use crate::core::state::{create_shared_state, SharedState};
use crate::core::types::{Initialize, Language};
use crate::diff::checkpoint::CheckpointManager;
use crate::diff::git::GitIntegration;
use crate::lsp::server::LSPServer;
use crate::lsp::types::LSPConfig;

/// The Winx agent that implements MCP functionality
pub struct WinxAgent {
    /// Shared state for the agent
    state: SharedState,

    /// LSP server for code analysis
    lsp_server: Option<Arc<Mutex<LSPServer>>>,

    /// Code manager for symbolic operations
    code_manager: Option<CodeManager>,

    /// Checkpoint manager for history tracking
    checkpoint_manager: Option<CheckpointManager>,

    /// Git integration for version control
    git_integration: Option<GitIntegration>,
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
            workspace_path.clone(),
            init.mode_name,
            init.code_writer_config,
            task_id,
        )?;

        let mut agent = Self {
            state,
            lsp_server: None,
            code_manager: None,
            checkpoint_manager: None,
            git_integration: None,
        };

        // Initialize LSP server and related components
        agent.init_components(&workspace_path).await?;

        // Process initial files if any
        if !init.initial_files_to_read.is_empty() {
            info!("Reading initial files: {:?}", init.initial_files_to_read);
            agent
                .read_initial_files(&init.initial_files_to_read)
                .await?;
        }

        Ok(agent)
    }

    /// Initialize LSP and related components
    async fn init_components(&mut self, workspace_path: &Path) -> Result<()> {
        // Detect language based on file extensions in the workspace
        let language = self.detect_language(workspace_path).await?;
        info!("Detected language: {:?}", language);

        // Initialize LSP server
        let lsp_config = LSPConfig {
            language,
            ignored_paths: vec![
                ".git".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
            ],
            gitignore_content: self.read_gitignore(workspace_path).await.ok(),
            trace_lsp_communication: false,
        };

        let mut lsp_server = LSPServer::new(
            workspace_path,
            language,
            lsp_config.ignored_paths.clone(),
            lsp_config.gitignore_content.clone(),
        );

        // Start the LSP server
        lsp_server.start().await?;

        let lsp_server = Arc::new(Mutex::new(lsp_server));
        self.lsp_server = Some(lsp_server.clone());

        // Initialize code manager
        let code_manager = CodeManager::new(lsp_server.clone(), workspace_path).await?;
        self.code_manager = Some(code_manager);

        // Initialize checkpoint manager
        let checkpoint_manager = CheckpointManager::new(workspace_path).await?;
        self.checkpoint_manager = Some(checkpoint_manager);

        // Initialize git integration
        let git_integration = GitIntegration::new(workspace_path);
        self.git_integration = Some(git_integration);

        Ok(())
    }

    /// Detect the primary language of the project
    async fn detect_language(&self, workspace_path: &Path) -> Result<Language> {
        // Look for language-specific files
        let rust_count = self
            .count_files_with_extension(workspace_path, "rs")
            .await?;
        let py_count = self
            .count_files_with_extension(workspace_path, "py")
            .await?;
        let js_count = self
            .count_files_with_extension(workspace_path, "js")
            .await?;
        let ts_count = self
            .count_files_with_extension(workspace_path, "ts")
            .await?;
        let go_count = self
            .count_files_with_extension(workspace_path, "go")
            .await?;
        let java_count = self
            .count_files_with_extension(workspace_path, "java")
            .await?;
        let cs_count = self
            .count_files_with_extension(workspace_path, "cs")
            .await?;
        let cpp_count = self
            .count_files_with_extension(workspace_path, "cpp")
            .await?
            + self
            .count_files_with_extension(workspace_path, "cc")
            .await?
            + self.count_files_with_extension(workspace_path, "h").await?;
        let rb_count = self
            .count_files_with_extension(workspace_path, "rb")
            .await?;

        // Determine the most common language
        let mut language = Language::Rust; // Default to Rust
        let mut max_count = rust_count;

        if py_count > max_count {
            language = Language::Python;
            max_count = py_count;
        }
        if js_count > max_count {
            language = Language::JavaScript;
            max_count = js_count;
        }
        if ts_count > max_count {
            language = Language::TypeScript;
            max_count = ts_count;
        }
        if go_count > max_count {
            language = Language::Go;
            max_count = go_count;
        }
        if java_count > max_count {
            language = Language::Java;
            max_count = java_count;
        }
        if cs_count > max_count {
            language = Language::CSharp;
            max_count = cs_count;
        }
        if cpp_count > max_count {
            language = Language::CPlusPlus;
            max_count = cpp_count;
        }
        if rb_count > max_count {
            language = Language::Ruby;
            max_count = rb_count;
        }

        // If no files were found, default to Rust (assuming that's what we're working with)
        if max_count == 0 {
            warn!("No language-specific files found, defaulting to Rust");
        }

        Ok(language)
    }

    /// Count files with a specific extension in the workspace
    async fn count_files_with_extension(
        &self,
        workspace_path: &Path,
        extension: &str,
    ) -> Result<usize> {
        let output = Command::new("find")
            .args([
                workspace_path.to_str().unwrap_or("."),
                "-type",
                "f",
                "-name",
                &format!("*.{}", extension),
                "-not",
                "-path",
                "*/\\.*/*",
                "-not",
                "-path",
                "*/target/*",
                "-not",
                "-path",
                "*/node_modules/*",
            ])
            .output()
            .await
            .context("Failed to execute find command")?;

        if !output.status.success() {
            return Ok(0);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let count = stdout.lines().count();

        Ok(count)
    }

    /// Read .gitignore file if it exists
    async fn read_gitignore(&self, workspace_path: &Path) -> Result<String> {
        let gitignore_path = workspace_path.join(".gitignore");

        if !gitignore_path.exists() {
            return Err(anyhow::anyhow!("No .gitignore file found"));
        }

        let content = tokio::fs::read_to_string(gitignore_path)
            .await
            .context("Failed to read .gitignore file")?;

        Ok(content)
    }

    /// Read initial files specified by the user
    async fn read_initial_files(&self, file_paths: &[String]) -> Result<()> {
        if let Some(code_manager) = &self.code_manager {
            for path in file_paths {
                // Convert absolute paths to relative paths
                let rel_path = self.get_relative_path(path)?;

                debug!("Reading initial file: {}", rel_path);

                match code_manager.read_file(&rel_path, 0, None).await {
                    Ok(_content) => {
                        debug!("Successfully read file: {}", rel_path);
                    }
                    Err(e) => {
                        warn!("Failed to read file {}: {}", rel_path, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get a relative path from an absolute path
    fn get_relative_path(&self, path: &str) -> Result<String> {
        let state = self.state.lock().unwrap();
        let abs_path = PathBuf::from(path);

        if abs_path.is_relative() {
            return Ok(path.to_string());
        }

        if !abs_path.starts_with(&state.workspace_path) {
            return Err(anyhow::anyhow!("Path is outside of workspace: {}", path));
        }

        let rel_path = abs_path
            .strip_prefix(&state.workspace_path)
            .context("Failed to get relative path")?;

        Ok(rel_path.to_string_lossy().to_string())
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

    /// Get the code manager
    pub fn code_manager(&self) -> Option<&CodeManager> {
        self.code_manager.as_ref()
    }

    /// Get the checkpoint manager
    pub fn checkpoint_manager(&self) -> Option<&CheckpointManager> {
        self.checkpoint_manager.as_ref()
    }

    /// Get the git integration
    pub fn git_integration(&self) -> Option<&GitIntegration> {
        self.git_integration.as_ref()
    }

    /// Create a checkpoint for the current state
    pub async fn create_checkpoint(&mut self, description: &str) -> Result<String> {
        if let (Some(checkpoint_manager), Some(code_manager)) =
            (self.checkpoint_manager.as_mut(), self.code_manager.as_ref())
        {
            // Get modified files
            let modified_files = code_manager.get_modified_files().await;

            // Create file changes
            let mut changes = Vec::new();
            for relative_path in modified_files {
                // Get original content (before modification) and current content
                let file_path = self
                    .state
                    .lock()
                    .unwrap()
                    .workspace_path
                    .join(&relative_path);
                if file_path.exists() {
                    match tokio::fs::read_to_string(&file_path).await {
                        Ok(current_content) => {
                            // For simplicity, we're using empty string as before content
                            // In a real implementation, you'd want to track the original content
                            let change = checkpoint_manager.create_file_change(
                                &relative_path,
                                "", // Original content not available in this simplified version
                                &current_content,
                            );
                            changes.push(change);
                        }
                        Err(e) => {
                            warn!("Failed to read file {}: {}", relative_path, e);
                        }
                    }
                }
            }

            // Create the checkpoint
            checkpoint_manager
                .create_checkpoint(description, changes)
                .await
        } else {
            Err(anyhow::anyhow!(
                "Checkpoint manager or code manager not initialized"
            ))
        }
    }
}
