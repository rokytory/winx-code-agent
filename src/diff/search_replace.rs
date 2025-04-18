use anyhow::{anyhow, Result};
use regex::Regex;
use similar::{ChangeTag, TextDiff};
use std::fmt;
use tracing::debug;

/// Defines tolerance levels for matching
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToleranceLevel {
    /// Exact match
    Exact,
    /// Ignore trailing whitespace
    IgnoreTrailingWhitespace,
    /// Ignore leading whitespace (indentation)
    IgnoreLeadingWhitespace,
    /// Ignore all whitespace
    IgnoreAllWhitespace,
}

impl ToleranceLevel {
    /// Returns a line processing function for this tolerance level
    pub fn processor(&self) -> fn(&str) -> String {
        match self {
            ToleranceLevel::Exact => |s| s.to_string(),
            ToleranceLevel::IgnoreTrailingWhitespace => |s| s.trim_end().to_string(),
            ToleranceLevel::IgnoreLeadingWhitespace => |s| s.trim_start().to_string(),
            ToleranceLevel::IgnoreAllWhitespace => {
                |s| s.split_whitespace().collect::<Vec<_>>().join("")
            }
        }
    }

    /// Returns a descriptive message for this tolerance level
    pub fn message(&self) -> &'static str {
        match self {
            ToleranceLevel::Exact => "Exact match",
            ToleranceLevel::IgnoreTrailingWhitespace => "Ignoring trailing whitespace",
            ToleranceLevel::IgnoreLeadingWhitespace => "Ignoring indentation",
            ToleranceLevel::IgnoreAllWhitespace => "Ignoring all whitespace",
        }
    }

    /// Returns whether this tolerance level should generate a warning
    pub fn should_warn(&self) -> bool {
        match self {
            ToleranceLevel::Exact => false,
            _ => true,
        }
    }

    /// Returns the score for this tolerance level (lower is better)
    pub fn score(&self) -> f64 {
        match self {
            ToleranceLevel::Exact => 0.0,
            ToleranceLevel::IgnoreTrailingWhitespace => 1.0,
            ToleranceLevel::IgnoreLeadingWhitespace => 5.0,
            ToleranceLevel::IgnoreAllWhitespace => 10.0,
        }
    }
}

/// Represents a match with tolerance
#[derive(Debug, Clone)]
pub struct ToleranceMatch {
    /// The tolerance level applied
    pub level: ToleranceLevel,
    /// The match score (lower is better)
    pub score: f64,
    /// Line range matched (start, end)
    pub range: (usize, usize),
    /// Whether this should generate a warning
    pub has_warning: bool,
}

impl ToleranceMatch {
    /// Creates a new ToleranceMatch instance
    pub fn new(level: ToleranceLevel, range: (usize, usize)) -> Self {
        Self {
            level,
            score: level.score(),
            range,
            has_warning: level.should_warn(),
        }
    }

    /// Returns a warning message for this match, if applicable
    pub fn warning_message(&self) -> Option<String> {
        if self.has_warning {
            Some(format!(
                "Warning: {} for block at lines {}-{}",
                self.level.message(),
                self.range.0 + 1,
                self.range.1 + 1
            ))
        } else {
            None
        }
    }
}

/// Represents a search/replace block for file editing
#[derive(Debug, Clone)]
pub struct SearchReplaceBlock {
    /// Search lines
    pub search_lines: Vec<String>,
    /// Replacement lines
    pub replace_lines: Vec<String>,
}

impl fmt::Display for SearchReplaceBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "<<<<<<< SEARCH")?;
        for line in &self.search_lines {
            writeln!(f, "{}", line)?;
        }
        writeln!(f, "=======")?;
        for line in &self.replace_lines {
            writeln!(f, "{}", line)?;
        }
        writeln!(f, ">>>>>>> REPLACE")?;
        Ok(())
    }
}

/// Result of an edit operation
#[derive(Debug, Clone)]
pub struct EditResult {
    /// Edited content
    pub content: String,
    /// Warnings generated during editing
    pub warnings: Vec<String>,
    /// Whether changes were made
    pub changes_made: bool,
}

// Regex patterns for block markers
#[allow(unused_imports)]
use lazy_static::lazy_static;

