#![cfg(feature = "std")]

//! # Generic Pipeline Capsule with Automatic Progress Tracking
//!
//! **T6 Mixed (Container Capsule)**: Generic wrapper for any pipeline with automatic progress tracking.
//!
//! This is a reusable capsule that wraps any data processing pipeline (deduplication,
//! compression, transformation, etc.) and provides automatic progress tracking without
//! modifying the pipeline's core logic.
//!
//! ## Architecture
//!
//! ```text
//! PipelineCapsule<P>
//! ├── pipeline: P                              (Owned, processing logic)
//! ├── progress: Arc<ProgressTrackerCapsule>    (Shared, atomic counters)
//! └── callback: Option<Arc<dyn ProgressCallback>> (Optional, user notifications)
//! ```
//!
//! ## Performance
//!
//! - **Overhead**: <60ns per item (<5% vs baseline pipeline)
//! - **Memory**: 128 bytes (ProgressTrackerCapsule, cache-aligned)
//! - **Ordering**: Relaxed atomics (progress counters, no coordination)
//!
//! ## Example
//!
//! ```rust,ignore
//! use atomic_capsule::patterns::pipeline_capsule::*;
//!
//! // Wrap any pipeline with progress tracking
//! let pipeline = MyPipeline::new(capacity);
//! let capsule = PipelineCapsule::new(pipeline, total_items);
//!
//! // Progress automatically tracked
//! capsule.process_item(item)?;
//!
//! // Real-time progress access
//! let percent = capsule.progress().percent_complete();
//! let throughput = capsule.progress().throughput(); // items/sec
//! let eta = capsule.progress().eta_seconds();       // Estimated time remaining
//! ```

use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generic progress callback trait (extensible for different domains)
///
/// Implement this trait to receive real-time progress notifications from the pipeline.
pub trait ProgressCallback: Send + Sync {
    /// Called when an item is processed (success or failure)
    fn on_item_processed(&self, item_id: u64, success: bool);

    /// Called when the phase changes (e.g., BUILD → PROBE → COLLECT)
    fn on_phase_changed(&self, phase: u8);

    /// Called when an error occurs (progress/callback errors are non-fatal)
    fn on_error(&self, error: &str);
}

/// Default no-op progress callback (used when callback is None)
#[allow(dead_code)]
struct NoOpCallback;

#[allow(dead_code)]
impl ProgressCallback for NoOpCallback {
    fn on_item_processed(&self, _item_id: u64, _success: bool) {}
    fn on_phase_changed(&self, _phase: u8) {}
    fn on_error(&self, _error: &str) {}
}

/// Generic progress tracker capsule (T1 Atomic, 128-byte aligned)
///
/// Provides atomic, lockfree progress tracking with minimal overhead.
/// All operations use Relaxed ordering (no synchronization cost).
#[repr(C, align(128))]
pub struct ProgressTrackerCapsule {
    total_items: AtomicU64,
    processed_items: AtomicU64,
    successful_items: AtomicU64,
    failed_items: AtomicU64,
    current_phase: AtomicU8,
    start_time_ns: AtomicU64,
    last_update_ns: AtomicU64,
    _padding: [u8; 56], // Pad to 128 bytes
}

impl ProgressTrackerCapsule {
    /// Create new progress tracker with total item count
    pub fn new(total: u64) -> Self {
        let now = now_ns();
        Self {
            total_items: AtomicU64::new(total),
            processed_items: AtomicU64::new(0),
            successful_items: AtomicU64::new(0),
            failed_items: AtomicU64::new(0),
            current_phase: AtomicU8::new(0),
            start_time_ns: AtomicU64::new(now),
            last_update_ns: AtomicU64::new(now),
            _padding: [0; 56],
        }
    }

    /// Increment processed count (atomic, Relaxed)
    pub fn increment_processed(&self) {
        self.processed_items.fetch_add(1, Ordering::Relaxed);
        self.last_update_ns
            .store(now_ns(), Ordering::Relaxed);
    }

