//! Lockfree Submission Pipeline
//!
//! Complete submission pipeline integrating all Phase 1 and Phase 2 capsules:
//!
//! Pipeline Flow:
//! 1. Circuit Breaker Check (Phase 1) - Should allow commands?
//! 2. GPU State Check (Phase 1) - Is GPU ready?
//! 3. Context State Check (Phase 2) - Is context ready?
//! 4. GuC CTB Check (Phase 2) - Is firmware ready?
//! 5. Fence Dependencies (Phase 2) - Are dependencies satisfied?
//! 6. Submit to Queue - Atomic queue reservation
//!
//! Target: <500ns complete pipeline latency

use crate::capsules::GpuStateCapsule;
use crate::circuit_breaker::GpuCircuitBreaker;
use crate::context::ContextCapsule;
use crate::fence::FenceCapsule;
use crate::guc_ctb::GucReadyCapsule;
use std::sync::Arc;

/// Submission Pipeline Result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionResult {
    /// Command accepted and submitted
    Accepted {
        /// Sequence number assigned
        seqno: u32,
    },
    /// Command rejected - circuit breaker active
    RejectedBreaker,
    /// Command rejected - GPU not ready
    RejectedGpuState,
    /// Command rejected - context not ready
    RejectedContext,
    /// Command rejected - GuC CTB full
    RejectedGuCFull,
    /// Command rejected - fence dependencies unsatisfied
    RejectedFenceDeps,
}

/// Submission Pipeline Coordinator
///
/// Chains all capsules together in lockfree coordination pattern.
/// All state checks are single atomic reads - no locks.
///
/// # UCE32 Analysis (Internal)
///
/// Q28 (Simplicity): Pipeline is simple chain of atomic reads - YES
/// Q29 (Constraints): Hardware CAS latency ~15ns, target 8 stages * 50ns = 400ns < 500ns target
/// Q30 (Validation): Benchmark pipeline latency with real hardware
/// Q31 (Rust): Arc enables safe shared ownership, atomic reads are zero-cost
/// Q32 (Nightly): Could use portable_simd for parallel capsule checks
pub struct SubmissionPipeline {
    // Phase 1 capsules
    gpu_state: Arc<GpuStateCapsule>,
    breaker: Arc<GpuCircuitBreaker>,

    // Phase 2 capsules
    context: Arc<ContextCapsule>,
    guc_ctb: Arc<GucReadyCapsule>,
    fence: Arc<FenceCapsule>,

    // Pipeline metrics
    /// #ASSUME_METRIC_ATOMIC: All increments are atomic
    /// #VERIFY_COUNTER_ACCURACY: fetch_add guarantees no lost updates
    submissions_total: std::sync::atomic::AtomicU64,
    rejections_total: std::sync::atomic::AtomicU64,
}

