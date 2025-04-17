// Winx - A performant code agent written in Rust
// Based on the WCGW architecture but optimized for performance

pub mod code;
pub mod commands;
pub mod core;
pub mod diff;
pub mod integrations;
pub mod lsp;
pub mod sql;
pub mod thinking;
pub mod utils;

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use tracing::{debug, info};
use regex::Regex;
use once_cell::sync::Lazy;

/// Regex for stripping ANSI color codes - very comprehensive
static ANSI_REGEX: Lazy<Regex> = Lazy::new(|| {
    // This matches all ANSI escape sequences used for colors and formatting
    // Enhanced pattern to catch more edge cases
    Regex::new(r"\x1b(?:[@-Z\\-_]|\[[0-9:;<=>?]*[ -/]*[@-~])").unwrap_or_else(|_| Regex::new(r"").unwrap())
});

/// Strip ANSI color codes from a string
pub fn strip_ansi_codes(input: &str) -> String {
    // First, replace standard escape sequences with regex
    let result = ANSI_REGEX.replace_all(input, "").to_string();
    
    // Then manually filter out any remaining control characters
    // This catches any non-standard or broken ANSI sequences
    result.chars()
        .filter(|&c| c >= ' ' || c == '\n' || c == '\r' || c == '\t')
        .collect()
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Function to analyze and debug JSON messages to find parsing problems
pub fn debug_json_bytes(data: &[u8], prefix: &str) {
    if data.is_empty() {
        return;
    }

    // Log bytes in hexadecimal format - show up to 100 bytes
    let display_limit = std::cmp::min(data.len(), 100);
    let hex_data = data.iter()
        .take(display_limit)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ");

    // Log the first bytes for detailed analysis
    info!("{} - Raw bytes (hex, first {}): {}", prefix, display_limit, hex_data);

    // Try to decode as UTF-8
    match std::str::from_utf8(data) {
        Ok(text) => {
            // Show a preview of the text (limited to avoid log overflow)
            let preview_len = std::cmp::min(text.len(), 200);
            let preview = if text.len() > preview_len {
                format!("{}... (truncated, total length: {})", &text[..preview_len], text.len())
            } else {
                text.to_string()
            };

            info!("{} - UTF-8 text: {}", prefix, preview);

            // If it's JSON, try to parse and examine structure
            if text.contains("jsonrpc") {
                // Check each character in the first bytes (where parsing errors often occur)
                for (i, &b) in data.iter().take(20).enumerate() {
                    let char_desc = if b < 32 || b > 126 {
                        format!("\\x{:02x} (control)", b)
                    } else {
                        format!("'{}' ({})", b as char, b)
                    };
                    info!("{} - Byte {}: {}", prefix, i, char_desc);
                }

                // Try to parse as JSON to identify parsing issues
                match serde_json::from_str::<serde_json::Value>(text) {
                    Ok(json) => {
                        // Check for important JSON-RPC fields
                        if let Some(obj) = json.as_object() {
                            info!("{} - JSON-RPC detected. Fields present: id={}, method={}, params={}",
                                prefix,
                                obj.contains_key("id"),
                                obj.contains_key("method"),
                                obj.contains_key("params")
                            );

                            // Log the structure of params if present
                            if let Some(params) = obj.get("params") {
                                info!("{} - Params type: {}", prefix, 
                                    if params.is_object() { "object" } 
                                    else if params.is_array() { "array" } 
                                    else { "other" }
                                );
                            }
                        }
                    }
                    Err(e) => {
                        info!("{} - JSON parsing failed: {}", prefix, e);
                    }
                }
            }
        }
        Err(e) => {
            info!("{} - Invalid UTF-8: {}", prefix, e);
        }
    }
}

/// Initialize the Winx agent with default settings
/// This method is kept for backward compatibility
pub fn init() -> Result<()> {
    // Default to colored output for CLI usage
    init_with_logger(true)
}

/// Initialize the Winx agent with workspace
pub fn init_with_workspace(workspace_path: &str) -> Result<()> {
    // Initialize with default logger
    init_with_logger(true)?;

    // Initialize terminal manager
    commands::terminal::init_terminal_manager(workspace_path.to_string());

    // Initialize memory store
    let memory_dir = core::memory::get_memory_dir()?;
    core::memory::create_shared_memory_store(memory_dir)?;

    Ok(())
}

/// Initialize the Winx agent with custom logger configuration
///
/// @param ansi_colors - Whether to enable ANSI color codes in logs
/// When used as an MCP server, this should be false to avoid JSON parsing errors
pub fn init_with_logger(ansi_colors: bool) -> Result<()> {
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;
    
    // Extremely simplified minimal approach to avoid type errors
    if !ansi_colors {
        // We'll use a simple writer - the bash.rs AnsiStrippingWriter will handle output separately
        fmt::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_ansi(false) // Disable ANSI explicitly
            .with_target(false) // Minimum formatting
            .without_time() // No timestamp formatting 
            .init();
            
        // Set panic hook to avoid ANSI codes in panics
        std::panic::set_hook(Box::new(|panic_info| {
            let text = format!("PANIC: {}", panic_info);
            let stripped = strip_ansi_codes(&text);
            eprintln!("{}", stripped);
        }));
        
        info!("Initializing Winx agent v{} (ANSI-free for MCP)", version());
    } else {
        // Default configuration for CLI usage
        fmt::fmt()
            .with_ansi(true)
            .with_env_filter(EnvFilter::from_default_env())
            .with_target(true)
            .init();

        info!("Initializing Winx agent v{}", version());
    }

    // Initialize once_cell modules
    if let Err(e) = initialize_once_cell_modules() {
        debug!("Warning: Some modules failed to initialize: {}", e);
    }

    Ok(())
}

/// Initialize any once_cell modules that need to be ready at startup
fn initialize_once_cell_modules() -> Result<()> {
    // Get current directory or user directory for defaults
    let default_dir = env::current_dir()
        .or_else(|_| dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory")))
        .context("Failed to determine a default workspace directory")?;

    // Initialize terminal manager with default directory
    commands::terminal::init_terminal_manager(default_dir.to_string_lossy().to_string());

    Ok(())
}
