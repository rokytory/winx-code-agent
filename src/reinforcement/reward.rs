// Reward system for the reinforcement learning agent
// This calculates rewards based on the state transitions and actions

use crate::reinforcement::{
    action::{ActionResult, AgentAction},
    state::CodebaseState,
};

/// Calculate the reward for a state transition
pub fn calculate_reward(
    previous_state: &CodebaseState,
    action: &AgentAction,
    current_state: &CodebaseState,
    action_result: &ActionResult,
) -> f64 {
    let mut reward = 0.0;

    // Base reward/penalty based on action result
    match action_result {
        ActionResult::Success(_) => reward += 1.0,
        ActionResult::Failure(_) => reward -= 1.0,
        ActionResult::Neutral => {}
        ActionResult::FileContent(_) => {} // Reading content is neutral
    }

    // Reward for fixing errors
    if current_state.syntax_errors.len() < previous_state.syntax_errors.len() {
        let errors_fixed = previous_state.syntax_errors.len() - current_state.syntax_errors.len();
        reward += 5.0 * errors_fixed as f64;
    }

    // Penalty for introducing errors
    if current_state.syntax_errors.len() > previous_state.syntax_errors.len() {
        let errors_introduced =
            current_state.syntax_errors.len() - previous_state.syntax_errors.len();
        reward -= 5.0 * errors_introduced as f64;
    }

    // Reward for improving build status
    if current_state.build_status == crate::reinforcement::state::BuildStatus::Success
        && previous_state.build_status != crate::reinforcement::state::BuildStatus::Success
    {
        reward += 10.0;
    }

    // Penalty for breaking build
    if current_state.build_status != crate::reinforcement::state::BuildStatus::Success
        && previous_state.build_status == crate::reinforcement::state::BuildStatus::Success
    {
        reward -= 10.0;
    }

    // Reward for improving test coverage
    if current_state.test_coverage > previous_state.test_coverage {
        reward += (current_state.test_coverage - previous_state.test_coverage) * 0.5;
    }

    // Action-specific adjustments
    match action {
        AgentAction::RunTests => {
            // Small bonus for running tests (we want to encourage this)
            reward += 0.5;
        }

        AgentAction::RunBuild => {
            // Small bonus for running build (we want to encourage this)
            reward += 0.5;
        }

        AgentAction::AnalyzeCode(_) => {
            // Small bonus for code analysis
            reward += 0.5;
        }

        AgentAction::ExecuteCommand(_) => {
            // Small cost for execution to encourage efficiency
            reward -= 0.1;
        }

        AgentAction::NoOp => {
            // Penalty for doing nothing
            reward -= 0.5;
        }

        _ => {}
    }

    reward
}

/// Reward modifiers based on user feedback
pub enum UserFeedbackRating {
    /// User is very satisfied with the action
    Positive,
    /// User is neutral about the action
    Neutral,
    /// User is dissatisfied with the action
    Negative,
}

/// Process user feedback and convert it to a reward
pub fn process_user_feedback(rating: UserFeedbackRating) -> f64 {
    match rating {
        UserFeedbackRating::Positive => 10.0,
        UserFeedbackRating::Neutral => 0.0,
        UserFeedbackRating::Negative => -10.0,
    }
}

/// Feedback from the user about an action
pub struct UserFeedback {
    /// The state and action that received feedback
    pub context: (CodebaseState, AgentAction),
    /// The user's rating
    pub rating: UserFeedbackRating,
    /// Optional comment from the user
    pub comment: Option<String>,
}

/// Learn from user feedback
pub fn learn_from_feedback(feedback: &UserFeedback) -> f64 {
    process_user_feedback(match feedback.rating {
        UserFeedbackRating::Positive => UserFeedbackRating::Positive,
        UserFeedbackRating::Neutral => UserFeedbackRating::Neutral,
        UserFeedbackRating::Negative => UserFeedbackRating::Negative,
    })
}
