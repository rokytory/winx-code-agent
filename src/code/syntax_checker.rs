use anyhow::Result;
use std::sync::Arc;
use tracing::debug;
use tree_sitter::{Language, Parser, Tree};

/// Result of syntax validation
#[derive(Debug, Clone)]
pub struct SyntaxValidationResult {
    /// Whether the syntax is valid
    pub is_valid: bool,
    /// List of syntax errors (line, column, message)
    pub errors: Vec<(usize, usize, String)>,
    /// Description of errors in human-readable format
    pub description: String,
}

/// Syntax validator for various programming languages
pub struct SyntaxValidator {
    /// Tree-sitter parsers for different languages
    parsers: std::collections::HashMap<String, (Parser, Language)>,
}

impl SyntaxValidator {
    /// Create a new syntax validator
    pub fn new() -> Result<Self> {
        let mut parsers = std::collections::HashMap::new();

        // Initialize languages that we've added dependencies for
        let mut rust_parser = Parser::new();
        let rust_lang = tree_sitter_rust::language();
        rust_parser.set_language(rust_lang)?;
        parsers.insert("rs".to_string(), (rust_parser, rust_lang));

        let mut js_parser = Parser::new();
        let js_lang = tree_sitter_javascript::language();
        js_parser.set_language(js_lang)?;
        parsers.insert("js".to_string(), (js_parser, js_lang));

        // Create a new parser for JSX since Parser doesn't implement Clone
        let mut jsx_parser = Parser::new();
        jsx_parser.set_language(js_lang)?;
        parsers.insert("jsx".to_string(), (jsx_parser, js_lang));

        let mut py_parser = Parser::new();
        let py_lang = tree_sitter_python::language();
        py_parser.set_language(py_lang)?;
        parsers.insert("py".to_string(), (py_parser, py_lang));

        Ok(Self { parsers })
    }

    /// Validate syntax for a given language extension and content
    pub fn validate(&mut self, extension: &str, content: &str) -> SyntaxValidationResult {
        let extension = extension.trim().to_lowercase();

        // Check if we support this language
        if !self.parsers.contains_key(&extension) {
            debug!("No syntax validator for extension: {}", extension);
            return SyntaxValidationResult {
                is_valid: true, // Assume valid for unsupported languages
                errors: Vec::new(),
                description: format!("No syntax validator available for .{} files", extension),
            };
        }

        // Parse the content
        if let Some((parser, _)) = self.parsers.get_mut(&extension) {
            let tree = parser.parse(content.as_bytes(), None);

            match tree {
                Some(tree) => {
                    self.analyze_syntax_errors(&tree, &extension)
                }
                None => {
                    SyntaxValidationResult {
                        is_valid: false,
                        errors: vec![(1, 1, "Failed to parse content".to_string())],
                        description: "Parsing failed - the syntax may be severely malformed".to_string(),
                    }
                }
            }
        } else {
            // Should never happen due to earlier check
            SyntaxValidationResult {
                is_valid: true,
                errors: Vec::new(),
                description: "No syntax validation performed".to_string(),
            }
        }
    }

    /// Analyze a syntax tree to find errors
    fn analyze_syntax_errors(&self, tree: &Tree, extension: &str) -> SyntaxValidationResult {
        // Tree-sitter doesn't directly report syntax errors
        // We need to look for ERROR nodes in the syntax tree

        // Get the root node
        let root_node = tree.root_node();

        // Check if the root node has an error
        let has_error = root_node.is_error() || root_node.has_error();

        if !has_error {
            return SyntaxValidationResult {
                is_valid: true,
                errors: Vec::new(),
                description: "Syntax is valid".to_string(),
            };
        }

        // Versão simplificada para evitar problemas de lifetime
        // Verifica apenas o nó raiz
        let mut syntax_errors = Vec::new();
        if root_node.is_error() || root_node.has_error() {
            let start = root_node.start_position();
            syntax_errors.push((
                start.row + 1, // Convert to 1-indexed for user display
                start.column + 1,
                format!("Syntax error at line {}, column {}", start.row + 1, start.column + 1),
            ));
        }

        // Build description
        let description = if syntax_errors.is_empty() {
            format!("Partial syntax errors detected in {} code", extension)
        } else {
            let mut desc = format!("Found {} syntax errors in {} code:\n", syntax_errors.len(), extension);
            for (i, (line, col, _)) in syntax_errors.iter().enumerate().take(5) {
                desc.push_str(&format!("- Error #{}: line {}, column {}\n", i + 1, line, col));
            }
            if syntax_errors.len() > 5 {
                desc.push_str(&format!("- ... and {} more errors\n", syntax_errors.len() - 5));
            }
            desc
        };

        SyntaxValidationResult {
            is_valid: false,
            errors: syntax_errors,
            description,
        }
    }
}

// Use once_cell to provide a shared instance
use once_cell::sync::OnceCell;
static SYNTAX_VALIDATOR: OnceCell<Arc<std::sync::Mutex<SyntaxValidator>>> = OnceCell::new();

/// Get the shared syntax validator instance
pub fn get_syntax_validator() -> Result<Arc<std::sync::Mutex<SyntaxValidator>>> {
    SYNTAX_VALIDATOR
        .get_or_try_init(|| {
            let validator = SyntaxValidator::new()?;
            Ok(Arc::new(std::sync::Mutex::new(validator)))
        })
        .cloned()
}

/// Validate syntax for a file
pub fn validate_syntax(extension: &str, content: &str) -> Result<SyntaxValidationResult> {
    let validator = get_syntax_validator()?;
    let mut validator_guard = validator.lock().unwrap();
    Ok(validator_guard.validate(extension, content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_syntax_validation() {
        let mut validator = SyntaxValidator::new().unwrap();

        // Valid Rust code
        let valid_code = r#"
fn main() {
    println!("Hello, world!");
}
"#;
        let result = validator.validate("rs", valid_code);
        assert!(result.is_valid);
        assert!(result.errors.is_empty());

        // Invalid Rust code
        let invalid_code = r#"
fn main() {
    println!("Unterminated string
}
"#;
        let result = validator.validate("rs", invalid_code);
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }
}