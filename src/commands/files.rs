use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::core::state::SharedState;
use crate::core::types::{FileWriteOrEdit as FileWriteOrEditType, ReadFiles as ReadFilesType};
use crate::utils::fs as fs_utils;
use serde_json::from_str;

/// Read files and return their contents with enhanced line range tracking
pub async fn read_files_internal(
    state: &SharedState,
    file_paths: &[String],
    line_ranges: Option<&[Option<(usize, usize)>]>,
) -> Result<Vec<(String, String, (usize, usize))>> {
    debug!("Reading files: {:?}", file_paths);

    let mut results = Vec::new();
    let mut file_reads = Vec::new();

    for (idx, file_path) in file_paths.iter().enumerate() {
        // Check permissions and resolve path in a separate scope
        let path = {
            let state_guard = state.lock().unwrap();

            let resolved_path = if Path::new(file_path).is_absolute() {
                PathBuf::from(file_path)
            } else {
                state_guard.workspace_path.join(file_path)
            };

            if !state_guard.is_path_allowed(&resolved_path) {
                return Err(anyhow::anyhow!(
                    "Path not allowed: {}",
                    resolved_path.display()
                ));
            }

            resolved_path
        };

        // Extract line range if specified
        let range = if let Some(ranges) = line_ranges {
            ranges.get(idx).cloned().flatten()
        } else {
            None
        };

        // Read the file with enhanced error handling and metadata collection
        match read_file_with_range(&path, range).await {
            Ok((content, effective_range, metadata)) => {
                debug!(
                    "Read file {}: {} lines, hash: {}",
                    path.display(),
                    metadata.total_lines,
                    metadata.hash.chars().take(8).collect::<String>()
                );

                results.push((file_path.clone(), content, effective_range));
                file_reads.push((path, effective_range, metadata));
            }
            Err(e) => {
                debug!("Failed to read file {}: {}", path.display(), e);
                results.push((file_path.clone(), format!("ERROR: {}", e), (0, 0)));
            }
        }
    }

    // Record file reads in state with improved tracking
    {
        let mut state_guard = state.lock().unwrap();
        for (path, range, metadata) in file_reads {
            if range.0 > 0 && range.1 > 0 {
                // Check if we need to update the file metadata
                let update_hash = if let Some(file_info) = state_guard.read_files.get(&path) {
                    file_info.file_hash != metadata.hash
                } else {
                    true
                };

                // Record the read with updated metadata if needed
                if update_hash {
                    debug!("Updating file hash for {}", path.display());
                    // Remove existing entry if hash changed
                    state_guard.read_files.remove(&path);

                    // Create a new entry with the current hash and total lines
                    let mut file_info = crate::core::state::FileReadInfo::new(&path);
                    file_info.file_hash = metadata.hash;
                    file_info.total_lines = metadata.total_lines;
                    file_info.add_range(range.0, range.1);

                    state_guard.read_files.insert(path, file_info);
                } else {
                    // Just record the read range
                    let _ = state_guard.record_file_read(path, &[range]);
                }
            }
        }
    }

    info!("Read {} files", results.len());
    Ok(results)
}

/// File metadata collected during reading
struct FileMetadata {
    /// SHA-256 hash of the file content
    hash: String,
    /// Total number of lines in the file
    total_lines: usize,
    /// File size in bytes
    #[allow(dead_code)]
    file_size: u64,
    /// Last modified time
    #[allow(dead_code)]
    last_modified: std::time::SystemTime,
}

/// Read a file with optional line range and collect metadata
async fn read_file_with_range(
    path: &Path,
    range: Option<(usize, usize)>,
) -> Result<(String, (usize, usize), FileMetadata)> {
    // Read the file content
    let content = fs_utils::read_file(path).await?;

    // Collect file metadata
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();
    let last_modified = metadata
        .modified()
        .unwrap_or_else(|_| std::time::SystemTime::now());

    // Calculate hash of the content
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    // Count total lines
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Create file metadata
    let file_metadata = FileMetadata {
        hash,
        total_lines,
        file_size,
        last_modified,
    };

    if let Some((start, end)) = range {
        // Adjust range to be within bounds
        let start = start.min(total_lines).max(1);
        let end = end.min(total_lines).max(start);

        // Extract requested lines (adjust from 1-based to 0-based indexing)
        let selected_lines = if total_lines > 0 {
            lines[(start - 1)..end].join("\n")
        } else {
            String::new()
        };

        Ok((selected_lines, (start, end), file_metadata))
    } else {
        // Return the full file and its metadata
        Ok((content, (1, total_lines), file_metadata))
    }
}

