use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

/// Global configuration for Winx
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinxConfig {
    /// Editor configuration
    pub editor: EditorConfig,
    /// LSP configuration
    pub lsp: LspConfig,
    /// Terminal configuration
    pub terminal: TerminalConfig,
    /// Memory configuration
    pub memory: MemoryConfig,
    /// Plugins configuration
    pub plugins: PluginsConfig,
    /// Mode configurations
    pub modes: ModesConfig,
    /// Network configuration
    pub network: NetworkConfig,
}

/// Editor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    /// Default file encoding
    pub default_encoding: String,
    /// Tab width in spaces
    pub tab_width: usize,
    /// Whether to use spaces for indentation
    pub use_spaces: bool,
    /// Whether to create backup files before editing
    pub backup_files: bool,
    /// Directory for backup files
    pub backup_dir: Option<PathBuf>,
}

/// LSP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Whether to enable caching
    pub enable_cache: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Paths to language servers
    pub language_server_paths: HashMap<String, PathBuf>,
}

/// Terminal configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Size of history buffer
    pub history_size: usize,
    /// Default timeout for commands in seconds
    pub default_timeout_seconds: u64,
    /// Whether to use ANSI colors in terminal output
    pub use_colors: bool,
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Whether to enable memory persistence
    pub persistence_enabled: bool,
    /// Whether to enable contextual memory
    pub context_memory_enabled: bool,
    /// TTL for memories in days
    pub memory_ttl_days: usize,
}

/// Plugins configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// List of enabled plugins
    pub enabled_plugins: Vec<String>,
    /// Directory for plugins
    pub plugin_dir: PathBuf,
}

/// Mode configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModesConfig {
    /// Wcgw mode configuration
    pub wcgw: WcgwModeConfig,
    /// Architect mode configuration
    pub architect: ArchitectModeConfig,
    /// Code writer mode configuration
    pub code_writer: CodeWriterModeConfig,
}

/// Wcgw mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcgwModeConfig {
    /// List of allowed commands (empty means all allowed)
    pub allowed_commands: Vec<String>,
    /// List of restricted paths
    pub restricted_paths: Vec<String>,
    /// Whether to require confirmation for file changes
    pub require_confirmation: bool,
}

/// Architect mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectModeConfig {
    /// Maximum number of files to read
    pub max_read_files: usize,
    /// Whether to enable writing
    pub enable_writing: bool,
}

/// Code writer mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeWriterModeConfig {
    /// Default glob patterns for editable files
    pub default_globs: Vec<String>,
    /// Default allowed commands
    pub default_commands: Vec<String>,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Whether to allow network access
    pub allow_network: bool,
    /// Proxy URL if any
    pub proxy_url: Option<String>,
    /// Timeout for network requests in seconds
    pub timeout_seconds: u64,
}

impl Default for WinxConfig {
    fn default() -> Self {
        Self {
            editor: EditorConfig {
                default_encoding: "utf-8".to_string(),
                tab_width: 4,
                use_spaces: true,
                backup_files: true,
                backup_dir: None,
            },
            lsp: LspConfig {
                timeout_seconds: 30,
                enable_cache: true,
                cache_ttl_seconds: 60,
                language_server_paths: HashMap::new(),
            },
            terminal: TerminalConfig {
                history_size: 1000,
                default_timeout_seconds: 10,
                use_colors: false,
            },
            memory: MemoryConfig {
                persistence_enabled: true,
                context_memory_enabled: true,
                memory_ttl_days: 30,
            },
            plugins: PluginsConfig {
                enabled_plugins: Vec::new(),
                plugin_dir: PathBuf::from("plugins"),
            },
            modes: ModesConfig {
                wcgw: WcgwModeConfig {
                    allowed_commands: Vec::new(), // All allowed
                    restricted_paths: Vec::new(),
                    require_confirmation: false,
                },
                architect: ArchitectModeConfig {
                    max_read_files: 100,
                    enable_writing: false,
                },
                code_writer: CodeWriterModeConfig {
                    default_globs: vec!["src/**/*.rs".to_string()],
                    default_commands: vec!["cargo".to_string(), "rustc".to_string()],
                },
            },
            network: NetworkConfig {
                allow_network: true,
                proxy_url: None,
                timeout_seconds: 30,
            },
        }
    }
}

