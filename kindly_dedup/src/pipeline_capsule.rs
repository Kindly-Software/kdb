//! # Dedup-Specific Pipeline Capsule
//!
//! **T6 Mixed (Container Capsule)**: Dedup-specific wrapper around ParallelDedupPipeline.
//!
//! This module provides convenient type aliases and helper methods for using the generic
//! PipelineCapsule with deduplication pipelines.
//!
//! ## Architecture
//!
//! ```text
//! DedupPipelineCapsule<'a>
//! └── PipelineCapsule<ParallelDedupPipeline<'a>>
//!     ├── pipeline: ParallelDedupPipeline<'a>      (Owned, dedup logic)
//!     ├── progress: Arc<ProgressTrackerCapsule>    (Shared, atomic counters)
//!     └── callback: Optional dedup-specific hook   (User notifications)
//! ```
//!
//! ## Performance
//!
//! - **Overhead**: <5% vs baseline pipeline
//! - **Throughput**: ≥365K docs/sec (vs 373K baseline, Phase 11 measured)
//! - **Latency**: 2.7 µs per document end-to-end
//!
//! ## Example
//!
//! ```rust,ignore
//! use kindly_dedup::DedupPipelineCapsule;
//! use atomic_capsule::CpuCapabilityCapsule;
//!
//! let cpu_caps = CpuCapabilityCapsule::detect();
//! let mut capsule = DedupPipelineCapsule::new_dedup(
//!     1_000_000,
//!     16,
//!     &cpu_caps,
//! )?;
//!
//! // Progress is automatically tracked
//! for (doc_id, text) in documents.iter() {
//!     capsule.add_document(*doc_id, text)?;
//!     capsule.record_success(*doc_id); // Record progress
//! }
//!
//! // Get real-time metrics
//! let percent = capsule.progress().percent_complete();
//! let throughput = capsule.progress().throughput();
//! let eta = capsule.progress().eta_seconds();
//! ```

use crate::parallel_pipeline::ParallelDedupPipeline;
use crate::pipeline::{DocId, JaccardThreshold, PipelineError};
use atomic_capsule::patterns::pipeline_capsule::{PipelineCapsule, ProgressCallback, ProgressTrackerCapsule};
use atomic_capsule::CpuCapabilityCapsule;
use std::sync::Arc;

/// Dedup-specific progress callback trait (extends ProgressCallback)
pub trait DedupProgressCallback: ProgressCallback {
    /// Called when a duplicate pair is found
    fn on_duplicate_found(&self, doc_id1: DocId, doc_id2: DocId, similarity: f64);

    /// Called when a document signature is computed
    fn on_signature_computed(&self, doc_id: DocId);
}

/// Type alias for dedup pipeline capsule
pub type DedupPipelineCapsule = PipelineCapsule<ParallelDedupPipeline>;

/// Create new dedup pipeline capsule with progress tracking
///
/// # Arguments
/// - `num_documents`: Total documents to process
/// - `num_threads`: Number of worker threads
/// - `cpu_caps`: CPU capability detection capsule
///
/// # Returns
/// - `Ok(capsule)`: Successfully created capsule
/// - `Err(error)`: Thread pool creation or initialization failed
///
/// # Performance
/// - Initialization: <10ms for 10M documents
/// - Memory: O(n) for signature storage + 512 KB Bloom filter
///
/// # Example
/// ```rust,ignore
/// use kindly_dedup::new_dedup_capsule;
/// use atomic_capsule::CpuCapabilityCapsule;
///
/// let cpu_caps = CpuCapabilityCapsule::detect();
/// let capsule = new_dedup_capsule(1_000_000, 16, &cpu_caps)?;
/// ```
pub fn new_dedup_capsule(
    num_documents: usize,
    num_threads: usize,
    cpu_caps: &CpuCapabilityCapsule,
) -> Result<DedupPipelineCapsule, PipelineError> {
    let pipeline = ParallelDedupPipeline::new(num_documents, num_threads, cpu_caps)?;
    Ok(PipelineCapsule::new(pipeline, num_documents as u64))
}

/// Create dedup pipeline capsule with optional callback
///
/// # Arguments
/// - `num_documents`: Total documents to process
/// - `num_threads`: Number of worker threads
/// - `cpu_caps`: CPU capability detection capsule
/// - `callback`: Optional progress callback (Arc-wrapped for thread safety)
///
/// # Returns
/// - `Ok(capsule)`: Successfully created capsule with callback
/// - `Err(error)`: Thread pool creation failed
///
/// # Example
/// ```rust,ignore
/// use kindly_dedup::new_dedup_capsule_with_callback;
///
/// let callback = Arc::new(MyCallback);
/// let capsule = new_dedup_capsule_with_callback(1_000_000, 16, &cpu_caps, callback)?;
/// ```
pub fn new_dedup_capsule_with_callback(
    num_documents: usize,
    num_threads: usize,
    cpu_caps: &CpuCapabilityCapsule,
    callback: Arc<dyn ProgressCallback>,
) -> Result<DedupPipelineCapsule, PipelineError> {
    let pipeline = ParallelDedupPipeline::new(num_documents, num_threads, cpu_caps)?;
    Ok(PipelineCapsule::with_callback(pipeline, num_documents as u64, callback))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_capsule_creation() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let result = new_dedup_capsule(100, 2, &cpu_caps);
        assert!(result.is_ok(), "Failed to create dedup capsule");
    }

    #[test]
    fn test_dedup_capsule_progress_tracking() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let capsule = new_dedup_capsule(100, 2, &cpu_caps).expect("Failed to create capsule");

        let progress = capsule.progress();
        assert_eq!(progress.total_items(), 100);
        assert_eq!(progress.percent_complete(), 0);
    }

    #[test]
    fn test_dedup_capsule_phase_tracking() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let capsule = new_dedup_capsule(100, 2, &cpu_caps).expect("Failed to create capsule");

        capsule.set_phase(1); // BUILD phase
        assert_eq!(capsule.progress().phase(), 1);

        capsule.set_phase(2); // PROBE phase
        assert_eq!(capsule.progress().phase(), 2);
    }

    #[test]
    fn test_dedup_capsule_with_callback() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct TestCallback {
            item_count: AtomicUsize,
        }

        impl ProgressCallback for TestCallback {
            fn on_item_processed(&self, _item_id: u64, _success: bool) {
                self.item_count.fetch_add(1, Ordering::Relaxed);
            }
            fn on_phase_changed(&self, _phase: u8) {}
            fn on_error(&self, _error: &str) {}
        }

        let cpu_caps = CpuCapabilityCapsule::detect();
        let callback = Arc::new(TestCallback {
            item_count: AtomicUsize::new(0),
        });

        let result = new_dedup_capsule_with_callback(100, 2, &cpu_caps, callback.clone());
        assert!(result.is_ok());

        let capsule = result.unwrap();
        capsule.record_success(1);
        assert_eq!(callback.item_count.load(Ordering::Relaxed), 1);
    }
}
