use crate::error::WinxError;
use crate::lsp::{
    client::{LspClient, LspClientFactory},
    symbol::{Symbol, SymbolManager},
    utils::{
        format_symbol_info, read_file_content, replace_range, to_lsp_range, write_file_content,
    },
};
use crate::WinxResult;
use anyhow::Result;
use async_trait::async_trait;
use log::info;
use rmcp::model::{CallToolResult, ErrorCode};
use rmcp::{tool, Error as McpError, RoleServer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;

/// Cache of LSP clients for different projects
static LSP_CLIENT_CACHE: once_cell::sync::Lazy<Mutex<HashMap<String, Arc<TokioMutex<LspClient>>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

/// Get or create an LSP client for a project
async fn get_lsp_client(root_path: impl AsRef<Path>) -> WinxResult<Arc<TokioMutex<LspClient>>> {
    let root_path = root_path.as_ref();
    let root_key = root_path.to_string_lossy().to_string();

    // Try to get an existing client from the cache
    {
        let cache = LSP_CLIENT_CACHE.lock().unwrap();
        if let Some(client) = cache.get(&root_key) {
            return Ok(client.clone());
        }
    }

    // No client found, create a new one
    info!("Creating new LSP client for {}", root_key);

    // Determine the language server type based on the project
    // For now, we'll use RustAnalyzer as an example
    // In a real implementation, we'd detect the language from the project files
    let factory = LspClientFactory::new();

    // Find a suitable file to determine the language server type
    let file_types = [
        "rs", "py", "js", "ts", "java", "go", "rb", "cs", "cpp", "c", "h",
    ];
    let mut file_path = None;

    for ext in &file_types {
        let glob_pattern = format!("{}/**/*.{}", root_path.display(), ext);
        if let Ok(paths) = glob::glob(&glob_pattern) {
            if let Some(path) = paths.take(1).next() {
                if let Ok(path) = path {
                    file_path = Some(path);
                    break;
                }
            }
        }
    }

    if file_path.is_none() {
        return Err(WinxError::lsp_error(
            "No suitable files found to determine language server type",
        ));
    }

    let client = Arc::new(TokioMutex::new(
        factory
            .create_client_for_file(file_path.unwrap(), root_path)
            .await?,
    ));

    // Cache the client
    {
        let mut cache = LSP_CLIENT_CACHE.lock().unwrap();
        cache.insert(root_key, client.clone());
    }

    Ok(client)
}

/// Tool for finding symbols in the codebase
#[derive(Debug, Default)]
pub struct FindSymbolTool {}

impl FindSymbolTool {
    pub fn new() -> Self {
        Self {}
    }
}

/// Parameters for finding symbols
#[derive(Debug, Deserialize)]
pub struct FindSymbolParams {
    /// Name of the symbol to find
    pub name: String,
    /// Path relative to workspace root to search within (optional)
    pub within_relative_path: Option<String>,
    /// Whether to include the bodies of the symbols in the result
    pub include_body: Option<bool>,
    /// Types of symbols to include (e.g., "class", "function", "method", etc.)
    pub include_types: Option<Vec<String>>,
    /// Whether to use substring matching
    pub substring_matching: Option<bool>,
    /// Maximum number of results to return (0 for all)
    pub max_results: Option<usize>,
}

#[tool(tool_box)]
impl FindSymbolTool {
    #[tool(description = "Find symbols by name in the codebase with semantic understanding.")]
    pub async fn find_symbol(&self, params: FindSymbolParams) -> Result<CallToolResult, McpError> {
        // Get the workspace root from the agent context
        let context = rmcp::service::get_context().ok_or_else(|| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                "No context available".to_string(),
                None,
            )
        })?;

        let agent_context = context
            .get_tool_data::<crate::tools::AgentContext>()
            .ok_or_else(|| {
                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    "Agent context not available".to_string(),
                    None,
                )
            })?;

        let workspace_root = PathBuf::from(&agent_context.cwd);
        if !workspace_root.exists() {
            return Err(McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!(
                    "Workspace root does not exist: {}",
                    workspace_root.display()
                ),
                None,
            ));
        }

        // Get or create an LSP client for the project
        let lsp_client = get_lsp_client(&workspace_root)
            .await
            .map_err(|e| e.to_mcp_error())?;

        // Create a symbol manager
        let lsp_client_lock = lsp_client.lock().await;
        let symbol_manager = SymbolManager::new(lsp_client_lock.clone(), &workspace_root);

        // Get the symbols from the LSP client
        let search_path = params
            .within_relative_path
            .unwrap_or_else(|| ".".to_string());

        // If search_path is a file, get symbols from that file
        // Otherwise, use a glob pattern to find files
        let mut symbols = Vec::new();
        let path = workspace_root.join(&search_path);

        if path.is_file() {
            let relative_path = search_path.clone();
            let file_symbols = symbol_manager
                .get_document_symbols(&relative_path)
                .await
                .map_err(|e| e.to_mcp_error())?;
            symbols.extend(file_symbols);
        } else if path.is_dir() {
            // For each supported file type, find matching files in the directory
            let file_types = [
                "rs", "py", "js", "ts", "java", "go", "rb", "cs", "cpp", "c", "h",
            ];
            for ext in &file_types {
                let glob_pattern = format!("{}/**/*.{}", path.display(), ext);
                if let Ok(paths) = glob::glob(&glob_pattern) {
                    for path_result in paths.take(100) {
                        if let Ok(file_path) = path_result {
                            if let Ok(relative_path) = file_path.strip_prefix(&workspace_root) {
                                let relative_path_str = relative_path.to_string_lossy().to_string();
                                match symbol_manager
                                    .get_document_symbols(&relative_path_str)
                                    .await
                                {
                                    Ok(file_symbols) => symbols.extend(file_symbols),
                                    Err(e) => {
                                        info!(
                                            "Failed to get symbols for {}: {}",
                                            relative_path_str, e
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Find symbols matching the search criteria
        let include_body = params.include_body.unwrap_or(false);
        let substring_matching = params.substring_matching.unwrap_or(false);

        let mut matching_symbols = Symbol::find_by_name(&symbols, &params.name, substring_matching);

        // Filter by symbol type if specified
        if let Some(types) = &params.include_types {
            matching_symbols.retain(|symbol| {
                let kind_str = format!("{:?}", symbol.kind).to_lowercase();
                types.iter().any(|t| kind_str.contains(&t.to_lowercase()))
            });
        }

        // Limit the number of results if specified
        if let Some(max_results) = params.max_results {
            if max_results > 0 && matching_symbols.len() > max_results {
                matching_symbols.truncate(max_results);
            }
        }

        // Add body content if requested
        if include_body {
            for symbol in &mut matching_symbols {
                match symbol.get_body(&workspace_root) {
                    Ok(body) => symbol.body = Some(body),
                    Err(e) => {
                        info!("Failed to get body for {}: {}", symbol.name, e);
                    }
                }
            }
        }

        // Convert the symbols to a serializable format
        let result = matching_symbols
            .iter()
            .map(|symbol| symbol.to_dict(include_body, true, 2))
            .collect::<Vec<_>>();

        let result_json = serde_json::to_string(&result).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize result: {}", e),
                None,
            )
        })?;

        Ok(CallToolResult::from(result_json))
    }
}

/// Tool for finding references to a symbol
#[derive(Debug, Default)]
pub struct FindReferencesTool {}

impl FindReferencesTool {
    pub fn new() -> Self {
        Self {}
    }
}

/// Parameters for finding references
#[derive(Debug, Deserialize)]
pub struct FindReferencesParams {
    /// Path to the file containing the symbol
    pub file_path: String,
    /// Line number (0-based) of the symbol
    pub line: u32,
    /// Column (0-based) of the symbol
    pub column: u32,
    /// Whether to include the bodies of the references in the result
    pub include_body: Option<bool>,
}

#[tool(tool_box)]
impl FindReferencesTool {
    #[tool(description = "Find references to a symbol in the codebase.")]
    pub async fn find_references(
        &self,
        params: FindReferencesParams,
    ) -> Result<CallToolResult, McpError> {
        // Get the workspace root from the agent context
        let context = rmcp::service::get_context().ok_or_else(|| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                "No context available".to_string(),
                None,
            )
        })?;

        let agent_context = context
            .get_tool_data::<crate::tools::AgentContext>()
            .ok_or_else(|| {
                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    "Agent context not available".to_string(),
                    None,
                )
            })?;

        let workspace_root = PathBuf::from(&agent_context.cwd);
        if !workspace_root.exists() {
            return Err(McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!(
                    "Workspace root does not exist: {}",
                    workspace_root.display()
                ),
                None,
            ));
        }

        // Get or create an LSP client for the project
        let lsp_client = get_lsp_client(&workspace_root)
            .await
            .map_err(|e| e.to_mcp_error())?;

        // Create a symbol manager
        let lsp_client_lock = lsp_client.lock().await;
        let symbol_manager = SymbolManager::new(lsp_client_lock.clone(), &workspace_root);

        // Find the symbol at the given location
        let relative_path = params.file_path.trim_start_matches('/');
        let symbol = symbol_manager
            .find_symbol_at_location(relative_path, params.line, params.column)
            .await
            .map_err(|e| e.to_mcp_error())?
            .ok_or_else(|| {
                McpError::new(
                    ErrorCode::INVALID_PARAMS,
                    format!(
                        "No symbol found at {}:{}:{}",
                        relative_path, params.line, params.column
                    ),
                    None,
                )
            })?;

        // Find references to the symbol
        let references = symbol_manager
            .find_references(&symbol)
            .await
            .map_err(|e| e.to_mcp_error())?;

        // Convert the references to a serializable format
        let result = serde_json::json!({
            "symbol": {
                "name": symbol.name,
                "kind": format!("{:?}", symbol.kind),
                "location": {
                    "file": symbol.location.relative_path,
                    "line": symbol.location.line,
                    "column": symbol.location.column,
                }
            },
            "references": references,
            "count": references.len(),
        });

        let result_json = serde_json::to_string(&result).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize result: {}", e),
                None,
            )
        })?;

        Ok(CallToolResult::from(result_json))
    }
}

