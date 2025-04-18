use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

/// Trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync + Clone {
    /// Get the name of the plugin
    fn name(&self) -> &'static str;

    /// Get the version of the plugin
    fn version(&self) -> &'static str;

    /// Get a description of the plugin
    fn description(&self) -> &'static str;

    /// Initialize the plugin
    async fn initialize(&self) -> Result<()>;

    /// Shutdown the plugin
    async fn shutdown(&self) -> Result<()>;

    /// Execute a command in the plugin
    async fn execute_command(&self, command: &str, args: &[String]) -> Result<String>;
}

/// Information about a plugin
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Name of the plugin
    pub name: String,
    /// Version of the plugin
    pub version: String,
    /// Description of the plugin
    pub description: String,
    /// Whether the plugin has been initialized
    pub initialized: bool,
}

/// Manager for plugins
#[derive(Clone)]
pub struct PluginManager {
    /// Registered plugins (stored in an enum to handle dynamic dispatch)
    plugins: HashMap<String, PluginEnum>,
    /// Path to plugin directory
    plugin_dir: PathBuf,
    /// Whether plugins have been initialized
    initialized: bool,
}

/// Enum to store different plugin types
#[derive(Clone)]
enum PluginEnum {
    Git(GitPlugin),
    CodeQuality(CodeQualityPlugin),
}

impl PluginEnum {
    async fn initialize(&self) -> Result<()> {
        match self {
            PluginEnum::Git(p) => p.initialize().await,
            PluginEnum::CodeQuality(p) => p.initialize().await,
        }
    }

    async fn shutdown(&self) -> Result<()> {
        match self {
            PluginEnum::Git(p) => p.shutdown().await,
            PluginEnum::CodeQuality(p) => p.shutdown().await,
        }
    }

    async fn execute_command(&self, command: &str, args: &[String]) -> Result<String> {
        match self {
            PluginEnum::Git(p) => p.execute_command(command, args).await,
            PluginEnum::CodeQuality(p) => p.execute_command(command, args).await,
        }
    }

