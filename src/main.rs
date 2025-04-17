use anyhow::{Context, Result};
use rmcp::ServiceExt;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::info;

use winx::{
    commands::tools::WinxTools,
    core::{state::create_shared_state, types::ModeType},
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

    // Initialize state with wcgw mode
    let state = create_shared_state(workspace_path.clone(), ModeType::Wcgw, None, None)
        .context("Failed to create agent state")?;

    // Create WinxTools instance
    let tools = WinxTools::new(state.clone());

    // Start the MCP server using stdio transport
    info!("Starting MCP server");
    let client = tools
        .serve(rmcp::transport::stdio())
        .await
        .context("Failed to start MCP server")?;

    info!("Winx agent started successfully");

    // Keep the application running until client disconnects
    client.waiting().await?;

    info!("Shutting down Winx agent");

    Ok(())
}
