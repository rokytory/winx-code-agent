use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info};

use crate::code::project_analyzer::{ProjectAnalysis, ProjectAnalyzer};
use crate::code::refactoring_engine::RefactoringEngine;
use crate::code::semantic_analyzer::SemanticAnalyzer;
use crate::core::state::SharedState;
use crate::diff::search_replace_enhanced::{EnhancedEditResult, EnhancedSearchReplace};
use crate::utils::fs;

/// Defines the agent's knowledge of a file
#[derive(Debug, Clone)]
pub struct FileKnowledge {
    /// Path to the file
    pub path: PathBuf,
    /// Last known modification time
    pub last_modified: std::time::SystemTime,
    /// Whether the file has been fully read
    pub fully_read: bool,
    /// Lines that have been read (1-indexed, inclusive ranges)
    pub read_ranges: Vec<(usize, usize)>,
    /// Total number of lines
    pub total_lines: usize,
    /// File hash for change detection
    pub hash: String,
    /// Language detected for the file
    pub language: String,
    /// Semantic structure of the file (if analyzed)
    pub semantic_structure: Option<SemanticStructure>,
    /// Number of times modified by the agent
    pub modifications: usize,
}

impl FileKnowledge {
    /// Create new file knowledge from a path
    pub fn new(path: impl AsRef<Path>, language: String) -> Result<Self> {
        let path = path.as_ref();

        let metadata = std::fs::metadata(path)
            .with_context(|| format!("Failed to read metadata for {}", path.display()))?;

        let last_modified = metadata
            .modified()
            .unwrap_or_else(|_| std::time::SystemTime::now());

        let content = fs::read_file_to_string(path)
            .with_context(|| format!("Failed to read content of {}", path.display()))?;

        // Calculate hash
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        // Count lines
        let total_lines = content.lines().count();

        Ok(Self {
            path: path.to_path_buf(),
            last_modified,
            fully_read: false,
            read_ranges: Vec::new(),
            total_lines,
            hash,
            language,
            semantic_structure: None,
            modifications: 0,
        })
    }

    /// Update file knowledge after reading
    pub fn mark_read_range(&mut self, start: usize, end: usize) -> Result<()> {
        // Normalize range (1-indexed, inclusive)
        let start = start.max(1);
        let end = end.min(self.total_lines);

        if start > end {
            return Ok(());
        }

        // Add the range, potentially merging with existing ranges
        let mut merged = false;
        for range in &mut self.read_ranges {
            // Check if ranges overlap or are adjacent
            if (start <= range.1 + 1) && (end + 1 >= range.0) {
                // Merge ranges
                range.0 = range.0.min(start);
                range.1 = range.1.max(end);
                merged = true;
                break;
            }
        }

        if !merged {
            self.read_ranges.push((start, end));
        }

        // Sort ranges by start line
        self.read_ranges.sort_by_key(|r| r.0);

        // Merge overlapping ranges after sorting
        let mut i = 0;
        while i < self.read_ranges.len() - 1 {
            if self.read_ranges[i].1 + 1 >= self.read_ranges[i + 1].0 {
                // Merge these ranges
                let merged_end = self.read_ranges[i + 1].1.max(self.read_ranges[i].1);
                self.read_ranges[i].1 = merged_end;
                self.read_ranges.remove(i + 1);
            } else {
                i += 1;
            }
        }

        // Check if the entire file has been read
        if self.read_ranges.len() == 1
            && self.read_ranges[0].0 == 1
            && self.read_ranges[0].1 == self.total_lines
        {
            self.fully_read = true;
        }

        Ok(())
    }

    /// Check if a file has changed since it was last seen
    pub fn has_changed(&self) -> Result<bool> {
        let path = &self.path;

        // Check if file still exists
        if !path.exists() {
            return Ok(true);
        }

        // Check modification time
        let metadata = std::fs::metadata(path)?;
        let current_modified = metadata
            .modified()
            .unwrap_or_else(|_| std::time::SystemTime::now());

        if current_modified > self.last_modified {
            // File was modified, check hash to be sure
            let content = fs::read_file_to_string(path)?;

            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let current_hash = format!("{:x}", hasher.finalize());

            return Ok(current_hash != self.hash);
        }

        Ok(false)
    }

