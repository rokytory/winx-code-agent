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
        self.issues
            .iter()
            .filter(|issue| issue.severity == severity)
            .collect()
    }

    /// Get issues for a specific file
    pub fn get_issues_for_file(&self, file_path: &Path) -> Vec<&StaticAnalysisIssue> {
        self.issues
            .iter()
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
                summary.push_str(&format!(
                    "- [ERROR] {}:{}: {}\n",
                    issue.path.display(),
                    issue.line,
                    issue.message
                ));
            }

            for issue in warnings.iter().take(5) {
                summary.push_str(&format!(
                    "- [WARNING] {}:{}: {}\n",
                    issue.path.display(),
                    issue.line,
                    issue.message
                ));
            }

            // If there are more issues than shown
            let shown = errors.len().min(5) + warnings.len().min(5);
            if self.issues.len() > shown {
                summary.push_str(&format!(
                    "- ... and {} more issues\n",
                    self.issues.len() - shown
                ));
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
        available_linters.insert(
            "python".to_string(),
            vec!["pylint".to_string(), "mypy".to_string()],
        );
        available_linters.insert("javascript".to_string(), vec!["eslint".to_string()]);
        available_linters.insert(
            "typescript".to_string(),
            vec!["eslint".to_string(), "tsc".to_string()],
        );

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
        self.available_linters
            .get(language)
            .cloned()
            .unwrap_or_default()
    }

    /// Analyze Rust code with Clippy
    pub async fn analyze_rust(
        &self,
        file_paths: &[impl AsRef<Path>],
    ) -> Result<StaticAnalysisReport> {
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
            .context("Failed to run clippy")?;

        if !output.status.success() {
            // Even if clippy returns non-zero, we still want to parse its output
            debug!("Clippy exited with non-zero status code");
        }

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(json) => {
                    // Look for diagnostic messages
                    if let Some(message) = json.get("message") {
                        if let (Some(level), Some(message_text), Some(spans)) = (
                            message.get("level"),
                            message.get("message"),
                            message.get("spans"),
                        ) {
                            if let (Some(level_str), Some(message_str)) =
                                (level.as_str(), message_text.as_str())
                            {
                                // Get severity
                                let severity = match level_str {
                                    "error" => Severity::Error,
                                    "warning" => Severity::Warning,
                                    _ => Severity::Info,
                                };

                                // Get the primary span (location)
                                if let Some(spans_array) = spans.as_array() {
                                    for span in spans_array {
                                        if let (
                                            Some(is_primary),
                                            Some(file_name),
                                            Some(line_number),
                                        ) = (
                                            span.get("is_primary"),
                                            span.get("file_name"),
                                            span.get("line_start"),
                                        ) {
                                            if is_primary.as_bool() == Some(true) {
                                                let column = span
                                                    .get("column_start")
                                                    .and_then(|c| c.as_u64())
                                                    .map(|c| c as usize);

                                                // Create issue
                                                let issue = StaticAnalysisIssue {
                                                    path: PathBuf::from(
                                                        file_name.as_str().unwrap_or(""),
                                                    ),
                                                    line: line_number.as_u64().unwrap_or(0)
                                                        as usize,
                                                    column,
                                                    message: message_str.to_string(),
                                                    severity,
                                                    code: json
                                                        .get("code")
                                                        .and_then(|c| c.get("code"))
                                                        .and_then(|c| c.as_str())
                                                        .map(|s| s.to_string()),
                                                    source: "clippy".to_string(),
                                                };

                                                report.add_issue(issue);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    // Skip lines that aren't valid JSON
                    debug!("Failed to parse clippy output line as JSON: {}", e);
                }
            }
        }

        Ok(report)
    }

    /// Analyze Python code with pylint
    pub async fn analyze_python(
        &self,
        file_paths: &[impl AsRef<Path>],
    ) -> Result<StaticAnalysisReport> {
        let mut report = StaticAnalysisReport::new();
        report.stats.files_analyzed = file_paths.len();

        // Check if pylint is available
        let pylint_check = Command::new("pylint").arg("--version").output();

        if pylint_check.is_err() || !pylint_check.unwrap().status.success() {
            warn!("pylint is not available. Skipping Python static analysis.");
            return Ok(report);
        }

        // Convert all paths to strings
        let file_path_strs: Vec<String> = file_paths
            .iter()
            .map(|p| p.as_ref().to_string_lossy().to_string())
            .collect();

        if file_path_strs.is_empty() {
            return Ok(report);
        }

        // Run pylint with JSON output
        let mut cmd = Command::new("pylint");
        cmd.arg("--output-format=json");

        // Add all file paths
        for path in &file_path_strs {
            cmd.arg(path);
        }

        let output = cmd.output().context("Failed to run pylint")?;

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.trim().is_empty() {
            return Ok(report);
        }

        match serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
            Ok(issues) => {
                for issue in issues {
                    if let (
                        Some(path),
                        Some(line),
                        Some(message),
                        Some(message_id),
                        Some(symbol),
                        Some(type_),
                    ) = (
                        issue.get("path"),
                        issue.get("line"),
                        issue.get("message"),
                        issue.get("message-id"),
                        issue.get("symbol"),
                        issue.get("type"),
                    ) {
                        if let (Some(path_str), Some(line_num), Some(message_str), Some(type_str)) = (
                            path.as_str(),
                            line.as_u64(),
                            message.as_str(),
                            type_.as_str(),
                        ) {
                            // Map pylint type to severity
                            let severity = match type_str {
                                "error" | "fatal" => Severity::Error,
                                "warning" => Severity::Warning,
                                "convention" => Severity::Hint,
                                _ => Severity::Info,
                            };

                            let column = issue
                                .get("column")
                                .and_then(|c| c.as_u64())
                                .map(|c| c as usize);

                            // Create issue
                            let issue = StaticAnalysisIssue {
                                path: PathBuf::from(path_str),
                                line: line_num as usize,
                                column,
                                message: message_str.to_string(),
                                severity,
                                code: Some(format!(
                                    "{} ({})",
                                    message_id.as_str().unwrap_or(""),
                                    symbol.as_str().unwrap_or("")
                                )),
                                source: "pylint".to_string(),
                            };

                            report.add_issue(issue);
                        }
                    }
                }
            }
            Err(e) => {
                debug!("Failed to parse pylint output as JSON: {}", e);
                debug!("Pylint output: {}", stdout);
            }
        }

        Ok(report)
    }

    /// Analyze JavaScript/TypeScript with ESLint
    pub async fn analyze_js_ts(
        &self,
        file_paths: &[impl AsRef<Path>],
    ) -> Result<StaticAnalysisReport> {
        let mut report = StaticAnalysisReport::new();
        report.stats.files_analyzed = file_paths.len();

        // Check if eslint is available
        let eslint_check = Command::new("npx").args(["eslint", "--version"]).output();

        if eslint_check.is_err() || !eslint_check.unwrap().status.success() {
            warn!("ESLint is not available. Skipping JavaScript/TypeScript static analysis.");
            return Ok(report);
        }

        // Convert all paths to strings
        let file_path_strs: Vec<String> = file_paths
            .iter()
            .map(|p| p.as_ref().to_string_lossy().to_string())
            .collect();

        if file_path_strs.is_empty() {
            return Ok(report);
        }

        // Run eslint with JSON output
        let mut cmd = Command::new("npx");
        cmd.arg("eslint").arg("--format=json");

        // Add all file paths
        for path in &file_path_strs {
            cmd.arg(path);
        }

        let output = cmd.output().context("Failed to run eslint")?;

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.trim().is_empty() {
            return Ok(report);
        }

        match serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
            Ok(file_results) => {
                for file_result in file_results {
                    if let (Some(file_path), Some(messages)) =
                        (file_result.get("filePath"), file_result.get("messages"))
                    {
                        if let (Some(path_str), Some(messages_array)) =
                            (file_path.as_str(), messages.as_array())
                        {
                            for message in messages_array {
                                if let (Some(line), Some(message_text), Some(severity_num)) = (
                                    message.get("line"),
                                    message.get("message"),
                                    message.get("severity"),
                                ) {
                                    if let (Some(line_num), Some(message_str), Some(severity_val)) = (
                                        line.as_u64(),
                                        message_text.as_str(),
                                        severity_num.as_u64(),
                                    ) {
                                        // Map ESLint severity to our severity
                                        // ESLint: 0 = off, 1 = warning, 2 = error
                                        let severity = match severity_val {
                                            2 => Severity::Error,
                                            1 => Severity::Warning,
                                            _ => Severity::Info,
                                        };

                                        let column = message
                                            .get("column")
                                            .and_then(|c| c.as_u64())
                                            .map(|c| c as usize);
                                        let rule_id = message
                                            .get("ruleId")
                                            .and_then(|r| r.as_str())
                                            .map(|s| s.to_string());

                                        // Create issue
                                        let issue = StaticAnalysisIssue {
                                            path: PathBuf::from(path_str),
                                            line: line_num as usize,
                                            column,
                                            message: message_str.to_string(),
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
                }
            }
            Err(e) => {
                debug!("Failed to parse eslint output as JSON: {}", e);
                debug!("ESLint output: {}", stdout);
            }
        }

        Ok(report)
    }

    /// Analyze a project based on its primary language
    pub async fn analyze_project(
        &self,
        project_analysis: &crate::code::analysis::ProjectAnalysis,
    ) -> Result<StaticAnalysisReport> {
        let mut report = StaticAnalysisReport::new();

        // Check if we have primary languages
        if project_analysis.primary_languages.is_empty() {
            warn!("No primary languages detected. Skipping static analysis.");
            return Ok(report);
        }

        // Group files by language
        let mut files_by_language: HashMap<String, Vec<PathBuf>> = HashMap::new();

        for file in &project_analysis.files {
            // Skip large files
            if file.size > 1_000_000 {
                continue;
            }

            // Skip test files for now
            if file.is_test {
                continue;
            }

            let language = file.language.to_lowercase();
            let simple_lang = if language.contains("typescript") {
                "typescript".to_string()
            } else if language.contains("javascript") {
                "javascript".to_string()
            } else if language.contains("python") {
                "python".to_string()
            } else if language == "rust" {
                "rust".to_string()
            } else {
                continue;
            };

            files_by_language
                .entry(simple_lang)
                .or_insert_with(Vec::new)
                .push(file.path.clone());
        }

        // Analyze each language
        for (language, files) in files_by_language {
            if files.is_empty() {
                continue;
            }

            info!("Running static analysis for {} files", language);

            let lang_report = match language.as_str() {
                "rust" => self.analyze_rust(&files).await?,
                "python" => self.analyze_python(&files).await?,
                "javascript" | "typescript" => self.analyze_js_ts(&files).await?,
                _ => continue,
            };

            // Merge reports
            for issue in lang_report.issues {
                report.add_issue(issue);
            }

            report.stats.files_analyzed += lang_report.stats.files_analyzed;
        }

        Ok(report)
    }
}
