use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

use crate::core::state::SharedState;
use crate::thinking::{SequentialThinking, Thought};

// Singleton pattern com inicialização segura usando lazy_static
lazy_static::lazy_static! {
    static ref THINKING_PROCESS: Arc<Mutex<SequentialThinking>> = Arc::new(Mutex::new(SequentialThinking::new()));
}

/// Get the thinking process singleton
fn get_thinking_process() -> Arc<Mutex<SequentialThinking>> {
    THINKING_PROCESS.clone()
}

/// Parameters for processing a thinking step
pub struct ThinkingParams<'a> {
    pub state: &'a SharedState,
    pub thought_content: &'a str,
    pub thought_number: usize,
    pub total_thoughts: usize,
    pub next_thought_needed: bool,
    pub is_revision: Option<bool>,
    pub revises_thought: Option<usize>,
    pub branch_from_thought: Option<usize>,
    pub branch_id: Option<String>,
    pub needs_more_thoughts: Option<bool>,
}

/// Process a sequential thinking step
pub async fn process_thinking(params: ThinkingParams<'_>) -> Result<String> {
    let ThinkingParams {
        thought_content,
        thought_number,
        total_thoughts,
        next_thought_needed,
        is_revision,
        revises_thought,
        branch_from_thought,
        branch_id,
        needs_more_thoughts,
        ..
    } = params;
    debug!(
        "Processing thought #{}: {}",
        thought_number, thought_content
    );

    // Create a new thought
    let thought = Thought {
        content: thought_content.to_string(),
        thought_number,
        total_thoughts,
        next_thought_needed,
        is_revision: is_revision.unwrap_or(false),
        revises_thought,
        branch_from_thought,
        branch_id,
        needs_more_thoughts: needs_more_thoughts.unwrap_or(false),
    };

    // Add the thought to the thinking process
    let thinking_process = get_thinking_process();
    let mut process = thinking_process.lock().unwrap();
    process.add_thought(thought)?;

    // Get a summary of the thinking process
    let summary = process.get_summary();

    info!(
        "Thought processing completed: {} total thoughts",
        process.get_thoughts().len()
    );
    Ok(summary)
}

/// Process sequential thinking from a JSON request
pub async fn process_sequential_thinking(_state: &SharedState, json_str: &str) -> Result<String> {
    debug!("Processing sequential thinking from JSON: {}", json_str);

    // Parse the JSON request
    let json: Value =
        serde_json::from_str(json_str).context("Failed to parse sequential thinking JSON")?;

    // Extract the thought content
    let thought = json
        .get("thought")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'thought' in JSON"))?;

    // Extract other parameters with defaults
    let thought_number = json
        .get("thoughtNumber")
        .and_then(Value::as_u64)
        .unwrap_or(1) as usize;

    let total_thoughts = json
        .get("totalThoughts")
        .and_then(Value::as_u64)
        .unwrap_or(1) as usize;

    let next_thought_needed = json
        .get("nextThoughtNeeded")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let is_revision = json.get("isRevision").and_then(Value::as_bool);

    let revises_thought = json
        .get("revisesThought")
        .and_then(Value::as_u64)
        .map(|v| v as usize);

    let branch_from_thought = json
        .get("branchFromThought")
        .and_then(Value::as_u64)
        .map(|v| v as usize);

    let branch_id = json
        .get("branchId")
        .and_then(Value::as_str)
        .map(String::from);

    let needs_more_thoughts = json.get("needsMoreThoughts").and_then(Value::as_bool);

    // Process the thought using the existing functionality
    process_thinking(ThinkingParams {
        state: _state,
        thought_content: thought,
        thought_number,
        total_thoughts,
        next_thought_needed,
        is_revision,
        revises_thought,
        branch_from_thought,
        branch_id,
        needs_more_thoughts,
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{state::create_shared_state, types::ModeType};
    use tempfile::tempdir;
    use tokio::runtime::Runtime;

    #[test]
    fn test_sequential_thinking() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let temp_dir = tempdir().unwrap();
            let state = create_shared_state(temp_dir.path(), ModeType::Wcgw, None, None).unwrap();

            // Para testes, criamos um novo processo de thinking local
            let _thinking_process = Arc::new(Mutex::new(SequentialThinking::new()));

            // Usamos o thinking process global para os testes
            // Observe que em uma aplicação real, seria melhor ter um processo de thinking por sessão

            // Add some thoughts
            let result = process_thinking(ThinkingParams {
                state: &state,
                thought_content: "This is the first thought",
                thought_number: 1,
                total_thoughts: 3,
                next_thought_needed: true,
                is_revision: None,
                revises_thought: None,
                branch_from_thought: None,
                branch_id: None,
                needs_more_thoughts: None,
            })
            .await
            .unwrap();

            assert!(result.contains("Thought #1"));

            let result = process_thinking(ThinkingParams {
                state: &state,
                thought_content: "This is the second thought",
                thought_number: 2,
                total_thoughts: 3,
                next_thought_needed: true,
                is_revision: None,
                revises_thought: None,
                branch_from_thought: None,
                branch_id: None,
                needs_more_thoughts: None,
            })
            .await
            .unwrap();

            assert!(result.contains("Thought #1"));
            assert!(result.contains("Thought #2"));

            let result = process_thinking(ThinkingParams {
                state: &state,
                thought_content: "This is the final thought",
                thought_number: 3,
                total_thoughts: 3,
                next_thought_needed: false,
                is_revision: None,
                revises_thought: None,
                branch_from_thought: None,
                branch_id: None,
                needs_more_thoughts: None,
            })
            .await
            .unwrap();

            assert!(result.contains("Thought #1"));
            assert!(result.contains("Thought #2"));
            assert!(result.contains("Thought #3"));
        });
    }
}
