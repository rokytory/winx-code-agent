use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info};

use crate::lsp::types::{LSPConfig, Language, Position, Range, Symbol, SymbolLocation};

/// Message to be sent to the language server
#[derive(Debug)]
enum ClientMessage {
    Initialize {
        root_path: PathBuf,
        response_tx: oneshot::Sender<Result<()>>,
    },
    Shutdown {
        response_tx: oneshot::Sender<Result<()>>,
    },
    OpenFile {
        file_path: PathBuf,
        response_tx: oneshot::Sender<Result<()>>,
    },
    CloseFile {
        file_path: PathBuf,
        response_tx: oneshot::Sender<Result<()>>,
    },
    FindSymbol {
        name: String,
        within_path: Option<PathBuf>,
        include_body: bool,
        response_tx: oneshot::Sender<Result<Vec<Symbol>>>,
    },
    FindReferences {
        location: SymbolLocation,
        include_body: bool,
        response_tx: oneshot::Sender<Result<Vec<Symbol>>>,
    },
    InsertText {
        file_path: PathBuf,
        position: Position,
        text: String,
        response_tx: oneshot::Sender<Result<Position>>,
    },
    DeleteText {
        file_path: PathBuf,
        range: Range,
        response_tx: oneshot::Sender<Result<String>>,
    },
    GetDocumentSymbols {
        file_path: PathBuf,
        include_body: bool,
        response_tx: oneshot::Sender<Result<Vec<Symbol>>>,
    },
}

/// Wrapper for an LSP client
pub struct LSPClient {
    config: LSPConfig,
    tx: mpsc::Sender<ClientMessage>,
    server_handle: Arc<Mutex<Option<Child>>>,
}

