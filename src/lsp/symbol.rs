use std::path::{Path, PathBuf};

use anyhow::Result;
use lsp_types::{DocumentSymbol, Location, Position, Range, SymbolInformation, SymbolKind};
use serde::{Deserialize, Serialize};

use crate::error::WinxError;
use crate::lsp::client::LspClient;
use crate::WinxResult;

/// Representation of a code symbol with location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Name of the symbol
    pub name: String,
    /// Kind of the symbol (class, function, variable, etc.)
    pub kind: SymbolKind,
    /// Location of the symbol (file path, range)
    pub location: SymbolLocation,
    /// Children of the symbol (methods in a class, etc.)
    pub children: Vec<Symbol>,
    /// Symbol body content (if requested)
    pub body: Option<String>,
}

/// Location of a symbol in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolLocation {
    /// Relative path to the file containing the symbol
    pub relative_path: String,
    /// Line number (0-based) where the symbol starts
    pub line: u32,
    /// Column (0-based) where the symbol starts
    pub column: u32,
    /// Range of the symbol (start and end position)
    pub range: SymbolRange,
}

/// Range of a symbol in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRange {
    /// Start position of the symbol
    pub start: SymbolPosition,
    /// End position of the symbol
    pub end: SymbolPosition,
}

/// Position in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPosition {
    /// Line number (0-based)
    pub line: u32,
    /// Column (0-based)
    pub column: u32,
}

impl Symbol {
    /// Convert from LSP DocumentSymbol
    pub fn from_document_symbol(symbol: &DocumentSymbol, relative_path: &str) -> Self {
        let children = symbol
            .children
            .as_ref()
            .map(|children| {
                children
                    .iter()
                    .map(|child| Self::from_document_symbol(child, relative_path))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            name: symbol.name.clone(),
            kind: symbol.kind,
            location: SymbolLocation {
                relative_path: relative_path.to_string(),
                line: symbol.selection_range.start.line,
                column: symbol.selection_range.start.character,
                range: SymbolRange {
                    start: SymbolPosition {
                        line: symbol.range.start.line,
                        column: symbol.range.start.character,
                    },
                    end: SymbolPosition {
                        line: symbol.range.end.line,
                        column: symbol.range.end.character,
                    },
                },
            },
            children,
            body: None,
        }
    }

    /// Convert from LSP SymbolInformation
    pub fn from_symbol_information(symbol: &SymbolInformation, root_path: &Path) -> WinxResult<Self> {
        let file_path = symbol
            .location
            .uri
            .to_file_path()
            .map_err(|()| WinxError::LspError("Invalid file URI".to_string()))?;

        let relative_path = file_path
            .strip_prefix(root_path)
            .map_err(|e| WinxError::LspError(format!("Failed to strip prefix: {}", e)))?
            .to_string_lossy()
            .to_string();

        Ok(Self {
            name: symbol.name.clone(),
            kind: symbol.kind,
            location: SymbolLocation {
                relative_path,
                line: symbol.location.range.start.line,
                column: symbol.location.range.start.character,
                range: SymbolRange {
                    start: SymbolPosition {
                        line: symbol.location.range.start.line,
                        column: symbol.location.range.start.character,
                    },
                    end: SymbolPosition {
                        line: symbol.location.range.end.line,
                        column: symbol.location.range.end.character,
                    },
                },
            },
            children: Vec::new(),
            body: None,
        })
    }

    /// Get the full body of the symbol by reading the file content
    pub fn get_body(&self, root_path: &Path) -> WinxResult<String> {
        let file_path = root_path.join(&self.location.relative_path);
        
        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| WinxError::LspError(format!("Failed to read file: {}", e)))?;
            
        let lines: Vec<&str> = content.lines().collect();
        
        let start_line = self.location.range.start.line as usize;
        let end_line = self.location.range.end.line as usize;
        
        let body = lines[start_line..=end_line].join("\n");
        
        Ok(body)
    }
    
    /// Find all symbols with the given name
    pub fn find_by_name(symbols: &[Symbol], name: &str, substring_match: bool) -> Vec<Symbol> {
        let mut result = Vec::new();
        
        for symbol in symbols {
            if (substring_match && symbol.name.contains(name)) || symbol.name == name {
                result.push(symbol.clone());
            }
            
            let mut children_result = Self::find_by_name(&symbol.children, name, substring_match);
            result.append(&mut children_result);
        }
        
        result
    }
    
    /// Find all symbols with the given kind
    pub fn find_by_kind(symbols: &[Symbol], kind: SymbolKind) -> Vec<Symbol> {
        let mut result = Vec::new();
        
        for symbol in symbols {
            if symbol.kind == kind {
                result.push(symbol.clone());
            }
            
            let mut children_result = Self::find_by_kind(&symbol.children, kind);
            result.append(&mut children_result);
        }
        
        result
    }
    