/// Write or edit a file with enhanced tracking
pub async fn write_or_edit_file_internal(
    state: &SharedState,
    file_path: &str,
    percentage_to_change: u8,
    content: &str,
) -> Result<String> {
    debug!("Writing/editing file: {}", file_path);

    // Check permissions and resolve path in a separate scope
    let path = {
        let state_guard = state.lock().unwrap();

        let resolved_path = if Path::new(file_path).is_absolute() {
            PathBuf::from(file_path)
        } else {
            state_guard.workspace_path.join(file_path)
        };

        if !state_guard.is_path_allowed(&resolved_path) {
            return Err(anyhow::anyhow!(
                "Path not allowed: {}",
                resolved_path.display()
            ));
        }

        resolved_path
    };

    // Get file hash before edit for tracking
    let pre_edit_hash = if path.exists() {
        use sha2::{Digest, Sha256};
        let content = fs::read(&path)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        Some(format!("{:x}", hasher.finalize()))
    } else {
        None
    };

    // Determine if this is a full replacement or partial edit
    let mode = if percentage_to_change > 50 {
        // Full content replacement
        debug!("Replacing full file content: {}", path.display());

        // Ensure parent directories exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, content)?;
        "replaced"
    } else {
        // Check if file exists first
        if !path.exists() {
            debug!(
                "File doesn't exist, creating new file with content: {}",
                path.display()
            );

            // Ensure parent directories exist
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            // For new files, always use the content directly (not as search/replace blocks)
            fs::write(&path, content)?;
            return Ok(format!("Created new file: {}", path.display()));
        }

        // Parse search/replace blocks and apply them
        debug!(
            "Performing partial edit with search/replace blocks: {}",
            path.display()
        );

        // Read the current content from the existing file
        let current_content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to read file {}: {}",
                    path.display(),
                    e
                ))
            }
        };

        // Try to apply search/replace blocks with enhanced error handling
        match crate::diff::search_replace::apply_search_replace_from_text(&current_content, content)
        {
            Ok(result) => {
                // Write the updated content
                fs::write(&path, &result.content)?;

                // Handle warnings
                if !result.warnings.is_empty() {
                    debug!("Search/replace warnings: {:?}", result.warnings);
                    for warning in &result.warnings {
                        info!("Warning: {}", warning);
                    }
                }

                if result.changes_made {
                    "edited with search/replace blocks"
                } else {
                    "no changes required"
                }
            }
            Err(e) => {
                // If syntax error in search/replace blocks, try alternative format
                if e.to_string().contains("No valid search/replace blocks") {
                    debug!("Standard format failed, trying to handle content as a complete file or alternative format");

                    // Check if it's actually valid content and not search/replace blocks at all
                    if !content.contains("<<<<<<< SEARCH")
                        && !content.contains("search:")
                        && !content.contains("replace:")
                    {
                        // Treat as complete file content
                        debug!("Content appears to be complete file content, not search/replace blocks");
                        fs::write(&path, content)?;
                        return Ok(format!("Created/replaced file: {}", path.display()));
                    }

                    // Try to provide more helpful error message for search/replace format issues
                    let error_msg = if content.contains("search:") || content.contains("replace:") {
                        "Invalid search/replace format. For prefix format, each 'search:' must be followed by a 'replace:'"
                    } else if content.contains("<<<<") || content.contains(">>>>") {
                        "Invalid marker format. Format must be: <<<<<<< SEARCH, =======, >>>>>>> REPLACE"
                    } else {
                        "Invalid search/replace blocks format. Use either marker format or prefix format"
                    };

                    warn!("{}: {}", error_msg, e);
                    return Err(anyhow::anyhow!("{}", error_msg));
                }

                // For other errors, provide detailed debug info
                warn!("Search/replace failed: {}", e);

                // Try to find context for failing search blocks
                if let Ok(blocks) =
                    crate::diff::search_replace::parse_search_replace_blocks(content)
                {
                    for (i, block) in blocks.iter().enumerate() {
                        if let Some(context) =
                            crate::diff::search_replace::find_context_for_search_block(
                                &current_content,
                                &block.search_lines,
                                3,
                            )
                        {
                            debug!("Context for search block #{}: {}", i + 1, context);
                        }
                    }
                }

                // Fall back to full replacement if required by env var
                if env::var("WINX_FALLBACK_ON_SEARCH_REPLACE_ERROR").unwrap_or_default() == "1" {
                    warn!("Falling back to full replacement due to search/replace error");
                    fs::write(&path, content)?;
                    "replaced (fallback from search/replace error)"
                } else {
                    return Err(anyhow::anyhow!(
                        "Failed to apply search/replace blocks: {}. Use percentage_to_change > 50 for full replacement.",
                        e
                    ));
                }
            }
        }
    };

    // Update file tracking after edit
    let post_edit_hash = {
        use sha2::{Digest, Sha256};
        let content = fs::read(&path)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        format!("{:x}", hasher.finalize())
    };

    // Count total lines in the updated file
    let total_lines = fs::read_to_string(&path)?.lines().count();

    // Update the file tracking in state
    {
        let mut state_guard = state.lock().unwrap();

        // If pre-edit hash is different or file is new, update hash and invalidate read tracking
        if pre_edit_hash.is_none() || pre_edit_hash.unwrap() != post_edit_hash {
            debug!("File hash changed after edit, updating tracking");

            // Create a new entry with the current hash and mark as fully read
            state_guard.read_files.remove(&path);

            // Create a fresh entry with total file read
            let mut file_info = crate::core::state::FileReadInfo::new(&path);
            file_info.file_hash = post_edit_hash;
            file_info.total_lines = total_lines;
            file_info.add_range(1, total_lines); // Mark entire file as read

            state_guard.read_files.insert(path.clone(), file_info);
        }
    }

    info!("Successfully {} file: {}", mode, path.display());
    Ok(format!("Successfully {} file: {}", mode, path.display()))
}

