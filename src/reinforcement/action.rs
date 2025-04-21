// Actions available to the reinforcement learning agent
// Each action maps directly to a specific tool in the winx-code-agent codebase

use std::path::PathBuf;

/// Defines all possible actions that the agent can take within the codebase
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentAction {
    /// Read a file
    ReadFile(PathBuf),

    /// Write to a new/empty file
    WriteFile(PathBuf, String),

    /// Edit a file using search/replace patterns
    EditFile(PathBuf, String, String), // (file path, search pattern, replacement text)

    /// Execute a bash command
    ExecuteCommand(String),

    /// Analyze code for syntax errors
    AnalyzeCode(PathBuf),

    /// Search for a symbol in the codebase
    SearchForSymbol(String),

    /// Run tests
    RunTests,

    /// Run build
    RunBuild,

    /// Suggest a fix for a syntax error
    SuggestFix(PathBuf, usize, usize), // file, line, column

    /// No-op action
    NoOp,
}

/// Maps an AgentAction to the corresponding executable ToolAction
/// This converts abstract agent actions into concrete tool implementations
pub fn map_action_to_tool(action: &AgentAction) -> ToolAction {
    match action {
        AgentAction::ReadFile(path) => ToolAction::ReadFiles {
            file_paths: vec![path.to_string_lossy().to_string()],
            show_line_numbers_reason: None,
        },
        AgentAction::WriteFile(path, content) => ToolAction::WriteIfEmpty {
            file_path: path.to_string_lossy().to_string(),
            file_content: content.clone(),
        },
        AgentAction::EditFile(path, search, replace) => ToolAction::FileEdit {
            file_path: path.to_string_lossy().to_string(),
            file_edit_using_search_replace_blocks: format!(
                "<<<<<<< SEARCH\n{}\n=======\n{}\n>>>>>>> REPLACE",
                search, replace
            ),
        },
        AgentAction::ExecuteCommand(cmd) => ToolAction::BashCommand {
            action_json: format!("{{\"command\": \"{}\"}}", cmd.replace("\"", "\\\"")),
            wait_for_seconds: None,
        },
        AgentAction::RunTests => ToolAction::BashCommand {
            action_json: String::from("{\"command\": \"if [ -f Cargo.toml ]; then cargo test; elif [ -f package.json ]; then npm test; elif [ -f requirements.txt ]; then python -m pytest; else echo \\\"No test command found\\\"; fi\"}"),
            wait_for_seconds: None,
        },
        AgentAction::RunBuild => ToolAction::BashCommand {
            action_json: String::from("{\"command\": \"if [ -f Cargo.toml ]; then cargo build; elif [ -f package.json ]; then npm run build; elif [ -f Makefile ]; then make; else echo \\\"No build command found\\\"; fi\"}"),
            wait_for_seconds: None,
        },
        AgentAction::AnalyzeCode(path) => ToolAction::BashCommand {
            action_json: format!(
                "{{\"command\": \"if [ -f Cargo.toml ]; then cargo check --message-format=json | grep \\\"{}\\\" -A 10; elif [ -f package.json ]; then npx eslint {} --format=json; else echo \\\"No analysis command found\\\"; fi\"}}",
                path.to_string_lossy(), path.to_string_lossy()
            ),
            wait_for_seconds: None,
        },
        AgentAction::SearchForSymbol(symbol) => ToolAction::BashCommand {
            action_json: format!("{{\"command\": \"grep -r \\\"{}\\\" --include=\\\"*.rs\\\" --include=\\\"*.js\\\" --include=\\\"*.py\\\" --include=\\\"*.java\\\" --include=\\\"*.cpp\\\" --include=\\\"*.h\\\" .\"}}", symbol),
            wait_for_seconds: None,
        },
        AgentAction::SuggestFix(_, _, _) => ToolAction::NoOp,
        AgentAction::NoOp => ToolAction::NoOp,
    }
}

