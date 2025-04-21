use anyhow::Result;
use regex::Regex;
use rmcp::{model::CallToolResult, model::ErrorCode, schemars, tool, Error as McpError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::file::search_replace::{
    apply_search_replace_with_fallback, is_search_replace_content, parse_search_replace_blocks,
};
use crate::file::syntax_checker::check_syntax;
use crate::tools::initialize::{Action, Initialize};

/// Parse file path with optional line ranges
/// Returns (path, start_line, end_line)
fn parse_line_ranges(file_path: &str) -> (String, Option<usize>, Option<usize>) {
    // Match patterns like "/path/to/file:10-20" or "/path/to/file:10-" or "/path/to/file:-20"
    let re = Regex::new(r"^(.+?)(?::(\d*)-(\d*))?$").unwrap();

    if let Some(caps) = re.captures(file_path) {
        let path = caps.get(1).unwrap().as_str().to_string();
        let start_line = caps.get(2).and_then(|m| m.as_str().parse::<usize>().ok());
        let end_line = caps.get(3).and_then(|m| m.as_str().parse::<usize>().ok());
        (path, start_line, end_line)
    } else {
        (file_path.to_string(), None, None)
    }
}

// Global whitelist for file access
lazy_static::lazy_static! {
    static ref FILE_WHITELIST: Arc<Mutex<HashMap<PathBuf, FileWhitelistData>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

// Track file read permissions
#[derive(Debug, Clone)]
struct FileWhitelistData {
    file_hash: String,
    line_ranges_read: Vec<(usize, usize)>,
    total_lines: usize,
}

impl FileWhitelistData {
    fn new(file_hash: String, line_ranges_read: Vec<(usize, usize)>, total_lines: usize) -> Self {
        Self {
            file_hash,
            line_ranges_read,
            total_lines,
        }
    }

    fn get_percentage_read(&self) -> f64 {
        if self.total_lines == 0 {
            return 100.0;
        }

        let mut lines_read = std::collections::HashSet::new();
        for (start, end) in &self.line_ranges_read {
            for line in *start..=*end {
                lines_read.insert(line);
            }
        }

        (lines_read.len() as f64 / self.total_lines as f64) * 100.0
    }

    fn is_read_enough(&self) -> bool {
        self.get_percentage_read() >= 99.0
    }

    #[allow(dead_code)]
    fn get_unread_ranges(&self) -> Vec<(usize, usize)> {
        if self.total_lines == 0 {
            return Vec::new();
        }

        let mut lines_read = std::collections::HashSet::new();
        for (start, end) in &self.line_ranges_read {
            for line in *start..=*end {
                lines_read.insert(line);
            }
        }

        let mut unread_ranges = Vec::new();
        let mut start_range = None;

        for i in 1..=self.total_lines {
            if !lines_read.contains(&i) {
                if start_range.is_none() {
                    start_range = Some(i);
                }
            } else if let Some(start) = start_range {
                unread_ranges.push((start, i - 1));
                start_range = None;
            }
        }

        if let Some(start) = start_range {
            unread_ranges.push((start, self.total_lines));
        }

        unread_ranges
    }
}

#[derive(Debug, Clone)]
pub struct FileOperations {
    // State is managed globally via FILE_WHITELIST
}

impl FileOperations {
    pub fn new() -> Self {
        Self {}
    }

    // Add file to whitelist with read ranges
    fn add_to_whitelist(
        &self,
        file_path: &Path,
        ranges: Vec<(usize, usize)>,
    ) -> Result<(), McpError> {
        let content = fs::read(file_path).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to read file: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&content);
        let file_hash = format!("{:x}", hasher.finalize());

        let total_lines = content.iter().filter(|&&b| b == b'\n').count() + 1;

        let mut whitelist = FILE_WHITELIST.lock().map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to acquire lock: {}", e),
                None,
            )
        })?;

        let entry = whitelist
            .entry(file_path.to_path_buf())
            .or_insert_with(|| FileWhitelistData::new(file_hash.clone(), Vec::new(), total_lines));

        // Update hash if changed
        entry.file_hash = file_hash;

        // Add new ranges
        for range in ranges {
            entry.line_ranges_read.push(range);
        }

        Ok(())
    }

    // Check if file can be overwritten
    #[allow(dead_code)]
    fn can_overwrite(&self, file_path: &Path) -> Result<bool, McpError> {
        let whitelist = FILE_WHITELIST.lock().map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to acquire lock: {}", e),
                None,
            )
        })?;

        if let Some(data) = whitelist.get(file_path) {
            // Check if file hash matches and enough of the file has been read
            if let Ok(content) = fs::read(file_path) {
                let mut hasher = Sha256::new();
                hasher.update(&content);
                let current_hash = format!("{:x}", hasher.finalize());

                return Ok(current_hash == data.file_hash && data.is_read_enough());
            }
        }

        Ok(false)
    }

    // Get unread ranges for a file
    #[allow(dead_code)]
    fn get_unread_ranges(&self, file_path: &Path) -> Result<Vec<(usize, usize)>, McpError> {
        let whitelist = FILE_WHITELIST.lock().map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to acquire lock: {}", e),
                None,
            )
        })?;

        if let Some(data) = whitelist.get(file_path) {
            return Ok(data.get_unread_ranges());
        }

        Ok(Vec::new())
    }
}

