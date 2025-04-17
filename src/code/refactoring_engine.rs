use crate::code::semantic_analyzer::{
    RefactoringSeverity, RefactoringSuggestion, SemanticAnalyzer,
};
use crate::lsp::types::{Position, Range, Symbol, SymbolKind};
use anyhow::{anyhow, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Patterns for code smells
#[derive(Debug, Clone)]
pub enum CodeSmell {
    /// Method or function is too long
    LongMethod,
    /// Class has too many methods
    GodClass,
    /// Method has too many parameters
    LongParameterList,
    /// Method has high cyclomatic complexity
    HighComplexity,
    /// Code duplication
    Duplication,
    /// Primitive obsession (using primitive types instead of objects)
    PrimitiveObsession,
    /// Feature envy (method uses more members from another class than its own)
    FeatureEnvy,
    /// Data class (class with only fields and accessors)
    DataClass,
    /// Switch statement (could use polymorphism)
    SwitchStatement,
    /// Refused bequest (subclass doesn't use inherited methods)
    RefusedBequest,
}

impl CodeSmell {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            CodeSmell::LongMethod => "Method is too long",
            CodeSmell::GodClass => "Class has too many responsibilities",
            CodeSmell::LongParameterList => "Method has too many parameters",
            CodeSmell::HighComplexity => "Method has high cyclomatic complexity",
            CodeSmell::Duplication => "Code duplication detected",
            CodeSmell::PrimitiveObsession => {
                "Primitive types used where an object would be more appropriate"
            }
            CodeSmell::FeatureEnvy => "Method uses more features from another class than its own",
            CodeSmell::DataClass => "Class only contains data with no behavior",
            CodeSmell::SwitchStatement => "Switch statement could be replaced with polymorphism",
            CodeSmell::RefusedBequest => "Subclass doesn't use inherited methods",
        }
    }
}

/// A suggestion for a refactoring operation
#[derive(Debug, Clone)]
pub struct RefactoringOperation {
    /// The type of refactoring to perform
    pub operation_type: RefactoringType,
    /// The source file path
    pub file_path: PathBuf,
    /// Range in the source file
    pub source_range: Range,
    /// Target file path (for Extract Class, etc.)
    pub target_file_path: Option<PathBuf>,
    /// New name (for Rename)
    pub new_name: Option<String>,
    /// Parameters to extract (for Extract Parameter)
    pub parameters: Option<Vec<String>>,
    /// Methods to move (for Move Method)
    pub methods: Option<Vec<String>>,
    /// Description of the refactoring
    pub description: String,
    /// Reason for the refactoring
    pub reason: String,
    /// Severity
    pub severity: RefactoringSeverity,
}

/// Types of refactoring operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefactoringType {
    /// Extract a method from a complicated method
    ExtractMethod,
    /// Extract a class from a God class
    ExtractClass,
    /// Extract an interface
    ExtractInterface,
    /// Move a method to another class
    MoveMethod,
    /// Rename a symbol
    Rename,
    /// Introduce a parameter object
    IntroduceParameterObject,
    /// Replace conditional with polymorphism
    ReplaceConditional,
    /// Inline a temporary variable
    InlineTemp,
    /// Introduce a factory
    IntroduceFactory,
    /// Pull up a method to a superclass
    PullUpMethod,
}

impl RefactoringType {
    /// Get a human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            RefactoringType::ExtractMethod => "Extract Method",
            RefactoringType::ExtractClass => "Extract Class",
            RefactoringType::ExtractInterface => "Extract Interface",
            RefactoringType::MoveMethod => "Move Method",
            RefactoringType::Rename => "Rename",
            RefactoringType::IntroduceParameterObject => "Introduce Parameter Object",
            RefactoringType::ReplaceConditional => "Replace Conditional with Polymorphism",
            RefactoringType::InlineTemp => "Inline Temporary Variable",
            RefactoringType::IntroduceFactory => "Introduce Factory",
            RefactoringType::PullUpMethod => "Pull Up Method",
        }
    }
}

