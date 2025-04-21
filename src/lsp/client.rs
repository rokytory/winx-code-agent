use anyhow::Result;
use async_trait::async_trait;
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, InitializeParams, Location, Position,
    ReferenceParams, SymbolInformation, TextDocumentIdentifier, TextDocumentPositionParams, Url,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;

use crate::error::WinxError;
use crate::WinxResult;

/// Supported language server types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageServerType {
    /// Rust analyzer for Rust code
    RustAnalyzer,
    /// Pyright for Python code
    Pyright,
    /// TypeScript language server
    TypeScript,
    /// Clangd for C/C++ code
    Clangd,
    /// Java language server
    Java,
    /// Go language server
    Gopls,
    /// Solargraph for Ruby code
    Ruby,
    /// Omnisharp for C# code
    CSharp,
}

impl LanguageServerType {
    /// Get the command to start the language server
    pub fn get_command(&self) -> (&str, Vec<String>) {
        match self {
            Self::RustAnalyzer => ("rust-analyzer", vec![]),
            Self::Pyright => ("pyright-langserver", vec!["--stdio".to_string()]),
            Self::TypeScript => (
                "typescript-language-server",
                vec!["--stdio".to_string()],
            ),
            Self::Clangd => ("clangd", vec![]),
            Self::Java => ("jdtls", vec![]),
            Self::Gopls => ("gopls", vec!["serve".to_string()]),
            Self::Ruby => ("solargraph", vec!["stdio".to_string()]),
            Self::CSharp => ("omnisharp", vec!["--languageserver".to_string()]),
        }
    }

    /// Detect the language server type from a file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Self::RustAnalyzer),
            "py" => Some(Self::Pyright),
            "ts" | "js" | "tsx" | "jsx" => Some(Self::TypeScript),
            "c" | "cpp" | "h" | "hpp" => Some(Self::Clangd),
            "java" => Some(Self::Java),
            "go" => Some(Self::Gopls),
            "rb" => Some(Self::Ruby),
            "cs" => Some(Self::CSharp),
            _ => None,
        }
    }
}

/// LSP Client for semantic code understanding
#[derive(Debug)]
pub struct LspClient {
    /// Connection to the language server
    connection: Arc<TokioMutex<Connection>>,
    /// Server process
    process: Arc<Mutex<Child>>,
    /// Root path of the project
    root_path: PathBuf,
    /// Request ID counter
    id_counter: Arc<Mutex<u64>>,
    /// Language server type
    server_type: LanguageServerType,
}

impl LspClient {
    /// Create a new LSP client by starting a language server process
    pub async fn new(server_type: LanguageServerType, root_path: impl AsRef<Path>) -> WinxResult<Self> {
        let root_path = root_path.as_ref().to_path_buf();
        
        // Get the command to start the language server
        let (command, args) = server_type.get_command();
        
        // Start the language server process
        let process = Command::new(command)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(&root_path)
            .spawn()
            .map_err(|e| WinxError::LspError(format!("Failed to start language server: {}", e)))?;
            
        // Create the LSP connection
        let (connection, stdin, stdout) = match Connection::stdio_transport(process.stdin.take().unwrap(), process.stdout.take().unwrap()) {
            Ok(conn) => conn,
            Err(e) => return Err(WinxError::LspError(format!("Failed to create LSP connection: {}", e))),
        };
        
        // Initialize the LSP connection
        let client = Self {
            connection: Arc::new(TokioMutex::new(connection)),
            process: Arc::new(Mutex::new(process)),
            root_path,
            id_counter: Arc::new(Mutex::new(0)),
            server_type,
        };
        
        // Initialize the language server
        client.initialize().await?;
        
        Ok(client)
    }
    
    /// Get the next request ID
    fn next_id(&self) -> u64 {
        let mut counter = self.id_counter.lock().unwrap();
        *counter += 1;
        *counter
    }
    
    /// Initialize the language server
    async fn initialize(&self) -> WinxResult<()> {
        let root_uri = Url::from_file_path(&self.root_path)
            .map_err(|()| WinxError::LspError(format!("Invalid root path: {:?}", self.root_path)))?;
            
        // Create initialize parameters
        let initialize_params = InitializeParams {
            root_uri: Some(root_uri),
            ..Default::default()
        };
        
        // Send initialize request
        self.send_request::<_, Value>("initialize", serde_json::to_value(initialize_params)?)
            .await?;
            
        // Send initialized notification
        self.send_notification("initialized", serde_json::to_value(())?)
            .await?;
            
        Ok(())
    }
    