    #[allow(dead_code)]
    fn name(&self) -> &'static str {
        match self {
            PluginEnum::Git(p) => p.name(),
            PluginEnum::CodeQuality(p) => p.name(),
        }
    }

    fn version(&self) -> &'static str {
        match self {
            PluginEnum::Git(p) => p.version(),
            PluginEnum::CodeQuality(p) => p.version(),
        }
    }

    fn description(&self) -> &'static str {
        match self {
            PluginEnum::Git(p) => p.description(),
            PluginEnum::CodeQuality(p) => p.description(),
        }
    }
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(plugin_dir: impl AsRef<Path>) -> Self {
        Self {
            plugins: HashMap::new(),
            plugin_dir: plugin_dir.as_ref().to_path_buf(),
            initialized: false,
        }
    }

    /// Register a git plugin
    pub fn register_git_plugin(&mut self, plugin: GitPlugin) -> Result<()> {
        let name = plugin.name().to_string();

        if self.plugins.contains_key(&name) {
            return Err(anyhow::anyhow!(
                "Plugin with name '{}' already registered",
                name
            ));
        }

        info!(
            "Registering plugin: {} v{}",
            plugin.name(),
            plugin.version()
        );
        self.plugins.insert(name, PluginEnum::Git(plugin));

        Ok(())
    }

    /// Register a code quality plugin
    pub fn register_code_quality_plugin(&mut self, plugin: CodeQualityPlugin) -> Result<()> {
        let name = plugin.name().to_string();

        if self.plugins.contains_key(&name) {
            return Err(anyhow::anyhow!(
                "Plugin with name '{}' already registered",
                name
            ));
        }

        info!(
            "Registering plugin: {} v{}",
            plugin.name(),
            plugin.version()
        );
        self.plugins.insert(name, PluginEnum::CodeQuality(plugin));

        Ok(())
    }

    /// Initialize all plugins
    pub async fn initialize_all(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        info!("Initializing {} plugins", self.plugins.len());
        let mut initialization_errors = Vec::new();

        for (name, plugin) in &self.plugins {
            info!("Initializing plugin: {}", name);

            if let Err(e) = plugin.initialize().await {
                // Instead of failing completely, just log the error and continue
                warn!(
                    "Failed to initialize plugin '{}': {} - continuing without it",
                    name, e
                );
                initialization_errors.push(format!("Plugin '{}': {}", name, e));
            }
        }

        self.initialized = true;

        if initialization_errors.is_empty() {
            info!("All plugins initialized successfully");
        } else {
            info!(
                "Plugins initialized with some failures: {}",
                initialization_errors.join(", ")
            );
        }

        Ok(())
    }

    /// Execute a command in a plugin
    pub async fn execute_command(
        &self,
        plugin_name: &str,
        command: &str,
        args: &[String],
    ) -> Result<String> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Plugins not initialized"));
        }

        if let Some(plugin) = self.plugins.get(plugin_name) {
            debug!(
                "Executing command '{}' in plugin '{}'",
                command, plugin_name
            );
            plugin.execute_command(command, args).await
        } else {
            Err(anyhow::anyhow!("Plugin '{}' not found", plugin_name))
        }
    }

    /// Shutdown all plugins
    pub async fn shutdown_all(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        info!("Shutting down {} plugins", self.plugins.len());

        for (name, plugin) in &self.plugins {
            info!("Shutting down plugin: {}", name);

            if let Err(e) = plugin.shutdown().await {
                warn!("Failed to shutdown plugin '{}': {}", name, e);
            }
        }

        self.initialized = false;
        info!("All plugins shut down");

        Ok(())
    }

    /// Get a list of all registered plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins
            .iter()
            .map(|(name, plugin)| PluginInfo {
                name: name.clone(),
                version: plugin.version().to_string(),
                description: plugin.description().to_string(),
                initialized: self.initialized,
            })
            .collect()
    }

    /// Check if a plugin is registered
    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Get the number of registered plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get the plugin directory
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }
}

/// Example plugin that provides git integration
#[derive(Clone, Debug)]
pub struct GitPlugin {
    workspace_dir: PathBuf,
}

impl GitPlugin {
    /// Create a new git plugin
    pub fn new(workspace_dir: impl AsRef<Path>) -> Self {
        Self {
            workspace_dir: workspace_dir.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl Plugin for GitPlugin {
    fn name(&self) -> &'static str {
        "git"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn description(&self) -> &'static str {
        "Git integration for Winx"
    }

    async fn initialize(&self) -> Result<()> {
        // Check if git is installed
        let git_cmd_result = tokio::process::Command::new("git")
            .arg("--version")
            .output()
            .await;

        if let Err(e) = git_cmd_result {
            warn!(
                "Git command check failed: {} - Git functionality will be limited",
                e
            );
            return Ok(()); // Continue without Git functionality
        }

        let output = git_cmd_result.unwrap();
        if !output.status.success() {
            warn!("Git is not installed - Git functionality will be limited");
            return Ok(()); // Continue without Git functionality
        }

        // Check if the workspace is a git repository
        let repo_check = tokio::process::Command::new("git")
            .current_dir(&self.workspace_dir)
            .args(["rev-parse", "--is-inside-work-tree"])
            .output()
            .await;

        if let Err(e) = repo_check {
            warn!(
                "Git repository check failed: {} - Git functionality will be limited",
                e
            );
            return Ok(()); // Continue without Git functionality
        }

        let output = repo_check.unwrap();
        if !output.status.success() {
            warn!("Workspace is not a git repository - Git functionality will be limited");
            return Ok(()); // Continue without Git functionality
        }

        info!("Git plugin initialized successfully with full functionality");
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        // Nothing to do
        Ok(())
    }

    async fn execute_command(&self, command: &str, args: &[String]) -> Result<String> {
        // Check if git is available first
        let git_check = tokio::process::Command::new("git")
            .arg("--version")
            .output()
            .await;

        if let Err(e) = git_check {
            return Err(anyhow::anyhow!("Git is not available: {}", e));
        }

        let output = git_check.unwrap();
        if !output.status.success() {
            return Err(anyhow::anyhow!("Git is not installed or not functioning"));
        }

        // Now execute the requested command
        match command {
            "status" => self.git_status().await,
            "branch" => self.git_branch().await,
            "log" => self.git_log().await,
            "diff" => self.git_diff(args).await,
            "commit" => self.git_commit(args).await,
            _ => Err(anyhow::anyhow!("Unknown git command: {}", command)),
        }
    }
}

impl GitPlugin {
    /// Get git status
    async fn git_status(&self) -> Result<String> {
        let output = tokio::process::Command::new("git")
            .current_dir(&self.workspace_dir)
            .args(["status", "--porcelain"])
            .output()
            .await
            .context("Failed to execute git status")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git status failed"));
        }

        String::from_utf8(output.stdout).context("Failed to parse git status output")
    }

    /// Get current branch
    async fn git_branch(&self) -> Result<String> {
        let output = tokio::process::Command::new("git")
            .current_dir(&self.workspace_dir)
            .args(["branch", "--show-current"])
            .output()
            .await
            .context("Failed to execute git branch")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git branch failed"));
        }

        Ok(String::from_utf8(output.stdout)
            .context("Failed to parse git branch output")?
            .trim()
            .to_string())
    }

