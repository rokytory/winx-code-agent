use crate::error::{WinxError, WinxResult};
use rmcp::{model::CallToolResult, schemars, tool, Error as McpError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::bash::state::BashState;
use crate::file::repository::RepositoryExplorer;

// Global shared state instances
lazy_static::lazy_static! {
    static ref REPO_EXPLORER: Arc<Mutex<RepositoryExplorer>> =
        Arc::new(Mutex::new(RepositoryExplorer::new()));
    static ref BASH_STATES: Arc<Mutex<HashMap<String, BashState>>> =
        Arc::new(Mutex::new(HashMap::new()));
    static ref CURRENT_MODE: Arc<Mutex<Mode>> =
        Arc::new(Mutex::new(Mode::Wcgw));
    static ref WORKSPACE_PATH: Arc<Mutex<PathBuf>> =
        Arc::new(Mutex::new(std::path::PathBuf::from(".")));
    static ref INITIALIZATION_STATUS: Arc<std::sync::atomic::AtomicBool> =
        Arc::new(std::sync::atomic::AtomicBool::new(false));
}

// Mode enum for different operational modes
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Wcgw,                         // Full permissions mode
    Architect,                    // Read-only mode for planning
    CodeWriter(CodeWriterConfig), // Restricted permissions for code editing
}

// Actions that can be performed
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    ReadFile,
    WriteFile,
    EditFile,
    ExecuteCommand,
    ReadImage,
    SaveContext,
}

#[derive(Debug, Clone)]
pub struct Initialize {
    // Using global state
}

impl Initialize {
    pub fn new() -> Self {
        Self {}
    }

    /// Check if the agent has been initialized
    pub fn was_initialized() -> bool {
        INITIALIZATION_STATUS.load(std::sync::atomic::Ordering::SeqCst)
    }

    // Get repository explorer
    fn get_repo_explorer(&self) -> WinxResult<Arc<Mutex<RepositoryExplorer>>> {
        Ok(Arc::clone(&REPO_EXPLORER))
    }

