use anyhow::Result;
use rmcp::{transport::io, ServiceExt};
use std::process::exit;
use winx_code_agent::server::CodeAgent;

mod logging;

// Função removida pois não estava sendo utilizada

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize enhanced logging
    logging::init_logging();

    log::info!("Starting Winx Code Agent");

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
