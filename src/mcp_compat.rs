// Módulo para garantir compatibilidade entre diferentes versões do protocolo MCP

use log::{debug, error, info, warn};
use std::env;
use std::path::PathBuf;

/// Detecta e retorna o caminho do workspace com base nas configurações disponíveis
pub fn detect_workspace() -> PathBuf {
    // Prioridade 1: Variável de ambiente WINX_WORKSPACE
    if let Ok(workspace) = env::var("WINX_WORKSPACE") {
        info!("Using workspace from WINX_WORKSPACE env var: {}", workspace);
        return PathBuf::from(workspace);
    }
    
    // Prioridade 2: Configuração do Claude
    if let Some(claude_workspace) = get_claude_config_workspace() {
        info!("Using workspace from Claude config: {}", claude_workspace.display());
        return claude_workspace;
    }
    
    // Prioridade 3: Diretório atual
    if let Ok(current_dir) = std::env::current_dir() {
        info!("Using current directory as workspace: {}", current_dir.display());
        return current_dir;
    }
    
    // Fallback: Home do usuário
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    warn!("No workspace configured, using home directory: {}", home_dir.display());
    
    home_dir
}

/// Tenta obter a configuração de workspace do arquivo de configuração do Claude
fn get_claude_config_workspace() -> Option<PathBuf> {
    let config_path = dirs::home_dir()
        .map(|home| home.join("Library/Application Support/Claude/claude_desktop_config.json"))?;
    
    if !config_path.exists() {
        debug!("Claude config file not found at {}", config_path.display());
        return None;
    }
    
    debug!("Found Claude config at {}", config_path.display());
    
    // Tenta ler o arquivo de configuração
    match std::fs::read_to_string(&config_path) {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(config) => {
                    // Tenta extrair o workspace da configuração do winx
                    if let Some(env) = config
                        .get("mcpServers")
                        .and_then(|servers| servers.get("winx"))
                        .and_then(|winx| winx.get("env"))
                        .and_then(|env| env.get("WINX_WORKSPACE"))
                        .and_then(|workspace| workspace.as_str())
                    {
                        debug!("Found WINX_WORKSPACE in Claude config: {}", env);
                        return Some(PathBuf::from(env));
                    }
                    
                    debug!("No WINX_WORKSPACE found in Claude config");
                    None
                }
                Err(e) => {
                    error!("Failed to parse Claude config: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            error!("Failed to read Claude config: {}", e);
            None
        }
    }
}

/// Detecta o nível de log preferido ou usa o padrão
pub fn detect_log_level() -> String {
    env::var("RUST_LOG").unwrap_or_else(|_| {
        // Tenta obter do arquivo de configuração do Claude
        match dirs::home_dir()
            .map(|home| home.join("Library/Application Support/Claude/claude_desktop_config.json"))
        {
            Some(config_path) if config_path.exists() => {
                match std::fs::read_to_string(&config_path) {
                    Ok(content) => {
                        match serde_json::from_str::<serde_json::Value>(&content) {
                            Ok(config) => {
                                if let Some(log_level) = config
                                    .get("mcpServers")
                                    .and_then(|servers| servers.get("winx"))
                                    .and_then(|winx| winx.get("env"))
                                    .and_then(|env| env.get("RUST_LOG"))
                                    .and_then(|level| level.as_str())
                                {
                                    return log_level.to_string();
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    Err(_) => {}
                }
            }
            _ => {}
        }
        
        // Valor padrão
        "info".to_string()
    })
}
