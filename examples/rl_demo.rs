// Example demonstrating the use of Reinforcement Learning in winx-code-agent
// This is a simplified example that shows how the RL system can be used

use rmcp::model::RequestContext;
use std::path::PathBuf;
use winx_code_agent::{
    reinforcement::{
        action::AgentAction, q_learning::QLearningSystem, reward::calculate_reward,
        state::CodebaseState,
    },
    tools::AgentContext,
    CodeAgent,
};

fn main() {
    // Initialize the code agent
    let mut agent = CodeAgent::new();

    // Enable reinforcement learning
    agent.set_rl_enabled(true);

    println!("Reinforcement Learning Demo for winx-code-agent");
    println!("-----------------------------------------------");

    // Create a request context
    let context = create_dummy_request_context();

    // Example: Select an optimal tool based on RL
    println!("\nSelecting optimal tool based on RL...");
    let optimal_tool = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(agent.select_optimal_tool(&context));

    if let Some((tool_name, params)) = optimal_tool {
        println!("Selected tool: {}", tool_name);
        println!(
            "Parameters: {}",
            serde_json::to_string_pretty(&params).unwrap()
        );

        // Process the result (in a real-world scenario, this would come from executing the tool)
        let result = "Command executed successfully. File 'example.txt' created.";
        agent.process_tool_result(&context, &tool_name, &params, result);
        println!("Result processed through RL system for learning.");
    } else {
        println!("No tool selected by RL system.");
    }

    // Example: Manual RL demonstration with a simplified state and action
    println!("\nDemonstrating RL with simplified state and action...");
    rl_manual_demonstration();
}

// Create a dummy request context for demonstration purposes
fn create_dummy_request_context() -> RequestContext {
    RequestContext {
        request_id: "demo-request-123".to_string(),
        tool_id: None,
        param_values: Default::default(),
        param_types: Default::default(),
        param_defaults: Default::default(),
    }
}

// Demonstrate RL manually with a simplified state and action
fn rl_manual_demonstration() {
    // Create a Q-Learning system
    let mut q_learning = QLearningSystem::new(0.1, 0.9, 0.2);

    // Create some states
    let mut state1 = CodebaseState::new(
        PathBuf::from("/tmp/project"),
        "Fix syntax errors".to_string(),
    );

    let mut state2 = CodebaseState::new(
        PathBuf::from("/tmp/project"),
        "Fix syntax errors".to_string(),
    );

    // Add a syntax error to state1
    state1.add_syntax_error(crate::reinforcement::state::SyntaxError {
        file_path: PathBuf::from("/tmp/project/src/main.rs"),
        line: 10,
        column: 5,
        message: "expected ';', found '}'".to_string(),
        severity: crate::reinforcement::state::ErrorSeverity::Error,
    });

    // Define actions
    let actions = [
        AgentAction::RunTests,
        AgentAction::RunBuild,
        AgentAction::EditFile(
            PathBuf::from("/tmp/project/src/main.rs"),
            "expected ';', found '}'".to_string(),
            "fixed syntax".to_string(),
        ),
    ];

    // Learn from some experiences
    for i in 0..10 {
        println!("\nIteration {}:", i + 1);

        // Select an action using the current policy
        let action = q_learning.select_action(&state1);
        println!("Selected action: {:?}", action);

        // Calculate reward (in a real scenario, this would come from the environment)
        let reward = if action
            == AgentAction::EditFile(
                PathBuf::from("/tmp/project/src/main.rs"),
                "expected ';', found '}'".to_string(),
                "fixed syntax".to_string(),
            ) {
            5.0 // High reward for fixing the error
        } else {
            0.5 // Small reward for exploring
        };

        println!("Reward: {}", reward);

        // Update the Q-value
        q_learning.update_q_value(&state1, &action, reward, &state2);

        // After a few iterations, the Q-learning should converge to prefer the EditFile action
        println!("Q-values:");
        for a in &actions {
            println!("  {:?}: {}", a, q_learning.get_q_value(&state1, a));
        }
    }

    println!("\nFinal action selection:");
    let final_action = q_learning.select_action(&state1);
    println!("The RL system now prefers: {:?}", final_action);
}
