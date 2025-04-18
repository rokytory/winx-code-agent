use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, warn};

// Conditional compilation for tree-sitter functionality
#[cfg(feature = "syntax_validation")]
use tree_sitter::{Language, Parser, Tree};

// External FFI declarations for tree-sitter language parsers
// Only include these when syntax validation is enabled
#[cfg(feature = "syntax_validation")]
extern "C" {
    fn tree_sitter_rust() -> Language;
    fn tree_sitter_javascript() -> Language;
    fn tree_sitter_python() -> Language;
}

// Set to false to disable syntax validation at runtime
static SYNTAX_VALIDATION_ENABLED: bool = false;

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

/// Syntax validator interface
pub struct SyntaxValidator {
    #[cfg(feature = "syntax_validation")]
    parsers: std::collections::HashMap<String, Parser>,

    #[cfg(not(feature = "syntax_validation"))]
    _dummy: bool, // Just to have a field when tree-sitter is disabled
}

impl SyntaxValidator {
    /// Create a new syntax validator
    pub fn new() -> Result<Self> {
        // Early return if syntax validation is disabled
        if !SYNTAX_VALIDATION_ENABLED {
            debug!("Syntax validation is disabled in this build");
            return Ok(Self::create_dummy());
        }

        #[cfg(feature = "syntax_validation")]
        {
            let mut parsers = std::collections::HashMap::new();

            // Initialize languages with robust error handling
            // Use a safe flag to track if tree-sitter libraries are available
            let mut tree_sitter_libs_available = true;

            // Try to initialize Rust parser
            if let Err(e) =
                Self::try_init_parser(&mut parsers, "rs", || unsafe { tree_sitter_rust() })
            {
                debug!("Failed to initialize Rust parser: {}", e);
                tree_sitter_libs_available = false;
            }

            // Only try other parsers if first one succeeded
            if tree_sitter_libs_available {
                // Try JavaScript
                if let Err(e) = Self::try_init_parser(&mut parsers, "js", || unsafe {
                    tree_sitter_javascript()
                }) {
                    debug!("Failed to initialize JavaScript parser: {}", e);
                } else {
                    // JSX uses same parser as JavaScript
                    if let Err(e) = Self::try_init_parser(&mut parsers, "jsx", || unsafe {
                        tree_sitter_javascript()
                    }) {
                        debug!("Failed to initialize JSX parser: {}", e);
                    }
                }

                // Try Python
                if let Err(e) =
                    Self::try_init_parser(&mut parsers, "py", || unsafe { tree_sitter_python() })
                {
                    debug!("Failed to initialize Python parser: {}", e);
                }
            }

            if parsers.is_empty() {
                debug!(
                    "No syntax parsers could be initialized - syntax validation will be disabled"
                );
            } else {
                let parser_names: Vec<String> = parsers.keys().cloned().collect();
                debug!(
                    "Initialized syntax parsers for: {}",
                    parser_names.join(", ")
                );
            }

            Ok(Self { parsers })
        }

        #[cfg(not(feature = "syntax_validation"))]
        {
            return Ok(Self::create_dummy());
        }
    }

    /// Create a dummy validator when syntax validation is disabled
    #[cfg(not(feature = "syntax_validation"))]
    fn create_dummy() -> Self {
        Self { _dummy: false }
    }

    /// Create a dummy validator when syntax validation is disabled
    #[cfg(feature = "syntax_validation")]
    fn create_dummy() -> Self {
        Self {
            parsers: std::collections::HashMap::new(),
        }
    }

    /// Helper to safely try to initialize a parser
    #[cfg(feature = "syntax_validation")]
    fn try_init_parser<F>(
        parsers: &mut std::collections::HashMap<String, Parser>,
        extension: &str,
        get_lang: F,
    ) -> Result<()>
    where
        F: FnOnce() -> Language,
    {
        let mut parser = Parser::new();
        let lang = (get_lang)();
        parser.set_language(&lang)?;
        parsers.insert(extension.to_string(), parser);
        Ok(())
    }

    /// Validate syntax for a given language extension and content
    pub fn validate(&mut self, extension: &str, content: &str) -> SyntaxValidationResult {
        // If syntax validation is disabled, always return valid
        warn!(
            extension = extension,
            "Validating syntax for extension: {}", extension
        );
        warn!(
            content = content,
            "Validating syntax for content: {}", content
        );
        if !SYNTAX_VALIDATION_ENABLED {
            return SyntaxValidationResult {
                is_valid: true,
                errors: Vec::new(),
                description: "Syntax validation is disabled".to_string(),
            };
        }

        #[cfg(feature = "syntax_validation")]
        {
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
            if let Some(parser) = self.parsers.get_mut(&extension) {
                let tree = parser.parse(content.as_bytes(), None);

                match tree {
                    Some(tree) => self.analyze_syntax_errors(&tree, &extension),
                    None => SyntaxValidationResult {
                        is_valid: false,
                        errors: vec![(1, 1, "Failed to parse content".to_string())],
                        description: "Parsing failed - the syntax may be severely malformed"
                            .to_string(),
                    },
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

        #[cfg(not(feature = "syntax_validation"))]
        {
            SyntaxValidationResult {
                is_valid: true,
                errors: Vec::new(),
                description: "Syntax validation is not available in this build".to_string(),
            }
        }
    }

    /// Analyze a syntax tree to find errors
    #[cfg(feature = "syntax_validation")]
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
                format!(
                    "Syntax error at line {}, column {}",
                    start.row + 1,
                    start.column + 1
                ),
            ));
        }

        // Build description
        let description = if syntax_errors.is_empty() {
            format!("Partial syntax errors detected in {} code", extension)
        } else {
            let mut desc = format!(
                "Found {} syntax errors in {} code:\n",
                syntax_errors.len(),
                extension
            );
            for (i, (line, col, _)) in syntax_errors.iter().enumerate().take(5) {
                desc.push_str(&format!(
                    "- Error #{}: line {}, column {}\n",
                    i + 1,
                    line,
                    col
                ));
            }
            if syntax_errors.len() > 5 {
                desc.push_str(&format!(
                    "- ... and {} more errors\n",
                    syntax_errors.len() - 5
                ));
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
// OnceCell import removed, as it's not used

/// Get the shared syntax validator instance
pub fn get_syntax_validator() -> Result<Arc<std::sync::Mutex<SyntaxValidator>>> {
    // If syntax validation is disabled, return a dummy validator
    if !SYNTAX_VALIDATION_ENABLED {
        warn!("Syntax validation is disabled. Using dummy validator.");
        return Ok(Arc::new(std::sync::Mutex::new(
            SyntaxValidator::create_dummy(),
        )));
    }

    // Try to initialize a syntax validator
    let validator_arc = Arc::new(std::sync::Mutex::new(
        SyntaxValidator::new().unwrap_or_else(|e| {
            debug!(
                "Failed to initialize syntax validator: {}. Using dummy validator.",
                e
            );
            SyntaxValidator::create_dummy()
        }),
    ));

    Ok(validator_arc)
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