#[derive(Debug, Clone)]
pub struct WriteIfEmpty {
    // State is managed globally via FILE_WHITELIST
}

impl WriteIfEmpty {
    pub fn new() -> Self {
        Self {}
    }

    // Check if file exists and is empty or doesn't exist
    #[allow(dead_code)]
    fn is_file_empty_or_nonexistent(&self, path: &Path) -> Result<bool, McpError> {
        if path.exists() {
            let metadata = fs::metadata(path).map_err(|e| {
                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Failed to get file metadata: {}", e),
                    Some(json!({"error": e.to_string()})),
                )
            })?;

            Ok(metadata.len() == 0)
        } else {
            Ok(true)
        }
    }

    // Check if it should redirect to a temporary path
    fn check_and_redirect(&self, path: &Path) -> Option<PathBuf> {
        // If the path is at the root but not in /tmp or in the user's home
        if path.starts_with("/")
            && !path.starts_with("/tmp")
            && !path.starts_with("/Users")
            && !path.starts_with("/home")
        {
            // Redirect to /tmp with the same filename
            let file_name = path.file_name().unwrap_or_default();
            let redirected = PathBuf::from("/tmp").join(file_name);

            log::warn!(
                "Redirecting file operation from {} to {} due to possible read-only filesystem",
                path.display(),
                redirected.display()
            );

            Some(redirected)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileEdit {
    // State is managed globally via FILE_WHITELIST
}

impl FileEdit {
    pub fn new() -> Self {
        Self {}
    }

    // Add file to whitelist with read ranges
    #[allow(dead_code)]
    fn add_to_whitelist(
        &self,
        file_path: &Path,
        ranges: Vec<(usize, usize)>,
    ) -> Result<(), McpError> {
        let content = fs::read(file_path).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to read file: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&content);
        let file_hash = format!("{:x}", hasher.finalize());

        let total_lines = content.iter().filter(|&&b| b == b'\n').count() + 1;

        let mut whitelist = FILE_WHITELIST.lock().map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to acquire lock: {}", e),
                None,
            )
        })?;

        let entry = whitelist
            .entry(file_path.to_path_buf())
            .or_insert_with(|| FileWhitelistData::new(file_hash.clone(), Vec::new(), total_lines));

        // Update hash if changed
        entry.file_hash = file_hash;

        // Add new ranges
        for range in ranges {
            entry.line_ranges_read.push(range);
        }

        Ok(())
    }

    // Check if file can be overwritten
    #[allow(dead_code)]
    fn can_overwrite(&self, file_path: &Path) -> Result<bool, McpError> {
        let whitelist = FILE_WHITELIST.lock().map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to acquire lock: {}", e),
                None,
            )
        })?;

        if let Some(data) = whitelist.get(file_path) {
            // Check if file hash matches and enough of the file has been read
            if let Ok(content) = fs::read(file_path) {
                let mut hasher = Sha256::new();
                hasher.update(&content);
                let current_hash = format!("{:x}", hasher.finalize());

                return Ok(current_hash == data.file_hash && data.is_read_enough());
            }
        }

        Ok(false)
    }

    // Get unread ranges for a file
    #[allow(dead_code)]
    fn get_unread_ranges(&self, file_path: &Path) -> Result<Vec<(usize, usize)>, McpError> {
        let whitelist = FILE_WHITELIST.lock().map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to acquire lock: {}", e),
                None,
            )
        })?;

        if let Some(data) = whitelist.get(file_path) {
            return Ok(data.get_unread_ranges());
        }

        Ok(Vec::new())
    }
}

