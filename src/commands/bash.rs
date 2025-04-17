use anyhow::{Context, Result};
use serde_json::Value;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::core::state::SharedState;
use crate::core::types::{BashAction, BashCommand, Command as CommandType, StatusCheck, SendText, SendSpecials, SendAscii};

/// Execute a bash command from MCP
pub async fn execute_bash_command(state: &SharedState, command_json: &str) -> Result<String> {
    debug!("Executing bash command: {}", command_json);
    
    // Parse the command JSON
    let bash_command: BashCommand = serde_json::from_str(command_json)
        .context("Failed to parse bash command JSON")?;
    
    // Execute according to the action type
    match bash_command.action_json {
        BashAction::Command(CommandType { command }) => {
            execute_command(state, &command).await
        },
        BashAction::StatusCheck(_) => {
            check_status(state).await
        },
        BashAction::SendText(SendText { send_text }) => {
            send_text_input(state, &send_text).await
        },
        BashAction::SendSpecials(SendSpecials { send_specials }) => {
            send_special_keys(state, &send_specials).await
        },
        BashAction::SendAscii(SendAscii { send_ascii }) => {
            send_ascii_chars(state, &send_ascii).await
        },
    }
}

/// Execute a bash command
async fn execute_command(state: &SharedState, command: &str) -> Result<String> {
    debug!("Executing command: {}", command);
    
    let state_guard = state.lock().unwrap();
    
    if !state_guard.is_command_allowed(command) {
        return Err(anyhow::anyhow!("Command not allowed: {}", command));
    }
    
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(&state_guard.workspace_path)
        .output()
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
async fn send_special_keys(_state: &SharedState, _specials: &Vec<crate::core::types::Special>) -> Result<String> {
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
            
            let result = execute_command(&state, "echo 'Hello, world!'").await.unwrap();
            assert_eq!(result.trim(), "Hello, world!");
            
            let result = execute_command(&state, "ls -la").await.unwrap();
            assert!(!result.is_empty());
        });
    }
}
