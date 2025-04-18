use anyhow::{anyhow, Result};
use tracing::debug;

use super::search_replace::{
    adjust_indentation, find_context_for_search_block, find_matches, parse_search_replace_blocks,
    similarity_score, EditResult, SearchReplaceBlock, ToleranceLevel, ToleranceMatch,
};

/// Enhanced result with detailed diagnostic information
#[derive(Debug, Clone)]
pub struct EnhancedEditResult {
    /// Standard edit result
    pub standard_result: EditResult,
    /// Detailed diagnostics for each block
    pub diagnostics: Vec<BlockDiagnostic>,
    /// Whether blocks were applied individually when batch failed
    pub applied_individually: bool,
    /// Suggestions for fixing failed blocks
    pub suggestions: Vec<String>,
}

/// Diagnostic information for a search/replace block
#[derive(Debug, Clone)]
pub struct BlockDiagnostic {
    /// Block index (0-based)
    pub index: usize,
    /// Whether the block was successfully applied
    pub success: bool,
    /// Matches found for this block
    pub matches: Vec<ToleranceMatch>,
    /// Tolerance level used (if successful)
    pub tolerance_used: Option<ToleranceLevel>,
    /// Error message (if unsuccessful)
    pub error: Option<String>,
    /// Line range after replacement
    pub replacement_range: Option<(usize, usize)>,
    /// Similar content found (if no exact match)
    pub similar_content: Option<String>,
    /// Similarity score (if similar content found)
    pub similarity_score: Option<f64>,
}

impl BlockDiagnostic {
    /// Check if a block has ambiguous matches
    pub fn has_ambiguous_matches(&self) -> bool {
        self.matches.len() > 1
    }

    /// Get a textual summary of this diagnostic
    pub fn get_summary(&self) -> String {
        if !self.success {
            if let Some(error) = &self.error {
                format!("Block #{}: Failed - {}", self.index + 1, error)
            } else {
                format!("Block #{}: Failed - Unknown error", self.index + 1)
            }
        } else if self.has_ambiguous_matches() {
            format!(
                "Block #{}: Success with {} matches (used first) - {}",
                self.index + 1,
                self.matches.len(),
                self.tolerance_used
                    .map_or("Unknown".to_string(), |t| t.message().to_string())
            )
        } else {
            format!(
                "Block #{}: Success - {}",
                self.index + 1,
                self.tolerance_used
                    .map_or("Unknown".to_string(), |t| t.message().to_string())
            )
        }
    }
}

/// Enhanced search/replace engine with better error handling and diagnostics
#[derive(Clone)]
pub struct EnhancedSearchReplace {
    /// Maximum number of attempts for individual blocks
    #[allow(dead_code)]
    max_individual_attempts: usize,
    /// Whether to attempt individual application when batch fails
    enable_individual_fallback: bool,
    /// Whether to attempt fixing indentation in blocks
    fix_indentation: bool,
    /// Minimum similarity score for suggesting similar content
    min_similarity_score: f64,
    /// Context lines to include in diagnostics
    diagnostic_context_lines: usize,
}

impl Default for EnhancedSearchReplace {
    fn default() -> Self {
        Self {
            max_individual_attempts: 3,
            enable_individual_fallback: true,
            fix_indentation: true,
            min_similarity_score: 0.7,
            diagnostic_context_lines: 3,
        }
    }
}

impl EnhancedSearchReplace {
    /// Create a new enhanced search/replace engine
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to enable individual block fallback
    pub fn with_individual_fallback(mut self, enable: bool) -> Self {
        self.enable_individual_fallback = enable;
        self
    }

    /// Set whether to attempt fixing indentation
    pub fn with_indentation_fixing(mut self, enable: bool) -> Self {
        self.fix_indentation = enable;
        self
    }

    /// Set the minimum similarity score for suggestions
    pub fn with_min_similarity_score(mut self, score: f64) -> Self {
        self.min_similarity_score = score;
        self
    }

    /// Set the number of context lines for diagnostics
    pub fn with_diagnostic_context_lines(mut self, lines: usize) -> Self {
        self.diagnostic_context_lines = lines;
        self
    }