    /// Calculate the percentage of the file that has been read
    pub fn percentage_read(&self) -> f64 {
        if self.total_lines == 0 {
            return 100.0;
        }

        let mut read_lines = 0;
        for &(start, end) in &self.read_ranges {
            read_lines += end - start + 1;
        }

        (read_lines as f64 / self.total_lines as f64) * 100.0
    }

    /// Get ranges of the file that haven't been read yet
    pub fn get_unread_ranges(&self) -> Vec<(usize, usize)> {
        if self.total_lines == 0 || self.read_ranges.is_empty() {
            return if self.total_lines > 0 {
                vec![(1, self.total_lines)]
            } else {
                Vec::new()
            };
        }

        let mut unread = Vec::new();
        let mut current_line = 1;

        for &(start, end) in &self.read_ranges {
            if current_line < start {
                unread.push((current_line, start - 1));
            }
            current_line = end + 1;
        }

        if current_line <= self.total_lines {
            unread.push((current_line, self.total_lines));
        }

        unread
    }

    /// Check if this file can be safely edited
    pub fn can_edit(&self) -> bool {
        self.fully_read || self.percentage_read() >= 95.0
    }

    /// Update file knowledge after modification
    pub fn mark_modified(&mut self) -> Result<()> {
        self.modifications += 1;

        // Re-read file to update hash and metadata
        if self.path.exists() {
            let content = fs::read_file_to_string(&self.path)?;

            // Update hash
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            self.hash = format!("{:x}", hasher.finalize());

            // Update modification time
            let metadata = std::fs::metadata(&self.path)?;
            self.last_modified = metadata
                .modified()
                .unwrap_or_else(|_| std::time::SystemTime::now());

            // Update total lines
            self.total_lines = content.lines().count();

            // Mark as fully read after modification
            self.fully_read = true;
            self.read_ranges = vec![(1, self.total_lines)];
        }

        Ok(())
    }
}

/// Represents semantic information about a code structure
#[derive(Debug, Clone)]
pub struct SemanticStructure {
    /// Symbols defined in the file (functions, classes, etc.)
    pub symbols: Vec<Symbol>,
    /// Imports or references to other files
    pub imports: Vec<String>,
    /// Dependencies on external libraries
    pub dependencies: Vec<String>,
}

/// Represents a code symbol (function, class, variable, etc.)
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, variable, etc.)
    pub kind: SymbolKind,
    /// Line range (1-indexed, inclusive)
    pub line_range: (usize, usize),
    /// Symbol signature (for functions, methods)
    pub signature: Option<String>,
    /// Child symbols
    pub children: Vec<Symbol>,
}

/// Kinds of code symbols
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
    Unknown,
}

/// Defines the types of code patterns
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodePattern {
    /// Common design pattern
    DesignPattern(String),
    /// Architectural pattern
    ArchitecturalPattern(String),
    /// Recurring code style
    StylePattern(String),
    /// Common idiom
    Idiom(String),
    /// Anti-pattern
    AntiPattern(String),
}

/// The VibeCode agent that provides intelligent code understanding and generation
#[derive(Clone)]
pub struct VibeAgent {
    /// Project analysis results
    project_analysis: Option<ProjectAnalysis>,
    /// File knowledge database
    file_knowledge: HashMap<PathBuf, FileKnowledge>,
    /// Project root directory
    project_root: Option<PathBuf>,
    /// Detected code patterns
    detected_patterns: Vec<(CodePattern, PathBuf)>,
    /// Enhanced search/replace engine
    search_replace: EnhancedSearchReplace,
    /// Semantic analyzer
    semantic_analyzer: SemanticAnalyzer,
    /// Refactoring engine
    refactoring_engine: RefactoringEngine,
    /// Shared state from the core system
    state: SharedState,
}

impl VibeAgent {
    /// Create a new VibeCode agent
    pub fn new(state: SharedState) -> Self {
        Self {
            project_analysis: None,
            file_knowledge: HashMap::new(),
            project_root: None,
            detected_patterns: Vec::new(),
            search_replace: EnhancedSearchReplace::new(),
            semantic_analyzer: SemanticAnalyzer::new(),
            refactoring_engine: RefactoringEngine::new(),
            state,
        }
    }

