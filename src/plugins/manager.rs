use anyhow::Result;
use rmcp::model::{CallToolResult, ErrorCode, Tool};
use rmcp::Error as McpError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Plugin configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    pub name: String,
    pub path: String,
    pub runtime_config: Option<RuntimeConfig>,
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
}

#[derive(Clone)]
struct PluginMetadata {
    #[allow(dead_code)]
    config: PluginConfig,
    tools: Vec<Tool>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            tool_plugin_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a plugin with the manager
    pub async fn register_plugin(&self, config: PluginConfig, tools: Vec<Tool>) -> Result<()> {
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

    /// Implementation placeholder for calling a tool
    /// This will be implemented when we add the actual plugin execution system
    pub async fn call_tool(
        &self,
        _tool_name: &str,
        _params: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        // This is just a placeholder - we'll implement actual plugin execution later
        Err(McpError::new(
            ErrorCode::INTERNAL_ERROR,
            "Plugin system not fully implemented yet".to_string(),
            Some(json!({"status": "not_implemented"})),
        ))
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
