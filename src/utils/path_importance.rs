use anyhow::Result;
use std::collections::HashMap;
use std::fs;
// Removido import não utilizado: use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::debug;

use crate::core::state::FileReadInfo;

/// Importance score types for ranking paths
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum ImportanceScore {
    /// Critical project files like README, Cargo.toml, etc.
    Critical(f64),
    /// Recently modified Git files
    RecentlyModified(f64),
    /// Frequently accessed files (high read/write count)
    FrequentlyAccessed(f64),
    /// Files matching common important patterns
    KeyPattern(f64),
    /// Regular files
    Regular(f64),
}

impl ImportanceScore {
    /// Get the numerical value of the score
    pub fn value(&self) -> f64 {
        match self {
            ImportanceScore::Critical(v) => v + 1000.0,
            ImportanceScore::RecentlyModified(v) => v + 500.0,
            ImportanceScore::FrequentlyAccessed(v) => v + 250.0,
            ImportanceScore::KeyPattern(v) => v + 100.0,
            ImportanceScore::Regular(v) => *v,
        }
    }
}

/// File patterns that are considered important
const IMPORTANT_PATTERNS: &[&str] = &[
    // Configuration files
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "yarn.lock",
    "Pipfile",
    "Pipfile.lock",
    "requirements.txt",
    "pyproject.toml",
    "setup.py",
    "Gemfile",
    "Gemfile.lock",
    "build.gradle",
    "pom.xml",
    "build.sbt",
    ".gitignore",
    ".editorconfig",
    "tsconfig.json",
    // Documentation
    "README.md",
    "README.txt",
    "CONTRIBUTING.md",
    "CHANGELOG.md",
    "LICENSE",
    // Source code entrypoints
    "main.rs",
    "lib.rs",
    "mod.rs",
    "index.js",
    "app.js",
    "app.py",
    "main.py",
    // Test directories
    "tests/",
    "test/",
    "__tests__/",
    "spec/",
    // Config directories
    ".github/",
    ".circleci/",
    ".vscode/",
];

/// File patterns that should be ignored
const IGNORE_PATTERNS: &[&str] = &[
    // Build artifacts and dependencies
    "target/",
    "node_modules/",
    "dist/",
    "build/",
    "venv/",
    ".env/",
    "__pycache__/",
    "*.pyc",
    "*.pyo",
    "*.pyd",
    "*.so",
    "*.dll",
    "*.dylib",
    // IDE files
    ".idea/",
    ".vscode/",
    ".vs/",
    "*.iml",
    "*.code-workspace",
    // Lock files
    "yarn.lock",
    "package-lock.json",
    "Cargo.lock",
    // Large data files
    "*.min.js",
    "*.min.css",
    "*.map",
    "*.gz",
    "*.zip",
    "*.tar",
    "*.jar",
    "*.war",
    "*.pdf",
    "*.doc",
    "*.docx",
    "*.xls",
    "*.xlsx",
    "*.ppt",
    "*.pptx",
    // Log files
    "*.log",
    "logs/",
    "log/",
    // Temporary files
    "*.tmp",
    "tmp/",
    "temp/",
    // System files
    ".DS_Store",
    "Thumbs.db",
];

/// Path importance analyzer
pub struct PathImportanceAnalyzer {
    /// File importance scores
    importance_scores: HashMap<PathBuf, ImportanceScore>,
    /// Workspace root path
    workspace_root: PathBuf,
    /// Last modified times for Git files
    git_file_timestamps: HashMap<PathBuf, SystemTime>,
    /// Map of file read statistics
    file_read_stats: HashMap<PathBuf, FileReadInfo>,
}

impl PathImportanceAnalyzer {
    /// Create a new path importance analyzer
    pub fn new(workspace_root: &Path, file_read_stats: HashMap<PathBuf, FileReadInfo>) -> Self {
        Self {
            importance_scores: HashMap::new(),
            workspace_root: workspace_root.to_path_buf(),
            git_file_timestamps: HashMap::new(),
            file_read_stats,
        }
    }

    /// Initialize the analyzer with Git history and existing file stats
    pub fn initialize(&mut self) -> Result<()> {
        // Scan the workspace for important files
        self.scan_workspace()?;

        // Try to get Git information if available
        if let Ok(git_files) = self.get_recent_git_files(10) {
            for (path, timestamp) in git_files {
                self.git_file_timestamps.insert(path.clone(), timestamp);
                // Add to importance scores with high priority
                self.importance_scores
                    .insert(path, ImportanceScore::RecentlyModified(0.0));
            }
        }

        // Factor in file access statistics
        for (path, file_info) in &self.file_read_stats {
            if let Some(ImportanceScore::Regular(_score)) = self.importance_scores.get(path) {
                // More reads/writes/edits = higher score
                let access_score =
                    file_info.line_ranges.len() as f64 * 0.5 + file_info.percentage_read() * 0.3;
                self.importance_scores.insert(
                    path.clone(),
                    ImportanceScore::FrequentlyAccessed(access_score),
                );
            }
        }

        debug!(
            "Path importance analyzer initialized with {} files",
            self.importance_scores.len()
        );
        Ok(())
    }