    /// Initialize the agent with a project directory
    pub async fn initialize(&mut self, project_dir: impl AsRef<Path>) -> Result<()> {
        let project_dir = project_dir.as_ref();

        if !project_dir.exists() || !project_dir.is_dir() {
            return Err(crate::utils::localized_error(
                "Project directory does not exist or is not a directory",
                "O diretório do projeto não existe ou não é um diretório",
                "El directorio del proyecto no existe o no es un directorio"
            ));
        }

        info!(
            "Initializing VibeAgent with project: {}",
            project_dir.display()
        );

        // Set project root
        self.project_root = Some(project_dir.to_path_buf());

        // Analyze project structure
        let analyzer = ProjectAnalyzer::new();
        let analysis = analyzer.analyze(project_dir)?;

        // Store analysis results
        self.project_analysis = Some(analysis.clone());

        // Initialize file knowledge for important files
        for file in &analysis.important_files {
            if file.exists() && file.is_file() {
                let language = self.detect_language(file);
                if let Ok(knowledge) = FileKnowledge::new(file, language) {
                    self.file_knowledge.insert(file.clone(), knowledge);
                }
            }
        }

        // Initialize semantic analyzer with project info
        self.semantic_analyzer.initialize(&analysis).await?;

        // Initialize refactoring engine with project info
        self.refactoring_engine.initialize(&analysis)?;

        info!("VibeAgent initialized successfully");
        Ok(())
    }

    /// Get file knowledge for a path, creating it if it doesn't exist
    pub fn get_file_knowledge(&mut self, path: impl AsRef<Path>) -> Result<&FileKnowledge> {
        let path = path.as_ref();

        if !self.file_knowledge.contains_key(path) {
            let language = self.detect_language(path);
            let knowledge = FileKnowledge::new(path, language)?;
            self.file_knowledge.insert(path.to_path_buf(), knowledge);
        }

        Ok(self.file_knowledge.get(path).unwrap())
    }

    /// Get mutable file knowledge for a path
    pub fn get_file_knowledge_mut(&mut self, path: impl AsRef<Path>) -> Result<&mut FileKnowledge> {
        let path = path.as_ref();

        if !self.file_knowledge.contains_key(path) {
            let language = self.detect_language(path);
            let knowledge = FileKnowledge::new(path, language)?;
            self.file_knowledge.insert(path.to_path_buf(), knowledge);
        }

        Ok(self.file_knowledge.get_mut(path).unwrap())
    }

    /// Detect language for a file based on extension and content
    fn detect_language(&self, path: impl AsRef<Path>) -> String {
        let path = path.as_ref();

        if let Some(ext) = path.extension() {
            match ext.to_string_lossy().to_lowercase().as_str() {
                "rs" => "Rust".to_string(),
                "js" => "JavaScript".to_string(),
                "jsx" => "JavaScript (React)".to_string(),
                "ts" => "TypeScript".to_string(),
                "tsx" => "TypeScript (React)".to_string(),
                "py" => "Python".to_string(),
                "java" => "Java".to_string(),
                "c" | "h" => "C".to_string(),
                "cpp" | "cc" | "cxx" | "hpp" => "C++".to_string(),
                "go" => "Go".to_string(),
                "rb" => "Ruby".to_string(),
                "php" => "PHP".to_string(),
                "swift" => "Swift".to_string(),
                "kt" | "kts" => "Kotlin".to_string(),
                "cs" => "C#".to_string(),
                "fs" => "F#".to_string(),
                "scala" => "Scala".to_string(),
                "sh" | "bash" => "Shell".to_string(),
                "html" | "htm" => "HTML".to_string(),
                "css" => "CSS".to_string(),
                "scss" | "sass" => "SCSS/Sass".to_string(),
                "json" => "JSON".to_string(),
                "yml" | "yaml" => "YAML".to_string(),
                "xml" => "XML".to_string(),
                "md" | "markdown" => "Markdown".to_string(),
                "toml" => "TOML".to_string(),
                _ => "Unknown".to_string(),
            }
        } else {
            // Try to detect language from filename
            match path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .as_deref()
            {
                Some("makefile") | Some("makefile.in") => "Makefile".to_string(),
                Some("dockerfile") => "Dockerfile".to_string(),
                Some(".gitignore") => "GitIgnore".to_string(),
                _ => "Unknown".to_string(),
            }
        }
    }

