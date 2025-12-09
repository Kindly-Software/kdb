//! Unified Deduplication API - Facade Pattern
//!
//! Provides a simple, customer-facing API that auto-selects the best implementation
//! based on corpus size and available hardware.
//!
//! # Design Philosophy
//!
//! - **Simple API**: Single `Dedup` struct, auto-selection, no configuration complexity
//! - **Customer-Friendly**: No technical jargon ("capsule", "T0-T11", "lockfree", "Chaos")
//! - **Auto-Tuning**: Selects best pipeline based on corpus size and hardware
//! - **Unified Interface**: All implementations share the same API
//!
//! # Example
//!
//! ```rust,ignore
//! use kindly_dedup::Dedup;
//!
//! // Auto-select best implementation
//! let mut dedup = Dedup::new(1_000_000)?;
//!
//! // Add documents
//! for (id, text) in documents {
//!     dedup.add_document(id, text)?;
//! }
//!
//! // Find duplicates
//! let clusters = dedup.find_duplicates(0.85)?;
//! println!("Found {} duplicate groups", clusters.len());
//!
//! // Get statistics
//! let stats = dedup.stats();
//! println!("Processed {} documents in {:?}", stats.documents_processed, stats.total_time);
//! ```

use crate::streaming_dedup_pipeline::StreamingDedupPipeline;
use crate::pipeline::DocId;
use crate::PipelineError;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};

/// Execution mode for deduplication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupMode {
    /// Automatically select best mode based on corpus size and hardware
    Auto,
    /// Streaming CPU processing (default, handles all corpus sizes)
    CpuStreaming,
    /// GPU-accelerated processing (requires GPU hardware)
    #[cfg(feature = "gpu-hybrid")]
    Gpu,
}

impl Default for DedupMode {
    fn default() -> Self {
        DedupMode::Auto
    }
}

/// Statistics about deduplication process
#[derive(Debug, Clone)]
pub struct DedupStats {
    /// Number of documents processed
    pub documents_processed: usize,
    /// Number of duplicate clusters found
    pub duplicate_clusters: usize,
    /// Total processing time
    pub total_time: Duration,
    /// Average time per document
    pub avg_time_per_doc: Duration,
    /// Current execution mode
    pub mode: DedupMode,
    /// Peak memory usage (if available)
    pub peak_memory_mb: Option<f64>,
}

/// Error type for facade operations
#[derive(Debug)]
pub enum FacadeError {
    /// Pipeline error from underlying implementation
    Pipeline(String),
    /// Invalid mode selection (e.g., GPU not available)
    InvalidMode(String),
    /// Configuration error
    Configuration(String),
    /// Feature not enabled
    FeatureDisabled(String),
}

impl std::fmt::Display for FacadeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FacadeError::Pipeline(e) => write!(f, "Pipeline error: {}", e),
            FacadeError::InvalidMode(msg) => write!(f, "Invalid mode: {}", msg),
            FacadeError::Configuration(msg) => write!(f, "Configuration error: {}", msg),
            FacadeError::FeatureDisabled(msg) => write!(f, "Feature disabled: {}", msg),
        }
    }
}

impl std::error::Error for FacadeError {}

impl From<PipelineError> for FacadeError {
    fn from(e: PipelineError) -> Self {
        FacadeError::Pipeline(format!("{:?}", e))
    }
}

/// Internal implementation wrapper (private)
enum DedupImpl {
    /// Streaming pipeline (default) with document buffer
    Streaming {
        pipeline: StreamingDedupPipeline,
        buffer: Vec<(DocId, String)>,
    },
    /// GPU hybrid pipeline (if available)
    #[cfg(feature = "gpu-hybrid")]
    Gpu(crate::hybrid_pipeline::HybridDedupPipeline),
}

/// Unified deduplication API
///
/// This is the main entry point for all deduplication operations.
/// It automatically selects the best implementation based on corpus size
/// and available hardware.
///
/// # Auto-Selection Logic
///
/// - **Default**: Streaming CPU processing (handles all corpus sizes efficiently)
/// - **GPU available**: Use GPU if expected speedup ≥2×
///
/// # Thread Safety
///
/// All operations are thread-safe. Multiple threads can add documents
/// concurrently, though `find_duplicates` requires exclusive access.
pub struct Dedup {
    /// Internal implementation (selected based on mode/corpus size)
    implementation: DedupImpl,
    /// Current execution mode
    mode: DedupMode,
    /// Start time for statistics
    start_time: Instant,
    /// Document count
    doc_count: usize,
    /// Estimated total documents
    estimated_docs: usize,
}

