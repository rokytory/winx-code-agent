// Winx - A performant code agent written in Rust
// Based on the WCGW architecture but optimized for performance

pub mod code;
pub mod commands;
pub mod core;
pub mod diff;
pub mod integrations;
pub mod lsp;
pub mod plugins;
pub mod sql;
pub mod thinking;
pub mod utils;

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::env;
use tracing::{debug, info};

/// Regex for stripping ANSI color codes - ultra comprehensive and robust
static ANSI_REGEX: Lazy<Regex> = Lazy::new(|| {
    // This matches all ANSI escape sequences used for colors and formatting
    // Ultra comprehensive pattern to catch all documented and undocumented ANSI sequences
    Regex::new(concat!(
        // Match ESC and CSI followed by any control sequence - main pattern
        r"[\x1b\x9b][\[\]()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[\dA-PR-TZcf-nq-uy=><~]|",
        // Match color codes (SGR sequences)
        r"[\x1b\x9b]\[[\x30-\x3F]*[\x20-\x2F]*[\x40-\x7E]|",
        // Match older style codes
        r"[\x1b\x9b][0-9;]*[a-zA-Z]|",
        // Catch any standalone ESC character
        r"\x1b|",
        // Match Unicode console codes (rare but possible)
        r"\u009b[^A-Za-z]*[A-Za-z]|",
        // Match CSI window manipulation
        r"\x1b\][0-9][^\x07]*\x07"
    ))
    .unwrap_or_else(|_| {
        debug!("Failed to compile ANSI regex, falling back to simpler pattern");
        Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap()
    })
});

/// Strip ANSI color codes from a string - ultra enhanced version with JSON safety
pub fn strip_ansi_codes(input: &str) -> String {
    // Pre-check to see if the input contains any escape sequences
    let has_ansi = input.contains('\u{001B}') || input.contains('\u{009B}');

    if !has_ansi {
        // If no ANSI detected, just perform basic control character filtering
        return input
            .chars()
            .filter(|&c| c >= ' ' || c == '\n' || c == '\r' || c == '\t')
            .collect::<String>();
    }

    // First pass: Replace standard escape sequences with regex
    let mut result = ANSI_REGEX.replace_all(input, "").to_string();

    // Second pass: Handle any broken or invalid ANSI sequences by filtering out control characters
    // This catches any non-standard or broken ANSI sequences
    let filtered = result
        .chars()
        .filter(|&c| c >= ' ' || c == '\n' || c == '\r' || c == '\t')
        .collect::<String>();

    // Check if it's JSON content and needs extra sanitization
    let is_json = filtered.contains("jsonrpc")
        || (filtered.contains('{') && filtered.contains('"') && filtered.contains(':'));

    // Apply JSON sanitization for JSON content
    let sanitized = if is_json {
        sanitize_json_text(&filtered)
    } else {
        filtered
    };

    // Final verification pass - ensure NO escape sequences remain
    if sanitized.contains('\u{001B}') || sanitized.contains('\u{009B}') {
        // If any escape sequences still remain, do a brute force character-by-character filtering
        debug!("ANSI stripping fallback: escape sequences detected after regex pass");
        sanitized
            .chars()
            .filter(|&c| (c >= ' ' && c <= '~') || c == '\n' || c == '\r' || c == '\t')
            .collect::<String>()
    } else {
        sanitized
    }
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
    let hex_data = data
        .iter()
        .take(display_limit)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ");

    // Log the first bytes for detailed analysis
    info!(
        "{} - Raw bytes (hex, first {}): {}",
        prefix, display_limit, hex_data
    );

    // Detect if bytes might contain ANSI codes
    let ansi_indicator = data.iter().any(|&b| b == 0x1b);
    if ansi_indicator {
        info!("{} - WARNING: ANSI escape codes detected in data", prefix);
    }

    // Try to decode as UTF-8 with aggressive cleaning
    let text = match std::str::from_utf8(data) {
        Ok(text) => {
            // Pre-emptively clean the text from ANSI and control characters
            let cleaned_text = strip_ansi_codes(text);
            cleaned_text
        }
        Err(_) => {
            // Fall back to lossy conversion and then clean it
            let text = String::from_utf8_lossy(data);
            let cleaned_text = strip_ansi_codes(&text);
            cleaned_text
        }
    };

    // Show a preview of the cleaned text
    let preview_len = std::cmp::min(text.len(), 200);
    let preview = if text.len() > preview_len {
        format!(
            "{}... (truncated, total length: {})",
            &text[..preview_len],
            text.len()
        )
    } else {
        text.to_string()
    };

    info!("{} - Cleaned text: {}", prefix, preview);

    // Extra sanitization for JSON-RPC messages
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

        // Apply additional JSON-specific sanitization
        let json_safe_text = sanitize_json_text(&text);

        // Try to parse the cleaned text as JSON
        match serde_json::from_str::<serde_json::Value>(&json_safe_text) {
            Ok(json) => {
                // Check for important JSON-RPC fields
                if let Some(obj) = json.as_object() {
                    info!(
                        "{} - JSON-RPC detected. Fields present: id={}, method={}, params={}",
                        prefix,
                        obj.contains_key("id"),
                        obj.contains_key("method"),
                        obj.contains_key("params")
                    );

                    // Log the structure of params if present
                    if let Some(params) = obj.get("params") {
                        info!(
                            "{} - Params type: {}",
                            prefix,
                            if params.is_object() {
                                "object"
                            } else if params.is_array() {
                                "array"
                            } else {
                                "other"
                            }
                        );
                    }
                }
            }
            Err(e) => {
                info!(
                    "{} - JSON parsing failed even after sanitization: {}",
                    prefix, e
                );

                // If ANSI codes could be causing the issue, log a suggestion
                if ansi_indicator {
                    info!("{} - Consider reviewing ANSI stripping logic or adding 'strip_ansi_codes' before JSON parsing", prefix);
                }
            }
        }
    }
}