    /// Mark a file as read with the given range
    pub fn mark_file_read(
        &mut self,
        path: impl AsRef<Path>,
        start: usize,
        end: usize,
    ) -> Result<()> {
        let file_path = path.as_ref();

        let knowledge = self.get_file_knowledge_mut(file_path)?;
        knowledge.mark_read_range(start, end)?;

        Ok(())
    }

    /// Check if a file can be safely edited
    pub fn can_edit_file(&mut self, path: impl AsRef<Path>) -> Result<bool> {
        let path = path.as_ref();

        if !path.exists() {
            // New file, can be created
            return Ok(true);
        }

        let knowledge = self.get_file_knowledge(path)?;

        if knowledge.has_changed()? {
            // File has changed since we last saw it
            debug!("File has changed since last seen: {}", path.display());
            return Ok(false);
        }

        Ok(knowledge.can_edit())
    }

    /// Get unread ranges for a file
    pub fn get_unread_ranges(&mut self, path: impl AsRef<Path>) -> Result<Vec<(usize, usize)>> {
        let knowledge = self.get_file_knowledge(path.as_ref())?;
        Ok(knowledge.get_unread_ranges())
    }

    /// Auto-read a file's content and update file knowledge with concurrency control
    pub async fn auto_read_file(&mut self, file_path: impl AsRef<Path>) -> Result<()> {
        let file_path = file_path.as_ref();
        debug!("Auto-reading file with concurrency control: {}", file_path.display());
        
        // Acquire a read lock for this operation
        let _guard = match crate::utils::concurrency::FileOperationGuard::for_reading(file_path).await {
            Ok(guard) => guard,
            Err(e) => {
                return Err(crate::utils::localized_error(
                    format!("Failed to acquire read lock on file {}: {}", file_path.display(), e),
                    format!("Falha ao adquirir bloqueio de leitura no arquivo {}: {}", file_path.display(), e),
                    format!("Error al adquirir bloqueo de lectura en el archivo {}: {}", file_path.display(), e)
                ));
            }
        };
        
        // Read the file content using fs_utils with concurrency protection
        let content = match crate::utils::fs::read_file(file_path).await {
            Ok(content) => content,
            Err(e) => {
                return Err(crate::utils::localized_error(
                    format!("Failed to read file {} during auto-read: {}", file_path.display(), e),
                    format!("Falha ao ler arquivo {} durante leitura automática: {}", file_path.display(), e),
                    format!("Error al leer archivo {} durante lectura automática: {}", file_path.display(), e)
                ));
            }
        };
        
        // Calculate hash for tracking
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        
        // Calculate total lines
        let total_lines = content.lines().count();
        
        // Update file knowledge
        if let Some(knowledge) = self.file_knowledge.get_mut(file_path) {
            // Update existing knowledge
            knowledge.hash = hash;
            knowledge.total_lines = total_lines;
            knowledge.fully_read = true;
            knowledge.read_ranges = vec![(1, total_lines)];
            
            // Update last modified time
            if let Ok(metadata) = std::fs::metadata(file_path) {
                if let Ok(modified) = metadata.modified() {
                    knowledge.last_modified = modified;
                }
            }
        } else {
            // Create new knowledge
            let mut knowledge = FileKnowledge::new(file_path, self.detect_language(file_path))?;
            knowledge.fully_read = true;
            knowledge.read_ranges = vec![(1, total_lines)];
            self.file_knowledge.insert(file_path.to_path_buf(), knowledge);
        }
        
        debug!("Successfully auto-read file: {}", file_path.display());
        Ok(())
    }

