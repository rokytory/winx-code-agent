use crate::lsp::client::LSPClient;
use crate::lsp::types::{Symbol, SymbolKind, SymbolLocation};
use anyhow::{anyhow, Context, Result};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

/// Result of a semantic analysis operation
#[derive(Debug, Clone)]
pub struct SemanticAnalysisResult {
    /// Symbols found in the analysis
    pub symbols: Vec<Symbol>,
    /// Relationships between symbols
    pub relationships: Vec<SymbolRelationship>,
    /// Analysis timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Relationship between symbols
#[derive(Debug, Clone)]
pub enum SymbolRelationship {
    /// Symbol A calls symbol B
    Calls { caller: String, callee: String },
    /// Symbol A imports symbol B
    Imports { importer: String, importee: String },
    /// Symbol A inherits from symbol B
    Inherits { child: String, parent: String },
    /// Symbol A implements interface B
    Implements {
        implementer: String,
        interface: String,
    },
    /// Symbol A uses symbol B
    Uses { user: String, used: String },
}

/// Module dependency information
#[derive(Debug, Clone)]
pub struct ModuleAnalysis {
    /// Path to the module file
    pub path: PathBuf,
    /// Imports declared in the module
    pub imports: Vec<Symbol>,
    /// Symbols exported by the module
    pub exports: Vec<Symbol>,
    /// Modules this module depends on
    pub dependencies: Vec<PathBuf>,
    /// Modules that depend on this module
    pub dependents: Vec<PathBuf>,
}

/// Dependency graph for a project
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Map from module path to ModuleAnalysis
    modules: HashMap<PathBuf, ModuleAnalysis>,
    /// Map from symbol name to module path
    symbol_to_module: HashMap<String, PathBuf>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            symbol_to_module: HashMap::new(),
        }
    }

    /// Add a module to the graph
    pub fn add_module(&mut self, module: ModuleAnalysis) {
        // Register symbol names
        for symbol in &module.exports {
            self.symbol_to_module
                .insert(symbol.name.clone(), module.path.clone());
        }

        // Add to modules
        self.modules.insert(module.path.clone(), module);
    }

    /// Find module containing a symbol
    pub fn find_module_for_symbol(&self, symbol_name: &str) -> Option<&ModuleAnalysis> {
        self.symbol_to_module
            .get(symbol_name)
            .and_then(|path| self.modules.get(path))
    }

    /// Get all modules in the graph
    pub fn get_all_modules(&self) -> impl Iterator<Item = &ModuleAnalysis> {
        self.modules.values()
    }

    /// Find modules affected by changes to a specific module
    pub fn find_affected_modules(&self, changed_path: &Path) -> Vec<PathBuf> {
        let mut affected = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start with the changed module
        queue.push_back(changed_path.to_path_buf());
        visited.insert(changed_path.to_path_buf());

        while let Some(module_path) = queue.pop_front() {
            // Find modules that depend on this module
            for (path, module) in &self.modules {
                if module.dependencies.contains(&module_path) && !visited.contains(path) {
                    affected.push(path.clone());
                    queue.push_back(path.clone());
                    visited.insert(path.clone());
                }
            }
        }

        affected
    }
}

/// Semantic analyzer for code analysis
#[derive(Clone)]
pub struct SemanticAnalyzer {
    lsp_client: Option<Arc<LSPClient>>,
    workspace_path: PathBuf,
    dependency_graph: Arc<Mutex<DependencyGraph>>,
}

impl SemanticAnalyzer {
    /// Get access to the LSP client
    pub fn get_lsp_client(&self) -> Option<Arc<LSPClient>> {
        self.lsp_client.clone()
    }

    /// Create a new semantic analyzer without LSP client
    pub fn new() -> Self {
        Self {
            lsp_client: None,
            workspace_path: PathBuf::new(),
            dependency_graph: Arc::new(Mutex::new(DependencyGraph::new())),
        }
    }

    /// Initialize the semantic analyzer with project analysis
    pub async fn initialize(
        &mut self,
        project_analysis: &crate::code::project_analyzer::ProjectAnalysis,
    ) -> Result<()> {
        self.workspace_path = project_analysis.root_dir.clone();
        info!(
            "Initialized semantic analyzer for workspace: {}",
            self.workspace_path.display()
        );
        Ok(())
    }

    /// Create a new semantic analyzer with LSP client
    pub fn new_with_lsp(lsp_client: Arc<LSPClient>, workspace_path: impl AsRef<Path>) -> Self {
        Self {
            lsp_client: Some(lsp_client),
            workspace_path: workspace_path.as_ref().to_path_buf(),
            dependency_graph: Arc::new(Mutex::new(DependencyGraph::new())),
        }
    }

