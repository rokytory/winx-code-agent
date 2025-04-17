use anyhow::{Context, Result};
use rmcp::{transport, ServiceExt};
use std::env;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use tokio::io::AsyncRead;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, error, info};

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
    let state = state::create_shared_state(workspace_path.clone(), ModeType::Wcgw, None, None)
        .context("Failed to create state for file tracking")?;

    // Auto-initialize important project files
    winx::init_file_tracking(&state, &[])
        .await
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
    let state = match state::create_shared_state(workspace_path.clone(), ModeType::Wcgw, None, None)
    {
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

    // Registrar ferramentas e marcar como inicializado
    winx::commands::tools::register_tools(state.clone()).context("Failed to register tools")?;
    info!("Tools registered and initialized successfully");

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

    // Create a custom transport wrapper that filters log-like messages
    let stdio_transport = transport::stdio();

    // Create a filtering transport adapter that wraps the standard stdio transport
    //let filtering_transport = FilteringTransport::new(stdio_transport);

    // Use standard transport
    let client_result = tools.serve(stdio_transport).await;
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

// NOTE: Temporarily commented out the FilteringTransport implementation
// until the dependencies and Transport trait issues are resolved

/*
/// A custom transport wrapper that filters out log-like messages
/// which might be mistakenly parsed as JSON
struct FilteringTransport<T> {
    inner: T,
}

impl<T> FilteringTransport<T> {
    fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Check if the data looks like a log message rather than JSON
    fn is_log_message(data: &[u8]) -> bool {
        // Skip empty data
        if data.is_empty() {
            return false;
        }

        // Check for common log patterns
        let log_indicators = [" INFO ", " DEBUG ", " WARN ", " ERROR ", " TRACE "];

        // Convert the first part of the data to a string for comparison
        let max_check_len = std::cmp::min(data.len(), 20);
        if let Ok(start_str) = std::str::from_utf8(&data[..max_check_len]) {
            // Check if it starts with any of our log indicators
            for &indicator in &log_indicators {
                if start_str.contains(indicator) {
                    debug!("Filtering out log-like message: {}", start_str);
                    return true;
                }
            }
        }

        false
    }
}

/// A filtering reader that silently discards log-like messages
struct FilteringReader<R> {
    inner: R,
}

impl<R: AsyncRead + Unpin> AsyncRead for FilteringReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // Temporary buffer to check the data before passing it on
        let mut temp_buf = vec![0u8; buf.capacity()];
        let mut temp_read_buf = tokio::io::ReadBuf::new(&mut temp_buf);

        match Pin::new(&mut self.inner).poll_read(cx, &mut temp_read_buf) {
            Poll::Ready(Ok(())) => {
                let filled = temp_read_buf.filled();
                if !filled.is_empty() && !FilteringTransport::<R>::is_log_message(filled) {
                    // Only copy non-log data to the actual buffer
                    buf.put_slice(filled);
                } else if !filled.is_empty() {
                    // If it was a log message, return "no data" but mark as ready
                    // This will prevent the caller from waiting on more data
                    debug!("Filtered out log-like message ({} bytes)", filled.len());
                    // We still need to return Ready to prevent blocking
                    return Poll::Ready(Ok(()));
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
*/
