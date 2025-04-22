use rmcp::model::ErrorCode;
use rmcp::Error as McpError;
use serde_json::json;
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

/// Main error type for the winx-code-agent
#[derive(Error, Debug)]
pub enum WinxError {
    #[error("IO error: {source}")]
    Io {
        source: std::io::Error,
        path: Option<PathBuf>,
    },

    #[error("Failed to execute bash command: {message}")]
    BashExecution { message: String },

    #[error("Shell not started or not available")]
    ShellNotStarted,

    #[error("Failed to acquire lock: {message}")]
    LockError { message: String },

    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },

    #[error("File operation failed: {message}")]
    FileOperation { message: String, path: PathBuf },

    #[error("Invalid argument: {message}")]
    InvalidArgument { message: String },

    #[error("Invalid file path: {path}")]
    InvalidPath { path: String },

    #[error("File too large: {path}")]
    FileTooLarge { path: PathBuf, size: u64 },

    #[error("Syntax error: {message}")]
    SyntaxError { message: String },

    #[error("Parse error: {message}")]
    ParseError { message: String },

    #[error("LSP error: {0}")]
    LspError(String),

    #[error("Symbol error: {message}")]
    SymbolError { message: String },

    #[error("Initialization required: {message}")]
    InitializationRequired { message: String },

    #[error("{0}")]
    Other(String),

    #[error("IO error: {0}")]
    IoError(String),
}

impl WinxError {
    /// Create a new IO error with path context
    pub fn io_error(err: std::io::Error, path: Option<impl Into<PathBuf>>) -> Self {
        Self::Io {
            source: err,
            path: path.map(|p| p.into()),
        }
    }

    /// Create a new bash execution error
    pub fn bash_error(message: impl Into<String>) -> Self {
        Self::BashExecution {
            message: message.into(),
        }
    }

    /// Create a new lock error
    pub fn lock_error(message: impl Into<String>) -> Self {
        Self::LockError {
            message: message.into(),
        }
    }

    /// Create a new permission denied error
    pub fn permission_error(message: impl Into<String>) -> Self {
        Self::PermissionDenied {
            message: message.into(),
        }
    }

    /// Create a new file operation error
    pub fn file_error(message: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self::FileOperation {
            message: message.into(),
            path: path.into(),
        }
    }

    /// Create a new invalid argument error
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::InvalidArgument {
            message: message.into(),
        }
    }

    /// Create a new invalid path error
    pub fn invalid_path(path: impl Into<String>) -> Self {
        Self::InvalidPath { path: path.into() }
    }

    /// Create a new file too large error
    pub fn file_too_large(path: impl Into<PathBuf>, size: u64) -> Self {
        Self::FileTooLarge {
            path: path.into(),
            size,
        }
    }

    /// Create a new syntax error
    pub fn syntax_error(message: impl Into<String>) -> Self {
        Self::SyntaxError {
            message: message.into(),
        }
    }

    /// Create a new parse error
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }

    /// Create a new LSP error
    pub fn lsp_error(message: impl Into<String>) -> Self {
        Self::LspError(message.into())
    }

    /// Create a new symbol error
    pub fn symbol_error(message: impl Into<String>) -> Self {
        Self::SymbolError {
            message: message.into(),
        }
    }

    /// Create a new initialization required error
    pub fn initialization_required(message: impl Into<String>) -> Self {
        Self::InitializationRequired {
            message: message.into(),
        }
    }

    /// Create a new generic error
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }

    /// Convert to MCP error for tool responses
    pub fn to_mcp_error(&self) -> McpError {
        match self {
            WinxError::Io { source, path } => {
                let message = if let Some(path) = path {
                    format!("IO error: {} (path: {})", source, path.display())
                } else {
                    format!("IO error: {}", source)
                };

                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    message,
                    Some(json!({
                        "error_type": "io_error",
                        "path": path.clone().map(|p| p.to_string_lossy().to_string()),
                        "details": source.to_string()
                    })),
                )
            }
            WinxError::BashExecution { message } => McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Command execution failed: {}", message),
                Some(json!({
                    "error_type": "bash_execution_error",
                    "details": message
                })),
            ),
            WinxError::ShellNotStarted => McpError::new(
                ErrorCode::INTERNAL_ERROR,
                "Shell not started or not available".to_string(),
                Some(json!({
                    "error_type": "shell_not_started",
                    "details": "Initialize the shell before executing commands"
                })),
            ),
            WinxError::LockError { message } => McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to acquire lock: {}", message),
                Some(json!({
                    "error_type": "lock_error",
                    "details": message
                })),
            ),
            WinxError::PermissionDenied { message } => McpError::new(
                ErrorCode::INVALID_REQUEST,
                format!("Permission denied: {}", message),
                Some(json!({
                    "error_type": "permission_denied",
                    "details": message
                })),
            ),
            WinxError::FileOperation { message, path } => McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!(
                    "File operation failed: {} (path: {})",
                    message,
                    path.display()
                ),
                Some(json!({
                    "error_type": "file_operation_error",
                    "path": path.to_string_lossy(),
                    "details": message
                })),
            ),
            WinxError::InvalidArgument { message } => McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Invalid argument: {}", message),
                Some(json!({
                    "error_type": "invalid_argument",
                    "details": message
                })),
            ),
            WinxError::InvalidPath { path } => McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Invalid file path: {}", path),
                Some(json!({
                    "error_type": "invalid_path",
                    "path": path
                })),
            ),
            WinxError::FileTooLarge { path, size } => McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("File too large: {} ({} bytes)", path.display(), size),
                Some(json!({
                    "error_type": "file_too_large",
                    "path": path.to_string_lossy(),
                    "size": size,
                    "size_mb": (*size as f64) / 1_000_000.0
                })),
            ),
            WinxError::SyntaxError { message } => McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Syntax error: {}", message),
                Some(json!({
                    "error_type": "syntax_error",
                    "details": message
                })),
            ),
            WinxError::ParseError { message } => McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Parse error: {}", message),
                Some(json!({
                    "error_type": "parse_error",
                    "details": message
                })),
            ),
            WinxError::LspError(message) => McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("LSP error: {}", message),
                Some(json!({
                    "error_type": "lsp_error",
                    "details": message
                })),
            ),
            WinxError::SymbolError { message } => McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Symbol error: {}", message),
                Some(json!({
                    "error_type": "symbol_error",
                    "details": message
                })),
            ),
            WinxError::InitializationRequired { message } => McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Initialization required: {}", message),
                Some(json!({
                    "error_type": "initialization_required",
                    "details": message,
                    "solution": "Call 'initialize' tool first before using other tools"
                })),
            ),
            WinxError::Other(message) => McpError::new(
                ErrorCode::INTERNAL_ERROR,
                message.clone(),
                Some(json!({
                    "error_type": "other_error",
                    "details": message
                })),
            ),
            WinxError::IoError(message) => McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("IO error: {}", message),
                Some(json!({
                    "error_type": "io_error",
                    "details": message
                })),
            ),
        }
    }
}

