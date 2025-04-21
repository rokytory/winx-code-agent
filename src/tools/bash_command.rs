use crate::error::{WinxError, WinxResult};
use rmcp::{model::CallToolResult, schemars, tool, Error as McpError};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::time::Duration;

use crate::bash::runner::{CommandRunner, ProcessStatus};
use crate::bash::security::{check_command_safety, DangerLevel};
use crate::tools::initialize::{Action, Initialize};

// Global command runner instance
lazy_static::lazy_static! {
    static ref COMMAND_RUNNER: Arc<Mutex<Option<CommandRunner>>> = Arc::new(Mutex::new(None));
}

#[derive(Debug, Clone)]
pub struct BashCommand {
    // Empty struct as state is managed globally
}

impl BashCommand {
    pub fn new() -> Self {
        Self {}
    }

    /// Handle complex commands that need special processing
    fn handle_complex_command(&self, command_json: &str) -> Result<ActionJson, McpError> {
        log::debug!("Handling complex command: {}", command_json);

        // Try to extract the command part
        if let Some(cmd_start) = command_json.find("command") {
            if let Some(cmd_content_start) = command_json[cmd_start..].find(':') {
                let cmd_start_pos = cmd_start + cmd_content_start + 1;
                let cmd_content = &command_json[cmd_start_pos..];

                // Extract the actual command string
                let cmd_str = if let Some(cmd_end) = cmd_content.find(',') {
                    cmd_content[..cmd_end].trim()
                } else if let Some(cmd_end) = cmd_content.find('}') {
                    cmd_content[..cmd_end].trim()
                } else {
                    cmd_content.trim()
                };

                // Clean up quotes
                let cmd_cleaned = cmd_str
                    .trim_start_matches('"')
                    .trim_start_matches('\'')
                    .trim_end_matches('"')
                    .trim_end_matches('\'');

                log::debug!("Extracted command: {}", cmd_cleaned);

                // For complex commands with redirections or quotes, use a temporary file approach
                if cmd_cleaned.contains(">") || cmd_cleaned.contains("'") {
                    log::info!("Converting complex command to use a temporary script file");

                    // Create a safer command that writes to a temporary script
                    let temp_script = format!("/tmp/winx-cmd-{}.sh", uuid::Uuid::new_v4());
                    let command_request = CommandRequest {
                        command: format!(
                            "cat > {} << 'WINX_SCRIPT_EOF'\n{}\nWINX_SCRIPT_EOF\nchmod +x {}\n{}",
                            temp_script, cmd_cleaned, temp_script, temp_script
                        ),
                    };

                    return Ok(ActionJson::Command(command_request));
                }

                // If we can't handle it specially, just try to use the command directly
                return Ok(ActionJson::Command(CommandRequest {
                    command: cmd_cleaned.to_string(),
                }));
            }
        }

        Err(
            WinxError::parse_error(format!("Could not parse complex command: {}", command_json))
                .to_mcp_error(),
        )
    }

    /// Check if a command requires terminal access
    fn command_requires_terminal(&self, command: &str) -> bool {
        // List of known interactive commands
        let interactive_commands = [
            "vim", "vi", "nano", "emacs", "less", "more", "top", "htop", "screen -", "tmux",
            "lynx", "mc", "ssh", "telnet",
        ];

        for &cmd in &interactive_commands {
            // Check for command at beginning of line or after pipe/semicolon
            if command.contains(&format!(" {} ", cmd))
                || command.starts_with(cmd)
                || command.contains(&format!("; {}", cmd))
                || command.contains(&format!("| {}", cmd))
            {
                return true;
            }
        }

        false
    }

