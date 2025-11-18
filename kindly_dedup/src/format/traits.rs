//! Core format reader traits and types

use crate::format::FormatError;
use std::io::Read;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// A document loaded from a format reader
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Document ID (unique identifier)
    pub id: usize,

    /// Document text content
    pub text: String,

    /// Optional metadata (URL, filename, etc.)
    pub url: Option<String>,
}

/// Format reader capsule trait
///
/// Defines the interface for reading documents from various formats.
/// Implementations must be Send + Sync for thread-safe usage.
///
/// # lockfree Compliance
///
/// - **Streaming**: Returns Iterator, consumes O(1) memory
/// - **100% Lockfree**: No mutex/RwLock usage
/// - **Thread-Safe**: Send + Sync enforced at trait level
///
/// # Object Safety
///
/// This trait is object-safe (no generic associated types).
/// It can be used as `Arc<dyn FormatReaderCapsule>` for runtime dispatch.
pub trait FormatReaderCapsule: Send + Sync {
    /// Get the format name (for logging and diagnostics)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let reader = JsonlReaderCapsule::new();
    /// assert_eq!(reader.format_name(), "JSONL");
    /// ```
    fn format_name(&self) -> &'static str;

    /// Get supported file extensions (without leading dot)
    ///
    /// Used for auto-detection and validation.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let reader = JsonlReaderCapsule::new();
    /// assert_eq!(reader.extensions(), &["jsonl"]);
    /// ```
    fn extensions(&self) -> &'static [&'static str];

    /// Stream documents from a byte buffer
    ///
    /// Takes owned buffer (Vec<u8>) instead of generic Read trait
    /// to allow object-safe trait usage with Arc<dyn>.
    ///
    /// # Arguments
    ///
    /// - `buffer`: Bytes to parse (owned, not borrowed)
    /// - `progress`: Optional progress tracker (AtomicU64, updated on each document)
    ///
    /// # Returns
    ///
    /// Vec<Result<Document, FormatError>>
    ///
    /// # Performance
    ///
    /// - **Latency**: Per-document parsing time (format-dependent)
    /// - **Memory**: O(N) where N = number of documents
    /// - **Throughput**: Format-dependent (2-10 MB/s typical)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use kindly_dedup::format::FormatReaderCapsule;
    /// use std::fs;
    ///
    /// let reader = JsonlReaderCapsule::new();
    /// let buffer = fs::read("corpus.jsonl")?;
    ///
    /// let docs = reader.read_from_buffer(buffer, None);
    /// for doc_result in docs {
    ///     let doc = doc_result?;
    ///     println!("Loaded: {}", doc.text);
    /// }
    /// ```
    fn read_from_buffer(&self, buffer: Vec<u8>, progress: Option<Arc<AtomicU64>>)
        -> Vec<Result<Document, FormatError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_creation() {
        let doc = Document {
            id: 1,
            text: "Hello, world!".to_string(),
            url: Some("http://example.com".to_string()),
        };

        assert_eq!(doc.id, 1);
        assert_eq!(doc.text, "Hello, world!");
        assert_eq!(doc.url, Some("http://example.com".to_string()));
    }

    #[test]
    fn test_document_equality() {
        let doc1 = Document {
            id: 1,
            text: "Test".to_string(),
            url: None,
        };

        let doc2 = Document {
            id: 1,
            text: "Test".to_string(),
            url: None,
        };

        assert_eq!(doc1, doc2);
    }

    #[test]
    fn test_document_clone() {
        let doc = Document {
            id: 1,
            text: "Test".to_string(),
            url: None,
        };

        let cloned = doc.clone();
        assert_eq!(doc, cloned);
    }
}
