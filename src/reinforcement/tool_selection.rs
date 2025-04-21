// Adaptive tool selection system
// Uses reinforcement learning to intelligently select the optimal tool based on context and past performance

use crate::reinforcement::{
    action::{
        get_tool_details, map_action_to_tool, map_tool_result_to_action_result, AgentAction,
        ToolAction,
    },
    q_learning::QLearningSystem,
    reward::calculate_reward,
    state::{CodebaseState, StateTracker},
    Policy,
};
use crate::WinxResult;

/// System for adaptively selecting tools based on reinforcement learning
/// Uses Q-learning to improve tool selection based on previous outcomes
#[derive(Debug, Clone)]
pub struct AdaptiveToolSystem {
    /// Q-learning system for action selection
    q_learning: QLearningSystem,
    /// State tracker for maintaining state history
    state_tracker: StateTracker,
    /// History of actions and their results
    action_history: Vec<(CodebaseState, AgentAction, f64, CodebaseState)>,
    /// Whether RL is enabled
    rl_enabled: bool,
}

impl AdaptiveToolSystem {
    /// Creates a new adaptive tool system with the provided Q-learning system
    pub fn new(q_learning: QLearningSystem) -> Self {
        Self {
            q_learning,
            state_tracker: StateTracker::new(),
            action_history: Vec::new(),
            rl_enabled: true,
        }
    }

    /// Enables or disables the reinforcement learning functionality
    /// When disabled, the system falls back to the default tool selection strategy
    pub fn set_rl_enabled(&mut self, enabled: bool) {
        self.rl_enabled = enabled;
    }

    /// Selects the optimal tool for a given context based on reinforcement learning
    /// Uses the current state extracted from context to determine the best action
    pub fn select_tool(&mut self, context: &crate::tools::AgentContext) -> WinxResult<ToolAction> {
        // Extract the current state from the context
        let current_state = self.state_tracker.extract_state(context);

        if self.rl_enabled {
            // Use RL to select the best action
            let selected_action = self.q_learning.select_action(&current_state);

            // Convert the action to a tool
            let tool = map_action_to_tool(&selected_action);

            Ok(tool)
        } else {
            // Fall back to default tool selection
            self.default_tool_selection(context)
        }
    }

    /// Default tool selection strategy that serves as a fallback when reinforcement learning is disabled
    /// Uses a simple heuristic approach instead of learned behaviors
    fn default_tool_selection(
        &self,
        _context: &crate::tools::AgentContext,
    ) -> WinxResult<ToolAction> {
        // This is a simplified version - in a real implementation,
        // this would be based on heuristics or the existing selection logic

        // For demonstration, just return a directory listing command
        // This provides a safe, informative fallback action that helps understand
        // the current environment state
        Ok(ToolAction::BashCommand {
            action_json: String::from("{\"command\": \"ls -la\"}"),
            wait_for_seconds: None,
        })
    }

    /// Processes the result of a tool execution and updates the reinforcement learning model
    /// Updates Q-values based on the observed reward and stores transitions for experience replay
    pub fn process_result(
        &mut self,
        context: &crate::tools::AgentContext,
        tool: &ToolAction,
        result: &str,
    ) -> WinxResult<()> {
        if !self.rl_enabled {
            return Ok(());
        }

        // Get the previous and current states
        let previous_state = self.state_tracker.get_previous_state();
        let current_state = self.state_tracker.extract_state(context);

        // Convert the tool to an action
        let action = self.convert_tool_to_action(tool);

        // Convert the result to an action result
        let action_result = map_tool_result_to_action_result(&action, result);

        // Calculate the reward
        let reward = calculate_reward(&previous_state, &action, &current_state, &action_result);

        // Update the Q-learning model with the new state transition information
        self.q_learning
            .update_q_value(&previous_state, &action, reward, &current_state);

        // Store this transition for future experience replay training
        // This allows the agent to learn from past experiences multiple times
        self.action_history
            .push((previous_state, action, reward, current_state));

        // Keep history at a reasonable size to prevent memory issues
        // Using a sliding window of 1000 recent transitions
        if self.action_history.len() > 1000 {
            self.action_history.remove(0);
        }

        // Perform experience replay occasionally (every 100 actions)
        // This helps stabilize learning by revisiting past experiences
        if self.action_history.len() % 100 == 0 {
            self.q_learning.experience_replay(10);
        }

        Ok(())
    }

    /// Converts a ToolAction back to an AgentAction
    /// This is the inverse of map_action_to_tool and is needed for the learning process
    fn convert_tool_to_action(&self, tool: &ToolAction) -> AgentAction {
        match tool {
            ToolAction::ReadFiles { file_paths, .. } => {
                if let Some(path) = file_paths.first() {
                    AgentAction::ReadFile(std::path::PathBuf::from(path))
                } else {
                    AgentAction::NoOp
                }
            }

            ToolAction::WriteIfEmpty {
                file_path,
                file_content,
            } => AgentAction::WriteFile(std::path::PathBuf::from(file_path), file_content.clone()),

            ToolAction::FileEdit {
                file_path,
                file_edit_using_search_replace_blocks: _,
            } => {
                // This is a simplified conversion - in a real implementation,
                // we would parse the search/replace blocks to extract the actual patterns
                // Currently using placeholder values since we can't easily recover the original
                AgentAction::EditFile(
                    std::path::PathBuf::from(file_path),
                    String::from("search"),
                    String::from("replace"),
                )
            }

            ToolAction::BashCommand { action_json, .. } => {
                // Try to extract the command from the action_json
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(action_json) {
                    if let Some(cmd) = json.get("command").and_then(|c| c.as_str()) {
                        // Heuristic mapping based on command content
                        // Identifies special command types like test or build commands
                        if cmd.contains("test") {
                            AgentAction::RunTests
                        } else if cmd.contains("build") || cmd.contains("make") {
                            AgentAction::RunBuild
                        } else {
                            AgentAction::ExecuteCommand(cmd.to_string())
                        }
                    } else {
                        AgentAction::ExecuteCommand(String::from("unknown"))
                    }
                } else {
                    AgentAction::ExecuteCommand(String::from("unknown"))
                }
            }

            ToolAction::NoOp => AgentAction::NoOp,
        }
    }

    /// Gets the tool details (name and parameters) for a given action
    /// Used to interface with the actual tool execution system
    pub fn get_tool_for_action(&self, action: &AgentAction) -> Option<(String, serde_json::Value)> {
        let tool = map_action_to_tool(action);
        get_tool_details(&tool)
    }

    /// Resets the system state
    /// Clears history and resets the state tracker to initial conditions
    pub fn reset(&mut self) {
        self.state_tracker = StateTracker::new();
        self.action_history.clear();
    }
}

impl Default for AdaptiveToolSystem {
    fn default() -> Self {
        Self::new(QLearningSystem::default())
    }
}