    /// Apply search/replace blocks with enhanced error handling and diagnostics
    pub fn apply_search_replace(
        &self,
        content: &str,
        blocks: &[SearchReplaceBlock],
    ) -> Result<EnhancedEditResult> {
        debug!(
            "Applying {} search/replace blocks with enhanced handling",
            blocks.len()
        );

        // Try batch application first
        match self.apply_batch(content, blocks) {
            Ok((result, diagnostics)) => {
                Ok(EnhancedEditResult {
                    standard_result: result,
                    diagnostics,
                    applied_individually: false,
                    suggestions: Vec::new(),
                })
            }
            Err(batch_error) => {
                // If batch failed and individual fallback is enabled, try individual application
                if self.enable_individual_fallback {
                    debug!("Batch application failed, attempting individual application");
                    match self.apply_individually(content, blocks) {
                        Ok((result, diagnostics, suggestions)) => {
                            Ok(EnhancedEditResult {
                                standard_result: result,
                                diagnostics,
                                applied_individually: true,
                                suggestions,
                            })
                        }
                        Err(individual_error) => {
                            // Both batch and individual failed
                            debug!("Both batch and individual application failed");
                            Err(anyhow!(
                                "Failed to apply search/replace blocks. Batch error: {}. Individual error: {}",
                                batch_error, individual_error
                            ))
                        }
                    }
                } else {
                    // Individual fallback disabled, return batch error
                    Err(anyhow!(
                        "Failed to apply search/replace blocks: {}",
                        batch_error
                    ))
                }
            }
        }
    }

