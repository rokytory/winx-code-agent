// Bellman equations implementation
// These equations form the mathematical foundation of value-based reinforcement learning methods,
// enabling the calculation of expected future rewards for state-action pairs

use crate::reinforcement::{action::AgentAction, state::CodebaseState, Policy};
use std::collections::HashMap;

/// Transition model interface for representing environment dynamics
///
/// A transition model captures how agent actions affect state changes and rewards.
/// It provides methods to calculate expected returns and transition probabilities,
/// which are essential components of the Bellman equations.
pub trait TransitionModel {
    /// Calculates the expected return (cumulative future reward) for a state-action pair
    ///
    /// This computes the expected value of taking action in state, considering all
    /// possible next states and their associated rewards, weighted by transition probabilities.
    fn expected_return(
        &self,
        state: &CodebaseState,
        action: &AgentAction,
        discount_factor: f64,
    ) -> f64;

    /// Calculates the expected return for a state-action pair following a specific policy
    ///
    /// This computes the expected value of taking action in state and then following
    /// the provided policy for all subsequent steps, using the discount factor to
    /// weight future rewards appropriately.
    fn expected_return_with_policy(
        &self,
        state: &CodebaseState,
        action: &AgentAction,
        policy: &dyn Policy,
        discount_factor: f64,
    ) -> f64;

    /// Gets the probability of transitioning to a specified next state and receiving a specific reward
    ///
    /// Returns the probability P(s',r|s,a) of reaching next_state and receiving reward
    /// after taking action in current_state.
    fn transition_probability(
        &self,
        current_state: &CodebaseState,
        action: &AgentAction,
        next_state: &CodebaseState,
        reward: f64,
    ) -> f64;
}

/// A transition model implementation that learns from historical state transitions
///
/// This model builds an empirical transition probability distribution based on
/// observed transitions from the agent's interaction history.
#[derive(Default)]
pub struct HistoricalTransitionModel {
    /// History of transitions (s, a, r, s')
    history: Vec<(CodebaseState, AgentAction, f64, CodebaseState)>,
}

impl HistoricalTransitionModel {
    /// Creates a new empty historical transition model
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a state-action-reward-nextstate transition tuple to the history
    ///
    /// These recorded transitions are used to estimate transition probabilities
    /// and expected returns based on empirical data.
    pub fn add_transition(
        &mut self,
        state: CodebaseState,
        action: AgentAction,
        reward: f64,
        next_state: CodebaseState,
    ) {
        self.history.push((state, action, reward, next_state));
    }

    /// Gets all historical transitions (reward and next state) for a specific state-action pair
    ///
    /// Returns a vector of (reward, next_state) pairs that were observed after
    /// taking action in state.
    fn get_transitions_for(
        &self,
        state: &CodebaseState,
        action: &AgentAction,
    ) -> Vec<(f64, CodebaseState)> {
        self.history
            .iter()
            .filter(|(s, a, _, _)| s == state && a == action)
            .map(|(_, _, r, s_prime)| (*r, s_prime.clone()))
            .collect()
    }
}

impl TransitionModel for HistoricalTransitionModel {
    fn expected_return(
        &self,
        state: &CodebaseState,
        action: &AgentAction,
        _discount_factor: f64,
    ) -> f64 {
        let transitions = self.get_transitions_for(state, action);

        if transitions.is_empty() {
            return 0.0;
        }

        // Calculate the average immediate reward observed for this state-action pair
        let total_reward: f64 = transitions.iter().map(|(r, _)| r).sum();

        // This is a simplified implementation that only considers immediate rewards
        // A complete implementation would also account for the expected future rewards
        // from each possible next state, weighted by their occurrence probability
        total_reward / transitions.len() as f64
    }

    fn expected_return_with_policy(
        &self,
        state: &CodebaseState,
        action: &AgentAction,
        policy: &dyn Policy,
        discount_factor: f64,
    ) -> f64 {
        let transitions = self.get_transitions_for(state, action);

        if transitions.is_empty() {
            return 0.0;
        }

        let mut total_return = 0.0;

        for (reward, next_state) in transitions.iter() {
            // For each possible next state, calculate the expected return
            // following the policy

            // Get the policy's action probabilities for the next state
            let possible_actions = get_possible_actions(next_state);
            let mut next_state_value = 0.0;

            for next_action in possible_actions {
                let action_prob = policy.action_probability(next_state, &next_action);
                let action_value = self.expected_return(next_state, &next_action, discount_factor);
                next_state_value += action_prob * action_value;
            }

            // Add the reward plus discounted next state value
            total_return += reward + discount_factor * next_state_value;
        }

        // Average over all transitions
        total_return / transitions.len() as f64
    }

    fn transition_probability(
        &self,
        current_state: &CodebaseState,
        action: &AgentAction,
        next_state: &CodebaseState,
        reward: f64,
    ) -> f64 {
        let transitions = self.get_transitions_for(current_state, action);

        if transitions.is_empty() {
            return 0.0;
        }

        // Count how many times we've seen this transition
        let matching_transitions = transitions
            .iter()
            .filter(|(r, s_prime)| (r - reward).abs() < 1e-6 && s_prime == next_state)
            .count();

        // Return the empirical probability
        matching_transitions as f64 / transitions.len() as f64
    }
}

