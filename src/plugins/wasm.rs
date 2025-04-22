use extism::{Manifest, Plugin, Wasm};
use rmcp::model::{CallToolResult, Tool};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::manager::PluginConfig;
use crate::error::{WinxError, WinxResult};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OciConfig {
    pub reference: String,
    pub registry: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub verify_signature: bool,
}

pub struct WasmPlugin {
    plugin: Plugin,
    #[allow(dead_code)]
    config: PluginConfig,
    #[allow(dead_code)]
    tools: Vec<Tool>,
}

#[derive(Clone)]
pub struct WasmPluginManager {
    plugins: Arc<RwLock<HashMap<String, WasmPlugin>>>,
    tool_plugin_map: Arc<RwLock<HashMap<String, String>>>,
    #[allow(dead_code)]
    cache_dir: PathBuf,
    verify_signatures: bool,
}

impl WasmPluginManager {
    pub fn new(cache_dir: PathBuf, verify_signatures: bool) -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            tool_plugin_map: Arc::new(RwLock::new(HashMap::new())),
            cache_dir,
            verify_signatures,
        }
    }

    pub async fn load_plugin(&self, config: &PluginConfig) -> WinxResult<()> {
        let manifest = self.create_manifest(config)?;
        let mut plugin = Plugin::new(&manifest, [], true)
            .map_err(|e| WinxError::other(format!("Failed to create plugin: {}", e)))?;

        let tools = self.extract_tools(&mut plugin)?;

        // Register tool names
        let mut tool_map = self.tool_plugin_map.write().await;
        for tool in &tools {
            let tool_name = tool.name.to_string();
            if let Some(existing_plugin) = tool_map.get(&tool_name) {
                if existing_plugin != &config.name {
                    return Err(WinxError::other(format!(
                        "Tool name collision: '{}' is provided by both '{}' and '{}'",
                        tool_name, existing_plugin, config.name
                    )));
                }
            }
            tool_map.insert(tool_name, config.name.clone());
        }

        let wasm_plugin = WasmPlugin {
            plugin,
            config: config.clone(),
            tools,
        };

        let mut plugins = self.plugins.write().await;
        plugins.insert(config.name.clone(), wasm_plugin);

        Ok(())
    }

    pub async fn load_from_oci(&self, config: &PluginConfig) -> WinxResult<Vec<u8>> {
        if let Some(_oci_ref) = &config.oci_reference {
            // TODO: Implement OCI registry pulling
            // This would involve:
            // 1. Parsing the OCI reference
            // 2. Authenticating with the registry
            // 3. Pulling the image layers
            // 4. Extracting the WASM module
            todo!("OCI registry support not yet implemented")
        } else {
            Err(WinxError::invalid_argument("No OCI reference provided"))
        }
    }

    fn create_manifest(&self, config: &PluginConfig) -> WinxResult<Manifest> {
        let wasm = if config.path.starts_with("oci://") {
            // TODO: Support OCI references
            todo!("OCI references not yet supported")
        } else {
            Wasm::file(&config.path)
        };

        let mut manifest = Manifest::new([wasm]);

        // Apply runtime configuration
        if let Some(runtime_config) = &config.runtime_config {
            if let Some(allowed_hosts) = &runtime_config.allowed_hosts {
                manifest.allowed_hosts = Some(allowed_hosts.clone());
            }

            if let Some(memory_limit) = runtime_config.memory_limit {
                // Configure memory limit
                manifest = manifest.with_config_key("memory", memory_limit.to_string());
            }

            if let Some(timeout_ms) = runtime_config.timeout_ms {
                manifest.timeout_ms = Some(timeout_ms);
            }
        }

        Ok(manifest)
    }

    fn extract_tools(&self, plugin: &mut Plugin) -> WinxResult<Vec<Tool>> {
        // Call the plugin to get its tool definitions
        let output = plugin
            .call::<(), String>("list_tools", ())
            .map_err(|e| WinxError::other(format!("Failed to call list_tools: {}", e)))?;

        let tools: Vec<Tool> = serde_json::from_str(&output)
            .map_err(|e| WinxError::other(format!("Failed to parse tools: {}", e)))?;

        Ok(tools)
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        params: serde_json::Value,
    ) -> WinxResult<CallToolResult> {
        let tool_map = self.tool_plugin_map.read().await;
        let plugin_name = tool_map
            .get(tool_name)
            .ok_or_else(|| WinxError::other(format!("Tool '{}' not found", tool_name)))?;

        let mut plugins = self.plugins.write().await;
        let wasm_plugin = plugins
            .get_mut(plugin_name)
            .ok_or_else(|| WinxError::other(format!("Plugin '{}' not found", plugin_name)))?;

        let input = serde_json::json!({
            "tool": tool_name,
            "params": params,
        });

        let output = wasm_plugin
            .plugin
            .call::<_, String>("call_tool", serde_json::to_vec(&input)?)
            .map_err(|e| WinxError::other(format!("Failed to call tool: {}", e)))?;

        let result: CallToolResult = serde_json::from_str(&output)
            .map_err(|e| WinxError::other(format!("Failed to parse tool result: {}", e)))?;

        Ok(result)
    }

    pub async fn verify_plugin_signature(&self, _path: &str) -> WinxResult<bool> {
        if !self.verify_signatures {
            return Ok(true);
        }

        // TODO: Implement signature verification with sigstore
        todo!("Signature verification not yet implemented")
    }
}