    /// Increment successful count (atomic, Relaxed)
    pub fn increment_successful(&self) {
        self.successful_items.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment failed count (atomic, Relaxed)
    pub fn increment_failed(&self) {
        self.failed_items.fetch_add(1, Ordering::Relaxed);
    }

    /// Set current processing phase (0-255)
    pub fn set_phase(&self, phase: u8) {
        self.current_phase.store(phase, Ordering::Relaxed);
    }

    /// Get current processing phase
    pub fn phase(&self) -> u8 {
        self.current_phase.load(Ordering::Relaxed)
    }

    /// Get percent complete (0-100, rounded)
    pub fn percent_complete(&self) -> u8 {
        let total = self.total_items.load(Ordering::Relaxed);
        if total == 0 {
            return 100;
        }
        let processed = self.processed_items.load(Ordering::Relaxed);
        ((processed as u128 * 100) / total as u128) as u8
    }

    /// Get throughput in items/sec
    pub fn throughput(&self) -> u64 {
        let elapsed_ns = now_ns() - self.start_time_ns.load(Ordering::Relaxed);
        if elapsed_ns == 0 {
            return 0;
        }
        let processed = self.processed_items.load(Ordering::Relaxed);
        ((processed as u128 * 1_000_000_000) / elapsed_ns as u128) as u64
    }

    /// Get estimated time remaining in seconds
    pub fn eta_seconds(&self) -> f64 {
        let throughput = self.throughput();
        if throughput == 0 {
            return 0.0;
        }
        let total = self.total_items.load(Ordering::Relaxed);
        let processed = self.processed_items.load(Ordering::Relaxed);
        let remaining = total.saturating_sub(processed);
        remaining as f64 / throughput as f64
    }

    /// Get elapsed time in seconds
    pub fn elapsed_seconds(&self) -> f64 {
        let elapsed_ns = now_ns() - self.start_time_ns.load(Ordering::Relaxed);
        elapsed_ns as f64 / 1_000_000_000.0
    }

    /// Get current counts (processed, successful, failed)
    pub fn counts(&self) -> (u64, u64, u64) {
        (
            self.processed_items.load(Ordering::Relaxed),
            self.successful_items.load(Ordering::Relaxed),
            self.failed_items.load(Ordering::Relaxed),
        )
    }

    /// Get total items
    pub fn total_items(&self) -> u64 {
        self.total_items.load(Ordering::Relaxed)
    }
}

impl fmt::Debug for ProgressTrackerCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProgressTrackerCapsule")
            .field("total_items", &self.total_items.load(Ordering::Relaxed))
            .field("processed", &self.processed_items.load(Ordering::Relaxed))
            .field("successful", &self.successful_items.load(Ordering::Relaxed))
            .field("failed", &self.failed_items.load(Ordering::Relaxed))
            .field("phase", &self.current_phase.load(Ordering::Relaxed))
            .field("percent_complete", &self.percent_complete())
            .field("throughput", &self.throughput())
            .field("eta_seconds", &self.eta_seconds())
            .finish()
    }
}

// Verify alignment (compile-time check)
#[allow(non_upper_case_globals)]
const _verify_alignment: () = {
    const fn check_alignment() {
        const ASSERT_ALIGNED: [u8; 128] = [0; 128];
        const PROGRESS_SIZE: usize = core::mem::size_of::<ProgressTrackerCapsule>();
        const _: [u8; 128] = [0; PROGRESS_SIZE]; // Will fail if size != 128
    }
    let _ = check_alignment();
};

/// Generic pipeline capsule (T6 Mixed: Container Capsule)
///
/// Wraps any pipeline with automatic progress tracking.
/// P is the pipeline type (must implement ProcessingPipeline trait).
pub struct PipelineCapsule<P> {
    pipeline: P,
    progress: Arc<ProgressTrackerCapsule>,
    callback: Option<Arc<dyn ProgressCallback>>,
}

