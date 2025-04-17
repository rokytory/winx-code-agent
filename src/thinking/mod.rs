pub mod sequential;
pub mod task_adherence;

pub use sequential::*;
pub use task_adherence::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};
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

/// Parameters for a task adherence check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAdherenceCheck {
    /// Original task description
    pub original_task: String,
    /// Current progress description
    pub current_progress: String,
    /// Additional context (optional)
    pub additional_context: Option<String>,
    /// Type of adherence check
    pub check_type: TaskAdherenceCheckType,
}

/// Type of task adherence check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskAdherenceCheckType {
    /// Check if the task is still on track
    TaskAdherence,
    /// Check if the task is completed
    Completion,
    /// Check what additional information is needed
    InformationNeeds,
}

/// Process a task adherence check
pub async fn process_task_adherence_check(state: &SharedState, check_json: &str) -> Result<String> {
    debug!("Processing task adherence check: {}", check_json);

    // Parse the check JSON
    let check: TaskAdherenceCheck = serde_json::from_str(check_json)?;

    // Prepare parameters
    let params = TaskAdherenceParams {
        original_task: check.original_task.clone(),
        current_progress: check.current_progress.clone(),
        additional_context: check.additional_context.clone(),
    };

    // Execute the appropriate check
    let evaluation = match check.check_type {
        TaskAdherenceCheckType::TaskAdherence => evaluate_task_adherence(state, params).await?,
        TaskAdherenceCheckType::Completion => evaluate_task_completion(state, params).await?,
        TaskAdherenceCheckType::InformationNeeds => {
            evaluate_information_needs(state, params).await?
        }
    };

    // Format the response
    let status_str = match evaluation.status {
        TaskAdherenceStatus::OnTrack => "🟢 On Track",
        TaskAdherenceStatus::Deviated => "🟠 Deviated",
        TaskAdherenceStatus::Completed => "✅ Completed",
        TaskAdherenceStatus::Abandoned => "⛔ Abandoned",
    };

    let mut response = format!(
        "Task Adherence Evaluation: {}\n\nAssessment: {}",
        status_str, evaluation.assessment
    );

    if let Some(recommendations) = evaluation.recommendations {
        response.push_str(&format!("\n\nRecommendations: {}", recommendations));
    }

    if evaluation.needs_redirection {
        response.push_str("\n\n⚠️ The task appears to need redirection to get back on track.");
    }

    Ok(response)
}