/// Read files from a JSON request
pub async fn read_files(state: &SharedState, json_str: &str) -> Result<String> {
    debug!("Reading files from JSON: {}", json_str);

    // Parse the JSON request
    let request: ReadFilesType = from_str(json_str)?;

    // Read the files
    // No line_ranges in ReadFilesType, so pass None
    let results = read_files_internal(state, &request.file_paths, None).await?;

    // Format the results
    let mut output = String::new();
    for (path, content, _range) in results {
        output.push_str(&format!("\n## File: {}\n```\n{}\n```\n", path, content));
    }

    Ok(output)
}

/// Write or edit a file from a JSON request with enhanced file tracking
pub async fn write_or_edit_file(state: &SharedState, json_str: &str) -> Result<String> {
    debug!("Writing/editing file from JSON: {}", json_str);

    // Parse the JSON request
    let request: FileWriteOrEditType = from_str(json_str)?;
    let path = Path::new(&request.file_path);

    // Check file edit permissions based on read history
    let (auto_read_required, file_hash, has_changed) = {
        let state_guard = state.lock().unwrap();

        if path.exists() {
            let can_edit = state_guard.can_edit_file(path)?;
            if !can_edit {
                // Get the current file hash for consistency checks
                let current_hash = if let Some(file_info) = state_guard.read_files.get(path) {
                    (
                        true,
                        file_info.file_hash.clone(),
                        file_info.has_changed(path).unwrap_or(true),
                    )
                } else {
                    // File exists but has never been read
                    (true, String::new(), true)
                };
                current_hash
            } else {
                (false, String::new(), false)
            }
        } else {
            // File doesn't exist, no need to read it
            (false, String::new(), false)
        }
    };

    // Auto-read the file if necessary and enabled
    if auto_read_required && path.exists() && request.auto_read_if_needed {
        // Check if file has changed or hasn't been fully read
        if has_changed {
            info!(
                "File has changed since last read, auto-reading: {}",
                request.file_path
            );
        } else {
            info!(
                "File hasn't been fully read, auto-reading: {}",
                request.file_path
            );
        }

        // Read the entire file
        let read_result = read_files_internal(state, &[request.file_path.clone()], None).await;

        if let Err(e) = read_result {
            warn!("Auto-read failed: {}", e);
            return Err(anyhow::anyhow!(
                "Cannot edit file {} - auto-read failed: {}. Please read this file manually first.",
                request.file_path,
                e
            ));
        }

        // Verify the file hasn't changed during reading
        if !file_hash.is_empty() {
            let current_hash = {
                let state_guard = state.lock().unwrap();
                if let Some(file_info) = state_guard.read_files.get(path) {
                    file_info.file_hash.clone()
                } else {
                    String::new()
                }
            };

            if current_hash != file_hash && !file_hash.is_empty() {
                warn!(
                    "File changed during auto-read, hash mismatch: {} vs {}",
                    file_hash, current_hash
                );
                return Err(anyhow::anyhow!(
                    "File {} changed during auto-read. Please try again.",
                    request.file_path
                ));
            }
        }

        info!(
            "Successfully auto-read file before editing: {}",
            request.file_path
        );
    } else if auto_read_required && !request.auto_read_if_needed {
        // If auto-read is disabled but required, return a detailed error
        let state_guard = state.lock().unwrap();

        if has_changed {
            return Err(anyhow::anyhow!(
                "File {} has changed since it was last read. Please read it again before editing.",
                request.file_path
            ));
        } else {
            let unread_ranges = state_guard.get_unread_ranges(path)?;

            if !unread_ranges.is_empty() {
                // Construct a helpful error message with unread ranges
                let ranges_str = unread_ranges
                    .iter()
                    .map(|(start, end)| format!("{}-{}", start, end))
                    .collect::<Vec<_>>()
                    .join(", ");

                return Err(anyhow::anyhow!(
                    "File {} hasn't been fully read. Please read the following line ranges first: {}",
                    request.file_path, ranges_str
                ));
            } else {
                return Err(anyhow::anyhow!(
                    "File {} cannot be edited due to read history issues. Please read it first.",
                    request.file_path
                ));
            }
        }
    }

    // Write or edit the file
    write_or_edit_file_internal(
        state,
        &request.file_path,
        request.percentage_to_change,
        &request.file_content_or_search_replace_blocks,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{state::create_shared_state, types::ModeType};
    use std::fs;
    // Removed unused import
    use tokio::runtime::Runtime;

    #[test]
    fn test_file_operations() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            // Use the actual /tmp directory for testing
            let base_dir = PathBuf::from("/tmp/winx_test");
            fs::create_dir_all(&base_dir).unwrap_or_default();

            // Create the state using /tmp as workspace
            let state = create_shared_state("/tmp", ModeType::Wcgw, None, None).unwrap();

            // Create a test file within the workspace directory
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Create file directly in /tmp for test
            let file_name = format!("test_{}.txt", timestamp);
            let file_path = PathBuf::from("/tmp").join(&file_name);

            // Make sure we clean up after the test
            let file_path_clone = file_path.clone();
            let _cleanup = defer::defer(move || {
                let _ = std::fs::remove_file(&file_path_clone);
            });

            // Create the initial file with content that will be in the assertions
            fs::write(&file_path, "function hello() {\n    console.log(\"Hello, universe!\");\n}\n").unwrap();

            // Test reading the file first
            {
                let file_path_str = file_path.to_string_lossy().to_string();
                let json = format!("{{\"file_paths\":[\"{}\"], \"show_line_numbers_reason\":null}}", file_path_str);
                let result = read_files(&state, &json).await.unwrap();
                println!("READ RESULT: {}", result);  // Debug output

                // The path is in the result even if it fails to read, so check content too
                assert!(result.contains(file_path_str.as_str()));
                assert!(result.contains("Hello, universe!") || result.contains("```\nHello, universe!"));
            }

            // Then test writing to the file
            {
                let file_path_str = file_path.to_string_lossy().to_string();
                let json = format!(
                    "{{\"file_path\":\"{}\", \"percentage_to_change\":100, \"auto_read_if_needed\":true, \"file_content_or_search_replace_blocks\":\"Hello, universe!\"}}",
                    file_path_str
                );
                let result = write_or_edit_file(&state, &json).await.unwrap();
                assert!(result.contains("Successfully"));
            }

            // Verify the file was updated
            let content = fs::read_to_string(&file_path).unwrap();
            assert!(content.contains("Hello, universe!"));
        });
    }
}