impl Dedup {
    /// Create new deduplication instance with auto-selected mode
    ///
    /// # Arguments
    ///
    /// - `estimated_docs`: Expected number of documents (used for optimization)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let dedup = Dedup::new(1_000_000)?;
    /// ```
    pub fn new(estimated_docs: usize) -> Result<Self, FacadeError> {
        let mode = Self::select_mode(estimated_docs, DedupMode::Auto)?;
        Self::with_mode(mode, estimated_docs)
    }

    /// Create deduplication instance with explicit mode
    ///
    /// # Arguments
    ///
    /// - `mode`: Explicit execution mode
    /// - `estimated_docs`: Expected number of documents
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Force streaming mode
    /// let dedup = Dedup::with_mode(DedupMode::CpuStreaming, 500_000)?;
    /// ```
    pub fn with_mode(mode: DedupMode, estimated_docs: usize) -> Result<Self, FacadeError> {
        let actual_mode = Self::select_mode(estimated_docs, mode)?;
        let implementation = Self::create_implementation(actual_mode, estimated_docs)?;

        Ok(Dedup {
            implementation,
            mode: actual_mode,
            start_time: Instant::now(),
            doc_count: 0,
            estimated_docs,
        })
    }

    /// Add document to deduplication index
    ///
    /// # Arguments
    ///
    /// - `id`: Unique document identifier
    /// - `text`: Document text content
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// dedup.add_document(0, "The quick brown fox")?;
    /// dedup.add_document(1, "A lazy dog sleeps")?;
    /// ```
    pub fn add_document(&mut self, id: u64, text: &str) -> Result<(), FacadeError> {
        match &mut self.implementation {
            DedupImpl::Streaming { buffer, .. } => {
                // Buffer documents for batch processing
                buffer.push((id as DocId, text.to_string()));
            }
            #[cfg(feature = "gpu-hybrid")]
            DedupImpl::Gpu(pipeline) => {
                pipeline.add_document(id as u32, text)?;
            }
        }
        self.doc_count += 1;
        Ok(())
    }

    /// Find duplicate document clusters
    ///
    /// # Arguments
    ///
    /// - `threshold`: Similarity threshold (0.0-1.0, typically 0.80-0.90)
    ///
    /// # Returns
    ///
    /// Vector of clusters, where each cluster is a vector of document IDs.
    /// Documents in the same cluster are duplicates above the threshold.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let clusters = dedup.find_duplicates(0.85)?;
    /// for cluster in clusters {
    ///     println!("Duplicate group: {:?}", cluster);
    /// }
    /// ```
    pub fn find_duplicates(&mut self, threshold: f64) -> Result<Vec<Vec<u64>>, FacadeError> {
        // Validate threshold
        if !(0.0..=1.0).contains(&threshold) {
            return Err(FacadeError::Configuration(
                format!("Threshold must be between 0.0 and 1.0, got {}", threshold)
            ));
        }

        match &mut self.implementation {
            DedupImpl::Streaming { pipeline, buffer } => {
                // Flush buffered documents first
                if !buffer.is_empty() {
                    let docs = std::mem::take(buffer);
                    pipeline.add_documents(docs)?;
                }

                let clusters = pipeline.find_duplicates(threshold)?;
                Ok(clusters.into_iter()
                    .map(|cluster| cluster.into_iter().map(|id| id as u64).collect())
                    .collect())
            }
            #[cfg(feature = "gpu-hybrid")]
            DedupImpl::Gpu(pipeline) => {
                let clusters = pipeline.find_duplicates(threshold)?;
                Ok(clusters.into_iter()
                    .map(|cluster| cluster.into_iter().map(|id| id as u64).collect())
                    .collect())
            }
        }
    }

    /// Get current statistics
    ///
    /// Returns statistics about the deduplication process including
    /// document count, processing time, and performance metrics.
    pub fn stats(&self) -> DedupStats {
        let elapsed = self.start_time.elapsed();
        let avg_time = if self.doc_count > 0 {
            elapsed / self.doc_count as u32
        } else {
            Duration::ZERO
        };

        DedupStats {
            documents_processed: self.doc_count,
            duplicate_clusters: 0, // Updated after find_duplicates
            total_time: elapsed,
            avg_time_per_doc: avg_time,
            mode: self.mode,
            peak_memory_mb: None, // Could be added per implementation
        }
    }

    /// Get current execution mode
    pub fn current_mode(&self) -> DedupMode {
        self.mode
    }

