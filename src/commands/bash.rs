use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::core::state::SharedState;
use crate::core::types::{
    BashAction, BashCommand, Command as CommandType, SendAscii, SendSpecials, SendText,
};

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

/// Execute a bash command
pub async fn execute_command(state: &SharedState, command: &str) -> Result<String> {
    debug!("Executing command: {}", command);

    // Check permission and get workspace path
    let workspace_path = {
        let state_guard = state.lock().unwrap();

        if !state_guard.is_command_allowed(command) {
            return Err(anyhow::anyhow!("Command not allowed: {}", command));
        }

        state_guard.workspace_path.clone()
    };

    // Execute the command with the workspace path
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(workspace_path)
        .output()
        .await
        .context("Failed to execute command")?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if !output.status.success() {
        debug!("Command failed with status: {}", output.status);
        debug!("Stderr: {}", stderr);
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

    info!("Command execution completed");
    Ok(result)
}

/// Check status of previous command
async fn check_status(_state: &SharedState) -> Result<String> {
    // In a real implementation, we would check the status of any running command
    // This is a simplified implementation
    Ok("No command currently running.".to_string())
}

/// Send text input to a running process
async fn send_text_input(_state: &SharedState, _text: &str) -> Result<String> {
    // In a real implementation, we would send the text to the stdin of a running process
    // This is a simplified implementation
    warn!("Text input not implemented yet");
    Ok("Text input sent".to_string())
}

/// Send special keys to a running process
async fn send_special_keys(
    _state: &SharedState,
    _specials: &Vec<crate::core::types::Special>,
) -> Result<String> {
    // In a real implementation, we would send special key codes to a running process
    // This is a simplified implementation
    warn!("Special keys input not implemented yet");
    Ok("Special keys sent".to_string())
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
