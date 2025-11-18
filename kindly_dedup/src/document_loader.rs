//! Document Loader - Multi-Format Support (JSONL, JSON, Plain Text)
//!
//! **Purpose**: Load documents from various file formats for deduplication
//!
//! **Supported Formats**:
//! 1. JSONL (line-delimited JSON): `{"id": "doc_0", "text": "..."}`
//! 2. JSON (array): `[{"id": "doc_0", "text": "..."}, ...]`
//! 3. Plain text: One document per line
//!
//! **Framework Compliance**:
//! - **UCE34 Q1-Q7**: Simple file I/O, no capsules needed (synchronous, bounded memory)
//! - **ASSUM**: 100% safe (no unsafe code, bounded allocations)
//! - **T28**: Comprehensive tests (unit + integration + error handling)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Document from corpus
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Document {
    /// Document ID (unique identifier)
    pub id: String,
    /// Document text content
    pub text: String,
}

/// File format detection result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// JSONL format (line-delimited JSON)
    Jsonl,
    /// JSON array format
    Json,
    /// Plain text (one document per line)
    PlainText,
}

/// Document loading error
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid JSONL at line {line}: {error}")]
    InvalidJsonl { line: usize, error: String },

    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("Empty file: {0}")]
    EmptyFile(String),

    #[error("Missing required field '{field}' in document at line {line}")]
    MissingField { field: String, line: usize },
}

// ============================================================================
// FORMAT DETECTION
// ============================================================================

/// Detect file format by examining content
///
/// **Strategy**:
/// 1. Try parse first line as JSON object → JSONL
/// 2. Try parse entire content as JSON array → JSON
/// 3. Fallback → Plain text
///
/// **Performance**: O(1) for JSONL, O(n) for JSON (parse attempt)
pub fn detect_format(content: &str) -> FileFormat {
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return FileFormat::PlainText; // Empty file defaults to plain text
    }

    // Try JSONL: First line is valid JSON object starting with '{'
    if let Some(first_line) = trimmed.lines().next() {
        let first_trimmed = first_line.trim();
        if first_trimmed.starts_with('{') && first_trimmed.ends_with('}') {
            if serde_json::from_str::<serde_json::Value>(first_trimmed).is_ok() {
                return FileFormat::Jsonl;
            }
        }
    }

    // Try JSON: Entire content is valid JSON array starting with '['
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return FileFormat::Json;
        }
    }

    // Fallback: Plain text
    FileFormat::PlainText
}

// ============================================================================
// DOCUMENT LOADERS
// ============================================================================

/// Load documents from JSONL format (line-delimited JSON)
///
/// **Format**: Each line is a JSON object with `id` and `text` fields
/// ```jsonl
/// {"id": "doc_0", "text": "Document content"}
/// {"id": "doc_1", "text": "Another document"}
/// ```
///
/// **Error Handling**: Skips invalid lines, returns error if all lines fail
pub fn load_jsonl(content: &str) -> Result<Vec<Document>, LoadError> {
    let mut documents = Vec::new();
    let mut errors = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue; // Skip empty lines
        }

        match serde_json::from_str::<Document>(trimmed) {
            Ok(doc) => {
                if doc.id.is_empty() {
                    errors.push((line_num, "empty id field".to_string()));
                } else if doc.text.is_empty() {
                    errors.push((line_num, "empty text field".to_string()));
                } else {
                    documents.push(doc);
                }
            }
            Err(e) => {
                errors.push((line_num, e.to_string()));
            }
        }
    }

    if documents.is_empty() {
        if let Some((line, error)) = errors.first() {
            return Err(LoadError::InvalidJsonl {
                line: *line,
                error: error.clone(),
            });
        }
        return Err(LoadError::EmptyFile("No valid documents found".to_string()));
    }

    Ok(documents)
}

