use anyhow::{anyhow, Result};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

// Regular expressions for detecting various search/replace block formats
// The module supports multiple syntax formats to accommodate different user preferences
lazy_static::lazy_static! {
    // Standard/official format
    static ref SEARCH_MARKER: Regex = Regex::new(r"^<<<<<<+\s*SEARCH\s*$").unwrap();
    static ref DIVIDER_MARKER: Regex = Regex::new(r"^======*\s*$").unwrap();
    static ref REPLACE_MARKER: Regex = Regex::new(r"^>>>>>>+\s*REPLACE\s*$").unwrap();

    // Legacy/alternate format markers (to detect common mistakes)
    static ref ORIGINAL_MARKER: Regex = Regex::new(r"^<<<<<<+\s*ORIGINAL\s*$").unwrap();
    static ref UPDATED_MARKER: Regex = Regex::new(r"^>>>>>>+\s*UPDATED\s*$").unwrap();

    // Alternative formats
    static ref SIMPLE_SEARCH_START: Regex = Regex::new(r"^<<<+\s*$").unwrap();
    static ref SIMPLE_REPLACE_END: Regex = Regex::new(r"^>>>+\s*$").unwrap();

    // Markdown code blocks as search/replace (```...```)
    static ref MARKDOWN_CODE_BLOCK: Regex = Regex::new(r"^```+\s*$").unwrap();
}

/// Represents a single search/replace operation with patterns to find and replace
#[derive(Debug, Clone, PartialEq)]
pub struct SearchReplaceBlock {
    /// Lines of text to search for in the target file
    pub search_lines: Vec<String>,

    /// Lines of text to replace the matched content with
    pub replace_lines: Vec<String>,

    /// Optional index to specify which occurrence to replace (0-based)
    /// None means apply to all occurrences
    /// Some(0) means apply to first occurrence, Some(1) to second, etc.
    pub occurrence_index: Option<usize>,
}

/// Error types specific to search/replace operations
/// Provides detailed context about what went wrong during parsing or matching
#[derive(Debug, thiserror::Error)]
pub enum SearchReplaceError {
    /// Error indicating incorrect syntax in search/replace blocks
    #[error("Search/Replace syntax error: {0}")]
    SyntaxError(String),

    /// Error indicating the search pattern wasn't found in the content
    #[error("Search block not found: {0}")]
    MatchError(String),

    /// Error indicating multiple matches found for a pattern that should be unique
    #[error("Multiple matches found: {0}")]
    MultipleMatchesError(String),

    /// Error indicating an ambiguous match requiring clarification
    #[error("Ambiguous match: {0}")]
    AmbiguousMatchError(String),

    /// Error indicating insufficient context to identify a unique match
    #[error("Missing context for unique match: {0}")]
    MissingContextError(String),
}

/// Result of checking whether a search block uniquely identifies a section of content
/// Provides detailed information about match status and suggestions for improvement
#[derive(Debug)]
pub enum UniquenessCheck {
    /// Block is unique with match info
    Unique(MatchResult),
    /// Block has multiple matches
    MultipleMatches {
        count: usize,
        sample_matches: Vec<(usize, usize)>, // start and end line numbers
        block_content: String,
    },
    /// Block not found
    NotFound {
        closest_match: Option<(MatchResult, f64)>, // match and similarity score
    },
}

/// Tolerance levels for pattern matching, with increasing flexibility
/// These allow for successful matches despite minor formatting differences
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Applies the tolerance rules to process a line for comparison
    /// Different tolerance levels transform the string in different ways
    pub fn process_line(&self, line: &str) -> String {
        match *self {
            Self::Exact => line.to_string(),
            Self::IgnoreTrailingWhitespace => line.trim_end().to_string(),
            Self::IgnoreLeadingWhitespace => line.trim_start().to_string(),
            Self::IgnoreAllWhitespace => line.split_whitespace().collect::<Vec<_>>().join(""),
        }
    }

    /// Gets the severity level of this tolerance for warning purposes
    /// Higher tolerance levels generate more prominent warnings
    pub fn severity(&self) -> &'static str {
        match self {
            Self::Exact => "SILENT",
            Self::IgnoreTrailingWhitespace => "SILENT",
            Self::IgnoreLeadingWhitespace => "WARNING",
            Self::IgnoreAllWhitespace => "WARNING",
        }
    }

    /// Gets the score multiplier for this tolerance level (lower is better)
    /// Used to rank matches when multiple tolerance levels produce results
    pub fn score_multiplier(&self) -> f64 {
        match self {
            Self::Exact => 1.0,
            Self::IgnoreTrailingWhitespace => 1.5,
            Self::IgnoreLeadingWhitespace => 10.0,
            Self::IgnoreAllWhitespace => 50.0,
        }
    }

    /// Gets the user-facing warning message for this tolerance level
    /// Explains what adjustments were made to achieve the match
    pub fn warning_message(&self) -> Option<&'static str> {
        match self {
            Self::Exact => None,
            Self::IgnoreTrailingWhitespace => None,
            Self::IgnoreLeadingWhitespace => {
                Some("Warning: matching without considering indentation (leading spaces).")
            }
            Self::IgnoreAllWhitespace => {
                Some("Warning: matching after removing all spaces in lines.")
            }
        }
    }
}

