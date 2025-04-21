pub mod bash_command;
pub mod context_save;
pub mod file_operations;
pub mod initialize;
// Temporarily commenting the semantic_code module
// pub mod semantic_code;

// Context for the agent
pub struct AgentContext {
    // Current working directory
    pub cwd: String,
    // Description of the current task
    pub task_description: String,
}
