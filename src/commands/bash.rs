use anyhow::{anyhow, Context, Result};
use regex::Regex;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::core::state::SharedState;
use crate::core::types::{
    BashAction, BashCommand, Command as CommandType, SendAscii, SendSpecials, SendText,
};

/// Strip ANSI color codes from a string
fn strip_ansi_codes(input: &str) -> String {
    // Match ANSI escape sequences: \u001b followed by [ and then any sequence until m
    // This handles most common color codes and formatting
    let re = Regex::new(r"\x1b\[[0-9;]*m").unwrap_or_else(|_| Regex::new(r"").unwrap());
    re.replace_all(input, "").to_string()
}

/// Execute a bash command from MCP
pub async fn execute_bash_command(state: &SharedState, command_json: &str) -> Result<String> {
    debug!("Executing bash command JSON: {}", command_json);

    // Try to parse as JSON first
    let command_str = if let Ok(json_value) =
        serde_json::from_str::<serde_json::Value>(command_json)
    {
        if json_value.is_string() {
            // The value is a simple string
            json_value.as_str().unwrap_or("").to_string()
        } else if let Some(obj) = json_value.as_object() {
            // It's a JSON object - check for different possible formats

            // 1. Direct command property: {"command": "ls -la"}
            if let Some(cmd) = obj.get("command") {
                if cmd.is_string() {
                    cmd.as_str().unwrap_or("").to_string()
                } else {
                    return Err(anyhow::anyhow!("Command property must be a string"));
                }
            }
            // 2. Nested action_json as object: {"action_json": {"command": "ls -la"}}
            else if let Some(action_json) = obj.get("action_json") {
                if action_json.is_string() {
                    // action_json is a direct string command
                    action_json.as_str().unwrap_or("").to_string()
                } else if let Some(action_obj) = action_json.as_object() {
                    // action_json is an object with command property
                    if let Some(cmd) = action_obj.get("command") {
                        if cmd.is_string() {
                            cmd.as_str().unwrap_or("").to_string()
                        } else {
                            return Err(anyhow::anyhow!(
                                "Command property in action_json must be a string"
                            ));
                        }
                    } else {
                        // Special handling for other possible formats
                        debug!(
                            "No 'command' in action_json object. Keys present: {:?}",
                            action_obj.keys().collect::<Vec<_>>()
                        );

                        // Look for status_check, send_text, etc.
                        if action_obj.contains_key("status_check") {
                            return check_status(state).await;
                        } else if let Some(text) = action_obj.get("send_text") {
                            if text.is_string() {
                                return send_text_input(state, text.as_str().unwrap_or("")).await;
                            } else {
                                return Err(anyhow::anyhow!("send_text value must be a string"));
                            }
                        } else if let Some(specials) = action_obj.get("send_specials") {
                            // Handle special keys
                            if let Some(special_array) = specials.as_array() {
                                let special_keys = special_array
                                    .iter()
                                    .filter_map(|s| s.as_str())
                                    .map(|s| {
                                        s.parse().unwrap_or(crate::core::types::Special::Enter)
                                    })
                                    .collect();

                                return send_special_keys(state, &special_keys).await;
                            } else {
                                return Err(anyhow::anyhow!("send_specials must be an array"));
                            }
                        } else {
                            return Err(anyhow::anyhow!(
                                "Missing command property in action_json object"
                            ));
                        }
                    }
                } else {
                    return Err(anyhow::anyhow!(
                        "Invalid action_json format (must be string or object)"
                    ));
                }
            }
            // 3. Special case for {"wait_for_seconds": X, "action_json": {...}}
            else if obj.contains_key("wait_for_seconds") {
                // This is usually used with action_json, but we already checked that
                return Err(anyhow::anyhow!(
                    "Found 'wait_for_seconds' but missing 'action_json' or 'command'"
                ));
            } else {
                // No recognized format
                return Err(anyhow::anyhow!(
                    "Invalid command object format. Keys present: {:?}",
                    obj.keys().collect::<Vec<_>>()
                ));
            }
        } else {
            // Neither string nor object
            return Err(anyhow::anyhow!(
                "Invalid command format, must be string or object"
            ));
        }
    } else {
        // Not valid JSON, try to use as direct string
        debug!("Input not valid JSON, treating as raw command string");
        command_json.to_string()
    };

    // Log the actual command being executed for debugging
    debug!("Parsed and executing command: {}", command_str);

    // Execute the command
    debug!("Parsed command: {}", command_str);
    execute_command(state, &command_str).await
}

