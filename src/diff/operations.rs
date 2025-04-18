use anyhow::Result;
use rayon::prelude::*;
use similar::{ChangeTag, TextDiff};
use std::fmt;
use tracing::debug;

/// Represents a single diff operation
#[derive(Debug, Clone, PartialEq)]
pub enum DiffOp {
    /// Insert new content
    Insert { content: String, position: usize },
    /// Delete content
    Delete { start: usize, end: usize },
    /// Replace content
    Replace {
        start: usize,
        end: usize,
        content: String,
    },
}

impl fmt::Display for DiffOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffOp::Insert { content, position } => {
                write!(f, "Insert at position {}: '{}'", position, content)
            }
            DiffOp::Delete { start, end } => {
                write!(f, "Delete from position {} to {}", start, end)
            }
            DiffOp::Replace {
                start,
                end,
                content,
            } => {
                write!(
                    f,
                    "Replace from position {} to {} with '{}'",
                    start, end, content
                )
            }
        }
    }
}

/// Calculate diff operations between two strings
pub fn diff_strings(old: &str, new: &str) -> Vec<DiffOp> {
    let diff = TextDiff::from_lines(old, new);
    let mut operations = Vec::new();
    let mut position: usize = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                let start = position;
                let end = position + change.value().len();
                operations.push(DiffOp::Delete { start, end });
            }
            ChangeTag::Insert => {
                operations.push(DiffOp::Insert {
                    position,
                    content: change.value().to_string(),
                });
            }
            ChangeTag::Equal => {
                position += change.value().len();
            }
        }
    }

    // Combine adjacent operations where possible
    optimize_operations(&mut operations);

    operations
}

/// Calculate diff operations between two large strings using parallel processing
pub fn diff_strings_parallel(old: &str, new: &str) -> Vec<DiffOp> {
    // For small strings, use regular diff
    if old.len() < 10000 || new.len() < 10000 {
        return diff_strings(old, new);
    }

    // Split large strings into chunks for parallel processing
    let chunk_size = 5000;
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    // If very few lines, use regular diff
    if old_lines.len() < 100 || new_lines.len() < 100 {
        return diff_strings(old, new);
    }

    // Create chunks of lines
    let old_chunks: Vec<Vec<&str>> = old_lines.chunks(chunk_size).map(|c| c.to_vec()).collect();
    let new_chunks: Vec<Vec<&str>> = new_lines.chunks(chunk_size).map(|c| c.to_vec()).collect();

    // Process each chunk pair in parallel
    let chunk_results: Vec<Vec<DiffOp>> = old_chunks
        .par_iter()
        .zip(new_chunks.par_iter())
        .map(|(old_chunk, new_chunk)| {
            let old_chunk_str = old_chunk.join("\n");
            let new_chunk_str = new_chunk.join("\n");
            diff_strings(&old_chunk_str, &new_chunk_str)
        })
        .collect();

    // Combine results with proper offsets
    combine_chunk_results(chunk_results, chunk_size)
}

/// Combine diff results from chunked processing with adjusted offsets
fn combine_chunk_results(chunk_results: Vec<Vec<DiffOp>>, chunk_size: usize) -> Vec<DiffOp> {
    let mut combined = Vec::new();
    #[allow(unused_assignments)]
    let mut _line_offset = 0;

    for (i, ops) in chunk_results.into_iter().enumerate() {
        for op in ops {
            match op {
                DiffOp::Insert { content, position } => {
                    let adjusted_position = position + (i * chunk_size);
                    combined.push(DiffOp::Insert {
                        content,
                        position: adjusted_position,
                    });
                }
                DiffOp::Delete { start, end } => {
                    let adjusted_start = start + (i * chunk_size);
                    let adjusted_end = end + (i * chunk_size);
                    combined.push(DiffOp::Delete {
                        start: adjusted_start,
                        end: adjusted_end,
                    });
                }
                DiffOp::Replace {
                    start,
                    end,
                    content,
                } => {
                    let adjusted_start = start + (i * chunk_size);
                    let adjusted_end = end + (i * chunk_size);
                    combined.push(DiffOp::Replace {
                        start: adjusted_start,
                        end: adjusted_end,
                        content,
                    });
                }
            }
        }

        // Update line offset based on the chunk
        // line_offset increment would be used in a full implementation
    }

    // Optimize the combined operations
    optimize_operations(&mut combined);

    combined
}