    /// Apply search/replace edit to a file with enhanced error handling
    pub async fn apply_search_replace(
        &mut self,
        file_path: impl AsRef<Path>,
        search_replace_text: &str,
    ) -> Result<EnhancedEditResult> {
        let file_path = file_path.as_ref();

        // Check if we can edit the file
        if !self.can_edit_file(file_path)? {
            // If the file can't be edited, auto-read it first
            debug!("File {} needs to be read before editing, auto-reading...", file_path.display());
            
            if let Err(e) = self.auto_read_file(file_path).await {
                // If auto-read fails, provide detailed error message
                let unread_ranges = self.get_unread_ranges(file_path)?;
                
                if !unread_ranges.is_empty() {
                    let ranges_str = unread_ranges
                        .iter()
                        .map(|(start, end)| format!("{}-{}", start, end))
                        .collect::<Vec<_>>()
                        .join(", ");

                    return Err(crate::utils::localized_error(
                        format!("Auto-read failed for {}. Please read the following line ranges manually: {}. Error: {}", 
                               file_path.display(), ranges_str, e),
                        format!("Leitura automática falhou para {}. Por favor, leia os seguintes intervalos de linha manualmente: {}. Erro: {}",
                               file_path.display(), ranges_str, e),
                        format!("Lectura automática falló para {}. Por favor, lea los siguientes rangos de línea manualmente: {}. Error: {}",
                               file_path.display(), ranges_str, e)
                    ));
                } else {
                    return Err(crate::utils::localized_error(
                        format!("File {} has changed since it was last read and auto-read failed: {}", 
                               file_path.display(), e),
                        format!("O arquivo {} foi alterado desde a última leitura e a leitura automática falhou: {}", 
                               file_path.display(), e),
                        format!("El archivo {} ha cambiado desde la última lectura y la lectura automática falló: {}", 
                               file_path.display(), e)
                    ));
                }
            }
            
            // Re-check if the file can be edited after auto-read
            if !self.can_edit_file(file_path)? {
                return Err(crate::utils::localized_error(
                    format!("File {} still cannot be edited after auto-read. It may have changed during reading.", 
                           file_path.display()),
                    format!("O arquivo {} ainda não pode ser editado após a leitura automática. Ele pode ter sido alterado durante a leitura.", 
                           file_path.display()),
                    format!("El archivo {} todavía no se puede editar después de la lectura automática. Puede haber cambiado durante la lectura.", 
                           file_path.display())
                ));
            }
            
            debug!("Successfully auto-read file, proceeding with edit");
        }

        // Acquire a concurrency lock for this file operation
        debug!("Acquiring lock for search/replace operation on {}", file_path.display());
        let _guard = match crate::utils::concurrency::FileOperationGuard::for_writing(file_path).await {
            Ok(guard) => guard,
            Err(e) => {
                return Err(crate::utils::localized_error(
                    format!("Failed to acquire exclusive lock on file {}: {}", file_path.display(), e),
                    format!("Falha ao adquirir bloqueio exclusivo no arquivo {}: {}", file_path.display(), e),
                    format!("Error al adquirir bloqueo exclusivo en el archivo {}: {}", file_path.display(), e)
                ));
            }
        };
        
        // Read the file content with concurrency protection
        let content = match fs::read_file(file_path).await {
            Ok(content) => content,
            Err(e) => {
                return Err(crate::utils::localized_error(
                    format!("Failed to read file {} for search/replace: {}", file_path.display(), e),
                    format!("Falha ao ler arquivo {} para busca/substituição: {}", file_path.display(), e),
                    format!("Error al leer archivo {} para búsqueda/reemplazo: {}", file_path.display(), e)
                ));
            }
        };

        // Apply the search/replace
        let result = self
            .search_replace
            .apply_from_text(&content, search_replace_text)?;

        // If changes were made, write back to the file
        if result.standard_result.changes_made {
            // Write content back to file with concurrency protection
            match fs::write_file(file_path, &result.standard_result.content).await {
                Ok(_) => {
                    // Update file knowledge
                    if let Ok(knowledge) = self.get_file_knowledge_mut(file_path) {
                        knowledge.mark_modified()?;
                    }
                    info!("Successfully edited file: {}", file_path.display());
                },
                Err(e) => {
                    return Err(crate::utils::localized_error(
                        format!("Failed to write changes to file {}: {}", file_path.display(), e),
                        format!("Falha ao escrever alterações no arquivo {}: {}", file_path.display(), e),
                        format!("Error al escribir cambios en el archivo {}: {}", file_path.display(), e)
                    ));
                }
            }
        } else {
            info!("No changes made to file: {}", file_path.display());
        }

        Ok(result)
    }

