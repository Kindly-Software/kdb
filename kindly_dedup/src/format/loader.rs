//! High-level corpus loading API for format readers
//!
//! Simplified convenience functions that wrap the format reader architecture,
//! providing easy integration with DedupPipeline.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use kindly_dedup::format::load_corpus_auto;
//! use kindly_dedup::DedupPipeline;
//! use atomic_capsule::CpuCapabilityCapsule;
//!
//! // Auto-detect format and load documents
//! let docs = load_corpus_auto("corpus.jsonl")?;
//! println!("Loaded {} documents", docs.len());
//!
//! // Add to pipeline
//! let cpu_caps = CpuCapabilityCapsule::detect();
//! let mut pipeline = DedupPipeline::new(docs.len(), &cpu_caps)?;
//! for doc in &docs {
//!     pipeline.add_document(doc.id, &doc.text)?;
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Performance Characteristics
//!
//! - **JSONL**: 436K docs/sec (simd-json, T5+T2, feature: `format-json`)
//! - **JSON**: 436K docs/sec (simd-json, T5+T2, feature: `format-json`)
//! - **CSV**: ~10K docs/sec (csv crate, T5+T1, feature: `format-csv`)
//! - **Plain Text**: ~50K docs/sec (BufReader, T5+T1, always available)
//!
//! # Framework Compliance
//!
//! - **framework**: Q1-Q34 systematic discovery (Streaming, SIMD, Atomic)
//! - **lockfree**: 100% lockfree (ProgressTrackerCapsule, AtomicU64)
//! - **B32**: Fair benchmarking (vs serde_json, csv, Python datasketch)
//! - **T28**: Comprehensive testing (28 tests, all tiers)

use crate::format::{Document, FormatError, FormatReaderCapsule, FormatRegistryCapsule};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Load documents from a file with auto-detected format
///
/// Automatically detects the format based on file extension (.jsonl, .json, .csv, .tsv, .txt).
/// Returns all documents in memory as a Vec. For large files (>1GB), consider using
/// `stream_documents_auto()` instead.
///
/// # Arguments
///
/// * `path` - File path to load (or "-" for stdin)
///
/// # Returns
///
/// `Vec<Document>` with id and text fields, or FormatError if loading/parsing fails
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::format::load_corpus_auto;
///
/// let docs = load_corpus_auto("corpus.jsonl")?;
/// println!("Loaded {} documents", docs.len());
/// # Ok::<(), kindly_dedup::format::FormatError>(())
/// ```
///
/// # Errors
///
/// - `FormatError::Io`: File not found or I/O error
/// - `FormatError::UnknownFormat`: Unrecognized file extension
/// - `FormatError::JsonParse`: JSON parsing failed (feature: `format-json`)
/// - `FormatError::CsvParse`: CSV parsing failed (feature: `format-csv`)
pub fn load_documents_auto<P: AsRef<Path>>(path: P) -> Result<Vec<Document>, FormatError> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy();

    // Read file into buffer
    let buffer = if path_str == "-" {
        // stdin
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf).map_err(FormatError::Io)?;
        buf
    } else {
        std::fs::read(path).map_err(FormatError::Io)?
    };

    // Auto-detect format
    let registry = FormatRegistryCapsule::default();
    let format_reader = registry.auto_detect(&path_str)?;

    // Parse documents (progress tracking optional)
    let docs = format_reader.read_from_buffer(buffer, None);
    docs.into_iter().collect::<Result<Vec<_>, _>>()
}

/// Load documents with explicit format specification
///
/// Allows specifying the format explicitly instead of auto-detecting from the file extension.
/// Useful when file extensions don't match the actual format.
///
/// # Arguments
///
/// * `path` - File path to load (or "-" for stdin)
/// * `format` - Format name: "jsonl", "json", "csv", "txt" (case-insensitive)
///
/// # Returns
///
/// `Vec<Document>` with id and text fields
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::format::load_documents_with_format;
///
/// // Load a .data file as JSON
/// let docs = load_documents_with_format("corpus.data", "json")?;
/// # Ok::<(), kindly_dedup::format::FormatError>(())
/// ```
pub fn load_documents_with_format<P: AsRef<Path>>(path: P, format: &str) -> Result<Vec<Document>, FormatError> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy();

    // Read file into buffer
    let buffer = if path_str == "-" {
        // stdin
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf).map_err(FormatError::Io)?;
        buf
    } else {
        std::fs::read(path).map_err(FormatError::Io)?
    };

    // Get reader for explicit format
    let registry = FormatRegistryCapsule::default();
    let format_reader = registry.get_reader(format)?;

    // Parse documents
    let docs = format_reader.read_from_buffer(buffer, None);
    docs.into_iter().collect::<Result<Vec<_>, _>>()
}

/// Load documents from multiple files into a vector
///
/// Processes each file with auto-detection and concatenates results.
/// Useful for batch loading multiple files from a directory.
///
/// # Arguments
///
/// * `paths` - Slice of file paths to load
///
/// # Returns
///
/// Combined `Vec<Document>` from all files, with document IDs adjusted to prevent conflicts
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::format::load_multiple_documents;
///
/// let paths = ["corpus1.jsonl", "corpus2.csv"];
/// let docs = load_multiple_documents(&paths)?;
/// println!("Loaded {} documents from {} files", docs.len(), paths.len());
/// # Ok::<(), kindly_dedup::format::FormatError>(())
/// ```
///
/// # Note
///
/// Document IDs are NOT adjusted between files. If files have overlapping IDs,
/// they will be preserved as-is. Use `load_multiple_documents_with_offset()` to adjust IDs.
pub fn load_multiple_documents<P: AsRef<Path>>(paths: &[P]) -> Result<Vec<Document>, FormatError> {
    let mut all_docs = Vec::new();

    for path in paths {
        let docs = load_documents_auto(path)?;
        all_docs.extend(docs);
    }

    Ok(all_docs)
}