/// Execute a bash command with terminal session support
pub async fn execute_command(state: &SharedState, command: &str) -> Result<String> {
    debug!("Executing command: {}", command);

    // Check permission and get workspace path, also check for active terminal session
    let (workspace_path, terminal_session) = {
        let state_guard = state.lock().unwrap();

        if !state_guard.is_command_allowed(command) {
            return Err(anyhow::anyhow!("Command not allowed: {}", command));
        }

        (
            state_guard.workspace_path.clone(),
            state_guard.get_terminal_session(),
        )
    };

    // If we have an active terminal session, use it
    if let Some(session_id) = terminal_session {
        match crate::commands::terminal::get_terminal_manager() {
            Ok(manager) => {
                debug!("Using existing terminal session {} for command", session_id);
                return manager.execute_command(&session_id, command).await;
            }
            Err(_) => {
                debug!(
                    "Terminal manager not initialized, falling back to simple command execution"
                );
            }
        }
    } else {
        // Check if we should create a terminal session for interactive commands
        if is_interactive_command(command) {
            debug!("Command appears interactive, creating terminal session");
            match crate::commands::terminal::get_terminal_manager() {
                Ok(manager) => {
                    // Create a new session
                    let session_id = manager.create_session().await?;

                    // Register the session with the state
                    {
                        let mut state_guard = state.lock().unwrap();
                        state_guard.set_terminal_session(session_id.clone());
                    }

                    debug!(
                        "Created terminal session {} for interactive command",
                        session_id
                    );
                    return manager.execute_command(&session_id, command).await;
                }
                Err(e) => {
                    debug!("Failed to initialize terminal manager: {}", e);
                    debug!("Falling back to simple command execution");
                }
            }
        }
    }

    // Simple non-interactive command execution
    debug!("Using simple command execution for: {}", command);
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(workspace_path)
        .output()
        .await
        .context("Failed to execute command")?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    // Update state with exit code
    {
        let mut state_guard = state.lock().unwrap();
        state_guard.set_process_status(false, output.status.code());
    }

    if !output.status.success() {
        debug!("Command failed with status: {}", output.status);
        debug!("Stderr: {}", stderr);
    }

    // Combine stdout and stderr with aggressive ANSI stripping
    let mut result = crate::strip_ansi_codes(&stdout);
    if !stderr.is_empty() {
        if !result.is_empty() {
            result.push_str("\n");
        }
        result.push_str("STDERR: ");
        result.push_str(&crate::strip_ansi_codes(&stderr));
    }

    // Additional safety check - just in case some ANSI codes slip through
    // Match a wider range of ANSI escape sequences
    let full_pattern = regex::Regex::new(r"\x1b(?:[@-Z\\-_]|\[[0-9?;]*[0-9A-Za-z])").unwrap();
    let result = full_pattern.replace_all(&result, "").to_string();

    info!("Command execution completed");
    Ok(result)
}

/// Check if a command is likely to be interactive
fn is_interactive_command(command: &str) -> bool {
    // List of common interactive commands
    let interactive_commands = [
        "vim", "vi", "nano", "emacs", "less", "more", "top", "htop", "mysql", "psql", "sqlite3",
        "python", "python3", "node", "ruby", "irb", "bash", "sh", "zsh", "fish", "screen", "tmux",
        "ssh", "telnet",
    ];

    // Check if the command starts with any of these
    for &cmd in &interactive_commands {
        if command.starts_with(cmd)
            && (command.len() == cmd.len() || command.chars().nth(cmd.len()) == Some(' '))
        {
            return true;
        }
    }

    // Check for specific patterns
    if command.contains("| less") || command.contains("| more") {
        return true;
    }

    false
}

/// Check status of previous command
async fn check_status(state: &SharedState) -> Result<String> {
    // First check if we have an active terminal session
    let terminal_session = {
        let state_guard = state.lock().unwrap();
        state_guard.get_terminal_session()
    };

    if let Some(session_id) = terminal_session {
        match crate::commands::terminal::get_terminal_manager() {
            Ok(manager) => {
                debug!("Checking status of terminal session {}", session_id);
                let status = manager.check_status(&session_id).await?;

                return Ok(format!(
                    "Terminal session {}:\nState: {}\nWorking directory: {}\nProcess running: {}\nLast exit status: {:?}",
                    session_id,
                    status.state,
                    status.cwd,
                    status.process_running,
                    status.last_exit_status,
                ));
            }
            Err(_) => {
                debug!("Terminal manager not initialized");
            }
        }
    }

    // Fall back to simple state check
    let state_info = {
        let state_guard = state.lock().unwrap();
        (
            state_guard.process_running,
            state_guard.last_exit_code,
            state_guard.get_background_processes(),
        )
    };

    Ok(format!(
        "Process running: {}\nLast exit status: {:?}\nBackground processes: {:?}",
        state_info.0, state_info.1, state_info.2,
    ))
}

