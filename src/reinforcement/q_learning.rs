// Q-Learning implementation for the reinforcement learning agent
// This implements the core Q-Learning algorithm for action selection and learning

use crate::reinforcement::{
    action::AgentAction,
    state::{CodebaseState, SimplifiedCodebaseState},
    Policy,
};
use rand::Rng;
use std::collections::HashMap;

/// Q-Learning system for action selection and learning
#[derive(Debug, Clone)]
pub struct QLearningSystem {
    /// Q-table mapping state-action pairs to expected future rewards
    q_table: HashMap<(SimplifiedCodebaseState, AgentAction), f64>,
    /// Learning rate (α) - how quickly new information overrides old
    learning_rate: f64,
    /// Discount factor (γ) - importance of future rewards
    discount_factor: f64,
    /// Exploration rate (ε) - probability of taking a random action
    exploration_rate: f64,
    /// Decay rate for exploration
    exploration_decay: f64,
    /// Minimum exploration rate
    min_exploration_rate: f64,
    /// Number of training iterations
    iterations: usize,
    /// History of actions for experience replay
    action_history: Vec<(
        SimplifiedCodebaseState,
        AgentAction,
        f64,
        SimplifiedCodebaseState,
    )>,
}

impl QLearningSystem {
    /// Create a new Q-Learning system
    pub fn new(learning_rate: f64, discount_factor: f64, exploration_rate: f64) -> Self {
        Self {
            q_table: HashMap::new(),
            learning_rate,
            discount_factor,
            exploration_rate,
            exploration_decay: 0.995, // Decay exploration rate by 0.5% per iteration
            min_exploration_rate: 0.05,
            iterations: 0,
            action_history: Vec::new(),
        }
    }

    /// Get the Q-value for a state-action pair
    pub fn get_q_value(&self, state: &CodebaseState, action: &AgentAction) -> f64 {
        let simplified_state = state.to_simplified_state();
        *self
            .q_table
            .get(&(simplified_state, action.clone()))
            .unwrap_or(&0.0)
    }

    /// Update the Q-value for a state-action pair
    pub fn update_q_value(
        &mut self,
        state: &CodebaseState,
        action: &AgentAction,
        reward: f64,
        next_state: &CodebaseState,
    ) {
        // Convert to simplified states for storage
        let simplified_state = state.to_simplified_state();
        let simplified_next_state = next_state.to_simplified_state();

        // Get current Q-value
        let current_q = *self
            .q_table
            .get(&(simplified_state.clone(), action.clone()))
            .unwrap_or(&0.0);

        // Get maximum Q-value for next state
        let max_next_q = self.get_max_q_value(&simplified_next_state);

        // Q(s,a) ← Q(s,a) + α[r + γ·max_a' Q(s',a') - Q(s,a)]
        let temporal_difference = reward + self.discount_factor * max_next_q - current_q;
        let new_q = current_q + self.learning_rate * temporal_difference;

        // Update Q-table
        self.q_table
            .insert((simplified_state.clone(), action.clone()), new_q);

        // Store for experience replay
        self.action_history.push((
            simplified_state,
            action.clone(),
            reward,
            simplified_next_state,
        ));

        // Keep history at a reasonable size
        if self.action_history.len() > 1000 {
            self.action_history.remove(0);
        }

        // Update iteration counter and decay exploration rate
        self.iterations += 1;
        self.exploration_rate =
            (self.exploration_rate * self.exploration_decay).max(self.min_exploration_rate);
    }

    /// Get the maximum Q-value for a state
    fn get_max_q_value(&self, state: &SimplifiedCodebaseState) -> f64 {
        let mut max_q = 0.0;

        // Check all state-action pairs for this state
        for ((s, _), q) in self.q_table.iter() {
            if s == state && *q > max_q {
                max_q = *q;
            }
        }

        max_q
    }

