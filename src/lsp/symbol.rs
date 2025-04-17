use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::lsp::client::LspClient;

/// Represents a symbol kind as defined in LSP
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
    Unknown = 0,
}

impl SymbolKind {
    /// Converts a numeric value to a SymbolKind
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => SymbolKind::File,
            2 => SymbolKind::Module,
            3 => SymbolKind::Namespace,
            4 => SymbolKind::Package,
            5 => SymbolKind::Class,
            6 => SymbolKind::Method,
            7 => SymbolKind::Property,
            8 => SymbolKind::Field,
            9 => SymbolKind::Constructor,
            10 => SymbolKind::Enum,
            11 => SymbolKind::Interface,
            12 => SymbolKind::Function,
            13 => SymbolKind::Variable,
            14 => SymbolKind::Constant,
            15 => SymbolKind::String,
            16 => SymbolKind::Number,
            17 => SymbolKind::Boolean,
            18 => SymbolKind::Array,
            19 => SymbolKind::Object,
            20 => SymbolKind::Key,
            21 => SymbolKind::Null,
            22 => SymbolKind::EnumMember,
            23 => SymbolKind::Struct,
            24 => SymbolKind::Event,
            25 => SymbolKind::Operator,
            26 => SymbolKind::TypeParameter,
            _ => SymbolKind::Unknown,
        }
    }

    /// Returns a human-readable label for the symbol kind
    pub fn label(&self) -> &'static str {
        match self {
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
            SymbolKind::Unknown => "Unknown",
        }
    }
}

/// Represents a position in a file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Line number (0-based)
    pub line: u32,
    /// Character offset within the line (0-based)
    pub character: u32,
}

/// Represents a range in a file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    /// Start position
    pub start: Position,
    /// End position
    pub end: Position,
}

/// Represents a location in a file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// File URI
    pub uri: String,
    /// Range within the file
    pub range: Range,
}

impl Location {
    /// Converts a Location to a file path and range
    pub fn to_path_and_range(&self) -> Result<(PathBuf, Range)> {
        let uri = self.uri.clone();
        let path = uri_to_path(&uri)?;
        let range = self.range;
        Ok((path, range))
    }
}

/// Represents a symbol in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: SymbolKind,
    /// Symbol location
    pub location: Location,
    /// Whether the symbol is deprecated
    pub deprecated: bool,
    /// Symbol container name (if any)
    pub container_name: Option<String>,
    /// Child symbols
    pub children: Vec<Symbol>,
}

impl Symbol {
    /// Returns the path and range of the symbol
    pub fn path_and_range(&self) -> Result<(PathBuf, Range)> {
        self.location.to_path_and_range()
    }

    /// Returns a human-readable description of the symbol
    pub fn description(&self) -> String {
        let kind_str = self.kind.label();
        let container = self.container_name.as_deref().unwrap_or("");

        if container.is_empty() {
            format!("{} {}", kind_str, self.name)
        } else {
            format!("{} {} in {}", kind_str, self.name, container)
        }
    }
}

/// Represents a symbol reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolReference {
    /// Reference location
    pub location: Location,
    /// Is this the definition
    pub is_definition: bool,
}

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

/// Symbol manager that interacts with LSP server
pub struct SymbolManager {
    client: LspClient,
}

impl SymbolManager {
    /// Creates a new symbol manager
    pub fn new(client: LspClient) -> Self {
        Self { client }
    }

    /// Gets document symbols for a file
    pub async fn get_document_symbols(&self, file_path: &Path) -> Result<Vec<Symbol>> {
        debug!("Getting document symbols for {}", file_path.display());

        let uri = path_to_uri(file_path);
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri
            }
        });

        let response = self
            .client
            .send_request("textDocument/documentSymbol", params)
            .await?;
        let symbols: Vec<Symbol> = serde_json::from_value(response)?;

        info!("Found {} symbols in {}", symbols.len(), file_path.display());
        Ok(symbols)
    }

    /// Finds symbols matching a query
    pub async fn find_symbols(&self, query: &str) -> Result<Vec<Symbol>> {
        debug!("Finding symbols matching query: {}", query);

        let params = serde_json::json!({
            "query": query
        });

        let response = self.client.send_request("workspace/symbol", params).await?;
        let symbols: Vec<Symbol> = serde_json::from_value(response)?;

        info!("Found {} symbols matching '{}'", symbols.len(), query);
        Ok(symbols)
    }

    /// Gets the definition for a symbol at a specific position
    pub async fn get_definition(
        &self,
        file_path: &Path,
        position: Position,
    ) -> Result<Vec<Location>> {
        debug!(
            "Getting definition at {}:{}",
            position.line, position.character
        );

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

        let response = self
            .client
            .send_request("textDocument/definition", params)
            .await?;

        // The response can be either a single Location or an array of Locations
        let locations: Vec<Location> = if response.is_array() {
            serde_json::from_value(response)?
        } else {
            let location: Location = serde_json::from_value(response)?;
            vec![location]
        };

        Ok(locations)
    }

    /// Gets all references to a symbol at a specific position
    pub async fn get_references(
        &self,
        file_path: &Path,
        position: Position,
        include_definition: bool,
    ) -> Result<Vec<Location>> {
        debug!(
            "Getting references at {}:{}",
            position.line, position.character
        );

        let uri = path_to_uri(file_path);
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": position.line,
                "character": position.character
            },
            "context": {
                "includeDeclaration": include_definition
            }
        });

        let response = self
            .client
            .send_request("textDocument/references", params)
            .await?;
        let locations: Vec<Location> = serde_json::from_value(response)?;

        info!("Found {} references", locations.len());
        Ok(locations)
    }

    /// Gets the hover information for a symbol at a specific position
    pub async fn get_hover(&self, file_path: &Path, position: Position) -> Result<Option<String>> {
        debug!(
            "Getting hover info at {}:{}",
            position.line, position.character
        );

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

        let response = self
            .client
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
    }

    /// Gets the source code of a symbol
    pub async fn get_symbol_source(&self, symbol: &Symbol) -> Result<String> {
        let (file_path, range) = symbol.path_and_range()?;

        // Read the file content
        let content = std::fs::read_to_string(&file_path)
            .context(format!("Failed to read file: {}", file_path.display()))?;

        // Extract the relevant lines
        let lines: Vec<&str> = content.lines().collect();
        let start_line = range.start.line as usize;
        let end_line = range.end.line as usize;

        if start_line >= lines.len() {
            return Err(anyhow::anyhow!("Invalid start line: {}", start_line));
        }

        let end_line = std::cmp::min(end_line, lines.len() - 1);

        let source_lines = &lines[start_line..=end_line];
        let mut source = source_lines.join("\n");

        // Apply character offsets for the first and last line
        if start_line == end_line {
            let start_char = range.start.character as usize;
            let end_char = range.end.character as usize;

            if !source.is_empty() && start_char < source.len() {
                let end_char = std::cmp::min(end_char, source.len());
                source = source[start_char..end_char].to_string();
            }
        }

        Ok(source)
    }

    /// Applies an edit to a symbol's source code
    pub async fn edit_symbol(&self, symbol: &Symbol, new_source: &str) -> Result<()> {
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
        let response = self
            .client
            .send_request("workspace/applyEdit", edit)
            .await?;

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
    }
}