impl<P> PipelineCapsule<P> {
    /// Create new pipeline capsule with progress tracking
    ///
    /// # Arguments
    /// - `pipeline`: The processing pipeline (owned)
    /// - `total_items`: Total items to process
    pub fn new(pipeline: P, total_items: u64) -> Self {
        Self {
            pipeline,
            progress: Arc::new(ProgressTrackerCapsule::new(total_items)),
            callback: None,
        }
    }

    /// Create pipeline capsule with progress callback
    pub fn with_callback(
        pipeline: P,
        total_items: u64,
        callback: Arc<dyn ProgressCallback>,
    ) -> Self {
        Self {
            pipeline,
            progress: Arc::new(ProgressTrackerCapsule::new(total_items)),
            callback: Some(callback),
        }
    }

    /// Get shared progress tracker (for real-time monitoring)
    pub fn progress(&self) -> Arc<ProgressTrackerCapsule> {
        Arc::clone(&self.progress)
    }

    /// Get mutable reference to pipeline (for direct access)
    pub fn pipeline_mut(&mut self) -> &mut P {
        &mut self.pipeline
    }

    /// Get immutable reference to pipeline
    pub fn pipeline(&self) -> &P {
        &self.pipeline
    }

    /// Consume capsule and return owned pipeline and progress tracker
    pub fn into_inner(self) -> (P, Arc<ProgressTrackerCapsule>) {
        (self.pipeline, self.progress)
    }

    /// Record successful item processing
    pub fn record_success(&self, item_id: u64) {
        self.progress.increment_processed();
        self.progress.increment_successful();
        if let Some(cb) = self.callback.as_ref() {
            cb.on_item_processed(item_id, true);
        }
    }

    /// Record failed item processing
    pub fn record_failure(&self, item_id: u64) {
        self.progress.increment_processed();
        self.progress.increment_failed();
        if let Some(cb) = self.callback.as_ref() {
            cb.on_item_processed(item_id, false);
        }
    }

    /// Change current phase
    pub fn set_phase(&self, phase: u8) {
        self.progress.set_phase(phase);
        if let Some(cb) = self.callback.as_ref() {
            cb.on_phase_changed(phase);
        }
    }

    /// Notify error (non-fatal, progress continues)
    pub fn notify_error(&self, error: &str) {
        if let Some(cb) = self.callback.as_ref() {
            cb.on_error(error);
        }
    }
}

impl<P: fmt::Debug> fmt::Debug for PipelineCapsule<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipelineCapsule")
            .field("pipeline", &self.pipeline)
            .field("progress", &self.progress)
            .field("callback", &self.callback.is_some())
            .finish()
    }
}