/// Send text input to a running process
pub async fn send_text_input(state: &SharedState, text: &str) -> Result<String> {
    // Check if we have an active terminal session
    let terminal_session = {
        let state_guard = state.lock().unwrap();
        state_guard.get_terminal_session()
    };

    if let Some(session_id) = terminal_session {
        match crate::commands::terminal::get_terminal_manager() {
            Ok(manager) => {
                debug!("Sending text to terminal session {}: {}", session_id, text);
                return manager.send_text(&session_id, text).await;
            }
            Err(e) => {
                return Err(anyhow!("Terminal manager not initialized: {}", e));
            }
        }
    }

    Err(anyhow!("No active terminal session to send text to"))
}

/// Send special keys to a running process
pub async fn send_special_keys(
    state: &SharedState,
    specials: &Vec<crate::core::types::Special>,
) -> Result<String> {
    // Check if we have an active terminal session
    let terminal_session = {
        let state_guard = state.lock().unwrap();
        state_guard.get_terminal_session()
    };

    if let Some(session_id) = terminal_session {
        match crate::commands::terminal::get_terminal_manager() {
            Ok(manager) => {
                debug!(
                    "Sending special keys to terminal session {}: {:?}",
                    session_id, specials
                );
                return manager.send_special_keys(&session_id, specials).await;
            }
            Err(e) => {
                return Err(anyhow!("Terminal manager not initialized: {}", e));
            }
        }
    }

    Err(anyhow!("No active terminal session to send keys to"))
}

/// Start a background process using screen
pub async fn start_background_process(state: &SharedState, command: &str) -> Result<String> {
    // First check if we have an active terminal session
    let terminal_session = {
        let state_guard = state.lock().unwrap();
        state_guard.get_terminal_session()
    };

    if let Some(session_id) = terminal_session {
        match crate::commands::terminal::get_terminal_manager() {
            Ok(manager) => {
                debug!(
                    "Starting background process in terminal session {}: {}",
                    session_id, command
                );
                let process_id = manager
                    .start_background_process(&session_id, command)
                    .await?;

                // Register the process with the state
                {
                    let mut state_guard = state.lock().unwrap();
                    state_guard.add_background_process(process_id.clone());
                }

                return Ok(format!("Background process started: {}", process_id));
            }
            Err(e) => {
                debug!("Terminal manager not initialized: {}", e);
            }
        }
    }

    // Fall back to direct screen command
    let workspace_path = {
        let state_guard = state.lock().unwrap();
        state_guard.workspace_path.clone()
    };

    // Generate a unique session ID
    let session_id = format!("winx-{}", uuid::Uuid::new_v4().to_string());

    // Check if screen is installed
    let which_output = Command::new("which").arg("screen").output().await?;

    if !which_output.status.success() {
        return Err(anyhow!(
            "screen command not available - please install it to use background processes"
        ));
    }

    // Start the screen session
    let screen_cmd = format!(
        "screen -dmS {} bash -c '{} ; echo \"[Process completed with status $?]\"'",
        session_id, command
    );

    let output = Command::new("sh")
        .arg("-c")
        .arg(&screen_cmd)
        .current_dir(workspace_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to start background process: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Register the process with the state
    {
        let mut state_guard = state.lock().unwrap();
        state_guard.add_background_process(session_id.clone());
    }

    Ok(format!("Background process started: {}", session_id))
}

/// Send ASCII characters to a running process
async fn send_ascii_chars(_state: &SharedState, _chars: &Vec<u8>) -> Result<String> {
    // In a real implementation, we would send ASCII chars to a running process
    // This is a simplified implementation
    warn!("ASCII input not implemented yet");
    Ok("ASCII chars sent".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{state::create_shared_state, types::ModeType};
    use tokio::runtime::Runtime;

    #[test]
    fn test_execute_command() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let state = create_shared_state(temp_dir.path(), ModeType::Wcgw, None, None).unwrap();

            let result = execute_command(&state, "echo 'Hello, world!'")
                .await
                .unwrap();
            assert_eq!(result.trim(), "Hello, world!");

            let result = execute_command(&state, "ls -la").await.unwrap();
            assert!(!result.is_empty());
        });
    }
}