impl SubmissionPipeline {
    /// Create new submission pipeline
    pub fn new(
        gpu_state: Arc<GpuStateCapsule>,
        breaker: Arc<GpuCircuitBreaker>,
        context: Arc<ContextCapsule>,
        guc_ctb: Arc<GucReadyCapsule>,
        fence: Arc<FenceCapsule>,
    ) -> Self {
        Self {
            gpu_state,
            breaker,
            context,
            guc_ctb,
            fence,
            submissions_total: std::sync::atomic::AtomicU64::new(0),
            rejections_total: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Submit command through complete pipeline
    ///
    /// Performs all pipeline stages with single atomic reads.
    /// Target latency: <500ns for complete pipeline.
    ///
    /// # ASSUM Safety
    ///
    /// #ASSUME_LOCKFREE_ONLY: All capsule reads are atomic, no locks
    /// #VERIFY_NO_BLOCKING: Pipeline uses only atomic operations
    ///
    /// #ASSUME_TOCTOU_SAFE: Each capsule check is atomic snapshot
    /// #VERIFY_TOCTOU_PREVENTED: Version checks in capsules prevent torn reads
    pub fn submit_command(
        &self,
        command_size: u32,
        seqno: u32,
        fence_value: u64,
    ) -> SubmissionResult {
        // Stage 1: Circuit Breaker Check
        // #ASSUME_MEMORY_ORDERING: Relaxed sufficient for breaker check
        // #VERIFY_ORDERING_SUFFICIENT: Breaker uses internal synchronization
        if !self.breaker.should_allow_command() {
            self.rejections_total
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return SubmissionResult::RejectedBreaker;
        }

        // Stage 2: GPU State Check
        // #ASSUME_MEMORY_ORDERING: Acquire ordering for state synchronization
        // #VERIFY_ORDERING_SUFFICIENT: State published with Release, read with Acquire
        let gpu_state = self.gpu_state.read();
        if !gpu_state.is_ready() {
            self.rejections_total
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return SubmissionResult::RejectedGpuState;
        }

        // Stage 3: Context State Check
        // Fast path using can_submit() for <5ns check
        if !self.context.can_submit() {
            self.rejections_total
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return SubmissionResult::RejectedContext;
        }

        // Stage 4: GuC CTB Readiness Check
        if !self.guc_ctb.has_space_for(command_size) {
            self.rejections_total
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return SubmissionResult::RejectedGuCFull;
        }

        // Stage 5: Fence Dependencies Check
        if !self.fence.is_signaled(fence_value) {
            self.rejections_total
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return SubmissionResult::RejectedFenceDeps;
        }

        // Stage 6: Accept and submit
        // #ASSUME_METRIC_ATOMIC: fetch_add is atomic
        // #VERIFY_COUNTER_ACCURACY: Hardware guarantees atomic increment
        self.submissions_total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        SubmissionResult::Accepted { seqno }
    }

    /// Fast path: Check if submission would succeed without full pipeline
    ///
    /// Performs minimal checks for early rejection.
    /// Target: <50ns for fast rejection path.
    ///
    /// #ASSUME_MEMORY_ORDERING: Relaxed sufficient for fast path
    /// #VERIFY_ORDERING_SUFFICIENT: Only breaker + context hot path, no synchronization needed
    #[inline(always)]
    pub fn can_submit_fast(&self) -> bool {
        // Only check breaker and context for ultra-fast path
        // Target: 2 atomic reads * ~5ns = ~10ns
        self.breaker.should_allow_command() && self.context.can_submit()
    }

    /// Get total submissions
    ///
    /// #ASSUME_MEMORY_ORDERING: Relaxed sufficient for metrics
    /// #VERIFY_ORDERING_SUFFICIENT: Metrics are approximate, don't need synchronization
    pub fn total_submissions(&self) -> u64 {
        self.submissions_total
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get total rejections
    pub fn total_rejections(&self) -> u64 {
        self.rejections_total
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get acceptance rate percentage
    pub fn acceptance_rate(&self) -> f64 {
        let total = self
            .submissions_total
            .load(std::sync::atomic::Ordering::Relaxed);
        let rejected = self
            .rejections_total
            .load(std::sync::atomic::Ordering::Relaxed);
        let all = total + rejected;

        if all == 0 {
            100.0
        } else {
            (total as f64 / all as f64) * 100.0
        }
    }

    /// Get Phase 2 capsule references for advanced coordination
    pub fn context_capsule(&self) -> &Arc<ContextCapsule> {
        &self.context
    }

    pub fn guc_capsule(&self) -> &Arc<GucReadyCapsule> {
        &self.guc_ctb
    }

    pub fn fence_capsule(&self) -> &Arc<FenceCapsule> {
        &self.fence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capsules::GpuState;
    use crate::circuit_breaker::QualityLevel;
    use crate::context::{ContextState, ContextUpdate};
    use crate::guc_ctb::GucCtbState;

    #[test]
    fn test_pipeline_creation() {
        let gpu_state = Arc::new(GpuStateCapsule::new());
        let breaker = Arc::new(GpuCircuitBreaker::new());
        let context = Arc::new(ContextCapsule::new());
        let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
        let fence = Arc::new(FenceCapsule::new(0));

        let pipeline = SubmissionPipeline::new(gpu_state, breaker, context, guc_ctb, fence);

        assert_eq!(pipeline.total_submissions(), 0);
        assert_eq!(pipeline.total_rejections(), 0);
    }

    #[test]
    fn test_pipeline_breaker_rejection() {
        let gpu_state = Arc::new(GpuStateCapsule::new());
        let breaker = Arc::new(GpuCircuitBreaker::new());
        let context = Arc::new(ContextCapsule::new());
        let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
        let fence = Arc::new(FenceCapsule::new(0));

        let pipeline = SubmissionPipeline::new(gpu_state, breaker.clone(), context, guc_ctb, fence);

        // Force breaker to L3 (paused)
        breaker.force_level(QualityLevel::L3);

        let result = pipeline.submit_command(1024, 1, 0);
        assert_eq!(result, SubmissionResult::RejectedBreaker);
        assert_eq!(pipeline.total_rejections(), 1);
    }

    #[test]
    fn test_pipeline_gpu_state_rejection() {
        let gpu_state = Arc::new(GpuStateCapsule::new());
        let breaker = Arc::new(GpuCircuitBreaker::new());
        let context = Arc::new(ContextCapsule::new());
        let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
        let fence = Arc::new(FenceCapsule::new(0));

        let pipeline = SubmissionPipeline::new(gpu_state.clone(), breaker, context, guc_ctb, fence);

        // Publish overheated GPU state
        let hot_state = GpuState {
            gpu_id: 0,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: 96, // Too hot!
            utilization: 50,
            valid: true,
        };
        gpu_state.publish(hot_state);

        let result = pipeline.submit_command(1024, 1, 0);
        assert_eq!(result, SubmissionResult::RejectedGpuState);
    }

    #[test]
    fn test_pipeline_context_rejection() {
        let gpu_state = Arc::new(GpuStateCapsule::new());
        let breaker = Arc::new(GpuCircuitBreaker::new());
        let context = Arc::new(ContextCapsule::new());
        let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
        let fence = Arc::new(FenceCapsule::new(0));

        let pipeline =
            SubmissionPipeline::new(gpu_state.clone(), breaker, context.clone(), guc_ctb, fence);

        // Publish valid GPU state
        let state = GpuState {
            gpu_id: 0,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: 65,
            utilization: 50,
            valid: true,
        };
        gpu_state.publish(state);

        // Publish ERROR context (not ready)
        let ctx_update = ContextUpdate {
            context_id: 1,
            priority: 0,
            state: ContextState::Error, // Error state!
            last_fence: 0,
            batch_count: 0,
            error_count: 1,
            timestamp_us: 0,
            resource_gen: 0,
            mem_usage_mb: 0,
            submission_count: 0,
        };
        context.publish(ctx_update);

        let result = pipeline.submit_command(1024, 1, 0);
        assert_eq!(result, SubmissionResult::RejectedContext);
    }

    #[test]
    fn test_pipeline_full_success() {
        let gpu_state = Arc::new(GpuStateCapsule::new());
        let breaker = Arc::new(GpuCircuitBreaker::new());
        let context = Arc::new(ContextCapsule::new());
        let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
        let fence = Arc::new(FenceCapsule::new(0));

        let pipeline = SubmissionPipeline::new(
            gpu_state.clone(),
            breaker,
            context.clone(),
            guc_ctb.clone(),
            fence.clone(),
        );

        // Set up all stages for success
        let state = GpuState {
            gpu_id: 0,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: 65,
            utilization: 50,
            valid: true,
        };
        gpu_state.publish(state);

        let ctx_update = ContextUpdate {
            context_id: 1,
            priority: 0,
            state: ContextState::Ready, // READY state
            last_fence: 0,
            batch_count: 0,
            error_count: 0,
            timestamp_us: 0,
            resource_gen: 0,
            mem_usage_mb: 0,
            submission_count: 0,
        };
        context.publish(ctx_update);

        let guc_state = GucCtbState {
            h2g_head: 0,
            h2g_tail: 1024,
            g2h_head: 0,
            g2h_tail: 0,
            capacity: 16 * 1024,
            pending_count: 0,
        };
        guc_ctb.publish(guc_state);

        // Signal fence (value 100)
        fence.signal(100, 1000);

        // Verify fence is actually signaled before submission
        assert!(
            fence.is_signaled(50),
            "Fence should be signaled for value 50"
        );

        // Submit should succeed - checking for fence value 50 which is < 100 (signaled)
        let result = pipeline.submit_command(512, 101, 50);
        match result {
            SubmissionResult::Accepted { seqno } => {
                assert_eq!(seqno, 101);
            }
            _ => panic!("Expected acceptance, got {:?}", result),
        }

        assert_eq!(pipeline.total_submissions(), 1);
        assert_eq!(pipeline.total_rejections(), 0);
        assert_eq!(pipeline.acceptance_rate(), 100.0);
    }

    #[test]
    fn test_fast_path_check() {
        let gpu_state = Arc::new(GpuStateCapsule::new());
        let breaker = Arc::new(GpuCircuitBreaker::new());
        let context = Arc::new(ContextCapsule::new());
        let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
        let fence = Arc::new(FenceCapsule::new(0));

        let pipeline = SubmissionPipeline::new(gpu_state, breaker, context.clone(), guc_ctb, fence);

        // Initially should fail (context not ready)
        assert!(!pipeline.can_submit_fast());

        // Publish ready context
        let ctx_update = ContextUpdate {
            context_id: 1,
            priority: 0,
            state: ContextState::Ready,
            last_fence: 0,
            batch_count: 0,
            error_count: 0,
            timestamp_us: 0,
            resource_gen: 0,
            mem_usage_mb: 0,
            submission_count: 0,
        };
        context.publish(ctx_update);

        // Now fast path should succeed
        assert!(pipeline.can_submit_fast());
    }
}
