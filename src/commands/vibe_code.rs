use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::code::vibe_agent::VibeAgent;
use crate::core::state::SharedState;

/// Request for initializing the VibeCode agent
#[derive(Debug, Serialize, Deserialize)]
pub struct InitVibeCodeRequest {
    /// Project directory
    pub project_dir: String,
}

/// Request for analyzing a specific file
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeFileRequest {
    /// File path
    pub file_path: String,
}

/// Request for applying search/replace to a file
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchReplaceRequest {
    /// File path
    pub file_path: String,
    /// Search/replace blocks
    pub search_replace_blocks: String,
}

/// Request for generating code suggestions
#[derive(Debug, Serialize, Deserialize)]
pub struct CodeSuggestionsRequest {
    /// File path
    pub file_path: String,
}

/// Initialize the VibeCode agent with a project directory
pub async fn init_vibe_code(state: &SharedState, json_str: &str) -> Result<String> {
    debug!("Initializing VibeCode agent from JSON: {}", json_str);

    // Parse the request
    let request: InitVibeCodeRequest =
        serde_json::from_str(json_str).with_context(|| "Failed to parse init_vibe_code request")?;

    // Create and initialize the VibeCode agent
    let mut agent = create_vibe_agent(state.clone())?;
    agent.initialize(&request.project_dir).await?;

    // Store the agent in state
    {
        let mut state_guard = state.lock().unwrap();
        state_guard.set_context_data("vibe_agent", agent);
    }

    info!(
        "VibeCode agent initialized with project: {}",
        request.project_dir
    );

    // Generate a project overview
    let agent = get_vibe_agent(state)?;
    let overview = agent
        .get_project_overview()
        .unwrap_or_else(|_| "Project overview not available.".to_string());

    Ok(format!(
        "VibeCode agent successfully initialized with project: {}\n\n{}",
        request.project_dir, overview
    ))
}

/// Analyze a specific file using the VibeCode agent
pub async fn analyze_file(state: &SharedState, json_str: &str) -> Result<String> {
    debug!(
        "Analyzing file using VibeCode agent from JSON: {}",
        json_str
    );

    // Parse the request
    let request: AnalyzeFileRequest =
        serde_json::from_str(json_str).with_context(|| "Failed to parse analyze_file request")?;

    // Get the VibeCode agent from state
    let mut agent = get_vibe_agent_mut(state)?;

    // Analyze the file
    let file_info = agent.get_file_info(&request.file_path)?;

    Ok(format!("File analysis result:\n\n{}", file_info))
}

/// Apply a search/replace operation to a file using the VibeCode agent
pub async fn apply_search_replace(state: &SharedState, json_str: &str) -> Result<String> {
    debug!(
        "Applying search/replace using VibeCode agent from JSON: {}",
        json_str
    );

    // Parse the request
    let request: SearchReplaceRequest =
        serde_json::from_str(json_str).with_context(|| "Failed to parse search_replace request")?;

    // Get the VibeCode agent from state
    let mut agent = get_vibe_agent_mut(state)?;

    // Check if the file can be edited
    if !agent.can_edit_file(&request.file_path)? {
        let unread_ranges = agent.get_unread_ranges(&request.file_path)?;

        if !unread_ranges.is_empty() {
            let ranges_str = unread_ranges
                .iter()
                .map(|(start, end)| format!("{}-{}", start, end))
                .collect::<Vec<_>>()
                .join(", ");

            return Err(anyhow!(
                "File {} hasn't been fully read. Please read the following line ranges first: {}",
                request.file_path,
                ranges_str
            ));
        } else {
            return Err(anyhow!(
                "File {} has changed since it was last read. Please read it again before editing.",
                request.file_path
            ));
        }
    }

    // Apply the search/replace
    let result = agent
        .apply_search_replace(&request.file_path, &request.search_replace_blocks)
        .await?;

    // Generate a report
    let report = agent.get_search_replace().generate_report(&result);

    Ok(format!(
        "Search/replace operation applied to {}.\n\n{}",
        request.file_path, report
    ))
}

/// Generate code suggestions for a file using the VibeCode agent
pub async fn generate_code_suggestions(state: &SharedState, json_str: &str) -> Result<String> {
    debug!(
        "Generating code suggestions using VibeCode agent from JSON: {}",
        json_str
    );

    // Parse the request
    let request: CodeSuggestionsRequest = serde_json::from_str(json_str)
        .with_context(|| "Failed to parse code_suggestions request")?;

    // Get the VibeCode agent from state
    let agent = get_vibe_agent(state)?;

    // Generate code suggestions
    let suggestions = agent.generate_code_suggestions(&request.file_path).await?;

    if suggestions.is_empty() {
        return Ok(format!(
            "No specific code suggestions available for {}.",
            request.file_path
        ));
    }

    Ok(format!(
        "Code suggestions for {}:\n\n{}",
        request.file_path,
        suggestions.join("\n")
    ))
}

/// Create a new VibeCode agent
fn create_vibe_agent(state: SharedState) -> Result<VibeAgent> {
    Ok(VibeAgent::new(state))
}

/// Get the VibeCode agent from state
fn get_vibe_agent(state: &SharedState) -> Result<VibeAgent> {
    let state_guard = state.lock().unwrap();

    // Get agent from context data
    let agent_ref = state_guard
        .get_context_data::<VibeAgent>("vibe_agent")
        .ok_or_else(|| anyhow!("VibeCode agent not initialized. Call init_vibe_code first."))?;

    // Clone the agent to avoid returning a reference to state_guard
    Ok((*agent_ref).clone())
}

/// Get a mutable reference to the VibeCode agent from state
fn get_vibe_agent_mut(state: &SharedState) -> Result<VibeAgent> {
    let state_guard = state.lock().unwrap();

    let agent = state_guard
        .get_context_data::<VibeAgent>("vibe_agent")
        .ok_or_else(|| anyhow!("VibeCode agent not initialized. Call init_vibe_code first."))?;

    // Clone the agent instead of returning a reference to state_guard
    Ok((*agent).clone())
}
