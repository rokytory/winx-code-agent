use anyhow::Result;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{tool, Error as McpError, RoleServer, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn};

use crate::code;
use crate::commands::{bash, files, vibe_code};
use crate::core::{memory, state::SharedState};
use crate::sql;
use crate::thinking;

// Flag global para controlar se a inicialização foi realizada
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Tool implementations to be registered with MCP
#[derive(Clone)]
pub struct WinxTools {
    state: SharedState,
}

/// Verifica se a inicialização foi realizada e lança erro se não
fn check_initialized() -> Result<(), McpError> {
    if !INITIALIZED.load(Ordering::SeqCst) {
        warn!("Attempt to use tool without prior initialization");
        return Err(McpError::internal_error(
            "initialize_error",
            Some(serde_json::json!({
                "error": crate::t!(
                    "Tool initialization required. Please call init_vibe_code first.",
                    "Inicialização da ferramenta necessária. Por favor, chame init_vibe_code primeiro.",
                    "Se requiere inicialización de herramienta. Por favor, llame a init_vibe_code primero."
                )
            })),
        ));
    }
    Ok(())
}

/// Reseta o estado de inicialização
pub fn reset_initialization() {
    INITIALIZED.store(false, Ordering::SeqCst);
    info!("Initialization state reset for tools");
}

