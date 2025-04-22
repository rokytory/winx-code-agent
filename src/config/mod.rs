// Configuration module for winx-code-agent
// This module provides persistence and configuration for the agent

#[allow(clippy::module_inception)]
pub mod config;
pub mod config_loader;
pub mod project_config;

// Re-export main types for easier access
pub use config::WinxConfig;
pub use config_loader::{ConfigLoader, TransportType};
pub use project_config::WinxProjectConfig;
