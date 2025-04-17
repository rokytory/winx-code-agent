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

/// Função para analisar e depurar mensagens JSON para encontrar problemas de parsing
pub fn debug_json_bytes(data: &[u8], prefix: &str) {
    if data.is_empty() {
        return;
    }
    
    // Log dos bytes em formato hexadecimal
    let hex_data = data.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ");
    
    // Log dos primeiros bytes para análise detalhada
    info!("{} - Raw bytes (hex): {}", prefix, hex_data);
    
    // Tentar decodificar como UTF-8
    match std::str::from_utf8(data) {
        Ok(text) => {
            info!("{} - UTF-8 text: {}", prefix, text);
            
            // Se for JSON, vamos examinar mais detalhes
            if text.contains("jsonrpc") {
                // Verificar cada caractere nos primeiros bytes (onde ocorre o erro)
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
    
    // Configurar formato extremamente simples se ansi_colors for false (modo MCP)
    if !ansi_colors {
        // Configuração mínima sem formatação que poderia interferir com JSON
        fmt::Subscriber::builder()
            .with_ansi(false)
            .with_writer(std::io::stderr) // Escreve logs para stderr em vez de stdout
            .with_env_filter(EnvFilter::from_default_env())
            .with_target(false)
            .without_time()
            .init();
        
        info!("Initializing Winx agent v{} (minimal log format for MCP)", version());
    } else {
        // Configuração padrão para uso em CLI
        fmt::Subscriber::builder()
            .with_ansi(true) 
            .with_env_filter(EnvFilter::from_default_env())
            .with_target(true)
            .init();
        
        info!("Initializing Winx agent v{}", version());
    }
    
    Ok(())
}