lazy_static! {
    static ref SEARCH_MARKER: Regex = Regex::new(r"^<{7,}\s*SEARCH\s*$").unwrap();
    static ref DIVIDER_MARKER: Regex = Regex::new(r"^={7,}\s*$").unwrap();
    static ref REPLACE_MARKER: Regex = Regex::new(r"^>{7,}\s*REPLACE\s*$").unwrap();
}

/// Syntax error for search/replace blocks
#[derive(Debug)]
pub struct SearchReplaceSyntaxError {
    pub message: String,
    pub line_number: Option<usize>,
}

impl fmt::Display for SearchReplaceSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(line) = self.line_number {
            write!(f, "Syntax error at line {}: {}", line, self.message)
        } else {
            write!(f, "Syntax error: {}", self.message)
        }
    }
}

impl std::error::Error for SearchReplaceSyntaxError {}

/// Parses search/replace blocks from text, supporting both standard format (with markers)
/// and alternative format with "search:" and "replace:" prefixes
pub fn parse_search_replace_blocks(text: &str) -> Result<Vec<SearchReplaceBlock>> {
    // First try the standard marker format (<<<<<<< SEARCH)
    if let Ok(blocks) = parse_marker_format(text) {
        return Ok(blocks);
    }

    // If standard format fails, try alternative format with "search:" and "replace:" prefixes
    if let Ok(blocks) = parse_prefix_format(text) {
        return Ok(blocks);
    }

    // If both formats fail, return an error
    Err(anyhow!(SearchReplaceSyntaxError {
        message: "No valid search/replace blocks found in either standard or alternative format"
            .to_string(),
        line_number: None,
    }))
}

/// Parse blocks using the standard marker format (<<<<<<< SEARCH)
fn parse_marker_format(text: &str) -> Result<Vec<SearchReplaceBlock>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        if SEARCH_MARKER.is_match(lines[i]) {
            let line_num = i + 1;
            let mut search_block = Vec::new();
            i += 1;

            // Collect search lines until the divider
            while i < lines.len() && !DIVIDER_MARKER.is_match(lines[i]) {
                if SEARCH_MARKER.is_match(lines[i]) || REPLACE_MARKER.is_match(lines[i]) {
                    return Err(anyhow!(SearchReplaceSyntaxError {
                        message: format!("Unexpected marker in SEARCH block: {}", lines[i]),
                        line_number: Some(i + 1),
                    }));
                }
                search_block.push(lines[i].to_string());
                i += 1;
            }

            if i >= lines.len() {
                return Err(anyhow!(SearchReplaceSyntaxError {
                    message: format!("Unclosed SEARCH block - missing ======= marker"),
                    line_number: Some(line_num),
                }));
            }

            // Check if search block is empty
            if search_block.is_empty() {
                return Err(anyhow!(SearchReplaceSyntaxError {
                    message: format!("SEARCH block cannot be empty"),
                    line_number: Some(line_num),
                }));
            }

            i += 1;
            let mut replace_block = Vec::new();

            // Collect replacement lines until the REPLACE marker
            while i < lines.len() && !REPLACE_MARKER.is_match(lines[i]) {
                if SEARCH_MARKER.is_match(lines[i]) || DIVIDER_MARKER.is_match(lines[i]) {
                    return Err(anyhow!(SearchReplaceSyntaxError {
                        message: format!("Unexpected marker in REPLACE block: {}", lines[i]),
                        line_number: Some(i + 1),
                    }));
                }
                replace_block.push(lines[i].to_string());
                i += 1;
            }

            if i >= lines.len() {
                return Err(anyhow!(SearchReplaceSyntaxError {
                    message: format!("Unclosed block - missing REPLACE marker"),
                    line_number: Some(line_num),
                }));
            }

            // Add the complete block
            blocks.push(SearchReplaceBlock {
                search_lines: search_block,
                replace_lines: replace_block,
            });

            i += 1;
        } else {
            if REPLACE_MARKER.is_match(lines[i]) || DIVIDER_MARKER.is_match(lines[i]) {
                return Err(anyhow!(SearchReplaceSyntaxError {
                    message: format!("Unexpected marker outside a block: {}", lines[i]),
                    line_number: Some(i + 1),
                }));
            }
            i += 1;
        }
    }

    if blocks.is_empty() {
        return Err(anyhow!(SearchReplaceSyntaxError {
            message: "No valid search/replace blocks found in marker format".to_string(),
            line_number: None,
        }));
    }

    Ok(blocks)
}

