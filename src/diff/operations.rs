use anyhow::Result;
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
}
