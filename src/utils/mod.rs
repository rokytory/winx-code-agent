pub mod concurrency;
pub mod directory_tree;
pub mod error_handling;
pub mod fs;
pub mod path_importance;
pub mod paths;

// Re-exports para facilitar o uso
pub use concurrency::{get_lock_manager, FileLockManager, FileOperationGuard, LockStatus};
pub use error_handling::{
    command_error, file_error, format_error, is_error_type, localized_error, state_error,
    ErrorContextExt, ErrorType, LocalizedError,
};