    /// Send a request to the language server
    async fn send_request<R, T>(&self, method: &str, params: T) -> WinxResult<R>
    where
        R: serde::de::DeserializeOwned,
        T: serde::Serialize,
    {
        let id = self.next_id();
        let params = serde_json::to_value(params)
            .map_err(|e| WinxError::LspError(format!("Failed to serialize request parameters: {}", e)))?;
            
        // Create request
        let request = Request {
            id: id.into(),
            method: method.to_string(),
            params,
        };
        
        // Send request
        {
            let mut conn = self.connection.lock().await;
            conn.sender.send(Message::Request(request))
                .map_err(|e| WinxError::LspError(format!("Failed to send request: {}", e)))?;
        }
        
        // Receive response
        loop {
            let msg = {
                let mut conn = self.connection.lock().await;
                conn.receiver.recv()
                    .map_err(|e| WinxError::LspError(format!("Failed to receive response: {}", e)))?
            };
            
            match msg {
                Message::Response(Response { id: res_id, result, error }) => {
                    if res_id == id.into() {
                        if let Some(error) = error {
                            return Err(WinxError::LspError(format!("LSP error: {:?}", error)));
                        }
                        
                        let result = result.ok_or_else(|| WinxError::LspError("No result in response".to_string()))?;
                        
                        return serde_json::from_value(result)
                            .map_err(|e| WinxError::LspError(format!("Failed to deserialize response: {}", e)));
                    }
                }
                Message::Notification(_) => {
                    // Ignore notifications
                }
                _ => {}
            }
        }
    }
    
    /// Send a notification to the language server
    async fn send_notification<T>(&self, method: &str, params: T) -> WinxResult<()>
    where
        T: serde::Serialize,
    {
        let params = serde_json::to_value(params)
            .map_err(|e| WinxError::LspError(format!("Failed to serialize notification parameters: {}", e)))?;
            
        // Create notification
        let notification = Notification {
            method: method.to_string(),
            params,
        };
        
        // Send notification
        let mut conn = self.connection.lock().await;
        conn.sender.send(Message::Notification(notification))
            .map_err(|e| WinxError::LspError(format!("Failed to send notification: {}", e)))?;
            
        Ok(())
    }
    
    /// Get document symbols for a file
    pub async fn get_document_symbols(&self, file_path: impl AsRef<Path>) -> WinxResult<DocumentSymbolResponse> {
        let file_path = file_path.as_ref();
        let uri = Url::from_file_path(file_path)
            .map_err(|()| WinxError::LspError(format!("Invalid file path: {:?}", file_path)))?;
            
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
        };
        
        self.send_request("textDocument/documentSymbol", params).await
    }
    
    /// Find references to a symbol at the given position
    pub async fn find_references(&self, file_path: impl AsRef<Path>, line: u32, character: u32) -> WinxResult<Vec<Location>> {
        let file_path = file_path.as_ref();
        let uri = Url::from_file_path(file_path)
            .map_err(|()| WinxError::LspError(format!("Invalid file path: {:?}", file_path)))?;
            
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };
        
        self.send_request("textDocument/references", params).await
    }
    
    /// Shutdown the language server
    pub async fn shutdown(&self) -> WinxResult<()> {
        // Send shutdown request
        self.send_request::<Value, Value>("shutdown", Value::Null).await?;
        
        // Send exit notification
        self.send_notification("exit", Value::Null).await?;
        
        // Terminate the process
        let mut process = self.process.lock().unwrap();
        process.kill()
            .map_err(|e| WinxError::LspError(format!("Failed to kill language server process: {}", e)))?;
            
        Ok(())
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Try to kill the process when the client is dropped
        if let Ok(mut process) = self.process.lock() {
            let _ = process.kill();
        }
    }
}

/// LSP client factory for creating clients for different languages
#[derive(Debug, Default)]
pub struct LspClientFactory;

impl LspClientFactory {
    /// Create a new LSP client factory
    pub fn new() -> Self {
        Self
    }
    
    /// Create a new LSP client for the given file
    pub async fn create_client_for_file(&self, file_path: impl AsRef<Path>, root_path: impl AsRef<Path>) -> WinxResult<LspClient> {
        let file_path = file_path.as_ref();
        let ext = file_path.extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| WinxError::LspError(format!("Failed to determine file extension: {:?}", file_path)))?;
            
        let server_type = LanguageServerType::from_extension(ext)
            .ok_or_else(|| WinxError::LspError(format!("Unsupported file extension: {}", ext)))?;
            
        LspClient::new(server_type, root_path).await
    }
}
