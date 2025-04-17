use anyhow::{anyhow, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Represents a project file with metadata
#[derive(Debug, Clone)]
pub struct ProjectFile {
    /// Path to the file
    pub path: PathBuf,
    /// File extension (without dot)
    pub extension: String,
    /// Language detected for the file
    pub language: String,
    /// Size in bytes
    pub size: u64,
    /// Detected dependencies from imports or includes
    pub dependencies: Vec<String>,
    /// Whether this is likely a test file
    pub is_test: bool,
}

/// Represents a detected dependency in the project
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Name of the dependency
    pub name: String,
    /// Where it was found
    pub found_in: Vec<PathBuf>,
    /// Whether it's an external dependency (vs internal)
    pub is_external: bool,
}

/// Represents the detected build system
#[derive(Debug, Clone, PartialEq)]
pub enum BuildSystem {
    /// Cargo (Rust)
    Cargo,
    /// npm/yarn (JavaScript/TypeScript)
    Npm,
    /// pip/setuptools/poetry (Python)
    Python,
    /// Maven (Java)
    Maven,
    /// Gradle (Java/Kotlin)
    Gradle,
    /// CMake (C/C++)
    CMake,
    /// Make (generic)
    Make,
    /// Bazel (Google's build system)
    Bazel,
    /// Unknown build system
    Unknown,
}

impl ToString for BuildSystem {
    fn to_string(&self) -> String {
        match self {
            BuildSystem::Cargo => "Cargo (Rust)".to_string(),
            BuildSystem::Npm => "npm/yarn (JavaScript/TypeScript)".to_string(),
            BuildSystem::Python => "Python (pip/setuptools/poetry)".to_string(),
            BuildSystem::Maven => "Maven (Java)".to_string(),
            BuildSystem::Gradle => "Gradle (Java/Kotlin)".to_string(),
            BuildSystem::CMake => "CMake (C/C++)".to_string(),
            BuildSystem::Make => "Make".to_string(),
            BuildSystem::Bazel => "Bazel".to_string(),
            BuildSystem::Unknown => "Unknown".to_string(),
        }
    }
}

/// Represents the analysis of a project
#[derive(Debug, Clone)]
pub struct ProjectAnalysis {
    /// Root directory of the project
    pub root_dir: PathBuf,
    /// Detected files in the project
    pub files: Vec<ProjectFile>,
    /// Detected dependencies
    pub dependencies: Vec<Dependency>,
    /// Detected build system
    pub build_system: BuildSystem,
    /// Detected primary languages
    pub primary_languages: Vec<String>,
    /// Important root-level files (READMEs, config files, etc)
    pub important_files: Vec<PathBuf>,
    /// Common patterns detected in code
    pub common_patterns: HashMap<String, usize>,
    /// Project structure metadata
    pub structure: ProjectStructure,
}

/// Represents the structure of a project
#[derive(Debug, Clone)]
pub struct ProjectStructure {
    /// Source directories
    pub source_dirs: Vec<PathBuf>,
    /// Test directories
    pub test_dirs: Vec<PathBuf>,
    /// Configuration files
    pub config_files: Vec<PathBuf>,
    /// Documentation directories
    pub doc_dirs: Vec<PathBuf>,
    /// Whether the project has a monorepo structure
    pub is_monorepo: bool,
    /// Component directories in a monorepo
    pub components: Vec<PathBuf>,
}

impl ProjectAnalysis {
    /// Create a new project analysis from a root directory
    pub fn new(root_dir: impl AsRef<Path>) -> Self {
        Self {
            root_dir: root_dir.as_ref().to_path_buf(),
            files: Vec::new(),
            dependencies: Vec::new(),
            build_system: BuildSystem::Unknown,
            primary_languages: Vec::new(),
            important_files: Vec::new(),
            common_patterns: HashMap::new(),
            structure: ProjectStructure {
                source_dirs: Vec::new(),
                test_dirs: Vec::new(),
                config_files: Vec::new(),
                doc_dirs: Vec::new(),
                is_monorepo: false,
                components: Vec::new(),
            },
        }
    }

