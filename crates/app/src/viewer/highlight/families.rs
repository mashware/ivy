//! Which language family a file is read as, and the words each family colours.
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    Rust,
    /// C, C++, Go, Java, Kotlin, Swift, C#, JavaScript, TypeScript and SCSS: line and block comments.
    CLike,
    /// CSS: block comments only, so a URL is never read as a comment.
    Css,
    Php,
    /// Python, shells, Ruby and the configuration formats: `#` comments.
    Hash,
    Json,
    Sql,
    Plain,
}

impl Family {
    /// The family a file is read as: by extension, then by a few well-known names, then by a
    /// `#!` first line. Anything else is plain text.
    pub(crate) fn of(path: &Path, first_line: &str) -> Self {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let extension = path
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase());
        let by_extension = match extension.as_deref() {
            Some("rs") => Some(Self::Rust),
            Some(
                "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" | "go" | "java" | "kt" | "kts" | "swift"
                | "cs" | "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" | "scss",
            ) => Some(Self::CLike),
            Some("css") => Some(Self::Css),
            Some("php") => Some(Self::Php),
            Some(
                "py" | "sh" | "bash" | "zsh" | "fish" | "rb" | "yaml" | "yml" | "toml" | "ini"
                | "cfg" | "conf" | "env",
            ) => Some(Self::Hash),
            Some("json") => Some(Self::Json),
            Some("sql") => Some(Self::Sql),
            Some("txt" | "log" | "csv" | "md" | "markdown") => Some(Self::Plain),
            _ => None,
        };
        by_extension.unwrap_or_else(|| match name.as_str() {
            "Makefile" | "makefile" | "Dockerfile" | ".env" => Self::Hash,
            _ if first_line.starts_with("#!") => Self::Hash,
            _ => Self::Plain,
        })
    }

    /// Whether `word` is one of this family's keywords.
    pub(crate) fn is_keyword(self, word: &str) -> bool {
        let words: &[&str] = match self {
            Self::Rust => RUST,
            Self::CLike => C_LIKE,
            Self::Php => PHP,
            Self::Hash => HASH,
            Self::Json => JSON,
            Self::Css => &[],
            Self::Sql => return SQL.iter().any(|k| k.eq_ignore_ascii_case(word)),
            Self::Plain => &[],
        };
        words.contains(&word)
    }

    /// Whether a capitalised identifier reads as a type name in this family.
    pub(crate) fn capital_is_type(self) -> bool {
        matches!(self, Self::Rust | Self::CLike | Self::Php)
    }
}

const RUST: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

const C_LIKE: &[&str] = &[
    "abstract",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "defer",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "final",
    "finally",
    "for",
    "fun",
    "func",
    "function",
    "go",
    "if",
    "implements",
    "import",
    "in",
    "interface",
    "let",
    "namespace",
    "new",
    "null",
    "nil",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "struct",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typedef",
    "typeof",
    "val",
    "var",
    "void",
    "while",
    "yield",
];

const PHP: &[&str] = &[
    "abstract",
    "array",
    "as",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "echo",
    "else",
    "elseif",
    "enum",
    "extends",
    "false",
    "final",
    "finally",
    "fn",
    "for",
    "foreach",
    "function",
    "if",
    "implements",
    "interface",
    "match",
    "namespace",
    "new",
    "null",
    "private",
    "protected",
    "public",
    "readonly",
    "require",
    "require_once",
    "return",
    "static",
    "switch",
    "throw",
    "trait",
    "true",
    "try",
    "use",
    "while",
    "yield",
];

const HASH: &[&str] = &[
    "and", "as", "case", "class", "def", "do", "done", "elif", "else", "end", "esac", "export",
    "False", "fi", "for", "from", "function", "if", "import", "in", "lambda", "local", "module",
    "None", "not", "or", "pass", "raise", "return", "then", "True", "try", "while", "with",
    "yield", "false", "true", "null",
];

const JSON: &[&str] = &["true", "false", "null"];

const SQL: &[&str] = &[
    "select",
    "from",
    "where",
    "insert",
    "into",
    "values",
    "update",
    "set",
    "delete",
    "create",
    "table",
    "index",
    "alter",
    "drop",
    "join",
    "left",
    "right",
    "inner",
    "outer",
    "on",
    "and",
    "or",
    "not",
    "null",
    "as",
    "group",
    "by",
    "order",
    "having",
    "limit",
    "primary",
    "key",
    "foreign",
    "references",
    "default",
    "unique",
];
