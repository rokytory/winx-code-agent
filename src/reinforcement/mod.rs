// Main module for Reinforcement Learning components
// This module implements the RL system for winx-code-agent as described in design docs

pub mod action;
pub mod bellman;
pub mod q_learning;
pub mod reward;
pub mod state;
pub mod tool_selection;

// Re-export main components for easier access
pub use action::AgentAction;
pub use q_learning::QLearningSystem;
pub use reward::calculate_reward;
pub use state::CodebaseState;
pub use tool_selection::AdaptiveToolSystem;

use crate::WinxResult;

/// Initialize the reinforcement learning system
pub fn initialize_rl_system() -> WinxResult<AdaptiveToolSystem> {
    // Create a new Q-Learning system with default parameters
    let q_learning = QLearningSystem::new(
        0.1, // learning_rate (alpha)
        0.9, // discount_factor (gamma)
        0.2, // exploration_rate (epsilon)
    );

    // Create and return the adaptive tool system
    Ok(AdaptiveToolSystem::new(q_learning))
}

/// Trait defining the interface for a reinforcement learning policy
pub trait Policy {
    /// Returns the probability of taking an action given a state
    fn action_probability(&self, state: &CodebaseState, action: &AgentAction) -> f64;

    /// Selects the best action for a given state
    fn select_action(&self, state: &CodebaseState) -> AgentAction;
}
