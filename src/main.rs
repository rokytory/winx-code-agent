use anyhow::{Context, Result};
use rmcp::ServiceExt;
use std::env;
use std::error::Error;
use std::path::PathBuf;
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
    // and provide workspace path for terminal and memory components
    winx::init_with_workspace(&workspace_path.to_string_lossy())
        .context("Failed to initialize Winx agent")?;

    // Log version and environment information
    info!(
        "Starting Winx v{} on {}",
        winx::version(),
        std::env::consts::OS
    );

    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let workspace_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        std::env::current_dir()?
    };

    info!("Using workspace path: {}", workspace_path.display());

    // Initialize state with wcgw mode and any stored task information
    let state = match create_shared_state(workspace_path.clone(), ModeType::Wcgw, None, None) {
        Ok(state) => {
            info!("Agent state created successfully");
            
            // Initialize syntax validator
            if let Err(e) = winx::code::get_syntax_validator() {
                info!("Warning: Syntax validator initialization failed: {}", e);
            }
            
            state
        }
        Err(e) => {
            eprintln!("Failed to create agent state: {}", e);
            return Err(anyhow::anyhow!("Failed to create agent state: {}", e));
        }
    };

    // Create WinxTools instance
    let tools = WinxTools::new(state.clone());

    // Configure MCP server
    info!("Starting MCP server using stdio transport");

    // Use standard stdio transport for MCP communication
    let transport = rmcp::transport::stdio();

    // Start the MCP server and keep it running until the client disconnects
    info!("Server starting...");

    // Add error handling and detailed logging
    let client_result = tools.serve(transport).await;
    let client = match client_result {
        Ok(client) => {
            info!("MCP server started successfully");
            client
        }
        Err(e) => {
            eprintln!("Failed to start MCP server: {}", e);
            info!("Error starting MCP server: {}", e);

            // Attempt to log more details about the error
            if let Some(source) = std::error::Error::source(&e) {
                info!("Caused by: {}", source);
            }

            return Err(anyhow::anyhow!("Failed to start MCP server: {}", e));
        }
    };

    info!("Winx agent started successfully, waiting for client requests");

    // Wait until the client disconnects with error handling
    match client.waiting().await {
        Ok(_) => {
            info!("Client disconnected gracefully");
        }
        Err(e) => {
            info!("Client connection error: {}", e);
        }
    }

    info!("Shutting down Winx agent");

    Ok(())
}
