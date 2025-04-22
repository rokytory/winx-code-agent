use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::WinxResult;
use crate::plugins::manager::PluginConfig;
use crate::security::{Role, SecurityManager};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WinxConfig {
    pub plugins: Vec<PluginConfig>,
    pub rl_system: RLConfig,
    pub semantic: SemanticConfig,
    pub security: SecurityConfig,
    pub transport: TransportConfig,
    pub telemetry: TelemetryConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RLConfig {
    pub enabled: bool,
    pub learning_rate: f64,
    pub discount_factor: f64,
    pub exploration_rate: f64,
    pub exploration_decay: f64,
    pub min_exploration_rate: f64,
    pub experience_replay_size: usize,
    pub batch_size: usize,
}

impl Default for RLConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            learning_rate: 0.1,
            discount_factor: 0.9,
            exploration_rate: 0.2,
            exploration_decay: 0.995,
            min_exploration_rate: 0.05,
            experience_replay_size: 1000,
            batch_size: 32,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SemanticConfig {
    pub enabled: bool,
    pub catalog: String,
    pub schema: String,
    pub cache_duration_secs: u64,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            catalog: "default".to_string(),
            schema: "public".to_string(),
            cache_duration_secs: 300,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub verify_signatures: bool,
    pub sandboxed: bool,
    pub allowed_paths: Vec<PathBuf>,
    pub allowed_hosts: Vec<String>,
    pub roles: Vec<Role>,
    pub default_role: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            verify_signatures: true,
            sandboxed: false,
            allowed_paths: vec![],
            allowed_hosts: vec![],
            roles: vec![
                SecurityManager::create_admin_role(),
                SecurityManager::create_readonly_role(),
            ],
            default_role: "default".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransportConfig {
    pub transport_type: TransportType,
    pub sse_port: u16,
    pub websocket_port: u16,
    pub http_port: u16,
    pub timeout_secs: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    Stdio,
    SSE,
    WebSocket,
    HTTP,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            transport_type: TransportType::Stdio,
            sse_port: 8080,
            websocket_port: 8081,
            http_port: 8082,
            timeout_secs: 30,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub batch_size: usize,
    pub flush_interval_secs: u64,
    pub include_errors: bool,
    pub include_performance: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            batch_size: 100,
            flush_interval_secs: 60,
            include_errors: true,
            include_performance: true,
        }
    }
}

impl WinxConfig {
    pub fn load(path: &Path) -> WinxResult<Self> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let content = std::fs::read_to_string(path)?;

        match ext {
            "json" => Ok(serde_json::from_str(&content)?),
            "yaml" | "yml" => Ok(serde_yaml::from_str(&content)?),
            "toml" => Ok(toml::from_str(&content)?),
            _ => Err(crate::error::WinxError::invalid_argument(format!(
                "Unsupported config format: {}",
                ext
            ))),
        }
    }

    pub fn save(&self, path: &Path) -> WinxResult<()> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let content = match ext {
            "json" => serde_json::to_string_pretty(self)?,
            "yaml" | "yml" => serde_yaml::to_string(self)?,
            "toml" => toml::to_string(self)?,
            _ => {
                return Err(crate::error::WinxError::invalid_argument(format!(
                    "Unsupported config format: {}",
                    ext
                )))
            }
        };

        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("winx-code-agent")
            .join("config.json")
    }

    pub fn apply(&self) -> WinxResult<()> {
        // TODO: Apply configuration to various subsystems
        Ok(())
    }
}
