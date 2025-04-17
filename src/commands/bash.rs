use anyhow::{Context, Result};
use std::process::Command;
use tracing::{debug, info};

use crate::core::state::SharedState;

/// Execute a bash command
pub async fn execute_command(state: &SharedState, command: &str) -> Result<String> {
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
