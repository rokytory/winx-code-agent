pub mod bash;
pub mod error;
pub mod file;
// Comentando temporariamente o módulo LSP que está causando erros
// pub mod lsp;
pub mod reinforcement;
pub mod server;
pub mod tools;

// Reexport error types and utilities
pub use error::{ErrorExt, WinxError, WinxResult};
pub use server::CodeAgent;

// Export logging macros
#[macro_export]
macro_rules! log_error {
    ($err:expr, $context:expr) => {
        log::error!("{}: {} (at {}:{})", $context, $err, file!(), line!())
    };
    ($err:expr) => {
        log::error!("{} (at {}:{})", $err, file!(), line!())
    };
}

// Macro to check if initialization has been performed
#[macro_export]
macro_rules! ensure_initialized {
    () => {
        if !crate::tools::initialize::Initialize::was_initialized() {
            return Err(crate::error::WinxError::initialization_required(
                "You must call 'initialize' before using this tool.",
            )
            .to_mcp_error());
        }
    };
    ($message:expr) => {
        if !crate::tools::initialize::Initialize::was_initialized() {
            return Err(crate::error::WinxError::initialization_required($message).to_mcp_error());
        }
    };
}

#[macro_export]
macro_rules! log_warn {
    ($msg:expr, $context:expr) => {
        log::warn!("{}: {} (at {}:{})", $context, $msg, file!(), line!())
    };
    ($msg:expr) => {
        log::warn!("{} (at {}:{})", $msg, file!(), line!())
    };
}

#[macro_export]
macro_rules! track_error {
    ($result:expr, $context:expr) => {
        match $result {
            Ok(value) => Ok(value),
            Err(err) => {
                $crate::log_error!(err, $context);
                Err(err)
            }
        }
    };
}

#[macro_export]
macro_rules! try_with_context {
    ($result:expr, $context:expr, $err_fn:expr) => {
        match $result {
            Ok(value) => Ok(value),
            Err(err) => {
                let context_str = format!("{}: {}", $context, err);
                log::error!("{} (at {}:{})", context_str, file!(), line!());
                Err($err_fn(context_str))
            }
        }
    };
}