    /// Get git log
    async fn git_log(&self) -> Result<String> {
        let output = tokio::process::Command::new("git")
            .current_dir(&self.workspace_dir)
            .args(["log", "--oneline", "-n", "10"])
            .output()
            .await
            .context("Failed to execute git log")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git log failed"));
        }

        String::from_utf8(output.stdout).context("Failed to parse git log output")
    }

    /// Get git diff
    async fn git_diff(&self, args: &[String]) -> Result<String> {
        let mut cmd = tokio::process::Command::new("git");
        cmd.current_dir(&self.workspace_dir).arg("diff");

        if !args.is_empty() {
            cmd.args(args);
        }

        let output = cmd.output().await.context("Failed to execute git diff")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git diff failed"));
        }

        String::from_utf8(output.stdout).context("Failed to parse git diff output")
    }

    /// Commit changes
    async fn git_commit(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Commit message required"));
        }

        // First stage changes if requested
        if args.len() > 1 && args[0] == "--stage" {
            let output = tokio::process::Command::new("git")
                .current_dir(&self.workspace_dir)
                .args(["add", "."])
                .output()
                .await
                .context("Failed to stage changes")?;

            if !output.status.success() {
                return Err(anyhow::anyhow!("Failed to stage changes"));
            }
        }

        let message = &args[args.len() - 1];

        let output = tokio::process::Command::new("git")
            .current_dir(&self.workspace_dir)
            .args(["commit", "-m", message])
            .output()
            .await
            .context("Failed to execute git commit")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Git commit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        String::from_utf8(output.stdout).context("Failed to parse git commit output")
    }
}

/// Example plugin that provides code quality checks
#[derive(Clone, Debug)]
pub struct CodeQualityPlugin;

impl CodeQualityPlugin {
    /// Create a new code quality plugin
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodeQualityPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for CodeQualityPlugin {
    fn name(&self) -> &'static str {
        "code_quality"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn description(&self) -> &'static str {
        "Code quality checks for Winx"
    }

    async fn initialize(&self) -> Result<()> {
        // Nothing to do
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        // Nothing to do
        Ok(())
    }

    async fn execute_command(&self, command: &str, args: &[String]) -> Result<String> {
        match command {
            "check_rust" => self.check_rust(args).await,
            "check_python" => self.check_python(args).await,
            "check_js" => self.check_js(args).await,
            _ => Err(anyhow::anyhow!("Unknown code quality command: {}", command)),
        }
    }
}

