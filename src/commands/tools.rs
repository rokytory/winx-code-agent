use anyhow::Result;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{tool, Error as McpError, RoleServer, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json;
use tracing::info;

use crate::code;
use crate::commands::{bash, files};
use crate::core::{memory, state::SharedState};
use crate::sql;
use crate::thinking;

/// Tool implementations to be registered with MCP
#[derive(Clone)]
pub struct WinxTools {
    state: SharedState,
}

#[tool(tool_box)]
impl WinxTools {
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }

    #[tool(description = "Create a new task session that can be resumed later")]
    async fn create_task(
        &self,
        #[tool(param)] name: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        let task_id = name.unwrap_or_else(|| memory::create_task_id());

        // Save current state to task
        let result = {
            let state_guard = self.state.lock().unwrap();
            match state_guard.save_to_task(&task_id) {
                Ok(_) => format!("Task created with ID: {}", task_id),
                Err(e) => format!("Failed to create task: {}", e),
            }
        };

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List available tasks")]
    async fn list_tasks(&self) -> Result<CallToolResult, McpError> {
        let memory_dir = match memory::get_memory_dir() {
            Ok(dir) => dir,
            Err(e) => {
                return Err(McpError::internal_error(
                    "memory_dir_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        let store = match memory::create_shared_memory_store(memory_dir) {
            Ok(store) => store,
            Err(e) => {
                return Err(McpError::internal_error(
                    "memory_store_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        // Use a temp variable to avoid releasing the lock before calling list_tasks
        let tasks = {
            let mut store_guard = store.lock().unwrap();
            // Call the method directly on the MemoryStore struct
            let task_list = store_guard.list_tasks();
            // Clone task_list to a new variable so we can release the lock
            task_list
        };

        if tasks.is_empty() {
            Ok(CallToolResult::success(vec![Content::text("No tasks available.")]))
        } else {
            let task_list = tasks.iter()
                .map(|task| format!("- {}", task))
                .collect::<Vec<_>>()
                .join("\n");

            Ok(CallToolResult::success(vec![Content::text(format!("Available tasks:\n{}", task_list))]))
        }
    }

    #[tool(description = "Start or resume a background process")]
    async fn start_background_process(
        &self,
        #[tool(param)] command: String,
    ) -> Result<CallToolResult, McpError> {
        let result = bash::start_background_process(&self.state, &command)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "background_process_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Validate syntax of code")]
    async fn validate_syntax(
        &self,
        #[tool(param)] extension: String,
        #[tool(param)] content: String,
    ) -> Result<CallToolResult, McpError> {
        let result = code::validate_syntax(&extension, &content)
            .map_err(|e| {
                McpError::internal_error(
                    "syntax_validator_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        let is_valid = result.is_valid;
        let description = result.description;

        let response = if is_valid {
            format!("Syntax validation passed for .{} file.", extension)
        } else {
            format!("Syntax validation failed for .{} file:\n{}", extension, description)
        };

        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    #[tool(description = "Send text to a running interactive process")]
    async fn send_text_input(
        &self,
        #[tool(param)] text: String,
    ) -> Result<CallToolResult, McpError> {
        let result = bash::send_text_input(&self.state, &text)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "send_text_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Send special keys to a running interactive process")]
    async fn send_special_keys(
        &self,
        #[tool(param)] keys: Vec<String>,
    ) -> Result<CallToolResult, McpError> {
        // Convert string keys to Special enum
        let special_keys = keys.iter()
            .map(|k| k.parse::<crate::core::types::Special>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                McpError::invalid_params(
                    "invalid_special_key",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        let result = bash::send_special_keys(&self.state, &special_keys)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "send_keys_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Execute a bash command")]
    async fn bash_command(
        &self,
        #[tool(param)] action_json: serde_json::Value,
        #[tool(param)] wait_for_seconds: Option<f32>,
    ) -> Result<CallToolResult, McpError> {
        // Enhanced version to handle nested action_json format from different clients
        // 1. JSON Object: {"command": "ls -la"}
        // 2. Simple String: "ls -la"
        // 3. Nested Object: {"action_json": {"command": "ls -la"}}

        info!("Bash command received: {:?}", action_json);

        let command = if action_json.is_string() {
            // If it's a simple string, use it as a command
            action_json.as_str().unwrap_or("").to_string()
        } else if let Some(obj) = action_json.as_object() {
            // If it's a JSON object, check for different formats
            if let Some(cmd) = obj.get("command") {
                // Direct command property
                if cmd.is_string() {
                    cmd.as_str().unwrap_or("").to_string()
                } else {
                    return Err(McpError::invalid_params(
                        "command_format_error",
                        Some(serde_json::json!({
                            "error": "command property must be a string"
                        })),
                    ));
                }
            } else if let Some(nested_action) = obj.get("action_json") {
                // Handle nested action_json format
                if nested_action.is_string() {
                    // If action_json is a string command
                    nested_action.as_str().unwrap_or("").to_string()
                } else if let Some(nested_obj) = nested_action.as_object() {
                    // If action_json is an object with command
                    if let Some(nested_cmd) = nested_obj.get("command") {
                        if nested_cmd.is_string() {
                            nested_cmd.as_str().unwrap_or("").to_string()
                        } else {
                            return Err(McpError::invalid_params(
                                "command_format_error",
                                Some(serde_json::json!({
                                    "error": "command property in action_json must be a string"
                                })),
                            ));
                        }
                    } else {
                        return Err(McpError::invalid_params(
                            "command_format_error",
                            Some(serde_json::json!({
                                "error": "command property is required in nested action_json object"
                            })),
                        ));
                    }
                } else {
                    return Err(McpError::invalid_params(
                        "command_format_error",
                        Some(serde_json::json!({
                            "error": "nested action_json must be a string or an object with a command property"
                        })),
                    ));
                }
            } else {
                return Err(McpError::invalid_params(
                    "command_format_error",
                    Some(serde_json::json!({
                        "error": "command or action_json property is required"
                    })),
                ));
            }
        } else {
            return Err(McpError::invalid_params(
                "command_format_error",
                Some(serde_json::json!({
                    "error": "action_json must be a string, an object with a command property, or an object with a nested action_json"
                })),
            ));
        };

        // Execute the command directly
        info!("Executing bash command: {}", command);

        // Execute the command
        let result = bash::execute_command(&self.state, &command)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "bash_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Read files from the filesystem")]
    async fn read_files(
        &self,
        #[tool(param)] file_paths: Vec<String>,
        #[tool(param)] show_line_numbers_reason: Option<String>,
        #[tool(param)] line_ranges: Option<Vec<Option<(usize, usize)>>>,
    ) -> Result<CallToolResult, McpError> {
        // Here we also ensure compatibility with different formats
        info!("Reading files: {:?}", file_paths);

        let request = ReadFiles {
            file_paths,
            show_line_numbers_reason,
            line_ranges,
        };

        let json = match serde_json::to_string(&request) {
            Ok(j) => j,
            Err(e) => {
                return Err(McpError::internal_error(
                    "serialize_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        let result = files::read_files(&self.state, &json).await.map_err(|e| {
            McpError::internal_error("file_error", Some(serde_json::Value::String(e.to_string())))
        })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Write or edit a file")]
    async fn file_write_or_edit(
        &self,
        #[tool(param)] file_path: String,
        #[tool(param)] percentage_to_change: u8,
        #[tool(param)] file_content_or_search_replace_blocks: String,
    ) -> Result<CallToolResult, McpError> {
        let request = FileWriteOrEdit {
            file_path,
            percentage_to_change,
            file_content_or_search_replace_blocks,
        };

        let json = match serde_json::to_string(&request) {
            Ok(j) => j,
            Err(e) => {
                return Err(McpError::internal_error(
                    "serialize_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        let result = files::write_or_edit_file(&self.state, &json)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "file_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Execute an SQL query")]
    async fn sql_query(&self, #[tool(param)] query: String) -> Result<CallToolResult, McpError> {
        let request = SqlQuery { query };
        let json = match serde_json::to_string(&request) {
            Ok(j) => j,
            Err(e) => {
                return Err(McpError::internal_error(
                    "serialize_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        let result = sql::execute_sql_query(&self.state, &json)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "sql_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Process sequential thinking for problem solving")]
    async fn sequential_thinking(
        &self,
        #[tool(param)] thought: String,
        #[tool(param)] next_thought_needed: bool,
        #[tool(param)] thought_number: usize,
        #[tool(param)] total_thoughts: usize,
        #[tool(param)] is_revision: Option<bool>,
        #[tool(param)] revises_thought: Option<usize>,
        #[tool(param)] branch_from_thought: Option<usize>,
        #[tool(param)] branch_id: Option<String>,
        #[tool(param)] needs_more_thoughts: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        let request = SequentialThinking {
            thought,
            next_thought_needed,
            thought_number,
            total_thoughts,
            is_revision,
            revises_thought,
            branch_from_thought,
            branch_id,
            needs_more_thoughts,
        };

        let json = match serde_json::to_string(&request) {
            Ok(j) => j,
            Err(e) => {
                return Err(McpError::internal_error(
                    "serialize_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        let result = thinking::process_sequential_thinking(&self.state, &json)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "thinking_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[tool(tool_box)]
impl ServerHandler for WinxTools {
    fn get_info(&self) -> ServerInfo {
        // Ensure we use the correct protocol version
        // and that the tools configuration is enabled
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Winx is a Rust code agent that allows executing bash commands, manipulating files, and executing SQL queries.".to_string()),
        }
    }

    // Implementation of additional methods that the Counter example includes

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        // Returns an empty list of resources
        Ok(ListResourcesResult {
            resources: Vec::new(),
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        // No resources to read
        Err(McpError::resource_not_found(
            "resource_not_found",
            Some(serde_json::json!({
                "uri": uri
            })),
        ))
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        // Returns an empty list of prompts
        Ok(ListPromptsResult {
            next_cursor: None,
            prompts: Vec::new(),
        })
    }

    async fn get_prompt(
        &self,
        GetPromptRequestParam { name, .. }: GetPromptRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        // No prompts to get
        Err(McpError::invalid_params(
            "prompt not found",
            Some(serde_json::json!({"name": name})),
        ))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        // Returns an empty list of resource templates
        Ok(ListResourceTemplatesResult {
            next_cursor: None,
            resource_templates: Vec::new(),
        })
    }
}

/// Initialize tool registration
pub fn register_tools(state: SharedState) -> Result<()> {
    info!("Registering Winx tools");

    // Create a new WinxTools instance with the shared state
    let _tools = WinxTools::new(state);

    // In a full implementation, we would register the tools with the RMCP server
    // But this is now handled by the tool macros

    info!("All Winx tools registered successfully");
    Ok(())
}

/// Basic bash command tool definition
#[derive(Debug, Serialize, Deserialize)]
pub struct BashCommand {
    pub command: String,
}

/// Basic file read tool definition
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadFiles {
    pub file_paths: Vec<String>,
    pub show_line_numbers_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_ranges: Option<Vec<Option<(usize, usize)>>>,
}

/// Basic file write/edit tool definition
#[derive(Debug, Serialize, Deserialize)]
pub struct FileWriteOrEdit {
    pub file_path: String,
    pub percentage_to_change: u8,
    pub file_content_or_search_replace_blocks: String,
}

/// SQL query tool definition
#[derive(Debug, Serialize, Deserialize)]
pub struct SqlQuery {
    pub query: String,
}

/// Sequential thinking tool definition
#[derive(Debug, Serialize, Deserialize)]
pub struct SequentialThinking {
    pub thought: String,
    pub next_thought_needed: bool,
    pub thought_number: usize,
    pub total_thoughts: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_revision: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revises_thought: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_from_thought: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs_more_thoughts: Option<bool>,
}