/// Comprehensive sanitization for JSON text - enhanced for RMCP compatibility
pub fn sanitize_json_text(text: &str) -> String {
    // Remove all control characters (0x00-0x1F except allowed whitespace)
    let mut sanitized = String::with_capacity(text.len());

    // First pass: Detect and log if there are ANSI escape codes
    let has_ansi = text.contains('\u{001B}') || text.contains('\u{009B}');
    if has_ansi {
        debug!("sanitize_json_text: ANSI escape codes detected in input");
    }

    // Check if this is likely a JSON-RPC message
    let is_json_rpc =
        text.contains("jsonrpc") && (text.contains("\"method\"") || text.contains("\"id\""));

    // Second pass: Remove all known problematic characters
    for c in text.chars() {
        match c {
            // Explicitly allowed whitespace characters
            '\n' | '\r' | '\t' => sanitized.push(c),

            // Printable ASCII range and beyond
            c if c >= ' ' => sanitized.push(c),

            // Skip all other control characters (including all ANSI escape sequences)
            _ => {}
        }
    }

    // Fix any malformed JSON structures
    // Only do this if the string appears to be JSON
    let looks_like_json = sanitized.contains('{')
        && sanitized.contains('"')
        && (sanitized.contains(':') || is_json_rpc);

    if looks_like_json {
        // Try to fix common JSON issues

        // Balance quotes if needed (only if not too many)
        let quote_count = sanitized.chars().filter(|&c| c == '"').count();
        if quote_count % 2 != 0 && quote_count < 100 {
            debug!("JSON sanitization: Fixing unbalanced quotes");
            sanitized.push('"');
        }

        // Balance braces and brackets if needed
        let open_braces = sanitized.chars().filter(|&c| c == '{').count();
        let close_braces = sanitized.chars().filter(|&c| c == '}').count();
        let open_brackets = sanitized.chars().filter(|&c| c == '[').count();
        let close_brackets = sanitized.chars().filter(|&c| c == ']').count();

        if open_braces > close_braces {
            debug!("JSON sanitization: Fixing unbalanced braces");
            for _ in 0..(open_braces - close_braces) {
                sanitized.push('}');
            }
        }

        if open_brackets > close_brackets {
            debug!("JSON sanitization: Fixing unbalanced brackets");
            for _ in 0..(open_brackets - close_brackets) {
                sanitized.push(']');
            }
        }

        // Validate the JSON if it looks like JSON-RPC
        if is_json_rpc {
            match serde_json::from_str::<serde_json::Value>(&sanitized) {
                Ok(_) => {
                    debug!("JSON-RPC message successfully validated after sanitization");
                }
                Err(e) => {
                    debug!(
                        "JSON-RPC message failed validation after sanitization: {}",
                        e
                    );

                    // Last resort: try to repair common issues in JSON-RPC messages
                    // Look for incomplete method or params structure
                    if sanitized.contains("\"method\":") && !sanitized.contains("\"params\":") {
                        debug!("Adding missing params field");
                        // Find a good position to insert params - after the method field
                        if let Some(method_pos) = sanitized.find("\"method\":") {
                            // Find the next quotation mark after method value
                            if let Some(next_quote) = sanitized[method_pos + 10..].find('"') {
                                let insert_pos = method_pos + 10 + next_quote + 1;
                                if insert_pos < sanitized.len() {
                                    sanitized.insert_str(insert_pos, ",\"params\":{}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    sanitized
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

    // Note: Plugin initialization is now expected to be done from an async context
    // outside this function to avoid nested runtimes

    Ok(())
}

/// Async version of workspace initialization for plugin system
/// Call this from an existing async context
pub async fn init_plugins_async(workspace_path: &str) -> Result<()> {
    // Initialize the plugin system in the existing async context
    plugins::initialize_plugins(workspace_path).await?;
    Ok(())
}

/// Initialize file tracking for initial files
pub async fn init_file_tracking(state: &core::state::SharedState, files: &[&str]) -> Result<()> {
    use crate::commands::files;
    use std::path::Path;
    
    // Filter for existing files within the workspace
    let state_guard = state.lock().unwrap();
    let workspace_path = state_guard.workspace_path.clone();
    drop(state_guard);
    
    let mut existing_files = Vec::new();
    for file in files {
        let path = Path::new(file);
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace_path.join(path)
        };
        
        if full_path.exists() && full_path.is_file() {
            existing_files.push(full_path.to_string_lossy().to_string());
        }
    }
    
    // Auto-read important project files
    let important_patterns = [
        "*.md",
        "*.toml",
        "*.json",
        "Cargo.lock",
        "README*",
        "CONTRIBUTING*",
        "LICENSE*",
        ".gitignore",
    ];
    
    for pattern in &important_patterns {
        if let Ok(glob_paths) = glob::glob(&workspace_path.join(pattern).to_string_lossy()) {
            for entry in glob_paths {
                if let Ok(path) = entry {
                    if path.is_file() {
                        existing_files.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    
    // Read files if any exist
    if !existing_files.is_empty() {
        info!("Auto-reading {} initial project files", existing_files.len());
        let _ = files::read_files_internal(state, &existing_files, None).await;
    }
    
    Ok(())
}

/// Initialize the Winx agent with custom logger configuration
///
/// @param ansi_colors - Whether to enable ANSI color codes in logs
/// When used as an MCP server, this should be false to avoid JSON parsing errors
pub fn init_with_logger(ansi_colors: bool) -> Result<()> {
    use tracing_subscriber::fmt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::EnvFilter;

    // Use the try_init() method instead of init() to handle cases where
    // a global subscriber has already been set
    let result = if !ansi_colors {
        // We'll use a simple writer - the bash.rs AnsiStrippingWriter will handle output separately
        fmt::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_ansi(false) // Disable ANSI explicitly
            .with_target(false) // Minimum formatting
            .without_time() // No timestamp formatting
            .try_init()
    } else {
        // Default configuration for CLI usage
        fmt::fmt()
            .with_ansi(true)
            .with_env_filter(EnvFilter::from_default_env())
            .with_target(true)
            .try_init()
    };

    // If initialization failed, log a warning but don't fail the whole app
    if let Err(e) = result {
        eprintln!(
            "Warning: Could not initialize logger: {}. Continuing anyway.",
            e
        );
    }

    // Set panic hook to avoid ANSI codes in panics
    std::panic::set_hook(Box::new(|panic_info| {
        let text = format!("PANIC: {}", panic_info);
        let stripped = strip_ansi_codes(&text);
        eprintln!("{}", stripped);
    }));

    if !ansi_colors {
        info!("Initializing Winx agent v{} (ANSI-free for MCP)", version());
    } else {
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
        .or_else(|_| {
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))
        })
        .context("Failed to determine a default workspace directory")?;

    // Initialize terminal manager with default directory
    commands::terminal::init_terminal_manager(default_dir.to_string_lossy().to_string());

    // Reseta o estado de inicialização para garantir que as ferramentas requerem inicialização adequada
    commands::tools::reset_initialization();
    
    Ok(())
}