impl Default for FileOperations {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadFilesParams {
    #[schemars(description = "Paths of files to read")]
    pub file_paths: Vec<String>,

    #[schemars(description = "Reason for showing line numbers")]
    pub show_line_numbers_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FileWriteOrEditParams {
    #[schemars(description = "Path of file to write or edit")]
    pub file_path: String,

    #[schemars(description = "Percentage of file to change (0-100)")]
    pub percentage_to_change: i32,

    #[schemars(description = "File content or search/replace blocks")]
    pub file_content_or_search_replace_blocks: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WriteIfEmptyParams {
    #[schemars(description = "Path of file to write")]
    pub file_path: String,

    #[schemars(description = "Content to write to file")]
    pub file_content: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FileEditParams {
    #[schemars(description = "Path of file to edit")]
    pub file_path: String,

    #[schemars(description = "Edit using search/replace blocks")]
    pub file_edit_using_search_replace_blocks: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadImageParams {
    #[schemars(description = "Path of image to read")]
    pub file_path: String,
}

impl Default for WriteIfEmpty {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for FileEdit {
    fn default() -> Self {
        Self::new()
    }
}

#[tool(tool_box)]
impl FileOperations {
    #[tool(description = "Read files from disk")]
    pub async fn read_files(
        &self,
        #[tool(aggr)] params: ReadFilesParams,
    ) -> Result<CallToolResult, McpError> {
        // Check if initialization has been done
        crate::ensure_initialized!("You must call 'initialize' before reading files.");

        // Check permission
        // Convert WinxError to McpError for permissions
        if let Err(e) = Initialize::check_permission(Action::ReadFile, None) {
            return Err(e.to_mcp_error());
        }

        let mut result = String::new();
        let mut file_ranges = Vec::new();

        for file_path in &params.file_paths {
            let path = PathBuf::from(file_path);
            if !path.exists() {
                result.push_str(&format!("\n{}: File does not exist\n", file_path));
                continue;
            }

            // Parse line ranges if present in the file path
            let (parsed_path, start_line, end_line) = parse_line_ranges(file_path);
            let effective_path = if parsed_path.is_empty() {
                file_path
            } else {
                &parsed_path
            };

            let range_path = PathBuf::from(effective_path);

            // Use a read operation that avoids cache to ensure more fresh data
            match fs::read(&range_path) {
                Ok(bytes) => {
                    // Convert bytes to string
                    let content = match String::from_utf8(bytes.clone()) {
                        Ok(text) => text,
                        Err(_) => {
                            // If not valid UTF-8, use a safer representation
                            String::from_utf8_lossy(&bytes).to_string()
                        }
                    };

                    // Check if file is large and requires chunking
                    const MAX_CONTENT_SIZE: usize = 1_000_000; // ~1MB
                    let is_large_file = content.len() > MAX_CONTENT_SIZE;

                    // Split content into lines
                    let lines: Vec<&str> = content.lines().collect();
                    let total_lines = lines.len();

                    // Apply line ranges or chunking for large files
                    let (start_idx, end_idx) =
                        if is_large_file && start_line.is_none() && end_line.is_none() {
                            // For large files without specified ranges, return only first chunk
                            let chunk_size = 500; // Number of lines per chunk
                            (0, chunk_size.min(total_lines))
                        } else {
                            (
                                start_line.map(|s| s.saturating_sub(1)).unwrap_or(0),
                                end_line.unwrap_or(total_lines),
                            )
                        };

                    let selected_lines = if start_idx < total_lines && end_idx > 0 {
                        &lines[start_idx.min(total_lines - 1)..end_idx.min(total_lines)]
                    } else {
                        &[]
                    };

                    // Add warning for large files
                    if is_large_file && start_line.is_none() && end_line.is_none() {
                        result.push_str(&format!(
                            "\nNote: {} is a large file ({:.2} MB, {} lines). Only showing first 500 lines. Use line ranges for specific sections (e.g. {}:501-1000).\n",
                            effective_path,
                            content.len() as f64 / 1_000_000.0,
                            total_lines,
                            effective_path
                        ));
                    }

                    // Add to result
                    let range_suffix = if start_line.is_some() || end_line.is_some() {
                        format!(
                            ":{}-{}",
                            start_line.map(|s| s.to_string()).unwrap_or_default(),
                            end_line.map(|e| e.to_string()).unwrap_or_default()
                        )
                    } else {
                        String::new()
                    };

                    result.push_str(&format!("\n{}{}\n```\n", effective_path, range_suffix));

                    // Calculate effective line range for whitelist
                    let effective_start = start_line.unwrap_or(1);
                    let effective_end = end_line.unwrap_or(total_lines);

                    // Add to file ranges for whitelist
                    file_ranges.push((range_path.clone(), (effective_start, effective_end)));

                    // Add line numbers if requested
                    if params.show_line_numbers_reason.is_some() {
                        for (i, line) in selected_lines.iter().enumerate() {
                            result.push_str(&format!("{} {}\n", i + start_idx + 1, line));
                        }
                    } else {
                        result.push_str(&selected_lines.join("\n"));
                        if !selected_lines.is_empty() {
                            result.push('\n');
                        }
                    }

                    result.push_str("```");

                    // Update whitelist with read ranges
                    self.add_to_whitelist(&range_path, vec![(effective_start, effective_end)])?;
                }
                Err(e) => {
                    result.push_str(&format!("\n{}: Error reading file: {}\n", file_path, e));
                }
            }
        }

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            result,
        )]))
    }

    #[tool(description = "Write or edit a file")]
    pub async fn file_write_or_edit(
        &self,
        #[tool(aggr)] params: FileWriteOrEditParams,
    ) -> Result<CallToolResult, McpError> {
        let path = PathBuf::from(&params.file_path);

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    McpError::new(
                        ErrorCode::INTERNAL_ERROR,
                        format!("Failed to create directory: {}", e),
                        Some(json!({"error": e.to_string()})),
                    )
                })?;
            }
        }

        // Check if this is a search/replace operation or direct write
        let is_edit = is_search_replace_content(
            &params.file_content_or_search_replace_blocks,
            params.percentage_to_change,
        );

        // If file exists, verify permissions
        if path.exists() {
            // Check if we have permission to overwrite
            if !self.can_overwrite(&path)? {
                // If permissions missing, try to provide the file content
                if let Ok(file_content) = fs::read_to_string(&path) {
                    // Get unread ranges
                    let unread_ranges = self.get_unread_ranges(&path)?;

                    if !unread_ranges.is_empty() {
                        // There are unread ranges, suggest reading them
                        let ranges_str = unread_ranges
                            .iter()
                            .map(|(start, end)| format!("{}-{}", start, end))
                            .collect::<Vec<_>>()
                            .join(", ");

                        return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                            format!("Error: you need to read more of the file before it can be overwritten.\nUnread line ranges: {}\n\nHere's the unread content:\n```\n{}\n```\n\nYou can now safely retry writing immediately considering the above information.",
                                    ranges_str, file_content)
                        )]));
                    } else {
                        // File hash doesn't match, file may have changed
                        return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                            format!("Error: the file has changed since last read.\n\nHere's the existing file:\n```\n{}\n```\n\nYou can now safely retry writing immediately considering the above information.",
                                    file_content)
                        )]));
                    }
                } else {
                    // Can't read the file for some reason
                    return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                        format!("Error: you need to read existing file {} at least once before it can be overwritten.", params.file_path)
                    )]));
                }
            }
        }

        // Handle search/replace editing
        if is_edit && path.exists() {
            // Read file
            let original_content = fs::read_to_string(&path).map_err(|e| {
                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Failed to read file for editing: {}", e),
                    Some(json!({"error": e.to_string()})),
                )
            })?;

            // Parse search/replace blocks
            let blocks = parse_search_replace_blocks(&params.file_content_or_search_replace_blocks)
                .map_err(|e| {
                    McpError::new(
                        ErrorCode::INVALID_PARAMS,
                        format!("Search/Replace syntax error: {}", e),
                        Some(json!({"error": e.to_string()})),
                    )
                })?;

            // Apply search/replace with fallback strategy for multiple blocks
            let result = apply_search_replace_with_fallback(&original_content, &blocks, |msg| {
                log::debug!("{}", msg);
            })
                .map_err(|e| {
                    // Keep the original error message which includes instructions for retry
                    McpError::new(
                        ErrorCode::INVALID_PARAMS,
                        format!("Search/Replace error: {}", e),
                        Some(json!({"error": e.to_string(), "percentage_to_change": params.percentage_to_change})),
                    )
                });

            match result {
                Ok((edited_content, warnings)) => {
                    // Write the edited file
                    fs::write(&path, edited_content).map_err(|e| {
                        McpError::new(
                            ErrorCode::INTERNAL_ERROR,
                            format!("Failed to write file: {}", e),
                            Some(json!({"error": e.to_string()})),
                        )
                    })?;

                    let success_msg = if warnings.is_empty() {
                        format!("Success: File edited at {}", params.file_path)
                    } else {
                        format!(
                            "Success: File edited at {}. Warnings: {}",
                            params.file_path,
                            warnings.join(", ")
                        )
                    };

                    return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                        success_msg,
                    )]));
                }
                Err(e) => return Err(e),
            }
        } else {
            // Direct write
            fs::write(&path, &params.file_content_or_search_replace_blocks).map_err(|e| {
                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Failed to write file: {}", e),
                    Some(json!({"error": e.to_string()})),
                )
            })?;
        }

        // Count lines in the new file for whitelist
        if let Ok(content) = fs::read_to_string(&path) {
            let lines = content.lines().count();
            // Add entire file to whitelist
            self.add_to_whitelist(&path, vec![(1, lines)])?;
        }

        let result = format!("Success: File written to {}", params.file_path);

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            result,
        )]))
    }

    #[tool(description = "Read an image from disk")]
    pub async fn read_image(
        &self,
        #[tool(aggr)] params: ReadImageParams,
    ) -> Result<CallToolResult, McpError> {
        // Check if initialization has been done
        crate::ensure_initialized!("You must call 'initialize' before reading images.");

        // Check permission
        // Convert WinxError to McpError for permissions
        if let Err(e) = Initialize::check_permission(Action::ReadImage, None) {
            return Err(e.to_mcp_error());
        }

        use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
        use std::io::Read;

        let path = PathBuf::from(&params.file_path);
        if !path.exists() {
            return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                format!("Error: File {} does not exist", params.file_path),
            )]));
        }

        let mut file = fs::File::open(&path).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to open file: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to read file data: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        let mime_type = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string();

        let base64_data = BASE64.encode(&buffer);

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            format!("data:{};base64,{}", mime_type, base64_data),
        )]))
    }
}

