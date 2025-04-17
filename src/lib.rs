// Winx - A performant code agent written in Rust
// Based on the WCGW architecture but optimized for performance

pub mod commands;
pub mod core;
pub mod diff;
pub mod integrations;
pub mod sql;
pub mod thinking;
pub mod utils;

use anyhow::Result;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Initialize the Winx agent
pub fn init() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Initializing Winx agent v{}", version());
    Ok(())
}
