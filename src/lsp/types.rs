use serde::{Deserialize, Serialize};

/// Language identifiers supported by LSP
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    CPlusPlus,
    Ruby,
}

impl Language {
    /// Get file extension matcher for this language
    pub fn get_file_extension(&self) -> &'static [&'static str] {
        match self {
            Language::Rust => &["rs"],
            Language::Python => &["py", "pyi", "pyx", "pyd"],
            Language::JavaScript => &["js", "jsx", "mjs"],
            Language::TypeScript => &["ts", "tsx"],
            Language::Go => &["go"],
            Language::Java => &["java"],
            Language::CSharp => &["cs"],
            Language::CPlusPlus => &["cpp", "cc", "cxx", "h", "hpp", "hxx"],
            Language::Ruby => &["rb"],
        }
    }

    /// Check if a file name is relevant for this language
    pub fn is_relevant_filename(&self, filename: &str) -> bool {
        let extensions = self.get_file_extension();
        let filename_lower = filename.to_lowercase();
        extensions
            .iter()
            .any(|ext| filename_lower.ends_with(&format!(".{}", ext)))
    }
}

/// Represents the location of a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolLocation {
    /// Relative path to the file containing the symbol
    pub relative_path: Option<String>,

    /// Line where the symbol is defined (0-based)
    pub line: Option<usize>,

    /// Column where the symbol is defined (0-based)
    pub column: Option<usize>,
}

/// Symbol kinds as defined in the LSP specification
/// https://microsoft.github.io/language-server-protocol/specifications/specification-current/#symbolKind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

/// Position in a text document (0-based line and column)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}

/// A range in a text document (start and end positions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// A symbol in a document, as returned by the language server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Name of the symbol (e.g., function name, class name)
    pub name: String,

    /// Kind of the symbol
    pub kind: SymbolKind,

    /// Location where the symbol is defined
    pub location: SymbolLocation,

    /// Range of the symbol's body in the file
    pub range: Range,

    /// Code body of the symbol (if requested)
    pub body: Option<String>,

    /// Children symbols, if any
    pub children: Vec<Symbol>,
}

/// Configuration for a language server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSPConfig {
    /// Language to use
    pub language: Language,

    /// Paths to ignore (glob patterns)
    pub ignored_paths: Vec<String>,

    /// Content of the .gitignore file, if any
    pub gitignore_content: Option<String>,

    /// Trace LSP communication for debugging
    pub trace_lsp_communication: bool,
}