/// Parse blocks using the alternative format with "search:" and "replace:" prefixes
fn parse_prefix_format(text: &str) -> Result<Vec<SearchReplaceBlock>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();

    // Variables to track the current block
    let mut in_search = false;
    let mut in_replace = false;
    let mut current_search_lines = Vec::new();
    let mut current_replace_lines = Vec::new();
    let mut line_num = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Check for start of search block
        if trimmed == "search:" {
            line_num = i + 1;
            // If we were already in a search/replace pair, finalize the block
            if !current_search_lines.is_empty() && in_replace {
                blocks.push(SearchReplaceBlock {
                    search_lines: current_search_lines,
                    replace_lines: current_replace_lines,
                });
                current_search_lines = Vec::new();
                current_replace_lines = Vec::new();
            } else if in_search {
                // Starting a new search without finishing the previous one
                return Err(anyhow!(SearchReplaceSyntaxError {
                    message: "Found 'search:' without matching 'replace:' for previous block"
                        .to_string(),
                    line_number: Some(i + 1),
                }));
            }

            in_search = true;
            in_replace = false;
            continue;
        }

        // Check for start of replace block
        if trimmed == "replace:" {
            // Can't have replace without search
            if !in_search || current_search_lines.is_empty() {
                return Err(anyhow!(SearchReplaceSyntaxError {
                    message: "Found 'replace:' without preceding 'search:' block".to_string(),
                    line_number: Some(i + 1),
                }));
            }

            in_search = false;
            in_replace = true;
            continue;
        }

        // Add to appropriate block
        if in_search {
            current_search_lines.push(line.to_string());
        } else if in_replace {
            current_replace_lines.push(line.to_string());
        }
    }

    // Don't forget the last block if there is one
    if in_search && !current_search_lines.is_empty() {
        // Missing replace block
        return Err(anyhow!(SearchReplaceSyntaxError {
            message: "Missing 'replace:' for last search block".to_string(),
            line_number: Some(line_num),
        }));
    } else if !current_search_lines.is_empty() && !current_replace_lines.is_empty() {
        blocks.push(SearchReplaceBlock {
            search_lines: current_search_lines,
            replace_lines: current_replace_lines,
        });
    }

    if blocks.is_empty() {
        return Err(anyhow!(SearchReplaceSyntaxError {
            message: "No valid search/replace blocks found in prefix format".to_string(),
            line_number: None,
        }));
    }

    Ok(blocks)
}

/// Finds matches for a search block in content with various tolerance levels
pub fn find_matches(
    content_lines: &[String],
    search_lines: &[String],
    start_line: usize,
) -> Vec<ToleranceMatch> {
    let mut matches = Vec::new();

    // List of tolerance levels to try, in order of preference
    let tolerance_levels = [
        ToleranceLevel::Exact,
        ToleranceLevel::IgnoreTrailingWhitespace,
        ToleranceLevel::IgnoreLeadingWhitespace,
        ToleranceLevel::IgnoreAllWhitespace,
    ];

    // Try each tolerance level
    for &level in &tolerance_levels {
        let processor = level.processor();

        // Apply processor to all search lines
        let processed_search_lines: Vec<String> =
            search_lines.iter().map(|line| processor(line)).collect();

        // Apply processor to all content lines
        let processed_content_lines: Vec<String> = content_lines
            .iter()
            .skip(start_line)
            .map(|line| processor(line))
            .collect();

        // Can't find matches if content or search is empty
        // or if content is smaller than search
        if processed_search_lines.is_empty()
            || processed_content_lines.is_empty()
            || processed_search_lines.len() > processed_content_lines.len()
        {
            continue;
        }

        // Look for exact matches with processed lines
        'outer: for i in 0..=processed_content_lines.len() - processed_search_lines.len() {
            for (j, search_line) in processed_search_lines.iter().enumerate() {
                if &processed_content_lines[i + j] != search_line {
                    continue 'outer;
                }
            }

            // Match found! Add the result
            matches.push(ToleranceMatch::new(
                level,
                (
                    start_line + i,
                    start_line + i + processed_search_lines.len() - 1,
                ),
            ));
        }

        // If we found matches with this tolerance level, no need to try higher levels
        if !matches.is_empty() {
            break;
        }
    }

    matches
}

