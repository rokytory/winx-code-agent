// Module to ensure compatibility between different versions of the MCP protocol

use log::{debug, error, info, warn};
use std::env;
use std::path::PathBuf;

/// Detects and returns the workspace path based on available configurations
pub fn detect_workspace() -> PathBuf {
    // Priority 1: WINX_WORKSPACE environment variable
    if let Ok(workspace) = env::var("WINX_WORKSPACE") {
        info!("Using workspace from WINX_WORKSPACE env var: {}", workspace);
        return PathBuf::from(workspace);
    }

    // Priority 2: Claude configuration
    if let Some(claude_workspace) = get_claude_config_workspace() {
        info!(
            "Using workspace from Claude config: {}",
            claude_workspace.display()
        );
        return claude_workspace;
    }

    // Priority 3: Current directory
    if let Ok(current_dir) = std::env::current_dir() {
        info!(
            "Using current directory as workspace: {}",
            current_dir.display()
        );
        return current_dir;
    }

    // Fallback: User's home directory
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    warn!(
        "No workspace configured, using home directory: {}",
        home_dir.display()
    );

    home_dir
}

/// Attempts to get workspace configuration from Claude's configuration file
fn get_claude_config_workspace() -> Option<PathBuf> {
    let config_path = dirs::home_dir()
        .map(|home| home.join("Library/Application Support/Claude/claude_desktop_config.json"))?;

    if !config_path.exists() {
        debug!("Claude config file not found at {}", config_path.display());
        return None;
    }

    debug!("Found Claude config at {}", config_path.display());

    // Tries to read the configuration file
    match std::fs::read_to_string(&config_path) {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(config) => {
                    // Tries to extract workspace from winx configuration
                    if let Some(env) = config
                        .get("mcpServers")
                        .and_then(|servers| servers.get("winx"))
                        .and_then(|winx| winx.get("env"))
                        .and_then(|env| env.get("WINX_WORKSPACE"))
                        .and_then(|workspace| workspace.as_str())
                    {
                        debug!("Found WINX_WORKSPACE in Claude config: {}", env);
                        return Some(PathBuf::from(env));
                    }

                    debug!("No WINX_WORKSPACE found in Claude config");
                    None
                }
                Err(e) => {
                    error!("Failed to parse Claude config: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            error!("Failed to read Claude config: {}", e);
            None
        }
    }
}

/// Detects preferred log level or uses default
pub fn detect_log_level() -> String {
    env::var("RUST_LOG").unwrap_or_else(|_| {
        // Tries to get from Claude's configuration file
        match dirs::home_dir()
            .map(|home| home.join("Library/Application Support/Claude/claude_desktop_config.json"))
        {
            Some(config_path) if config_path.exists() => {
                match std::fs::read_to_string(&config_path) {
                    Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(config) => {
                            if let Some(log_level) = config
                                .get("mcpServers")
                                .and_then(|servers| servers.get("winx"))
                                .and_then(|winx| winx.get("env"))
                                .and_then(|env| env.get("RUST_LOG"))
                                .and_then(|level| level.as_str())
                            {
                                return log_level.to_string();
                            }
                        }
                        Err(_) => {}
                    },
                    Err(_) => {}
                }
            }
            _ => {}
        }

        // Valor padrão
        "info".to_string()
    })
}
