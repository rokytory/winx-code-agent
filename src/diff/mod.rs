pub mod checkpoint;
pub mod edit;
pub mod fuzzy_search;
pub mod git;
pub mod large_file;
pub mod operations;
pub mod search_replace;
pub mod search_replace_enhanced;

// Re-exportações específicas para evitar ambiguidade
pub use checkpoint::*;
pub use edit::*;
pub use fuzzy_search::*;
pub use git::*;

// Re-exportação explícita de large_file para evitar ambiguidade com operations
pub use large_file::{
    apply_operations as apply_file_operations, // Renomeando para evitar ambiguidade
    process_large_file,
    EditOperation,
    FileOperation,
};

// Re-exportação explícita de operations para evitar ambiguidade com large_file
pub use operations::{apply_operations, diff_strings, diff_strings_parallel, DiffOp};

pub use search_replace::*;
pub use search_replace_enhanced::*;
