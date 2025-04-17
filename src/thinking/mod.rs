pub mod sequential;

pub use sequential::*;

use anyhow::Result;
use tracing::{debug, info};

use crate::core::state::SharedState;

/// Process sequential thinking
pub async fn process_sequential_thinking(
    state: &SharedState,
    thinking_json: &str,
) -> Result<String> {
    debug!("Processing sequential thinking: {}", thinking_json);

    // Parse the thinking JSON
    let thinking: crate::commands::tools::SequentialThinking = serde_json::from_str(thinking_json)?;

    // In a real implementation, this would process the thinking and store it
    let _state_guard = state.lock().unwrap();

    info!(
        "Sequential thinking processed: thought #{}/{}",
        thinking.thought_number, thinking.total_thoughts
    );

    // Format the response
    let response = format!(
        "Thought #{}/{}:\n{}\n\nNext thought needed: {}",
        thinking.thought_number,
        thinking.total_thoughts,
        thinking.thought,
        thinking.next_thought_needed
    );

    Ok(response)
}