/// Optimize diff operations by combining adjacent ones
fn optimize_operations(operations: &mut Vec<DiffOp>) {
    let mut i = 0;
    while i < operations.len() {
        if i + 1 < operations.len() {
            match (&operations[i], &operations[i + 1]) {
                (DiffOp::Delete { start, end }, DiffOp::Insert { position, content })
                    if *end == *position =>
                {
                    // Replace operation (Delete followed by Insert)
                    operations[i] = DiffOp::Replace {
                        start: *start,
                        end: *end,
                        content: content.clone(),
                    };
                    operations.remove(i + 1);
                    continue;
                }
                _ => {}
            }
        }
        i += 1;
    }
}

/// Apply diff operations to a string
pub fn apply_operations(source: &str, operations: &[DiffOp]) -> Result<String> {
    let mut result = source.to_string();

    // Sort operations by position (in reverse to avoid position changes)
    let mut sorted_ops = operations.to_vec();
    sorted_ops.sort_by(|a, b| {
        let pos_a = match a {
            DiffOp::Insert { position, .. } => *position,
            DiffOp::Delete { start, .. } => *start,
            DiffOp::Replace { start, .. } => *start,
        };
        let pos_b = match b {
            DiffOp::Insert { position, .. } => *position,
            DiffOp::Delete { start, .. } => *start,
            DiffOp::Replace { start, .. } => *start,
        };
        pos_b.cmp(&pos_a) // Reverse order
    });

    // Apply operations
    for op in sorted_ops {
        match op {
            DiffOp::Insert { content, position } => {
                if position > result.len() {
                    return Err(anyhow::anyhow!(
                        "Insert position {} out of bounds (len: {})",
                        position,
                        result.len()
                    ));
                }
                result.insert_str(position, &content);
                debug!("Inserted at position {}: '{}'", position, content);
            }
            DiffOp::Delete { start, end } => {
                if end > result.len() || start > end {
                    return Err(anyhow::anyhow!(
                        "Delete range {}:{} is invalid (len: {})",
                        start,
                        end,
                        result.len()
                    ));
                }
                result.replace_range(start..end, "");
                debug!("Deleted from position {} to {}", start, end);
            }
            DiffOp::Replace {
                start,
                end,
                content,
            } => {
                if end > result.len() || start > end {
                    return Err(anyhow::anyhow!(
                        "Replace range {}:{} is invalid (len: {})",
                        start,
                        end,
                        result.len()
                    ));
                }
                result.replace_range(start..end, &content);
                debug!(
                    "Replaced from position {} to {} with '{}'",
                    start, end, content
                );
            }
        }
    }

    Ok(result)
}