/// Engine for suggesting and applying refactorings
pub struct RefactoringEngine {
    semantic_analyzer: Option<Arc<SemanticAnalyzer>>,
    workspace_path: PathBuf,
    pending_operations: Vec<RefactoringOperation>,
    applied_operations: Vec<RefactoringOperation>,
}

impl RefactoringEngine {
    /// Create a new refactoring engine
    pub fn new() -> Self {
        Self {
            semantic_analyzer: None,
            workspace_path: PathBuf::new(),
            pending_operations: Vec::new(),
            applied_operations: Vec::new(),
        }
    }
    
    /// Initialize the refactoring engine with project analysis
    pub fn initialize(&mut self, project_analysis: &crate::code::project_analyzer::ProjectAnalysis) -> Result<()> {
        self.workspace_path = project_analysis.root_dir.clone();
        info!("Initialized refactoring engine for workspace: {}", self.workspace_path.display());
        Ok(())
    }
    
    /// Create a new refactoring engine with semantic analyzer
    pub fn new_with_analyzer(semantic_analyzer: Arc<SemanticAnalyzer>, workspace_path: impl AsRef<Path>) -> Self {
        Self {
            semantic_analyzer: Some(semantic_analyzer),
            workspace_path: workspace_path.as_ref().to_path_buf(),
            pending_operations: Vec::new(),
            applied_operations: Vec::new(),
        }
    }

    /// Analyze a file for potential refactorings
    pub async fn analyze_file(
        &self,
        file_path: impl AsRef<Path>,
    ) -> Result<Vec<RefactoringOperation>> {
        let file_path = file_path.as_ref();
        info!(
            "Analyzing file for refactoring opportunities: {}",
            file_path.display()
        );

        let mut refactorings = Vec::new();

        // Get complexity report
        let complexity_report = match &self.semantic_analyzer {
            Some(analyzer) => analyzer.analyze_complexity(file_path).await?,
            None => return Err(anyhow!("Semantic analyzer not initialized")),
        };

        // Check for long methods
        for func in &complexity_report.function_metrics {
            if func.line_count > 50 {
                // Get the symbol for this function
                if let Some(symbol) = self.find_symbol_by_name(file_path, &func.name).await? {
                    refactorings.push(RefactoringOperation {
                        operation_type: RefactoringType::ExtractMethod,
                        file_path: file_path.to_path_buf(),
                        source_range: symbol.range,
                        target_file_path: None,
                        new_name: None,
                        parameters: None,
                        methods: None,
                        description: format!("Extract method from '{}'", func.name),
                        reason: format!("Method is too long ({} lines)", func.line_count),
                        severity: if func.line_count > 100 {
                            RefactoringSeverity::High
                        } else {
                            RefactoringSeverity::Medium
                        },
                    });
                }
            }

            // Check for high complexity
            if func.cyclomatic_complexity > 10 {
                if let Some(symbol) = self.find_symbol_by_name(file_path, &func.name).await? {
                    refactorings.push(RefactoringOperation {
                        operation_type: RefactoringType::ExtractMethod,
                        file_path: file_path.to_path_buf(),
                        source_range: symbol.range,
                        target_file_path: None,
                        new_name: None,
                        parameters: None,
                        methods: None,
                        description: format!("Extract method from '{}'", func.name),
                        reason: format!(
                            "Method has high cyclomatic complexity ({})",
                            func.cyclomatic_complexity
                        ),
                        severity: if func.cyclomatic_complexity > 15 {
                            RefactoringSeverity::High
                        } else {
                            RefactoringSeverity::Medium
                        },
                    });
                }
            }

            // Check for long parameter list
            if func.parameter_count > 5 {
                if let Some(symbol) = self.find_symbol_by_name(file_path, &func.name).await? {
                    refactorings.push(RefactoringOperation {
                        operation_type: RefactoringType::IntroduceParameterObject,
                        file_path: file_path.to_path_buf(),
                        source_range: symbol.range,
                        target_file_path: None,
                        new_name: None,
                        parameters: None,
                        methods: None,
                        description: format!("Introduce parameter object for '{}'", func.name),
                        reason: format!(
                            "Method has too many parameters ({})",
                            func.parameter_count
                        ),
                        severity: if func.parameter_count > 8 {
                            RefactoringSeverity::High
                        } else {
                            RefactoringSeverity::Medium
                        },
                    });
                }
            }
        }

        // Check for God classes
        for class in &complexity_report.class_metrics {
            if class.method_count > 15 {
                if let Some(symbol) = self.find_symbol_by_name(file_path, &class.name).await? {
                    refactorings.push(RefactoringOperation {
                        operation_type: RefactoringType::ExtractClass,
                        file_path: file_path.to_path_buf(),
                        source_range: symbol.range,
                        target_file_path: None,
                        new_name: None,
                        parameters: None,
                        methods: None,
                        description: format!("Extract class from '{}'", class.name),
                        reason: format!("Class has too many methods ({})", class.method_count),
                        severity: if class.method_count > 25 {
                            RefactoringSeverity::High
                        } else {
                            RefactoringSeverity::Medium
                        },
                    });
                }
            }
        }

        // Check for duplicated code
        self.detect_code_duplication(file_path, &mut refactorings)
            .await?;

        Ok(refactorings)
    }