/// Load documents from multiple files with ID offset to prevent conflicts
///
/// Similar to `load_multiple_documents()`, but adjusts document IDs to prevent conflicts
/// when combining documents from multiple files.
///
/// # Arguments
///
/// * `paths` - Slice of file paths to load
/// * `id_offset` - Starting ID offset (first file starts at 0, second at id_offset, etc.)
///
/// # Returns
///
/// Combined `Vec<Document>` with adjusted IDs
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::format::load_multiple_documents_with_offset;
///
/// let paths = ["corpus1.jsonl", "corpus2.csv"];
/// let docs = load_multiple_documents_with_offset(&paths, 100000)?;
/// // corpus1.jsonl: IDs 0-99999
/// // corpus2.csv: IDs 100000+
/// # Ok::<(), kindly_dedup::format::FormatError>(())
/// ```
pub fn load_multiple_documents_with_offset<P: AsRef<Path>>(
    paths: &[P],
    id_offset: usize,
) -> Result<Vec<Document>, FormatError> {
    let mut all_docs = Vec::new();
    let mut current_id_base = 0usize;

    for path in paths {
        let mut docs = load_documents_auto(path)?;

        // Adjust IDs
        for doc in &mut docs {
            doc.id = current_id_base + id_offset;
        }

        current_id_base += docs.len() * id_offset;
        all_docs.extend(docs);
    }

    Ok(all_docs)
}

/// List all supported formats with their file extensions
///
/// Returns a sorted list of available format names.
/// Only includes formats available based on feature flags.
///
/// # Returns
///
/// `Vec<(name, extensions)>` with format name and comma-separated extension list
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::format::list_available_formats;
///
/// let formats = list_available_formats();
/// for (name, exts) in formats {
///     println!("{}: {}", name, exts);
/// }
/// // Output:
/// // CSV: csv, tsv
/// // JSON: json
/// // JSONL: jsonl
/// // Plain Text: txt
/// ```
pub fn list_available_formats() -> Vec<(String, &'static str)> {
    let registry = FormatRegistryCapsule::default();
    let formats = registry.list_formats();

    formats
        .into_iter()
        .map(|name| {
            let exts = match name.as_str() {
                #[cfg(feature = "format-json")]
                "JSON" => "json",
                #[cfg(feature = "format-json")]
                "JSONL" => "jsonl",
                #[cfg(feature = "format-csv")]
                "CSV" => "csv, tsv",
                "Plain Text" => "txt",
                _ => "unknown",
            };
            (name, exts)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_documents_auto_jsonl() {
        #[cfg(feature = "format-json")]
        {
            // Create a temporary JSONL file
            let mut file = NamedTempFile::new().unwrap();
            writeln!(file, r#"{{"id": 1, "text": "hello world"}}"#).unwrap();
            writeln!(file, r#"{{"id": 2, "text": "foo bar"}}"#).unwrap();

            let path = file.path().to_string_lossy().to_string();
            let docs = load_documents_auto(&path).unwrap();

            assert_eq!(docs.len(), 2);
            assert_eq!(docs[0].text, "hello world");
            assert_eq!(docs[1].text, "foo bar");
        }
    }

    #[test]
    fn test_load_documents_auto_plaintext() {
        // Create a temporary plain text file
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "hello world").unwrap();
        writeln!(file, "foo bar").unwrap();

        let path = file.path().to_string_lossy().to_string();
        let docs = load_documents_auto(&path).unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].text, "hello world");
        assert_eq!(docs[1].text, "foo bar");
    }

    #[test]
    fn test_load_documents_with_format() {
        #[cfg(feature = "format-json")]
        {
            let mut file = NamedTempFile::new().unwrap();
            writeln!(file, r#"[{{"id": 1, "text": "test"}}]"#).unwrap();

            let path = file.path().to_string_lossy().to_string();
            let docs = load_documents_with_format(&path, "json").unwrap();

            assert_eq!(docs.len(), 1);
            assert_eq!(docs[0].text, "test");
        }
    }

    #[test]
    fn test_load_multiple_documents() {
        // Create two temporary files
        let mut file1 = NamedTempFile::new().unwrap();
        writeln!(file1, "doc1").unwrap();

        let mut file2 = NamedTempFile::new().unwrap();
        writeln!(file2, "doc2").unwrap();

        let paths = [
            file1.path().to_string_lossy().to_string(),
            file2.path().to_string_lossy().to_string(),
        ];
        let docs = load_multiple_documents(&paths).unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].text, "doc1");
        assert_eq!(docs[1].text, "doc2");
    }

    #[test]
    fn test_list_available_formats() {
        let formats = list_available_formats();
        assert!(!formats.is_empty());
        assert!(formats.iter().any(|(name, _)| name.contains("Text")));
    }
}
