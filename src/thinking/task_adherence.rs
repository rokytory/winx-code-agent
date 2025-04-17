use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::state::SharedState;

/// Parameters for task adherence evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAdherenceParams {
    /// Original task description
    pub original_task: String,
    /// Current progress description
    pub current_progress: String,
    /// Additional context (optional)
    pub additional_context: Option<String>,
}

/// Status of task adherence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskAdherenceStatus {
    /// Task is on track
    OnTrack,
    /// Task has deviated from the original goal
    Deviated,
    /// Task has been completed
    Completed,
    /// Task has been abandoned
    Abandoned,
}

/// Result of a task adherence evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAdherenceEvaluation {
    /// Status of the task
    pub status: TaskAdherenceStatus,
    /// Assessment of the current progress
    pub assessment: String,
    /// Recommendations for how to proceed (if any)
    pub recommendations: Option<String>,
    /// Whether the task needs redirection
    pub needs_redirection: bool,
}

/// Analyzes how to collect additional information for a task
pub async fn evaluate_information_needs(
    _state: &SharedState,
    params: TaskAdherenceParams,
) -> Result<TaskAdherenceEvaluation> {
    // In a real implementation, this would analyze what information is needed
    // For now, we'll provide a simple implementation
    Ok(TaskAdherenceEvaluation {
        status: TaskAdherenceStatus::OnTrack,
        assessment: format!("Analyzing information needs for: {}", params.original_task),
        recommendations: Some(String::from(
            "Consider collecting more context about the task requirements.",
        )),
        needs_redirection: false,
    })
}

/// Evaluates whether the task execution is still on track
pub async fn evaluate_task_adherence(
    _state: &SharedState,
    params: TaskAdherenceParams,
) -> Result<TaskAdherenceEvaluation> {
    // In a real implementation, this would compare current progress to the task
    // For now, we'll provide a simple implementation
    let is_on_track = params.current_progress.contains(&params.original_task);

    Ok(TaskAdherenceEvaluation {
        status: if is_on_track {
            TaskAdherenceStatus::OnTrack
        } else {
            TaskAdherenceStatus::Deviated
        },
        assessment: if is_on_track {
            format!(
                "Task execution is aligned with the original goal: {}",
                params.original_task
            )
        } else {
            format!(
                "Task may have deviated from the original goal: {}",
                params.original_task
            )
        },
        recommendations: if is_on_track {
            None
        } else {
            Some(String::from(
                "Consider reviewing the original task objectives.",
            ))
        },
        needs_redirection: !is_on_track,
    })
}

/// Evaluates whether the task has been completed
pub async fn evaluate_task_completion(
    _state: &SharedState,
    params: TaskAdherenceParams,
) -> Result<TaskAdherenceEvaluation> {
    // In a real implementation, this would determine if the task is complete
    // For now, we'll provide a simple implementation
    let completion_keywords = ["completed", "done", "finished", "implemented"];
    let is_completed = completion_keywords
        .iter()
        .any(|&keyword| params.current_progress.to_lowercase().contains(keyword));

    Ok(TaskAdherenceEvaluation {
        status: if is_completed {
            TaskAdherenceStatus::Completed
        } else {
            TaskAdherenceStatus::OnTrack
        },
        assessment: if is_completed {
            String::from("Task appears to be completed based on progress description.")
        } else {
            String::from("Task is still in progress.")
        },
        recommendations: if is_completed {
            Some(String::from(
                "Verify all requirements have been met and consider any follow-up tasks.",
            ))
        } else {
            Some(String::from("Continue with the current approach."))
        },
        needs_redirection: false,
    })
}