    // Get or create bash state for a specific mode
    fn get_bash_state(&self, mode_name: &str) -> WinxResult<Arc<Mutex<BashState>>> {
        let mut states = BASH_STATES.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire BASH_STATES lock: {}", e))
        })?;

        if !states.contains_key(mode_name) {
            states.insert(mode_name.to_string(), BashState::new());
        }

        let state = states.get(mode_name).unwrap().clone();
        Ok(Arc::new(Mutex::new(state)))
    }

    // Set the current mode
    fn set_mode(&self, mode: Mode) -> WinxResult<()> {
        let mut current_mode = CURRENT_MODE.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire CURRENT_MODE lock: {}", e))
        })?;

        *current_mode = mode;
        Ok(())
    }

    // Get the current mode
    pub fn get_current_mode() -> WinxResult<Mode> {
        let current_mode = CURRENT_MODE.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire CURRENT_MODE lock: {}", e))
        })?;

        Ok(current_mode.clone())
    }

    // Obter o caminho do workspace atual
    pub fn get_workspace_path() -> WinxResult<PathBuf> {
        let workspace_path = WORKSPACE_PATH.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire WORKSPACE_PATH lock: {}", e))
        })?;

        Ok(workspace_path.clone())
    }

    // Atualizar o caminho do workspace
    fn update_workspace_path(&self, path: PathBuf) -> WinxResult<()> {
        let mut workspace_path = WORKSPACE_PATH.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire WORKSPACE_PATH lock: {}", e))
        })?;

        *workspace_path = path;
        Ok(())
    }

    // Check if an action is allowed in the current mode
    /// Check if a directory has write permissions
    pub fn check_directory_writable(dir_path: &Path) -> bool {
        // Log the directory being checked for debugging
        log::debug!(
            "Checking write permission for directory: {}",
            dir_path.display()
        );

        // First check if directory exists
        if !dir_path.exists() {
            log::warn!("Directory {} doesn't exist", dir_path.display());
            return false;
        }

        // Try to create a temporary file to test write access
        let temp_file_name = format!(".winx-test-{}", uuid::Uuid::new_v4());
        let temp_file_path = dir_path.join(&temp_file_name);
        let write_result = std::fs::File::create(&temp_file_path);

        if let Ok(file) = write_result {
            // Successfully created file, clean up and return true
            drop(file); // Close the file
            let remove_result = std::fs::remove_file(&temp_file_path);
            if let Err(e) = remove_result {
                log::warn!(
                    "Failed to clean up test file {}: {}",
                    temp_file_path.display(),
                    e
                );
                // Continue even if cleanup fails
            }
            log::debug!("Directory {} is writable", dir_path.display());
            true
        } else {
            // Failed to create file, directory is not writable
            if let Some(err) = write_result.err() {
                log::warn!(
                    "Directory {} is not writable: {} ({})",
                    dir_path.display(),
                    err,
                    err.kind()
                );

                // Log more detailed information about specific error types
                match err.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        log::warn!(
                            "Permission denied error when writing to {}",
                            dir_path.display()
                        );
                    }
                    std::io::ErrorKind::ReadOnlyFilesystem => {
                        log::warn!("Read-only filesystem detected at {}", dir_path.display());
                    }
                    _ => {}
                }
            }
            false
        }
    }

    pub fn check_permission(action: Action, path: Option<&str>) -> WinxResult<()> {
        let current_mode = CURRENT_MODE.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire CURRENT_MODE lock: {}", e))
        })?;

        match *current_mode {
            Mode::Wcgw => {
                // All actions are allowed in wcgw mode
                Ok(())
            }
            Mode::Architect => {
                // Only read operations are allowed in architect mode
                match action {
                    Action::ReadFile | Action::ReadImage => Ok(()),
                    _ => Err(WinxError::permission_error(format!(
                        "Action {:?} is not allowed in architect mode",
                        action
                    ))),
                }
            }
            Mode::CodeWriter(ref config) => {
                match action {
                    Action::ReadFile | Action::ReadImage => {
                        // Reading is always allowed
                        Ok(())
                    }
                    Action::WriteFile | Action::EditFile => {
                        // Check if the path matches allowed globs
                        if let Some(file_path) = path {
                            if config.allowed_globs.contains(&"all".to_string()) {
                                return Ok(());
                            }

                            // Check if any glob matches
                            for glob_pattern in &config.allowed_globs {
                                if let Ok(glob) = glob::Pattern::new(glob_pattern) {
                                    if glob.matches(file_path) {
                                        return Ok(());
                                    }
                                }
                            }

                            // No matching glob found
                            Err(WinxError::permission_error(
                                format!("File path {} does not match any allowed glob pattern in code-writer mode", file_path)
                            ))
                        } else {
                            Err(WinxError::invalid_argument(
                                "No file path provided for write/edit action",
                            ))
                        }
                    }
                    Action::ExecuteCommand => {
                        // Check if commands are allowed
                        if config.allowed_commands.contains(&"all".to_string()) {
                            Ok(())
                        } else {
                            // For simplicity, we're not checking specific commands here
                            // In a real implementation, you would check if the specific command is allowed
                            Err(WinxError::permission_error(
                                "Command execution restricted in code-writer mode",
                            ))
                        }
                    }
                    Action::SaveContext => {
                        // Context saving is always allowed
                        Ok(())
                    }
                }
            }
        }
    }
}

impl Default for Initialize {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct InitializeParams {
    #[schemars(description = "Type of initialization")]
    pub r#type: String,

    #[schemars(description = "Workspace path")]
    pub any_workspace_path: String,

    #[schemars(description = "Files to read initially")]
    pub initial_files_to_read: Vec<String>,

    #[schemars(description = "Task ID to resume")]
    pub task_id_to_resume: String,

    #[schemars(description = "Mode name")]
    pub mode_name: String,

