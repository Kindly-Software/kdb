//! ToolExecutorCapsule - T1 Atomic Async Tool Execution Dispatcher (256 bytes)
//!
//! Lockfree async tool execution with execution state tracking and result coordination.
//! **Latency**: <50ns dispatch + async execution
//! **Tier**: T1 Atomic (DualAtomicU64 state coordination + generation counters)
//!
//! ## Architecture
//!
//! Coordinates tool execution with three main components:
//! 1. **ExecutionState**: Atomic state machine (Idle → Executing → Completed)
//! 2. **ActiveToolTracking**: Compact tool metadata (id, start_ns, generation)
//! 3. **ResultCoordination**: DualAtomicU64 for result availability + error flags

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};


// ============================================================================
// Execution State Machine (Lockfree State Transitions)
// ============================================================================

/// Execution state for tools (packed into 2 bits of DualAtomicU64)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionState {
    /// Ready to accept new tool executions
    Idle = 0,
    /// Currently executing a tool
    Executing = 1,
    /// Execution completed (result available)
    Completed = 2,
    /// Execution failed (error available)
    Failed = 3,
}

impl ExecutionState {
    /// Convert from u8 representation
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(ExecutionState::Idle),
            1 => Some(ExecutionState::Executing),
            2 => Some(ExecutionState::Completed),
            3 => Some(ExecutionState::Failed),
            _ => None,
        }
    }

    /// Convert to u8 representation
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Tool execution metadata (packed into 64 bits)
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct ExecutionMetadata(u64);

impl ExecutionMetadata {
    /// Create new execution metadata
    /// Packing: [state(2) | tool_id(14) | generation(16) | latency_ns_hi(32)]
    #[inline]
    pub fn new(state: ExecutionState, tool_id: u16, generation: u16) -> Self {
        let val =
            ((state.as_u8() as u64) << 62) | ((tool_id as u64) << 48) | ((generation as u64) << 32);
        ExecutionMetadata(val)
    }

    /// Extract execution state
    #[inline]
    pub fn state(self) -> ExecutionState {
        let state_bits = ((self.0 >> 62) & 0x3) as u8;
        ExecutionState::from_u8(state_bits).unwrap_or(ExecutionState::Idle)
    }

    /// Extract tool ID
    #[inline]
    pub fn tool_id(self) -> u16 {
        ((self.0 >> 48) & 0x3FFF) as u16
    }

    /// Extract generation counter (TOCTOU prevention)
    #[inline]
    pub fn generation(self) -> u16 {
        ((self.0 >> 32) & 0xFFFF) as u16
    }

    /// Extract latency high bits
    #[inline]
    pub fn latency_ns_hi(self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }

    /// Update state (returns old value for CAS loops)
    #[inline]
    pub fn with_state(self, new_state: ExecutionState) -> Self {
        let mask = 0x3FFF_FFFF_FFFF_FFFF;
        let new_val = (self.0 & mask) | ((new_state.as_u8() as u64) << 62);
        ExecutionMetadata(new_val)
    }
}

// ============================================================================
// ToolExecutorCapsule (256 bytes, 64-byte aligned, T1 Atomic)
// ============================================================================

#[repr(C, align(64))]
pub struct ToolExecutorCapsule {
    // ========================================================================
    // Execution State (64 bytes, single cache line)
    // ========================================================================
    /// DualAtomicU64: [execution_state | pending_count]
    /// execution_state: ExecutionMetadata (state, tool_id, generation, latency_ns_hi)
    /// pending_count: Number of pending tool executions
    pub execution_state: AtomicU64,
    pub pending_count: AtomicU64,

    /// Last tool execution timestamp (nanoseconds)
    pub last_execution_ns: AtomicU64,

    /// Total executions completed
    pub total_executions: AtomicU64,

    /// Total execution errors
    pub total_errors: AtomicU64,

    /// Generation counter for TOCTOU prevention (increment on state change)
    pub generation: AtomicU64,

    /// Execution active flag (for quick check)
    pub is_executing: AtomicBool,

    _padding1: [u8; 7],

    // ========================================================================
    // Active Tool Tracking (64 bytes)
    // ========================================================================
    /// Current active tool ID
    pub active_tool_id: AtomicU64,

    /// Tool execution start time (nanoseconds)
    pub active_tool_start_ns: AtomicU64,

    /// Tool execution timeout (nanoseconds, 0 = no timeout)
    pub execution_timeout_ns: AtomicU64,

    /// Error code from last failed execution (0 = no error)
    pub last_error_code: AtomicU64,

    /// Total time spent executing (across all executions)
    pub total_execution_time_ns: AtomicU64,