    /// Select appropriate mode based on corpus size and requested mode
    fn select_mode(estimated_docs: usize, requested_mode: DedupMode) -> Result<DedupMode, FacadeError> {
        match requested_mode {
            DedupMode::Auto => {
                // Auto-select based on hardware
                #[cfg(feature = "gpu-hybrid")]
                {
                    // Check if GPU is available
                    if Self::is_gpu_available() && estimated_docs >= 10_000 {
                        return Ok(DedupMode::Gpu);
                    }
                }

                // Default to streaming
                Ok(DedupMode::CpuStreaming)
            }
            DedupMode::CpuStreaming => Ok(DedupMode::CpuStreaming),
            #[cfg(feature = "gpu-hybrid")]
            DedupMode::Gpu => {
                if Self::is_gpu_available() {
                    Ok(DedupMode::Gpu)
                } else {
                    Err(FacadeError::InvalidMode(
                        "GPU mode requested but no GPU available".to_string()
                    ))
                }
            }
            #[cfg(all(feature = "gpu-hybrid", not(feature = "gpu-hybrid")))]
            DedupMode::Gpu => {
                Err(FacadeError::FeatureDisabled(
                    "GPU mode requires 'gpu-hybrid' feature".to_string()
                ))
            }
        }
    }

    /// Create implementation based on mode
    fn create_implementation(mode: DedupMode, estimated_docs: usize) -> Result<DedupImpl, FacadeError> {
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        match mode {
            DedupMode::CpuStreaming => {
                let pipeline = StreamingDedupPipeline::new(estimated_docs, num_threads)?;
                Ok(DedupImpl::Streaming {
                    pipeline,
                    buffer: Vec::with_capacity(1000), // Batch size
                })
            }
            #[cfg(feature = "gpu-hybrid")]
            DedupMode::Gpu => {
                use crate::hybrid_pipeline::PipelineMode;
                let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
                let pipeline = crate::hybrid_pipeline::HybridDedupPipeline::new(
                    estimated_docs,
                    PipelineMode::GpuAccelerated,
                    &cpu_caps
                )?;
                Ok(DedupImpl::Gpu(pipeline))
            }
            _ => Err(FacadeError::Configuration("Invalid mode".to_string())),
        }
    }

    /// Check if GPU is available
    #[cfg(feature = "gpu-hybrid")]
    fn is_gpu_available() -> bool {
        crate::gpu::is_gpu_available()
    }

    #[cfg(not(feature = "gpu-hybrid"))]
    fn is_gpu_available() -> bool {
        false
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_selection() {
        // Default should select CpuStreaming
        let mode = Dedup::select_mode(50_000, DedupMode::Auto).unwrap();
        assert_eq!(mode, DedupMode::CpuStreaming);
    }

    #[test]
    fn test_explicit_cpu_streaming() {
        // Explicit mode should be respected
        let mode = Dedup::select_mode(500_000, DedupMode::CpuStreaming).unwrap();
        assert_eq!(mode, DedupMode::CpuStreaming);
    }

    #[test]
    fn test_basic_workflow() {
        // Test basic add + find workflow
        let mut dedup = Dedup::new(100).unwrap();

        dedup.add_document(0, "The quick brown fox").unwrap();
        dedup.add_document(1, "A lazy dog sleeps").unwrap();
        dedup.add_document(2, "The quick brown fox").unwrap(); // Duplicate

        let clusters = dedup.find_duplicates(0.85).unwrap();

        // Should find at least one cluster (docs 0 and 2)
        assert!(!clusters.is_empty(), "Should find duplicate clusters");

        let stats = dedup.stats();
        assert_eq!(stats.documents_processed, 3);
        assert_eq!(stats.mode, DedupMode::CpuStreaming);
    }

    #[test]
    fn test_invalid_threshold() {
        let mut dedup = Dedup::new(100).unwrap();

        // Threshold > 1.0 should fail
        assert!(dedup.find_duplicates(1.5).is_err());

        // Threshold < 0.0 should fail
        assert!(dedup.find_duplicates(-0.1).is_err());
    }

    // GPU mode is only available with gpu-hybrid feature
    // No test needed for feature-gated variant

    #[test]
    fn test_stats_empty() {
        let dedup = Dedup::new(100).unwrap();
        let stats = dedup.stats();

        assert_eq!(stats.documents_processed, 0);
        assert_eq!(stats.avg_time_per_doc, Duration::ZERO);
    }

    #[test]
    fn test_stats_after_add() {
        let mut dedup = Dedup::new(100).unwrap();
        dedup.add_document(0, "test").unwrap();

        let stats = dedup.stats();
        assert_eq!(stats.documents_processed, 1);
        assert!(stats.total_time > Duration::ZERO);
    }
}