    // Initialize the command runner if not already initialized
    fn ensure_initialized(&self) -> WinxResult<()> {
        let mut runner = COMMAND_RUNNER.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire lock for command runner: {}", e))
        })?;

        if runner.is_none() {
            // Get the workspace path from initializer
            let workspace_path = match crate::tools::initialize::Initialize::get_workspace_path() {
                Ok(path) => {
                    if path.exists() {
                        path.to_string_lossy().to_string()
                    } else {
                        log::warn!("Workspace path doesn't exist, using current directory");
                        std::env::current_dir()
                            .unwrap_or_else(|_| std::path::PathBuf::from("."))
                            .to_string_lossy()
                            .to_string()
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Failed to get workspace path: {}, using current directory",
                        e
                    );
                    std::env::current_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        .to_string_lossy()
                        .to_string()
                }
            };

            let mut cmd_runner = CommandRunner::new(&workspace_path);
            if let Err(e) = cmd_runner.start_shell() {
                log::error!("Failed to start shell in '{}': {}", workspace_path, e);
                // Try again with home directory as fallback
                let home_dir = dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .to_string_lossy()
                    .to_string();
                log::warn!("Falling back to home directory: {}", home_dir);
                cmd_runner = CommandRunner::new(&home_dir);
                cmd_runner.start_shell()?;
            }
            *runner = Some(cmd_runner);
            log::info!("Command runner initialized successfully");
        }

        Ok(())
    }

    // Get the command runner
    fn get_runner(&self) -> WinxResult<CommandRunner> {
        self.ensure_initialized()?;

        let runner = COMMAND_RUNNER.lock().map_err(|e| {
            WinxError::lock_error(format!("Failed to acquire lock for command runner: {}", e))
        })?;

        if let Some(ref runner) = *runner {
            // Clone the CommandRunner
            Ok(runner.clone())
        } else {
            Err(WinxError::ShellNotStarted)
        }
    }
}