    /// Add a detected file to the analysis
    pub fn add_file(&mut self, file: ProjectFile) {
        self.files.push(file);
    }

    /// Add a dependency to the analysis
    pub fn add_dependency(&mut self, dependency: Dependency) {
        // Check if dependency already exists
        if let Some(existing) = self.dependencies.iter_mut().find(|d| d.name == dependency.name) {
            // Merge found_in paths
            for path in dependency.found_in {
                if !existing.found_in.contains(&path) {
                    existing.found_in.push(path);
                }
            }
        } else {
            self.dependencies.push(dependency);
        }
    }

    /// Set the build system
    pub fn set_build_system(&mut self, build_system: BuildSystem) {
        self.build_system = build_system;
    }

    /// Add a primary language
    pub fn add_primary_language(&mut self, language: &str) {
        if !self.primary_languages.contains(&language.to_string()) {
            self.primary_languages.push(language.to_string());
        }
    }

    /// Add an important file
    pub fn add_important_file(&mut self, path: impl AsRef<Path>) {
        let path_buf = path.as_ref().to_path_buf();
        if !self.important_files.contains(&path_buf) {
            self.important_files.push(path_buf);
        }
    }

    /// Add a common pattern
    pub fn add_common_pattern(&mut self, pattern: &str) {
        *self.common_patterns.entry(pattern.to_string()).or_insert(0) += 1;
    }

    /// Generate a markdown report of the analysis
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!("# Project Analysis: {}\n\n", 
            self.root_dir.file_name().unwrap_or_default().to_string_lossy()));

        // Build system
        md.push_str(&format!("## Build System\n\n{}\n\n", self.build_system.to_string()));

        // Primary languages
        md.push_str("## Primary Languages\n\n");
        for lang in &self.primary_languages {
            md.push_str(&format!("- {}\n", lang));
        }
        md.push_str("\n");

        // Important files
        md.push_str("## Important Files\n\n");
        for file in &self.important_files {
            md.push_str(&format!("- `{}`\n", file.to_string_lossy()));
        }
        md.push_str("\n");

        // Project structure
        md.push_str("## Project Structure\n\n");
        
        if self.structure.is_monorepo {
            md.push_str("This appears to be a monorepo.\n\n");
            
            md.push_str("### Components\n\n");
            for component in &self.structure.components {
                md.push_str(&format!("- `{}`\n", component.to_string_lossy()));
            }
            md.push_str("\n");
        }
        
        md.push_str("### Source Directories\n\n");
        for dir in &self.structure.source_dirs {
            md.push_str(&format!("- `{}`\n", dir.to_string_lossy()));
        }
        md.push_str("\n");
        
        md.push_str("### Test Directories\n\n");
        for dir in &self.structure.test_dirs {
            md.push_str(&format!("- `{}`\n", dir.to_string_lossy()));
        }
        md.push_str("\n");
        
        // Dependencies
        md.push_str("## Dependencies\n\n");
        
        let external_deps: Vec<_> = self.dependencies.iter()
            .filter(|d| d.is_external)
            .collect();
        
        if !external_deps.is_empty() {
            md.push_str("### External Dependencies\n\n");
            for dep in external_deps {
                md.push_str(&format!("- **{}** (found in {} files)\n", 
                    dep.name, dep.found_in.len()));
            }
            md.push_str("\n");
        }
        
        let internal_deps: Vec<_> = self.dependencies.iter()
            .filter(|d| !d.is_external)
            .collect();
        
        if !internal_deps.is_empty() {
            md.push_str("### Internal Module Dependencies\n\n");
            for dep in internal_deps {
                md.push_str(&format!("- **{}** (used in {} files)\n", 
                    dep.name, dep.found_in.len()));
            }
            md.push_str("\n");
        }
        
        // Common patterns
        if !self.common_patterns.is_empty() {
            md.push_str("## Common Patterns\n\n");
            let mut patterns: Vec<_> = self.common_patterns.iter().collect();
            patterns.sort_by(|a, b| b.1.cmp(a.1));
            
            for (pattern, count) in patterns.iter().take(10) {
                md.push_str(&format!("- **{}**: found {} times\n", pattern, count));
            }
            md.push_str("\n");
        }
        
        // Statistics
        md.push_str("## Statistics\n\n");
        
        let total_size: u64 = self.files.iter().map(|f| f.size).sum();
        let file_extensions: HashMap<_, _> = self.files.iter()
            .fold(HashMap::new(), |mut map, file| {
                *map.entry(file.extension.clone()).or_insert(0) += 1;
                map
            });
        
        md.push_str(&format!("- Total files: {}\n", self.files.len()));
        md.push_str(&format!("- Total size: {} KB\n", total_size / 1024));
        
        md.push_str("\n### File Types\n\n");
        let mut extensions: Vec<_> = file_extensions.iter().collect();
        extensions.sort_by(|a, b| b.1.cmp(a.1));
        
        for (ext, count) in extensions {
            if ext.is_empty() {
                md.push_str(&format!("- No extension: {} files\n", count));
            } else {
                md.push_str(&format!("- .{}: {} files\n", ext, count));
            }
        }
        
        md
    }
}