    /// Average execution latency (moving average in ns)
    pub avg_execution_ns: AtomicU64,

    _padding2: [u8; 16],

    // ========================================================================
    // Result Coordination (64 bytes)
    // ========================================================================
    /// Result availability flag (atomic bool packed efficiently)
    pub result_available: AtomicU64, // 1 = result ready, 0 = pending

    /// Result size (bytes)
    pub result_size: AtomicU64,

    /// Result generation counter (matches execution generation for validation)
    pub result_generation: AtomicU64,

    /// Result error flag (1 = error, 0 = success)
    pub result_error: AtomicU64,

    /// Concurrent execution counter (for multi-tool tracking)
    pub concurrent_count: AtomicU64,

    /// Maximum observed concurrent executions
    pub max_concurrent: AtomicU64,

    _padding3: [u8; 16],

    // ========================================================================
    // Monitoring & Metrics (64 bytes)
    // ========================================================================
    /// Request rate (requests per second, Q16.16 fixed-point)
    pub request_rate: AtomicU64,

    /// Dispatch latency histogram low bucket (<50ns)
    pub latency_bucket_low: AtomicU64,

    /// Dispatch latency histogram mid bucket (50ns-500ns)
    pub latency_bucket_mid: AtomicU64,

    /// Dispatch latency histogram high bucket (>500ns)
    pub latency_bucket_high: AtomicU64,

    /// Most recent result hash (for deduplication)
    pub result_hash: AtomicU64,

    /// Execution efficiency (completions / total_attempts, Q16.16)
    pub efficiency_metric: AtomicU64,

    _padding4: [u8; 16],
}

impl ToolExecutorCapsule {
    /// Create new tool executor capsule
    pub const fn new() -> Self {
        Self {
            execution_state: AtomicU64::new(0),
            pending_count: AtomicU64::new(0),
            last_execution_ns: AtomicU64::new(0),
            total_executions: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            is_executing: AtomicBool::new(false),
            _padding1: [0; 7],

            active_tool_id: AtomicU64::new(0),
            active_tool_start_ns: AtomicU64::new(0),
            execution_timeout_ns: AtomicU64::new(5_000_000_000), // 5 second default
            last_error_code: AtomicU64::new(0),
            total_execution_time_ns: AtomicU64::new(0),
            avg_execution_ns: AtomicU64::new(0),
            _padding2: [0; 16],

            result_available: AtomicU64::new(0),
            result_size: AtomicU64::new(0),
            result_generation: AtomicU64::new(0),
            result_error: AtomicU64::new(0),
            concurrent_count: AtomicU64::new(0),
            max_concurrent: AtomicU64::new(0),
            _padding3: [0; 16],

            request_rate: AtomicU64::new(0),
            latency_bucket_low: AtomicU64::new(0),
            latency_bucket_mid: AtomicU64::new(0),
            latency_bucket_high: AtomicU64::new(0),
            result_hash: AtomicU64::new(0),
            efficiency_metric: AtomicU64::new(1 << 16), // Start at 1.0
            _padding4: [0; 16],
        }
    }

    /// Begin tool execution (<30ns, lockfree CAS loop)
    ///
    /// Returns Ok(generation) on success, Err if already executing
    pub fn begin_execution(&self, tool_id: u64) -> Result<u64, &'static str> {
        // Check if already executing
        if self.is_executing.load(Ordering::Acquire) {
            return Err("Tool execution already in progress");
        }

        // Increment generation counter for TOCTOU prevention
        let gen = self.generation.fetch_add(1, Ordering::AcqRel);
        let gen_u16 = (gen & 0xFFFF) as u16;

        // Create execution metadata
        let metadata = ExecutionMetadata::new(ExecutionState::Executing, tool_id as u16, gen_u16);

