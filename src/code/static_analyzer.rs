use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::{debug, info, warn};

/// Severity level for static analysis issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "Error"),
            Severity::Warning => write!(f, "Warning"),
            Severity::Info => write!(f, "Info"),
            Severity::Hint => write!(f, "Hint"),
        }
    }
}

/// Static analysis issue with location and description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticAnalysisIssue {
    /// File path where the issue was found
    pub path: PathBuf,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed, if available)
    pub column: Option<usize>,
    /// Description of the issue
    pub message: String,
    /// Severity level
    pub severity: Severity,
    /// Issue code (if any)
    pub code: Option<String>,
    /// Source of the issue (which tool)
    pub source: String,
}

/// Statistics about static analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisStats {
    /// Number of errors found
    pub errors: usize,
    /// Number of warnings found
    pub warnings: usize,
    /// Number of informational issues found
    pub infos: usize,
    /// Number of hints found
    pub hints: usize,
    /// Number of files analyzed
    pub files_analyzed: usize,
}

impl AnalysisStats {
    /// Create a new empty stats object
    pub fn new() -> Self {
        Self {
            errors: 0,
            warnings: 0,
            infos: 0,
            hints: 0,
            files_analyzed: 0,
        }
    }
    
    /// Add an issue to the stats
    pub fn add_issue(&mut self, severity: &Severity) {
        match severity {
            Severity::Error => self.errors += 1,
            Severity::Warning => self.warnings += 1,
            Severity::Info => self.infos += 1,
            Severity::Hint => self.hints += 1,
        }
    }
}

/// Results of static analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticAnalysisReport {
    /// Issues found during analysis
    pub issues: Vec<StaticAnalysisIssue>,
    /// Statistics about the analysis
    pub stats: AnalysisStats,
    /// When the analysis was performed
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl StaticAnalysisReport {
    /// Create a new empty report
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            stats: AnalysisStats::new(),
            timestamp: chrono::Utc::now(),
        }
    }
    
    /// Add an issue to the report
    pub fn add_issue(&mut self, issue: StaticAnalysisIssue) {
        self.stats.add_issue(&issue.severity);
        self.issues.push(issue);
    }
    
    /// Get issues filtered by severity
    pub fn get_issues_by_severity(&self, severity: Severity) -> Vec<&StaticAnalysisIssue> {
        self.issues.iter()
            .filter(|issue| issue.severity == severity)
            .collect()
    }
    
    /// Get issues for a specific file
    pub fn get_issues_for_file(&self, file_path: &Path) -> Vec<&StaticAnalysisIssue> {
        self.issues.iter()
            .filter(|issue| issue.path == file_path)
            .collect()
    }
    
    /// Format a summary of the analysis
    pub fn format_summary(&self) -> String {
        let mut summary = String::new();
        
        summary.push_str(&format!("Static Analysis Report - {}\n", self.timestamp));
        summary.push_str(&format!("Files analyzed: {}\n", self.stats.files_analyzed));
        summary.push_str(&format!("Issues found: {}\n", self.issues.len()));
        summary.push_str(&format!("  Errors: {}\n", self.stats.errors));
        summary.push_str(&format!("  Warnings: {}\n", self.stats.warnings));
        summary.push_str(&format!("  Info: {}\n", self.stats.infos));
        summary.push_str(&format!("  Hints: {}\n", self.stats.hints));
        
        if !self.issues.is_empty() {
            summary.push_str("\nTop issues:\n");
            
            // First list errors, then warnings
            let errors = self.get_issues_by_severity(Severity::Error);
            let warnings = self.get_issues_by_severity(Severity::Warning);
            
            for issue in errors.iter().take(5) {
                summary.push_str(&format!("- [ERROR] {}:{}: {}\n", 
                    issue.path.display(), 
                    issue.line,
                    issue.message
                ));
            }
            
            for issue in warnings.iter().take(5) {
                summary.push_str(&format!("- [WARNING] {}:{}: {}\n", 
                    issue.path.display(), 
                    issue.line,
                    issue.message
                ));
            }
            
            // If there are more issues than shown
            let shown = errors.len().min(5) + warnings.len().min(5);
            if self.issues.len() > shown {
                summary.push_str(&format!("- ... and {} more issues\n", self.issues.len() - shown));
            }
        }
        
        summary
    }
}