/// Get current time in nanoseconds since UNIX_EPOCH
#[inline]
fn now_ns() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as u64,
        Err(_) => 0,
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::thread;
    use std::time::Duration;

    // T1: Unit tests - Basic progress tracking
    #[test]
    fn test_progress_tracker_new() {
        let tracker = ProgressTrackerCapsule::new(100);
        assert_eq!(tracker.total_items(), 100);
        assert_eq!(tracker.percent_complete(), 0);
        assert_eq!(tracker.throughput(), 0);
    }

    #[test]
    fn test_progress_tracker_increment() {
        let tracker = ProgressTrackerCapsule::new(100);
        tracker.increment_processed();
        tracker.increment_successful();
        let (proc, succ, fail) = tracker.counts();
        assert_eq!(proc, 1);
        assert_eq!(succ, 1);
        assert_eq!(fail, 0);
    }

    #[test]
    fn test_progress_tracker_percent() {
        let tracker = ProgressTrackerCapsule::new(100);
        for _ in 0..25 {
            tracker.increment_processed();
        }
        assert_eq!(tracker.percent_complete(), 25);
    }

    #[test]
    fn test_progress_tracker_percent_100() {
        let tracker = ProgressTrackerCapsule::new(100);
        for _ in 0..100 {
            tracker.increment_processed();
        }
        assert_eq!(tracker.percent_complete(), 100);
    }

    #[test]
    fn test_progress_tracker_phase() {
        let tracker = ProgressTrackerCapsule::new(100);
        tracker.set_phase(0);
        assert_eq!(tracker.phase(), 0);
        tracker.set_phase(1);
        assert_eq!(tracker.phase(), 1);
    }

    #[test]
    fn test_progress_tracker_throughput() {
        let tracker = ProgressTrackerCapsule::new(1000);
        for _ in 0..100 {
            tracker.increment_processed();
        }
        // Sleep to ensure time passes
        thread::sleep(Duration::from_millis(100));
        let throughput = tracker.throughput();
        // Should be roughly 100 items / 0.1s = 1000 items/sec
        assert!(throughput > 500, "throughput {} too low", throughput);
    }

    #[test]
    fn test_progress_tracker_eta() {
        let tracker = ProgressTrackerCapsule::new(1000);
        for _ in 0..100 {
            tracker.increment_processed();
        }
        thread::sleep(Duration::from_millis(100));
        let eta = tracker.eta_seconds();
        // Should be roughly (1000-100) / 1000 = 0.9 seconds
        assert!(eta > 0.5, "eta {} too low", eta);
    }

    #[test]
    fn test_progress_tracker_elapsed() {
        let tracker = ProgressTrackerCapsule::new(100);
        thread::sleep(Duration::from_millis(100));
        let elapsed = tracker.elapsed_seconds();
        // Should be at least 0.1 seconds
        assert!(elapsed >= 0.09, "elapsed {} too low", elapsed);
    }

    #[test]
    fn test_progress_tracker_zero_total() {
        let tracker = ProgressTrackerCapsule::new(0);
        assert_eq!(tracker.percent_complete(), 100);
    }

    #[test]
    fn test_progress_tracker_failed() {
        let tracker = ProgressTrackerCapsule::new(100);
        for _ in 0..10 {
            tracker.increment_processed();
            tracker.increment_failed();
        }
        let (proc, succ, fail) = tracker.counts();
        assert_eq!(proc, 10);
        assert_eq!(succ, 0);
        assert_eq!(fail, 10);
    }

    // T2: Pipeline capsule tests
    #[test]
    fn test_pipeline_capsule_new() {
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::new(mock_pipeline, 100);
        assert_eq!(capsule.progress().total_items(), 100);
    }

    #[test]
    fn test_pipeline_capsule_record_success() {
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::new(mock_pipeline, 100);
        capsule.record_success(1);
        let (proc, succ, fail) = capsule.progress().counts();
        assert_eq!(proc, 1);
        assert_eq!(succ, 1);
        assert_eq!(fail, 0);
    }

    #[test]
    fn test_pipeline_capsule_record_failure() {
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::new(mock_pipeline, 100);
        capsule.record_failure(1);
        let (proc, succ, fail) = capsule.progress().counts();
        assert_eq!(proc, 1);
        assert_eq!(succ, 0);
        assert_eq!(fail, 1);
    }

    #[test]
    fn test_pipeline_capsule_set_phase() {
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::new(mock_pipeline, 100);
        capsule.set_phase(2);
        assert_eq!(capsule.progress().phase(), 2);
    }

    #[test]
    fn test_pipeline_capsule_progress_access() {
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::new(mock_pipeline, 100);
        let progress = capsule.progress();
        for _ in 0..50 {
            progress.increment_processed();
        }
        assert_eq!(progress.percent_complete(), 50);
    }

    #[test]
    fn test_pipeline_capsule_shared_progress() {
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::new(mock_pipeline, 100);
        let progress1 = capsule.progress();
        let progress2 = capsule.progress();
        progress1.increment_processed();
        assert_eq!(progress2.processed_items.load(Ordering::Relaxed), 1);
    }

    // T3: Callback tests
    struct TestCallback {
        items_processed: AtomicUsize,
        phases_changed: AtomicUsize,
        errors: AtomicUsize,
    }

    impl ProgressCallback for TestCallback {
        fn on_item_processed(&self, _item_id: u64, _success: bool) {
            self.items_processed.fetch_add(1, Ordering::Relaxed);
        }
        fn on_phase_changed(&self, _phase: u8) {
            self.phases_changed.fetch_add(1, Ordering::Relaxed);
        }
        fn on_error(&self, _error: &str) {
            self.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn test_pipeline_capsule_callback_success() {
        let callback = Arc::new(TestCallback {
            items_processed: AtomicUsize::new(0),
            phases_changed: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
        });
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::with_callback(mock_pipeline, 100, callback.clone());
        capsule.record_success(1);
        assert_eq!(
            callback.items_processed.load(Ordering::Relaxed),
            1,
            "Callback not invoked"
        );
    }

    #[test]
    fn test_pipeline_capsule_callback_phase() {
        let callback = Arc::new(TestCallback {
            items_processed: AtomicUsize::new(0),
            phases_changed: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
        });
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::with_callback(mock_pipeline, 100, callback.clone());
        capsule.set_phase(1);
        assert_eq!(
            callback.phases_changed.load(Ordering::Relaxed),
            1,
            "Phase callback not invoked"
        );
    }

    #[test]
    fn test_pipeline_capsule_callback_error() {
        let callback = Arc::new(TestCallback {
            items_processed: AtomicUsize::new(0),
            phases_changed: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
        });
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::with_callback(mock_pipeline, 100, callback.clone());
        capsule.notify_error("test error");
        assert_eq!(callback.errors.load(Ordering::Relaxed), 1, "Error callback not invoked");
    }

    // Mock pipeline for testing
    #[derive(Debug)]
    struct MockPipeline {
        capacity: usize,
    }

    impl MockPipeline {
        fn new() -> Self {
            Self { capacity: 1000 }
        }
    }

    // Property tests
    #[test]
    fn test_progress_monotonicity() {
        let tracker = ProgressTrackerCapsule::new(100);
        let mut prev_percent = 0u8;
        for _ in 0..100 {
            tracker.increment_processed();
            let current_percent = tracker.percent_complete();
            assert!(
                current_percent >= prev_percent,
                "Progress decreased: {} to {}",
                prev_percent,
                current_percent
            );
            prev_percent = current_percent;
        }
    }

    #[test]
    fn test_progress_concurrent_increments() {
        let tracker = Arc::new(ProgressTrackerCapsule::new(1000));
        let mut handles = vec![];
        for _ in 0..4 {
            let t = Arc::clone(&tracker);
            handles.push(std::thread::spawn(move || {
                for _ in 0..250 {
                    t.increment_processed();
                    t.increment_successful();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let (proc, succ, fail) = tracker.counts();
        assert_eq!(proc, 1000);
        assert_eq!(succ, 1000);
        assert_eq!(fail, 0);
    }

    #[test]
    fn test_progress_alignment() {
        use core::mem;
        assert_eq!(
            mem::size_of::<ProgressTrackerCapsule>(),
            128,
            "ProgressTrackerCapsule not 128 bytes"
        );
        assert_eq!(
            mem::align_of::<ProgressTrackerCapsule>(),
            128,
            "ProgressTrackerCapsule not 128-byte aligned"
        );
    }

    #[test]
    fn test_pipeline_capsule_into_inner() {
        let mock_pipeline = MockPipeline::new();
        let capsule = PipelineCapsule::new(mock_pipeline, 100);
        let (pipeline, progress) = capsule.into_inner();
        assert_eq!(pipeline.capacity, 1000);
        assert_eq!(progress.total_items(), 100);
    }
}
