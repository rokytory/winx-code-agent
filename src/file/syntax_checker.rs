use std::path::Path;

/// Performs basic syntax checking on a file's content based on its file extension
///
/// This function analyzes code files and reports common syntax issues without
/// requiring a full compiler or language server. It's meant to catch simple
/// mistakes quickly rather than provide comprehensive validation.
///
/// @param file_path - Path to the file being checked (used to determine file type)
/// @param content - The content to analyze for syntax issues
/// @return A vector of warning messages if issues are found
pub fn check_syntax(file_path: &Path, content: &str) -> Vec<String> {
    let mut warnings = Vec::new();

    // Get file extension
    if let Some(extension) = file_path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();

        match ext.as_str() {
            "rs" => {
                // Basic Rust syntax checks
                if !content.contains("fn ") && content.contains("pub fn") {
                    warnings
                        .push("Warning: File contains 'pub fn' but no regular 'fn' declarations. Check function visibility.".to_string());
                }

                // Check for unbalanced brackets
                let open_brackets = content.matches('{').count();
                let close_brackets = content.matches('}').count();
                if open_brackets != close_brackets {
                    warnings.push(format!(
                        "Warning: Unbalanced brackets - {} open vs {} closed",
                        open_brackets, close_brackets
                    ));
                }

                // Check for unbalanced parentheses
                let open_parens = content.matches('(').count();
                let close_parens = content.matches(')').count();
                if open_parens != close_parens {
                    warnings.push(format!(
                        "Warning: Unbalanced parentheses - {} open vs {} closed",
                        open_parens, close_parens
                    ));
                }
            }
            "py" => {
                // Basic Python syntax checks

                // Check indentation consistency
                let mut spaces_per_indent = None;
                for line in content.lines() {
                    if line.trim().is_empty() || line.trim().starts_with('#') {
                        continue;
                    }

                    let spaces = line.len() - line.trim_start().len();
                    if spaces > 0 {
                        if let Some(standard) = spaces_per_indent {
                            if spaces % standard != 0 {
                                warnings.push(format!(
                                    "Warning: Inconsistent indentation - line uses {} spaces but standard is multiple of {}",
                                    spaces, standard
                                ));
                                break;
                            }
                        } else if spaces > 0 {
                            spaces_per_indent = Some(spaces);
                        }
                    }
                }

                // Check for colons in control structures
                if content.contains("if ") && !content.contains("if:") {
                    warnings.push("Warning: 'if' statement without colon. Python requires a colon after condition statements.".to_string());
                }
                if content.contains("def ") && !content.contains("def:") {
                    warnings.push("Warning: 'def' statement without colon. Python functions require a colon after parameter list.".to_string());
                }
            }
            "js" | "ts" | "jsx" | "tsx" => {
                // Basic JavaScript/TypeScript syntax checks

                // Check for unbalanced brackets
                let open_brackets = content.matches('{').count();
                let close_brackets = content.matches('}').count();
                if open_brackets != close_brackets {
                    warnings.push(format!(
                        "Warning: Unbalanced brackets - {} open vs {} closed",
                        open_brackets, close_brackets
                    ));
                }

                // Check for missing semicolons
                let lines = content.lines().collect::<Vec<&str>>();
                for (i, line_content) in lines.iter().enumerate() {
                    let line = line_content.trim();
                    if !line.is_empty()
                        && !line.ends_with('{')
                        && !line.ends_with('}')
                        && !line.ends_with(';')
                        && !line.ends_with(':')
                        && !line.starts_with("//")
                        && !line.contains("=>")
                        && !line.contains("function")
                    {
                        warnings.push(format!(
                            "Warning: Possible missing semicolon at line {}. Most JavaScript statements should end with a semicolon.",
                            i + 1
                        ));
                    }
                }
            }
            "go" => {
                // Basic Go syntax checks

                // Check for unbalanced brackets
                let open_brackets = content.matches('{').count();
                let close_brackets = content.matches('}').count();
                if open_brackets != close_brackets {
                    warnings.push(format!(
                        "Warning: Unbalanced brackets - {} open vs {} closed",
                        open_brackets, close_brackets
                    ));
                }

                if !content.contains("package ") {
                    warnings.push("Warning: Missing 'package' declaration. All Go files must declare their package name.".to_string());
                }
            }
            "html" => {
                // Basic HTML syntax checks

                // Check for common HTML tags
                let has_html = content.contains("<html") || content.contains("<HTML");
                let has_head = content.contains("<head") || content.contains("<HEAD");
                let has_body = content.contains("<body") || content.contains("<BODY");

                if has_html && (!has_head || !has_body) {
                    warnings.push("Warning: HTML file contains <html> tag but is missing <head> or <body> tags. Standard HTML documents require both.".to_string());
                }

                // Check for unbalanced tags (very basic)
                for tag in ["div", "p", "span", "a", "table", "tr", "td"].iter() {
                    let open_tags = content.matches(&format!("<{}", tag)).count();
                    let close_tags = content.matches(&format!("</{}", tag)).count();
                    if open_tags != close_tags {
                        warnings.push(format!(
                            "Warning: Unbalanced <{}> tags - {} opening tags vs {} closing tags. Each opening tag requires a matching closing tag.",
                            tag, open_tags, close_tags
                        ));
                    }
                }
            }
            "css" => {
                // Basic CSS syntax checks

                // Check for unbalanced brackets
                let open_brackets = content.matches('{').count();
                let close_brackets = content.matches('}').count();
                if open_brackets != close_brackets {
                    warnings.push(format!(
                        "Warning: Unbalanced brackets - {} open vs {} closed",
                        open_brackets, close_brackets
                    ));
                }
            }
            "json" => {
                // Basic JSON syntax checks

                // Try to parse the JSON
                match serde_json::from_str::<serde_json::Value>(content) {
                    Ok(_) => {} // Valid JSON
                    Err(e) => {
                        warnings.push(format!("Warning: Invalid JSON structure - {}. Check for missing commas, brackets, or quotes.", e));
                    }
                }
            }
            _ => {
                // Generic checks for most file types

                // Check for very long lines
                for (i, line) in content.lines().enumerate() {
                    if line.len() > 120 {
                        warnings.push(format!(
                            "Warning: Line {} is very long ({} characters). Consider breaking it into multiple lines for better readability.",
                            i + 1,
                            line.len()
                        ));
                    }
                }
            }
        }
    }

    warnings
}