/// Static code analyzer
pub struct StaticAnalyzer {
    /// Workspace root path
    workspace_path: PathBuf,
    /// Available linters by language
    available_linters: HashMap<String, Vec<String>>,
}

impl StaticAnalyzer {
    /// Create a new static analyzer
    pub fn new(workspace_path: impl AsRef<Path>) -> Self {
        let mut available_linters = HashMap::new();
        
        // Register available linters for each language
        available_linters.insert("rust".to_string(), vec!["clippy".to_string()]);
        available_linters.insert("python".to_string(), vec!["pylint".to_string(), "mypy".to_string()]);
        available_linters.insert("javascript".to_string(), vec!["eslint".to_string()]);
        available_linters.insert("typescript".to_string(), vec!["eslint".to_string(), "tsc".to_string()]);
        
        Self {
            workspace_path: workspace_path.as_ref().to_path_buf(),
            available_linters,
        }
    }
    
    /// Get the list of supported languages
    pub fn get_supported_languages(&self) -> Vec<String> {
        self.available_linters.keys().cloned().collect()
    }
    
    /// Check if a language is supported
    pub fn is_language_supported(&self, language: &str) -> bool {
        self.available_linters.contains_key(language)
    }
    
    /// Get available linters for a language
    pub fn get_available_linters(&self, language: &str) -> Vec<String> {
        self.available_linters.get(language)
            .cloned()
            .unwrap_or_default()
    }
    
