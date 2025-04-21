use crate::reinforcement::{initialize_rl_system, AdaptiveToolSystem};
use crate::tools::{
    bash_command::BashCommand,
    context_save::ContextSave,
    file_operations::{FileEdit, FileOperations, WriteIfEmpty},
    initialize::Initialize,
    // Comentando temporariamente os módulos LSP que estão causando erros
    // semantic_code::{AddSymbolTool, EditSymbolTool, FindReferencesTool, FindSymbolTool},
};
use rmcp::{
    model::{CallToolResult, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, Error as McpError, RoleServer, ServerHandler,
};

#[derive(Debug, Clone)]
pub struct CodeAgent {
    initialize: Initialize,
    bash_command: BashCommand,
    file_ops: FileOperations,
    write_if_empty: WriteIfEmpty,
    file_edit: FileEdit,
    context_save: ContextSave,
    // Comentando temporariamente os campos relacionados ao LSP
    // find_symbol: FindSymbolTool,
    // find_references: FindReferencesTool,
    // edit_symbol: EditSymbolTool,
    // add_symbol: AddSymbolTool,
    adaptive_tool_system: Option<AdaptiveToolSystem>,
    rl_enabled: bool,
}

impl CodeAgent {
    pub fn new() -> Self {
        // Try to initialize the RL system
        let adaptive_tool_system = match initialize_rl_system() {
            Ok(system) => Some(system),
            Err(err) => {
                log::error!("Failed to initialize RL system: {}", err);
                None
            }
        };

        Self {
            initialize: Initialize::new(),
            bash_command: BashCommand::new(),
            file_ops: FileOperations::new(),
            write_if_empty: WriteIfEmpty::new(),
            file_edit: FileEdit::new(),
            context_save: ContextSave::new(),
            // Comentando temporariamente as inicializações relacionadas ao LSP
            // find_symbol: FindSymbolTool::new(),
            // find_references: FindReferencesTool::new(),
            // edit_symbol: EditSymbolTool::new(),
            // add_symbol: AddSymbolTool::new(),
            adaptive_tool_system,
            rl_enabled: false, // Disabled by default until fully tested
        }
    }

    /// Enable or disable reinforcement learning
    pub fn set_rl_enabled(&mut self, enabled: bool) {
        self.rl_enabled = enabled;

        if let Some(tool_system) = &mut self.adaptive_tool_system {
            tool_system.set_rl_enabled(enabled);
        }
    }

    /// Select the optimal tool for a given context
    pub async fn select_optimal_tool(
        &self,
        context: &RequestContext<RoleServer>,
    ) -> Option<(String, serde_json::Value)> {
        if self.rl_enabled && self.adaptive_tool_system.is_some() {
            let agent_context = self.create_agent_context(context);

            // Use RL to select the best tool
            if let Some(tool_system) = &self.adaptive_tool_system {
                match tool_system.clone().select_tool(&agent_context) {
                    Ok(tool_action) => {
                        return crate::reinforcement::action::get_tool_details(&tool_action);
                    }
                    Err(err) => {
                        log::error!("Failed to select tool with RL: {}", err);
                    }
                }
            }
        }

        // Fall back to default behavior
        None
    }

    /// Process the result of a tool execution and update the RL model
    pub fn process_tool_result(
        &mut self,
        context: &RequestContext<RoleServer>,
        tool_name: &str,
        params: &serde_json::Value,
        result: &str,
    ) {
        if !self.rl_enabled || self.adaptive_tool_system.is_none() {
            return;
        }

        let agent_context = self.create_agent_context(context);

        // Convert the tool name and params to a ToolAction
        let tool_action = self.create_tool_action(tool_name, params);

        // Process the result
        if let Some(tool_system) = &mut self.adaptive_tool_system {
            if let Err(err) = tool_system.process_result(&agent_context, &tool_action, result) {
                log::error!("Failed to process tool result with RL: {}", err);
            }
        }
    }

    /// Create an agent context from a request context
    fn create_agent_context(
        &self,
        context: &RequestContext<RoleServer>,
    ) -> crate::tools::AgentContext {
        // This would be implemented to extract the relevant information from the request context
        crate::tools::AgentContext {
            cwd: context.id.to_string(),     // This is just a placeholder
            task_description: String::new(), // This would be extracted from the request
        }
    }

    /// Create a tool action from a tool name and parameters
    fn create_tool_action(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> crate::reinforcement::action::ToolAction {
        match tool_name {
            "read_files" => {
                let file_paths = params
                    .get("file_paths")
                    .and_then(|p| p.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let show_line_numbers_reason = params
                    .get("show_line_numbers_reason")
                    .and_then(|r| r.as_str())
                    .map(String::from);

                crate::reinforcement::action::ToolAction::ReadFiles {
                    file_paths,
                    show_line_numbers_reason,
                }
            }

            "write_if_empty" => {
                let file_path = params
                    .get("file_path")
                    .and_then(|p| p.as_str())
                    .map(String::from)
                    .unwrap_or_default();

                let file_content = params
                    .get("file_content")
                    .and_then(|c| c.as_str())
                    .map(String::from)
                    .unwrap_or_default();

                crate::reinforcement::action::ToolAction::WriteIfEmpty {
                    file_path,
                    file_content,
                }
            }

            "file_edit" => {
                let file_path = params
                    .get("file_path")
                    .and_then(|p| p.as_str())
                    .map(String::from)
                    .unwrap_or_default();

                let file_edit_using_search_replace_blocks = params
                    .get("file_edit_using_search_replace_blocks")
                    .and_then(|b| b.as_str())
                    .map(String::from)
                    .unwrap_or_default();

                crate::reinforcement::action::ToolAction::FileEdit {
                    file_path,
                    file_edit_using_search_replace_blocks,
                }
            }

            "bash_command" => {
                let action_json = params
                    .get("action_json")
                    .and_then(|a| a.as_str())
                    .map(String::from)
                    .unwrap_or_default();

                let wait_for_seconds = params.get("wait_for_seconds").and_then(|w| w.as_f64());

                crate::reinforcement::action::ToolAction::BashCommand {
                    action_json,
                    wait_for_seconds,
                }
            }

            _ => crate::reinforcement::action::ToolAction::NoOp,
        }
    }
}

impl Default for CodeAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[tool(tool_box)]
impl CodeAgent {
    #[tool(
        description = "\n- Always call this at the start of the conversation before using any of the shell tools from winx.\n- Use `any_workspace_path` to initialize the shell in the appropriate project directory.\n- If the user has mentioned a workspace or project root or any other file or folder use it to set `any_workspace_path`.\n- If user has mentioned any files use `initial_files_to_read` to read, use absolute paths only (~ allowed)\n- By default use mode \"wcgw\"\n- In \"code-writer\" mode, set the commands and globs which user asked to set, otherwise use 'all'.\n- Use type=\"first_call\" if it's the first call to this tool.\n- Use type=\"user_asked_mode_change\" if in a conversation user has asked to change mode.\n- Use type=\"reset_shell\" if in a conversation shell is not working after multiple tries.\n- Use type=\"user_asked_change_workspace\" if in a conversation user asked to change workspace\n"
    )]
    async fn initialize(
        &self,
        #[tool(aggr)] params: crate::tools::initialize::InitializeParams,
    ) -> Result<CallToolResult, McpError> {
        self.initialize.initialize(params).await
    }

    #[tool(
        description = "\n- Execute a bash command. This is stateful (beware with subsequent calls).\n- Status of the command and the current working directory will always be returned at the end.\n- The first or the last line might be `(...truncated)` if the output is too long.\n- Always run `pwd` if you get any file or directory not found error to make sure you're not lost.\n- Run long running commands in background using screen instead of \"&\".\n- Do not use 'cat' to read files, use ReadFiles tool instead\n- In order to check status of previous command, use `status_check` with empty command argument.\n- Only command is allowed to run at a time. You need to wait for any previous command to finish before running a new one.\n- Programs don't hang easily, so most likely explanation for no output is usually that the program is still running, and you need to check status again.\n- Do not send Ctrl-c before checking for status till 10 minutes or whatever is appropriate for the program to finish.\n"
    )]
    async fn bash_command(
        &self,
        #[tool(aggr)] params: crate::tools::bash_command::BashCommandParams,
    ) -> Result<CallToolResult, McpError> {
        self.bash_command.bash_command(params).await
    }

    #[tool(
        description = "\n- Read full file content of one or more files.\n- Provide absolute paths only (~ allowed)\n- Only if the task requires line numbers understanding:\n    - You may populate \"show_line_numbers_reason\" with your reason, by default null/empty means no line numbers are shown.\n    - You may extract a range of lines. E.g., `/path/to/file:1-10` for lines 1-10. You can drop start or end like `/path/to/file:1-` or `/path/to/file:-10` \n"
    )]
    async fn read_files(
        &self,
        #[tool(aggr)] params: crate::tools::file_operations::ReadFilesParams,
    ) -> Result<CallToolResult, McpError> {
        self.file_ops.read_files(params).await
    }

    #[tool(description = "Create new files or write to empty files only")]
    async fn write_if_empty(
        &self,
        #[tool(aggr)] params: crate::tools::file_operations::WriteIfEmptyParams,
    ) -> Result<CallToolResult, McpError> {
        self.write_if_empty.write_if_empty(params).await
    }

    #[tool(
        description = "\n- Edits existing files using search/replace blocks.\n- Uses Aider-like search and replace syntax.\n- File edit has spacing tolerant matching, with warning on issues like indentation mismatch.\n- If there's no match, the closest match is returned to help fix mistakes.\n"
    )]
    async fn file_edit(
        &self,
        #[tool(aggr)] params: crate::tools::file_operations::FileEditParams,
    ) -> Result<CallToolResult, McpError> {
        self.file_edit.file_edit(params).await
    }

    #[tool(description = "Read an image file and return its base64-encoded content")]
    async fn read_image(
        &self,
        #[tool(aggr)] params: crate::tools::file_operations::ReadImageParams,
    ) -> Result<CallToolResult, McpError> {
        self.file_ops.read_image(params).await
    }

    #[tool(
        description = "\nSaves provided description and file contents of all the relevant file paths or globs in a single text file.\n- Provide random unqiue id or whatever user provided.\n- Leave project path as empty string if no project path"
    )]
    async fn context_save(
        &self,
        #[tool(aggr)] params: crate::tools::context_save::ContextSaveParams,
    ) -> Result<CallToolResult, McpError> {
        self.context_save.context_save(params).await
    }

    // Comentando temporariamente os métodos relacionados ao LSP
    // #[tool(
    //     description = "Find symbols by name in the codebase with semantic understanding."
    // )]
    // async fn find_symbol(
    //     &self,
    //     #[tool(aggr)] params: crate::tools::semantic_code::FindSymbolParams,
    // ) -> Result<CallToolResult, McpError> {
    //     self.find_symbol.find_symbol(params).await
    // }
    //
    // #[tool(
    //     description = "Find references to a symbol in the codebase."
    // )]
    // async fn find_references(
    //     &self,
    //     #[tool(aggr)] params: crate::tools::semantic_code::FindReferencesParams,
    // ) -> Result<CallToolResult, McpError> {
    //     self.find_references.find_references(params).await
    // }
    //
    // #[tool(
    //     description = "Edit a symbol in the codebase with semantic understanding."
    // )]
    // async fn edit_symbol(
    //     &self,
    //     #[tool(aggr)] params: crate::tools::semantic_code::EditSymbolParams,
    // ) -> Result<CallToolResult, McpError> {
    //     self.edit_symbol.edit_symbol(params).await
    // }
    //
    // #[tool(
    //     description = "Add a new symbol to the codebase with semantic understanding."
    // )]
    // async fn add_symbol(
    //     &self,
    //     #[tool(aggr)] params: crate::tools::semantic_code::AddSymbolParams,
    // ) -> Result<CallToolResult, McpError> {
    //     self.add_symbol.add_symbol(params).await
    // }
}

#[tool(tool_box)]
impl ServerHandler for CodeAgent {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "A code agent that provides shell and coding tools for AI assistants, enabling safe execution of commands and file operations".to_string(),
            ),
        }
    }
}