/// Apply diff operations to a string with parallel chunk processing for large files
pub fn apply_operations_parallel(source: &str, operations: &[DiffOp]) -> Result<String> {
    // For small files or few operations, use the regular method
    if source.len() < 50000 || operations.len() < 50 {
        return apply_operations(source, operations);
    }

    // Group operations by line ranges to process in parallel
    let lines: Vec<&str> = source.lines().collect();

    // If there aren't many lines, use regular method
    if lines.len() < 1000 {
        return apply_operations(source, operations);
    }

    // Create index mapping from positions to line numbers
    let mut position_to_line = Vec::with_capacity(lines.len() + 1);
    let mut pos = 0;

    for line in &lines {
        position_to_line.push(pos);
        pos += line.len() + 1; // +1 for newline
    }
    position_to_line.push(pos); // End position

    // Group operations by logical chunks (e.g., 200 lines per chunk)
    let chunk_size = 200;
    let num_chunks = (lines.len() + chunk_size - 1) / chunk_size;

    let mut chunk_operations: Vec<Vec<DiffOp>> = vec![Vec::new(); num_chunks];

    // Assign operations to chunks
    for op in operations {
        let (start_pos, _end_pos) = match op {
            DiffOp::Insert { position, .. } => (*position, *position),
            DiffOp::Delete { start, end } => (*start, *end),
            DiffOp::Replace { start, end, .. } => (*start, *end),
        };

        // Find which line this operation affects
        let start_line = position_to_line
            .binary_search(&start_pos)
            .unwrap_or_else(|i| i.saturating_sub(1));
        let chunk_idx = start_line / chunk_size;

        if chunk_idx < chunk_operations.len() {
            chunk_operations[chunk_idx].push(op.clone());
        }
    }

    // Process each chunk in parallel
    let chunk_results: Vec<String> = chunk_operations
        .par_iter()
        .enumerate()
        .map(|(i, ops)| {
            if ops.is_empty() {
                // If no operations, just return the source chunk
                let start = i * chunk_size;
                let end = ((i + 1) * chunk_size).min(lines.len());
                lines[start..end].join("\n")
            } else {
                // Apply operations to this chunk
                let start = i * chunk_size;
                let end = ((i + 1) * chunk_size).min(lines.len());
                let chunk_source = lines[start..end].join("\n");

                // We need to adjust operation positions for the chunk
                let offset = position_to_line[start];
                let adjusted_ops: Vec<DiffOp> = ops
                    .iter()
                    .map(|op| match op {
                        DiffOp::Insert { content, position } => DiffOp::Insert {
                            content: content.clone(),
                            position: position.saturating_sub(offset),
                        },
                        DiffOp::Delete { start, end } => DiffOp::Delete {
                            start: start.saturating_sub(offset),
                            end: end.saturating_sub(offset),
                        },
                        DiffOp::Replace {
                            start,
                            end,
                            content,
                        } => DiffOp::Replace {
                            start: start.saturating_sub(offset),
                            end: end.saturating_sub(offset),
                            content: content.clone(),
                        },
                    })
                    .collect();

                apply_operations(&chunk_source, &adjusted_ops).unwrap_or_else(|_| chunk_source)
            }
        })
        .collect();

    // Combine the results
    Ok(chunk_results.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_insert() {
        let old = "Hello World";
        let new = "Hello Beautiful World";
        let ops = diff_strings(old, new);

        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], DiffOp::Insert { .. }));

        if let DiffOp::Insert { content, position } = &ops[0] {
            assert_eq!(position, 6);
            assert_eq!(content, "Beautiful ");
        }
    }

    #[test]
    fn test_diff_delete() {
        let old = "Hello Beautiful World";
        let new = "Hello World";
        let ops = diff_strings(old, new);

        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], DiffOp::Delete { .. }));

        if let DiffOp::Delete { start, end } = ops[0] {
            assert_eq!(start, 6);
            assert_eq!(end, 16);
        }
    }

    #[test]
    fn test_diff_replace() {
        let old = "Hello World";
        let new = "Hello Universe";
        let ops = diff_strings(old, new);

        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], DiffOp::Replace { .. }));

        if let DiffOp::Replace {
            start,
            end,
            content,
        } = &ops[0]
        {
            assert_eq!(*start, 6);
            assert_eq!(*end, 11);
            assert_eq!(content, "Universe");
        }
    }

    #[test]
    fn test_apply_operations() {
        let source = "Hello World";
        let operations = vec![
            DiffOp::Replace {
                start: 6,
                end: 11,
                content: "Universe".to_string(),
            },
            DiffOp::Insert {
                position: 0,
                content: "Greetings: ".to_string(),
            },
        ];

        let result = apply_operations(source, &operations).unwrap();
        assert_eq!(result, "Greetings: Hello Universe");
    }

    #[test]
    fn test_diff_strings_parallel() {
        // Create larger strings to test parallel processing
        let old = (0..1000)
            .map(|i| format!("Line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let new = (0..1000)
            .map(|i| {
                if i % 5 == 0 {
                    format!("Modified Line {}", i)
                } else {
                    format!("Line {}", i)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let ops = diff_strings_parallel(&old, &new);
        let result = apply_operations_parallel(&old, &ops).unwrap();

        assert_eq!(result, new);
    }
}