impl Default for BashCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CommandRequest {
    #[schemars(description = "Command to execute")]
    pub command: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct StatusCheckRequest {
    #[schemars(description = "Check status of running command")]
    pub status_check: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SendTextRequest {
    #[schemars(description = "Text to send")]
    pub send_text: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SendSpecialsRequest {
    #[schemars(description = "Special keys to send")]
    pub send_specials: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum ActionJson {
    Command(CommandRequest),
    StatusCheck(StatusCheckRequest),
    SendText(SendTextRequest),
    SendSpecials(SendSpecialsRequest),
    SendAscii { send_ascii: Vec<i32> },
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct BashCommandParams {
    #[schemars(description = "Action to perform")]
    pub action_json: serde_json::Value, // Accept any JSON value

    #[schemars(description = "Wait for seconds before returning")]
    pub wait_for_seconds: Option<f64>,
}

#[tool(tool_box)]
impl BashCommand {
    #[tool(description = "Execute a bash command or interact with running processes")]
    pub async fn bash_command(
        &self,
        #[tool(aggr)] params: BashCommandParams,
    ) -> Result<CallToolResult, McpError> {
        // Check if initialization has been done
        crate::ensure_initialized!("You must call 'initialize' before executing bash commands.");

        // Log para fins de diagnóstico
        log::info!("BashCommand: Executing command with params: {:?}", params);

        // Check permission
        Initialize::check_permission(Action::ExecuteCommand, None).map_err(|e| {
            log::error!("Permission check failed: {:?}", e);
            e.to_mcp_error()
        })?;

        self.ensure_initialized().map_err(|e| {
            log::error!("Shell initialization failed: {:?}", e);
            e.to_mcp_error()
        })?;

        let runner = match self.get_runner() {
            Ok(r) => r,
            Err(e) => {
                log::error!("Failed to get command runner: {:?}", e);
                return Err(e.to_mcp_error());
            }
        };

        let timeout = params.wait_for_seconds.unwrap_or(5.0);

        // Parse the action_json with improved error handling
        let action_json = if let Ok(json_str) = serde_json::to_string(&params.action_json) {
            // First try: If it's a string (Claude often sends JSON as string), try to parse it
            if json_str.starts_with('"') && json_str.ends_with('"') {
                let inner_json = json_str
                    .trim_start_matches('"')
                    .trim_end_matches('"')
                    .replace("\\\"", "\"");

                // Check for common escaping issues with shell commands
                let sanitized_json = if inner_json.contains("\\n") || inner_json.contains("\\\\") {
                    log::debug!("Sanitizing JSON with escaped newlines and backslashes");
                    // For commands with newlines and double escapes, try to carefully handle them
                    inner_json.replace("\\n", "\n").replace("\\\\", "\\")
                } else {
                    inner_json
                };

                match serde_json::from_str::<ActionJson>(&sanitized_json) {
                    Ok(action) => action,
                    Err(e1) => {
                        log::warn!(
                            "Failed to parse inner JSON: {}, trying direct deserialization",
                            e1
                        );
                        // Try direct deserialization as a fallback
                        match serde_json::from_value::<ActionJson>(params.action_json.clone()) {
                            Ok(action) => action,
                            Err(e2) => {
                                // Handle common errors with complex shell commands
                                if sanitized_json.contains("command")
                                    && (sanitized_json.contains(">")
                                        || sanitized_json.contains("echo")
                                            && sanitized_json.contains("'"))
                                {
                                    // Special case for echo with complex output redirection
                                    log::info!("Detected complex shell command, attempting to fix");
                                    self.handle_complex_command(&sanitized_json)?
                                } else {
                                    let error = WinxError::parse_error(format!(
                                        "Invalid action_json format: {}. Original: {}",
                                        e2, json_str
                                    ));
                                    return Err(error.to_mcp_error());
                                }
                            }
                        }
                    }
                }
            } else {
                // Try direct deserialization
                match serde_json::from_value::<ActionJson>(params.action_json.clone()) {
                    Ok(action) => action,
                    Err(e) => {
                        let error = WinxError::parse_error(format!(
                            "Invalid action_json format: {}. Original: {}",
                            e, json_str
                        ));
                        return Err(error.to_mcp_error());
                    }
                }
            }
        } else {
            let error = WinxError::invalid_argument(
                "Invalid action_json format: Could not serialize to string",
            );
            return Err(error.to_mcp_error());
        };

        let result = match action_json {
            ActionJson::Command(cmd) => {
                // Check command safety first
                match check_command_safety(&cmd.command) {
                    DangerLevel::Dangerous(reason) => {
                        log::warn!("Dangerous command rejected: {}", cmd.command);
                        return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                            format!("Error: Command refused for security reasons.\n\nReason: {}\n\nPlease use a safer alternative.", reason)
                        )]));
                    },
                    DangerLevel::Warning(reason) => {
                        log::warn!("Suspicious command allowed with warning: {}", cmd.command);
                        let warning = format!(
                            "Warning: {}\n\nProceeding with caution.\n\n",
                            reason
                        );
                        
                        // Execute command but add warning to output
                        runner
                            .execute(&cmd.command)
                            .await
                            .map_err(|e| e.to_mcp_error())?;

                        // Wait a bit to collect output
                        tokio::time::sleep(Duration::from_secs_f64(timeout)).await;

                        // Get output
                        let (stdout, stderr) = runner.get_output();
                        let status_info = runner.get_status_info();

                        // Add warning to the output
                        format!("{}{}\n{}\n\n{}", warning, stdout, stderr, status_info)
                    },
                    DangerLevel::Safe => {
                        // Verify if command needs terminal access before executing
                        if self.command_requires_terminal(&cmd.command) {
                            let warning = format!(
                                "Warning: Command '{}' may require an interactive terminal and might not work correctly.\n\n",
                                cmd.command
                            );
                            log::warn!("Command requires terminal: {}", cmd.command);

                            // Execute command but add warning to output
                            runner
                                .execute(&cmd.command)
                                .await
                                .map_err(|e| e.to_mcp_error())?;

                            // Wait a bit to collect output
                            tokio::time::sleep(Duration::from_secs_f64(timeout)).await;

                            // Get output
                            let (stdout, stderr) = runner.get_output();
                            let status_info = runner.get_status_info();

                            // Add warning to the output
                            format!("{}{}\n{}\n\n{}", warning, stdout, stderr, status_info)
                        } else {
                    // Regular command execution
                    runner
                        .execute(&cmd.command)
                        .await
                        .map_err(|e| e.to_mcp_error())?;

                    // Wait a bit to collect output
                    tokio::time::sleep(Duration::from_secs_f64(timeout)).await;

                    // Get output
                    let (stdout, stderr) = runner.get_output();
                    let status_info = runner.get_status_info();

                    // Primeiro, vamos registrar a saída para fins de diagnóstico
                    log::debug!(
                        "Command output - stdout: {:?}, stderr: {:?}",
                        stdout,
                        stderr
                    );

                    // Check if output is empty and add a note
                    if stdout.trim().is_empty() && stderr.trim().is_empty() {
                        // Log detalhado do comando executado e seu resultado
                        log::info!(
                            "Command '{}' produced no output - checking PWD",
                            cmd.command
                        );

                        // Execute pwd to verify current directory
                        runner.execute("pwd").await.map_err(|e| e.to_mcp_error())?;
                        tokio::time::sleep(Duration::from_secs_f64(1.0)).await;
                        let (pwd_out, _) = runner.get_output();

                        log::info!("Current directory: {}", pwd_out.trim());

                        // Additional attempt to execute common commands directly
                        if cmd.command.contains("ls")
                            || cmd.command.contains("find")
                            || cmd.command.contains("cat")
                        {
                            // Tente executar o comando diretamente para verificação
                            log::info!("Attempting direct execution for: {}", cmd.command);
                            let parts: Vec<&str> = cmd.command.split_whitespace().collect();
                            if !parts.is_empty() {
                                let cmd_name = parts[0];
                                let args = &parts[1..];

                                let output = std::process::Command::new(cmd_name)
                                    .args(args)
                                    .current_dir(pwd_out.trim())
                                    .output();

                                match output {
                                    Ok(output) => {
                                        let cmd_output = String::from_utf8_lossy(&output.stdout);
                                        let cmd_error = String::from_utf8_lossy(&output.stderr);
                                        log::info!("Direct command stdout: {}", cmd_output);
                                        log::info!("Direct command stderr: {}", cmd_error);

                                        if !cmd_output.is_empty() {
                                            format!("{}\n\n{}", cmd_output, status_info)
                                        } else if !cmd_error.is_empty() {
                                            format!("{}\n\n{}", cmd_error, status_info)
                                        } else {
                                            format!("Command executed successfully but produced no output. Current directory: {}\n\n{}", 
                                                pwd_out.trim(), status_info)
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Direct command execution failed: {}", e);
                                        format!("Command execution attempt: {}. Current directory: {}\n\n{}", 
                                            e, pwd_out.trim(), status_info)
                                    }
                                }
                            } else {
                                format!("Command executed successfully but produced no output. Current directory: {}\n\n{}", 
                                    pwd_out.trim(), status_info)
                            }
                        } else if cmd.command.contains("echo") {
                            // If it's an echo command, show what's being echoed
                            let echo_text = cmd.command.trim_start_matches("echo").trim();
                            format!(
                                "{}\n\n{}",
                                echo_text.trim_matches('\'').trim_matches('"'),
                                status_info
                            )
                        } else {
                            format!("Command executed successfully.\n\n{}", status_info)
                        }
                    } else {
                        format!("{}\n{}\n\n{}", stdout, stderr, status_info)
                    }
                }
            }
            ActionJson::StatusCheck(_) => {
                // Check status
                let status = runner.check_status(timeout).await;

                let status_str = match status {
                    ProcessStatus::Running => "status = still running".to_string(),
                    ProcessStatus::Exited(code) => {
                        format!("status = process exited with code {}", code)
                    }
                    ProcessStatus::NotRunning => {
                        "No running command to check status of".to_string()
                    }
                };

                let cwd = runner.get_cwd();
                format!("{}\ncwd = {}", status_str, cwd)
            }
            ActionJson::SendText(text) => {
                // Send text to process
                {
                    runner
                        .send_text(&text.send_text)
                        .await
                        .map_err(|e| e.to_mcp_error())?;
                }

                // Wait a bit to collect output
                tokio::time::sleep(Duration::from_secs_f64(timeout)).await;

                // Get output
                let (stdout, stderr) = runner.get_output();
                let status_info = runner.get_status_info();

                format!("{}\n{}\n\n{}", stdout, stderr, status_info)
            }
            ActionJson::SendSpecials(specials) => {
                // Enhanced special key handling
                let mut special_keys_handled = Vec::new();

                for special in &specials.send_specials {
                    match special.as_str() {
                        "Ctrl-c" => {
                            runner
                                .send_interrupt()
                                .await
                                .map_err(|e| e.to_mcp_error())?;
                            special_keys_handled.push("Ctrl-c");
                        }
                        "Enter" => {
                            runner.send_text("\n").await.map_err(|e| e.to_mcp_error())?;
                            special_keys_handled.push("Enter");
                        }
                        "Key-up" => {
                            runner
                                .send_text("\x1b[A")
                                .await
                                .map_err(|e| e.to_mcp_error())?;
                            special_keys_handled.push("↑");
                        }
                        "Key-down" => {
                            runner
                                .send_text("\x1b[B")
                                .await
                                .map_err(|e| e.to_mcp_error())?;
                            special_keys_handled.push("↓");
                        }
                        "Key-left" => {
                            runner
                                .send_text("\x1b[D")
                                .await
                                .map_err(|e| e.to_mcp_error())?;
                            special_keys_handled.push("←");
                        }
                        "Key-right" => {
                            runner
                                .send_text("\x1b[C")
                                .await
                                .map_err(|e| e.to_mcp_error())?;
                            special_keys_handled.push("→");
                        }
                        "Ctrl-d" => {
                            runner
                                .send_text("\x04")
                                .await
                                .map_err(|e| e.to_mcp_error())?;
                            special_keys_handled.push("Ctrl-d");
                        }
                        _ => {
                            let error = WinxError::invalid_argument(format!(
                                "Unsupported special key: {}. Supported keys: Ctrl-c, Ctrl-d, Enter, Key-up, Key-down, Key-left, Key-right",
                                special
                            ));
                            return Err(error.to_mcp_error());
                        }
                    }
                }

                // Wait a bit to collect output
                tokio::time::sleep(Duration::from_secs_f64(timeout)).await;

                // Get output
                let (stdout, stderr) = runner.get_output();
                let status_info = runner.get_status_info();

                // Include the keys that were sent in the output
                let keys_sent = if !special_keys_handled.is_empty() {
                    format!("Sent keys: {}\n\n", special_keys_handled.join(", "))
                } else {
                    "".to_string()
                };

                format!("{}{}\n{}\n\n{}", keys_sent, stdout, stderr, status_info)
            }
            ActionJson::SendAscii { send_ascii } => {
                for ascii in send_ascii {
                    let ch = char::from_u32(ascii as u32).unwrap_or(' ');
                    {
                        runner
                            .send_text(&ch.to_string())
                            .await
                            .map_err(|e| e.to_mcp_error())?;
                    }
                }

                // Wait a bit to collect output
                tokio::time::sleep(Duration::from_secs_f64(timeout)).await;

                // Get output
                let (stdout, stderr) = runner.get_output();
                let status_info = runner.get_status_info();

                format!("{}\n{}\n\n{}", stdout, stderr, status_info)
            }
        };

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            result,
        )]))
    }
}