#[tool(tool_box)]
impl FileEdit {
    #[tool(
        description = "\n- Edits existing files using search/replace blocks.\n- Uses intelligent search and replace with multiple tolerance levels.\n- Automatically fixes indentation issues when possible.\n- Provides detailed feedback on match failures with suggested fixes.\n- Includes fallback strategy for multiple block edits.\n"
    )]
    pub async fn file_edit(
        &self,
        #[tool(aggr)] params: FileEditParams,
    ) -> Result<CallToolResult, McpError> {
        // Check if initialization has been done
        crate::ensure_initialized!("You must call 'initialize' before editing files.");

        // Check permission
        // Convert WinxError to McpError for permissions
        if let Err(e) = Initialize::check_permission(Action::EditFile, Some(&params.file_path)) {
            return Err(e.to_mcp_error());
        }

        let path = PathBuf::from(&params.file_path);

        // Check if file exists
        if !path.exists() {
            return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                format!(
                    "Error: File {} does not exist. Use WriteIfEmpty to create new files.",
                    params.file_path
                ),
            )]));
        }

        // Read file
        let original_content = fs::read_to_string(&path).map_err(|e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to read file for editing: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        // Check for common syntax problems before processing
        let input = &params.file_edit_using_search_replace_blocks;
        if input.contains("<<<<<<< ORIGINAL") || input.contains(">>>>>>> UPDATED") {
            // Common syntax problem - using ORIGINAL instead of SEARCH or UPDATED instead of REPLACE
            let error_msg = if input.contains("<<<<<<< ORIGINAL") {
                "You are using '<<<<<<< ORIGINAL' which is incorrect syntax. Use '<<<<<<< SEARCH' instead."
            } else {
                "You are using '>>>>>>> UPDATED' which is incorrect syntax. Use '>>>>>>> REPLACE' instead."
            };

            return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                format!(
                    "Search/Replace syntax error: {}\n\nPlease use one of these formats:\n\n\
                 1. Standard format (recommended):\n\
                 <<<<<<< SEARCH\n\
                 search content\n\
                 =======\n\
                 replace content\n\
                 >>>>>>> REPLACE\n\n\
                 2. Simple format:\n\
                 <<<\n\
                 search content\n\
                 >>>\n\
                 replace content\n\n\
                 3. Markdown code blocks format:\n\
                 ```\n\
                 search content\n\
                 ```\n\n\
                 ```\n\
                 replace content\n\
                 ```\n\n\
                 Common mistakes to avoid:\n\
                 - Using ORIGINAL instead of SEARCH\n\
                 - Using UPDATED instead of REPLACE\n\
                 - Missing the divider (=======)\n\
                 - Not including enough context to make the match unique",
                    error_msg
                ),
            )]));
        }

        // Parse search/replace blocks with better error handling
        let blocks =
            match parse_search_replace_blocks(&params.file_edit_using_search_replace_blocks) {
                Ok(blocks) => blocks,
                Err(e) => {
                    // Provide helpful examples with the error message
                    let helpful_message = format!(
                        "Search/Replace syntax error: {}\n\nPlease use one of these formats:\n\n\
                     1. Standard format (recommended):\n\
                     <<<<<<< SEARCH\n\
                     search content\n\
                     =======\n\
                     replace content\n\
                     >>>>>>> REPLACE\n\n\
                     2. Simple format:\n\
                     <<<\n\
                     search content\n\
                     >>>\n\
                     replace content\n\n\
                     3. Markdown code blocks format:\n\
                     ```\n\
                     search content\n\
                     ```\n\n\
                     ```\n\
                     replace content\n\
                     ```\n\n\
                     Common mistakes to avoid:\n\
                     - Using ORIGINAL instead of SEARCH\n\
                     - Using UPDATED instead of REPLACE\n\
                     - Missing the divider (=======)\n\
                     - Not including enough context to make the match unique\n\n\
                     Try again with one of these formats.",
                        e
                    );

                    return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                        helpful_message,
                    )]));
                }
            };

        // Apply search/replace with fallback strategy
        let result = apply_search_replace_with_fallback(&original_content, &blocks, |msg| {
            log::debug!("{}", msg);
        })
        .map_err(|e| {
            McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Search/Replace error: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        });

        match result {
            Ok((edited_content, warnings)) => {
                // Check syntax before writing
                let syntax_warnings = check_syntax(&path, &edited_content);

                // Get a copy of edited content for hashing later
                let edited_content_copy = edited_content.clone();

                // Write the edited file
                fs::write(&path, &edited_content).map_err(|e| {
                    McpError::new(
                        ErrorCode::INTERNAL_ERROR,
                        format!("Failed to write file: {}", e),
                        Some(json!({"error": e.to_string()})),
                    )
                })?;

                // Update whitelist entry
                if let Ok(content) = fs::read_to_string(&path) {
                    let lines = content.lines().count();

                    // Create hash of new content
                    let mut hasher = Sha256::new();
                    hasher.update(edited_content_copy.as_bytes());
                    let file_hash = format!("{:x}", hasher.finalize());

                    let mut whitelist = FILE_WHITELIST.lock().map_err(|e| {
                        McpError::new(
                            ErrorCode::INTERNAL_ERROR,
                            format!("Failed to acquire lock: {}", e),
                            None,
                        )
                    })?;

                    whitelist.insert(
                        path.clone(),
                        FileWhitelistData::new(file_hash, vec![(1, lines)], lines),
                    );
                }

                let mut all_warnings = warnings.clone();
                all_warnings.extend(syntax_warnings);

                let success_msg = if all_warnings.is_empty() {
                    format!("Success: File edited at {}", params.file_path)
                } else {
                    format!(
                        "Success: File edited at {}. Warnings: {}",
                        params.file_path,
                        all_warnings.join(", ")
                    )
                };

                Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    success_msg,
                )]))
            }
            Err(e) => Err(e),
        }
    }
}