    /// Find a symbol by name in a file
    async fn find_symbol_by_name(&self, file_path: &Path, name: &str) -> Result<Option<Symbol>> {
        let symbols = self
            .semantic_analyzer
            .as_ref()
            .ok_or_else(|| anyhow!("Semantic analyzer not initialized"))?
            .get_lsp_client()
            .get_document_symbols(file_path, true)
            .await
            .context("Failed to get document symbols")?;

        // Find the symbol with matching name
        for symbol in symbols {
            if symbol.name == name {
                return Ok(Some(symbol));
            }

            // Check children (for methods in classes)
            for child in &symbol.children {
                if child.name == name {
                    return Ok(Some(child.clone()));
                }
            }
        }

        Ok(None)
    }

    /// Detect duplicated code
    async fn detect_code_duplication(
        &self,
        file_path: &Path,
        refactorings: &mut Vec<RefactoringOperation>,
    ) -> Result<()> {
        // This is a simplified implementation - real duplicate detection
        // would use more sophisticated algorithms like AST comparison or fingerprinting

        // Read the file content
        let content = std::fs::read_to_string(file_path).context("Failed to read file")?;

        let lines: Vec<&str> = content.lines().collect();

        // Very basic fingerprinting for each block of code (5 lines)
        let block_size = 5;
        let mut fingerprints = HashMap::new();

        for i in 0..(lines.len().saturating_sub(block_size) + 1) {
            let block = &lines[i..(i + block_size.min(lines.len() - i))];
            let normalized = normalize_block(block);

            fingerprints
                .entry(normalized)
                .or_insert_with(Vec::new)
                .push((i, i + block_size.min(lines.len() - i)));
        }

        // Check for duplicates
        for (_, positions) in fingerprints {
            if positions.len() > 1 {
                // Create refactoring suggestion
                let first_pos = positions[0];

                // Convert to Range
                let range = Range {
                    start: Position {
                        line: first_pos.0,
                        character: 0,
                    },
                    end: Position {
                        line: first_pos.1,
                        character: lines[first_pos.1.saturating_sub(1)].len(),
                    },
                };

                refactorings.push(RefactoringOperation {
                    operation_type: RefactoringType::ExtractMethod,
                    file_path: file_path.to_path_buf(),
                    source_range: range,
                    target_file_path: None,
                    new_name: None,
                    parameters: None,
                    methods: None,
                    description: "Extract method for duplicated code".to_string(),
                    reason: format!("Code block duplicated {} times", positions.len()),
                    severity: if positions.len() > 3 {
                        RefactoringSeverity::High
                    } else {
                        RefactoringSeverity::Medium
                    },
                });

                // Limit to avoid too many suggestions
                if refactorings.len() > 20 {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Apply a refactoring operation
    pub async fn apply_refactoring(&self, operation: &RefactoringOperation) -> Result<String> {
        match operation.operation_type {
            RefactoringType::ExtractMethod => self.extract_method(operation).await,
            RefactoringType::IntroduceParameterObject => {
                self.introduce_parameter_object(operation).await
            }
            RefactoringType::ExtractClass => self.extract_class(operation).await,
            // Other refactorings would be implemented similarly
            _ => Err(anyhow::anyhow!(
                "Refactoring type not implemented yet: {}",
                operation.operation_type.name()
            )),
        }
    }

    /// Extract a method
    async fn extract_method(&self, operation: &RefactoringOperation) -> Result<String> {
        // Read file content
        let content =
            std::fs::read_to_string(&operation.file_path).context("Failed to read file")?;

        let lines: Vec<&str> = content.lines().collect();

        // Extract the code to be refactored
        let start_line = operation.source_range.start.line;
        let end_line = operation.source_range.end.line;

        // Get method context (indentation, etc.)
        let indentation = if start_line > 0 {
            get_indentation(lines[start_line])
        } else {
            ""
        };

        let additional_indent = "    "; // Default indentation for extracted method

        // Generate a method name
        let method_name = operation
            .new_name
            .clone()
            .unwrap_or_else(|| "extracted_method".to_string());

        // Create method signature - this would need more sophisticated analysis in a real implementation
        let method_signature = format!("{}fn {}() -> Result<()> {{", indentation, method_name);

        // Create method body
        let mut method_body = Vec::new();
        method_body.push(method_signature);

        for i in start_line..=end_line {
            if i < lines.len() {
                let line_indent = get_indentation(lines[i]);
                let additional = if line_indent.starts_with(indentation) {
                    &line_indent[indentation.len()..]
                } else {
                    ""
                };

                method_body.push(format!(
                    "{}{}{}{}",
                    indentation,
                    additional_indent,
                    additional,
                    lines[i].trim_start()
                ));
            }
        }

        method_body.push(format!("{}}}", indentation));

        // Return a preview of the extracted method
        Ok(method_body.join("\n"))
    }

    /// Extract a class
    async fn extract_class(&self, operation: &RefactoringOperation) -> Result<String> {
        // This would require more sophisticated analysis in a real implementation
        // For now, just provide a template

        let class_name = operation
            .new_name
            .clone()
            .unwrap_or_else(|| "ExtractedClass".to_string());

        let template = format!(
            r#"pub struct {} {{
    // TODO: Add fields
}}

impl {} {{
    pub fn new() -> Self {{
        Self {{
            // TODO: Initialize fields
        }}
    }}
    
    // TODO: Add methods
}}
"#,
            class_name, class_name
        );

        Ok(template)
    }

    /// Introduce a parameter object
    async fn introduce_parameter_object(&self, operation: &RefactoringOperation) -> Result<String> {
        // This would require more sophisticated analysis in a real implementation
        // For now, just provide a template

        let class_name = operation
            .new_name
            .clone()
            .unwrap_or_else(|| "Parameters".to_string());

        let template = format!(
            r#"pub struct {} {{
    // TODO: Add fields for parameters
}}

impl {} {{
    pub fn new(/* TODO: Add parameters */) -> Self {{
        Self {{
            // TODO: Initialize fields
        }}
    }}
}}

// Example usage:
// fn original_method({} params) {{
//     // Use params.field1, params.field2, etc.
// }}
"#,
            class_name,
            class_name,
            class_name.to_lowercase()
        );

        Ok(template)
    }
}

/// Normalize a block of code for comparison (simplified)
fn normalize_block(block: &[&str]) -> String {
    block
        .iter()
        .map(|line| {
            // Remove indentation, comments and trim
            let line = line.trim();
            if let Some(idx) = line.find("//") {
                &line[..idx]
            } else {
                line
            }
        })
        .filter(|line| !line.is_empty()) // Remove empty lines
        .collect::<Vec<_>>()
        .join("\n")
}

/// Get the indentation of a line
fn get_indentation(line: &str) -> &str {
    let mut end = 0;
    for (i, c) in line.chars().enumerate() {
        if !c.is_whitespace() {
            break;
        }
        end = i + 1;
    }
    &line[..end]
}