/// Applies a list of search/replace blocks to content
pub fn apply_search_replace(content: &str, blocks: &[SearchReplaceBlock]) -> Result<EditResult> {
    let content_lines: Vec<String> = content.lines().map(ToString::to_string).collect();
    let mut result_lines = content_lines.clone();
    let mut warnings = Vec::new();
    let mut changes_made = false;

    // Process each block sequentially
    for (block_idx, block) in blocks.iter().enumerate() {
        // Start searching from the beginning of content for the first block
        // or from after the previous block for subsequent blocks
        let start_line = if block_idx == 0 {
            0
        } else {
            // Here we should use the position after the last applied block, but for simplicity,
            // we're just starting from the beginning for each block
            0
        };

        // Find matches for this search block
        let matches = find_matches(&result_lines, &block.search_lines, start_line);

        if matches.is_empty() {
            // Try to find the substring with the smallest edit distance
            return Err(anyhow!(
                "Could not find a match for search block #{}: {:?}",
                block_idx + 1,
                block.search_lines
            ));
        }

        // Use the best match (first in list, as they're ordered by preference)
        let best_match = &matches[0];

        // Add warnings if needed
        if let Some(warning) = best_match.warning_message() {
            warnings.push(warning);
        }

        // Apply the replacement
        let (start, end) = best_match.range;

        // Debug to check what we're replacing
        debug!(
            "Replacing block at lines {}-{} with new content of {} lines",
            start + 1,
            end + 1,
            block.replace_lines.len()
        );

        // When we have multiple matches, warn about ambiguity
        if matches.len() > 1 {
            let warning = format!(
                "Warning: Search block #{} matches multiple parts of the file. Using the first match.",
                block_idx + 1
            );
            warnings.push(warning);
        }

        // Fix indentation of the replacement block if needed
        let replace_lines = if best_match.level == ToleranceLevel::IgnoreLeadingWhitespace {
            adjust_indentation(
                &result_lines[start..end + 1],
                &block.search_lines,
                &block.replace_lines,
            )
        } else {
            block.replace_lines.clone()
        };

        // Replace the lines
        result_lines.splice(start..end + 1, replace_lines);
        changes_made = true;
    }

    // Join the lines back into a string
    let result_content = result_lines.join("\n");

    Ok(EditResult {
        content: result_content,
        warnings,
        changes_made,
    })
}

/// Adjusts indentation of replacement lines based on original lines
pub fn adjust_indentation(
    original_lines: &[String],
    search_lines: &[String],
    replace_lines: &[String],
) -> Vec<String> {
    // If there are no lines to adjust, return replacement lines as is
    if original_lines.is_empty() || search_lines.is_empty() || replace_lines.is_empty() {
        return replace_lines.to_vec();
    }

    // Function to get indentation of a line
    let get_indentation = |line: &str| -> String {
        let mut indent = String::new();
        for c in line.chars() {
            if c.is_whitespace() {
                indent.push(c);
            } else {
                break;
            }
        }
        indent
    };

    // Calculate indentation difference between original and search
    let mut indentation_diff = None;

    for (orig_line, search_line) in original_lines.iter().zip(search_lines.iter()) {
        if orig_line.trim().is_empty() || search_line.trim().is_empty() {
            continue;
        }

        let orig_indent = get_indentation(orig_line);
        let search_indent = get_indentation(search_line);

        // If we haven't set the indentation difference yet, set it now
        if indentation_diff.is_none() {
            indentation_diff = Some((orig_indent, search_indent));
        }
    }

    // If we couldn't determine indentation difference, return lines as is
    let (orig_indent, search_indent) = match indentation_diff {
        Some(diff) => diff,
        None => return replace_lines.to_vec(),
    };

    // Calculate target indentation
    let target_indent = if orig_indent.len() >= search_indent.len() {
        &orig_indent[..orig_indent.len() - search_indent.len()]
    } else {
        ""
    };

    // Apply indentation to replacement lines
    replace_lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                line.clone()
            } else {
                let line_indent = get_indentation(line);
                format!("{}{}", target_indent, &line[line_indent.len()..])
            }
        })
        .collect()
}

