use anyhow::{anyhow, Result};
use regex::Regex;
use similar::{ChangeTag, TextDiff};
use std::cmp::Ordering;
use std::collections::HashMap;
use tracing::debug;

/// Match quality in fuzzy search
#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyMatch {
    /// The matched text
    pub text: String,
    /// The match score (0-100, higher is better)
    pub score: f64,
    /// Start position in source
    pub start: usize,
    /// End position in source
    pub end: usize,
    /// Type of match
    pub match_type: MatchType,
}

/// Types of fuzzy matches
#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    /// Exact match
    Exact,
    /// Match with whitespace differences
    Whitespace,
    /// Match with indentation differences
    Indentation,
    /// Match with minor word changes
    WordChanges,
    /// Match with structural similarity
    Structural,
}

impl MatchType {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            MatchType::Exact => "Exact match",
            MatchType::Whitespace => "Match with whitespace differences",
            MatchType::Indentation => "Match with indentation differences",
            MatchType::WordChanges => "Match with minor word changes",
            MatchType::Structural => "Match with structural similarity",
        }
    }

    /// Get a numeric score for this match type
    pub fn score(&self) -> f64 {
        match self {
            MatchType::Exact => 100.0,
            MatchType::Whitespace => 90.0,
            MatchType::Indentation => 80.0,
            MatchType::WordChanges => 70.0,
            MatchType::Structural => 50.0,
        }
    }
}

/// Find fuzzy matches for a pattern in content
pub fn find_fuzzy_matches(content: &str, pattern: &str, threshold: f64) -> Vec<FuzzyMatch> {
    debug!(
        "Finding fuzzy matches for pattern of length {}",
        pattern.len()
    );

    let mut matches = Vec::new();

    // Try exact match first
    if let Some(pos) = content.find(pattern) {
        matches.push(FuzzyMatch {
            text: pattern.to_string(),
            score: 100.0,
            start: pos,
            end: pos + pattern.len(),
            match_type: MatchType::Exact,
        });
        return matches;
    }

    // Try with normalized whitespace
    let normalized_pattern = normalize_whitespace(pattern);
    let normalized_content = normalize_whitespace(content);

    if let Some(pos) = normalized_content.find(&normalized_pattern) {
        // Find the corresponding position in the original content
        let start = find_original_position(content, &normalized_content, pos);
        let end =
            find_original_position(content, &normalized_content, pos + normalized_pattern.len());

        matches.push(FuzzyMatch {
            text: content[start..end].to_string(),
            score: 90.0,
            start,
            end,
            match_type: MatchType::Whitespace,
        });
        return matches;
    }

    // Try with normalized indentation
    let normalized_pattern = normalize_indentation(pattern);

    // Process content by blocks to find potential indentation matches
    let content_blocks = content.split("\n\n").collect::<Vec<_>>();

    for (block_idx, block) in content_blocks.iter().enumerate() {
        let normalized_block = normalize_indentation(block);

        if let Some(pos) = normalized_block.find(&normalized_pattern) {
            // Calculate original block start position
            let block_start = content_blocks[..block_idx]
                .iter()
                .map(|b| b.len() + 2) // +2 for the "\n\n" separator
                .sum::<usize>();

            // Find the corresponding position in the original content
            let start = block_start + find_original_position(block, &normalized_block, pos);
            let end = block_start
                + find_original_position(block, &normalized_block, pos + normalized_pattern.len());

            matches.push(FuzzyMatch {
                text: content[start..end].to_string(),
                score: 80.0,
                start,
                end,
                match_type: MatchType::Indentation,
            });

            // If we found a high-quality match, return
            if matches.len() >= 3 {
                return matches;
            }
        }
    }

    // Try fuzzy word-by-word matching if still not found
    if matches.is_empty() {
        let mut best_match = None;
        let mut best_score = 0.0;

        // Split content into chunks of approximately the same size as the pattern
        let chunk_size = pattern.len().clamp(20, 100);
        let step_size = chunk_size / 2; // Overlap chunks

        for i in (0..content.len()).step_by(step_size) {
            let end_idx = (i + chunk_size).min(content.len());
            if end_idx - i < pattern.len() / 2 {
                continue; // Skip chunks that are too small
            }

            let chunk = &content[i..end_idx];
            let score = calculate_similarity_score(chunk, pattern);

            if score > threshold && score > best_score {
                best_score = score;
                best_match = Some(FuzzyMatch {
                    text: chunk.to_string(),
                    score,
                    start: i,
                    end: end_idx,
                    match_type: if score > 75.0 {
                        MatchType::WordChanges
                    } else {
                        MatchType::Structural
                    },
                });
            }
        }

        if let Some(m) = best_match {
            matches.push(m);
        }
    }

    // Sort matches by score (highest first)
    matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

    matches
}

