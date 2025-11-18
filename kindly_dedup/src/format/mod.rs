//! Format Reader Capsule Architecture (Streaming + lockfree)
//!
//! Extensible, lockfree format support for kindly_dedup.
//!
//! # Architecture
//!
//! The format module provides a trait-based abstraction for reading documents from various formats
//! (JSONL, JSON, CSV, Plain Text). The architecture follows T5 (Streaming) + T2 (SIMD) + T1 (Atomic)
//! tiers for maximum performance and memory efficiency.
//!
//! # Core Design
//!
//! The [`FormatReaderCapsule`] trait defines a simple 3-method interface:
//! - `stream_documents()`: Returns an iterator over documents (O(1) memory)
//! - `format_name()`: Returns the format name for logging
//! - `extensions()`: Returns supported file extensions for auto-detection
//!
//! # Performance
//!
//! | Format | Implementation | Speedup | Tier |
//! |--------|---------------|---------|------|
//! | **JSONL** | simd-json | 2.31× | T5 + T2 |
//! | **JSON** | simd-json | 2.31× | T5 + T2 |
//! | **CSV** | csv crate | 1× | T5 + T1 |
//! | **Plain Text** | BufReader | 1× | T5 + T1 |
//!
//! # Example
//!
//! ```rust,ignore
//! use kindly_dedup::format::{FormatReaderCapsule, FormatRegistryCapsule};
//! use std::fs::File;
//!
//! // Auto-detect format by file extension
//! let registry = FormatRegistryCapsule::default();
//! let reader = registry.auto_detect("corpus.jsonl")?;
//!
//! let file = File::open("corpus.jsonl")?;
//! for doc_result in reader.stream_documents(file, None) {
//!     let doc = doc_result?;
//!     println!("Doc {}: {}", doc.id, doc.text);
//! }
//! ```
//!
//! # lockfree Compliance
//!
//! - **100% Lockfree**: AtomicU64 progress tracking, no mutex/RwLock
//! - **Streaming**: O(1) memory (BufReader buffering, iterator-based)
//! - **SIMD**: simd-json for 2.31× JSON speedup
//! - **Atomic**: <5ns progress increment/read
//! - **Cache-Aligned**: ProgressTrackerCapsule fits in single 64-byte cache line

pub mod error;
pub mod loader;
pub mod progress;
pub mod registry;
pub mod traits;

#[cfg(feature = "format-json")]
pub mod jsonl;

#[cfg(feature = "format-json")]
pub mod json;

#[cfg(feature = "format-csv")]
pub mod csv;

pub mod plaintext;

// Re-export public API
pub use error::FormatError;
pub use loader::{
    list_available_formats, load_documents_auto, load_documents_with_format, load_multiple_documents,
    load_multiple_documents_with_offset,
};
pub use progress::ProgressTrackerCapsule;
pub use registry::FormatRegistryCapsule;
pub use traits::{Document, FormatReaderCapsule};

#[cfg(feature = "format-csv")]
pub use csv::CsvConfig;

use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// Load documents from a file into a dedup pipeline
///
/// Auto-detects the format based on file extension.
///
/// # Arguments
///
/// - `path`: File path (or "-" for stdin)
/// - `progress`: Optional progress tracker (wrapped in Arc<AtomicU64>)
///
/// # Returns
///
/// Number of documents loaded, or FormatError if loading fails
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::DedupPipeline;
/// use kindly_dedup::format::load_corpus;
/// use atomic_capsule::CpuCapabilityCapsule;
///
/// let cpu_caps = CpuCapabilityCapsule::detect();
/// let mut pipeline = DedupPipeline::new(100_000, &cpu_caps);
///
/// let count = load_corpus(&mut pipeline, "corpus.jsonl", None)?;
/// println!("Loaded {} documents", count);
/// # Ok::<(), kindly_dedup::format::FormatError>(())
/// ```
pub fn load_corpus<P: AsRef<std::path::Path>>(
    pipeline: &mut crate::pipeline::DedupPipeline,
    path: P,
    progress: Option<Arc<AtomicU64>>,
) -> Result<usize, FormatError> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy();

    // Read file into buffer
    let buffer = if path_str == "-" {
        // stdin
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut std::io::stdin(), &mut buf).map_err(|e| FormatError::Io(e))?;
        buf
    } else {
        std::fs::read(path).map_err(|e| FormatError::Io(e))?
    };

    // Auto-detect format
    let registry = FormatRegistryCapsule::default();
    let format_reader = registry.auto_detect(&path_str)?;

    // Parse documents
    let docs = format_reader.read_from_buffer(buffer, progress);

    // Add to pipeline
    let mut count = 0usize;
    for doc_result in docs {
        let doc = doc_result?;
        pipeline
            .add_document(doc.id, &doc.text)
            .map_err(|e| FormatError::Custom(format!("Pipeline error: {}", e)))?;
        count += 1;
    }

    Ok(count)
}

/// Load documents from a file with explicit format
///
/// # Arguments
///
/// - `format`: Format name ("jsonl", "json", "csv", "txt")
/// - `path`: File path (or "-" for stdin)
/// - `progress`: Optional progress tracker
///
/// # Returns
///
/// Number of documents loaded
pub fn load_corpus_with_format<P: AsRef<std::path::Path>>(
    pipeline: &mut crate::pipeline::DedupPipeline,
    path: P,
    format: &str,
    progress: Option<Arc<AtomicU64>>,
) -> Result<usize, FormatError> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy();

    // Read file into buffer
    let buffer = if path_str == "-" {
        // stdin
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut std::io::stdin(), &mut buf).map_err(|e| FormatError::Io(e))?;
        buf
    } else {
        std::fs::read(path).map_err(|e| FormatError::Io(e))?
    };

    // Get reader for explicit format
    let registry = FormatRegistryCapsule::default();
    let format_reader = registry.get_reader(format)?;

    // Parse documents
    let docs = format_reader.read_from_buffer(buffer, progress);

    // Add to pipeline
    let mut count = 0usize;
    for doc_result in docs {
        let doc = doc_result?;
        pipeline
            .add_document(doc.id, &doc.text)
            .map_err(|e| FormatError::Custom(format!("Pipeline error: {}", e)))?;
        count += 1;
    }

    Ok(count)
}
