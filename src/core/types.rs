use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::core::state::SharedState;
pub use crate::lsp::types::Language;

/// Shared context for MCP tools
#[derive(Clone)]
pub struct WinxContext {
    pub state: SharedState,
}

impl WinxContext {
    /// Create a new context with the shared state
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }
}

/// Available modes for the Winx agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Wcgw,
    Architect,
    CodeWriter(CodeWriterConfig),
}

/// Configuration for CodeWriter mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeWriterConfig {
    pub allowed_globs: AllowedItems,
    pub allowed_commands: AllowedItems,
}

/// Represents items that can be either "all" or a specific list
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AllowedItems {
    All(String),
    Specific(Vec<String>),
}

impl AllowedItems {
    pub fn is_all(&self) -> bool {
        match self {
            AllowedItems::All(s) => s == "all",
            _ => false,
        }
    }

    /// Update relative globs to absolute paths
    pub fn update_relative_paths(&mut self, workspace_root: &Path) {
        if let AllowedItems::Specific(items) = self {
            for item in items.iter_mut() {
                if !item.starts_with('/') && !item.starts_with('~') {
                    let path_str = item.clone();
                    *item = workspace_root.join(&path_str).to_string_lossy().to_string();
                }
            }
        }
    }
}

/// Initialize request for the Winx agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Initialize {
    pub init_type: InitType,
    pub any_workspace_path: String,
    pub initial_files_to_read: Vec<String>,
    pub task_id_to_resume: String,
    pub mode_name: ModeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_writer_config: Option<CodeWriterConfig>,
}

/// Type of initialization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InitType {
    FirstCall,
    UserAskedModeChange,
    ResetShell,
    UserAskedChangeWorkspace,
}

/// Mode type specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModeType {
    Wcgw,
    Architect,
    CodeWriter,
}

/// Command execution model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub command: String,
}

/// Status check model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusCheck {
    pub status_check: bool,
}

/// Text input model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendText {
    pub send_text: String,
}

/// Special key presses
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Special {
    Enter,
    KeyUp,
    KeyDown,
    KeyLeft,
    KeyRight,
    CtrlC,
    CtrlD,
}

impl std::str::FromStr for Special {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "enter" => Ok(Special::Enter),
            "key-up" => Ok(Special::KeyUp),
            "key-down" => Ok(Special::KeyDown),
            "key-left" => Ok(Special::KeyLeft),
            "key-right" => Ok(Special::KeyRight),
            "ctrl-c" => Ok(Special::CtrlC),
            "ctrl-d" => Ok(Special::CtrlD),
            _ => Err(format!("Unknown special key: {}", s)),
        }
    }
}

/// Special key presses model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendSpecials {
    pub send_specials: Vec<Special>,
}

/// ASCII character input model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendAscii {
    pub send_ascii: Vec<u8>,
}

/// Bash command execution model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashCommand {
    #[serde(flatten)]
    pub action_json: BashAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_for_seconds: Option<f32>,
}

/// Types of bash actions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BashAction {
    Command(Command),
    StatusCheck(StatusCheck),
    SendText(SendText),
    SendSpecials(SendSpecials),
    SendAscii(SendAscii),
}

/// File reading model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFiles {
    pub file_paths: Vec<String>,
    pub show_line_numbers_reason: Option<String>,
}

/// Enhanced file write/edit tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadState {
    /// Hash of the file content when last read
    pub file_hash: String,
    /// Ranges of lines that have been read
    pub read_ranges: Vec<(usize, usize)>,
    /// Time when the file was last read
    pub last_read: chrono::DateTime<chrono::Utc>,
}

impl FileReadState {
    /// Create a new file read state with a single range covering the entire file
    pub fn new(hash: String, total_lines: usize) -> Self {
        Self {
            file_hash: hash,
            read_ranges: vec![(1, total_lines)],
            last_read: chrono::Utc::now(),
        }
    }

    /// Create a new file read state with specific line ranges
    pub fn with_ranges(hash: String, ranges: Vec<(usize, usize)>) -> Self {
        Self {
            file_hash: hash,
            read_ranges: ranges,
            last_read: chrono::Utc::now(),
        }
    }

    /// Check if a file has been fully read
    pub fn is_fully_read(&self) -> bool {
        // If there are no ranges, the file hasn't been read at all
        if self.read_ranges.is_empty() {
            return false;
        }

        // Sort ranges for easier processing
        let mut sorted_ranges = self.read_ranges.clone();
        sorted_ranges.sort_by_key(|r| r.0);

        // Check if there are gaps in the ranges
        let mut current_end = 0;
        for (start, end) in sorted_ranges {
            if start > current_end + 1 {
                return false;
            }
            current_end = current_end.max(end);
        }

        true
    }

    /// Get unread ranges in a file
    pub fn get_unread_ranges(&self, total_lines: usize) -> Vec<(usize, usize)> {
        if self.read_ranges.is_empty() {
            return vec![(1, total_lines)];
        }

        let mut sorted_ranges = self.read_ranges.clone();
        sorted_ranges.sort_by_key(|r| r.0);

        let mut unread_ranges = Vec::new();
        let mut current_pos = 1;

        for (start, end) in sorted_ranges {
            if start > current_pos {
                unread_ranges.push((current_pos, start - 1));
            }
            current_pos = end + 1;
        }

        if current_pos <= total_lines {
            unread_ranges.push((current_pos, total_lines));
        }

        unread_ranges
    }

    /// Add a read range to the file state
    pub fn add_read_range(&mut self, start: usize, end: usize) {
        self.read_ranges.push((start, end));
        self.last_read = chrono::Utc::now();

        // Merge overlapping ranges
        self.merge_ranges();
    }

    /// Merge overlapping ranges for more efficient storage
    fn merge_ranges(&mut self) {
        if self.read_ranges.len() <= 1 {
            return;
        }

        self.read_ranges.sort_by_key(|r| r.0);

        let mut merged = Vec::new();
        let mut current = self.read_ranges[0];

        for (start, end) in self.read_ranges.iter().skip(1) {
            if *start <= current.1 + 1 {
                // Ranges overlap or are adjacent, merge them
                current.1 = current.1.max(*end);
            } else {
                // No overlap, add current range and start a new one
                merged.push(current);
                current = (*start, *end);
            }
        }

        merged.push(current);
        self.read_ranges = merged;
    }
}

/// File writing/editing model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteOrEdit {
    pub file_path: String,
    pub percentage_to_change: u8,
    pub file_content_or_search_replace_blocks: String,
    #[serde(default = "default_auto_read")]
    pub auto_read_if_needed: bool,
}

/// Default value for auto_read_if_needed (true for backward compatibility)
fn default_auto_read() -> bool {
    true
}

/// Context saving model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSave {
    pub id: String,
    pub project_root_path: String,
    pub description: String,
    pub relevant_file_globs: Vec<String>,
}

/// Image reading model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadImage {
    pub file_path: String,
}