    /// Analyze a module to extract dependencies and exports
    pub async fn analyze_module(&self, module_path: impl AsRef<Path>) -> Result<ModuleAnalysis> {
        let module_path = module_path.as_ref();
        info!("Analyzing module: {}", module_path.display());

        // Ensure the file is opened in the LSP client
        if let Some(client) = &self.lsp_client {
            client.open_file(module_path).await?;
        } else {
            return Err(anyhow!("LSP client not initialized"));
        }

        // Get document symbols
        let symbols = if let Some(client) = &self.lsp_client {
            client
                .get_document_symbols(module_path, true)
                .await
                .context("Failed to get document symbols")?
        } else {
            return Err(anyhow!("LSP client not initialized"));
        };

        // Extract imports - we'll use Variable symbols with special naming patterns to detect imports
        // as the LSP spec doesn't have a specific Import symbol kind
        let imports = symbols
            .iter()
            .filter(|s| {
                // For simplicity, we'll consider patterns like "import", "require", "use" etc.
                // as potential indicators of import statements
                s.kind == SymbolKind::Variable
                    && (s.name.contains("import")
                        || s.name.contains("require")
                        || s.name.starts_with("use"))
            })
            .cloned()
            .collect::<Vec<_>>();

        // Consider functions, classes, interfaces, constants, etc. as potential exports
        let exports = symbols
            .iter()
            .filter(|s| {
                matches!(
                    s.kind,
                    SymbolKind::Function
                        | SymbolKind::Class
                        | SymbolKind::Interface
                        | SymbolKind::Constant
                        | SymbolKind::Enum
                        | SymbolKind::Struct
                )
            })
            .cloned()
            .collect::<Vec<_>>();

        // Attempt to resolve import paths to actual module files
        let mut dependencies = Vec::new();
        for import in &imports {
            if let Some(import_path) = self.resolve_import_path(import, module_path)? {
                dependencies.push(import_path);
            }
        }

        // Create module analysis
        let analysis = ModuleAnalysis {
            path: module_path.to_path_buf(),
            imports,
            exports,
            dependencies,
            dependents: Vec::new(), // Will be filled in when building the full graph
        };

        // Update the dependency graph
        {
            let mut graph = self.dependency_graph.lock().unwrap();
            graph.add_module(analysis.clone());
        }

        Ok(analysis)
    }