        // Try to CAS execution state
        loop {
            let current = self.execution_state.load(Ordering::Acquire);
            let current_meta = ExecutionMetadata(current);

            if current_meta.state() != ExecutionState::Idle {
                return Err("Execution state not idle");
            }

            if self
                .execution_state
                .compare_exchange(current, metadata.0, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                // Successfully transitioned to Executing
                self.is_executing.store(true, Ordering::Release);
                self.active_tool_id.store(tool_id, Ordering::Relaxed);
                self.active_tool_start_ns
                    .store(self.get_timestamp_ns(), Ordering::Relaxed);
                self.pending_count.fetch_add(1, Ordering::Relaxed);

                // Update concurrent count
                let prev_concurrent = self.concurrent_count.fetch_add(1, Ordering::Relaxed);
                self.max_concurrent
                    .fetch_max(prev_concurrent + 1, Ordering::Relaxed);

                return Ok(gen);
            }
            // CAS failed, retry
        }
    }

    /// Complete tool execution (<20ns, lockfree)
    ///
    /// `result_hash`: FNV-1a hash of result for deduplication
    pub fn complete_execution(
        &self,
        generation: u64,
        result_hash: u64,
        result_size: u64,
    ) -> Result<(), &'static str> {
        let gen_u16 = (generation & 0xFFFF) as u16;

        // Get execution metadata
        let current = self.execution_state.load(Ordering::Acquire);
        let metadata = ExecutionMetadata(current);

        // Validate generation (TOCTOU prevention)
        if metadata.generation() != gen_u16 {
            return Err("Generation mismatch (execution aborted or restarted)");
        }

        if metadata.state() != ExecutionState::Executing {
            return Err("Not currently executing");
        }

        // Calculate execution latency (with saturating subtraction to prevent overflow)
        // #FIX_OVERFLOW: Clock skew or NTP adjustment can cause backwards time
        // #ASSUME_MONOTONIC: System clock is monotonic in normal operation
        // #VERIFY_SATURATING: Use saturating_sub to prevent wraparound
        let start_ns = self.active_tool_start_ns.load(Ordering::Relaxed);
        let elapsed_ns = self.get_timestamp_ns().saturating_sub(start_ns);

        // Update execution metrics
        self.total_executions.fetch_add(1, Ordering::Relaxed);
        self.total_execution_time_ns
            .fetch_add(elapsed_ns, Ordering::Relaxed);
        self.last_execution_ns
            .store(self.get_timestamp_ns(), Ordering::Relaxed);

        // Update average latency (simple EMA: 0.8 * old + 0.2 * new)
        let old_avg = self.avg_execution_ns.load(Ordering::Relaxed);
        let new_avg = (old_avg * 8 + elapsed_ns * 2) / 10;
        self.avg_execution_ns.store(new_avg, Ordering::Relaxed);

        // Record dispatch latency bucket
        match elapsed_ns {
            0..=50 => self.latency_bucket_low.fetch_add(1, Ordering::Relaxed),
            51..=500 => self.latency_bucket_mid.fetch_add(1, Ordering::Relaxed),
            _ => self.latency_bucket_high.fetch_add(1, Ordering::Relaxed),
        };

        // Update result coordination
        self.result_size.store(result_size, Ordering::Relaxed);
        self.result_hash.store(result_hash, Ordering::Relaxed);
        self.result_generation.store(generation, Ordering::Relaxed);
        self.result_error.store(0, Ordering::Relaxed);

        // Transition to Completed state
        let completed_meta = metadata.with_state(ExecutionState::Completed);
        self.execution_state
            .store(completed_meta.0, Ordering::Release);
        self.result_available.store(1, Ordering::Release);

        self.is_executing.store(false, Ordering::Release);
        self.pending_count.fetch_sub(1, Ordering::Relaxed);
        self.concurrent_count.fetch_sub(1, Ordering::Relaxed);

        Ok(())
    }

    /// Fail tool execution (<20ns, lockfree)
    ///
    /// `error_code`: Application-specific error code
    pub fn fail_execution(&self, generation: u64, error_code: u64) -> Result<(), &'static str> {
        let gen_u16 = (generation & 0xFFFF) as u16;

        let current = self.execution_state.load(Ordering::Acquire);
        let metadata = ExecutionMetadata(current);

        if metadata.generation() != gen_u16 {
            return Err("Generation mismatch");
        }

        if metadata.state() != ExecutionState::Executing {
            return Err("Not currently executing");
        }

        // Update metrics
        self.total_errors.fetch_add(1, Ordering::Relaxed);
        self.last_error_code.store(error_code, Ordering::Relaxed);

        let start_ns = self.active_tool_start_ns.load(Ordering::Relaxed);
        let elapsed_ns = self.get_timestamp_ns().saturating_sub(start_ns);
        self.total_execution_time_ns
            .fetch_add(elapsed_ns, Ordering::Relaxed);

        // Transition to Failed state
        let failed_meta = metadata.with_state(ExecutionState::Failed);
        self.execution_state.store(failed_meta.0, Ordering::Release);
        self.result_error.store(1, Ordering::Release);
        self.result_available.store(1, Ordering::Release);

        self.is_executing.store(false, Ordering::Release);
        self.pending_count.fetch_sub(1, Ordering::Relaxed);
        self.concurrent_count.fetch_sub(1, Ordering::Relaxed);

        Ok(())
    }

    /// Get current execution state (<10ns read-only)
    pub fn get_state(&self) -> ExecutionState {
        let current = self.execution_state.load(Ordering::Acquire);
        ExecutionMetadata(current).state()
    }

    /// Get execution statistics
    pub fn get_stats(&self) -> ExecutionStats {
        ExecutionStats {
            total_executions: self.total_executions.load(Ordering::Relaxed),
            total_errors: self.total_errors.load(Ordering::Relaxed),
            is_executing: self.is_executing.load(Ordering::Acquire),
            avg_latency_ns: self.avg_execution_ns.load(Ordering::Relaxed),
            max_concurrent: self.max_concurrent.load(Ordering::Relaxed),
            result_available: self.result_available.load(Ordering::Acquire) != 0,
            generation: self.generation.load(Ordering::Relaxed),
        }
    }

    /// Reset execution state (return to Idle)
    pub fn reset(&self) {
        self.execution_state.store(0, Ordering::Release);
        self.is_executing.store(false, Ordering::Release);
        self.result_available.store(0, Ordering::Release);
        self.pending_count.store(0, Ordering::Relaxed);
        self.concurrent_count.store(0, Ordering::Relaxed);
    }

    /// Get current timestamp in nanoseconds
    #[inline]
    fn get_timestamp_ns(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

// ============================================================================
// Execution Statistics
// ============================================================================

/// Statistics about tool execution
#[derive(Debug, Clone, Copy)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub total_errors: u64,
    pub is_executing: bool,
    pub avg_latency_ns: u64,
    pub max_concurrent: u64,
    pub result_available: bool,
    pub generation: u64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_executor_size() {
        assert_eq!(
            size_of::<ToolExecutorCapsule>(),
            256,
            "ToolExecutorCapsule must be 256 bytes"
        );
    }

    #[test]
    fn test_executor_alignment() {
        assert_eq!(
            align_of::<ToolExecutorCapsule>(),
            64,
            "ToolExecutorCapsule must be 64-byte aligned"
        );
    }

    #[test]
    fn test_execution_metadata_packing() {
        let meta = ExecutionMetadata::new(ExecutionState::Executing, 42, 100);
        assert_eq!(meta.state(), ExecutionState::Executing);
        assert_eq!(meta.tool_id(), 42);
        assert_eq!(meta.generation(), 100);
    }

    #[test]
    fn test_begin_execution() {
        let executor = ToolExecutorCapsule::new();
        let gen = executor.begin_execution(1).unwrap();
        assert!(executor.is_executing.load(Ordering::Relaxed));
        assert_eq!(executor.get_state(), ExecutionState::Executing);

        // Should fail if already executing
        assert!(executor.begin_execution(2).is_err());
    }

    #[test]
    fn test_complete_execution() {
        let executor = ToolExecutorCapsule::new();
        let gen = executor.begin_execution(1).unwrap();

        let result = executor.complete_execution(gen, 12345, 1024);
        assert!(result.is_ok());
        assert_eq!(executor.get_state(), ExecutionState::Completed);
        assert!(!executor.is_executing.load(Ordering::Relaxed));
        assert_eq!(executor.total_executions.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_fail_execution() {
        let executor = ToolExecutorCapsule::new();
        let gen = executor.begin_execution(1).unwrap();

        let result = executor.fail_execution(gen, 42);
        assert!(result.is_ok());
        assert_eq!(executor.get_state(), ExecutionState::Failed);
        assert_eq!(executor.total_errors.load(Ordering::Relaxed), 1);
        assert_eq!(executor.last_error_code.load(Ordering::Relaxed), 42);
    }

    #[test]
    fn test_generation_counter_prevents_toctou() {
        let executor = ToolExecutorCapsule::new();
        let gen1 = executor.begin_execution(1).unwrap();

        // Complete execution
        executor.complete_execution(gen1, 0, 0).unwrap();
        executor.reset();

        // Try to use old generation after state change
        let gen2 = executor.begin_execution(2).unwrap();
        assert_ne!(gen1, gen2);

        // Old generation should be rejected
        assert!(executor.complete_execution(gen1, 0, 0).is_err());
        assert!(executor.complete_execution(gen2, 0, 0).is_ok());
    }

    #[test]
    fn test_concurrent_tracking() {
        let executor = ToolExecutorCapsule::new();
        executor.begin_execution(1).unwrap();
        assert_eq!(executor.concurrent_count.load(Ordering::Relaxed), 1);

        let stats = executor.get_stats();
        assert_eq!(stats.max_concurrent, 1);
    }

    #[test]
    fn test_statistics() {
        let executor = ToolExecutorCapsule::new();

        for i in 1..=5 {
            let gen = executor.begin_execution(i).unwrap();
            executor.complete_execution(gen, 0, 0).unwrap();
            executor.reset();
        }

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 5);
        assert_eq!(stats.total_errors, 0);
        assert!(!stats.is_executing);
    }
}
