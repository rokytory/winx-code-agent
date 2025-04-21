// Configuration module for winx-code-agent
// This module provides persistence and configuration for the agent

pub mod config_loader;
pub mod project_config;

// Re-export main types for easier access
pub use config_loader::{ConfigLoader, TransportType, WinxConfig};
pub use project_config::WinxProjectConfig;