/// Registra as ferramentas e marca como inicializado
pub fn register_tools(_state: SharedState) -> Result<()> {
    // Marca como inicializado
    INITIALIZED.store(true, Ordering::SeqCst);

    // Inicializa o suporte a idiomas
    crate::core::i18n::init_language_support();

    info!("Tools registered and initialized successfully");
    Ok(())
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
        check_initialized()?;
        let task_id = name.unwrap_or_else(memory::create_task_id);

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
        check_initialized()?;
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
            let store_guard = store.lock().unwrap();
            // Call the method directly on the MemoryStore struct
            // Clone the task list so we can release the lock
            store_guard.list_tasks()
        };

        if tasks.is_empty() {
            Ok(CallToolResult::success(vec![Content::text(
                "No tasks available.",
            )]))
        } else {
            let task_list = tasks
                .iter()
                .map(|task| format!("- {}", task))
                .collect::<Vec<_>>()
                .join("\n");

            Ok(CallToolResult::success(vec![Content::text(format!(
                "Available tasks:\n{}",
                task_list
            ))]))
        }
    }

    #[tool(description = "Start or resume a background process")]
    async fn start_background_process(
        &self,
        #[tool(param)] command: String,
    ) -> Result<CallToolResult, McpError> {
        check_initialized()?;
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
        check_initialized()?;
        let result = code::validate_syntax(&extension, &content).map_err(|e| {
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
            format!(
                "Syntax validation failed for .{} file:\n{}",
                extension, description
            )
        };

        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    #[tool(description = "Send text to a running interactive process")]
    async fn send_text_input(
        &self,
        #[tool(param)] text: String,
    ) -> Result<CallToolResult, McpError> {
        check_initialized()?;
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
        check_initialized()?;
        // Convert string keys to Special enum
        let special_keys = keys
            .iter()
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
        #[tool(param)] _wait_for_seconds: Option<f32>,
    ) -> Result<CallToolResult, McpError> {
        check_initialized()?;
        // Enhanced version to handle nested action_json format from different clients
        // 1. JSON Object: {"command": "ls -la"}
        // 2. Simple String: "ls -la"
        // 3. Nested Object: {"action_json": {"command": "ls -la"}}
        // 4. String JSON: "{\"command\": \"ls -la\"}"

        info!("Bash command received: {:?}", action_json);

        let command = if action_json.is_string() {
            // If it's a simple string, check if it's a JSON string first
            let action_str = action_json.as_str().unwrap_or("");

            if action_str.starts_with('{') && action_str.contains("command") {
                // Try to parse the string as JSON
                match serde_json::from_str::<serde_json::Value>(action_str) {
                    Ok(parsed) => {
                        if let Some(cmd) = parsed.get("command") {
                            if cmd.is_string() {
                                cmd.as_str().unwrap_or("").to_string()
                            } else {
                                return Err(McpError::invalid_params(
                                    "command_format_error",
                                    Some(serde_json::json!({
                                        "error": "command must be a string in parsed JSON string"
                                    })),
                                ));
                            }
                        } else {
                            return Err(McpError::invalid_params(
                                "command_format_error",
                                Some(serde_json::json!({
                                    "error": "command field is required in parsed JSON string"
                                })),
                            ));
                        }
                    }
                    Err(_) => {
                        // If not a valid JSON, use the string as command directly
                        action_str.to_string()
                    }
                }
            } else {
                // Regular string command
                action_str.to_string()
            }
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
                    // If action_json is a string, try to parse it as JSON first
                    let nested_str = nested_action.as_str().unwrap_or("");

                    if nested_str.starts_with('{') && nested_str.contains("command") {
                        // Try to parse the string as JSON
                        match serde_json::from_str::<serde_json::Value>(nested_str) {
                            Ok(parsed) => {
                                if let Some(cmd) = parsed.get("command") {
                                    if cmd.is_string() {
                                        cmd.as_str().unwrap_or("").to_string()
                                    } else {
                                        return Err(McpError::invalid_params(
                                            "command_format_error",
                                            Some(serde_json::json!({
                                                "error": "command must be a string in nested JSON string"
                                            })),
                                        ));
                                    }
                                } else {
                                    return Err(McpError::invalid_params(
                                        "command_format_error",
                                        Some(serde_json::json!({
                                            "error": "command field is required in nested JSON string"
                                        })),
                                    ));
                                }
                            }
                            Err(_) => {
                                // If not a valid JSON, use the string as command directly
                                nested_str.to_string()
                            }
                        }
                    } else {
                        // Regular string command
                        nested_str.to_string()
                    }
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
        check_initialized()?;
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
        check_initialized()?;
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
        check_initialized()?;
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
    #[allow(clippy::too_many_arguments)]
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
        check_initialized()?;

        // Use a ThinkingParams struct in the process_thinking function
        // to avoid too many arguments warning
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

    #[tool(description = "Initialize the VibeCode agent with project understanding")]
    async fn init_vibe_code(
        &self,
        #[tool(param)] project_dir: String,
    ) -> Result<CallToolResult, McpError> {
        check_initialized()?;

        let request = vibe_code::InitVibeCodeRequest { project_dir };

        let json = match serde_json::to_string(&request) {
            Ok(j) => j,
            Err(e) => {
                return Err(McpError::internal_error(
                    "serialize_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        let result = vibe_code::init_vibe_code(&self.state, &json)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "vibe_code_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Analyze a file using the VibeCode agent")]
    async fn analyze_file_with_vibe_code(
        &self,
        #[tool(param)] file_path: String,
    ) -> Result<CallToolResult, McpError> {
        check_initialized()?;

        let request = vibe_code::AnalyzeFileRequest { file_path };

        let json = match serde_json::to_string(&request) {
            Ok(j) => j,
            Err(e) => {
                return Err(McpError::internal_error(
                    "serialize_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        let result = vibe_code::analyze_file(&self.state, &json)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "vibe_code_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Apply search/replace with intelligent error handling")]
    async fn smart_search_replace(
        &self,
        #[tool(param)] file_path: String,
        #[tool(param)] search_replace_blocks: String,
    ) -> Result<CallToolResult, McpError> {
        check_initialized()?;

        let request = vibe_code::SearchReplaceRequest {
            file_path,
            search_replace_blocks,
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

        let result = vibe_code::apply_search_replace(&self.state, &json)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "vibe_code_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Generate code suggestions based on project patterns")]
    async fn generate_code_suggestions(
        &self,
        #[tool(param)] file_path: String,
    ) -> Result<CallToolResult, McpError> {
        check_initialized()?;

        let request = vibe_code::CodeSuggestionsRequest { file_path };

        let json = match serde_json::to_string(&request) {
            Ok(j) => j,
            Err(e) => {
                return Err(McpError::internal_error(
                    "serialize_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        let result = vibe_code::generate_code_suggestions(&self.state, &json)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "vibe_code_error",
                    Some(serde_json::Value::String(e.to_string())),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Change the interface language (en, pt, es)")]
    async fn change_language(
        &self,
        #[tool(param)] language_code: String,
    ) -> Result<CallToolResult, McpError> {
        check_initialized()?;

        let request = crate::commands::language::LanguageRequest { language_code };

        let json = match serde_json::to_string(&request) {
            Ok(j) => j,
            Err(e) => {
                return Err(McpError::internal_error(
                    "serialize_error",
                    Some(serde_json::Value::String(e.to_string())),
                ))
            }
        };

        let result = crate::commands::language::change_language(&json).map_err(|e| {
            McpError::internal_error(
                "language_error",
                Some(serde_json::Value::String(e.to_string())),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List available languages and current language")]
    async fn list_languages(&self) -> Result<CallToolResult, McpError> {
        check_initialized()?;

        let result = crate::commands::language::list_available_languages().map_err(|e| {
            McpError::internal_error(
                "language_error",
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
        let current_lang = crate::core::i18n::get_language();

        // Customized instructions based on language
        let instructions = match current_lang {
            crate::core::i18n::Language::English =>
                "Winx is a Rust code agent that allows executing bash commands, manipulating files, and executing SQL queries.",
            crate::core::i18n::Language::Portuguese =>
                "Winx é um agente de código Rust que permite executar comandos bash, manipular arquivos e executar consultas SQL.",
            crate::core::i18n::Language::Spanish =>
                "Winx es un agente de código Rust que permite ejecutar comandos bash, manipular archivos y ejecutar consultas SQL.",
        };

        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(instructions.to_string()),
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

/// Gets the current internationalized description for the given tool ID
pub fn get_tool_description(tool_id: &str) -> &'static str {
    use crate::commands::localized_descriptions;

    match tool_id {
        "create_task" => localized_descriptions::create_task_description(),
        "list_tasks" => localized_descriptions::list_tasks_description(),
        "start_background_process" => {
            localized_descriptions::start_background_process_description()
        }
        "validate_syntax" => localized_descriptions::validate_syntax_description(),
        "send_text_input" => localized_descriptions::send_text_input_description(),
        "send_special_keys" => localized_descriptions::send_special_keys_description(),
        "bash_command" => localized_descriptions::bash_command_description(),
        "read_files" => localized_descriptions::read_files_description(),
        "file_write_or_edit" => localized_descriptions::file_write_or_edit_description(),
        "sql_query" => localized_descriptions::sql_query_description(),
        "sequential_thinking" => localized_descriptions::sequential_thinking_description(),
        "init_vibe_code" => localized_descriptions::init_vibe_code_description(),
        "analyze_file_with_vibe_code" => {
            localized_descriptions::analyze_file_with_vibe_code_description()
        }
        "smart_search_replace" => localized_descriptions::smart_search_replace_description(),
        "generate_code_suggestions" => {
            localized_descriptions::generate_code_suggestions_description()
        }
        _ => "Unknown tool",
    }
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