/// Gets all possible actions that can be taken from a given state
///
/// This is a simplified implementation that returns a fixed set of actions.
/// In a complete implementation, available actions would depend on the specific
/// state properties (e.g., current file permissions, project structure).
fn get_possible_actions(state: &CodebaseState) -> Vec<AgentAction> {
    vec![
        AgentAction::RunTests,
        AgentAction::RunBuild,
        AgentAction::ExecuteCommand("ls -la".to_string()),
        AgentAction::ReadFile(state.current_dir.clone()),
        AgentAction::AnalyzeCode(state.current_dir.clone()),
        AgentAction::NoOp,
    ]
}

/// Calculates the state-value function V_π(s) using the Bellman equation
///
/// The state-value function represents the expected cumulative future reward
/// starting from state s and following policy π thereafter.
///
/// Mathematically: V_π(s) = Σ_a π(a|s) Σ_{s',r} p(s',r|s,a)[r + γV_π(s')]
///
/// where:
/// - π(a|s) is the probability of taking action a in state s under policy π
/// - p(s',r|s,a) is the probability of transitioning to state s' and receiving reward r
/// - γ is the discount factor for future rewards
pub fn state_value_function(
    state: &CodebaseState,
    policy: &dyn Policy,
    transitions: &dyn TransitionModel,
    discount_factor: f64,
    _value_function: &dyn Fn(&CodebaseState) -> f64,
) -> f64 {
    let possible_actions = get_possible_actions(state);
    let mut value = 0.0;

    for action in possible_actions {
        let action_prob = policy.action_probability(state, &action);
        let expected_return = transitions.expected_return(state, &action, discount_factor);

        value += action_prob * expected_return;
    }

    value
}

/// Calculates the action-value function Q_π(s,a) using the Bellman equation
///
/// The action-value function represents the expected cumulative future reward
/// starting from state s, taking action a, and following policy π thereafter.
///
/// Mathematically: Q_π(s,a) = Σ_{s',r} p(s',r|s,a)[r + γ Σ_{a'} π(a'|s')Q_π(s',a')]
///
/// where:
/// - p(s',r|s,a) is the probability of transitioning to state s' and receiving reward r
/// - π(a'|s') is the probability of taking action a' in state s' under policy π
/// - γ is the discount factor for future rewards
pub fn action_value_function(
    state: &CodebaseState,
    action: &AgentAction,
    policy: &dyn Policy,
    transitions: &dyn TransitionModel,
    discount_factor: f64,
    _q_function: &dyn Fn(&CodebaseState, &AgentAction) -> f64,
) -> f64 {
    transitions.expected_return_with_policy(state, action, policy, discount_factor)
}

/// Performs value iteration algorithm to compute the optimal value function
///
/// Value iteration is a dynamic programming algorithm that iteratively improves
/// the estimate of the optimal value function until convergence, allowing the
/// agent to find the policy that maximizes expected future rewards.
///
/// Parameters:
/// - states: All possible states in the environment
/// - actions: All possible actions in the environment
/// - transitions: The transition model for environment dynamics
/// - discount_factor: Weight for future rewards (between 0 and 1)
/// - theta: Convergence threshold for terminating iteration
/// - max_iterations: Maximum number of iterations to prevent infinite loops
pub fn value_iteration(
    states: &[CodebaseState],
    actions: &[AgentAction],
    transitions: &dyn TransitionModel,
    discount_factor: f64,
    theta: f64, // Convergence threshold
    max_iterations: usize,
) -> HashMap<CodebaseState, f64> {
    use std::collections::HashMap;

    // Initialize value function
    let mut values: HashMap<CodebaseState, f64> = states.iter().map(|s| (s.clone(), 0.0)).collect();

    for iteration in 0..max_iterations {
        let mut delta: f64 = 0.0;

        for state in states {
            let old_value = *values.get(state).unwrap();

            // Calculate the maximum value over all actions
            let mut max_value = f64::NEG_INFINITY;

            for action in actions {
                let mut value = 0.0;

                // For each possible next state and reward
                for next_state in states {
                    // Check for transitions with standard reward values
                    // These values should ideally be derived from a reward model or configuration
                    for &reward in &[0.0, 1.0, -1.0, 5.0, -5.0, 10.0, -10.0] {
                        let prob =
                            transitions.transition_probability(state, action, next_state, reward);

                        if prob > 0.0 {
                            let next_value = *values.get(next_state).unwrap();
                            value += prob * (reward + discount_factor * next_value);
                        }
                    }
                }

                max_value = max_value.max(value);
            }

            // Update value function
            values.insert(state.clone(), max_value);

            // Calculate delta for convergence check
            delta = delta.max((old_value - max_value).abs());
        }

        // Check for convergence
        if delta < theta {
            println!(
                "Value iteration converged after {} iterations",
                iteration + 1
            );
            break;
        }
    }

    values
}
