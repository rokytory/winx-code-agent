use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::lsp::client::LSPClient;
use crate::lsp::server::LSPServer;
use crate::lsp::types::{Position, Range, Symbol, SymbolKind, SymbolLocation};

/// Converts a file URI to a PathBuf
pub fn uri_to_path(uri: &str) -> Result<PathBuf> {
    if !uri.starts_with("file://") {
        return Err(anyhow::anyhow!("URI is not a file URI: {}", uri));
    }

    let path_str = uri.trim_start_matches("file://");
    let path = Path::new(path_str);

    Ok(path.to_path_buf())
}

/// Converts a PathBuf to a file URI
pub fn path_to_uri(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    format!("file://{}", path_str)
}

/// SymbolManager provides a unified interface for symbol-based operations
/// across different language servers, combining functionality previously
/// split between lsp/symbol.rs and code/symbol.rs
pub struct SymbolManager {
    /// LSP client for direct protocol operations
    lsp_client: Option<LSPClient>,

    /// LSP server for high-level operations
    lsp_server: Option<Arc<Mutex<LSPServer>>>,

    /// Root path of the project
    root_path: PathBuf,
}

impl SymbolManager {
    /// Creates a new symbol manager using an LSP client directly
    pub fn new_with_client(lsp_client: LSPClient, root_path: impl AsRef<Path>) -> Self {
        Self {
            lsp_client: Some(lsp_client),
            lsp_server: None,
            root_path: root_path.as_ref().to_path_buf(),
        }
    }

    /// Creates a new symbol manager using an LSP server
    pub fn new_with_server(lsp_server: Arc<Mutex<LSPServer>>, root_path: impl AsRef<Path>) -> Self {
        Self {
            lsp_client: None,
            lsp_server: Some(lsp_server),
            root_path: root_path.as_ref().to_path_buf(),
        }
    }