/// Applies search/replace blocks from text containing the blocks
pub fn apply_search_replace_from_text(
    content: &str,
    search_replace_text: &str,
) -> Result<EditResult> {
    let blocks = parse_search_replace_blocks(search_replace_text)?;
    apply_search_replace(content, &blocks)
}

/// Finds the line with the smallest edit distance compared to a search line
pub fn find_best_match_line(content_lines: &[String], search_line: &str) -> Option<(usize, f64)> {
    if content_lines.is_empty() {
        return None;
    }

    let mut best_score = f64::MAX;
    let mut best_idx = 0;

    // Preprocess the search line - remove leading/trailing whitespace
    let search_line_trimmed = search_line.trim();
    let search_line_nospace = search_line_trimmed.split_whitespace().collect::<String>();

    for (i, line) in content_lines.iter().enumerate() {
        // Try different matching strategies for better results

        // 1. Direct string comparison (fastest)
        if line.trim() == search_line_trimmed {
            return Some((i, 0.0)); // Perfect match
        }

        // 2. Case-insensitive comparison
        if line.trim().to_lowercase() == search_line_trimmed.to_lowercase() {
            return Some((i, 0.1)); // Almost perfect match
        }

        // 3. Whitespace-insensitive comparison
        let line_nospace = line.split_whitespace().collect::<String>();
        if line_nospace == search_line_nospace {
            return Some((i, 0.2)); // Good match ignoring whitespace
        }

        // 4. Only if the above approaches fail, fall back to edit distance
        let line_str = line.to_string();
        let search_str = search_line.to_string();
        let diff = TextDiff::from_chars(&line_str, &search_str);
        let ops_count = diff
            .iter_all_changes()
            .filter(|c| c.tag() != ChangeTag::Equal)
            .count();
        let max_len = line.len().max(search_line.len());
        let score = if max_len > 0 {
            ops_count as f64 / max_len as f64
        } else {
            0.0
        };

        if score < best_score {
            best_score = score;
            best_idx = i;
        }
    }

    Some((best_idx, best_score))
}

/// Calculate similarity score between two strings (0-1, higher is better)
pub fn similarity_score(s1: &str, s2: &str) -> f64 {
    let diff = TextDiff::from_chars(s1, s2);

    let mut unchanged = 0;
    #[allow(unused_assignments)]
    let mut _changed = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => unchanged += change.value().chars().count(),
            _ => _changed += change.value().chars().count(),
        }
    }

    let total = s1.chars().count() + s2.chars().count();
    if total == 0 {
        return 1.0; // Both strings empty = perfect match
    }

    (2.0 * unchanged as f64) / total as f64
}