/// Project analyzer that analyzes a project's structure and contents
pub struct ProjectAnalyzer {
    /// Ignored directories
    ignored_dirs: GlobSet,
    /// Ignored files
    ignored_files: GlobSet,
}

impl Default for ProjectAnalyzer {
    fn default() -> Self {
        let mut builder = GlobSetBuilder::new();
        
        // Common directories to ignore
        for pattern in &[
            "**/node_modules/**",
            "**/target/**",
            "**/.git/**",
            "**/.idea/**",
            "**/.vscode/**",
            "**/build/**",
            "**/dist/**",
            "**/__pycache__/**",
            "**/.DS_Store",
        ] {
            builder.add(Glob::new(pattern).unwrap());
        }
        
        let ignored_dirs = builder.build().unwrap();
        
        let mut builder = GlobSetBuilder::new();
        
        // Common files to ignore
        for pattern in &[
            "**/*.lock",
            "**/*.log",
            "**/*.tmp",
            "**/*.temp",
            "**/*.swp",
        ] {
            builder.add(Glob::new(pattern).unwrap());
        }
        
        let ignored_files = builder.build().unwrap();
        
        Self {
            ignored_dirs,
            ignored_files,
        }
    }
}

impl ProjectAnalyzer {
    /// Create a new project analyzer
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add custom ignore patterns for directories
    pub fn add_ignored_dirs(&mut self, patterns: &[&str]) -> Result<()> {
        let mut builder = GlobSetBuilder::new();
        
        // We can't access existing patterns, so we recreate the default ones
        for pattern in &[
            "**/node_modules/**",
            "**/target/**",
            "**/.git/**",
            "**/.idea/**",
            "**/.vscode/**",
            "**/build/**",
            "**/dist/**",
            "**/__pycache__/**",
            "**/.DS_Store",
        ] {
            builder.add(Glob::new(pattern)?);
        }
        
        // Add new patterns
        for pattern in patterns {
            builder.add(Glob::new(pattern)?);
        }
        
        self.ignored_dirs = builder.build()?;
        Ok(())
    }
    
    /// Add custom ignore patterns for files
    pub fn add_ignored_files(&mut self, patterns: &[&str]) -> Result<()> {
        let mut builder = GlobSetBuilder::new();
        
        // We can't access existing patterns, so we recreate the default ones
        for pattern in &[
            "**/*.lock",
            "**/*.log",
            "**/*.tmp",
            "**/*.temp",
            "**/*.swp",
        ] {
            builder.add(Glob::new(pattern)?);
        }
        
        // Add new patterns
        for pattern in patterns {
            builder.add(Glob::new(pattern)?);
        }
        
        self.ignored_files = builder.build()?;
        Ok(())
    }
    
    /// Check if a path should be ignored
    fn should_ignore(&self, path: &Path) -> bool {
        self.ignored_dirs.is_match(path) || self.ignored_files.is_match(path)
    }
    