/// Normalize whitespace in a string (collapse multiple spaces to one)
fn normalize_whitespace(s: &str) -> String {
    let re = Regex::new(r"\s+").unwrap();
    re.replace_all(s, " ").to_string()
}

/// Normalize indentation (remove leading whitespace from each line)
fn normalize_indentation(s: &str) -> String {
    s.lines()
        .map(|line| line.trim_start())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Find the position in the original string corresponding to a position in the normalized string
fn find_original_position(original: &str, normalized: &str, normalized_pos: usize) -> usize {
    let mut orig_pos = 0;
    let mut norm_pos = 0;

    for c in original.chars() {
        if norm_pos == normalized_pos {
            break;
        }

        orig_pos += 1;

        // We need to match the normalization logic here
        if !c.is_whitespace()
            || (c == ' ' && (norm_pos == 0 || normalized.chars().nth(norm_pos - 1).unwrap() != ' '))
        {
            norm_pos += 1;
        }
    }

    orig_pos
}

/// Calculate similarity score between two strings (0-100)
fn calculate_similarity_score(s1: &str, s2: &str) -> f64 {
    let diff = TextDiff::from_chars(s1, s2);

    let mut unchanged = 0;
    let mut _changed = 0; // Prefixado com _ pois não é usado

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => unchanged += change.value().chars().count(),
            _ => _changed += change.value().chars().count(),
        }
    }

    let total = s1.chars().count() + s2.chars().count();
    if total == 0 {
        return 0.0;
    }

    100.0 * (2.0 * unchanged as f64) / total as f64
}

/// Apply replacements to content with fuzzy matching
pub fn apply_fuzzy_replacements(
    content: &str,
    replacements: &[(String, String)],
    threshold: f64,
) -> Result<(String, Vec<String>)> {
    let mut result = content.to_string();
    let mut warnings = Vec::new();

    // Keep track of offsets due to replacements
    let mut offset = 0;

    for (search, replace) in replacements {
        let matches = find_fuzzy_matches(&result, search, threshold);

        if matches.is_empty() {
            let warning = format!("No matches found for pattern: {}", truncate(search, 50));
            warnings.push(warning);
            continue;
        }

        // Get the best match
        let best_match = &matches[0];

        // Warn if not an exact match
        if best_match.match_type != MatchType::Exact {
            let warning = format!(
                "Using {} ({:.1}% confidence) for: {}",
                best_match.match_type.description(),
                best_match.score,
                truncate(search, 50)
            );
            warnings.push(warning);
        }

        // Apply the replacement
        let start = best_match.start + offset;
        let end = best_match.end + offset;

        result.replace_range(start..end, replace);

        // Update offset
        let size_difference = replace.len() as i64 - (end - start) as i64;
        // Convert back to usize safely, handling negative offsets
        if size_difference < 0 {
            offset = offset.saturating_sub((-size_difference) as usize);
        } else {
            offset += size_difference as usize;
        }
    }

    Ok((result, warnings))
}

/// Advanced search and replace with fuzzy matching and indentation preservation
pub fn smart_search_replace(
    content: &str,
    search: &str,
    replace: &str,
    preserve_indentation: bool,
) -> Result<(String, Vec<String>)> {
    let mut warnings = Vec::new();

    // Find the best match
    let matches = find_fuzzy_matches(content, search, 50.0);

    if matches.is_empty() {
        return Err(anyhow!(
            "Could not find a suitable match for the search pattern"
        ));
    }

    let best_match = &matches[0];

    // Extract the match
    let matched_text = &content[best_match.start..best_match.end];

    // Analyze indentation if requested
    let replacement = if preserve_indentation {
        apply_indentation_pattern(matched_text, replace)
    } else {
        replace.to_string()
    };

    // Warn if not an exact match
    if best_match.match_type != MatchType::Exact {
        let warning = format!(
            "Using {} ({:.1}% confidence)",
            best_match.match_type.description(),
            best_match.score
        );
        warnings.push(warning);
    }

    // Apply the replacement
    let mut result = content.to_string();
    result.replace_range(best_match.start..best_match.end, &replacement);

    Ok((result, warnings))
}

