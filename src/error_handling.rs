use crate::error::{WinxError, WinxResult};
use std::path::Path;

/// Executes an operation with proper context-aware error handling
pub fn with_context<T, F, C>(operation: F, context: C) -> WinxResult<T>
where
    F: FnOnce() -> WinxResult<T>,
    C: AsRef<str>,
{
    operation().map_err(|e| {
        log::error!("{}: {} (at {}:{})", context.as_ref(), e, file!(), line!());
        WinxError::other(format!("{}: {}", context.as_ref(), e))
    })
}

/// Handles file operation errors with file path context
pub fn with_file_context<T, F>(operation: F, path: impl AsRef<Path>) -> WinxResult<T>
where
    F: FnOnce() -> std::io::Result<T>,
{
    let path_ref = path.as_ref();
    operation().map_err(|e| {
        log::error!("File operation failed on {}: {}", path_ref.display(), e);
        WinxError::file_error(e.to_string(), path_ref)
    })
}

/// Safely executes filesystem operations with appropriate error handling
pub fn fs_operation<T, F>(operation: F, path: impl AsRef<Path>, operation_name: &str) -> WinxResult<T>
where
    F: FnOnce() -> std::io::Result<T>,
{
    let path_ref = path.as_ref();
    operation().map_err(|e| {
        log::error!(
            "{} failed on {}: {} (at {}:{})",
            operation_name,
            path_ref.display(),
            e,
            file!(),
            line!()
        );
        
        match e.kind() {
            std::io::ErrorKind::NotFound => {
                WinxError::invalid_path(path_ref.to_string_lossy().to_string())
            }
            std::io::ErrorK