    /// Apply blocks in batch
    fn apply_batch(
        &self,
        content: &str,
        blocks: &[SearchReplaceBlock],
    ) -> Result<(EditResult, Vec<BlockDiagnostic>)> {
        let content_lines: Vec<String> = content.lines().map(ToString::to_string).collect();
        let mut result_lines = content_lines.clone();
        let mut warnings = Vec::new();
        let mut changes_made = false;
        let mut diagnostics = Vec::new();

        // Process each block sequentially
        for (block_idx, block) in blocks.iter().enumerate() {
            // Start searching from the beginning of content for the first block
            // or after the last processed block
            let start_line = 0; // Simplified - in a real implementation would track previous blocks

            // Find matches for this search block
            let matches = find_matches(&result_lines, &block.search_lines, start_line);

            // Create diagnostic entry
            let mut diagnostic = BlockDiagnostic {
                index: block_idx,
                success: false,
                matches: matches.clone(),
                tolerance_used: None,
                error: None,
                replacement_range: None,
                similar_content: None,
                similarity_score: None,
            };

            if matches.is_empty() {
                // Find similar content for diagnostic purposes
                if let Some(context) = find_context_for_search_block(
                    &result_lines.join("\n"),
                    &block.search_lines,
                    self.diagnostic_context_lines,
                ) {
                    let score = self.calculate_block_similarity(&block.search_lines, &context);
                    diagnostic.similar_content = Some(context);
                    diagnostic.similarity_score = Some(score);
                }

                diagnostic.error = Some(format!(
                    "Could not find a match for search block #{}",
                    block_idx + 1
                ));

                diagnostics.push(diagnostic);

                return Err(anyhow!(
                    "Could not find a match for search block #{}: {:?}",
                    block_idx + 1,
                    block.search_lines
                ));
            }

            // Use the best match (first in list, as they're ordered by preference)
            let best_match = &matches[0];

            // Update diagnostic with match info
            diagnostic.success = true;
            diagnostic.tolerance_used = Some(best_match.level);

            // Add warnings if needed
            if let Some(warning) = best_match.warning_message() {
                warnings.push(warning);
            }

            // Apply the replacement
            let (start, end) = best_match.range;
            diagnostic.replacement_range = Some((start, end));

            // Debug to check what we're replacing
            debug!(
                "Replacing block #{} at lines {}-{} with new content of {} lines",
                block_idx + 1,
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
            let replace_lines = if self.fix_indentation
                && best_match.level == ToleranceLevel::IgnoreLeadingWhitespace
            {
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

            diagnostics.push(diagnostic);
        }

        // Join the lines back into a string
        let result_content = result_lines.join("\n");

        Ok((
            EditResult {
                content: result_content,
                warnings,
                changes_made,
            },
            diagnostics,
        ))
    }

    /// Apply blocks individually
    fn apply_individually(
        &self,
        content: &str,
        blocks: &[SearchReplaceBlock],
    ) -> Result<(EditResult, Vec<BlockDiagnostic>, Vec<String>)> {
        let mut current_content = content.to_string();
        let mut all_warnings = Vec::new();
        let mut changes_made = false;
        let mut diagnostics = Vec::new();
        let mut suggestions = Vec::new();

        // Process each block individually
        for (block_idx, block) in blocks.iter().enumerate() {
            // Try to apply this single block
            match self.apply_single_block(&current_content, block, block_idx) {
                Ok((block_result, block_diagnostic)) => {
                    // Update tracking variables
                    current_content = block_result.content;
                    all_warnings.extend(block_result.warnings);
                    changes_made |= block_result.changes_made;
                    diagnostics.push(block_diagnostic);
                }
                Err(e) => {
                    // This block failed to apply
                    let mut diagnostic = BlockDiagnostic {
                        index: block_idx,
                        success: false,
                        matches: Vec::new(),
                        tolerance_used: None,
                        error: Some(e.to_string()),
                        replacement_range: None,
                        similar_content: None,
                        similarity_score: None,
                    };

                    // Try to find similar content for diagnostic
                    // No need to split into lines here as find_context_for_search_block handles it
                    if let Some(context) = find_context_for_search_block(
                        &current_content,
                        &block.search_lines,
                        self.diagnostic_context_lines,
                    ) {
                        let score = self.calculate_block_similarity(&block.search_lines, &context);
                        // Clone context before moving it
                        let context_clone = context.clone();
                        diagnostic.similar_content = Some(context);
                        diagnostic.similarity_score = Some(score);

                        // Generate suggestions
                        let block_suggestions =
                            self.generate_suggestions(block, &context_clone, score);
                        suggestions.extend(block_suggestions);
                    }

                    diagnostics.push(diagnostic);
                }
            }
        }

        // If no blocks were successfully applied, return error
        if !changes_made {
            return Err(anyhow!("Failed to apply any search/replace blocks"));
        }

        Ok((
            EditResult {
                content: current_content,
                warnings: all_warnings,
                changes_made,
            },
            diagnostics,
            suggestions,
        ))
    }

    /// Apply a single block
    fn apply_single_block(
        &self,
        content: &str,
        block: &SearchReplaceBlock,
        block_idx: usize,
    ) -> Result<(EditResult, BlockDiagnostic)> {
        let content_lines: Vec<String> = content.lines().map(ToString::to_string).collect();

        // Find matches for this search block
        let matches = find_matches(&content_lines, &block.search_lines, 0);

        // Create diagnostic entry
        let mut diagnostic = BlockDiagnostic {
            index: block_idx,
            success: false,
            matches: matches.clone(),
            tolerance_used: None,
            error: None,
            replacement_range: None,
            similar_content: None,
            similarity_score: None,
        };

        if matches.is_empty() {
            diagnostic.error = Some(format!(
                "Could not find a match for search block #{}",
                block_idx + 1
            ));

            return Err(anyhow!(
                "Could not find a match for search block #{}: {:?}",
                block_idx + 1,
                block.search_lines
            ));
        }

        // Use the best match
        let best_match = &matches[0];

        // Update diagnostic
        diagnostic.success = true;
        diagnostic.tolerance_used = Some(best_match.level);

        let mut warnings = Vec::new();

        // Add warnings if needed
        if let Some(warning) = best_match.warning_message() {
            warnings.push(warning);
        }

        // Apply the replacement
        let (start, end) = best_match.range;
        diagnostic.replacement_range = Some((start, end));

        // When we have multiple matches, warn about ambiguity
        if matches.len() > 1 {
            let warning = format!(
                "Warning: Search block #{} matches multiple parts of the file. Using the first match.",
                block_idx + 1
            );
            warnings.push(warning);
        }

        // Fix indentation if needed
        let replace_lines = if self.fix_indentation
            && best_match.level == ToleranceLevel::IgnoreLeadingWhitespace
        {
            adjust_indentation(
                &content_lines[start..end + 1],
                &block.search_lines,
                &block.replace_lines,
            )
        } else {
            block.replace_lines.clone()
        };

        // Apply the replacement
        let mut result_lines = content_lines.clone();
        result_lines.splice(start..end + 1, replace_lines);
        let result_content = result_lines.join("\n");

        Ok((
            EditResult {
                content: result_content,
                warnings,
                changes_made: true,
            },
            diagnostic,
        ))
    }

    /// Calculate similarity between a search block and found content
    fn calculate_block_similarity(&self, search_lines: &[String], context: &str) -> f64 {
        let search_text = search_lines.join("\n");
        similarity_score(&search_text, context)
    }

    /// Generate suggestions for fixing a failed block
    fn generate_suggestions(
        &self,
        block: &SearchReplaceBlock,
        similar_content: &str,
        similarity: f64,
    ) -> Vec<String> {
        let mut suggestions = Vec::new();

        if similarity < self.min_similarity_score {
            // Not similar enough for specific suggestions
            suggestions.push(
                "Consider rewriting the search block to match the actual content.".to_string(),
            );
            return suggestions;
        }

        // Split context into lines for analysis
        let context_lines: Vec<&str> = similar_content.lines().collect();
        let search_lines = &block.search_lines;

        // Find differences line by line
        for (i, search_line) in search_lines.iter().enumerate() {
            if i >= context_lines.len() {
                suggestions.push(format!(
                    "Search block is longer than actual content. Consider removing lines after line {}.",
                    i
                ));
                break;
            }

            let context_line = context_lines[i];

            // Calculate line similarity
            let line_similarity = similarity_score(search_line, context_line);

            if line_similarity < 0.8 {
                // Substantial difference in this line
                suggestions.push(format!(
                    "Search line {} differs significantly from the actual content. Found: '{}'",
                    i + 1,
                    context_line
                ));

                // Common issues: whitespace, quotes, brackets
                if search_line.trim() != context_line.trim() {
                    suggestions.push(format!(
                        "Check for content differences in line {}. Actual: '{}', Expected: '{}'",
                        i + 1,
                        context_line.trim(),
                        search_line.trim()
                    ));
                } else {
                    suggestions.push(format!(
                        "Check for whitespace/indentation differences in line {}.",
                        i + 1
                    ));
                }
            }
        }

        // Check if context is longer than search
        if context_lines.len() > search_lines.len() {
            suggestions.push(format!(
                "Actual content has {} more lines than search block. Consider adding these lines to your search.",
                context_lines.len() - search_lines.len()
            ));
        }

        // Add generic suggestions
        suggestions.push(
            "Consider adding more context (lines before/after) to make the search block more specific.".to_string()
        );

        if suggestions.is_empty() {
            suggestions.push(
                "The content seems similar but doesn't match exactly. Check for invisible characters or encoding issues.".to_string()
            );
        }

        suggestions
    }

    /// Apply search/replace blocks from text
    pub fn apply_from_text(
        &self,
        content: &str,
        search_replace_text: &str,
    ) -> Result<EnhancedEditResult> {
        // Try to parse the blocks, with more detailed error handling
        match parse_search_replace_blocks(search_replace_text) {
            Ok(blocks) => self.apply_search_replace(content, &blocks),
            Err(e) => {
                // Log the error for debugging
                debug!("Failed to parse search/replace blocks: {}", e);
                debug!("Original text: {}", search_replace_text);

                // Convert to a more user-friendly error message
                let err_msg = if e.to_string().contains("No valid search/replace blocks") {
                    String::from(
                        "Failed to parse search/replace blocks: No valid blocks found. \
                    Use either <<<<<<< SEARCH/=======/>>>>>>> REPLACE format \
                    or search:/replace: prefix format.",
                    )
                } else {
                    e.to_string()
                };

                Err(anyhow!("{}", err_msg))
            }
        }
    }

    /// Generate a detailed report of the edit result
    pub fn generate_report(&self, result: &EnhancedEditResult) -> String {
        let mut report = String::new();

        report.push_str("# Search/Replace Operation Report\n\n");

        // Overview
        report.push_str("## Overview\n\n");
        report.push_str(&format!(
            "- **Status**: {}\n",
            if result.standard_result.changes_made {
                "Changes Applied"
            } else {
                "No Changes Made"
            }
        ));
        report.push_str(&format!(
            "- **Applied Individually**: {}\n",
            if result.applied_individually {
                "Yes (batch application failed)"
            } else {
                "No (batch application succeeded)"
            }
        ));

        // Warning summary
        if !result.standard_result.warnings.is_empty() {
            report.push_str(&format!(
                "- **Warnings**: {}\n",
                result.standard_result.warnings.len()
            ));
        }

        report.push('\n');

        // Block diagnostics
        report.push_str("## Block Diagnostics\n\n");

        for diagnostic in &result.diagnostics {
            report.push_str(&format!("### Block #{}\n\n", diagnostic.index + 1));
            report.push_str(&format!(
                "- **Status**: {}\n",
                if diagnostic.success {
                    "Success"
                } else {
                    "Failed"
                }
            ));

            if let Some(tolerance) = &diagnostic.tolerance_used {
                report.push_str(&format!("- **Tolerance**: {}\n", tolerance.message()));
            }

            if diagnostic.has_ambiguous_matches() {
                report.push_str(&format!(
                    "- **Ambiguity**: {} matches found (used first)\n",
                    diagnostic.matches.len()
                ));
            }

            if let Some(range) = diagnostic.replacement_range {
                report.push_str(&format!(
                    "- **Replaced Lines**: {}-{}\n",
                    range.0 + 1,
                    range.1 + 1
                ));
            }

            if let Some(error) = &diagnostic.error {
                report.push_str(&format!("- **Error**: {}\n", error));
            }

            if let Some(score) = diagnostic.similarity_score {
                report.push_str(&format!("- **Similarity**: {:.1}%\n", score * 100.0));
            }

            if let Some(content) = &diagnostic.similar_content {
                report.push_str("\n**Similar Content Found**:\n");
                report.push_str("```\n");
                report.push_str(content);
                report.push_str("\n```\n");
            }

            report.push('\n');
        }

        // Suggestions
        if !result.suggestions.is_empty() {
            report.push_str("## Suggestions\n\n");

            for suggestion in &result.suggestions {
                report.push_str(&format!("- {}\n", suggestion));
            }

            report.push('\n');
        }

        // Warnings in detail
        if !result.standard_result.warnings.is_empty() {
            report.push_str("## Warnings\n\n");

            for warning in &result.standard_result.warnings {
                report.push_str(&format!("- {}\n", warning));
            }

            report.push('\n');
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_search_replace_success() {
        let engine = EnhancedSearchReplace::new();

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

        let result = engine.apply_search_replace(content, &blocks).unwrap();
        assert!(result.standard_result.changes_made);
        assert_eq!(
            result.standard_result.content,
            "function hello() {\n    console.log(\"Hello, World!\");\n}\n\nconst x = 10;"
        );
        assert_eq!(result.diagnostics.len(), 2);
        assert!(result.diagnostics[0].success);
        assert!(result.diagnostics[1].success);
        assert!(!result.applied_individually);
    }

    #[test]
    fn test_apply_search_replace_fallback() {
        let engine = EnhancedSearchReplace::new();

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
            // Block with a typo in search that won't match exactly
            SearchReplaceBlock {
                search_lines: vec!["const y = 5;".to_string()],
                replace_lines: vec!["const y = 10;".to_string()],
            },
        ];

        // This will fail in batch mode but succeed with first block in individual mode
        let result = engine.apply_search_replace(content, &blocks).unwrap();
        assert!(result.standard_result.changes_made);
        assert_eq!(
            result.standard_result.content,
            "function hello() {\n    console.log(\"Hello, World!\");\n}\n\nconst x = 5;"
        );
        assert_eq!(result.diagnostics.len(), 2);
        assert!(result.diagnostics[0].success);
        assert!(!result.diagnostics[1].success);
        assert!(result.applied_individually);
        assert!(!result.suggestions.is_empty()); // Should have suggestions for fixing
    }

    #[test]
    fn test_generate_report() {
        let engine = EnhancedSearchReplace::new();

        let content = "function hello() {\n    console.log(\"Hello\");\n}\n\nconst x = 5;";

        let blocks = vec![SearchReplaceBlock {
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
        }];

        let result = engine.apply_search_replace(content, &blocks).unwrap();
        let report = engine.generate_report(&result);

        assert!(report.contains("# Search/Replace Operation Report"));
        assert!(report.contains("**Status**: Changes Applied"));
        assert!(report.contains("Block #1"));
        assert!(report.contains("**Status**: Success"));
    }
}