/// Processes a tool's execution result and converts it back to an appropriate ActionResult
/// This interprets the string output from tools and classifies the outcome as success, failure, etc.
pub fn map_tool_result_to_action_result(action: &AgentAction, result: &str) -> ActionResult {
    match action {
        AgentAction::ReadFile(_) => ActionResult::FileContent(result.to_string()),

        AgentAction::WriteFile(_, _) => {
            if result.contains("Success") {
                ActionResult::Success(result.to_string())
            } else {
                ActionResult::Failure(result.to_string())
            }
        }

        AgentAction::EditFile(_, _, _) => {
            if result.contains("Success") {
                ActionResult::Success(result.to_string())
            } else {
                ActionResult::Failure(result.to_string())
            }
        }

        AgentAction::ExecuteCommand(_) => {
            if result.contains("process exited with code 0") {
                ActionResult::Success(result.to_string())
            } else {
                ActionResult::Failure(result.to_string())
            }
        }

        AgentAction::RunTests => {
            // Check for standard success patterns in test output across different frameworks
            if result.contains("test result: ok") || result.contains("passing") {
                ActionResult::Success(result.to_string())
            } else {
                ActionResult::Failure(result.to_string())
            }
        }

        AgentAction::RunBuild => {
            // For build commands, "Finished" without "error" typically indicates success
            // This is a heuristic that works for Cargo and similar build systems
            if result.contains("Finished") && !result.contains("error") {
                ActionResult::Success(result.to_string())
            } else {
                ActionResult::Failure(result.to_string())
            }
        }

        AgentAction::AnalyzeCode(_) => {
            // Handle code analysis results:
            // - No command available or empty result → Neutral (no effect)
            // - Contains "error" → Failure
            // - Otherwise → Success
            if result.contains("No analysis command found") || result.is_empty() {
                ActionResult::Neutral
            } else if result.contains("error") {
                ActionResult::Failure(result.to_string())
            } else {
                ActionResult::Success(result.to_string())
            }
        }

        AgentAction::SearchForSymbol(_) => {
            if result.is_empty() {
                ActionResult::Neutral
            } else {
                ActionResult::Success(result.to_string())
            }
        }

        AgentAction::SuggestFix(_, _, _) => ActionResult::Neutral,

        AgentAction::NoOp => ActionResult::Neutral,
    }
}

/// Available concrete tool implementations that can be executed
/// These represent the actual implementation mechanisms for agent actions
#[derive(Debug, Clone)]
pub enum ToolAction {
    /// Read one or more files
    ReadFiles {
        file_paths: Vec<String>,
        show_line_numbers_reason: Option<String>,
    },

    /// Write to a new/empty file
    WriteIfEmpty {
        file_path: String,
        file_content: String,
    },

    /// Edit a file using search/replace
    FileEdit {
        file_path: String,
        file_edit_using_search_replace_blocks: String,
    },

    /// Execute a bash command
    BashCommand {
        action_json: String,
        wait_for_seconds: Option<f64>,
    },

    /// No-op action
    NoOp,
}

/// Represents the outcome of an action after execution
/// Provides classification and details about how an action completed
#[derive(Debug, Clone)]
pub enum ActionResult {
    /// Action succeeded
    Success(String),

    /// Action failed
    Failure(String),

    /// Action had no effect (neutral)
    Neutral,

    /// File content
    FileContent(String),
}

/// Converts a ToolAction into the specific tool name and parameter structure
/// required by the execution system
pub fn get_tool_details(action: &ToolAction) -> Option<(String, serde_json::Value)> {
    match action {
        ToolAction::ReadFiles {
            file_paths,
            show_line_numbers_reason,
        } => {
            let params = serde_json::json!({
                "file_paths": file_paths,
                "show_line_numbers_reason": show_line_numbers_reason
            });
            Some(("read_files".to_string(), params))
        }

        ToolAction::WriteIfEmpty {
            file_path,
            file_content,
        } => {
            let params = serde_json::json!({
                "file_path": file_path,
                "file_content": file_content
            });
            Some(("write_if_empty".to_string(), params))
        }

        ToolAction::FileEdit {
            file_path,
            file_edit_using_search_replace_blocks,
        } => {
            let params = serde_json::json!({
                "file_path": file_path,
                "file_edit_using_search_replace_blocks": file_edit_using_search_replace_blocks
            });
            Some(("file_edit".to_string(), params))
        }

        ToolAction::BashCommand {
            action_json,
            wait_for_seconds,
        } => {
            let params = serde_json::json!({
                "action_json": action_json,
                "wait_for_seconds": wait_for_seconds
            });
            Some(("bash_command".to_string(), params))
        }

        ToolAction::NoOp => None,
    }
}
