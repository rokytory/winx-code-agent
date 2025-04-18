use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};

use crate::lsp::client::LSPClient;
use crate::lsp::types::{LSPConfig, Language, Symbol, SymbolLocation};

/// Wrapper for LSP servers
pub struct LSPServer {
    /// Root path of the project
    root_path: PathBuf,

    /// LSP configuration
    config: LSPConfig,

    /// LSP client for communication
    client: Arc<Mutex<Option<LSPClient>>>,

    /// Whether the server is running
    is_running: bool,
}

impl LSPServer {
    /// Create a new LSP server for the given language
    pub fn new(
        root_path: impl AsRef<Path>,
        language: Language,
        ignored_paths: Vec<String>,
        gitignore_content: Option<String>,
    ) -> Self {
        let config = LSPConfig {
            language,
            ignored_paths,
            gitignore_content,
            trace_lsp_communication: false,
        };

        Self {
            root_path: root_path.as_ref().to_path_buf(),
            config,
            client: Arc::new(Mutex::new(None)),
            is_running: false,
        }
    }

    /// Start the LSP server
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running {
            debug!("LSP server already running");
            return Ok(());
        }

        info!(
            "Starting LSP server for language: {:?}",
            self.config.language
        );

        // Create a new LSP client with improved error handling
        match LSPClient::new(self.config.clone(), &self.root_path).await {
            Ok(client) => {
                // Store the client
                let mut client_guard = self.client.lock().unwrap();
                *client_guard = Some(client);
                drop(client_guard);

                self.is_running = true;
                info!("LSP server started successfully");

                Ok(())
            }
            Err(e) => {
                // Enhanced error reporting
                error!("Failed to create LSP client: {}", e);

                // Provide more context about the error for debugging
                if let Some(source) = e.source() {
                    error!("Caused by: {}", source);

                    // Extract more error details if available
                    if let Some(deeper_source) = source.source() {
                        error!("Root cause: {}", deeper_source);
                    }
                }

                // Return the error with context
                Err(anyhow::anyhow!("Failed to create LSP client: {}", e))
            }
        }
    }

    /// Stop the LSP server
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            debug!("LSP server not running");
            return Ok(());
        }

        info!("Stopping LSP server");

        // Shutdown the client
        let mut client_guard = self.client.lock().unwrap();
        if let Some(client) = client_guard.take() {
            client.shutdown().await?;
        }

        self.is_running = false;
        info!("LSP server stopped successfully");

        Ok(())
    }

    /// Check if the server is running
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Find a symbol by name in the workspace
    pub async fn find_symbol(
        &self,
        name: &str,
        within_path: Option<PathBuf>,
        include_body: bool,
    ) -> Result<Vec<Symbol>> {
        if !self.is_running {
            return Err(anyhow::anyhow!("LSP server not running"));
        }

        let client_guard = self.client.lock().unwrap();
        if let Some(client) = &*client_guard {
            client.find_symbol(name, within_path, include_body).await
        } else {
            Err(anyhow::anyhow!("LSP client not initialized"))
        }
    }

    /// Find references to a symbol
    pub async fn find_references(
        &self,
        location: SymbolLocation,
        include_body: bool,
    ) -> Result<Vec<Symbol>> {
        if !self.is_running {
            return Err(anyhow::anyhow!("LSP server not running"));
        }

        let client_guard = self.client.lock().unwrap();
        if let Some(client) = &*client_guard {
            client.find_references(location, include_body).await
        } else {
            Err(anyhow::anyhow!("LSP client not initialized"))
        }
    }

    /// Get symbols in a document
    pub async fn get_document_symbols(
        &self,
        relative_path: &str,
        include_body: bool,
    ) -> Result<Vec<Symbol>> {
        if !self.is_running {
            return Err(anyhow::anyhow!("LSP server not running"));
        }

        let file_path = self.root_path.join(relative_path);

        let client_guard = self.client.lock().unwrap();
        if let Some(client) = &*client_guard {
            client.get_document_symbols(file_path, include_body).await
        } else {
            Err(anyhow::anyhow!("LSP client not initialized"))
        }
    }

    /// Insert text at a position in a file
    pub async fn insert_text_at_position(
        &self,
        relative_path: &str,
        line: usize,
        column: usize,
        text: &str,
    ) -> Result<()> {
        if !self.is_running {
            return Err(anyhow::anyhow!("LSP server not running"));
        }

        let file_path = self.root_path.join(relative_path);
        let position = crate::lsp::types::Position {
            line,
            character: column,
        };

        let client_guard = self.client.lock().unwrap();
        if let Some(client) = &*client_guard {
            client.insert_text(file_path, position, text).await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("LSP client not initialized"))
        }
    }

    /// Delete text between positions in a file
    pub async fn delete_text_between_positions(
        &self,
        relative_path: &str,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Result<String> {
        if !self.is_running {
            return Err(anyhow::anyhow!("LSP server not running"));
        }

        let file_path = self.root_path.join(relative_path);
        let range = crate::lsp::types::Range {
            start: crate::lsp::types::Position {
                line: start_line,
                character: start_column,
            },
            end: crate::lsp::types::Position {
                line: end_line,
                character: end_column,
            },
        };

        let client_guard = self.client.lock().unwrap();
        if let Some(client) = &*client_guard {
            client.delete_text(file_path, range).await
        } else {
            Err(anyhow::anyhow!("LSP client not initialized"))
        }
    }

    /// Replace a symbol's body
    pub async fn replace_symbol_body(
        &self,
        _location: SymbolLocation,
        _new_body: &str,
    ) -> Result<()> {
        if !self.is_running {
            return Err(anyhow::anyhow!("LSP server not running"));
        }

        // To implement this, we need to:
        // 1. Find the symbol by location
        // 2. Get its body range
        // 3. Delete the old body
        // 4. Insert the new body

        // This is a placeholder implementation
        info!("Replace symbol body operation not yet implemented");
        Ok(())
    }

    /// Insert content after a symbol
    pub async fn insert_after_symbol(
        &self,
        _location: SymbolLocation,
        _content: &str,
    ) -> Result<()> {
        if !self.is_running {
            return Err(anyhow::anyhow!("LSP server not running"));
        }

        // To implement this, we need to:
        // 1. Find the symbol by location
        // 2. Get its end position
        // 3. Insert the content at that position

        // This is a placeholder implementation
        info!("Insert after symbol operation not yet implemented");
        Ok(())
    }

    /// Insert content before a symbol
    pub async fn insert_before_symbol(
        &self,
        _location: SymbolLocation,
        _content: &str,
    ) -> Result<()> {
        if !self.is_running {
            return Err(anyhow::anyhow!("LSP server not running"));
        }

        // To implement this, we need to:
        // 1. Find the symbol by location
        // 2. Get its start position
        // 3. Insert the content at that position

        // This is a placeholder implementation
        info!("Insert before symbol operation not yet implemented");
        Ok(())
    }
}
