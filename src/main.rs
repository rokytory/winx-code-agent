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
    // Configure environment variables for debugging if not already defined
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "rmcp=trace,winx=trace");
    }
    
    // Ensure there are no ANSI color codes activated (by default)
    if env::var("NO_COLOR").is_err() {
        env::set_var("NO_COLOR", "1");
    }
    
    // Initialize with ANSI colors explicitly disabled for MCP compatibility
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

    // Configure MCP server
    info!("Starting MCP server using stdio transport");
    
    // Use standard stdio transport for MCP communication
    let transport = rmcp::transport::stdio();
    
    // Start the MCP server and keep it running until the client disconnects
    info!("Server starting...");
    let client = tools
        .serve(transport)
        .await
        .context("Failed to start MCP server")?;

    info!("Winx agent started successfully");

    // Wait until the client disconnects
    client.waiting().await?;

    info!("Shutting down Winx agent");

    Ok(())
}
