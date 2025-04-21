// Q-Learning implementation for the reinforcement learning agent
// This implements the core Q-Learning algorithm, which learns optimal action-value
// functions (Q-values) through experience and uses them for intelligent action selection

use crate::reinforcement::{
    action::AgentAction,
    state::{CodebaseState, SimplifiedCodebaseState},
    Policy,
};
use rand::Rng;
use std::collections::HashMap;

/// Q-Learning system for action selection and value function learning
///
/// Q-Learning is a model-free reinforcement learning algorithm that learns
/// the expected utility (Q-value) of taking a given action in a given state.
/// It updates these values based on observed rewards and uses them to guide
/// future decision making.
#[derive(Debug, Clone)]
pub struct QLearningSystem {
    /// Q-table mapping state-action pairs to expected future rewards
    q_table: HashMap<(SimplifiedCodebaseState, AgentAction), f64>,
    /// Learning rate (α) - controls how quickly new information overrides old values
    /// Higher values (closer to 1) emphasize recent experiences more strongly
    learning_rate: f64,

    /// Discount factor (γ) - determines the importance of future rewards
    /// Values closer to 1 make the agent more forward-looking and strategic
    discount_factor: f64,

    /// Exploration rate (ε) - probability of taking a random action instead of the best known action
    /// Balances exploration (trying new actions) vs. exploitation (using known good actions)
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
    /// Creates a new Q-Learning system with specified hyperparameters
    ///
    /// @param learning_rate - Alpha (α) value between 0-1 controlling learning speed
    /// @param discount_factor - Gamma (γ) value between 0-1 weighting future rewards
    /// @param exploration_rate - Epsilon (ε) value between 0-1 controlling exploration probability
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

    /// Gets the current Q-value (expected future reward) for a state-action pair
    /// Returns 0.0 if the state-action pair hasn't been encountered before
    pub fn get_q_value(&self, state: &CodebaseState, action: &AgentAction) -> f64 {
        let simplified_state = state.to_simplified_state();
        *self
            .q_table
            .get(&(simplified_state, action.clone()))
            .unwrap_or(&0.0)
    }

    /// Updates the Q-value for a state-action pair using the Q-learning update rule
    ///
    /// Implements the core Q-learning equation:
    /// Q(s,a) ← Q(s,a) + α[r + γ·max_a' Q(s',a') - Q(s,a)]
    ///
    /// Where:
    /// - α is the learning rate
    /// - r is the immediate reward
    /// - γ is the discount factor
    /// - max_a' Q(s',a') is the maximum Q-value for the next state
    /// - Q(s,a) is the current Q-value
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

        // Calculate the temporal difference (TD) error: r + γ·max_a' Q(s',a') - Q(s,a)
        // This represents the difference between the current Q-value estimate
        // and the new estimate based on the observed reward and next state
        let temporal_difference = reward + self.discount_factor * max_next_q - current_q;

        // Update the Q-value using the learning rate to control step size
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

    /// Gets the maximum Q-value for any action in a given state
    /// This identifies the value of the best known action from this state
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

    /// Gets all actions that have known Q-values for a given state
    /// Returns pairs of (action, q-value) for all actions attempted in this state
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

    /// Gets the action with the highest Q-value for a given state
    /// This represents the current best known action for exploitation
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

    /// Gets all actions that can potentially be taken from a given state
    /// This simplified implementation returns a fixed set of actions,
    /// but a complete implementation would analyze the state to determine
    /// contextually appropriate actions
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

    /// Performs experience replay to improve learning stability and efficiency
    ///
    /// Experience replay randomly samples past experiences and re-learns from them,
    /// which helps break correlations between sequential experiences and allows
    /// the agent to learn more efficiently from historical data.
    ///
    /// @param batch_size - Number of past experiences to replay
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

        // If the action is not available in this state, its probability is zero
        if !available_actions.contains(action) {
            return 0.0;
        }

        // Implement epsilon-greedy policy:
        // - With probability (1-ε), select the best known action
        // - With probability ε, select randomly from all available actions
        let best_action = self.get_best_action(&simplified_state);

        if let Some(best) = best_action {
            if &best == action {
                // This is the best action:
                // Probability = (1-ε) [exploitation probability]
                //            + (ε/|A|) [chance of selecting it randomly]
                1.0 - self.exploration_rate
                    + (self.exploration_rate / available_actions.len() as f64)
            } else {
                // This is not the best action, only selected during exploration
                // Probability = ε/|A| [random selection during exploration]
                self.exploration_rate / available_actions.len() as f64
            }
        } else {
            // No best action is known yet (no prior experience in this state)
            // Default to uniform random selection among all available actions
            1.0 / available_actions.len() as f64
        }
    }

    fn select_action(&self, state: &CodebaseState) -> AgentAction {
        let simplified_state = state.to_simplified_state();
        let available_actions = self.get_available_actions(state);

        // If no actions are available, return NoOp as a safe default
        if available_actions.is_empty() {
            return AgentAction::NoOp;
        }

        let mut rng = rand::rng();

        // Exploration phase: with probability ε, choose a random action
        // This ensures the agent continues to explore the environment
        if rng.random::<f64>() < self.exploration_rate {
            let index = rng.random_range(0..available_actions.len());
            return available_actions[index].clone();
        }

        // Exploitation phase: choose the action with highest Q-value
        // This leverages the agent's learned knowledge
        if let Some(action) = self.get_best_action(&simplified_state) {
            // Verify that the best action is currently available
            // (State simplification might lose some context)
            if available_actions.contains(&action) {
                return action;
            }
        }

        // Fallback: if no best action is known or available,
        // select randomly (similar to pure exploration)
        let index = rng.random_range(0..available_actions.len());
        available_actions[index].clone()
    }
}

impl Default for QLearningSystem {
    fn default() -> Self {
        // Default hyperparameters:
        // - learning_rate (α) = 0.1: Moderate learning speed
        // - discount_factor (γ) = 0.9: Strong emphasis on future rewards
        // - exploration_rate (ε) = 0.2: 20% exploration, 80% exploitation
        Self::new(0.1, 0.9, 0.2)
    }
}
