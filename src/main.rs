use anyhow::{Context, Result};
use rmcp::ServiceExt;
use std::path::PathBuf;
use std::env;
use tracing::info;

use winx::{
    commands::tools::WinxTools,
    core::{state::create_shared_state, types::ModeType},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Configura variáveis de ambiente para debug se não estiverem definidas
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "rmcp=trace,winx=trace");
    }
    
    // Garante que não há ANSI color codes ativados (por padrão)
    if env::var("NO_COLOR").is_err() {
        env::set_var("NO_COLOR", "1");
    }
    
    // Inicializa com cores ANSI explicitamente desativadas para compatibilidade MCP
    winx::init_with_logger(false).context("Failed to initialize Winx agent")?;

    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let workspace_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        std::env::current_dir()?
    };

    info!("Using workspace path: {}", workspace_path.display());

    // Initialize state with wcgw mode
    let state = create_shared_state(workspace_path.clone(), ModeType::Wcgw, None, None)
        .context("Failed to create agent state")?;

    // Create WinxTools instance
    let tools = WinxTools::new(state.clone());

    // Configura o servidor MCP
    info!("Starting MCP server using stdio transport");
    
    // Usa o transporte padrão stdio para comunicação via MCP
    let transport = rmcp::transport::stdio();
    
    // Start the MCP server e mantém ele rodando até o cliente desconectar
    info!("Server starting...");
    let client = tools
        .serve(transport)
        .await
        .context("Failed to start MCP server")?;

    info!("Winx agent started successfully");

    // Aguarda até o cliente desconectar
    client.waiting().await?;

    info!("Shutting down Winx agent");

    Ok(())
}