    /// Convert to a dictionary for JSON serialization
    pub fn to_dict(&self, include_body: bool, include_children: bool, depth: usize) -> serde_json::Value {
        let mut dict = serde_json::json!({
            "name": self.name,
            "kind": self.kind as u32,
            "location": {
                "relativePath": self.location.relative_path,
                "line": self.location.line,
                "column": self.location.column,
                "range": {
                    "start": {
                        "line": self.location.range.start.line,
                        "column": self.location.range.start.column,
                    },
                    "end": {
                        "line": self.location.range.end.line,
                        "column": self.location.range.end.column,
                    },
                },
            },
        });
        
        if include_body && self.body.is_some() {
            dict["body"] = serde_json::Value::String(self.body.clone().unwrap());
        }
        
        if include_children && depth > 0 {
            let children = self.children.iter().map(|child| {
                child.to_dict(include_body, include_children, depth - 1)
            }).collect::<Vec<_>>();
            
            dict["children"] = serde_json::Value::Array(children);
        }
        
        dict
    }
}

/// Symbol manager for working with code symbols
#[derive(Debug)]
pub struct SymbolManager {
    /// LSP client
    lsp_client: LspClient,
    /// Root path of the project
    root_path: PathBuf,
}

impl SymbolManager {
    /// Create a new symbol manager
    pub fn new(lsp_client: LspClient, root_path: impl AsRef<Path>) -> Self {
        Self {
            lsp_client,
            root_path: root_path.as_ref().to_path_buf(),
        }
    }
    
    /// Get symbols for a file
    pub async fn get_document_symbols(&self, relative_path: impl AsRef<Path>) -> WinxResult<Vec<Symbol>> {
        let file_path = self.root_path.join(relative_path.as_ref());
        
        let response = self.lsp_client.get_document_symbols(&file_path).await?;
        
        let symbols = match response {
            lsp_types::DocumentSymbolResponse::Nested(document_symbols) => {
                document_symbols
                    .iter()
                    .map(|symbol| {
                        Symbol::from_document_symbol(
                            symbol,
                            relative_path.as_ref().to_string_lossy().as_ref(),
                        )
                    })
                    .collect()
            }
            lsp_types::DocumentSymbolResponse::Flat(symbol_infos) => {
                symbol_infos
                    .iter()
                    .filter_map(|symbol| {
                        Symbol::from_symbol_information(symbol, &self.root_path).ok()
                    })
                    .collect()
            }
        };
        
        Ok(symbols)
    }
    
    /// Find references to a symbol
    pub async fn find_references(&self, symbol: &Symbol) -> WinxResult<Vec<SymbolLocation>> {
        let file_path = self.root_path.join(&symbol.location.relative_path);
        
        let locations = self.lsp_client
            .find_references(&file_path, symbol.location.line, symbol.location.column)
            .await?;
            
        let references = locations
            .into_iter()
            .filter_map(|location| {
                let file_path = match location.uri.to_file_path() {
                    Ok(path) => path,
                    Err(_) => return None,
                };
                
                let relative_path = match file_path.strip_prefix(&self.root_path) {
                    Ok(path) => path.to_string_lossy().to_string(),
                    Err(_) => return None,
                };
                
                Some(SymbolLocation {
                    relative_path,
                    line: location.range.start.line,
                    column: location.range.start.character,
                    range: SymbolRange {
                        start: SymbolPosition {
                            line: location.range.start.line,
                            column: location.range.start.character,
                        },
                        end: SymbolPosition {
                            line: location.range.end.line,
                            column: location.range.end.character,
                        },
                    },
                })
            })
            .collect();
            
        Ok(references)
    }
    
    /// Find a symbol at a specific location
    pub async fn find_symbol_at_location(&self, relative_path: impl AsRef<Path>, line: u32, column: u32) -> WinxResult<Option<Symbol>> {
        let symbols = self.get_document_symbols(relative_path.as_ref()).await?;
        
        // Find the symbol at the given location
        for symbol in symbols {
            if self.is_position_within_symbol(&symbol, line, column) {
                return Ok(Some(symbol));
            }
            
            // Check children recursively
            if let Some(found) = self.find_symbol_in_children(&symbol, line, column) {
                return Ok(Some(found));
            }
        }
        
        Ok(None)
    }
    
    /// Check if a position is within a symbol
    fn is_position_within_symbol(&self, symbol: &Symbol, line: u32, column: u32) -> bool {
        let start_line = symbol.location.range.start.line;
        let start_column = symbol.location.range.start.column;
        let end_line = symbol.location.range.end.line;
        let end_column = symbol.location.range.end.column;
        
        (line > start_line || (line == start_line && column >= start_column)) &&
        (line < end_line || (line == end_line && column <= end_column))
    }
    
    /// Find a symbol in the children of another symbol
    fn find_symbol_in_children(&self, parent: &Symbol, line: u32, column: u32) -> Option<Symbol> {
        for child in &parent.children {
            if self.is_position_within_symbol(child, line, column) {
                return Some(child.clone());
            }
            
            // Check children recursively
            if let Some(found) = self.find_symbol_in_children(child, line, column) {
                return Some(found);
            }
        }
        
        None
    }
}
