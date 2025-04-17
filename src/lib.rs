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

use anyhow::Result;
use tracing::{info, debug};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Function to analyze and debug JSON messages to find parsing problems
pub fn debug_json_bytes(data: &[u8], prefix: &str) {
    if data.is_empty() {
        return;
    }
    
    // Log bytes in hexadecimal format
    let hex_data = data.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ");
    
    // Log the first bytes for detailed analysis
    info!("{} - Raw bytes (hex): {}", prefix, hex_data);
    
    // Try to decode as UTF-8
    match std::str::from_utf8(data) {
        Ok(text) => {
            info!("{} - UTF-8 text: {}", prefix, text);
            
            // If it's JSON, examine more details
            if text.contains("jsonrpc") {
                // Check each character in the first bytes (where the error occurs)
                for (i, &b) in data.iter().take(10).enumerate() {
                    let char_desc = if b < 32 || b > 126 {
                        format!("\\x{:02x} (control)", b)
                    } else {
                        format!("'{}' ({})", b as char, b)
                    };
                    info!("{} - Byte {}: {}", prefix, i, char_desc);
                }
            }
        },
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

/// Initialize the Winx agent with custom logger configuration
/// 
/// @param ansi_colors - Whether to enable ANSI color codes in logs
/// When used as an MCP server, this should be false to avoid JSON parsing errors
pub fn init_with_logger(ansi_colors: bool) -> Result<()> {
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;
    
    // Configure extremely simple format if ansi_colors is false (MCP mode)
    if !ansi_colors {
        // Minimal configuration without formatting that could interfere with JSON
        fmt::Subscriber::builder()
            .with_ansi(false)
            .with_writer(std::io::stderr) // Write logs to stderr instead of stdout
            .with_env_filter(EnvFilter::from_default_env())
            .with_target(false)
            .without_time()
            .init();
        
        info!("Initializing Winx agent v{} (minimal log format for MCP)", version());
    } else {
        // Default configuration for CLI usage
        fmt::Subscriber::builder()
            .with_ansi(true) 
            .with_env_filter(EnvFilter::from_default_env())
            .with_target(true)
            .init();
        
        info!("Initializing Winx agent v{}", version());
    }
    
    Ok(())
}