    /// Generate code suggestions based on project context
    pub async fn generate_code_suggestions(
        &self,
        file_path: impl AsRef<Path>,
    ) -> Result<Vec<String>> {
        let file_path = file_path.as_ref();

        // Ensure we have project analysis
        if self.project_analysis.is_none() {
            return Err(anyhow!(
                "Project analysis not available. Initialize the agent first."
            ));
        }

        // Get file language
        let language = self.detect_language(file_path);

        // Generate suggestions based on project patterns and language
        let mut suggestions = Vec::new();

        if !self.detected_patterns.is_empty() {
            suggestions.push(format!("Consider following these project patterns:"));

            for (pattern, _) in &self.detected_patterns {
                match pattern {
                    CodePattern::DesignPattern(name) => {
                        suggestions.push(format!("- Use the {} design pattern", name));
                    }
                    CodePattern::ArchitecturalPattern(name) => {
                        suggestions.push(format!("- Follow the {} architectural pattern", name));
                    }
                    CodePattern::StylePattern(name) => {
                        suggestions.push(format!("- Apply the {} code style", name));
                    }
                    CodePattern::Idiom(name) => {
                        suggestions.push(format!("- Use the {} idiom", name));
                    }
                    _ => {}
                }
            }
        }

        // Add language-specific suggestions
        match language.as_str() {
            "Rust" => {
                suggestions.push("Rust best practices:".to_string());
                suggestions.push("- Use Result/Option for error handling".to_string());
                suggestions.push("- Prefer immutable variables with `let`".to_string());
                suggestions.push("- Use strong typing and avoid `unwrap()`".to_string());
            }
            "JavaScript" | "JavaScript (React)" | "TypeScript" | "TypeScript (React)" => {
                suggestions.push("JS/TS best practices:".to_string());
                suggestions.push("- Use const/let instead of var".to_string());
                suggestions.push("- Use async/await for asynchronous code".to_string());
                suggestions.push("- Use destructuring for cleaner code".to_string());
            }
            "Python" => {
                suggestions.push("Python best practices:".to_string());
                suggestions.push("- Follow PEP 8 style guide".to_string());
                suggestions.push("- Use list/dict comprehensions where appropriate".to_string());
                suggestions
                    .push("- Use context managers (with statement) for resources".to_string());
            }
            _ => {}
        }

        Ok(suggestions)
    }

    /// Get a project overview report
    pub fn get_project_overview(&self) -> Result<String> {
        if let Some(analysis) = &self.project_analysis {
            Ok(analysis.to_markdown())
        } else {
            Err(anyhow!(
                "Project analysis not available. Initialize the agent first."
            ))
        }
    }

    /// Get detailed information about a specific file
    pub fn get_file_info(&mut self, path: impl AsRef<Path>) -> Result<String> {
        let path = path.as_ref();
        let knowledge = self.get_file_knowledge(path)?;

        let mut info = String::new();

        info.push_str(&format!("# File: {}\n\n", path.display()));
        info.push_str(&format!("- **Language**: {}\n", knowledge.language));
        info.push_str(&format!("- **Size**: {} lines\n", knowledge.total_lines));
        info.push_str(&format!(
            "- **Read**: {:.1}%\n",
            knowledge.percentage_read()
        ));
        info.push_str(&format!(
            "- **Modifications**: {}\n",
            knowledge.modifications
        ));

        info.push_str("\n## Read Status\n\n");

        if knowledge.fully_read {
            info.push_str("This file has been fully read.\n");
        } else {
            info.push_str("### Read Ranges\n\n");

            for &(start, end) in &knowledge.read_ranges {
                info.push_str(&format!("- Lines {}-{}\n", start, end));
            }

            info.push_str("\n### Unread Ranges\n\n");

            for (start, end) in knowledge.get_unread_ranges() {
                info.push_str(&format!("- Lines {}-{}\n", start, end));
            }
        }

        if let Some(structure) = &knowledge.semantic_structure {
            info.push_str("\n## Semantic Structure\n\n");

            info.push_str("### Symbols\n\n");

            let structure_copy = structure.clone(); // Clone to avoid borrowing issues
            for symbol in &structure_copy.symbols {
                // Clone symbol to avoid borrowing issues
                let symbol_clone = symbol.clone();
                // Store a copy of self.format_symbol directly to avoid borrowing issues
                let formatted_text = self.format_symbol_to_string(&symbol_clone, 0);
                info.push_str(&formatted_text);
            }

            info.push_str("\n### Imports\n\n");

            for import in &structure_copy.imports {
                info.push_str(&format!("- `{}`\n", import));
            }

            info.push_str("\n### Dependencies\n\n");

            for dependency in &structure_copy.dependencies {
                info.push_str(&format!("- `{}`\n", dependency));
            }
        }

        Ok(info)
    }