/// Finds the context with lines that best match a search block
pub fn find_context_for_search_block(
    content: &str,
    search_block: &[String],
    context_lines: usize,
) -> Option<String> {
    if search_block.is_empty() {
        return None;
    }

    let content_lines: Vec<String> = content.lines().map(ToString::to_string).collect();
    if content_lines.is_empty() {
        return None;
    }

    // Try to find multiple potential matches
    let mut potential_matches = Vec::new();

    // Try exact block matching first
    'outer: for i in 0..=content_lines.len().saturating_sub(search_block.len()) {
        let mut match_score = 0.0;

        for (j, search_line) in search_block.iter().enumerate() {
            if i + j >= content_lines.len() {
                continue 'outer;
            }

            let content_line = &content_lines[i + j];

            // Direct equality check for exact matches
            if content_line.trim() == search_line.trim() {
                continue; // Perfect match for this line
            }

            // Calculate similarity score
            let line_score = similarity_score(content_line, search_line);

            // If any line is too dissimilar, this isn't a good block match
            if line_score < 0.7 {
                continue 'outer;
            }

            match_score += 1.0 - line_score; // Lower is better
        }

        // Record this potential match with its score
        match_score /= search_block.len() as f64;
        potential_matches.push((i, match_score));
    }

    // If we found potential whole-block matches, use the best one
    if !potential_matches.is_empty() {
        // Sort by score (lower is better)
        potential_matches
            .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let (best_idx, best_score) = potential_matches[0];

        // If the match is good enough
        if best_score < 0.3 {
            // Calculate line range for context
            let start_idx = best_idx.saturating_sub(context_lines);
            let end_idx = (best_idx + search_block.len() + context_lines).min(content_lines.len());

            // Extract context lines
            let context = content_lines[start_idx..end_idx].join("\n");
            return Some(context);
        }
    }

    // Fall back to finding the best match for just the first line
    let (best_idx, score) = match find_best_match_line(&content_lines, &search_block[0]) {
        Some(result) => result,
        None => return None,
    };

    // If the match is too poor, return None
    if score > 0.5 {
        return None;
    }

    // Calculate line range for context
    let start_idx = best_idx.saturating_sub(context_lines);
    let end_idx = (best_idx + search_block.len() + context_lines).min(content_lines.len());

    // Extract context lines
    let context = content_lines[start_idx..end_idx].join("\n");

    Some(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_search_replace_blocks() {
        let text = r#"<<<<<<< SEARCH
function hello() {
    console.log("Hello");
}
=======
function hello() {
    console.log("Hello, World!");
}
>>>>>>> REPLACE
<<<<<<< SEARCH
const x = 5;
=======
const x = 10;
>>>>>>> REPLACE"#;

        let blocks = parse_search_replace_blocks(text).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].search_lines.len(), 3);
        assert_eq!(blocks[0].replace_lines.len(), 3);
        assert_eq!(blocks[1].search_lines.len(), 1);
        assert_eq!(blocks[1].replace_lines.len(), 1);
    }

    #[test]
    fn test_parse_invalid_block() {
        let text = r#"<<<<<<< SEARCH
function hello() {
    console.log("Hello");
}
>>>>>>> REPLACE"#;

        let result = parse_search_replace_blocks(text);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_matches() {
        let content = vec![
            "function hello() {".to_string(),
            "    console.log(\"Hello\");".to_string(),
            "}".to_string(),
        ];

        let search = vec![
            "function hello() {".to_string(),
            "    console.log(\"Hello\");".to_string(),
            "}".to_string(),
        ];

        let matches = find_matches(&content, &search, 0);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].range, (0, 2));
        assert_eq!(matches[0].level, ToleranceLevel::Exact);
    }

    #[test]
    fn test_find_matches_with_tolerance() {
        let content = vec![
            "function hello() {".to_string(),
            "        console.log(\"Hello\");".to_string(),
            "}".to_string(),
        ];

        let search = vec![
            "function hello() {".to_string(),
            "    console.log(\"Hello\");".to_string(),
            "}".to_string(),
        ];

        let matches = find_matches(&content, &search, 0);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].range, (0, 2));
        assert_eq!(matches[0].level, ToleranceLevel::IgnoreLeadingWhitespace);
    }

    #[test]
    fn test_apply_search_replace() {
        let content = "function hello() {\n    console.log(\"Hello\");\n}\n\nconst x = 5;";

        let blocks = vec![
            SearchReplaceBlock {
                search_lines: vec![
                    "function hello() {".to_string(),
                    "    console.log(\"Hello\");".to_string(),
                    "}".to_string(),
                ],
                replace_lines: vec![
                    "function hello() {".to_string(),
                    "    console.log(\"Hello, World!\");".to_string(),
                    "}".to_string(),
                ],
            },
            SearchReplaceBlock {
                search_lines: vec!["const x = 5;".to_string()],
                replace_lines: vec!["const x = 10;".to_string()],
            },
        ];

        let result = apply_search_replace(content, &blocks).unwrap();
        assert!(result.changes_made);
        assert_eq!(
            result.content,
            "function hello() {\n    console.log(\"Hello, World!\");\n}\n\nconst x = 10;"
        );
    }

    #[test]
    fn test_adjust_indentation() {
        let original = vec![
            "    function hello() {".to_string(),
            "        console.log(\"Hello\");".to_string(),
            "    }".to_string(),
        ];

        let search = vec![
            "function hello() {".to_string(),
            "    console.log(\"Hello\");".to_string(),
            "}".to_string(),
        ];

        let replace = vec![
            "function hello() {".to_string(),
            "    console.log(\"Hello, World!\");".to_string(),
            "}".to_string(),
        ];

        let adjusted = adjust_indentation(&original, &search, &replace);
        assert_eq!(
            adjusted,
            vec![
                "    function hello() {".to_string(),
                "        console.log(\"Hello, World!\");".to_string(),
                "    }".to_string(),
            ]
        );
    }
}