    #[schemars(description = "Code writer configuration")]
    pub code_writer_config: Option<serde_json::Value>,
}

// CodeWriter mode configuration
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CodeWriterConfig {
    #[schemars(description = "Allowed file globs")]
    pub allowed_globs: Vec<String>,

    #[schemars(description = "Allowed commands")]
    pub allowed_commands: Vec<String>,
}

#[tool(tool_box)]
impl Initialize {
    #[tool(description = "Initialize a workspace and environment")]
    pub async fn initialize(
        &self,
        #[tool(aggr)] params: InitializeParams,
    ) -> Result<CallToolResult, McpError> {
        // Wrap the implementation in a try block to use our custom error handling
        let result = self.initialize_impl(params).await;

        // Convert any WinxError to McpError
        match result {
            Ok(result) => Ok(result),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    // Implementation with custom error handling
    async fn initialize_impl(&self, params: InitializeParams) -> WinxResult<CallToolResult> {
        // If any_workspace_path is empty, try to use the WINX_WORKSPACE environment variable
        let workspace_path = if params.any_workspace_path.trim().is_empty() {
            if let Ok(env_workspace) = std::env::var("WINX_WORKSPACE") {
                log::info!(
                    "Using WINX_WORKSPACE environment variable: {}",
                    env_workspace
                );
                PathBuf::from(env_workspace)
            } else {
                PathBuf::from(&params.any_workspace_path)
            }
        } else {
            PathBuf::from(&params.any_workspace_path)
        };

        // Create directory if it doesn't exist and isn't empty
        if !workspace_path.exists() && params.any_workspace_path.trim() != "" {
            // Expand any home directory symbol (~) in the path
            let expanded_path = if params.any_workspace_path.starts_with("~/")
                || params.any_workspace_path == "~"
            {
                if let Some(home_dir) = dirs::home_dir() {
                    if params.any_workspace_path == "~" {
                        home_dir
                    } else {
                        home_dir.join(params.any_workspace_path.strip_prefix("~/").unwrap())
                    }
                } else {
                    workspace_path.clone()
                }
            } else {
                workspace_path.clone()
            };

            // Try to create the directory but don't fail initialization if it fails
            let create_result = std::fs::create_dir_all(&expanded_path);
            if let Err(ref e) = create_result {
                log::warn!(
                    "Failed to create directory '{}': {} ({})",
                    expanded_path.display(),
                    e,
                    e.kind()
                );
                // Continue the initialization process with a known good directory
                // Rather than failing completely when directory creation fails
                if e.kind() == std::io::ErrorKind::PermissionDenied
                    || e.kind() == std::io::ErrorKind::Other
                    || e.kind() == std::io::ErrorKind::ReadOnlyFilesystem
                {
                    // Covers "Read-only file system"
                    // Log warning with more details
                    log::warn!(
                        "Using fallback directory instead of requested path '{}' due to permission issue: {} ({})",
                        expanded_path.display(), e, e.kind()
                    );

                    // Fallback to /tmp directory as it's usually writable
                    let tmp_dir = PathBuf::from("/tmp");
                    if tmp_dir.exists() && Self::check_directory_writable(&tmp_dir) {
                        // Create a unique project-specific subdirectory in /tmp
                        let unique_id = uuid::Uuid::new_v4().to_string();
                        let tmp_project_dir = tmp_dir.join(format!("winx-workspace-{}", unique_id));
                        match std::fs::create_dir_all(&tmp_project_dir) {
                            Ok(_) => {
                                log::info!(
                                    "Created temporary workspace at {}",
                                    tmp_project_dir.display()
                                );

                                // Obter novamente o estado bash para atualizar
                                // This avoids the variable not found error
                                let bash_state_tmp = self.get_bash_state(&params.mode_name)?;
                                let mut state_tmp = bash_state_tmp.lock().map_err(|e2| {
                                    WinxError::lock_error(format!(
                                        "Failed to acquire bash state lock: {}",
                                        e2
                                    ))
                                })?;

                                // Update the workspace path
                                state_tmp.update_cwd(tmp_project_dir.clone());
                                state_tmp.set_workspace_root(tmp_project_dir.clone());
                                self.update_workspace_path(tmp_project_dir.clone())?;

                                return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                                    format!("Warning: Failed to create or access directory '{}': {} ({}). Using temporary workspace at '{}' instead.\n\nPlease specify a different workspace path with write permissions.",
                                            workspace_path.display(), e, e.kind(), tmp_project_dir.display())
                                )]));
                            }
                            Err(e2) => {
                                log::warn!(
                                    "Failed to create temp directory at {}: {} ({})",
                                    tmp_project_dir.display(),
                                    e2,
                                    e2.kind()
                                );

                                // Try a second temporary location if the first one fails
                                let user_tmp = std::env::var("TMPDIR")
                                    .ok()
                                    .map(PathBuf::from)
                                    .filter(|p| p.exists() && Self::check_directory_writable(p));

                                if let Some(alt_tmp) = user_tmp {
                                    let alt_tmp_dir =
                                        alt_tmp.join(format!("winx-workspace-{}", unique_id));
                                    if std::fs::create_dir_all(&alt_tmp_dir).is_ok() {
                                        log::info!(
                                            "Created alternative temporary workspace at {}",
                                            alt_tmp_dir.display()
                                        );

                                        let bash_state_alt =
                                            self.get_bash_state(&params.mode_name)?;
                                        let mut state_alt =
                                            bash_state_alt.lock().map_err(|e3| {
                                                WinxError::lock_error(format!(
                                                    "Failed to acquire bash state lock: {}",
                                                    e3
                                                ))
                                            })?;

                                        state_alt.update_cwd(alt_tmp_dir.clone());
                                        state_alt.set_workspace_root(alt_tmp_dir.clone());
                                        self.update_workspace_path(alt_tmp_dir.clone())?;

                                        return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                                            format!("Warning: Failed to create or access directory '{}': {}. Using alternative temporary workspace at '{}' instead.\n\nPlease specify a different workspace path with write permissions.",
                                                    workspace_path.display(), e, alt_tmp_dir.display())
                                        )]));
                                    }
                                }
                            }
                        }
                    }

                    // If tmp directory isn't available, fallback to current directory or home
                    let fallback_dir = std::env::current_dir()
                        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));

                    // Use the fallback directory for the rest of the function
                    if fallback_dir.exists() {
                        // Obter novamente o estado bash para atualizar
                        // This avoids the variable not found error
                        let bash_state_fb = self.get_bash_state(&params.mode_name)?;
                        let mut state_fb = bash_state_fb.lock().map_err(|e2| {
                            WinxError::lock_error(format!(
                                "Failed to acquire bash state lock: {}",
                                e2
                            ))
                        })?;

                        // Update the workspace path
                        state_fb.update_cwd(fallback_dir.clone());
                        state_fb.set_workspace_root(fallback_dir.clone());
                        self.update_workspace_path(fallback_dir.clone())?;

                        return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                            format!("Warning: Failed to create or access directory '{}': {}. Using '{}' instead.\n\nPlease specify a different workspace path with write permissions.",
                                    workspace_path.display(), e, fallback_dir.display())
                        )]));
                    }
                }
            }
        }

        // Get system info
        let system_info = format!(
            "System: {}, Arch: {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );

        // Get bash state for this mode
        let bash_state = self.get_bash_state(&params.mode_name)?;
        let mut state = bash_state.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire bash state lock: {}", e))
        })?;

        // Update paths in bash state and global workspace path
        if workspace_path.exists() {
            state.update_cwd(workspace_path.clone());
            state.set_workspace_root(workspace_path.clone());
            self.update_workspace_path(workspace_path.clone())?;
        } else {
            // Try to use the current directory as fallback
            match std::env::current_dir() {
                Ok(current_dir) => {
                    state.update_cwd(current_dir.clone());
                    state.set_workspace_root(current_dir.clone());
                    self.update_workspace_path(current_dir.clone())?;
                    log::warn!(
                        "Workspace path '{}' doesn't exist, using current directory: '{}'",
                        params.any_workspace_path,
                        current_dir.display()
                    );
                }
                Err(e) => {
                    log::error!("Failed to get current directory: {}", e);
                    // Keep the existing settings
                }
            }
        }

        // Set mode in bash state
        state.set_mode(params.mode_name.clone());

        // Get repository explorer
        let repo_explorer = self.get_repo_explorer()?;
        let explorer = repo_explorer.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire repo explorer lock: {}", e))
        })?;

        // Check if the directory is writable and add a warning if not
        let is_writable =
            workspace_path.exists() && Self::check_directory_writable(&workspace_path);

        // Analyze workspace structure
        let repo_context = if workspace_path.exists() {
            let writable_status = if !is_writable {
                "\n\n⚠️ WARNING: The workspace directory appears to be read-only. File creation and editing operations may fail.\n"
            } else {
                ""
            };

            match explorer.explore_workspace(&workspace_path) {
                Ok(context) => format!("# Workspace structure\n{}{}", context, writable_status),
                Err(e) => format!("Error analyzing workspace: {}{}", e, writable_status),
            }
        } else {
            format!(
                "Workspace path doesn't exist: {}",
                params.any_workspace_path
            )
        };

        // Get recent files if any
        let recent_files = if workspace_path.exists() {
            match explorer.get_recent_files(&workspace_path, 10) {
                Ok(files) => {
                    let paths: Vec<String> =
                        files.iter().map(|p| format!("- {}", p.display())).collect();
                    if !paths.is_empty() {
                        format!("\n# Recent files\n{}", paths.join("\n"))
                    } else {
                        String::new()
                    }
                }
                Err(_) => String::new(),
            }
        } else {
            String::new()
        };

        // Handle task resumption
        let memory = if !params.task_id_to_resume.is_empty() {
            // Try to load the saved task context
            let app_dir = dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("winx-code-agent")
                .join("memory");

            let task_file = app_dir.join(format!("{}.txt", params.task_id_to_resume));

            if task_file.exists() {
                match fs::read_to_string(&task_file) {
                    Ok(content) => {
                        // Extract project root if available
                        let mut project_root = String::new();
                        for line in content.lines() {
                            if line.starts_with("# PROJECT ROOT = ") {
                                project_root = line
                                    .strip_prefix("# PROJECT ROOT = ")
                                    .unwrap_or("")
                                    .to_string();
                                break;
                            }
                        }

                        // If found a project root and it exists, update the workspace
                        if !project_root.is_empty() {
                            let project_path = PathBuf::from(&project_root);
                            if project_path.exists() && project_path.is_dir() {
                                // Update bash state with the project path
                                state.update_cwd(project_path.clone());
                                state.set_workspace_root(project_path);
                            }
                        }

                        format!(
                            "Resuming task: {}\n\nTask context loaded. Project root: {}\n\n{}",
                            params.task_id_to_resume,
                            project_root,
                            content.lines().take(10).collect::<Vec<_>>().join("\n")
                        )
                    }
                    Err(e) => format!("Failed to load task {}: {}", params.task_id_to_resume, e),
                }
            } else {
                format!("Task {} not found", params.task_id_to_resume)
            }
        } else {
            "No task to resume".to_string()
        };

        // Read initial files if requested
        let initial_files_content = if !params.initial_files_to_read.is_empty() {
            let mut content = String::new();
            content.push_str("\n# Initial files\n");

            for file_path in &params.initial_files_to_read {
                let path = if file_path.starts_with("/") {
                    PathBuf::from(file_path)
                } else {
                    workspace_path.join(file_path)
                };

                if path.exists() && path.is_file() {
                    match fs::read_to_string(&path) {
                        Ok(file_content) => {
                            content.push_str(&format!(
                                "\n## {}\n```\n{}\n```\n",
                                path.display(),
                                file_content
                            ));
                        }
                        Err(e) => {
                            content.push_str(&format!(
                                "\n## {}\nError reading file: {}\n",
                                path.display(),
                                e
                            ));
                        }
                    }
                } else {
                    content.push_str(&format!("\n## {}\nFile does not exist\n", path.display()));
                }
            }

            content
        } else {
            String::new()
        };

        // Parse and set the current mode
        let current_mode = match params.mode_name.as_str() {
            "wcgw" => Mode::Wcgw,
            "architect" => Mode::Architect,
            "code_writer" => {
                if let Some(config_json) = &params.code_writer_config {
                    if let Ok(config) =
                        serde_json::from_value::<CodeWriterConfig>(config_json.clone())
                    {
                        Mode::CodeWriter(config)
                    } else {
                        // Default code writer config if parsing fails
                        Mode::CodeWriter(CodeWriterConfig {
                            allowed_globs: vec!["all".to_string()],
                            allowed_commands: vec!["all".to_string()],
                        })
                    }
                } else {
                    // Default code writer config if none provided
                    Mode::CodeWriter(CodeWriterConfig {
                        allowed_globs: vec!["all".to_string()],
                        allowed_commands: vec!["all".to_string()],
                    })
                }
            }
            _ => Mode::Wcgw, // Default to wcgw mode
        };

        // Set the current mode in the global state
        self.set_mode(current_mode.clone())?;

        // Generate mode-specific info
        let mode_info = match &current_mode {
            Mode::Wcgw => "\n# Mode: wcgw\nAll operations are allowed.".to_string(),
            Mode::Architect => "\n# Mode: architect\nOnly read operations are allowed. This mode is designed for planning and understanding code.".to_string(),
            Mode::CodeWriter(config) => {
                format!(
                    "\n# Mode: code_writer\nAllowed globs: {:?}\nAllowed commands: {:?}\nRestricted to specified paths and commands.",
                    config.allowed_globs, config.allowed_commands
                )
            }
        };

        // Set initialization status to true
        INITIALIZATION_STATUS.store(true, std::sync::atomic::Ordering::SeqCst);

        // Build the result
        let result = format!(
            "Initialized with mode: {}\n{}\n{}{}{}{}{}\n{}",
            params.mode_name,
            repo_context,
            recent_files,
            initial_files_content,
            mode_info,
            memory,
            "\n\n---\n\nAdditional instructions:\n    Always run `pwd` if you get any file or directory not found error to make sure you're not lost, or to get absolute cwd.\n\n    Always write production ready, syntactically correct code.\n",
            system_info
        );

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            result,
        )]))
    }
}
