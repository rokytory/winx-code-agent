use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use tracing::{debug, info, warn};

/// Represents the syntax validation result
#[derive(Debug, Clone)]
pub struct SyntaxValidationResult {
    /// Whether the syntax is valid
    pub is_valid: bool,
    /// Error message if any
    pub error_message: Option<String>,
    /// Line number of error if any
    pub error_line: Option<usize>,
    /// Column number of error if any
    pub error_column: Option<usize>,
    /// File that was validated
    pub file_path: PathBuf,
    /// Language of the file
    pub language: String,
}

impl SyntaxValidationResult {
    /// Create a new valid result
    pub fn valid(file_path: impl AsRef<Path>, language: impl Into<String>) -> Self {
        Self {
            is_valid: true,
            error_message: None,
            error_line: None,
            error_column: None,
            file_path: file_path.as_ref().to_path_buf(),
            language: language.into(),
        }
    }

    /// Create a new invalid result
    pub fn invalid(
        file_path: impl AsRef<Path>,
        language: impl Into<String>,
        error_message: impl Into<String>,
        error_line: Option<usize>,
        error_column: Option<usize>,
    ) -> Self {
        Self {
            is_valid: false,
            error_message: Some(error_message.into()),
            error_line,
            error_column,
            file_path: file_path.as_ref().to_path_buf(),
            language: language.into(),
        }
    }
}

/// Syntax checker for validating code
pub struct SyntaxChecker {
    // Tree-sitter parsers could be added here if enabled
}

impl SyntaxChecker {
    /// Create a new syntax checker
    pub fn new() -> Self {
        Self {}
    }

    /// Validate syntax of a file
    pub async fn validate_file_syntax(
        &self,
        file_path: impl AsRef<Path>,
    ) -> Result<SyntaxValidationResult> {
        let file_path = file_path.as_ref();
        info!("Validating syntax of {}", file_path.display());

        // Get file extension to determine language
        let extension = file_path
            .extension()
            .map(|ext| ext.to_string_lossy().to_lowercase())
            .ok_or_else(|| anyhow!("File has no extension"))?;

        let language = match extension.as_str() {
            "rs" => "rust",
            "py" => "python",
            "js" => "javascript",
            "jsx" => "javascript",
            "ts" => "typescript",
            "tsx" => "typescript",
            "json" => "json",
            "c" | "h" => "c",
            "cpp" | "cc" | "cxx" | "hpp" => "cpp",
            "go" => "go",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "java" => "java",
            "kt" => "kotlin",
            "scala" => "scala",
            "sh" => "bash",
            "sql" => "sql",
            "html" | "htm" => "html",
            "css" => "css",
            "scss" => "scss",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            _ => return Err(anyhow!("Unsupported file extension: {}", extension)),
        };

        // Use language-specific syntax checkers
        match language {
            "rust" => self.validate_rust(file_path),
            "python" => self.validate_python(file_path),
            "javascript" | "typescript" => self.validate_js_ts(file_path, language),
            "json" => self.validate_json(file_path),
            _ => {
                // For other languages, use a generic "does it parse" approach
                warn!(
                    "No specialized syntax checker for {}; using basic validation",
                    language
                );
                self.generic_syntax_check(file_path, language)
            }
        }
    }

    /// Validate syntax of Rust code
    fn validate_rust(&self, file_path: impl AsRef<Path>) -> Result<SyntaxValidationResult> {
        let file_path = file_path.as_ref();
        debug!("Using rustc to validate {}", file_path.display());

        // Use rustc for syntax checking
        let output = Command::new("rustc")
            .args(["--error-format=json", "--emit=metadata", "--crate-type=lib"])
            .arg(file_path)
            .output()
            .context("Failed to execute rustc for syntax checking")?;

        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            return Ok(SyntaxValidationResult::valid(file_path, "rust"));
        }