    /// Resolve an import to a file path
    fn resolve_import_path(
        &self,
        import: &Symbol,
        current_module: &Path,
    ) -> Result<Option<PathBuf>> {
        // This is a simplified implementation - in a real system, we would need
        // language-specific logic to resolve imports based on the language's module system

        // Extract the import path from the symbol name
        let import_name = &import.name;

        // Remove quotes if present
        let import_path = import_name
            .trim_start_matches('"')
            .trim_end_matches('"')
            .trim_start_matches('\'')
            .trim_end_matches('\'');

        // Check if it's a relative import
        if import_path.starts_with("./") || import_path.starts_with("../") {
            // Resolve relative to the current module
            let parent_dir = current_module.parent().ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to get parent directory of {}",
                    current_module.display()
                )
            })?;

            let resolved_path = parent_dir.join(import_path);

            // Check if the path exists, either directly or with common extensions
            if resolved_path.exists() {
                return Ok(Some(resolved_path));
            }

            // Try with common extensions
            for ext in &[".rs", ".js", ".py", ".ts", ".jsx", ".tsx"] {
                let with_ext = resolved_path.with_extension(ext.trim_start_matches('.'));
                if with_ext.exists() {
                    return Ok(Some(with_ext));
                }
            }

            // If no exact match, check for directory with index file
            if resolved_path.is_dir() {
                for index_file in &[
                    "index.js",
                    "index.ts",
                    "index.jsx",
                    "index.tsx",
                    "mod.rs",
                    "__init__.py",
                ] {
                    let index_path = resolved_path.join(index_file);
                    if index_path.exists() {
                        return Ok(Some(index_path));
                    }
                }
            }

            // Not found but still a valid path reference
            debug!(
                "Import path {} from {} could not be resolved to an existing file",
                import_path,
                current_module.display()
            );
            return Ok(None);
        } else {
            // For absolute/library imports, we would need more project-specific logic
            // For now, just log that we can't resolve it
            debug!(
                "Absolute import {} cannot be resolved without project-specific rules",
                import_path
            );
            Ok(None)
        }
    }

    /// Build a dependency graph for the entire project
    pub async fn build_dependency_graph(&self, start_paths: &[PathBuf]) -> Result<DependencyGraph> {
        info!(
            "Building dependency graph starting from {} paths",
            start_paths.len()
        );

        // Clear existing graph
        {
            let mut graph = self.dependency_graph.lock().unwrap();
            *graph = DependencyGraph::new();
        }

        // Track visited modules to avoid cycles
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start with provided paths
        for path in start_paths {
            queue.push_back(path.clone());
        }

        // Process queue
        while let Some(module_path) = queue.pop_front() {
            if !visited.insert(module_path.clone()) {
                continue; // Already visited
            }

            // Analyze the module
            match self.analyze_module(&module_path).await {
                Ok(module) => {
                    // Add dependencies to the queue
                    for dep in &module.dependencies {
                        if !visited.contains(dep) {
                            queue.push_back(dep.clone());
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to analyze module {}: {}", module_path.display(), e);
                }
            }
        }

        // Now update dependents
        let graph_clone = {
            let graph = self.dependency_graph.lock().unwrap();
            graph.clone()
        };

        let mut updated_graph = graph_clone;

        // For each module, update its dependents
        // We need to avoid borrowing the same map mutably more than once at a time
        let mut dependents_updates: Vec<(PathBuf, PathBuf)> = Vec::new();

        // First collect all dependency relationships
        for (path, module) in &updated_graph.modules {
            for dep_path in &module.dependencies {
                dependents_updates.push((dep_path.clone(), path.clone()));
            }
        }

        // Then apply the updates
        for (dep_path, dependent_path) in dependents_updates {
            if let Some(dep_module) = updated_graph.modules.get_mut(&dep_path) {
                dep_module.dependents.push(dependent_path);
            }
        }

        Ok(updated_graph)
    }

    /// Find symbols that match a specific pattern
    pub async fn find_symbols(
        &self,
        pattern: &str,
        symbol_kinds: &[SymbolKind],
    ) -> Result<Vec<Symbol>> {
        info!(
            "Searching for symbols matching {} of kinds {:?}",
            pattern, symbol_kinds
        );

        // Use the LSP client to find symbols
        let mut results = if let Some(client) = &self.lsp_client {
            client
                .find_symbol(pattern, None::<PathBuf>, true)
                .await
                .context("Failed to find symbols")?
        } else {
            return Err(anyhow!("LSP client not initialized"));
        };

        // Filter by requested kinds if provided
        if !symbol_kinds.is_empty() {
            results.retain(|s| symbol_kinds.contains(&s.kind));
        }

        Ok(results)
    }

    /// Find all usages of a symbol in the workspace
    pub async fn find_all_usages(&self, symbol_name: &str) -> Result<Vec<SymbolLocation>> {
        info!("Finding all usages of symbol {}", symbol_name);

        // First find the symbol definition
        let symbols = self.find_symbols(symbol_name, &[]).await?;

        if symbols.is_empty() {
            return Ok(Vec::new());
        }

        // For each definition, find references
        let mut all_locations = Vec::new();

        for symbol in symbols {
            if let Some(loc) = symbol.location.relative_path.as_ref() {
                let location = SymbolLocation {
                    relative_path: Some(loc.clone()),
                    line: symbol.range.start.line.into(),
                    column: symbol.range.start.character.into(),
                };

                let references = if let Some(client) = &self.lsp_client {
                    client
                        .find_references(location, false)
                        .await
                        .context("Failed to find references")?
                } else {
                    Vec::new()
                };

                // Extract locations
                for ref_symbol in references {
                    all_locations.push(ref_symbol.location);
                }
            }
        }

        Ok(all_locations)
    }

    /// Find modules affected by changes to specific files
    pub async fn find_affected_modules(&self, changed_files: &[PathBuf]) -> Result<Vec<PathBuf>> {
        info!(
            "Finding modules affected by changes to {} files",
            changed_files.len()
        );

        let graph = self.dependency_graph.lock().unwrap();

        let mut affected = Vec::new();
        for path in changed_files {
            let path_affected = graph.find_affected_modules(path);
            affected.extend(path_affected);
        }

        // Deduplicate
        affected.sort();
        affected.dedup();

        Ok(affected)
    }

    /// Analyze code complexity for a module
    pub async fn analyze_complexity(
        &self,
        module_path: impl AsRef<Path>,
    ) -> Result<CodeComplexityReport> {
        let module_path = module_path.as_ref();
        info!("Analyzing complexity of module: {}", module_path.display());

        // Get document symbols
        let symbols = if let Some(client) = &self.lsp_client {
            client
                .get_document_symbols(module_path, true)
                .await
                .context("Failed to get document symbols")?
        } else {
            return Err(anyhow!("LSP client not initialized"));
        };

        let mut function_metrics = Vec::new();
        let mut class_metrics = Vec::new();

        // Analyze functions
        for symbol in &symbols {
            match symbol.kind {
                SymbolKind::Function | SymbolKind::Method => {
                    if let Some(body) = &symbol.body {
                        // Calculate metrics
                        let line_count = body.lines().count();
                        let cyclomatic_complexity = Self::calculate_cyclomatic_complexity(body);

                        function_metrics.push(FunctionComplexity {
                            name: symbol.name.clone(),
                            line_count,
                            cyclomatic_complexity,
                            parameter_count: Self::estimate_parameter_count(body),
                        });
                    }
                }
                SymbolKind::Class | SymbolKind::Struct => {
                    if let Some(body) = &symbol.body {
                        // Calculate class metrics
                        let method_count = symbol
                            .children
                            .iter()
                            .filter(|c| c.kind == SymbolKind::Method)
                            .count();

                        let field_count = symbol
                            .children
                            .iter()
                            .filter(|c| matches!(c.kind, SymbolKind::Field | SymbolKind::Property))
                            .count();

                        class_metrics.push(ClassComplexity {
                            name: symbol.name.clone(),
                            method_count,
                            field_count,
                            line_count: body.lines().count(),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(CodeComplexityReport {
            module_path: module_path.to_path_buf(),
            function_metrics,
            class_metrics,
        })
    }

    /// Calculate cyclomatic complexity (simplified)
    fn calculate_cyclomatic_complexity(code: &str) -> usize {
        // A very simple approximation
        let lines = code.lines().collect::<Vec<_>>();

        // Count branching statements
        let mut complexity = 1; // Base complexity

        for line in lines {
            let line = line.trim();

            // Count common branching constructs
            if line.starts_with("if ")
                || line.contains(" if ")
                || line.starts_with("else if ")
                || line.starts_with("while ")
                || line.contains(" while ")
                || line.starts_with("for ")
                || line.contains(" for ")
                || line.contains(" && ")
                || line.contains(" || ")
                || line.contains("switch")
                || line.contains("case ")
                || line.contains("match ")
            {
                complexity += 1;
            }
        }

        complexity
    }

    /// Estimate parameter count for a function
    fn estimate_parameter_count(code: &str) -> usize {
        // Simplistic approximation
        // Look for the first opening and closing parenthesis
        if let (Some(open_paren), Some(close_paren)) = (code.find('('), code.find(')')) {
            if open_paren < close_paren {
                let params = &code[open_paren + 1..close_paren];
                // Count commas and add 1, unless empty
                if params.trim().is_empty() {
                    0
                } else {
                    params.matches(',').count() + 1
                }
            } else {
                0
            }
        } else {
            0
        }
    }
}

/// Report on code complexity
#[derive(Debug, Clone)]
pub struct CodeComplexityReport {
    /// Path to the module
    pub module_path: PathBuf,
    /// Metrics for functions
    pub function_metrics: Vec<FunctionComplexity>,
    /// Metrics for classes
    pub class_metrics: Vec<ClassComplexity>,
}

/// Complexity metrics for a function
#[derive(Debug, Clone)]
pub struct FunctionComplexity {
    /// Function name
    pub name: String,
    /// Number of lines
    pub line_count: usize,
    /// Cyclomatic complexity
    pub cyclomatic_complexity: usize,
    /// Number of parameters
    pub parameter_count: usize,
}

/// Complexity metrics for a class
#[derive(Debug, Clone)]
pub struct ClassComplexity {
    /// Class name
    pub name: String,
    /// Number of methods
    pub method_count: usize,
    /// Number of fields
    pub field_count: usize,
    /// Number of lines
    pub line_count: usize,
}

impl CodeComplexityReport {
    /// Get summary statistics
    pub fn get_summary(&self) -> ComplexitySummary {
        let mut total_lines = 0;
        let mut max_function_complexity = 0;
        let mut complex_functions = 0;

        for func in &self.function_metrics {
            total_lines += func.line_count;

            if func.cyclomatic_complexity > max_function_complexity {
                max_function_complexity = func.cyclomatic_complexity;
            }

            if func.cyclomatic_complexity > 10 {
                complex_functions += 1;
            }
        }

        let avg_method_count = if !self.class_metrics.is_empty() {
            self.class_metrics
                .iter()
                .map(|c| c.method_count)
                .sum::<usize>() as f64
                / self.class_metrics.len() as f64
        } else {
            0.0
        };

        ComplexitySummary {
            total_functions: self.function_metrics.len(),
            total_classes: self.class_metrics.len(),
            total_lines,
            max_function_complexity,
            complex_functions,
            avg_method_count,
        }
    }

    /// Get refactoring suggestions based on complexity
    pub fn get_refactoring_suggestions(&self) -> Vec<RefactoringSuggestion> {
        let mut suggestions = Vec::new();

        // Check for complex functions
        for func in &self.function_metrics {
            if func.cyclomatic_complexity > 15 {
                suggestions.push(RefactoringSuggestion::ExtractMethod {
                    source: func.name.clone(),
                    reason: format!(
                        "Function has high cyclomatic complexity ({}). Consider breaking it down into smaller functions.",
                        func.cyclomatic_complexity
                    ),
                    severity: RefactoringSeverity::High,
                });
            } else if func.cyclomatic_complexity > 10 {
                suggestions.push(RefactoringSuggestion::ExtractMethod {
                    source: func.name.clone(),
                    reason: format!(
                        "Function has moderate cyclomatic complexity ({}). Consider simplifying.",
                        func.cyclomatic_complexity
                    ),
                    severity: RefactoringSeverity::Medium,
                });
            }

            if func.line_count > 100 {
                suggestions.push(RefactoringSuggestion::ExtractMethod {
                    source: func.name.clone(),
                    reason: format!(
                        "Function is very long ({} lines). Consider breaking it down.",
                        func.line_count
                    ),
                    severity: RefactoringSeverity::High,
                });
            } else if func.line_count > 50 {
                suggestions.push(RefactoringSuggestion::ExtractMethod {
                    source: func.name.clone(),
                    reason: format!(
                        "Function is long ({} lines). Consider breaking it down.",
                        func.line_count
                    ),
                    severity: RefactoringSeverity::Medium,
                });
            }

            if func.parameter_count > 5 {
                suggestions.push(RefactoringSuggestion::IntroduceParameterObject {
                    source: func.name.clone(),
                    reason: format!(
                        "Function has many parameters ({}). Consider grouping related parameters.",
                        func.parameter_count
                    ),
                    severity: RefactoringSeverity::Medium,
                });
            }
        }

        // Check for complex classes
        for class in &self.class_metrics {
            if class.method_count > 20 {
                suggestions.push(RefactoringSuggestion::ExtractClass {
                    source: class.name.clone(),
                    reason: format!(
                        "Class has many methods ({}). Consider splitting responsibilities.",
                        class.method_count
                    ),
                    severity: RefactoringSeverity::High,
                });
            } else if class.method_count > 10 {
                suggestions.push(RefactoringSuggestion::ExtractClass {
                    source: class.name.clone(),
                    reason: format!(
                        "Class has several methods ({}). Consider if all belong together.",
                        class.method_count
                    ),
                    severity: RefactoringSeverity::Medium,
                });
            }

            if class.field_count > 15 {
                suggestions.push(RefactoringSuggestion::ExtractClass {
                    source: class.name.clone(),
                    reason: format!(
                        "Class has many fields ({}). Consider splitting into smaller classes.",
                        class.field_count
                    ),
                    severity: RefactoringSeverity::High,
                });
            }
        }

        suggestions
    }
}

/// Summary of complexity metrics
#[derive(Debug, Clone)]
pub struct ComplexitySummary {
    /// Total number of functions
    pub total_functions: usize,
    /// Total number of classes
    pub total_classes: usize,
    /// Total lines in functions and methods
    pub total_lines: usize,
    /// Maximum function complexity
    pub max_function_complexity: usize,
    /// Number of functions above complexity threshold
    pub complex_functions: usize,
    /// Average method count per class
    pub avg_method_count: f64,
}

/// Refactoring suggestion types
#[derive(Debug, Clone)]
pub enum RefactoringSuggestion {
    /// Extract a method from a complex function
    ExtractMethod {
        /// Source function name
        source: String,
        /// Reason for suggestion
        reason: String,
        /// Severity
        severity: RefactoringSeverity,
    },
    /// Extract a class from a complex class
    ExtractClass {
        /// Source class name
        source: String,
        /// Reason for suggestion
        reason: String,
        /// Severity
        severity: RefactoringSeverity,
    },
    /// Group parameters into an object
    IntroduceParameterObject {
        /// Source function name
        source: String,
        /// Reason for suggestion
        reason: String,
        /// Severity
        severity: RefactoringSeverity,
    },
}

/// Refactoring severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefactoringSeverity {
    /// Low priority
    Low,
    /// Medium priority
    Medium,
    /// High priority
    High,
}
