use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::lsp::client::LSPClient;
use crate::lsp::server::LSPServer;
use crate::lsp::types::{Position, Range, Symbol, SymbolKind, SymbolLocation};

/// Manager for symbol-based code operations
pub struct SymbolManager {
    /// LSP server for language-specific operations
    lsp_server: Arc<Mutex<LSPServer>>,
    
    /// Root path of the project
    root_path: PathBuf,
}

impl SymbolManager {
    /// Create a new symbol manager
    pub fn new(lsp_server: Arc<Mutex<LSPServer>>, root_path: impl AsRef<Path>) -> Self {
        Self {
            lsp_server,
            root_path: root_path.as_ref().to_path_buf(),
        }
    }
    
    /// Find symbols by name in the workspace
    pub async fn find_by_name(
        &self,
        name: &str,
        within_path: Option<&Path>,
        include_body: bool,
        substring_matching: bool,
        include_kinds: Option<Vec<SymbolKind>>,
        exclude_kinds: Option<Vec<SymbolKind>>,
    ) -> Result<Vec<Symbol>> {
        info!("Finding symbols with name '{}'{}", name, 
              if substring_matching { " (substring matching)" } else { "" });
        
        let server = self.lsp_server.lock().await;
        let within_path_buf = within_path.map(PathBuf::from);
        
        let symbols = server.find_symbol(name, within_path_buf, include_body).await?;
        
        // Filter symbols based on criteria
        let filtered_symbols = symbols.into_iter().filter(|s| {
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
        }).collect();
        
        Ok(filtered_symbols)
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
        if location.relative_path.is_none() || location.line.is_none() || location.column.is_none() {
            return Err(anyhow::anyhow!("Invalid symbol location - missing path, line, or column"));
        }
        
        let relative_path = location.relative_path.as_ref().unwrap();
        let line = location.line.unwrap();
        let column = location.column.unwrap();
        
        info!("Finding references to symbol at {}:{}:{}", relative_path, line, column);
        
        let server = self.lsp_server.lock().await;
        let symbols = server.find_references(location.clone(), include_body).await?;
        
        // Filter by kinds if specified
        let filtered_symbols = if include_kinds.is_some() || exclude_kinds.is_some() {
            symbols.into_iter().filter(|s| {
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
            }).collect()
        } else {
            symbols
        };
        
        Ok(filtered_symbols)
    }
    
    /// Replace the body of a symbol
    pub async fn replace_body(&self, location: &SymbolLocation, new_body: &str) -> Result<()> {
        info!("Replacing symbol body");
        
        // Ensure we have a valid location
        if location.relative_path.is_none() || location.line.is_none() || location.column.is_none() {
            return Err(anyhow::anyhow!("Invalid symbol location - missing path, line, or column"));
        }
        
        let server = self.lsp_server.lock().await;
        server.replace_symbol_body(location.clone(), new_body).await?;
        
        Ok(())
    }
    
    /// Insert text after a symbol
    pub async fn insert_after(&self, location: &SymbolLocation, content: &str) -> Result<()> {
        info!("Inserting text after symbol");
        
        // Ensure we have a valid location
        if location.relative_path.is_none() || location.line.is_none() || location.column.is_none() {
            return Err(anyhow::anyhow!("Invalid symbol location - missing path, line, or column"));
        }
        
        let server = self.lsp_server.lock().await;
        server.insert_after_symbol(location.clone(), content).await?;
        
        Ok(())
    }
    
    /// Insert text before a symbol
    pub async fn insert_before(&self, location: &SymbolLocation, content: &str) -> Result<()> {
        info!("Inserting text before symbol");
        
        // Ensure we have a valid location
        if location.relative_path.is_none() || location.line.is_none() || location.column.is_none() {
            return Err(anyhow::anyhow!("Invalid symbol location - missing path, line, or column"));
        }
        
        let server = self.lsp_server.lock().await;
        server.insert_before_symbol(location.clone(), content).await?;
        
        Ok(())
    }
    
    /// Insert text at a specific line in a file
    pub async fn insert_at_line(&self, relative_path: &str, line: usize, content: &str) -> Result<()> {
        info!("Inserting text at line {} in file {}", line, relative_path);
        
        let server = self.lsp_server.lock().await;
        server.insert_text_at_position(relative_path, line, 0, content).await?;
        
        Ok(())
    }
    
    /// Delete lines in a file
    pub async fn delete_lines(&self, relative_path: &str, start_line: usize, end_line: usize) -> Result<()> {
        info!("Deleting lines {} to {} in file {}", start_line, end_line, relative_path);
        
        let server = self.lsp_server.lock().await;
        server.delete_text_between_positions(
            relative_path, 
            start_line, 0, 
            end_line + 1, 0
        ).await?;
        
        Ok(())
    }
    
    /// Get document symbols for a file
    pub async fn get_document_symbols(&self, relative_path: &str, include_body: bool) -> Result<Vec<Symbol>> {
        info!("Getting symbols for document {}", relative_path);
        
        let server = self.lsp_server.lock().await;
        let symbols = server.get_document_symbols(relative_path, include_body).await?;
        
        Ok(symbols)
    }
    
    /// Find a symbol by its location
    pub async fn find_by_location(&self, location: &SymbolLocation) -> Result<Option<Symbol>> {
        if location.relative_path.is_none() {
            return Ok(None);
        }
        
        let relative_path = location.relative_path.as_ref().unwrap();
        let symbols = self.get_document_symbols(relative_path, false).await?;
        
        // Find the symbol that matches the location
        for symbol in symbols {
            if symbol.location.line == location.line && symbol.location.column == location.column {
                return Ok(Some(symbol));
            }
        }
        
        Ok(None)
    }
}