/// Tool for editing a symbol
#[derive(Debug, Default)]
pub struct EditSymbolTool {}

impl EditSymbolTool {
    pub fn new() -> Self {
        Self {}
    }
}

/// Parameters for editing a symbol
#[derive(Debug, Deserialize)]
pub struct EditSymbolParams {
    /// Path to the file containing the symbol
    pub file_path: String,
    /// Line number (0-based) of the symbol
    pub line: u32,
    /// Column (0-based) of the symbol
    pub column: u32,
    /// New body content for the symbol
    pub new_body: String,
}

#[tool(tool_box)]
impl EditSymbolTool {
    #[tool(description = "Edit a symbol in the codebase with semantic understanding.")]
    pub async fn edit_symbol(&self, params: EditSymbolParams) -> Result<CallToolResult, McpError> {
        // Get the workspace root from the agent context
        let context = rmcp::service::get_context().ok_or_else(|| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                "No context available".to_string(),
                None,
            )
        })?;

        let agent_context = context
            .get_tool_data::<crate::tools::AgentContext>()
            .ok_or_else(|| {
                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    "Agent context not available".to_string(),
                    None,
                )
            })?;

        let workspace_root = PathBuf::from(&agent_context.cwd);
        if !workspace_root.exists() {
            return Err(McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!(
                    "Workspace root does not exist: {}",
                    workspace_root.display()
                ),
                None,
            ));
        }

        // Get or create an LSP client for the project
        let lsp_client = get_lsp_client(&workspace_root)
            .await
            .map_err(|e| e.to_mcp_error())?;

        // Create a symbol manager
        let lsp_client_lock = lsp_client.lock().await;
        let symbol_manager = SymbolManager::new(lsp_client_lock.clone(), &workspace_root);

        // Find the symbol at the given location
        let relative_path = params.file_path.trim_start_matches('/');
        let symbol = symbol_manager
            .find_symbol_at_location(relative_path, params.line, params.column)
            .await
            .map_err(|e| e.to_mcp_error())?
            .ok_or_else(|| {
                McpError::new(
                    ErrorCode::INVALID_PARAMS,
                    format!(
                        "No symbol found at {}:{}:{}",
                        relative_path, params.line, params.column
                    ),
                    None,
                )
            })?;

        // Read the file content
        let file_path = workspace_root.join(relative_path);
        let content = read_file_content(&file_path).map_err(|e| e.to_mcp_error())?;

        // Replace the symbol body
        let range = to_lsp_range(&symbol.location.range);
        let new_content = replace_range(&content, &range, &params.new_body);

        // Write the updated content back to the file
        write_file_content(&file_path, &new_content).map_err(|e| e.to_mcp_error())?;

        let result = serde_json::json!({
            "success": true,
            "message": format!("Updated symbol '{}' in '{}'", symbol.name, relative_path),
            "symbol": {
                "name": symbol.name,
                "kind": format!("{:?}", symbol.kind),
                "location": {
                    "file": symbol.location.relative_path,
                    "line": symbol.location.line,
                    "column": symbol.location.column,
                }
            }
        });

        let result_json = serde_json::to_string(&result).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize result: {}", e),
                None,
            )
        })?;

        Ok(CallToolResult::from(result_json))
    }
}