impl Default for ToleranceLevel {
    fn default() -> Self {
        Self::Exact
    }
}

/// Tolerance hit for tracking which tolerances were applied
#[derive(Debug, Clone)]
pub struct ToleranceHit {
    pub level: ToleranceLevel,
    pub count: usize,
}

/// Match result with tolerance information
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub range: Range<usize>,
    pub tolerances: Vec<ToleranceHit>,
    pub score: f64,
}

/// Parses search/replace blocks from input text, supporting multiple syntax formats
///
/// This function attempts to parse the input using different formats in order:
/// 1. Standard format (`<<<<<<< SEARCH`, `=======`, `>>>>>>> REPLACE`)
/// 2. Simple format (`<<<`, `>>>`)
/// 3. Markdown code blocks format (pairs of "```" blocks)
///
/// It also supports occurrence index specification using a comment format:
/// `# occurrence: 0` (first match), `# occurrence: 1` (second match), etc.
///
/// If parsing fails, it provides helpful error messages with format examples.
pub fn parse_search_replace_blocks(
    input: &str,
) -> Result<Vec<SearchReplaceBlock>, SearchReplaceError> {
    let lines: Vec<&str> = input.lines().collect();

    // First, try to parse using the standard format
    let standard_blocks: Vec<SearchReplaceBlock> = parse_standard_format(&lines);
    if !standard_blocks.is_empty() {
        return Ok(standard_blocks);
    }

    // If standard format fails, try to parse using simple format (<<<...>>>)
    let simple_blocks = parse_simple_format(&lines);
    if !simple_blocks.is_empty() {
        return Ok(simple_blocks);
    }

    // If simple format fails too, try to parse using markdown code blocks
    let markdown_blocks = parse_markdown_format(&lines);
    if !markdown_blocks.is_empty() {
        return Ok(markdown_blocks);
    }

    // Check for common syntax errors in the input to provide targeted help
    let input_str = lines.join("\n");
    if input_str.contains("<<<<<<< ORIGINAL") || input_str.contains(">>>>>>> UPDATED") {
        // Detect specific common syntax mistakes (likely from git merge conflict style)
        let error_msg = if input_str.contains("<<<<<<< ORIGINAL") {
            "Syntax error: Found '<<<<<<< ORIGINAL' which is incorrect. Use '<<<<<<< SEARCH' instead."
        } else {
            "Syntax error: Found '>>>>>>> UPDATED' which is incorrect. Use '>>>>>>> REPLACE' instead."
        };

        Err(SearchReplaceError::SyntaxError(format!(
            "{}\n\nPlease use one of these formats:\n\n\
             1. Standard format (recommended):\n\
             <<<<<<< SEARCH\n\
             search content\n\
             =======\n\
             replace content\n\
             >>>>>>> REPLACE\n\n\
             For specific occurrence (optional):\n\
             # occurrence: 0  // first occurrence\n\
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
        )))
    } else {
        // Generic error message
        Err(SearchReplaceError::SyntaxError(
            "No valid search replace blocks found. Please use one of these formats:\n\n\
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
             ```\n"
                .to_string(),
        ))
    }
}

/// Verifies if a search block uniquely identifies a section in the given content
///
/// This function checks whether a search pattern matches exactly one location
/// in the content, and provides helpful diagnostics when uniqueness issues arise.
pub fn verify_search_block_uniqueness(content: &str, search_block: &str) -> UniquenessCheck {
    let content_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let search_lines: Vec<String> = search_block.lines().map(|s| s.to_string()).collect();

    // Don't check empty blocks
    if search_lines.is_empty() {
        return UniquenessCheck::NotFound {
            closest_match: None,
        };
    }

    // Try with different tolerance levels
    let tolerance_levels = [
        ToleranceLevel::Exact,
        ToleranceLevel::IgnoreTrailingWhitespace,
        ToleranceLevel::IgnoreLeadingWhitespace,
        ToleranceLevel::IgnoreAllWhitespace,
    ];

    let matches = find_matches(&content_lines, &search_lines, &tolerance_levels);

    match matches.len() {
        0 => {
            // No exact matches, try to find the closest one for helpful error message
            let closest = find_closest_match(&content_lines, &search_lines);
            UniquenessCheck::NotFound {
                closest_match: closest,
            }
        }
        1 => UniquenessCheck::Unique(matches[0].clone()),
        _ => {
            // Multiple matches, provide context with improved guidance
            let mut sample_matches = Vec::new();
            for (idx, m) in matches.iter().take(4).enumerate() {
                sample_matches.push((m.range.start, m.range.end));

                log::debug!(
                    "Match {}: lines {}-{} with tolerance {:?}",
                    idx + 1,
                    m.range.start + 1,
                    m.range.end,
                    m.tolerances.iter().map(|t| &t.level).collect::<Vec<_>>()
                );
            }

            // Provide more helpful guidance based on search block length
            let block_length = search_lines.len();
            let suggestion = if block_length < 3 {
                "\n\nSUGGESTION: This search block is very short (less than 3 lines), which increases the chance of multiple matches. Try including more unique context."
            } else if block_length < 5 {
                "\n\nSUGGESTION: Include more unique lines around your block to make the match unique."
            } else {
                "\n\nSUGGESTION: Your block has good length but still matches multiple locations. Check for repeated patterns in the file."
            };

            UniquenessCheck::MultipleMatches {
                count: matches.len(),
                sample_matches,
                block_content: search_block.to_string() + suggestion,
            }
        }
    }
}

/// Finds the closest approximate match for a search block when exact matching fails
///
/// When a search block isn't found exactly, this function attempts to find similar
/// content using various fuzzy matching strategies. This helps provide useful
/// suggestions to the user about why their search pattern failed.
///
/// Returns a tuple with the match result and similarity score if a close match is found
fn find_closest_match(
    content_lines: &[String],
    search_lines: &[String],
) -> Option<(MatchResult, f64)> {
    if content_lines.is_empty() || search_lines.is_empty() {
        return None;
    }

    let mut best_match = None;
    let mut best_score = 0.0;

    // Skip empty search lines
    let search_lines_filtered = search_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();

    if search_lines_filtered.is_empty() {
        return None;
    }

    // Try different matching strategies

    // Strategy 1: Line-by-line exact match with tolerance for whitespace
    let min_block_size = 3.min(search_lines_filtered.len());

    for i in 0..content_lines.len().saturating_sub(min_block_size) {
        let mut matching_lines = 0;
        let max_lines = (i + search_lines_filtered.len()).min(content_lines.len());

        for (search_idx, content_idx) in (0..search_lines_filtered.len()).zip(i..max_lines) {
            if content_lines[content_idx].trim() == search_lines_filtered[search_idx].trim() {
                matching_lines += 1;
            }
        }

        let score = matching_lines as f64 / search_lines_filtered.len() as f64;
        if score > best_score && score > 0.4 {
            // At least 40% match
            best_score = score;
            best_match = Some((
                MatchResult {
                    range: i..i + search_lines_filtered.len().min(content_lines.len() - i),
                    tolerances: vec![ToleranceHit {
                        level: ToleranceLevel::IgnoreLeadingWhitespace,
                        count: matching_lines,
                    }],
                    score: 1.0 - score, // Lower is better
                },
                score,
            ));
        }
    }

    // Strategy 2: Word-based similarity for more fuzzy matching
    if best_score < 0.6 {
        for start in 0..content_lines
            .len()
            .saturating_sub(search_lines_filtered.len())
            + 1
        {
            let mut similarity = 0.0;

            // Calculate similarity for the sequence
            for (i, search_line) in search_lines_filtered.iter().enumerate() {
                if start + i < content_lines.len() {
                    let search_words: Vec<&str> = search_line.split_whitespace().collect();
                    let content_words: Vec<&str> =
                        content_lines[start + i].split_whitespace().collect();

                    // Word-based similarity score
                    let mut word_matches = 0;
                    for search_word in &search_words {
                        if content_words.contains(search_word) {
                            word_matches += 1;
                        }
                    }

                    if !search_words.is_empty() {
                        similarity += word_matches as f64 / search_words.len() as f64;
                    }
                }
            }

            // Average similarity across all lines
            similarity /= search_lines_filtered.len() as f64;

            if similarity > best_score {
                best_score = similarity;

                best_match = Some((
                    MatchResult {
                        range: start
                            ..start + search_lines_filtered.len().min(content_lines.len() - start),
                        tolerances: vec![ToleranceHit {
                            level: ToleranceLevel::IgnoreAllWhitespace,
                            count: (similarity * search_lines_filtered.len() as f64) as usize,
                        }],
                        score: 1.0 - similarity,
                    },
                    similarity,
                ));
            }
        }
    }

    best_match
}

/// Parses blocks using the standard format with explicit SEARCH/REPLACE markers
///
/// Standard format example:
/// ```text
/// <<<<<<< SEARCH
/// search content
/// =======
/// replace content
/// >>>>>>> REPLACE
/// ```
///
/// With occurrence index:
/// ```text
/// # occurrence: 0
/// <<<<<<< SEARCH
/// search content
/// =======
/// replace content
/// >>>>>>> REPLACE
/// ```
fn parse_standard_format(lines: &[&str]) -> Vec<SearchReplaceBlock> {
    let mut blocks: Vec<SearchReplaceBlock> = Vec::new();
    let mut i = 0;

    // Check for common syntax errors before parsing
    for (idx, line) in lines.iter().enumerate() {
        if ORIGINAL_MARKER.is_match(line) {
            log::warn!(
                "Found '<<<<<<< ORIGINAL' at line {}. Use '<<<<<<< SEARCH' instead.",
                idx + 1
            );
            // Don't immediately return, continue parsing to try to find valid blocks
        }
        if UPDATED_MARKER.is_match(line) {
            log::warn!(
                "Found '>>>>>>> UPDATED' at line {}. Use '>>>>>>> REPLACE' instead.",
                idx + 1
            );
            // Don't immediately return, continue parsing to try to find valid blocks
        }
    }

    while i < lines.len() {
        // Check for occurrence index comment
        let mut occurrence_index = None;
        if i < lines.len() && lines[i].trim().starts_with("# occurrence:") {
            let parts: Vec<&str> = lines[i].split(':').collect();
            if parts.len() == 2 {
                if let Ok(index) = parts[1].trim().parse::<usize>() {
                    occurrence_index = Some(index);
                }
            }
            i += 1;
        }

        if i < lines.len() && SEARCH_MARKER.is_match(lines[i]) {
            let mut search_block = Vec::new();
            i += 1;

            // Collect search lines until divider
            while i < lines.len() && !DIVIDER_MARKER.is_match(lines[i]) {
                if SEARCH_MARKER.is_match(lines[i]) || REPLACE_MARKER.is_match(lines[i]) {
                    // Invalid block, skip
                    log::warn!("Invalid syntax: Nested markers found");
                    return Vec::new();
                }
                search_block.push(lines[i].to_string());
                i += 1;
            }

            if i >= lines.len() {
                log::warn!("Invalid syntax: Unclosed block, missing '=======' divider");
                // Unclosed block, skip
                return Vec::new();
            }

            i += 1; // Skip the divider

            // Collect replace lines until replace marker
            let mut replace_block = Vec::new();
            while i < lines.len() && !REPLACE_MARKER.is_match(lines[i]) {
                if SEARCH_MARKER.is_match(lines[i]) || DIVIDER_MARKER.is_match(lines[i]) {
                    // Invalid block, skip
                    return Vec::new();
                }
                replace_block.push(lines[i].to_string());
                i += 1;
            }

            if i >= lines.len() {
                // Unclosed block, skip
                return Vec::new();
            }

            i += 1; // Skip the replace marker

            // Block is valid, add it to the result
            blocks.push(SearchReplaceBlock {
                search_lines: search_block,
                replace_lines: replace_block,
                occurrence_index,
            });
        } else if lines[i].trim().starts_with("#") {
            // Skip other comments
            i += 1;
        } else if REPLACE_MARKER.is_match(lines[i]) || DIVIDER_MARKER.is_match(lines[i]) {
            // Stray marker, invalid format
            return Vec::new();
        } else {
            i += 1;
        }
    }

    blocks
}

/// Parses blocks using the simplified format with minimal markers
///
/// Simple format example:
/// ```text
/// <<<
/// search content
/// >>>
/// <<<
/// replace content
/// >>>
/// ```
fn parse_simple_format(lines: &[&str]) -> Vec<SearchReplaceBlock> {
    // Find all block markers
    let mut markers = Vec::new();
    for (i, &line) in lines.iter().enumerate() {
        if SIMPLE_SEARCH_START.is_match(line) || SIMPLE_REPLACE_END.is_match(line) {
            markers.push((i, line));
        }
    }

    // Count & validate markers
    let markers_count = markers.len();
    if markers_count % 2 != 0 {
        log::warn!(
            "Invalid simple format: odd number of markers ({})",
            markers_count
        );
        return Vec::new();
    }

    // Build list of pairs of blocks
    let mut blocks = Vec::new();
    let mut i = 0;

    // Validate marker pairing. Expected pattern is:
    // open markers at even indices (0, 2, 4, ...)
    // close markers at odd indices (1, 3, 5, ...)
    let mut is_valid = true;
    for (idx, (_, line)) in markers.iter().enumerate() {
        let is_expected_open = idx % 2 == 0;
        let is_open = SIMPLE_SEARCH_START.is_match(line);
        let is_close = SIMPLE_REPLACE_END.is_match(line);

        if (is_expected_open && !is_open) || (!is_expected_open && !is_close) {
            log::warn!(
                "Invalid marker pattern at index {}: expected {}, found '{}'",
                idx,
                if is_expected_open {
                    "open marker"
                } else {
                    "close marker"
                },
                line
            );
            is_valid = false;
            break;
        }
    }

    if !is_valid {
        return Vec::new();
    }

    // Go through markers in pairs of open/close/open/close
    while i + 3 < markers.len() {
        let (search_start_idx, search_start_line) = markers[i];
        let (search_end_idx, search_end_line) = markers[i + 1];
        let (replace_start_idx, replace_start_line) = markers[i + 2];
        let (replace_end_idx, replace_end_line) = markers[i + 3];

        // Validate markers are correct types
        if SIMPLE_SEARCH_START.is_match(search_start_line)
            && SIMPLE_REPLACE_END.is_match(search_end_line)
            && SIMPLE_SEARCH_START.is_match(replace_start_line)
            && SIMPLE_REPLACE_END.is_match(replace_end_line)
        {
            // Extract content
            let search_content = lines[(search_start_idx + 1)..search_end_idx]
                .iter()
                .map(|&s| s.to_string())
                .collect::<Vec<_>>();
            let replace_content = lines[(replace_start_idx + 1)..replace_end_idx]
                .iter()
                .map(|&s| s.to_string())
                .collect::<Vec<_>>();

            // Add block
            blocks.push(SearchReplaceBlock {
                search_lines: search_content,
                replace_lines: replace_content,
                occurrence_index: None, // default to all occurrences
            });
        }

        i += 4; // Move to next set of 4 markers
    }

    blocks
}

/// Parses blocks using markdown code blocks for search/replace pairs
///
/// Markdown format example:
/// ```
/// ```
/// search content
/// ```
///
/// ```
/// replace content
/// ```
/// ```
fn parse_markdown_format(lines: &[&str]) -> Vec<SearchReplaceBlock> {
    let mut blocks: Vec<SearchReplaceBlock> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Look for opening markdown code block ```
        if MARKDOWN_CODE_BLOCK.is_match(lines[i]) {
            let search_start = i + 1;
            // We've already updated i above, no need to increment again
            // Find the closing markdown code block ```
            let mut search_end = search_start;
            while search_end < lines.len() && !MARKDOWN_CODE_BLOCK.is_match(lines[search_end]) {
                search_end += 1;
            }

            // If we didn't find closing marker, or search block is empty, this format is invalid
            if search_end >= lines.len() || search_start >= search_end {
                return Vec::new();
            }

            // Extract search content
            let search_block: Vec<String> = lines[search_start..search_end]
                .iter()
                .map(|&s| s.to_string())
                .collect();

            // Move past the closing marker
            i = search_end + 1;

            // Skip any empty lines between code blocks
            while i < lines.len() && lines[i].trim().is_empty() {
                i += 1;
            }

            // Look for next opening markdown code block
            if i >= lines.len() || !MARKDOWN_CODE_BLOCK.is_match(lines[i]) {
                return Vec::new(); // Need another ``` for the replace block
            }

            let replace_start = i + 1;
            // We already updated i above, no need to increment again
            // Find the next closing markdown code block
            let mut replace_end = replace_start;
            while replace_end < lines.len() && !MARKDOWN_CODE_BLOCK.is_match(lines[replace_end]) {
                replace_end += 1;
            }

            // If we didn't find closing marker, or replace block is empty, this format is invalid
            if replace_end >= lines.len() || replace_start >= replace_end {
                return Vec::new();
            }

            // Extract replace content
            let replace_block: Vec<String> = lines[replace_start..replace_end]
                .iter()
                .map(|&s| s.to_string())
                .collect();

            // Move past the closing marker
            i = replace_end + 1;

            // Add the block
            blocks.push(SearchReplaceBlock {
                search_lines: search_block,
                replace_lines: replace_block,
                occurrence_index: None, // default to all occurrences
            });
        } else {
            i += 1; // Skip non-marker lines
        }
    }

    blocks
}

/// Determines if content is a search/replace block based on markers and structure
///
/// This heuristic function analyzes the content to detect whether it's intended
/// as a search/replace operation rather than a complete file replacement.
pub fn is_search_replace_content(content: &str, percentage_to_change: i32) -> bool {
    // Check for any search marker in the first few lines
    let lines: Vec<&str> = content.lines().take(10).collect();
    for line in &lines {
        if SEARCH_MARKER.is_match(line)
            || SIMPLE_SEARCH_START.is_match(line)
            || MARKDOWN_CODE_BLOCK.is_match(line)
        {
            return true;
        }
    }

    // If percentage is low, look deeper at content
    if percentage_to_change <= 50 {
        // Count markdown code blocks - if we have at least two pairs, it's likely a search/replace
        let code_block_count = content
            .lines()
            .filter(|line| MARKDOWN_CODE_BLOCK.is_match(line))
            .count();

        if code_block_count >= 4 && code_block_count % 2 == 0 {
            return true;
        }

        // Check for any marker
        for line in content.lines() {
            if SEARCH_MARKER.is_match(line)
                || DIVIDER_MARKER.is_match(line)
                || REPLACE_MARKER.is_match(line)
                || SIMPLE_SEARCH_START.is_match(line)
                || SIMPLE_REPLACE_END.is_match(line)
            {
                return true;
            }
        }
    }

    false
}

/// Finds all possible matches for a search block using multiple tolerance levels
///
/// This function tries to match the search block against the content using
/// increasingly flexible tolerance levels, collecting all matches found.
/// This helps handle formatting inconsistencies while still finding the right content.
fn find_matches(
    content_lines: &[String],
    search_lines: &[String],
    tolerance_levels: &[ToleranceLevel],
) -> Vec<MatchResult> {
    let mut matches = Vec::new();
    // Track positions already found to avoid duplication
    let mut found_positions = HashSet::new();

    // Log search context for debugging
    if !search_lines.is_empty() {
        let search_preview = if search_lines.len() > 3 {
            format!(
                "\n{}\n...\n{}",
                search_lines[0],
                search_lines[search_lines.len() - 1]
            )
        } else {
            search_lines.join("\n")
        };
        log::debug!(
            "Searching for block with {} lines: {}",
            search_lines.len(),
            search_preview
        );
    }

    // First, check if we're searching for an entire file
    let search_text = search_lines.join("\n");
    let content_text = content_lines.join("\n");

    // If the search block is the entire file, treat it as a special case
    if search_text == content_text {
        log::debug!("Exact full file match detected");
        return vec![MatchResult {
            range: 0..content_lines.len(),
            tolerances: vec![ToleranceHit {
                level: ToleranceLevel::Exact,
                count: 0,
            }],
            score: 0.0,
        }];
    }

    // Try each tolerance level
    for tolerance in tolerance_levels {
        let processed_search_lines: Vec<String> = search_lines
            .iter()
            .map(|line| tolerance.process_line(line))
            .collect();

        let processed_content_lines: Vec<String> = content_lines
            .iter()
            .map(|line| tolerance.process_line(line))
            .collect();

        // Build a map for faster lookup
        let mut search_positions = HashMap::new();
        for (i, content_line) in processed_content_lines.iter().enumerate() {
            search_positions
                .entry(content_line.clone())
                .or_insert_with(Vec::new)
                .push(i);
        }

        // Log stats for debugging
        log::debug!(
            "Search with tolerance {:?}: {} unique line patterns in content",
            tolerance,
            search_positions.len()
        );

        // Skip if the first line isn't found at all - quick fail
        if !search_positions.contains_key(&processed_search_lines[0]) {
            log::debug!(
                "First line of search block not found with tolerance {:?}",
                tolerance
            );
            continue;
        }

        // Find all potential matches for the first line
        if let Some(first_line_positions) = search_positions.get(&processed_search_lines[0]) {
            log::debug!(
                "Found {} potential starting positions for first line",
                first_line_positions.len()
            );
            for &pos in first_line_positions {
                // Check if we've already found a match at this position
                if found_positions.contains(&pos) {
                    log::debug!("Skipping already found position at line {}", pos);
                    continue;
                }

                if pos + search_lines.len() > content_lines.len() {
                    continue;
                }

                // Check if all lines match
                let mut all_match = true;
                let mut tolerance_count = 0;

                for (i, search_line) in processed_search_lines.iter().enumerate() {
                    let content_pos = pos + i;
                    if content_pos >= processed_content_lines.len()
                        || &processed_content_lines[content_pos] != search_line
                    {
                        all_match = false;
                        break;
                    }

                    // Count actual differences between original lines
                    if content_lines[content_pos] != search_lines[i] {
                        tolerance_count += 1;
                    }
                }

                if all_match {
                    // Add the position to the set of found positions
                    found_positions.insert(pos);

                    matches.push(MatchResult {
                        range: pos..(pos + search_lines.len()),
                        tolerances: vec![ToleranceHit {
                            level: tolerance.clone(),
                            count: tolerance_count,
                        }],
                        score: tolerance.score_multiplier() * tolerance_count as f64,
                    });

                    // If this is the complete file content, we don't need more matches
                    if search_lines.len() == content_lines.len() && pos == 0 {
                        log::debug!("Found full file match - no need to continue searching");
                        return matches;
                    }
                }
            }
        }
    }

    // Sort matches by score (lower is better)
    matches.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());

    matches
}

// The find_closest_match function was defined previously

/// Fix indentation in replace lines based on matched content
fn fix_indentation(
    matched_lines: &[String],
    search_lines: &[String],
    replace_lines: &[String],
) -> Vec<String> {
    if matched_lines.is_empty() || search_lines.is_empty() || replace_lines.is_empty() {
        return replace_lines.to_vec();
    }

    // Extract indentation from lines
    let get_indent = |line: &str| -> String {
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

    // Get indentation from non-empty lines
    let matched_indents: Vec<String> = matched_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| get_indent(line))
        .collect();

    let search_indents: Vec<String> = search_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| get_indent(line))
        .collect();

    if matched_indents.is_empty()
        || search_indents.is_empty()
        || matched_indents.len() != search_indents.len()
    {
        return replace_lines.to_vec();
    }

    // Calculate indentation differences
    let indent_diffs: Vec<isize> = matched_indents
        .iter()
        .zip(search_indents.iter())
        .map(|(matched, search)| search.len() as isize - matched.len() as isize)
        .collect();

    // Check if all differences are the same
    if !indent_diffs.iter().all(|&diff| diff == indent_diffs[0]) {
        return replace_lines.to_vec();
    }

    let diff = indent_diffs[0];
    if diff == 0 {
        return replace_lines.to_vec();
    }

    // Apply indentation adjustment
    replace_lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                line.clone()
            } else {
                let indent = get_indent(line);
                if diff < 0 {
                    // Add indentation
                    format!("{}{}", " ".repeat((-diff) as usize), line)
                } else if diff as usize <= indent.len() {
                    // Remove indentation
                    line.chars().skip(diff as usize).collect()
                } else {
                    // Cannot remove more indentation than exists
                    line.trim_start().to_string()
                }
            }
        })
        .collect()
}

