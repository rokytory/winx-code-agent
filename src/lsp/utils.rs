use std::path::{Path, PathBuf};

use lsp_types::{Position, Range};

use crate::error::WinxError;
use crate::lsp::symbol::{Symbol, SymbolPosition, SymbolRange};
use crate::WinxResult;

/// Convert a relative path to an absolute path
pub fn to_absolute_path(relative_path: impl AsRef<Path>, root_path: impl AsRef<Path>) -> PathBuf {
    root_path.as_ref().join(relative_path.as_ref())
}

/// Get the extension of a file path
pub fn get_file_extension(path: impl AsRef<Path>) -> Option<String> {
    path.as_ref()
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_string())
}

/// Read the content of a file
pub fn read_file_content(path: impl AsRef<Path>) -> WinxResult<String> {
    std::fs::read_to_string(path.as_ref()).map_err(|e| WinxError::IoError(e.to_string()))
}

/// Write content to a file
pub fn write_file_content(path: impl AsRef<Path>, content: &str) -> WinxResult<()> {
    let path = path.as_ref();

    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| WinxError::IoError(format!("Failed to create directory: {}", e)))?;
    }

    std::fs::write(path, content)
        .map_err(|e| WinxError::IoError(format!("Failed to write file: {}", e)))
}

/// Extract a range of content from a file
pub fn extract_range(content: &str, range: &Range) -> String {
    let lines: Vec<&str> = content.lines().collect();

    let start_line = range.start.line as usize;
    let start_char = range.start.character as usize;
    let end_line = range.end.line as usize;
    let end_char = range.end.character as usize;

    if start_line == end_line {
        // Single line range
        let line = lines.get(start_line).unwrap_or(&"");
        if start_char >= line.len() {
            return String::new();
        }
        let end_pos = end_char.min(line.len());
        return line[start_char..end_pos].to_string();
    }

    // Multi-line range
    let mut result = String::new();

    // First line (partial)
    if let Some(line) = lines.get(start_line) {
        if start_char < line.len() {
            result.push_str(&line[start_char..]);
        }
        result.push('\n');
    }

    // Middle lines (full)
    for line_idx in (start_line + 1)..end_line {
        if let Some(line) = lines.get(line_idx) {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Last line (partial)
    if let Some(line) = lines.get(end_line) {
        let end_pos = end_char.min(line.len());
        result.push_str(&line[..end_pos]);
    }

    result
}

/// Replace a range of content in a file
pub fn replace_range(content: &str, range: &Range, replacement: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();

    let start_line = range.start.line as usize;
    let start_char = range.start.character as usize;
    let end_line = range.end.line as usize;
    let end_char = range.end.character as usize;

    let mut result = String::new();

    // Add lines before the range
    for (_i, line) in lines.iter().enumerate().take(start_line) {
        result.push_str(line);
        result.push('\n');
    }

    // Add the start of the first affected line
    if let Some(line) = lines.get(start_line) {
        let prefix_len = start_char.min(line.len());
        result.push_str(&line[..prefix_len]);
    }

    // Add the replacement text
    result.push_str(replacement);

    // Add the end of the last affected line
    if let Some(line) = lines.get(end_line) {
        let suffix_start = end_char.min(line.len());
        result.push_str(&line[suffix_start..]);
        result.push('\n');
    }

    // Add lines after the range
    for (i, line) in lines.iter().enumerate().skip(end_line + 1) {
        result.push_str(line);
        if i < lines.len() - 1 {
            result.push('\n');
        }
    }

    result
}

/// Insert text at a specific position in the content
pub fn insert_at_position(content: &str, position: &Position, text: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    let line_idx = position.line as usize;
    let char_idx = position.character as usize;

    // Ensure we have enough lines
    while lines.len() <= line_idx {
        lines.push(String::new());
    }

    // Insert the text at the specified position
    let line = &mut lines[line_idx];
    if char_idx > line.len() {
        // Pad the line if necessary
        line.push_str(&" ".repeat(char_idx - line.len()));
    }
    line.insert_str(char_idx, text);

    lines.join("\n")
}

/// Format symbol information for display
pub fn format_symbol_info(
    symbol: &Symbol,
    include_children: bool,
    depth: usize,
    indent: &str,
) -> String {
    let kind_str = match symbol.kind {
        lsp_types::SymbolKind::FILE => "File",
        lsp_types::SymbolKind::MODULE => "Module",
        lsp_types::SymbolKind::NAMESPACE => "Namespace",
        lsp_types::SymbolKind::PACKAGE => "Package",
        lsp_types::SymbolKind::CLASS => "Class",
        lsp_types::SymbolKind::METHOD => "Method",
        lsp_types::SymbolKind::PROPERTY => "Property",
        lsp_types::SymbolKind::FIELD => "Field",
        lsp_types::SymbolKind::CONSTRUCTOR => "Constructor",
        lsp_types::SymbolKind::ENUM => "Enum",
        lsp_types::SymbolKind::INTERFACE => "Interface",
        lsp_types::SymbolKind::FUNCTION => "Function",
        lsp_types::SymbolKind::VARIABLE => "Variable",
        lsp_types::SymbolKind::CONSTANT => "Constant",
        lsp_types::SymbolKind::STRING => "String",
        lsp_types::SymbolKind::NUMBER => "Number",
        lsp_types::SymbolKind::BOOLEAN => "Boolean",
        lsp_types::SymbolKind::ARRAY => "Array",
        lsp_types::SymbolKind::OBJECT => "Object",
        lsp_types::SymbolKind::KEY => "Key",
        lsp_types::SymbolKind::NULL => "Null",
        lsp_types::SymbolKind::ENUM_MEMBER => "EnumMember",
        lsp_types::SymbolKind::STRUCT => "Struct",
        lsp_types::SymbolKind::EVENT => "Event",
        lsp_types::SymbolKind::OPERATOR => "Operator",
        lsp_types::SymbolKind::TYPE_PARAMETER => "TypeParameter",
        _ => "Unknown",
    };

    let mut result = format!(
        "{}{} {} ({}:{}:{})",
        indent,
        kind_str,
        symbol.name,
        symbol.location.relative_path,
        symbol.location.line + 1,
        symbol.location.column + 1
    );

    if include_children && depth > 0 && !symbol.children.is_empty() {
        result.push('\n');

        for child in &symbol.children {
            let child_info =
                format_symbol_info(child, include_children, depth - 1, &format!("{}  ", indent));
            result.push_str(&child_info);
            result.push('\n');
        }
    }

    result
}

/// Create a Position from line and character
pub fn create_position(line: u32, character: u32) -> Position {
    Position { line, character }
}

/// Create a Range from start and end positions
pub fn create_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
    Range {
        start: create_position(start_line, start_char),
        end: create_position(end_line, end_char),
    }
}

/// Convert a SymbolRange to an LSP Range
pub fn to_lsp_range(range: &SymbolRange) -> Range {
    Range {
        start: Position {
            line: range.start.line,
            character: range.start.column,
        },
        end: Position {
            line: range.end.line,
            character: range.end.column,
        },
    }
}

/// Convert an LSP Range to a SymbolRange
pub fn from_lsp_range(range: &Range) -> SymbolRange {
    SymbolRange {
        start: SymbolPosition {
            line: range.start.line,
            column: range.start.character,
        },
        end: SymbolPosition {
            line: range.end.line,
            column: range.end.character,
        },
    }
}
