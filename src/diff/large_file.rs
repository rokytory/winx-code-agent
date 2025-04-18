// Large file operations module for efficient handling of large files
// Parts of files content is cached to improve performance

use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use tracing::debug;

// Definindo os tipos que estavam faltando
#[derive(Debug, Clone)]
pub enum EditOperation {
    ReplaceLines {
        start_line: u64,
        end_line: u64,
        new_content: String,
    },
    // Adicione outros tipos de operações conforme necessário
}

#[derive(Debug, Clone)]
pub enum FileOperation {
    Edit(EditOperation),
    // Adicione outros tipos de operações conforme necessário
}

/// Process a large file with line-by-line operations
pub fn process_large_file<P: AsRef<Path>>(path: P, operations: &[EditOperation]) -> Result<()> {
    let path = path.as_ref();
    debug!("Processing large file: {}", path.display());

    // Check if the file exists
    if !path.exists() {
        return Err(anyhow::anyhow!("File not found: {}", path.display()));
    }

    // Create a temporary file
    let temp_path = path.with_extension("tmp");
    let mut temp_file = fs::File::create(&temp_path).context(format!(
        "Failed to create temp file: {}",
        temp_path.display()
    ))?;

    // Open the input file for reading and read all lines
    let file = fs::File::open(path).context(format!("Failed to open file: {}", path.display()))?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .collect::<std::io::Result<Vec<String>>>()
        .context("Failed to read lines from file")?;

    // Sort operations by starting line
    let mut sorted_ops = operations.to_vec();
    sorted_ops.sort_by_key(|op| match op {
        EditOperation::ReplaceLines { start_line, .. } => *start_line,
    });

    // Process line by line
    let mut line_index = 0;
    let mut next_op_index = 0;
    let total_lines = lines.len();

    while line_index < total_lines {
        let current_line = line_index as u64;

        // Check if we need to perform an operation at this line
        if next_op_index < sorted_ops.len() {
            match &sorted_ops[next_op_index] {
                EditOperation::ReplaceLines {
                    start_line,
                    end_line,
                    new_content,
                } => {
                    if current_line == *start_line {
                        // Write the replacement content instead of the original lines
                        temp_file
                            .write_all(new_content.as_bytes())
                            .context("Failed to write replacement content")?;

                        // Skip original lines that were replaced
                        line_index = (*end_line + 1) as usize;
                        next_op_index += 1;
                        continue;
                    }
                }
            }
        }

        // Write the current line if not skipped by an operation
        if line_index < total_lines {
            temp_file
                .write_all(lines[line_index].as_bytes())
                .context("Failed to write line")?;
            temp_file
                .write_all(b"\n")
                .context("Failed to write newline")?;
        }

        line_index += 1;
    }

    // Finalize any operations that weren't applied (e.g., appending to the end of the file)
    while next_op_index < sorted_ops.len() {
        match &sorted_ops[next_op_index] {
            EditOperation::ReplaceLines {
                start_line,
                new_content,
                ..
            } => {
                if *start_line >= line_index as u64 {
                    // This operation is beyond the end of the file, append it
                    temp_file
                        .write_all(new_content.as_bytes())
                        .context("Failed to write appended content")?;
                }
            }
        }
        next_op_index += 1;
    }

    // Flush and close the temporary file
    temp_file
        .flush()
        .context("Failed to flush temporary file")?;
    drop(temp_file);

    // Replace the original file with the temporary file
    fs::rename(&temp_path, path).context(format!(
        "Failed to replace original file with temp file: {}",
        path.display()
    ))?;

    debug!("Large file processed successfully: {}", path.display());
    Ok(())
}

/// Apply a set of operations to a large file
pub fn apply_operations<P: AsRef<Path>>(path: P, operations: &[FileOperation]) -> Result<()> {
    let path = path.as_ref();

    // Collect all edit operations
    let mut edit_operations = Vec::new();

    for op in operations {
        // Using let instead of if let since this pattern is irrefutable
        let FileOperation::Edit(edit_op) = op;
        edit_operations.push(edit_op.clone());
    }

    // Process the file with the collected edit operations
    process_large_file(path, &edit_operations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_process_large_file() {
        // Create a temporary file with some content
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();
        writeln!(file, "Line 4").unwrap();
        writeln!(file, "Line 5").unwrap();
        file.flush().unwrap();

        // Create operations to replace both Line 2 (index 1) and Line 3 (index 2)
        let operations = vec![EditOperation::ReplaceLines {
            start_line: 1, // Line 2 (0-indexed)
            end_line: 2,   // Line 3 (0-indexed)
            new_content: "New Line 2\nNew Line 3\n".to_string(),
        }];

        // Process the file
        let path = file.path().to_path_buf();
        process_large_file(&path, &operations).unwrap();

        // Read the processed file and verify the changes
        let content = fs::read_to_string(&path).unwrap();

        // Expected content: Line 1, then new lines 2-3, then Line 4-5
        let expected = "Line 1\nNew Line 2\nNew Line 3\nLine 4\nLine 5\n";

        println!("ACTUAL CONTENT: '{}'", content);
        println!("EXPECTED: '{}'", expected);

        assert_eq!(content, expected);
    }
}