    /// Gets the LSP client if available
    pub fn get_lsp_client(&self) -> Result<&LSPClient> {
        self.lsp_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("LSP client not available"))
    }

    /// Gets document symbols for a file
    pub async fn get_document_symbols(
        &self,
        file_path: impl AsRef<Path>,
        include_body: bool,
    ) -> Result<Vec<Symbol>> {
        let file_path = file_path.as_ref();
        debug!("Getting document symbols for {}", file_path.display());

        if let Some(lsp_client) = &self.lsp_client {
            let uri = path_to_uri(file_path);
            let params = serde_json::json!({
                "textDocument": {
                    "uri": uri
                }
            });

            let response = lsp_client
                .send_request("textDocument/documentSymbol", params)
                .await?;
            let symbols: Vec<Symbol> = serde_json::from_value(response)?;

            info!("Found {} symbols in {}", symbols.len(), file_path.display());
            Ok(symbols)
        } else if let Some(_server) = &self.lsp_server {
            let server_guard = _server.lock().await;
            let relative_path = self.to_relative_path(file_path)?;

            let symbols = server_guard
                .get_document_symbols(&relative_path.to_string_lossy(), include_body)
                .await?;

            info!("Found {} symbols in {}", symbols.len(), file_path.display());
            Ok(symbols)
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Finds symbols matching a query
    pub async fn find_symbols(&self, query: &str) -> Result<Vec<Symbol>> {
        debug!("Finding symbols matching query: {}", query);

        if let Some(lsp_client) = &self.lsp_client {
            let params = serde_json::json!({
                "query": query
            });

            let response = lsp_client.send_request("workspace/symbol", params).await?;
            let symbols: Vec<Symbol> = serde_json::from_value(response)?;

            info!("Found {} symbols matching '{}'", symbols.len(), query);
            Ok(symbols)
        } else if let Some(server) = &self.lsp_server {
            // Use server to find symbols
            let server_guard = server.lock().await;

            // Server API might be slightly different, so we adapt
            let symbols = server_guard.find_symbol(query, None, false).await?;

            info!("Found {} symbols matching '{}'", symbols.len(), query);
            Ok(symbols)
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Find symbols by name in the workspace
    pub async fn find_by_name(
        &self,
        name: &str,
        within_path: Option<impl AsRef<Path>>,
        include_body: bool,
        substring_matching: bool,
        include_kinds: Option<Vec<SymbolKind>>,
        exclude_kinds: Option<Vec<SymbolKind>>,
    ) -> Result<Vec<Symbol>> {
        info!(
            "Finding symbols with name '{}'{}",
            name,
            if substring_matching {
                " (substring matching)"
            } else {
                ""
            }
        );

        let within_path_buf = within_path.map(|p| p.as_ref().to_path_buf());

        if let Some(server) = &self.lsp_server {
            let server_guard = server.lock().await;

            let symbols = server_guard
                .find_symbol(name, within_path_buf, include_body)
                .await?;

            // Filter symbols based on criteria
            let filtered_symbols = symbols
                .into_iter()
                .filter(|s| {
                    // Apply substring matching if needed
                    if !substring_matching && s.name != name {
                        return false;
                    }

                    // Filter by included kinds
                    if let Some(ref kinds) = include_kinds {
                        if !kinds.contains(&s.kind) {
                            return false;
                        }
                    }

                    // Filter by excluded kinds
                    if let Some(ref kinds) = exclude_kinds {
                        if kinds.contains(&s.kind) {
                            return false;
                        }
                    }

                    true
                })
                .collect();

            Ok(filtered_symbols)
        } else {
            // Fallback to workspace/symbol if LSP client is available
            let symbols = self.find_symbols(name).await?;

            // Apply additional filtering
            let filtered_symbols = symbols
                .into_iter()
                .filter(|s| {
                    // Apply substring matching if needed
                    if !substring_matching && s.name != name {
                        return false;
                    }

                    // Filter by paths if requested
                    if let Some(ref within) = within_path_buf {
                        if let Ok((path, _)) = s.path_and_range() {
                            if !path.starts_with(within) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }

                    // Filter by included kinds
                    if let Some(ref kinds) = include_kinds {
                        if !kinds.contains(&s.kind) {
                            return false;
                        }
                    }

                    // Filter by excluded kinds
                    if let Some(ref kinds) = exclude_kinds {
                        if kinds.contains(&s.kind) {
                            return false;
                        }
                    }

                    true
                })
                .collect();

            Ok(filtered_symbols)
        }
    }

    /// Gets the definition for a symbol at a specific position
    pub async fn get_definition(
        &self,
        file_path: impl AsRef<Path>,
        position: Position,
    ) -> Result<Vec<SymbolLocation>> {
        let file_path = file_path.as_ref();
        debug!(
            "Getting definition at {}:{} in {}",
            position.line,
            position.character,
            file_path.display()
        );

        if let Some(lsp_client) = &self.lsp_client {
            let uri = path_to_uri(file_path);
            let params = serde_json::json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": position.line,
                    "character": position.character
                }
            });

            let _response = lsp_client
                .send_request("textDocument/definition", params)
                .await?;

            // Convert from Location to SymbolLocation
            // This conversion would need to be implemented based on the response format
            unimplemented!("Conversion from LSP Location to SymbolLocation not implemented yet");
        } else if let Some(_server) = &self.lsp_server {
            // Use server to get definitions
            // Implementation would depend on server API
            unimplemented!("Get definition using LSP server not implemented yet");
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Find symbols that reference the symbol at the given location
    pub async fn find_referencing_symbols(
        &self,
        location: &SymbolLocation,
        include_body: bool,
        include_kinds: Option<Vec<SymbolKind>>,
        exclude_kinds: Option<Vec<SymbolKind>>,
    ) -> Result<Vec<Symbol>> {
        // Ensure we have a valid location
        if location.relative_path.is_none() || location.line.is_none() || location.column.is_none()
        {
            return Err(anyhow::anyhow!(
                "Invalid symbol location - missing path, line, or column"
            ));
        }

        let relative_path = location.relative_path.as_ref().unwrap();
        let line = location.line.unwrap();
        let column = location.column.unwrap();

        info!(
            "Finding references to symbol at {}:{}:{}",
            relative_path, line, column
        );

        if let Some(server) = &self.lsp_server {
            let server_guard = server.lock().await;
            let symbols = server_guard
                .find_references(location.clone(), include_body)
                .await?;

            // Filter by kinds if specified
            let filtered_symbols = if include_kinds.is_some() || exclude_kinds.is_some() {
                symbols
                    .into_iter()
                    .filter(|s| {
                        if let Some(ref kinds) = include_kinds {
                            if !kinds.contains(&s.kind) {
                                return false;
                            }
                        }

                        if let Some(ref kinds) = exclude_kinds {
                            if kinds.contains(&s.kind) {
                                return false;
                            }
                        }

                        true
                    })
                    .collect()
            } else {
                symbols
            };

            Ok(filtered_symbols)
        } else if let Some(_lsp_client) = &self.lsp_client {
            // Implementation would depend on client API
            unimplemented!("Find referencing symbols using LSP client not implemented yet");
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Gets the hover information for a symbol at a specific position
    pub async fn get_hover(
        &self,
        file_path: impl AsRef<Path>,
        position: Position,
    ) -> Result<Option<String>> {
        let file_path = file_path.as_ref();
        debug!(
            "Getting hover info at {}:{}",
            position.line, position.character
        );

        if let Some(lsp_client) = &self.lsp_client {
            let uri = path_to_uri(file_path);
            let params = serde_json::json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": position.line,
                    "character": position.character
                }
            });

            let response = lsp_client
                .send_request("textDocument/hover", params)
                .await?;

            // If no hover information is available, we might get null
            if response.is_null() {
                return Ok(None);
            }

            // Extract the contents from the hover response
            let contents = response
                .get("contents")
                .ok_or_else(|| anyhow::anyhow!("Missing 'contents' in hover response"))?;

            // The contents can be in different formats
            let hover_text = if contents.is_string() {
                contents.as_str().unwrap().to_string()
            } else if contents.is_object() {
                // It might be a MarkedString or MarkupContent
                if let Some(value) = contents.get("value") {
                    value.as_str().unwrap_or("").to_string()
                } else {
                    serde_json::to_string(contents)?
                }
            } else if contents.is_array() {
                // It might be an array of MarkedString
                let contents_array = contents.as_array().unwrap();
                let texts: Vec<String> = contents_array
                    .iter()
                    .map(|item| {
                        if item.is_string() {
                            item.as_str().unwrap().to_string()
                        } else if let Some(value) = item.get("value") {
                            value.as_str().unwrap_or("").to_string()
                        } else {
                            "".to_string()
                        }
                    })
                    .filter(|s| !s.is_empty())
                    .collect();

                texts.join("\n\n")
            } else {
                "".to_string()
            };

            if hover_text.is_empty() {
                Ok(None)
            } else {
                Ok(Some(hover_text))
            }
        } else if let Some(_server) = &self.lsp_server {
            // Implementation would depend on server API
            unimplemented!("Get hover using LSP server not implemented yet");
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Gets the source code of a symbol
    pub async fn get_symbol_source(&self, symbol: &Symbol) -> Result<String> {
        let (file_path, range) = symbol.path_and_range()?;

        // Read the file content
        let content = std::fs::read_to_string(&file_path)
            .context(format!("Failed to read file: {}", file_path.display()))?;

        // Extract the relevant lines
        let lines: Vec<&str> = content.lines().collect();
        let start_line = range.start.line;
        let end_line = range.end.line;

        if start_line >= lines.len() {
            return Err(anyhow::anyhow!("Invalid start line: {}", start_line));
        }

        let end_line = std::cmp::min(end_line, lines.len() - 1);

        let source_lines = &lines[start_line..=end_line];
        let mut source = source_lines.join("\n");

        // Apply character offsets for the first and last line
        if start_line == end_line {
            let start_char = range.start.character;
            let end_char = range.end.character;

            if !source.is_empty() && start_char < source.len() {
                let end_char = std::cmp::min(end_char, source.len());
                source = source[start_char..end_char].to_string();
            }
        }

        Ok(source)
    }

    /// Replace the body of a symbol
    pub async fn replace_body(&self, location: &SymbolLocation, new_body: &str) -> Result<()> {
        info!("Replacing symbol body");

        // Ensure we have a valid location
        if location.relative_path.is_none() || location.line.is_none() || location.column.is_none()
        {
            return Err(anyhow::anyhow!(
                "Invalid symbol location - missing path, line, or column"
            ));
        }

        if let Some(server) = &self.lsp_server {
            let server_guard = server.lock().await;
            server_guard
                .replace_symbol_body(location.clone(), new_body)
                .await?;

            Ok(())
        } else if let Some(_lsp_client) = &self.lsp_client {
            // Implementation would depend on client API
            unimplemented!("Replace symbol body using LSP client not implemented yet");
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Insert text after a symbol
    pub async fn insert_after(&self, location: &SymbolLocation, content: &str) -> Result<()> {
        info!("Inserting text after symbol");

        // Ensure we have a valid location
        if location.relative_path.is_none() || location.line.is_none() || location.column.is_none()
        {
            return Err(anyhow::anyhow!(
                "Invalid symbol location - missing path, line, or column"
            ));
        }

        if let Some(server) = &self.lsp_server {
            let server_guard = server.lock().await;
            server_guard
                .insert_after_symbol(location.clone(), content)
                .await?;

            Ok(())
        } else if let Some(_lsp_client) = &self.lsp_client {
            // Implementation would depend on client API
            unimplemented!("Insert after symbol using LSP client not implemented yet");
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Insert text before a symbol
    pub async fn insert_before(&self, location: &SymbolLocation, content: &str) -> Result<()> {
        info!("Inserting text before symbol");

        // Ensure we have a valid location
        if location.relative_path.is_none() || location.line.is_none() || location.column.is_none()
        {
            return Err(anyhow::anyhow!(
                "Invalid symbol location - missing path, line, or column"
            ));
        }

        if let Some(server) = &self.lsp_server {
            let server_guard = server.lock().await;
            server_guard
                .insert_before_symbol(location.clone(), content)
                .await?;

            Ok(())
        } else if let Some(_lsp_client) = &self.lsp_client {
            // Implementation would depend on client API
            unimplemented!("Insert before symbol using LSP client not implemented yet");
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Insert text at a specific line in a file
    pub async fn insert_at_line(
        &self,
        relative_path: &str,
        line: usize,
        content: &str,
    ) -> Result<()> {
        info!("Inserting text at line {} in file {}", line, relative_path);

        if let Some(server) = &self.lsp_server {
            let server_guard = server.lock().await;
            server_guard
                .insert_text_at_position(relative_path, line, 0, content)
                .await?;

            Ok(())
        } else if let Some(_lsp_client) = &self.lsp_client {
            // Implementation would depend on client API
            unimplemented!("Insert at line using LSP client not implemented yet");
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Delete lines in a file
    pub async fn delete_lines(
        &self,
        relative_path: &str,
        start_line: usize,
        end_line: usize,
    ) -> Result<()> {
        info!(
            "Deleting lines {} to {} in file {}",
            start_line, end_line, relative_path
        );

        if let Some(server) = &self.lsp_server {
            let server_guard = server.lock().await;
            server_guard
                .delete_text_between_positions(relative_path, start_line, 0, end_line + 1, 0)
                .await?;

            Ok(())
        } else if let Some(_lsp_client) = &self.lsp_client {
            // Implementation would depend on client API
            unimplemented!("Delete lines using LSP client not implemented yet");
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Applies an edit to a symbol's source code
    pub async fn edit_symbol(&self, symbol: &Symbol, new_source: &str) -> Result<()> {
        if let Some(lsp_client) = &self.lsp_client {
            let (file_path, range) = symbol.path_and_range()?;

            // Create the edit
            let edit = serde_json::json!({
                "changes": {
                    path_to_uri(&file_path): [
                        {
                            "range": range,
                            "newText": new_source
                        }
                    ]
                }
            });

            // Apply the edit
            let response = lsp_client.send_request("workspace/applyEdit", edit).await?;

            // Check if the edit was applied successfully
            let applied = response
                .get("applied")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if !applied {
                let failure_reason = response
                    .get("failureReason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown reason");

                return Err(anyhow::anyhow!("Failed to apply edit: {}", failure_reason));
            }

            info!("Successfully edited symbol '{}'", symbol.name);
            Ok(())
        } else if let Some(_server) = &self.lsp_server {
            // Convert Symbol to SymbolLocation and call replace_body
            // This is a simplification - actual implementation might be more complex
            let location = SymbolLocation {
                relative_path: Some(symbol.location.relative_path.clone().unwrap_or_default()),
                line: Some(symbol.range.start.line),
                column: Some(symbol.range.start.character),
            };

            self.replace_body(&location, new_source).await
        } else {
            Err(anyhow::anyhow!("No LSP client or server available"))
        }
    }

    /// Converts an absolute path to a relative path within the project
    fn to_relative_path(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref();

        if path.is_absolute() {
            // Make path relative to root_path
            if let Ok(rel_path) = path.strip_prefix(&self.root_path) {
                Ok(rel_path.to_path_buf())
            } else {
                Err(anyhow::anyhow!(
                    "Path {} is not within the project root {}",
                    path.display(),
                    self.root_path.display()
                ))
            }
        } else {
            // Already relative, just return it
            Ok(path.to_path_buf())
        }
    }
}

// Extend Symbol to add helpful methods
impl Symbol {
    /// Returns the path and range of the symbol
    pub fn path_and_range(&self) -> Result<(PathBuf, Range)> {
        // Convert the symbol location to a path and range
        let relative_path = self
            .location
            .relative_path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Symbol location is missing relative_path"))?;

        Ok((PathBuf::from(relative_path), self.range.clone()))
    }

    /// Returns a human-readable description of the symbol
    pub fn description(&self) -> String {
        let kind_label = match self.kind {
            SymbolKind::File => "File",
            SymbolKind::Module => "Module",
            SymbolKind::Namespace => "Namespace",
            SymbolKind::Package => "Package",
            SymbolKind::Class => "Class",
            SymbolKind::Method => "Method",
            SymbolKind::Property => "Property",
            SymbolKind::Field => "Field",
            SymbolKind::Constructor => "Constructor",
            SymbolKind::Enum => "Enum",
            SymbolKind::Interface => "Interface",
            SymbolKind::Function => "Function",
            SymbolKind::Variable => "Variable",
            SymbolKind::Constant => "Constant",
            SymbolKind::String => "String",
            SymbolKind::Number => "Number",
            SymbolKind::Boolean => "Boolean",
            SymbolKind::Array => "Array",
            SymbolKind::Object => "Object",
            SymbolKind::Key => "Key",
            SymbolKind::Null => "Null",
            SymbolKind::EnumMember => "EnumMember",
            SymbolKind::Struct => "Struct",
            SymbolKind::Event => "Event",
            SymbolKind::Operator => "Operator",
            SymbolKind::TypeParameter => "TypeParameter",
        };

        format!("{} {}", kind_label, self.name)
    }
}