        // Try to parse the error message
        for line in stderr.lines() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(message) = json.get("message") {
                    let error_text = message
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");
                    let spans = message
                        .get("spans")
                        .and_then(|s| s.as_array())
                        .unwrap_or(&vec![]);

                    // Find the primary span for line/column info
                    let primary_span = spans.iter().find(|span| {
                        span.get("is_primary")
                            .and_then(|p| p.as_bool())
                            .unwrap_or(false)
                    });

                    let line_num = primary_span
                        .and_then(|s| s.get("line_start"))
                        .and_then(|l| l.as_u64())
                        .map(|l| l as usize);

                    let column = primary_span
                        .and_then(|s| s.get("column_start"))
                        .and_then(|c| c.as_u64())
                        .map(|c| c as usize);

                    return Ok(SyntaxValidationResult::invalid(
                        file_path, "rust", error_text, line_num, column,
                    ));
                }
            }
        }

        // If we can't parse the error, return a generic error
        Ok(SyntaxValidationResult::invalid(
            file_path,
            "rust",
            format!(
                "Syntax error: {}",
                stderr.lines().next().unwrap_or("Unknown error")
            ),
            None,
            None,
        ))
    }

    /// Validate syntax of Python code
    fn validate_python(&self, file_path: impl AsRef<Path>) -> Result<SyntaxValidationResult> {
        let file_path = file_path.as_ref();
        debug!("Using python to validate {}", file_path.display());

        // Use Python's compile function to check syntax
        let output = Command::new("python")
            .args(["-m", "py_compile", file_path.to_string_lossy().as_ref()])
            .output()
            .context("Failed to execute python for syntax checking")?;

        if output.status.success() {
            return Ok(SyntaxValidationResult::valid(file_path, "python"));
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let error_text = stderr.trim();

        // Try to parse line number from error message
        // Format is typically: "file.py:line:col: SyntaxError: message"
        let mut line_num = None;
        let mut column = None;

        if let Some(line_info) = error_text.split(':').nth(1) {
            if let Ok(line) = usize::from_str(line_info) {
                line_num = Some(line);

                // Try to get column info
                if let Some(col_info) = error_text.split(':').nth(2) {
                    if let Ok(col) = usize::from_str(col_info) {
                        column = Some(col);
                    }
                }
            }
        }

        Ok(SyntaxValidationResult::invalid(
            file_path,
            "python",
            error_text.to_string(),
            line_num,
            column,
        ))
    }

    /// Validate syntax of JavaScript/TypeScript code
    fn validate_js_ts(
        &self,
        file_path: impl AsRef<Path>,
        language: &str,
    ) -> Result<SyntaxValidationResult> {
        let file_path = file_path.as_ref();
        debug!("Using node to validate {}", file_path.display());

        let is_typescript = language == "typescript";
        let script_content = format!(
            "try {{ 
                require('{}').readFileSync('{}', 'utf-8'); 
                process.exit(0); 
            }} catch(e) {{ 
                console.error(e.message); 
                process.exit(1); 
            }}",
            if is_typescript { "typescript" } else { "fs" },
            file_path.to_string_lossy().replace('\\', "\\\\")
        );

        // Use node to evaluate a simple script that loads and parses the file
        let output = Command::new("node")
            .args(["-e", &script_content])
            .output()
            .context("Failed to execute node for syntax checking")?;

        if output.status.success() {
            return Ok(SyntaxValidationResult::valid(file_path, language));
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let error_text = stderr.trim();

        // Try to parse line and column from error
        // Format varies, but often contains something like "line X, column Y"
        let mut line_num = None;
        let mut column = None;

        if let Some(line_pos) = error_text.find("line ") {
            let remaining = &error_text[line_pos + 5..];
            if let Some(end_pos) = remaining.find(',') {
                if let Ok(line) = usize::from_str(&remaining[..end_pos].trim()) {
                    line_num = Some(line);
                }
            }
        }

        if let Some(col_pos) = error_text.find("column ") {
            let remaining = &error_text[col_pos + 7..];
            if let Some(end_pos) = remaining.find(|c: char| !c.is_digit(10)) {
                if let Ok(col) = usize::from_str(&remaining[..end_pos].trim()) {
                    column = Some(col);
                }
            }
        }

        Ok(SyntaxValidationResult::invalid(
            file_path,
            language,
            error_text.to_string(),
            line_num,
            column,
        ))
    }

    /// Validate syntax of JSON
    fn validate_json(&self, file_path: impl AsRef<Path>) -> Result<SyntaxValidationResult> {
        let file_path = file_path.as_ref();
        debug!("Validating JSON syntax: {}", file_path.display());

        // Read the file content
        let content = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read JSON file: {}", file_path.display()))?;

        // Try to parse as JSON
        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(_) => Ok(SyntaxValidationResult::valid(file_path, "json")),
            Err(e) => {
                // serde_json errors often include line/column info
                let error_text = e.to_string();
                let mut line_num = None;
                let mut column = None;

                // Try to extract line and column from error message
                if let Some(line_pos) = error_text.find("line ") {
                    let remaining = &error_text[line_pos + 5..];
                    if let Some(end_pos) = remaining.find(|c: char| !c.is_digit(10)) {
                        if let Ok(line) = usize::from_str(&remaining[..end_pos].trim()) {
                            line_num = Some(line);
                        }
                    }
                }

                if let Some(col_pos) = error_text.find("column ") {
                    let remaining = &error_text[col_pos + 7..];
                    if let Some(end_pos) = remaining.find(|c: char| !c.is_digit(10)) {
                        if let Ok(col) = usize::from_str(&remaining[..end_pos].trim()) {
                            column = Some(col);
                        }
                    }
                }

                Ok(SyntaxValidationResult::invalid(
                    file_path, "json", error_text, line_num, column,
                ))
            }
        }
    }

    /// Generic syntax check
    fn generic_syntax_check(
        &self,
        file_path: impl AsRef<Path>,
        language: &str,
    ) -> Result<SyntaxValidationResult> {
        let file_path = file_path.as_ref();
        debug!("Doing basic validation for {}", file_path.display());

        // Simple check: just verify file exists and is readable
        match std::fs::read_to_string(file_path) {
            Ok(_) => Ok(SyntaxValidationResult::valid(file_path, language)),
            Err(e) => Ok(SyntaxValidationResult::invalid(
                file_path,
                language,
                format!("Could not read file: {}", e),
                None,
                None,
            )),
        }
    }

    /// Validate syntax of source code string
    pub fn validate_source(
        &self,
        content: &str,
        language: &str,
        extension: Option<&str>,
    ) -> Result<bool> {
        let ext = match extension {
            Some(ext) => ext.to_string(),
            None => match language {
                "rust" => "rs".to_string(),
                "python" => "py".to_string(),
                "javascript" => "js".to_string(),
                "typescript" => "ts".to_string(),
                "json" => "json".to_string(),
                "c" => "c".to_string(),
                "cpp" => "cpp".to_string(),
                "go" => "go".to_string(),
                "ruby" => "rb".to_string(),
                "php" => "php".to_string(),
                "swift" => "swift".to_string(),
                "java" => "java".to_string(),
                "kotlin" => "kt".to_string(),
                "scala" => "scala".to_string(),
                "bash" => "sh".to_string(),
                "sql" => "sql".to_string(),
                "html" => "html".to_string(),
                "css" => "css".to_string(),
                "scss" => "scss".to_string(),
                "yaml" => "yaml".to_string(),
                "toml" => "toml".to_string(),
                _ => return Err(anyhow!("Unsupported language: {}", language)),
            },
        };

        // Create a temporary file with the content
        let temp_dir = tempfile::tempdir()?;
        let temp_file = temp_dir.path().join(format!("temp.{}", ext));

        std::fs::write(&temp_file, content).with_context(|| {
            format!("Failed to write to temporary file: {}", temp_file.display())
        })?;

        // Validate the temporary file
        let result = self.validate_file_syntax(&temp_file)?;

        Ok(result.is_valid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_json_validation() {
        let checker = SyntaxChecker::new();
        let temp_dir = tempdir().unwrap();

        // Valid JSON
        let valid_json = r#"{"name": "test", "value": 42}"#;
        let valid_file = temp_dir.path().join("valid.json");
        File::create(&valid_file)
            .unwrap()
            .write_all(valid_json.as_bytes())
            .unwrap();

        let result = checker.validate_json(&valid_file).unwrap();
        assert!(result.is_valid);

        // Invalid JSON
        let invalid_json = r#"{"name": "test", value: 42}"#; // Missing quotes around value
        let invalid_file = temp_dir.path().join("invalid.json");
        File::create(&invalid_file)
            .unwrap()
            .write_all(invalid_json.as_bytes())
            .unwrap();

        let result = checker.validate_json(&invalid_file).unwrap();
        assert!(!result.is_valid);
    }

    #[test]
    fn test_generic_checker() {
        let checker = SyntaxChecker::new();
        let temp_dir = tempdir().unwrap();

        // Existing file
        let file = temp_dir.path().join("test.txt");
        File::create(&file)
            .unwrap()
            .write_all(b"Hello world")
            .unwrap();

        let result = checker.generic_syntax_check(&file, "text").unwrap();
        assert!(result.is_valid);

        // Non-existing file
        let nonexistent = temp_dir.path().join("nonexistent.txt");
        let result = checker.generic_syntax_check(&nonexistent, "text").unwrap();
        assert!(!result.is_valid);
    }
}
