use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub fn update_relative_paths(&mut self, workspace_root: &PathBuf) {
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

/// File writing/editing model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteOrEdit {
    pub file_path: String,
    pub percentage_to_change: u8,
    pub file_content_or_search_replace_blocks: String,
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
