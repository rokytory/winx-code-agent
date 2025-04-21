// State representation for Reinforcement Learning
// Captures the current state of the codebase and project environment

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Metadata about a file in the codebase
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileMetadata {
    /// Size of the file in bytes
    pub size: usize,
    /// Last modification timestamp
    pub last_modified: u64,
    /// File extension
    pub extension: Option<String>,
    /// Whether the file contains syntax errors
    pub has_syntax_errors: bool,
}

/// Status of the build process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildStatus {
    /// Build succeeded
    Success,
    /// Build failed
    Failed,
    /// Build status unknown
    Unknown,
}

/// A change in the codebase
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Change {
    /// Path of the file that was changed
    pub file_path: PathBuf,
    /// Type of change
    pub change_type: ChangeType,
    /// Timestamp of the change
    pub timestamp: u64,
}

/// Type of change in the codebase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeType {
    /// File was created
    Created,
    /// File was modified
    Modified,
    /// File was deleted
    Deleted,
}

/// Representation of a syntax error
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyntaxError {
    /// Path of the file containing the error
    pub file_path: PathBuf,
    /// Line number where the error occurs
    pub line: usize,
    /// Column number where the error occurs
    pub column: usize,
    /// Error message
    pub message: String,
    /// Error severity
    pub severity: ErrorSeverity,
}

/// Severity of a syntax error
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorSeverity {
    /// Error that prevents compilation
    Error,
    /// Warning that doesn't prevent compilation
    Warning,
    /// Informational message
    Info,
}

/// State of a codebase at a particular point in time
#[derive(Debug, Clone)]
pub struct CodebaseState {
    /// File system representation
    pub file_structure: HashMap<PathBuf, FileMetadata>,
    /// Syntax errors in the codebase
    pub syntax_errors: Vec<SyntaxError>,
    /// Test coverage percentage (0-100)
    pub test_coverage: f64,
    /// Build status
    pub build_status: BuildStatus,
    /// Recent changes to the codebase
    pub recent_changes: Vec<Change>,
    /// Description of the current task
    pub task_description: String,
    /// Current working directory
    pub current_dir: PathBuf,
}

impl PartialEq for CodebaseState {
    fn eq(&self, other: &Self) -> bool {
        // Compare only the essential fields for equality
        // This simplifies state comparison for RL algorithms
        self.syntax_errors.len() == other.syntax_errors.len()
            && (self.test_coverage - other.test_coverage).abs() < 0.001
            && self.build_status == other.build_status
            && self.file_structure.len() == other.file_structure.len()
    }
}

impl Eq for CodebaseState {}

impl Hash for CodebaseState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash only the essential fields
        // This is a simplified hash for RL state space
        self.syntax_errors.len().hash(state);
        ((self.test_coverage * 1000.0) as u64).hash(state);
        self.build_status.hash(state);
        self.file_structure.len().hash(state);
    }
}

impl CodebaseState {
    /// Create a new empty codebase state
    pub fn new(current_dir: PathBuf, task_description: String) -> Self {
        Self {
            file_structure: HashMap::new(),
            syntax_errors: Vec::new(),
            test_coverage: 0.0,
            build_status: BuildStatus::Unknown,
            recent_changes: Vec::new(),
            task_description,
            current_dir,
        }
    }

    /// Update the state based on a file change
    pub fn update_file(&mut self, path: PathBuf, metadata: FileMetadata) {
        let path_clone = path.clone();
        self.file_structure.insert(path, metadata);

        // Record as a change
        self.recent_changes.push(Change {
            file_path: path_clone,
            change_type: ChangeType::Modified,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });

        // Keep only the most recent changes
        if self.recent_changes.len() > 10 {
            self.recent_changes.remove(0);
        }
    }

    /// Add a syntax error to the state
    pub fn add_syntax_error(&mut self, error: SyntaxError) {
        self.syntax_errors.push(error);
    }

    /// Clear all syntax errors for a specific file
    pub fn clear_syntax_errors_for_file(&mut self, file_path: &Path) {
        self.syntax_errors
            .retain(|error| error.file_path != file_path);
    }

    /// Update the build status
    pub fn set_build_status(&mut self, status: BuildStatus) {
        self.build_status = status;
    }

    /// Update the test coverage
    pub fn set_test_coverage(&mut self, coverage: f64) {
        self.test_coverage = coverage.clamp(0.0, 100.0);
    }

    /// Create a simplified version of the state for use in RL algorithms
    pub fn to_simplified_state(&self) -> SimplifiedCodebaseState {
        SimplifiedCodebaseState {
            file_count: self.file_structure.len(),
            error_count: self.syntax_errors.len(),
            warning_count: self
                .syntax_errors
                .iter()
                .filter(|e| e.severity == ErrorSeverity::Warning)
                .count(),
            test_coverage: (self.test_coverage as usize),
            build_success: self.build_status == BuildStatus::Success,
        }
    }
}

/// A simplified version of CodebaseState for efficient RL state representation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimplifiedCodebaseState {
    /// Number of files in the codebase
    pub file_count: usize,
    /// Number of syntax errors
    pub error_count: usize,
    /// Number of warnings
    pub warning_count: usize,
    /// Test coverage percentage (0-100) as an integer
    pub test_coverage: usize,
    /// Whether the build is successful
    pub build_success: bool,
}

/// Extracts the current state of the codebase from the environment
#[derive(Debug, Clone)]
pub struct StateTracker {
    /// The previous state of the codebase
    previous_state: Option<CodebaseState>,
    /// The current state of the codebase
    current_state: Option<CodebaseState>,
}

impl StateTracker {
    /// Create a new state tracker
    pub fn new() -> Self {
        Self {
            previous_state: None,
            current_state: None,
        }
    }

    /// Extract the current state from the agent context
    pub fn extract_state(&mut self, context: &crate::tools::AgentContext) -> CodebaseState {
        // Store the current state as the previous state
        if let Some(current) = self.current_state.take() {
            self.previous_state = Some(current);
        }

        // Create a new state based on the context
        // This would use the actual context data in a real implementation
        let state = CodebaseState::new(
            PathBuf::from(context.cwd.clone()),
            context.task_description.clone(),
        );

        // Store and return the new state
        self.current_state = Some(state.clone());
        state
    }

    /// Get the previous state
    pub fn get_previous_state(&self) -> CodebaseState {
        self.previous_state.clone().unwrap_or_else(|| {
            // Create a default state if no previous state exists
            CodebaseState::new(PathBuf::from("/"), String::new())
        })
    }
}

impl Default for StateTracker {
    fn default() -> Self {
        Self::new()
    }
}