    /// Get all actions with Q-values for a state
    fn get_actions_with_q_values(
        &self,
        state: &SimplifiedCodebaseState,
    ) -> Vec<(AgentAction, f64)> {
        let mut actions = Vec::new();

        for ((s, a), q) in self.q_table.iter() {
            if s == state {
                actions.push((a.clone(), *q));
            }
        }

        actions
    }

    /// Get the best action for a state
    fn get_best_action(&self, state: &SimplifiedCodebaseState) -> Option<AgentAction> {
        let actions = self.get_actions_with_q_values(state);

        if actions.is_empty() {
            return None;
        }

        actions
            .into_iter()
            .max_by(|(_, q1), (_, q2)| q1.partial_cmp(q2).unwrap())
            .map(|(a, _)| a)
    }

    /// Get all available actions for a state
    fn get_available_actions(&self, state: &CodebaseState) -> Vec<AgentAction> {
        // This would be tailored to the state in a real implementation
        // For simplicity, we return a fixed set of actions
        vec![
            AgentAction::RunTests,
            AgentAction::RunBuild,
            AgentAction::ExecuteCommand("ls -la".to_string()),
            AgentAction::ReadFile(state.current_dir.clone()),
            AgentAction::AnalyzeCode(state.current_dir.clone()),
            AgentAction::NoOp,
        ]
    }

    /// Perform experience replay to improve learning
    pub fn experience_replay(&mut self, batch_size: usize) {
        // Skip if not enough experiences
        if self.action_history.len() < batch_size {
            return;
        }

        // Select random experiences from history
        let mut rng = rand::rng();
        let history_len = self.action_history.len();

        for _ in 0..batch_size {
            let index = rng.random_range(0..history_len);
            let (state, action, reward, next_state) = &self.action_history[index];

            // Get current Q-value
            let current_q = *self
                .q_table
                .get(&(state.clone(), action.clone()))
                .unwrap_or(&0.0);

            // Get maximum Q-value for next state
            let max_next_q = self.get_max_q_value(next_state);

            // Q(s,a) ← Q(s,a) + α[r + γ·max_a' Q(s',a') - Q(s,a)]
            let temporal_difference = reward + self.discount_factor * max_next_q - current_q;
            let new_q = current_q + self.learning_rate * temporal_difference;

            // Update Q-table
            self.q_table.insert((state.clone(), action.clone()), new_q);
        }
    }
}

impl Policy for QLearningSystem {
    fn action_probability(&self, state: &CodebaseState, action: &AgentAction) -> f64 {
        let simplified_state = state.to_simplified_state();
        let available_actions = self.get_available_actions(state);

        // If the action is not available, return 0
        if !available_actions.contains(action) {
            return 0.0;
        }

        // Epsilon-greedy policy
        let best_action = self.get_best_action(&simplified_state);

        if let Some(best) = best_action {
            if &best == action {
                // With probability (1-ε), choose the best action
                1.0 - self.exploration_rate
                    + (self.exploration_rate / available_actions.len() as f64)
            } else {
                // With probability ε, choose a random action
                self.exploration_rate / available_actions.len() as f64
            }
        } else {
            // No best action known, choose randomly
            1.0 / available_actions.len() as f64
        }
    }

    fn select_action(&self, state: &CodebaseState) -> AgentAction {
        let simplified_state = state.to_simplified_state();
        let available_actions = self.get_available_actions(state);

        if available_actions.is_empty() {
            return AgentAction::NoOp;
        }

        let mut rng = rand::rng();

        // Exploration: with probability ε, choose a random action
        if rng.random::<f64>() < self.exploration_rate {
            let index = rng.random_range(0..available_actions.len());
            return available_actions[index].clone();
        }

        // Exploitation: choose the best action
        if let Some(action) = self.get_best_action(&simplified_state) {
            // Check if the action is available
            if available_actions.contains(&action) {
                return action;
            }
        }

        // Fall back to a random action if no best action is known
        let index = rng.random_range(0..available_actions.len());
        available_actions[index].clone()
    }
}

impl Default for QLearningSystem {
    fn default() -> Self {
        Self::new(0.1, 0.9, 0.2)
    }
}