/// Load documents from JSON array format
///
/// **Format**: Single JSON array containing document objects
/// ```json
/// [
///   {"id": "doc_0", "text": "Document content"},
///   {"id": "doc_1", "text": "Another document"}
/// ]
/// ```
pub fn load_json(content: &str) -> Result<Vec<Document>, LoadError> {
    let documents: Vec<Document> = serde_json::from_str(content).map_err(|e| LoadError::InvalidJson(e.to_string()))?;

    if documents.is_empty() {
        return Err(LoadError::EmptyFile("JSON array is empty".to_string()));
    }

    // Validate all documents have non-empty id and text
    for (idx, doc) in documents.iter().enumerate() {
        if doc.id.is_empty() {
            return Err(LoadError::MissingField {
                field: "id".to_string(),
                line: idx + 1,
            });
        }
        if doc.text.is_empty() {
            return Err(LoadError::MissingField {
                field: "text".to_string(),
                line: idx + 1,
            });
        }
    }

    Ok(documents)
}

/// Load documents from plain text format (one document per line)
///
/// **Format**: Each non-empty line is a document
/// ```text
/// First document content
/// Second document content
/// ```
///
/// **Auto-generated IDs**: doc_0, doc_1, doc_2, ...
pub fn load_plain_text(content: &str) -> Result<Vec<Document>, LoadError> {
    let mut documents = Vec::new();
    let mut doc_id = 0;

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            documents.push(Document {
                id: format!("doc_{}", doc_id),
                text: trimmed.to_string(),
            });
            doc_id += 1;
        }
    }

    if documents.is_empty() {
        return Err(LoadError::EmptyFile("No non-empty lines found".to_string()));
    }

    Ok(documents)
}

// ============================================================================
// UNIFIED LOADER
// ============================================================================

/// Load documents from file with automatic format detection
///
/// **Strategy**:
/// 1. Read file content
/// 2. Detect format (JSONL, JSON, Plain Text)
/// 3. Parse using format-specific loader
///
/// **Performance**: O(n) where n = file size
/// **Memory**: O(n) (entire file read into memory)
pub fn load_documents<P: AsRef<Path>>(path: P) -> Result<Vec<Document>, LoadError> {
    let path_ref = path.as_ref();

    if !path_ref.exists() {
        return Err(LoadError::FileNotFound(path_ref.display().to_string()));
    }

    let content = fs::read_to_string(path_ref)?;

    if content.trim().is_empty() {
        return Err(LoadError::EmptyFile(path_ref.display().to_string()));
    }

    let format = detect_format(&content);

    match format {
        FileFormat::Jsonl => load_jsonl(&content),
        FileFormat::Json => load_json(&content),
        FileFormat::PlainText => load_plain_text(&content),
    }
}

/// Load documents with explicit format (skip detection)
pub fn load_documents_with_format<P: AsRef<Path>>(path: P, format: FileFormat) -> Result<Vec<Document>, LoadError> {
    let path_ref = path.as_ref();

    if !path_ref.exists() {
        return Err(LoadError::FileNotFound(path_ref.display().to_string()));
    }

    let content = fs::read_to_string(path_ref)?;

    if content.trim().is_empty() {
        return Err(LoadError::EmptyFile(path_ref.display().to_string()));
    }

    match format {
        FileFormat::Jsonl => load_jsonl(&content),
        FileFormat::Json => load_json(&content),
        FileFormat::PlainText => load_plain_text(&content),
    }
}