impl CodeQualityPlugin {
    /// Check Rust code quality
    async fn check_rust(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("File path required"));
        }

        let file_path = &args[0];

        // Run clippy on the specific file
        let mut cmd = tokio::process::Command::new("cargo");
        cmd.args(["clippy", file_path, "--", "-D", "warnings"]);

        let output = cmd
            .output()
            .await
            .context("Failed to execute cargo clippy")?;

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        if !output.status.success() {
            return Ok(format!("Clippy found issues:\n{}{}", stdout, stderr));
        }

        Ok("No issues found".to_string())
    }

    /// Check Python code quality
    async fn check_python(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("File path required"));
        }

        let file_path = &args[0];

        // Run flake8
        let output = tokio::process::Command::new("flake8")
            .arg(file_path)
            .output()
            .await
            .context("Failed to execute flake8")?;

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        if !output.status.success() {
            return Ok(format!("Flake8 found issues:\n{}{}", stdout, stderr));
        }

        Ok("No issues found".to_string())
    }

    /// Check JavaScript code quality
    async fn check_js(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("File path required"));
        }

        let file_path = &args[0];

        // Run eslint
        let output = tokio::process::Command::new("eslint")
            .arg(file_path)
            .output()
            .await
            .context("Failed to execute eslint")?;

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        if !output.status.success() {
            return Ok(format!("ESLint found issues:\n{}{}", stdout, stderr));
        }

        Ok("No issues found".to_string())
    }
}

/// Global plugin manager
pub static PLUGIN_MANAGER: once_cell::sync::Lazy<Arc<Mutex<PluginManager>>> =
    once_cell::sync::Lazy::new(|| {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("winx")
            .join("plugins");

        Arc::new(Mutex::new(PluginManager::new(data_dir)))
    });

/// Initialize the plugin system
pub async fn initialize_plugins(workspace_dir: impl AsRef<Path>) -> Result<()> {
    let workspace_dir = workspace_dir.as_ref();

    // Obtain the manager with lock
    let mut manager = {
        let lock = PLUGIN_MANAGER.lock().unwrap();
        lock.clone()
    };

    // Register built-in plugins
    // Use if-let for each plugin registration to continue even if one fails
    if let Err(e) = manager.register_git_plugin(GitPlugin::new(workspace_dir)) {
        warn!(
            "Failed to register git plugin: {} - continuing without it",
            e
        );
    } else {
        info!("Git plugin registered successfully");
    }

    if let Err(e) = manager.register_code_quality_plugin(CodeQualityPlugin::new()) {
        warn!(
            "Failed to register code quality plugin: {} - continuing without it",
            e
        );
    } else {
        info!("Code quality plugin registered successfully");
    }

    // Initialize all plugins, but don't fail if initialization has issues
    if let Err(e) = manager.initialize_all().await {
        warn!(
            "Plugin initialization encountered issues: {} - continuing with partial functionality",
            e
        );
    }

    info!(
        "Plugin system initialized with {} plugins",
        manager.plugin_count()
    );

    Ok(())
}

/// Execute a plugin command
pub async fn execute_plugin_command(
    plugin_name: &str,
    command: &str,
    args: &[String],
) -> Result<String> {
    // Get a clone of the manager without holding the lock across await
    let manager_clone = {
        let manager = PLUGIN_MANAGER.lock().unwrap();
        manager.clone()
    };
    manager_clone
        .execute_command(plugin_name, command, args)
        .await
}

/// Get list of available plugins
pub fn list_plugins() -> Vec<PluginInfo> {
    let manager = PLUGIN_MANAGER.lock().unwrap();
    manager.list_plugins()
}

/// Shutdown the plugin system
pub async fn shutdown_plugins() -> Result<()> {
    // Get a clone of the manager without holding the lock across await
    let mut manager_clone = {
        let manager = PLUGIN_MANAGER.lock().unwrap();
        manager.clone()
    };
    manager_clone.shutdown_all().await
}