    /// Analyze Rust code with Clippy
    pub async fn analyze_rust(&self, file_paths: &[impl AsRef<Path>]) -> Result<StaticAnalysisReport> {
        let mut report = StaticAnalysisReport::new();
        report.stats.files_analyzed = file_paths.len();
        
        // Check if clippy is available
        let clippy_check = Command::new("rustup")
            .args(["run", "clippy", "--version"])
            .output();
            
        if clippy_check.is_err() || !clippy_check.unwrap().status.success() {
            warn!("Clippy is not available. Skipping Rust static analysis.");
            return Ok(report);
        }
        
        // Run clippy on the workspace
        let output = Command::new("cargo")
            .args(["clippy", "--message-format=json"])
            .current_dir(&self.workspace_path)
            .output()
            .context("Failed to run Clippy")?;
            
        // Process clippy output
        let stdout = String::from_utf8(output.stdout).context("Invalid UTF-8 in Clippy output")?;
        
        for line in stdout.lines() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(message) = value.get("message") {
                    if let (Some(level), Some(message_text)) = (message.get("level"), message.get("message")) {
                        if let (Some(level_str), Some(message_str)) = (level.as_str(), message_text.as_str()) {
                            // Extract information from spans
                            if let Some(spans) = message.get("spans") {
                                if let Some(spans_arr) = spans.as_array() {
                                    if let Some(primary_span) = spans_arr.iter().find(|span| 
                                        span.get("is_primary").map_or(false, |v| v.as_bool().unwrap_or(false))) 
                                    {
                                        if let (Some(file_name), Some(line_num)) = (
                                            primary_span.get("file_name").and_then(|v| v.as_str()),
                                            primary_span.get("line_start").and_then(|v| v.as_u64()),
                                        ) {
                                            let column = primary_span.get("column_start")
                                                .and_then(|v| v.as_u64())
                                                .map(|c| c as usize);
                                                
                                            let severity = match level_str {
                                                "error" => Severity::Error,
                                                "warning" => Severity::Warning,
                                                "note" => Severity::Info,
                                                "help" => Severity::Hint,
                                                _ => Severity::Info,
                                            };
                                            
                                            let code = message.get("code")
                                                .and_then(|c| c.get("code"))
                                                .and_then(|c| c.as_str())
                                                .map(String::from);
                                                
                                            let issue = StaticAnalysisIssue {
                                                path: PathBuf::from(file_name),
                                                line: line_num as usize,
                                                column,
                                                message: message_str.to_string(),
                                                severity,
                                                code,
                                                source: "clippy".to_string(),
                                            };
                                            
                                            report.add_issue(issue);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(report)
    }
    
    /// Analyze Python code with Pylint
    pub async fn analyze_python(&self, file_paths: &[impl AsRef<Path>]) -> Result<StaticAnalysisReport> {
        let mut report = StaticAnalysisReport::new();
        report.stats.files_analyzed = file_paths.len();
        
        // Check if pylint is available
        let pylint_check = Command::new("pylint")
            .arg("--version")
            .output();
            
        if pylint_check.is_err() || !pylint_check.unwrap().status.success() {
            warn!("Pylint is not available. Skipping Python static analysis.");
            return Ok(report);
        }
        
        // Prepare command with all file paths
        let mut args = vec!["--output-format=json"];
        
        for path in file_paths {
            args.push(path.as_ref().to_str().unwrap_or_default());
        }
        
        // Run pylint
        let output = Command::new("pylint")
            .args(&args)
            .current_dir(&self.workspace_path)
            .output()
            .context("Failed to run Pylint")?;
            
        // Process pylint output
        let stdout = String::from_utf8(output.stdout).context("Invalid UTF-8 in Pylint output")?;
        
        if let Ok(pylint_issues) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
            for issue_value in pylint_issues {
                if let (
                    Some(path),
                    Some(line),
                    Some(message),
                    Some(message_id),
                    Some(symbol),
                ) = (
                    issue_value.get("path").and_then(|v| v.as_str()),
                    issue_value.get("line").and_then(|v| v.as_u64()),
                    issue_value.get("message").and_then(|v| v.as_str()),
                    issue_value.get("message-id").and_then(|v| v.as_str()),
                    issue_value.get("symbol").and_then(|v| v.as_str()),
                ) {
                    let column = issue_value.get("column").and_then(|v| v.as_u64()).map(|c| c as usize);
                    
                    let severity = match issue_value.get("type").and_then(|v| v.as_str()) {
                        Some("error") => Severity::Error,
                        Some("warning") => Severity::Warning,
                        Some("convention") => Severity::Hint,
                        Some("refactor") => Severity::Info,
                        _ => Severity::Info,
                    };
                    
                    let issue = StaticAnalysisIssue {
                        path: PathBuf::from(path),
                        line: line as usize,
                        column,
                        message: message.to_string(),
                        severity,
                        code: Some(format!("{}:{}", message_id, symbol)),
                        source: "pylint".to_string(),
                    };
                    
                    report.add_issue(issue);
                }
            }
        }
        
        Ok(report)
    }
    
    /// Analyze JavaScript/TypeScript code with ESLint
    pub async fn analyze_javascript(&self, file_paths: &[impl AsRef<Path>]) -> Result<StaticAnalysisReport> {
        let mut report = StaticAnalysisReport::new();
        report.stats.files_analyzed = file_paths.len();
        
        // Check if eslint is available
        let eslint_check = Command::new("eslint")
            .arg("--version")
            .output();
            
        if eslint_check.is_err() || !eslint_check.unwrap().status.success() {
            warn!("ESLint is not available. Skipping JavaScript/TypeScript static analysis.");
            return Ok(report);
        }
        
        // Prepare command with all file paths
        let mut args = vec!["--format=json"];
        
        for path in file_paths {
            args.push(path.as_ref().to_str().unwrap_or_default());
        }
        
        // Run eslint
        let output = Command::new("eslint")
            .args(&args)
            .current_dir(&self.workspace_path)
            .output()
            .context("Failed to run ESLint")?;
            
        // Process eslint output
        let stdout = String::from_utf8(output.stdout).context("Invalid UTF-8 in ESLint output")?;
        
        if let Ok(eslint_results) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
            for file_result in eslint_results {
                if let (Some(file_path), Some(messages)) = (
                    file_result.get("filePath").and_then(|v| v.as_str()),
                    file_result.get("messages").and_then(|v| v.as_array()),
                ) {
                    for message in messages {
                        if let (Some(line), Some(message_text)) = (
                            message.get("line").and_then(|v| v.as_u64()),
                            message.get("message").and_then(|v| v.as_str()),
                        ) {
                            let column = message.get("column").and_then(|v| v.as_u64()).map(|c| c as usize);
                            
                            let severity = match message.get("severity").and_then(|v| v.as_u64()) {
                                Some(2) => Severity::Error,
                                Some(1) => Severity::Warning,
                                _ => Severity::Info,
                            };
                            
                            let rule_id = message.get("ruleId").and_then(|v| v.as_str()).map(String::from);
                            
                            let issue = StaticAnalysisIssue {
                                path: PathBuf::from(file_path),
                                line: line as usize,
                                column,
                                message: message_text.to_string(),
                                severity,
                                code: rule_id,
                                source: "eslint".to_string(),
                            };
                            
                            report.add_issue(issue);
                        }
                    }
                }
            }
        }
        
        Ok(report)
    }
    
    /// Analyze a file based on its extension
    pub async fn analyze_file(&self, file_path: impl AsRef<Path>) -> Result<StaticAnalysisReport> {
        let path = file_path.as_ref();
        
        // Determine language based on file extension
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
            
        match extension.as_str() {
            "rs" => self.analyze_rust(&[path]).await,
            "py" => self.analyze_python(&[path]).await,
            "js" | "jsx" => self.analyze_javascript(&[path]).await,
            "ts" | "tsx" => self.analyze_javascript(&[path]).await,
            _ => {
                warn!("No static analyzer available for extension: {}", extension);
                Ok(StaticAnalysisReport::new())
            }
        }
    }
    
    /// Analyze multiple files
    pub async fn analyze_files(&self, file_paths: &[impl AsRef<Path>]) -> Result<StaticAnalysisReport> {
        let mut combined_report = StaticAnalysisReport::new();
        combined_report.stats.files_analyzed = file_paths.len();
        
        // Group files by language for more efficient analysis
        let mut rust_files = Vec::new();
        let mut python_files = Vec::new();
        let mut js_files = Vec::new();
        
        for path in file_paths {
            let path = path.as_ref();
            
            // Skip directories
            if path.is_dir() {
                continue;
            }
            
            // Group by extension
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                match ext.to_lowercase().as_str() {
                    "rs" => rust_files.push(path),
                    "py" => python_files.push(path),
                    "js" | "jsx" | "ts" | "tsx" => js_files.push(path),
                    _ => {} // Skip unsupported file types
                }
            }
        }
        
        // Run analysis for each language group
        if !rust_files.is_empty() {
            let rust_report = self.analyze_rust(&rust_files).await?;
            combined_report.issues.extend(rust_report.issues);
            combined_report.stats.errors += rust_report.stats.errors;
            combined_report.stats.warnings += rust_report.stats.warnings;
            combined_report.stats.infos += rust_report.stats.infos;
            combined_report.stats.hints += rust_report.stats.hints;
        }
        
        if !python_files.is_empty() {
            let python_report = self.analyze_python(&python_files).await?;
            combined_report.issues.extend(python_report.issues);
            combined_report.stats.errors += python_report.stats.errors;
            combined_report.stats.warnings += python_report.stats.warnings;
            combined_report.stats.infos += python_report.stats.infos;
            combined_report.stats.hints += python_report.stats.hints;
        }
        
        if !js_files.is_empty() {
            let js_report = self.analyze_javascript(&js_files).await?;
            combined_report.issues.extend(js_report.issues);
            combined_report.stats.errors += js_report.stats.errors;
            combined_report.stats.warnings += js_report.stats.warnings;
            combined_report.stats.infos += js_report.stats.infos;
            combined_report.stats.hints += js_report.stats.hints;
        }
        
        Ok(combined_report)
    }
    
    /// Get suggested fixes for issues
    pub fn get_fix_suggestions(&self, issue: &StaticAnalysisIssue) -> Option<String> {
        match (issue.source.as_str(), issue.code.as_deref()) {
            ("clippy", Some(code)) => {
                // Common clippy fix suggestions
                match code {
                    "unused_variables" => 
                        Some("Consider prefixing with underscore to mark as intentionally unused: `_variable_name`".to_string()),
                    "redundant_clone" => 
                        Some("Remove unnecessary `.clone()` call".to_string()),
                    _ => None,
                }
            },
            ("pylint", Some(code)) => {
                // Common pylint fix suggestions
                if code.contains("unused-import") {
                    Some("Remove the unused import".to_string())
                } else if code.contains("missing-docstring") {
                    Some("Add docstring to document the function/class/module".to_string())
                } else {
                    None
                }
            },
            ("eslint", Some(code)) => {
                // Common ESLint fix suggestions
                match code {
                    "no-unused-vars" => 
                        Some("Remove the unused variable or prefix with underscore".to_string()),
                    "semi" => 
                        Some("Add semicolon at the end of the line".to_string()),
                    _ => None,
                }
            },
            _ => None,
        }
    }
    
    /// Save analysis report to a file
    pub async fn save_report(
        &self, 
        report: &StaticAnalysisReport, 
        output_path: Option<impl AsRef<Path>>
    ) -> Result<PathBuf> {
        let default_path = self.workspace_path.join(format!(
            "static_analysis_report_{}.json", 
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        ));
        
        let path = output_path.map(|p| p.as_ref().to_path_buf()).unwrap_or(default_path);
        
        // Serialize the report to JSON
        let json = serde_json::to_string_pretty(report)?;
        
        // Write to file
        fs::write(&path, json).await?;
        
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_report_creation() {
        let mut report = StaticAnalysisReport::new();
        
        // Add some test issues
        report.add_issue(StaticAnalysisIssue {
            path: PathBuf::from("/test/file.rs"),
            line: 10,
            column: Some(5),
            message: "Test error".to_string(),
            severity: Severity::Error,
            code: Some("E001".to_string()),
            source: "test".to_string(),
        });
        
        report.add_issue(StaticAnalysisIssue {
            path: PathBuf::from("/test/file.rs"),
            line: 20,
            column: Some(15),
            message: "Test warning".to_string(),
            severity: Severity::Warning,
            code: Some("W001".to_string()),
            source: "test".to_string(),
        });
        
        // Check stats
        assert_eq!(report.stats.errors, 1);
        assert_eq!(report.stats.warnings, 1);
        
        // Check filtering
        let errors = report.get_issues_by_severity(Severity::Error);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "Test error");
        
        // Check file filtering
        let file_issues = report.get_issues_for_file(&PathBuf::from("/test/file.rs"));
        assert_eq!(file_issues.len(), 2);
    }
    
    #[tokio::test]
    async fn test_analyzer_creation() {
        let temp_dir = tempdir().unwrap();
        let analyzer = StaticAnalyzer::new(temp_dir.path());
        
        // Check supported languages
        let languages = analyzer.get_supported_languages();
        assert!(languages.contains(&"rust".to_string()));
        assert!(languages.contains(&"python".to_string()));
        
        // Check linters
        let rust_linters = analyzer.get_available_linters("rust");
        assert!(!rust_linters.is_empty());
        assert!(rust_linters.contains(&"clippy".to_string()));
    }
}