// ============================================================================
// TESTS (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TIER 1: UNIT TESTS (Q1-Q7)
    // ========================================================================

    // Q1: Core behaviors
    #[test]
    fn test_detect_format_jsonl() {
        let content = r#"{"id": "doc_0", "text": "Test"}
{"id": "doc_1", "text": "Another"}"#;
        assert_eq!(detect_format(content), FileFormat::Jsonl);
    }

    #[test]
    fn test_detect_format_json() {
        let content = r#"[{"id": "doc_0", "text": "Test"}]"#;
        assert_eq!(detect_format(content), FileFormat::Json);
    }

    #[test]
    fn test_detect_format_plain_text() {
        let content = "Line 1\nLine 2\nLine 3";
        assert_eq!(detect_format(content), FileFormat::PlainText);
    }

    #[test]
    fn test_load_jsonl_valid() {
        let content = r#"{"id": "doc_0", "text": "First document"}
{"id": "doc_1", "text": "Second document"}"#;

        let docs = load_jsonl(content).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].id, "doc_0");
        assert_eq!(docs[0].text, "First document");
        assert_eq!(docs[1].id, "doc_1");
        assert_eq!(docs[1].text, "Second document");
    }

    #[test]
    fn test_load_json_valid() {
        let content = r#"[
            {"id": "doc_0", "text": "First document"},
            {"id": "doc_1", "text": "Second document"}
        ]"#;

        let docs = load_json(content).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].id, "doc_0");
        assert_eq!(docs[1].id, "doc_1");
    }

    #[test]
    fn test_load_plain_text_valid() {
        let content = "First document\nSecond document\n\nThird document";

        let docs = load_plain_text(content).unwrap();
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].id, "doc_0");
        assert_eq!(docs[0].text, "First document");
        assert_eq!(docs[1].id, "doc_1");
        assert_eq!(docs[2].id, "doc_2");
    }

    // Q2: Edge cases
    #[test]
    fn test_load_jsonl_empty_lines() {
        let content = r#"{"id": "doc_0", "text": "First"}

{"id": "doc_1", "text": "Second"}

"#;

        let docs = load_jsonl(content).unwrap();
        assert_eq!(docs.len(), 2); // Empty lines skipped
    }

    #[test]
    fn test_load_jsonl_partial_invalid() {
        let content = r#"{"id": "doc_0", "text": "Valid"}
{invalid json}
{"id": "doc_2", "text": "Also valid"}"#;

        let docs = load_jsonl(content).unwrap();
        assert_eq!(docs.len(), 2); // Invalid line skipped
        assert_eq!(docs[0].id, "doc_0");
        assert_eq!(docs[1].id, "doc_2");
    }

    #[test]
    fn test_load_json_empty_array() {
        let content = "[]";
        let result = load_json(content);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoadError::EmptyFile(_)));
    }

    #[test]
    fn test_load_plain_text_empty() {
        let content = "\n\n\n";
        let result = load_plain_text(content);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoadError::EmptyFile(_)));
    }

    // Q3: Error handling
    #[test]
    fn test_load_jsonl_all_invalid() {
        let content = "{invalid}\n{also invalid}";
        let result = load_jsonl(content);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoadError::InvalidJsonl { .. }));
    }

    #[test]
    fn test_load_json_invalid() {
        let content = "{not an array}";
        let result = load_json(content);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoadError::InvalidJson(_)));
    }

    #[test]
    fn test_load_jsonl_missing_id() {
        let content = r#"{"text": "Missing id field"}"#;
        let result = load_jsonl(content);
        assert!(result.is_err()); // All docs invalid → error
    }

    #[test]
    fn test_load_jsonl_empty_text() {
        let content = r#"{"id": "doc_0", "text": ""}"#;
        let result = load_jsonl(content);
        assert!(result.is_err()); // All docs invalid → error
    }

    // Q4: Boundary values
    #[test]
    fn test_detect_format_empty() {
        assert_eq!(detect_format(""), FileFormat::PlainText);
        assert_eq!(detect_format("   \n\n  "), FileFormat::PlainText);
    }

    #[test]
    fn test_load_plain_text_single_line() {
        let docs = load_plain_text("Single document").unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id, "doc_0");
    }

    #[test]
    fn test_load_jsonl_single_document() {
        let docs = load_jsonl(r#"{"id": "doc_0", "text": "Only one"}"#).unwrap();
        assert_eq!(docs.len(), 1);
    }

    // Q5: Input validation
    #[test]
    fn test_detect_format_invalid_json_like() {
        // Starts with '{' but invalid JSON → Plain text
        let content = "{not valid json";
        assert_eq!(detect_format(content), FileFormat::PlainText);
    }

    #[test]
    fn test_detect_format_array_like_but_invalid() {
        // Starts with '[' but invalid JSON → Plain text
        let content = "[not valid json";
        assert_eq!(detect_format(content), FileFormat::PlainText);
    }

    // Q6: State management (stateless module, N/A)

    // Q7: Concurrency (single-threaded reads, no concurrency issues)
}