    /// Scan the workspace for files and evaluate their importance
    fn scan_workspace(&mut self) -> Result<()> {
        let workspace_path = &self.workspace_root;

        // Helper function to check if a path should be ignored
        fn should_ignore(path: &Path) -> bool {
            for pattern in IGNORE_PATTERNS {
                if let Some(dir_name) = pattern.strip_suffix('/') {
                    // Check directory pattern
                    if path.to_string_lossy().contains(&format!("/{}/", dir_name)) {
                        return true;
                    }
                } else if let Some(file_name) = path.file_name() {
                    // Check file pattern with glob-like matching
                    let pattern = Path::new(pattern);
                    if let Some(pattern_file_name) = pattern.file_name() {
                        if pattern_file_name.to_string_lossy().starts_with('*') {
                            // Simple extension matching
                            let ext = &pattern_file_name.to_string_lossy()[1..];
                            if file_name.to_string_lossy().ends_with(ext) {
                                return true;
                            }
                        } else if file_name == pattern_file_name {
                            return true;
                        }
                    }
                }
            }
            false
        }

        // Helper function to check if a path is important
        fn is_important(path: &Path) -> bool {
            for pattern in IMPORTANT_PATTERNS {
                if let Some(dir_name) = pattern.strip_suffix('/') {
                    // Check directory pattern
                    if path.to_string_lossy().contains(&format!("/{}/", dir_name)) {
                        return true;
                    }
                } else if let Some(file_name) = path.file_name() {
                    // Check exact file name
                    if file_name.to_string_lossy() == *pattern {
                        return true;
                    }
                }
            }
            false
        }

        // Walk the workspace directory recursively
        fn visit_dirs(
            dir: &Path,
            base: &Path,
            scores: &mut HashMap<PathBuf, ImportanceScore>,
        ) -> Result<()> {
            if dir.is_dir() {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();

                    // Skip ignored paths
                    if should_ignore(&path) {
                        continue;
                    }

                    if path.is_dir() {
                        visit_dirs(&path, base, scores)?;
                    } else {
                        // Calculate path's importance
                        let rel_path = path.strip_prefix(base).unwrap_or(&path);

                        if is_important(&path) {
                            scores.insert(path.clone(), ImportanceScore::Critical(0.0));
                        } else {
                            // Basic score based on path depth - shallower paths are more important
                            let depth = rel_path.components().count() as f64;
                            let depth_score = 10.0 / (depth + 1.0);
                            scores.insert(path.clone(), ImportanceScore::Regular(depth_score));
                        }
                    }
                }
            }
            Ok(())
        }

        // Start the directory traversal
        visit_dirs(workspace_path, workspace_path, &mut self.importance_scores)?;

        Ok(())
    }

    /// Get a list of recently modified files from Git history
    fn get_recent_git_files(&self, limit: usize) -> Result<Vec<(PathBuf, SystemTime)>> {
        let mut result = Vec::new();

        // Try to execute git command to get recent files
        let output = std::process::Command::new("git")
            .args(["log", "--name-only", "--pretty=format:%at", "-n", "50"])
            .current_dir(&self.workspace_root)
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);

                // Parse output - each commit has timestamp followed by files
                let mut timestamp = SystemTime::now();
                let mut seen_files = std::collections::HashSet::new();

                for line in output_str.lines() {
                    if line.is_empty() {
                        continue;
                    }

                    // Check if line is a timestamp (all digits)
                    if line.chars().all(|c| c.is_ascii_digit()) {
                        // Parse timestamp as seconds since epoch
                        if let Ok(secs) = line.parse::<u64>() {
                            timestamp =
                                std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
                        }
                    } else {
                        // Line is a file path
                        let file_path = self.workspace_root.join(line);

                        // Only include existing files we haven't seen yet
                        if file_path.exists() && !seen_files.contains(&file_path) {
                            seen_files.insert(file_path.clone());
                            result.push((file_path, timestamp));

                            // Stop once we have enough files
                            if result.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// Get most important paths from the workspace
    pub fn get_important_paths(&self, limit: usize) -> Vec<PathBuf> {
        // Sort paths by importance score
        let mut paths: Vec<_> = self.importance_scores.iter().collect();
        paths.sort_by(|a, b| {
            b.1.value()
                .partial_cmp(&a.1.value())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Return limited number of most important paths
        paths
            .into_iter()
            .take(limit)
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Print important files for debugging
    pub fn debug_print_important_files(&self, limit: usize) {
        let paths = self.get_important_paths(limit);
        debug!("Top {} important files:", paths.len());
        for (i, path) in paths.iter().enumerate() {
            if let Some(score) = self.importance_scores.get(path) {
                debug!("  {}. {} (score: {:?})", i + 1, path.display(), score);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_path_importance_basic() {
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create some test files
        fs::create_dir_all(temp_path.join("src")).unwrap();
        fs::create_dir_all(temp_path.join("tests")).unwrap();
        fs::write(temp_path.join("README.md"), "test").unwrap();
        fs::write(temp_path.join("Cargo.toml"), "test").unwrap();
        fs::write(temp_path.join("src/main.rs"), "test").unwrap();
        fs::write(temp_path.join("src/lib.rs"), "test").unwrap();

        let mut analyzer = PathImportanceAnalyzer::new(temp_path, HashMap::new());
        analyzer.initialize().unwrap();

        let important_paths = analyzer.get_important_paths(10);

        // Check that README.md, Cargo.toml, src/main.rs, and src/lib.rs are all included
        assert!(important_paths.contains(&temp_path.join("README.md")));
        assert!(important_paths.contains(&temp_path.join("Cargo.toml")));
        assert!(important_paths.contains(&temp_path.join("src/main.rs")));
        assert!(important_paths.contains(&temp_path.join("src/lib.rs")));
    }
}
