// Configuration module for winx-code-agent
// This module provides persistence and configuration for the agent

pub mod project_config;
pub mod serena_compat;

// Re-export main types for easier access
pub use project_config::WinxProjectConfig;