impl LSPClient {
    /// Create a new LSP client for the given language
    pub async fn new(config: LSPConfig, root_path: impl AsRef<Path>) -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);
        let server_handle = Arc::new(Mutex::new(None));
        let server_handle_clone = server_handle.clone();

        // Start the client task
        tokio::spawn(Self::run_client_loop(
            config.clone(),
            rx,
            server_handle_clone,
        ));

        // Create the client
        let client = Self {
            config,
            tx,
            server_handle,
        };

        // Initialize the language server
        client.initialize(root_path).await?;

        Ok(client)
    }

    /// Initialize the language server
    pub async fn initialize(&self, root_path: impl AsRef<Path>) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(ClientMessage::Initialize {
                root_path: root_path.as_ref().to_path_buf(),
                response_tx,
            })
            .await
            .context("Failed to send initialize message")?;

        response_rx
            .await
            .context("Failed to receive initialize response")??;
        Ok(())
    }

    /// Shutdown the language server
    pub async fn shutdown(&self) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(ClientMessage::Shutdown { response_tx })
            .await
            .context("Failed to send shutdown message")?;

        response_rx
            .await
            .context("Failed to receive shutdown response")??;

        // Also kill the server process
        let mut handle = self.server_handle.lock().unwrap();
        if let Some(mut child) = handle.take() {
            let _ = child.kill();
        }

        Ok(())
    }

    /// Open a file in the language server
    pub async fn open_file(&self, file_path: impl AsRef<Path>) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(ClientMessage::OpenFile {
                file_path: file_path.as_ref().to_path_buf(),
                response_tx,
            })
            .await
            .context("Failed to send open file message")?;

        response_rx
            .await
            .context("Failed to receive open file response")??;
        Ok(())
    }

    /// Close a file in the language server
    pub async fn close_file(&self, file_path: impl AsRef<Path>) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(ClientMessage::CloseFile {
                file_path: file_path.as_ref().to_path_buf(),
                response_tx,
            })
            .await
            .context("Failed to send close file message")?;

        response_rx
            .await
            .context("Failed to receive close file response")??;
        Ok(())
    }

    /// Find a symbol by name in the workspace
    pub async fn find_symbol(
        &self,
        name: &str,
        within_path: Option<impl AsRef<Path>>,
        include_body: bool,
    ) -> Result<Vec<Symbol>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(ClientMessage::FindSymbol {
                name: name.to_string(),
                within_path: within_path.map(|p| p.as_ref().to_path_buf()),
                include_body,
                response_tx,
            })
            .await
            .context("Failed to send find symbol message")?;

        response_rx
            .await
            .context("Failed to receive find symbol response")??;
        Ok(vec![]) // Placeholder until implementation
    }

    /// Find references to a symbol
    pub async fn find_references(
        &self,
        location: SymbolLocation,
        include_body: bool,
    ) -> Result<Vec<Symbol>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(ClientMessage::FindReferences {
                location,
                include_body,
                response_tx,
            })
            .await
            .context("Failed to send find references message")?;

        response_rx
            .await
            .context("Failed to receive find references response")??;
        Ok(vec![]) // Placeholder until implementation
    }

    /// Insert text at a position in a file
    pub async fn insert_text(
        &self,
        file_path: impl AsRef<Path>,
        position: Position,
        text: &str,
    ) -> Result<Position> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(ClientMessage::InsertText {
                file_path: file_path.as_ref().to_path_buf(),
                position,
                text: text.to_string(),
                response_tx,
            })
            .await
            .context("Failed to send insert text message")?;

        response_rx
            .await
            .context("Failed to receive insert text response")??;
        Ok(Position {
            line: 0,
            character: 0,
        }) // Placeholder until implementation
    }

    /// Delete text in a range in a file
    pub async fn delete_text(&self, file_path: impl AsRef<Path>, range: Range) -> Result<String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(ClientMessage::DeleteText {
                file_path: file_path.as_ref().to_path_buf(),
                range,
                response_tx,
            })
            .await
            .context("Failed to send delete text message")?;

        response_rx
            .await
            .context("Failed to receive delete text response")??;
        Ok("".to_string()) // Placeholder until implementation
    }

    /// Get symbols in a document
    pub async fn get_document_symbols(
        &self,
        file_path: impl AsRef<Path>,
        include_body: bool,
    ) -> Result<Vec<Symbol>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(ClientMessage::GetDocumentSymbols {
                file_path: file_path.as_ref().to_path_buf(),
                include_body,
                response_tx,
            })
            .await
            .context("Failed to send get document symbols message")?;

        response_rx
            .await
            .context("Failed to receive get document symbols response")??;
        Ok(vec![]) // Placeholder until implementation
    }

    /// Run the client loop that communicates with the language server
    async fn run_client_loop(
        config: LSPConfig,
        mut rx: mpsc::Receiver<ClientMessage>,
        server_handle: Arc<Mutex<Option<Child>>>,
    ) {
        // TODO: Implement the client loop
        // This should:
        // 1. Start the language server process
        // 2. Handle communication with the server via stdin/stdout
        // 3. Process the received messages and respond to the client

        while let Some(message) = rx.recv().await {
            match message {
                ClientMessage::Initialize {
                    root_path,
                    response_tx,
                } => {
                    // Start the language server process
                    let server_process = Self::start_server_process(&config.language, &root_path);
                    match server_process {
                        Ok(process) => {
                            // Store the server handle
                            let mut handle = server_handle.lock().unwrap();
                            *handle = Some(process);

                            // TODO: Send initialize request to the server

                            let _ = response_tx.send(Ok(()));
                        }
                        Err(e) => {
                            error!("Failed to start language server: {}", e);
                            let _ = response_tx.send(Err(e));
                        }
                    }
                }
                ClientMessage::Shutdown { response_tx } => {
                    // TODO: Send shutdown request to the server

                    let _ = response_tx.send(Ok(()));
                    break;
                }
                // Handle other messages
                _ => {
                    // TODO: Implement other message handlers
                }
            }
        }
    }

    /// Start the language server process for the given language
    fn start_server_process(language: &Language, root_path: &Path) -> Result<Child> {
        // Execute the appropriate command for the language
        let command = match language {
            Language::Rust => Command::new("rust-analyzer")
                .args(["--stdio"])
                .current_dir(root_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context("Failed to start rust-analyzer")?,
            Language::Python => Command::new("pyright-langserver")
                .args(["--stdio"])
                .current_dir(root_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context("Failed to start pyright-langserver")?,
            // TODO: Add support for other languages
            _ => return Err(anyhow::anyhow!("Language not supported: {:?}", language)),
        };

        Ok(command)
    }
}

impl Drop for LSPClient {
    fn drop(&mut self) {
        // Kill the server process if it's still running
        let mut handle = self.server_handle.lock().unwrap();
        if let Some(mut child) = handle.take() {
            let _ = child.kill();
        }
    }
}
