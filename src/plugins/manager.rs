use anyhow::Result;
use rmcp::model::{CallToolResult, ErrorCode, Tool};
use rmcp::Error as McpError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::wasm::WasmPluginManager;

// Plugin configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    pub name: String,
    pub path: String,
    pub runtime_config: Option<RuntimeConfig>,
    #[serde(default)]
    pub oci_reference: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub plugin_type: PluginType,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Wasm,
    Native,
    Remote,
}

impl Default for PluginType {
    fn default() -> Self {
        Self::Wasm
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeConfig {
    pub allowed_hosts: Option<Vec<String>>,
    pub allowed_paths: Option<Vec<String>>,
    pub env_vars: Option<HashMap<String, String>>,
    pub memory_limit: Option<usize>,
    pub timeout_ms: Option<u64>,
}

/// Manages the loading and execution of plugins
#[derive(Clone)]
pub struct PluginManager {
    // Maps plugin names to their metadata
    plugins: Arc<RwLock<HashMap<String, PluginMetadata>>>,
    // Maps tool names to their plugin names
    tool_plugin_map: Arc<RwLock<HashMap<String, String>>>,
    // Cache directory for plugin downloads
    #[allow(dead_code)]
    cache_dir: PathBuf,
    // Whether to verify plugin signatures
    verify_signatures: bool,
    // WebAssembly plugin manager
    wasm_manager: WasmPluginManager,
}

#[derive(Clone)]
struct PluginMetadata {
    #[allow(dead_code)]
    config: PluginConfig,
    tools: Vec<Tool>,
}

impl PluginManager {
    pub fn new() -> Self {
        // Use system cache directory or fallback to local .cache
        let cache_dir = dirs::cache_dir()
            .map(|d| d.join("winx-code-agent").join("plugins"))
            .unwrap_or_else(|| PathBuf::from(".cache/plugins"));

        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            tool_plugin_map: Arc::new(RwLock::new(HashMap::new())),
            cache_dir: cache_dir.clone(),
            verify_signatures: true,
            wasm_manager: WasmPluginManager::new(cache_dir, true),
        }
    }

    pub fn with_settings(cache_dir: PathBuf, verify_signatures: bool) -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            tool_plugin_map: Arc::new(RwLock::new(HashMap::new())),
            cache_dir: cache_dir.clone(),
            verify_signatures,
            wasm_manager: WasmPluginManager::new(cache_dir, verify_signatures),
        }
    }

    /// Registers a plugin with the manager
    pub async fn register_plugin(&self, config: PluginConfig, tools: Vec<Tool>) -> Result<()> {
        if !config.enabled {
            return Ok(());
        }

        match config.plugin_type {
            PluginType::Wasm => {
                // Delegate to WASM manager
                self.wasm_manager
                    .load_plugin(&config)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to load WASM plugin: {}", e))?;
            }
            PluginType::Native => {
                // Native plugin registration (traditional approach)
                let plugin_name = config.name.clone();

                // Update tool -> plugin mapping
                let mut tool_map = self.tool_plugin_map.write().await;
                for tool in &tools {
                    // Check for name collisions
                    let tool_name = tool.name.to_string();
                    if let Some(existing_plugin) = tool_map.get(&tool_name) {
                        if existing_plugin != &plugin_name {
                            return Err(anyhow::anyhow!(
                                "Tool name collision: '{}' is provided by both '{}' and '{}'",
                                tool_name,
                                existing_plugin,
                                plugin_name
                            ));
                        }
                    }
                    tool_map.insert(tool_name, plugin_name.clone());
                }

                // Add plugin to registry
                let metadata = PluginMetadata { config, tools };
                let mut plugins = self.plugins.write().await;
                plugins.insert(plugin_name, metadata);
            }
            PluginType::Remote => {
                // TODO: Implement remote plugin registration
                todo!("Remote plugin support not yet implemented");
            }
        }

        Ok(())
    }

    /// Gets all registered tools across all plugins
    pub async fn get_all_tools(&self) -> Vec<Tool> {
        let plugins = self.plugins.read().await;
        let mut tools = Vec::new();

        for metadata in plugins.values() {
            tools.extend(metadata.tools.clone());
        }

        tools
    }

    /// Gets the plugin that provides a specific tool
    pub async fn get_plugin_for_tool(&self, tool_name: &str) -> Option<String> {
        let tool_map = self.tool_plugin_map.read().await;
        tool_map.get(tool_name).cloned()
    }

    /// Calls a tool provided by a plugin
    pub async fn call_tool(
        &self,
        tool_name: &str,
        params: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        // Check if the tool belongs to a plugin
        let tool_map = self.tool_plugin_map.read().await;
        if let Some(plugin_name) = tool_map.get(tool_name) {
            let plugins = self.plugins.read().await;
            if let Some(metadata) = plugins.get(plugin_name) {
                match metadata.config.plugin_type {
                    PluginType::Wasm => {
                        // Delegate to WASM manager
                        self.wasm_manager
                            .call_tool(tool_name, params)
                            .await
                            .map_err(|e| e.to_mcp_error())
                    }
                    PluginType::Native => {
                        // Native plugin calls (not yet implemented)
                        Err(McpError::new(
                            ErrorCode::INTERNAL_ERROR,
                            "Native plugin execution not yet implemented".to_string(),
                            Some(json!({"status": "not_implemented"})),
                        ))
                    }
                    PluginType::Remote => {
                        // Remote plugin calls
                        Err(McpError::new(
                            ErrorCode::INTERNAL_ERROR,
                            "Remote plugin execution not yet implemented".to_string(),
                            Some(json!({"status": "not_implemented"})),
                        ))
                    }
                }
            } else {
                Err(McpError::new(
                    ErrorCode::INVALID_PARAMS,
                    format!("Plugin '{}' not found", plugin_name),
                    None,
                ))
            }
        } else {
            // Tool doesn't belong to a plugin
            Err(McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Tool '{}' not registered", tool_name),
                None,
            ))
        }
    }

    /// Gets whether signature verification is enabled
    pub fn verify_signatures(&self) -> bool {
        self.verify_signatures
    }

    /// Sets whether to verify plugin signatures
    pub fn set_verify_signatures(&mut self, verify: bool) {
        self.verify_signatures = verify;
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