/// Apply indentation pattern from source to target
fn apply_indentation_pattern(source: &str, target: &str) -> String {
    let source_lines: Vec<&str> = source.lines().collect();
    let target_lines: Vec<&str> = target.lines().collect();

    if source_lines.is_empty() || target_lines.is_empty() {
        return target.to_string();
    }

    // Extract indentation from first line of source
    let first_line_indentation = get_indentation(source_lines[0]);

    // Build a map of relative indentation levels
    let mut indentation_map = HashMap::new();
    for (i, line) in source_lines.iter().enumerate() {
        let indent = get_indentation(line);
        let rel_indent = indent.len() as isize - first_line_indentation.len() as isize;
        indentation_map.insert(i % target_lines.len(), rel_indent);
    }

    // Calculate first line indentation for target
    let _target_first_indent = get_indentation(target_lines[0]); // Prefixado com _ pois não é usado

    // Apply indentation to target lines
    let mut result = Vec::new();
    for (i, line) in target_lines.iter().enumerate() {
        // Skip empty lines
        if line.trim().is_empty() {
            result.push(line.to_string());
            continue;
        }

        let _line_indent = get_indentation(line); // Prefixado com _ pois não é usado

        // Calculate target indentation
        let rel_indent = indentation_map.get(&i).copied().unwrap_or(0);
        let target_indent = if rel_indent >= 0 {
            first_line_indentation.to_string() + &" ".repeat(rel_indent as usize)
        } else {
            // Remove some indentation but don't go negative
            first_line_indentation[..first_line_indentation.len()
                - (-rel_indent as usize).min(first_line_indentation.len())]
                .to_string()
        };

        // Replace indentation
        let new_line = format!("{}{}", target_indent, line.trim_start());
        result.push(new_line);
    }

    result.join("\n")
}

/// Get the indentation (leading whitespace) of a line
fn get_indentation(line: &str) -> &str {
    let trimmed_start = line.len() - line.trim_start().len();
    &line[..trimmed_start]
}

/// Truncate a string with ellipsis if too long
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_fuzzy_matches_exact() {
        let content = "function hello() {\n    console.log('Hello');\n}";
        let pattern = "console.log('Hello');";

        let matches = find_fuzzy_matches(content, pattern, 50.0);

        assert!(!matches.is_empty());
        assert_eq!(matches[0].match_type, MatchType::Exact);
        assert_eq!(matches[0].score, 100.0);
    }

    #[test]
    fn test_find_fuzzy_matches_whitespace() {
        let content = "function hello() {\n    console.log('Hello');\n}";
        let pattern = "console.log ( 'Hello' ) ;";

        let matches = find_fuzzy_matches(content, pattern, 50.0);

        assert!(!matches.is_empty());
        assert!(matches[0].score > 80.0);
    }

    #[test]
    fn test_find_fuzzy_matches_indentation() {
        let content = "function hello() {\n    console.log('Hello');\n}";
        let pattern = "console.log('Hello');";

        let matches = find_fuzzy_matches(content, pattern, 50.0);

        assert!(!matches.is_empty());
        assert_eq!(matches[0].match_type, MatchType::Exact);
    }

    #[test]
    fn test_apply_fuzzy_replacements() {
        let content = "function hello() {\n    console.log('Hello');\n}";
        let replacements = vec![(
            "console.log('Hello');".to_string(),
            "console.log('Hello, World!');".to_string(),
        )];

        let (result, warnings) = apply_fuzzy_replacements(content, &replacements, 50.0).unwrap();

        assert!(warnings.is_empty());
        assert_eq!(
            result,
            "function hello() {\n    console.log('Hello, World!');\n}"
        );
    }

    #[test]
    fn test_apply_indentation_pattern() {
        let source = "    function hello() {\n        console.log('Hello');\n    }";
        let target = "function world() {\n    console.log('World');\n}";

        let result = apply_indentation_pattern(source, target);

        assert_eq!(
            result,
            "    function world() {\n        console.log('World');\n    }"
        );
    }
}