/// Remove leading and trailing blank lines
// Remove leading and trailing blank lines (not currently used but kept for future use)
#[allow(dead_code)]
fn remove_empty_boundary_lines(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    let mut start = 0;
    let mut end = lines.len() - 1;

    while start <= end && lines[start].trim().is_empty() {
        start += 1;
    }

    while end >= start && lines[end].trim().is_empty() {
        end -= 1;
    }

    if start > end {
        return Vec::new();
    }

    lines[start..=end].to_vec()
}

/// Apply search/replace blocks to file content
pub fn apply_search_replace(
    original_content: &str,
    blocks: &[SearchReplaceBlock],
    logger: impl Fn(&str),
) -> Result<(String, Vec<String>)> {
    let mut content = original_content.to_string();
    let mut warnings = Vec::new();

    // Define tolerance levels to try in order
    let tolerance_levels = vec![
        ToleranceLevel::Exact,
        ToleranceLevel::IgnoreTrailingWhitespace,
        ToleranceLevel::IgnoreLeadingWhitespace,
        ToleranceLevel::IgnoreAllWhitespace,
    ];

    // Process each block
    for block in blocks {
        let content_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let search_text = block.search_lines.join("\n");
        let replace_lines = block.replace_lines.clone();

        // Find all possible matches with different tolerance levels
        let matches = find_matches(&content_lines, &block.search_lines, &tolerance_levels);

        if matches.is_empty() {
            // No match found, try to find a similar block
            if let Some((match_result, similarity)) =
                find_closest_match(&content_lines, &block.search_lines)
            {
                // Extract the context from the match result
                let range = match_result.range.clone();
                let context_lines = content_lines
                    [range.start.saturating_sub(2)..range.end.min(content_lines.len())]
                    .join("\n");

                let error_msg = format!(
                    "Search block not found in file content. Here's a similar section ({}% similarity):\n```\n{}\n```\n\nRetry immediately with same \"percentage_to_change\" using search replace blocks fixing the search block.",
                    (similarity * 100.0).round(),
                    context_lines
                );
                return Err(anyhow!(SearchReplaceError::MatchError(error_msg)));
            } else {
                return Err(anyhow!(SearchReplaceError::MatchError(format!(
                    "Search block not found in file content:\n```\n{}\n```\n\nRetry immediately with same \"percentage_to_change\" using search replace blocks fixing the search block.",
                    search_text
                ))));
            }
        }

        // Check if we need to handle specific occurrence index
        let target_match = if let Some(index) = block.occurrence_index {
            // User specified which occurrence to replace
            if index >= matches.len() {
                return Err(anyhow!(SearchReplaceError::MatchError(format!(
                    "Requested occurrence index {} but only found {} matches. Valid indices: 0 to {}",
                    index,
                    matches.len(),
                    matches.len() - 1
                ))));
            }
            &matches[index]
        } else {
            // No specific index, handle as before
            #[cfg(not(test))]
            {
                // Check if the search block is the entire file
                let search_content = block.search_lines.join("\n");
                let full_content = content_lines.join("\n");

                // If we're searching for exactly the whole file, just use the first match
                if search_content == full_content {
                    log::debug!("Search block is the entire file content - using first match");
                } else {
                    let best_score = matches[0].score;
                    let matches_with_best_score: Vec<_> = matches
                        .iter()
                        .filter(|m| (m.score - best_score).abs() < 1e-6)
                        .collect();

                    // Check if we have matches at different positions (not just duplicates)
                    let unique_positions: HashSet<_> = matches_with_best_score
                        .iter()
                        .map(|m| (m.range.start, m.range.end))
                        .collect();

                    if unique_positions.len() > 1 {
                        // Multiple matches found with the same score at different positions
                        let block_content = block.search_lines.join("\n");

                        // Analyze context for each match to help the user
                        let mut match_contexts = Vec::new();
                        for (idx, m) in matches_with_best_score.iter().take(4).enumerate() {
                            // Get some lines before and after the match to show context
                            let start = m.range.start.saturating_sub(3); // 3 linhas antes
                            let end = (m.range.end + 3).min(content_lines.len()); // 3 linhas depois

                            let context = content_lines[start..end].join("\n");
                            match_contexts.push(format!(
                                "Match #{} at lines {}-{}:\n```\n{}\n```",
                                idx,
                                start + 1,
                                end,
                                context
                            ));
                        }

                        // Incluir o contexto na mensagem de erro e sugestões mais claras
                        let context_str = match_contexts.join("\n\n");

                        return Err(anyhow!(SearchReplaceError::MultipleMatchesError(format!(
                            "The following block matched {} times:\n```\n{}\n```\n\nHere are the matches found:\n{}\n\nRecommendations to fix this problem:\n1. Include more unique context before and after the block to make the match unique\n2. Include neighboring lines or distinctive characteristics of the text\n3. Specify which occurrence to replace using occurrence_index (0-based)\n4. Use the complete content of the file if it's small\n5. Check for duplicate sections in the file\n\nRetry immediately with same \"percentage_to_change\" using search replace blocks with more context, or specify occurrence_index.",
                            unique_positions.len(),
                            block_content,
                            context_str
                        ))));
                    }
                }
            }

            // We have a unique best match
            &matches[0]
        };

        // Log which tolerance level was used
        for hit in &target_match.tolerances {
            if let Some(warning) = hit.level.warning_message() {
                logger(warning);
                warnings.push(warning.to_string());
            }
        }

        // Get the matched lines for indentation fixing
        let matched_lines = content_lines[target_match.range.clone()].to_vec();

        // Fix indentation in replace lines
        let adjusted_replace_lines =
            fix_indentation(&matched_lines, &block.search_lines, &replace_lines);

        // Create new content with replacement
        let mut new_content_lines = content_lines[..target_match.range.start].to_vec();
        new_content_lines.extend(adjusted_replace_lines);
        new_content_lines.extend(content_lines[target_match.range.end..].to_vec());

        content = new_content_lines.join("\n");
    }

    // Deduplicate warnings
    let warnings: HashSet<String> = warnings.into_iter().collect();

    Ok((content, warnings.into_iter().collect()))
}