    /// Get a reference to the search/replace engine
    pub fn get_search_replace(&self) -> &EnhancedSearchReplace {
        &self.search_replace
    }

    /// Format a symbol for display
    fn format_symbol(&self, symbol: &Symbol, indent: usize, output: &mut String) {
        let indent_str = "  ".repeat(indent);

        output.push_str(&format!(
            "{}* **{}**: {} (lines {}-{})\n",
            indent_str,
            symbol.kind.to_string(),
            symbol.name,
            symbol.line_range.0,
            symbol.line_range.1
        ));

        if let Some(signature) = &symbol.signature {
            output.push_str(&format!("{}  - Signature: `{}`\n", indent_str, signature));
        }

        // Format children with increased indentation
        for child in &symbol.children {
            self.format_symbol(child, indent + 1, output);
        }
    }

    /// Format a symbol to a string without modifying an existing String
    fn format_symbol_to_string(&self, symbol: &Symbol, indent: usize) -> String {
        let mut output = String::new();
        let indent_str = "  ".repeat(indent);

        output.push_str(&format!(
            "{}* **{}**: {} (lines {}-{})\n",
            indent_str,
            symbol.kind.to_string(),
            symbol.name,
            symbol.line_range.0,
            symbol.line_range.1
        ));

        if let Some(signature) = &symbol.signature {
            output.push_str(&format!("{}  - Signature: `{}`\n", indent_str, signature));
        }

        // Format children with increased indentation
        for child in &symbol.children {
            let child_str = self.format_symbol_to_string(child, indent + 1);
            output.push_str(&child_str);
        }

        output
    }
}

impl SymbolKind {
    /// Get string representation of symbol kind
    fn to_string(&self) -> String {
        match self {
            SymbolKind::File => "File".to_string(),
            SymbolKind::Module => "Module".to_string(),
            SymbolKind::Namespace => "Namespace".to_string(),
            SymbolKind::Package => "Package".to_string(),
            SymbolKind::Class => "Class".to_string(),
            SymbolKind::Method => "Method".to_string(),
            SymbolKind::Property => "Property".to_string(),
            SymbolKind::Field => "Field".to_string(),
            SymbolKind::Constructor => "Constructor".to_string(),
            SymbolKind::Enum => "Enum".to_string(),
            SymbolKind::Interface => "Interface".to_string(),
            SymbolKind::Function => "Function".to_string(),
            SymbolKind::Variable => "Variable".to_string(),
            SymbolKind::Constant => "Constant".to_string(),
            SymbolKind::String => "String".to_string(),
            SymbolKind::Number => "Number".to_string(),
            SymbolKind::Boolean => "Boolean".to_string(),
            SymbolKind::Array => "Array".to_string(),
            SymbolKind::Object => "Object".to_string(),
            SymbolKind::Key => "Key".to_string(),
            SymbolKind::Null => "Null".to_string(),
            SymbolKind::EnumMember => "EnumMember".to_string(),
            SymbolKind::Struct => "Struct".to_string(),
            SymbolKind::Event => "Event".to_string(),
            SymbolKind::Operator => "Operator".to_string(),
            SymbolKind::TypeParameter => "TypeParameter".to_string(),
            SymbolKind::Unknown => "Unknown".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_file_knowledge() {
        // Create a temp file
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");

        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "fn main() {{").unwrap();
        writeln!(file, "    println!(\"Hello, world!\");").unwrap();
        writeln!(file, "}}").unwrap();

        // Create file knowledge
        let mut knowledge = FileKnowledge::new(&file_path, "Rust".to_string()).unwrap();

        // Test marking ranges as read
        knowledge.mark_read_range(1, 2).unwrap();
        assert_eq!(knowledge.read_ranges, vec![(1, 2)]);
        assert_eq!(knowledge.percentage_read(), 2.0 / 3.0 * 100.0);
        assert!(!knowledge.fully_read);

        // Mark the whole file as read
        knowledge.mark_read_range(1, 3).unwrap();
        assert_eq!(knowledge.read_ranges, vec![(1, 3)]);
        assert_eq!(knowledge.percentage_read(), 100.0);
        assert!(knowledge.fully_read);

        // Test unread ranges
        let unread = knowledge.get_unread_ranges();
        assert_eq!(unread.len(), 0);
    }
}