    /// Detect the file language from extension
    fn detect_language(&self, path: &Path) -> String {
        let extension = path.extension()
            .map(|ext| ext.to_string_lossy().to_lowercase())
            .unwrap_or_default()
            .to_string();
        
        match extension.as_str() {
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
            "graphql" | "gql" => "GraphQL".to_string(),
            "sql" => "SQL".to_string(),
            _ => format!("Unknown ({})", extension),
        }
    }
    
    /// Check if a file is likely a test file
    fn is_test_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        let file_name = path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
            .to_string();
        
        path_str.contains("/test/") || 
        path_str.contains("/tests/") || 
        path_str.contains("/spec/") || 
        file_name.starts_with("test_") || 
        file_name.ends_with("_test.") || 
        file_name.ends_with(".test.") || 
        file_name.ends_with("_spec.") || 
        file_name.ends_with(".spec.")
    }
    
    /// Extract dependencies from a file (simple version)
    fn extract_dependencies(&self, path: &Path, content: &str) -> Vec<String> {
        let mut dependencies = Vec::new();
        let file_name = path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
            .to_string();
        
        // Very basic detection - would need to be expanded for real usage
        if let Some(ext) = path.extension() {
            match ext.to_string_lossy().as_ref() {
                "rs" => {
                    // Rust use/mod statements
                    for line in content.lines() {
                        let line = line.trim();
                        if line.starts_with("use ") && !line.starts_with("use {") {
                            let mut parts = line.split("::");
                            if let Some(first) = parts.next() {
                                let dep = first.trim_start_matches("use ").trim();
                                if !dep.is_empty() && dep != "crate" && dep != "self" && dep != "super" {
                                    dependencies.push(dep.to_string());
                                }
                            }
                        } else if line.starts_with("extern crate ") {
                            let dep = line.trim_start_matches("extern crate ").trim_end_matches(';').trim();
                            if !dep.is_empty() {
                                dependencies.push(dep.to_string());
                            }
                        }
                    }
                },
                "js" | "jsx" | "ts" | "tsx" => {
                    // JS/TS import statements
                    for line in content.lines() {
                        let line = line.trim();
                        if line.starts_with("import ") && line.contains(" from ") {
                            let parts: Vec<&str> = line.split(" from ").collect();
                            if parts.len() >= 2 {
                                let source = parts[1].trim().trim_matches(|c| c == '\'' || c == '"' || c == ';');
                                if !source.is_empty() && !source.starts_with(".") {
                                    dependencies.push(source.to_string());
                                }
                            }
                        } else if line.starts_with("require(") {
                            // Simple require pattern
                            if let Some(start) = line.find("require(") {
                                if let Some(end) = line[start..].find(")") {
                                    let require = &line[start + 8..start + end].trim_matches(|c| c == '\'' || c == '"');
                                    if !require.is_empty() && !require.starts_with(".") {
                                        dependencies.push(require.to_string());
                                    }
                                }
                            }
                        }
                    }
                },
                "py" => {
                    // Python imports
                    for line in content.lines() {
                        let line = line.trim();
                        if line.starts_with("import ") {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 2 {
                                let package = parts[1].split(".").next().unwrap_or("").trim_end_matches(",");
                                if !package.is_empty() {
                                    dependencies.push(package.to_string());
                                }
                            }
                        } else if line.starts_with("from ") && line.contains(" import ") {
                            let parts: Vec<&str> = line.split(" import ").collect();
                            if parts.len() >= 2 {
                                let package = parts[0].trim_start_matches("from ").split(".").next().unwrap_or("");
                                if !package.is_empty() {
                                    dependencies.push(package.to_string());
                                }
                            }
                        }
                    }
                },
                // Add other languages as needed
                _ => {},
            }
        }
        
        dependencies
    }
    
    /// Detect build system from root directory
    fn detect_build_system(&self, root_dir: &Path) -> BuildSystem {
        let has_file = |file: &str| -> bool {
            root_dir.join(file).exists()
        };
        
        if has_file("Cargo.toml") {
            BuildSystem::Cargo
        } else if has_file("package.json") {
            BuildSystem::Npm
        } else if has_file("setup.py") || has_file("pyproject.toml") || has_file("requirements.txt") {
            BuildSystem::Python
        } else if has_file("pom.xml") {
            BuildSystem::Maven
        } else if has_file("build.gradle") || has_file("build.gradle.kts") {
            BuildSystem::Gradle
        } else if has_file("CMakeLists.txt") {
            BuildSystem::CMake
        } else if has_file("Makefile") || has_file("makefile") {
            BuildSystem::Make
        } else if has_file("WORKSPACE") || has_file("BUILD") || has_file("BUILD.bazel") {
            BuildSystem::Bazel
        } else {
            BuildSystem::Unknown
        }
    }
    
    /// Detect if a project is a monorepo
    fn detect_monorepo(&self, root_dir: &Path, build_system: &BuildSystem) -> (bool, Vec<PathBuf>) {
        let mut components = Vec::new();
        
        match build_system {
            BuildSystem::Cargo => {
                // Check for workspace members in Cargo.toml
                if let Ok(content) = fs::read_to_string(root_dir.join("Cargo.toml")) {
                    if content.contains("[workspace]") && content.contains("members = [") {
                        return (true, components); // Basic detection for now
                    }
                }
            },
            BuildSystem::Npm => {
                // Check for workspaces in package.json
                if let Ok(content) = fs::read_to_string(root_dir.join("package.json")) {
                    if content.contains("\"workspaces\"") {
                        return (true, components); // Basic detection for now
                    }
                }
                
                // Check for common monorepo directories
                let packages_dir = root_dir.join("packages");
                if packages_dir.exists() && packages_dir.is_dir() {
                    if let Ok(entries) = fs::read_dir(packages_dir) {
                        for entry in entries.filter_map(Result::ok) {
                            let path = entry.path();
                            if path.is_dir() && path.join("package.json").exists() {
                                components.push(path);
                            }
                        }
                    }
                    
                    if !components.is_empty() {
                        return (true, components);
                    }
                }
            },
            _ => {},
        }
        
        (false, components)
    }
    
    /// Detect primary languages based on file counts
    fn detect_primary_languages(&self, files: &[ProjectFile]) -> Vec<String> {
        let mut language_counts = HashMap::new();
        
        for file in files {
            *language_counts.entry(file.language.clone()).or_insert(0) += 1;
        }
        
        // Get languages that make up significant portions of the codebase
        let total_files = files.len() as f64;
        let threshold = 0.05; // 5% or more
        
        let mut primary_languages: Vec<(String, usize)> = language_counts.into_iter()
            .filter(|(_, count)| (*count as f64 / total_files) >= threshold)
            .collect();
        
        primary_languages.sort_by(|a, b| b.1.cmp(&a.1));
        
        primary_languages.into_iter()
            .map(|(lang, _)| lang)
            .collect()
    }
    
    /// Identify important directories in the project
    fn identify_project_structure(&self, root_dir: &Path, files: &[ProjectFile]) -> ProjectStructure {
        let mut structure = ProjectStructure {
            source_dirs: Vec::new(),
            test_dirs: Vec::new(),
            config_files: Vec::new(),
            doc_dirs: Vec::new(),
            is_monorepo: false,
            components: Vec::new(),
        };
        
        // Map of directory paths to file counts
        let mut dir_counts: HashMap<PathBuf, usize> = HashMap::new();
        
        // Identify common directories
        for file in files {
            if let Some(parent) = file.path.parent() {
                let parent_path = parent.to_path_buf();
                *dir_counts.entry(parent_path.clone()).or_insert(0) += 1;
                
                // Check for test directories
                if self.is_test_file(&file.path) {
                    let mut current = parent_path;
                    loop {
                        let is_test_dir = current.file_name()
                            .map(|name| {
                                let name_str = name.to_string_lossy().to_lowercase();
                                name_str == "test" || name_str == "tests" || name_str == "spec" || name_str == "specs"
                            })
                            .unwrap_or(false);
                        
                        if is_test_dir && !structure.test_dirs.contains(&current) {
                            structure.test_dirs.push(current.clone());
                            break;
                        }
                        
                        if let Some(parent) = current.parent() {
                            if parent == root_dir || !parent.starts_with(root_dir) {
                                break;
                            }
                            current = parent.to_path_buf();
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        
        // Sort directories by file count
        let mut dirs: Vec<(PathBuf, usize)> = dir_counts.into_iter().collect();
        dirs.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Get common source directories
        let source_dirs = ["src", "source", "lib", "app", "main"];
        
        for (dir, _) in dirs.iter().take(10) {
            let is_src_dir = dir.file_name()
                .map(|name| {
                    let name_str = name.to_string_lossy().to_lowercase();
                    source_dirs.contains(&name_str.as_str())
                })
                .unwrap_or(false);
            
            if is_src_dir && !structure.source_dirs.contains(dir) {
                structure.source_dirs.push(dir.clone());
            }
        }
        
        // If no source dirs found, use top directories
        if structure.source_dirs.is_empty() && !dirs.is_empty() {
            structure.source_dirs.push(dirs[0].0.clone());
        }
        
        // Identify documentation directories
        let doc_dirs = root_dir.join("docs");
        if doc_dirs.exists() && doc_dirs.is_dir() {
            structure.doc_dirs.push(doc_dirs);
        }
        
        let doc_dirs = root_dir.join("documentation");
        if doc_dirs.exists() && doc_dirs.is_dir() {
            structure.doc_dirs.push(doc_dirs);
        }
        
        // Identify config files
        let config_patterns = [
            "config.json", "config.yaml", "config.yml", "config.toml",
            ".gitignore", ".editorconfig", "tsconfig.json", "Dockerfile",
            ".eslintrc", ".prettierrc", ".env", "docker-compose.yml"
        ];
        
        for pattern in config_patterns {
            let config_file = root_dir.join(pattern);
            if config_file.exists() && config_file.is_file() {
                structure.config_files.push(config_file);
            }
        }
        
        structure
    }
    
    /// Analyze a project directory
    pub fn analyze(&self, root_dir: impl AsRef<Path>) -> Result<ProjectAnalysis> {
        let root_dir = root_dir.as_ref();
        
        if !root_dir.exists() || !root_dir.is_dir() {
            return Err(anyhow!("Root directory does not exist or is not a directory"));
        }
        
        info!("Analyzing project at {}", root_dir.display());
        
        // Create analysis result
        let mut analysis = ProjectAnalysis::new(root_dir);
        
        // Detect build system
        let build_system = self.detect_build_system(root_dir);
        analysis.set_build_system(build_system.clone());
        
        // Detect if monorepo
        let (is_monorepo, components) = self.detect_monorepo(root_dir, &build_system);
        analysis.structure.is_monorepo = is_monorepo;
        analysis.structure.components = components;
        
        // Find important files at root level
        let important_patterns = [
            "README.md", "README", "LICENSE", "CONTRIBUTING.md", 
            "CHANGELOG.md", "SECURITY.md", "CODE_OF_CONDUCT.md"
        ];
        
        for pattern in important_patterns {
            let file_path = root_dir.join(pattern);
            if file_path.exists() && file_path.is_file() {
                analysis.add_important_file(&file_path);
            }
        }
        
        // Recursively scan files
        let mut files = Vec::new();
        self.scan_directory(root_dir, &mut files)?;
        
        // Process each file
        for file_path in files {
            let extension = file_path.extension()
                .map(|ext| ext.to_string_lossy().to_lowercase())
                .unwrap_or_default()
                .to_string();
            
            let language = self.detect_language(&file_path);
            let is_test = self.is_test_file(&file_path);
            
            let file_metadata = match fs::metadata(&file_path) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            
            let size = file_metadata.len();
            
            // Read file content for dependency extraction (for smaller files)
            let mut dependencies = Vec::new();
            if size < 1_000_000 {  // Skip files larger than 1MB
                if let Ok(content) = fs::read_to_string(&file_path) {
                    dependencies = self.extract_dependencies(&file_path, &content);
                }
            }
            
            // Add file to analysis
            let project_file = ProjectFile {
                path: file_path.clone(),
                extension,
                language,
                size,
                dependencies: dependencies.clone(),
                is_test,
            };
            
            analysis.add_file(project_file);
            
            // Process dependencies
            for dep in dependencies {
                // Determine if external or internal
                let is_external = !dep.starts_with("crate::") && 
                                  !dep.starts_with("super::") && 
                                  !dep.starts_with("self::") &&
                                  !dep.starts_with("./") && 
                                  !dep.starts_with("../");
                
                let dependency = Dependency {
                    name: dep,
                    found_in: vec![file_path.clone()],
                    is_external,
                };
                
                analysis.add_dependency(dependency);
            }
        }
        
        // Detect primary languages
        let primary_languages = self.detect_primary_languages(&analysis.files);
        for lang in primary_languages {
            analysis.add_primary_language(&lang);
        }
        
        // Identify project structure
        analysis.structure = self.identify_project_structure(root_dir, &analysis.files);
        
        Ok(analysis)
    }
    
    /// Recursively scan directory for files
    fn scan_directory(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if self.should_ignore(dir) {
            return Ok(());
        }
        
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                debug!("Error reading directory {}: {}", dir.display(), e);
                return Ok(());
            }
        };
        
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            
            let path = entry.path();
            
            if self.should_ignore(&path) {
                continue;
            }
            
            if path.is_dir() {
                self.scan_directory(&path, files)?;
            } else if path.is_file() {
                files.push(path);
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;
    
    #[test]
    fn test_detect_build_system() {
        let analyzer = ProjectAnalyzer::new();
        
        let dir = tempdir().unwrap();
        let path = dir.path();
        
        // Create a Cargo.toml file
        let cargo_file = path.join("Cargo.toml");
        let mut file = File::create(cargo_file).unwrap();
        writeln!(file, "[package]").unwrap();
        writeln!(file, "name = \"test\"").unwrap();
        writeln!(file, "version = \"0.1.0\"").unwrap();
        
        let build_system = analyzer.detect_build_system(path);
        assert_eq!(build_system, BuildSystem::Cargo);
    }
    
    #[test]
    fn test_is_test_file() {
        let analyzer = ProjectAnalyzer::new();
        
        assert!(analyzer.is_test_file(Path::new("/path/to/test/file.rs")));
        assert!(analyzer.is_test_file(Path::new("/path/to/tests/file.rs")));
        assert!(analyzer.is_test_file(Path::new("/path/to/test_file.rs")));
        assert!(analyzer.is_test_file(Path::new("/path/to/file_test.rs")));
        assert!(analyzer.is_test_file(Path::new("/path/to/file.test.js")));
        assert!(!analyzer.is_test_file(Path::new("/path/to/file.rs")));
    }
    
    #[test]
    fn test_extract_dependencies() {
        let analyzer = ProjectAnalyzer::new();
        
        let rust_content = "
use std::fs;
use anyhow::Result;
use crate::utils;
extern crate serde;
";
        
        let deps = analyzer.extract_dependencies(Path::new("test.rs"), rust_content);
        assert!(deps.contains(&"std".to_string()));
        assert!(deps.contains(&"anyhow".to_string()));
        assert!(deps.contains(&"serde".to_string()));
        assert!(!deps.contains(&"crate".to_string()));
        
        let js_content = "
import React from 'react';
import { useState } from 'react';
import axios from 'axios';
import './styles.css';
const fs = require('fs');
";
        
        let deps = analyzer.extract_dependencies(Path::new("test.js"), js_content);
        assert!(deps.contains(&"react".to_string()));
        assert!(deps.contains(&"axios".to_string()));
        assert!(deps.contains(&"fs".to_string()));
        assert!(!deps.contains(&"./styles.css".to_string()));
    }
}
