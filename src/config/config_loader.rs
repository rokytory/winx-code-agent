use crate::plugins::manager::PluginConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Enum representing transport types
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    Stdio,
    Ssh,
    Wss,
}

/// Main configuration structure for winx-code-agent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WinxConfig {
    /// Path to the storage directory
    pub storage_path: Option<PathBuf>,

    /// Default workspace path
    pub default_workspace: Option<PathBuf>,

    /// Plugin configurations
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,

    /// Transport type for MCP communication
    #[serde(default = "default_transport")]
    pub transport: TransportType,

    /// Transport-specific settings
    #[serde(default)]
    pub transport_config: HashMap<String, String>,

    /// Environment variables to pass to tools
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// Whether to enable reinforcement learning
    #[serde(default)]
    pub enable_reinforcement_learning: bool,

    /// Debug mode
    #[serde(default)]
    pub debug: bool,
}

fn default_transport() -> TransportType {
    TransportType::Stdio
}

/// The configuration loader
pub struct ConfigLoader {
    config_path: PathBuf,
}

impl ConfigLoader {
    /// Create a new config loader
    pub fn new() -> Self {
        let config_path = Self::default_config_path();
        Self { config_path }
    }

    /// Set a custom config path
    pub fn with_path(path: PathBuf) -> Self {
        Self { config_path: path }
    }

    /// Get the default config path
    fn default_config_path() -> PathBuf {
        #[cfg(target_os = "macos")]
        let base_path = dirs::home_dir()
            .map(|home| home.join("Library/Application Support/Claude/claude_desktop_config.json"))
            .unwrap_or_else(|| PathBuf::from("claude_desktop_config.json"));

        #[cfg(target_os = "windows")]
        let base_path = dirs::config_dir()
            .map(|config| config.join("Claude/claude_desktop_config.json"))
            .unwrap_or_else(|| PathBuf::from("claude_desktop_config.json"));

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let base_path = dirs::config_dir()
            .map(|config| config.join("claude/claude_desktop_config.json"))
            .unwrap_or_else(|| PathBuf::from("claude_desktop_config.json"));

        base_path
    }

    /// Load the configuration
    pub fn load(&self) -> Result<WinxConfig> {
        // Try to load from the config file
        if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path)
                .with_context(|| format!("Failed to read config file: {:?}", self.config_path))?;

            // Parse the config file as JSON
            let json_value: serde_json::Value = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {:?}", self.config_path))?;

            // Extract winx section from mcpServers
            if let Some(mcp_servers) = json_value.get("mcpServers").and_then(|v| v.as_object()) {
                if let Some(winx_config) = mcp_servers.get("winx") {
                    let config: WinxConfig = serde_json::from_value(winx_config.clone())
                        .with_context(|| "Failed to parse winx configuration")?;
                    return Ok(config);
                }
            }
        }

        // Return default config if file doesn't exist or parsing fails
        Ok(self.default_config())
    }

    /// Get the default configuration
    fn default_config(&self) -> WinxConfig {
        WinxConfig {
            storage_path: dirs::data_local_dir().map(|d| d.join("winx-code-agent")),
            default_workspace: dirs::home_dir(),
            plugins: HashMap::new(),
            transport: TransportType::Stdio,
            transport_config: HashMap::new(),
            environment: HashMap::new(),
            enable_reinforcement_learning: false,
            debug: false,
        }
    }

    /// Save the configuration
    pub fn save(&self, config: &WinxConfig) -> Result<()> {
        // Ensure the parent directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        // If the file exists, read and update it
        let mut full_config = if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path)
                .with_context(|| format!("Failed to read config file: {:?}", self.config_path))?;
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        // Update the winx section in mcpServers
        if !full_config.is_object() {
            full_config = serde_json::json!({});
        }

        let mcp_servers = full_config
            .as_object_mut()
            .unwrap()
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}));

        if !mcp_servers.is_object() {
            *mcp_servers = serde_json::json!({});
        }

        mcp_servers
            .as_object_mut()
            .unwrap()
            .insert("winx".to_string(), serde_json::to_value(config)?);

        // Write the config file
        let content = serde_json::to_string_pretty(&full_config)?;
        fs::write(&self.config_path, content)
            .with_context(|| format!("Failed to write config file: {:?}", self.config_path))?;

        Ok(())
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}