/// Manager for configuration
pub struct ConfigManager {
    /// Global configuration
    global_config: Option<WinxConfig>,
    /// Project-specific configurations
    project_configs: HashMap<PathBuf, WinxConfig>,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        Self {
            global_config: None,
            project_configs: HashMap::new(),
        }
    }

    /// Load global configuration
    pub fn load_global_config(&mut self) -> Result<()> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
            .join("winx");

        let config_path = config_dir.join("config.toml");

        if config_path.exists() {
            info!(
                "Loading global configuration from {}",
                config_path.display()
            );
            let config_str = fs::read_to_string(&config_path)?;
            let config: WinxConfig = toml::from_str(&config_str)?;
            self.global_config = Some(config);
        } else {
            // Create default configuration
            info!(
                "Creating default global configuration at {}",
                config_path.display()
            );
            let default_config = WinxConfig::default();

            fs::create_dir_all(&config_dir)?;
            fs::write(&config_path, toml::to_string_pretty(&default_config)?)?;

            self.global_config = Some(default_config);
        }

        Ok(())
    }

    /// Load project-specific configuration
    pub fn load_project_config(&mut self, project_path: impl AsRef<Path>) -> Result<()> {
        let project_path = project_path.as_ref();
        let config_path = project_path.join(".winx").join("config.toml");

        if config_path.exists() {
            info!(
                "Loading project configuration from {}",
                config_path.display()
            );
            let config_str = fs::read_to_string(&config_path)?;
            let config: WinxConfig = toml::from_str(&config_str)?;
            self.project_configs
                .insert(project_path.to_path_buf(), config);
        } else if let Some(global_config) = &self.global_config {
            // If project config doesn't exist, use a copy of the global config
            info!(
                "No project configuration found, using global configuration for {}",
                project_path.display()
            );
            self.project_configs
                .insert(project_path.to_path_buf(), global_config.clone());
        }

        Ok(())
    }

    /// Get configuration for a project
    pub fn get_config(&self, project_path: impl AsRef<Path>) -> Result<WinxConfig> {
        let project_path = project_path.as_ref();

        if let Some(config) = self.project_configs.get(project_path) {
            return Ok(config.clone());
        }

        if let Some(global_config) = &self.global_config {
            return Ok(global_config.clone());
        }

        Err(anyhow::anyhow!("No configuration available"))
    }

    /// Save project-specific configuration
    pub fn save_project_config(
        &self,
        project_path: impl AsRef<Path>,
        config: &WinxConfig,
    ) -> Result<()> {
        let project_path = project_path.as_ref();
        let config_dir = project_path.join(".winx");

        fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");
        info!("Saving project configuration to {}", config_path.display());

        fs::write(&config_path, toml::to_string_pretty(config)?)?;

        Ok(())
    }

    /// Create and save a default project configuration
    pub fn create_default_project_config(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<WinxConfig> {
        let project_path = project_path.as_ref();
        let config = WinxConfig::default();

        self.save_project_config(project_path, &config)?;

        Ok(config)
    }
}

/// Global shared configuration manager
pub static CONFIG_MANAGER: once_cell::sync::Lazy<Arc<Mutex<ConfigManager>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(ConfigManager::new())));

/// Initialize the configuration system
pub fn initialize_config() -> Result<()> {
    info!("Initializing configuration system");

    let mut manager = CONFIG_MANAGER.lock().unwrap();
    manager.load_global_config()?;

    Ok(())
}

/// Get configuration for a project
pub fn get_project_config(project_path: impl AsRef<Path>) -> Result<WinxConfig> {
    let project_path = project_path.as_ref();
    let mut manager = CONFIG_MANAGER.lock().unwrap();

    // Load project config if not already loaded
    if !manager.project_configs.contains_key(project_path) {
        manager.load_project_config(project_path)?;
    }

    manager.get_config(project_path)
}

/// Save configuration for a project
pub fn save_project_config(project_path: impl AsRef<Path>, config: &WinxConfig) -> Result<()> {
    let project_path = project_path.as_ref();
    let mut manager = CONFIG_MANAGER.lock().unwrap();

    manager.save_project_config(project_path, config)?;

    // Update the in-memory config
    manager
        .project_configs
        .insert(project_path.to_path_buf(), config.clone());

    Ok(())
}

/// Create a default project configuration
pub fn create_default_project_config(project_path: impl AsRef<Path>) -> Result<WinxConfig> {
    let manager = CONFIG_MANAGER.lock().unwrap();
    manager.create_default_project_config(project_path)
}
