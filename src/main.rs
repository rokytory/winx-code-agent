use anyhow::{Context, Result};
use rmcp::ServiceExt;
use std::env;
use std::path::PathBuf;
use tracing::{error, info};

use winx::{
    commands::tools::WinxTools,
    core::{state, types::ModeType},
};

/// Simple main function without custom transport wrapper
/// This will use the default RMCP transport but with ANSI stripping
/// configured through environment variables
#[tokio::main]
async fn main() -> Result<()> {
    // Configure environment variables for debugging if not already defined
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "rmcp=trace,winx=trace");
    }

    // ANSI COLOR DISABLING SECTION
    // ---------------------------
    // Be extremely aggressive about disabling all color codes in all output

    // General color disabling env vars - highest priority
    env::set_var("NO_COLOR", "1"); // Respected by many modern CLI tools
    env::set_var("CLICOLOR", "0"); // Used by BSD/macOS tools
    env::set_var("CLICOLOR_FORCE", "0"); // Override forced colors
    env::set_var("TERM", "dumb"); // Old-school way to disable colors
    env::set_var("COLORTERM", "0"); // Another terminal color control

    // Rust-specific color controls
    env::set_var("RUST_LOG_STYLE", "never"); // Controls tracing crate style
    env::set_var("RUST_LOG_COLOR", "never"); // Another tracing control
    env::set_var("RUST_LOG_FORMAT", "json"); // Prevent colored formatter
    env::set_var("RUST_BACKTRACE_COLOR", "never"); // Disable colored backtraces
    env::set_var("RUST_TEST_COLOR", "never"); // Disable test colors
    env::set_var("CARGO_TERM_COLOR", "never"); // Disable cargo colors

    // LSP-specific environment variables
    env::set_var("TS_NODE_PRETTY", "false"); // TypeScript LSP
    env::set_var("RUST_ANALYZER_LOG", "error"); // Reduce rust-analyzer output
    env::set_var("RUST_ANALYZER_COLOR", "never"); // Disable rust-analyzer colors
    env::set_var("PYRIGHT_PYTHON_DEBUG", "0"); // Turn off pyright debug
    env::set_var("PYRIGHT_NO_COLOR", "1"); // Turn off pyright colors

    // Node.js color controls (for JavaScript/TypeScript LSP)
    env::set_var("NODE_DISABLE_COLORS", "1"); // Disable Node.js colors
    env::set_var("FORCE_COLOR", "0"); // Another Node.js color control

    // Force plain output for all child processes
    env::set_var("FORCE_PLAIN_OUTPUT", "1"); // Custom var for our own usage

    info!("All color codes disabled via environment variables");

    // Parse command-line arguments using a simple approach that handles flags
    let args: Vec<String> = std::env::args().collect();
    let workspace_path = if args.len() > 1 {
        // Check if the argument is a flag (starts with -)
        if args[1].starts_with('-') {
            // If it's --help or -h, show help and exit
            if args[1] == "--help" || args[1] == "-h" {
                println!("Usage: winx [WORKSPACE_PATH]");
                println!("If no workspace path is provided, the current directory will be used.");
                println!("\nOptions:");
                println!("  -h, --help    Show this help message and exit");
                println!("  --version     Show version information and exit");
                std::process::exit(0);
            } else if args[1] == "--version" {
                println!("Winx version {}", winx::version());
                std::process::exit(0);
            } else {
                // Unrecognized flag
                eprintln!("Error: Unrecognized option: {}", args[1]);
                eprintln!("Try 'winx --help' for more information.");
                std::process::exit(1);
            }
        }

        // Not a flag, treat as workspace path
        PathBuf::from(&args[1])
    } else {
        std::env::current_dir()?
    };

    // Initialize with ANSI colors explicitly disabled for MCP compatibility
    // and provide workspace path for terminal and memory components
    winx::init_with_logger(false)?;
    winx::init_with_workspace(&workspace_path.to_string_lossy())
        .context("Failed to initialize Winx agent")?;

    // Initialize plugins in the existing async context
    winx::init_plugins_async(&workspace_path.to_string_lossy())
        .await
        .context("Failed to initialize plugins")?;
        
    // Get state for auto-initialization of file tracking
    let state = state::create_shared_state(
        workspace_path.clone(), 
        ModeType::Wcgw, 
        None, 
        None
    ).context("Failed to create state for file tracking")?;
    
    // Auto-initialize important project files
    winx::init_file_tracking(&state, &[]).await
        .context("Failed to initialize file tracking")?;

    // Log version and environment information
    info!(
        "Starting Winx v{} on {}",
        winx::version(),
        std::env::consts::OS
    );

    info!("Using workspace path: {}", workspace_path.display());

    // Test ANSI stripping and JSON sanitization
    let test_string = "\u{001B}[2m2025-04-17T08:08:46.729Z [winx] [info] Server started and connected successfully\u{001B}[0m";
    let sanitized = winx::strip_ansi_codes(test_string);
    let json_safe = winx::sanitize_json_text(&sanitized);
    info!(
        "ANSI stripping test - Original contains escape codes: {}",
        test_string.contains('\u{001B}')
    );
    info!(
        "ANSI stripping test - Sanitized contains escape codes: {}",
        sanitized.contains('\u{001B}')
    );
    info!(
        "ANSI stripping test - JSON-safe contains escape codes: {}",
        json_safe.contains('\u{001B}')
    );

    // Initialize state with wcgw mode and any stored task information
    let state = match state::create_shared_state(workspace_path.clone(), ModeType::Wcgw, None, None) {
        Ok(state) => {
            info!("Agent state created successfully");

            // Initialize syntax validator with robust error handling
            #[cfg(feature = "syntax_validation")]
            {
                match winx::code::get_syntax_validator() {
                    Ok(_) => {
                        info!("Syntax validator initialized successfully");
                    }
                    Err(e) => {
                        info!("Syntax validator initialization failed: {}", e);
                        info!("Continuing without syntax validation");
                    }
                }
            }

            #[cfg(not(feature = "syntax_validation"))]
            {
                info!("Syntax validation feature is not enabled in this build");
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

    // Start the MCP server and keep it running until the client disconnects
    info!("Server starting...");

    // Use standard stdio transport
    info!("Starting MCP server using stdio transport with ANSI protection");

    // Start the MCP server and keep it running until the client disconnects
    info!("Server starting...");

    // Use standard transport - with our environment variables set to disable ANSI codes
    let std_transport = rmcp::transport::stdio();

    // Add error handling and detailed logging
    let client_result = tools.serve(std_transport).await;
    let client = match client_result {
        Ok(client) => {
            info!("MCP server started successfully");
            client
        }
        Err(e) => {
            eprintln!("Failed to start MCP server: {}", e);
            error!("Error starting MCP server: {}", e);

            // Attempt to log more details about the error
            if let Some(source) = std::error::Error::source(&e) {
                error!("Caused by: {}", source);
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