/// Tool for adding a new symbol after an existing one
#[derive(Debug, Default)]
pub struct AddSymbolTool {}

impl AddSymbolTool {
    pub fn new() -> Self {
        Self {}
    }
}

/// Parameters for adding a symbol
#[derive(Debug, Deserialize)]
pub struct AddSymbolParams {
    /// Path to the file containing the anchor symbol (or file)
    pub file_path: String,
    /// Line number (0-based) of the anchor symbol (optional)
    pub line: Option<u32>,
    /// Column (0-based) of the anchor symbol (optional)
    pub column: Option<u32>,
    /// Position type (before/after anchor symbol, or at specific line)
    pub position: String,
    /// Content of the new symbol to add
    pub content: String,
}

#[tool(tool_box)]
impl AddSymbolTool {
    #[tool(description = "Add a new symbol to the codebase with semantic understanding.")]
    pub async fn add_symbol(&self, params: AddSymbolParams) -> Result<CallToolResult, McpError> {
        // Get the workspace root from the agent context
        let context = rmcp::service::get_context().ok_or_else(|| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                "No context available".to_string(),
                None,
            )
        })?;

        let agent_context = context
            .get_tool_data::<crate::tools::AgentContext>()
            .ok_or_else(|| {
                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    "Agent context not available".to_string(),
                    None,
                )
            })?;

        let workspace_root = PathBuf::from(&agent_context.cwd);
        if !workspace_root.exists() {
            return Err(McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!(
                    "Workspace root does not exist: {}",
                    workspace_root.display()
                ),
                None,
            ));
        }

        // Read the file content
        let relative_path = params.file_path.trim_start_matches('/');
        let file_path = workspace_root.join(relative_path);
        let mut content = read_file_content(&file_path).map_err(|e| e.to_mcp_error())?;

        // Handle different position types
        match params.position.to_lowercase().as_str() {
            "before" | "after" => {
                // Need to find the anchor symbol
                if params.line.is_none() || params.column.is_none() {
                    return Err(McpError::new(
                        ErrorCode::INVALID_PARAMS,
                        "Line and column must be provided for before/after positioning".to_string(),
                        None,
                    ));
                }

                // Get or create an LSP client for the project
                let lsp_client = get_lsp_client(&workspace_root)
                    .await
                    .map_err(|e| e.to_mcp_error())?;

                // Create a symbol manager
                let lsp_client_lock = lsp_client.lock().await;
                let symbol_manager = SymbolManager::new(lsp_client_lock.clone(), &workspace_root);

                // Find the symbol at the given location
                let symbol = symbol_manager
                    .find_symbol_at_location(
                        relative_path,
                        params.line.unwrap(),
                        params.column.unwrap(),
                    )
                    .await
                    .map_err(|e| e.to_mcp_error())?
                    .ok_or_else(|| {
                        McpError::new(
                            ErrorCode::INVALID_PARAMS,
                            format!(
                                "No symbol found at {}:{}:{}",
                                relative_path,
                                params.line.unwrap(),
                                params.column.unwrap()
                            ),
                            None,
                        )
                    })?;

                // Get the position to insert the new content
                let range = to_lsp_range(&symbol.location.range);
                let insert_pos = if params.position == "before" {
                    range.start
                } else {
                    range.end
                };

                // Insert the new content
                let new_content = format!("{}\n", params.content);
                let lines: Vec<&str> = content.lines().collect();
                let mut result = String::new();

                for (i, line) in lines.iter().enumerate() {
                    let line_num = i as u32;

                    if line_num == insert_pos.line {
                        if params.position == "before" {
                            result.push_str(&new_content);
                            result.push_str(line);
                            result.push('\n');
                        } else {
                            result.push_str(line);
                            result.push('\n');
                            result.push_str(&new_content);
                        }
                    } else {
                        result.push_str(line);
                        result.push('\n');
                    }
                }

                content = result;
            }
            "at" => {
                // Insert at a specific line
                if params.line.is_none() {
                    return Err(McpError::new(
                        ErrorCode::INVALID_PARAMS,
                        "Line must be provided for 'at' positioning".to_string(),
                        None,
                    ));
                }

                let line_num = params.line.unwrap() as usize;
                let lines: Vec<&str> = content.lines().collect();
                let mut result = String::new();

                // Ensure the line number is valid
                if line_num > lines.len() {
                    return Err(McpError::new(
                        ErrorCode::INVALID_PARAMS,
                        format!(
                            "Line number {} is out of range (max: {})",
                            line_num,
                            lines.len()
                        ),
                        None,
                    ));
                }

                // Insert the new content at the specified line
                for (i, line) in lines.iter().enumerate() {
                    if i == line_num {
                        result.push_str(&params.content);
                        result.push('\n');
                    }
                    result.push_str(line);
                    result.push('\n');
                }

                content = result;
            }
            _ => {
                return Err(McpError::new(
                    ErrorCode::INVALID_PARAMS,
                    format!("Invalid position type: {}", params.position),
                    None,
                ));
            }
        }

        // Write the updated content back to the file
        write_file_content(&file_path, &content).map_err(|e| e.to_mcp_error())?;

        let result = serde_json::json!({
            "success": true,
            "message": format!("Added new symbol in '{}' at position '{}'", relative_path, params.position),
            "file_path": relative_path,
        });

        let result_json = serde_json::to_string(&result).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize result: {}", e),
                None,
            )
        })?;

        Ok(CallToolResult::from(result_json))
    }
}
