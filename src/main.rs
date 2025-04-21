use anyhow::Result;
use rmcp::{transport::io, ServiceExt};
use std::path::PathBuf;
use std::process::exit;
use winx_code_agent::server::CodeAgent;

mod logging;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize enhanced logging
    logging::init_logging();

    // Set the default workspace (current directory)
    let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    log::info!(
        "Starting Winx Code Agent (using workspace: {})",
        workspace.display()
    );

    // Pre-configure environment variable for InitializeParams
    std::env::set_var("WINX_WORKSPACE", workspace.to_string_lossy().to_string());

    let agent = CodeAgent::new();
    let transport = io::stdio();

    // Serve the agent with improved error handling
    match agent.serve(transport).await {
        Ok(server) => {
            log::info!("Server initialized successfully");
            match server.waiting().await {
                Ok(reason) => {
                    log::info!("Server shutdown gracefully: {:?}", reason);
                    Ok(())
                }
                Err(e) => {
                    log::error!(
                        "Server error during operation: {} (at {}:{})",
                        e,
                        file!(),
                        line!()
                    );
                    exit(1);
                }
            }
        }
        Err(e) => {
            log::error!(
                "Failed to initialize server: {} (at {}:{})",
                e,
                file!(),
                line!()
            );
            exit(1);
        }
    }
}
