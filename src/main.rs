use anyhow::{Context, Result};
use rmcp::{ServiceExt};
use rmcp::transport::TokioChildProcess;
use tokio::process::Command;
use tracing::info;
use std::path::PathBuf;

use winx::{
    core::{state::create_shared_state, types::ModeType},
    commands::register_tools,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the Winx agent
    winx::init().context("Failed to initialize Winx agent")?;
    
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let workspace_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        std::env::current_dir()?
    };
    
    info!("Using workspace path: {}", workspace_path.display());
    
    // Initialize state
    let state = create_shared_state(
        workspace_path.clone(), 
        ModeType::Wcgw, 
        None, 
        None
    ).context("Failed to create agent state")?;
    
    // Register tools
    register_tools(state.clone()).context("Failed to register tools")?;
    
    // Start the MCP server - as tools já são registradas pelo register_tools
    info!("Starting MCP server");
    
    // Ao invés de iniciar um MCP client, vamos esperar conexões
    info!("Waiting for MCP connections...");
    
    info!("Winx agent started successfully");
    
    // Keep the application running until Ctrl+C or termination
    tokio::signal::ctrl_c().await.context("Failed to listen for Ctrl+C")?;
    info!("Shutting down Winx agent");
    
    Ok(())
}