#[tool(tool_box)]
impl WriteIfEmpty {
    #[tool(description = "Create new files or write to empty files only")]
    pub async fn write_if_empty(
        &self,
        #[tool(aggr)] params: WriteIfEmptyParams,
    ) -> Result<CallToolResult, McpError> {
        // Check if initialization has been done
        crate::ensure_initialized!("You must call 'initialize' before creating files.");

        // Check permission
        // Convert WinxError to McpError for permissions
        if let Err(e) = Initialize::check_permission(Action::WriteFile, Some(&params.file_path)) {
            return Err(e.to_mcp_error());
        }

        let original_path = PathBuf::from(&params.file_path);

        // Check if the path needs to be redirected (read-only filesystem)
        let path = if let Some(redirected_path) = self.check_and_redirect(&original_path) {
            // If redirected, let's use the new path
            redirected_path
        } else {
            original_path.clone()
        };

        // Create warning message if there was a redirection
        let warning = if path != original_path {
            Some(format!(
                "Warning: Using {} instead of {} because the target may be in a read-only location",
                path.display(),
                original_path.display()
            ))
        } else {
            None
        };

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                let create_result = fs::create_dir_all(parent);
                if let Err(ref e) = create_result {
                    // Check if the error is a permission or read-only file system issue
                    if e.kind() == std::io::ErrorKind::PermissionDenied
                        || e.kind() == std::io::ErrorKind::Other
                    {
                        // Try to create in /tmp as a last option
                        let tmp_path =
                            PathBuf::from("/tmp").join(path.file_name().unwrap_or_default());
                        let tmp_parent = tmp_path.parent().unwrap_or(&tmp_path);

                        if let Err(tmp_err) = fs::create_dir_all(tmp_parent) {
                            // If even /tmp doesn't work, return improved error
                            return Err(McpError::new(
                                ErrorCode::INVALID_PARAMS,
                                format!(
                                    "Could not create directory even in /tmp: {}. Please specify a path where you have write permissions.",
                                    tmp_err
                                ),
                                Some(json!({"error": tmp_err.to_string()})),
                            ));
                        }

                        // Use the path in /tmp directly, instead of calling recursively
                        let warning = format!(
                            "Warning: Could not create directory '{}' due to permission error: {}. Using '{}' instead.",
                            path.display(), e, tmp_path.display()
                        );

                        fs::write(&tmp_path, &params.file_content).map_err(|e| {
                            McpError::new(
                                ErrorCode::INTERNAL_ERROR,
                                format!("Failed to write file to /tmp path: {}", e),
                                Some(json!({"error": e.to_string()})),
                            )
                        })?;

                            // Add to whitelist for future edits
                        let lines = params.file_content.lines().count();

                        // Create content hash
                        let mut hasher = Sha256::new();
                        hasher.update(params.file_content.as_bytes());
                        let file_hash = format!("{:x}", hasher.finalize());

                        let mut whitelist = FILE_WHITELIST.lock().map_err(|e| {
                            McpError::new(
                                ErrorCode::INTERNAL_ERROR,
                                format!("Failed to acquire lock: {}", e),
                                None,
                            )
                        })?;

                        whitelist.insert(
                            tmp_path.clone(),
                            FileWhitelistData::new(file_hash, vec![(1, lines)], lines),
                        );

                        return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                            format!(
                                "{}\n\nSuccess: New file created at {}",
                                warning,
                                tmp_path.display()
                            ),
                        )]));
                    } else {
                        // Other kinds of errors
                        return Err(McpError::new(
                            ErrorCode::INTERNAL_ERROR,
                            format!("Failed to create directory: {}", e),
                            Some(json!({"error": e.to_string()})),
                        ));
                    }
                }
            }
        }

        // Check if file exists and is not empty
        if path.exists() {
            let metadata = fs::metadata(&path).map_err(|e| {
                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Failed to get file metadata: {}", e),
                    Some(json!({"error": e.to_string()})),
                )
            })?;

            if metadata.len() > 0 {
                return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    format!("Error: File {} already exists and is not empty. Use FileEdit to modify existing files.", path.display())
                )]));
            }
        }

        // Check syntax before writing
        let syntax_warnings = check_syntax(&path, &params.file_content);

        // Write to the file with improved error handling
        match fs::write(&path, &params.file_content) {
            Ok(_) => {
                // Success case
            }
            Err(e) => {
                // Check if it's a permission or read-only file system error
                if e.kind() == std::io::ErrorKind::PermissionDenied
                    || e.kind() == std::io::ErrorKind::Other
                {
                    // Covers "Read-only file system"
                    // Try writing to /tmp as a last resort
                    let tmp_path = PathBuf::from("/tmp").join(path.file_name().unwrap_or_default());

                    match fs::write(&tmp_path, &params.file_content) {
                        Ok(_) => {
                            // Add to whitelist for future edits
                            let lines = params.file_content.lines().count();

                            // Create content hash
                            let mut hasher = Sha256::new();
                            hasher.update(params.file_content.as_bytes());
                            let file_hash = format!("{:x}", hasher.finalize());

                            let mut whitelist = FILE_WHITELIST.lock().map_err(|e| {
                                McpError::new(
                                    ErrorCode::INTERNAL_ERROR,
                                    format!("Failed to acquire lock: {}", e),
                                    None,
                                )
                            })?;

                            whitelist.insert(
                                tmp_path.clone(),
                                FileWhitelistData::new(file_hash, vec![(1, lines)], lines),
                            );

                            let result = format!(
                                "Warning: Could not write to {} due to permission error. File created at {} instead.",
                                path.display(),
                                tmp_path.display()
                            );

                            return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                                result,
                            )]));
                        }
                        Err(tmp_err) => {
                            return Err(McpError::new(
                                ErrorCode::INTERNAL_ERROR,
                                format!(
                                    "Failed to write file to both original location and /tmp. Original error: {}. /tmp error: {}", 
                                    e, tmp_err
                                ),
                                Some(json!({"original_error": e.to_string(), "tmp_error": tmp_err.to_string()})),
                            ));
                        }
                    }
                } else {
                    // Handle other kinds of errors
                    return Err(McpError::new(
                        ErrorCode::INTERNAL_ERROR,
                        format!("Failed to write file: {}", e),
                        Some(json!({"error": e.to_string()})),
                    ));
                }
            }
        };

        // Add to whitelist for future edits
        if let Ok(content) = fs::read_to_string(&path) {
            let lines = content.lines().count();

            // Create shared whitelist handling
            let mut hasher = Sha256::new();
            hasher.update(params.file_content.as_bytes());
            let file_hash = format!("{:x}", hasher.finalize());

            let mut whitelist = FILE_WHITELIST.lock().map_err(|e| {
                McpError::new(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Failed to acquire lock: {}", e),
                    None,
                )
            })?;

            whitelist.insert(
                path.clone(),
                FileWhitelistData::new(file_hash, vec![(1, lines)], lines),
            );
        }

        // Build result message
        let mut result = String::new();

        // Add warning if necessary
        if let Some(warn_msg) = warning {
            result.push_str(&format!("{}\n\n", warn_msg));
        }

        // Adicionar mensagem de sucesso
        if syntax_warnings.is_empty() {
            result.push_str(&format!("Success: New file created at {}", path.display()));
        } else {
            result.push_str(&format!(
                "Success: New file created at {}. Syntax warnings: {}",
                path.display(),
                syntax_warnings.join(", ")
            ));
        }

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            result,
        )]))
    }
}