// Implement From for std::io::Error
impl From<std::io::Error> for WinxError {
    fn from(error: std::io::Error) -> Self {
        WinxError::io_error(error, None::<PathBuf>)
    }
}

// Implement From for serde_json::Error
impl From<serde_json::Error> for WinxError {
    fn from(error: serde_json::Error) -> Self {
        WinxError::parse_error(error.to_string())
    }
}

// Implement From for serde_yaml::Error
impl From<serde_yaml::Error> for WinxError {
    fn from(error: serde_yaml::Error) -> Self {
        WinxError::parse_error(error.to_string())
    }
}

// Implement From for toml::de::Error
impl From<toml::de::Error> for WinxError {
    fn from(error: toml::de::Error) -> Self {
        WinxError::parse_error(error.to_string())
    }
}

// Implement From for toml::ser::Error
impl From<toml::ser::Error> for WinxError {
    fn from(error: toml::ser::Error) -> Self {
        WinxError::parse_error(error.to_string())
    }
}

/// Result type alias using WinxError
pub type WinxResult<T> = Result<T, WinxError>;

/// Extension trait for converting errors to WinxError
pub trait ErrorExt<T> {
    /// Convert to WinxResult with added context
    fn with_context(self, message: impl AsRef<str>) -> WinxResult<T>;

    /// Convert to WinxResult with file path context
    fn with_path(self, path: impl Into<PathBuf>) -> WinxResult<T>;
}

impl<T, E: fmt::Display> ErrorExt<T> for Result<T, E> {
    fn with_context(self, message: impl AsRef<str>) -> WinxResult<T> {
        self.map_err(|e| WinxError::other(format!("{}: {}", message.as_ref(), e)))
    }

    fn with_path(self, path: impl Into<PathBuf>) -> WinxResult<T> {
        let path = path.into();
        self.map_err(|e| {
            // Try to detect if it's an IO error from the error message
            if e.to_string().contains("permission denied") || e.to_string().contains("not found") {
                // Create a new IO error from the message
                let io_err = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
                WinxError::io_error(io_err, Some(path))
            } else {
                WinxError::file_error(e.to_string(), path)
            }
        })
    }
}

/// Contextual error mapping function
pub fn map_io_err<P: Into<PathBuf>>(path: P) -> impl FnOnce(std::io::Error) -> WinxError {
    let path = path.into();
    move |err| WinxError::io_error(err, Some(path))
}

/// Map any error to a WinxError
pub fn map_error<T>(result: Result<T, impl std::fmt::Display>) -> Result<T, WinxError> {
    result.map_err(|e| WinxError::other(e.to_string()))
}

/// Try an operation and map any error to a WinxError
pub fn try_operation<T, E: std::fmt::Display + std::error::Error>(
    operation: impl FnOnce() -> Result<T, E>,
) -> Result<T, WinxError> {
    operation().map_err(|e| WinxError::other(e.to_string()))
}

/// Add context to an error message
pub fn with_context<T, E: std::fmt::Display + std::error::Error>(
    result: Result<T, E>,
    context: impl AsRef<str>,
) -> Result<T, WinxError> {
    result.map_err(|e| WinxError::other(format!("{}: {}", context.as_ref(), e)))
}

/// Handle file operations with proper context
pub fn with_file_context<T>(
    operation: impl FnOnce() -> Result<T, std::io::Error>,
    path: impl AsRef<std::path::Path>,
) -> Result<T, WinxError> {
    operation().map_err(|e| WinxError::io_error(e, Some(path.as_ref())))
}