// Fallback function for trying blocks individually if multiple blocks fail
pub fn apply_search_replace_with_fallback(
    original_content: &str,
    blocks: &[SearchReplaceBlock],
    logger: impl Fn(&str) + Clone,
) -> Result<(String, Vec<String>)> {
    // Try all blocks at once first
    match apply_search_replace(original_content, blocks, logger.clone()) {
        Ok(result) => Ok(result),
        Err(err) => {
            // If we have multiple blocks and it failed, try them one at a time
            if blocks.len() > 1 {
                logger("Trying blocks individually in sequence...");

                let mut current_content = original_content.to_string();
                let mut all_warnings = Vec::new();

                for block in blocks {
                    match apply_search_replace(&current_content, &[block.clone()], logger.clone()) {
                        Ok((new_content, warnings)) => {
                            current_content = new_content;
                            all_warnings.extend(warnings);
                        }
                        Err(block_err) => {
                            // If individual block fails, propagate that error with improved message
                            log::warn!(
                                "Block #{} failed to apply",
                                blocks.iter().position(|b| b == block).unwrap_or(0) + 1
                            );
                            return Err(block_err);
                        }
                    }
                }

                Ok((current_content, all_warnings))
            } else {
                // Single block, just return the original error
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_with_occurrence_index() {
        let input = r#"# occurrence: 1
<<<<<<< SEARCH
foo
=======
bar
>>>>>>> REPLACE"#;

        let blocks = parse_search_replace_blocks(input).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].occurrence_index, Some(1));
        assert_eq!(blocks[0].search_lines, vec!["foo"]);
        assert_eq!(blocks[0].replace_lines, vec!["bar"]);
    }

    #[test]
    fn test_multiple_matches_with_occurrence_index() {
        let content = "foo\nbar\nfoo\nbaz\nfoo\nqux";
        let blocks = vec![SearchReplaceBlock {
            search_lines: vec!["foo".to_string()],
            replace_lines: vec!["replaced".to_string()],
            occurrence_index: Some(1), // replace second occurrence
        }];

        let (result, _) = apply_search_replace(content, &blocks, |_| {}).unwrap();

        // Check that only the second occurrence was replaced
        let expected = "foo\nbar\nreplaced\nbaz\nfoo\nqux";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_occurrence_index_out_of_bounds() {
        let content = "foo\nbar\nfoo";
        let blocks = vec![SearchReplaceBlock {
            search_lines: vec!["foo".to_string()],
            replace_lines: vec!["replaced".to_string()],
            occurrence_index: Some(5), // index out of bounds
        }];

        let result = apply_search_replace(content, &blocks, |_| {});
        assert!(result.is_err());

        if let Err(err) = result {
            let error_msg = err.to_string();
            assert!(error_msg.contains("Requested occurrence index 5 but only found 2 matches"));
        }
    }

    #[test]
    fn test_parse_without_occurrence_index() {
        let input = r#"<<<<<<< SEARCH
foo
=======
bar
>>>>>>> REPLACE"#;

        let blocks = parse_search_replace_blocks(input).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].occurrence_index, None);
        assert_eq!(blocks[0].search_lines, vec!["foo"]);
        assert_eq!(blocks[0].replace_lines, vec!["bar"]);
    }
}
