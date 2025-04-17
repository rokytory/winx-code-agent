use anyhow::Result;
use rmcp::model::*;
use rmcp::{tool, Error as McpError, ServerHandler, RoleServer};
use rmcp::service::RequestContext;
use serde::{Deserialize, Serialize};
use serde_json;
use tracing::info;

use crate::commands::{bash, files};
use crate::core::state::SharedState;
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

    #[tool(description = "Execute a bash command")]
    async fn bash_command(
        &self,
        #[tool(param)] action_json: serde_json::Value,
        #[tool(param)] wait_for_seconds: Option<f32>,
    ) -> Result<CallToolResult, McpError> {
        // Claude can send action_json in two ways:
        // 1. As a simple string: "ls -la"
        // 2. As a JSON object: {"command": "ls -la"}
        
        // We check the received format and adapt accordingly
        let command_json = if action_json.is_string() {
            // If it's a simple string, we convert it to the expected format
            let cmd_string = action_json.as_str().unwrap_or("");
            let bash_cmd = crate::core::types::BashCommand {
                action_json: crate::core::types::BashAction::Command(
                    crate::core::types::Command {
                        command: cmd_string.to_string(),
                    }
                ),
                wait_for_seconds,
            };
            serde_json::to_string(&bash_cmd).unwrap_or_default()
        } else {
            // If it's already a JSON object, we use it as is
            serde_json::to_string(&action_json).unwrap_or_default()
        };
        
        info!("Executing bash command: {}", command_json);
        
        // Execute the command using the correct format
        let result = bash::execute_bash_command(&self.state, &command_json)
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
    ) -> Result<CallToolResult, McpError> {
        // Here we also ensure compatibility with different formats
        info!("Reading files: {:?}", file_paths);
        
        let request = ReadFiles {
            file_paths,
            show_line_numbers_reason,
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
            Some(serde_json::json!({"name": name}))
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
