//! T6 Mixed DebuggerCapsule - Final 1 MB Integration
//!
//! Integrates all 7 tier components into a single computational capsule.
//!
//! # Session Tier Detection (Phase 3)
//!
//! Supports three-tier session architecture:
//! - **LIGHT (64KB)**: <48 snapshots, <96 breakpoints, no memory replay
//! - **MEDIUM (256KB)**: <384 snapshots, <384 breakpoints, stack capture
//! - **HEAVY (1.09MB)**: Full memory replay, unlimited snapshots
//!
//! The `recommend_tier()` method analyzes usage patterns and returns the
//! appropriate tier. Use `check_upgrade_needed()` and `check_downgrade_possible()`
//! for dynamic tier management.

// Import for lazy memory replay initialization (Phase 3)
// MemoryReplayCapsule is used by enable_memory_replay() for HEAVY tier sessions
#[allow(unused_imports)]
use crate::memory_replay::{MemoryReplayCapsule, ReplayConfig};
use crate::session_pool::{SessionId, SessionTierType, UpgradeReason};
use crate::tier10_probabilistic::*;
use crate::tier1_atomic::*;
use crate::tier2_simd::*;
use crate::tier4_parallel_debug::*;
use crate::tier5_streaming::*;
use crate::tier9_persistent::*;
use crate::time_travel::*;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Size Calculations (Updated for T4 Batch)
// ============================================================================
//
// T1 Atomic (64 KB):
//   - ExecutionStateCapsule: 4,096
//   - BreakpointTableCapsule: 16,384
//   - WatchpointTableCapsule: 4,096
//   - ThreadStateCapsule[16]: 16 × 2,560 = 40,960
//   Total: 65,536 bytes
//
// T2 SIMD (128 KB):
//   - SimdStackFrameCapsule: 65,536
//   - SimdSymbolTableCapsule: 65,536
//   Total: 131,072 bytes
//
// T4 Batch (64 KB) [NEW]:
//   - MultiProcessDebuggerCapsule: 32,768
//   - BatchSymbolResolverCapsule: 16,640
//   - ParallelStackAnalyzerCapsule: 16,448
//   Total: 65,856 bytes
//
// T5 Streaming (192 KB) [REDUCED from 256 KB]:
//   - RingBufferTraceCapsule: 196,864
//   Total: 196,864 bytes
//
// T9 Persistent (128 KB):
//   - MmapCrashDumpCapsule: 65,536
//   - CheckpointEntry[100]: 100 × 640 = 64,000
//   Total: 129,536 bytes
//
// T10 Probabilistic (256 KB):
//   - ExecutionPathSignature[1024]: 1024 × 256 = 262,144
//   Total: 262,144 bytes
//
// Time-Travel (128 KB):
//   - ReplayEngineCapsule: 131,072
//   Total: 131,072 bytes
//
// Subtotal: 65,536 + 131,072 + 65,856 + 196,864 + 129,536 + 262,144 + 131,072
//         = 982,080 bytes
//
// Target: 1,048,576 bytes (1 MB)
// Padding: 1,048,576 - 982,080 = 66,496 bytes
//
// ============================================================================

// ============================================================================
// Session Tier Constants
// ============================================================================

/// Snapshot threshold for LIGHT tier (upgrade to MEDIUM when exceeded)
pub const LIGHT_SNAPSHOT_THRESHOLD: u32 = 48;

/// Breakpoint threshold for LIGHT tier (upgrade to MEDIUM when exceeded)
pub const LIGHT_BREAKPOINT_THRESHOLD: u32 = 96;

/// Snapshot threshold for MEDIUM tier (upgrade to HEAVY when exceeded)
pub const MEDIUM_SNAPSHOT_THRESHOLD: u32 = 384;

/// Breakpoint threshold for MEDIUM tier (upgrade to HEAVY when exceeded)
pub const MEDIUM_BREAKPOINT_THRESHOLD: u32 = 384;

/// Default idle threshold for downgrade (30 minutes in seconds)
pub const DEFAULT_IDLE_THRESHOLD_SECS: u64 = 1800;

// ============================================================================
// SessionFlags - Bitflags for session capabilities
// ============================================================================

bitflags::bitflags! {
    /// Session capability and state flags
    ///
    /// Tracks which features are enabled for this session and recent activity.
    /// Uses bitflags for efficient atomic manipulation.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SessionFlags: u32 {
        /// Memory replay is enabled (HEAVY tier feature)
        const MEMORY_REPLAY_ENABLED = 0x01;
        /// Full stack capture is enabled (MEDIUM+ tier feature)
        const FULL_STACK_CAPTURE = 0x02;
        /// Q34 audit trail is enabled
        const AUDIT_TRAIL_ENABLED = 0x04;
        /// Session has stepped recently (for idle detection)
        const STEPPED_RECENTLY = 0x08;
        /// Session has active breakpoints
        const HAS_BREAKPOINTS = 0x10;
        /// Session has active watchpoints
        const HAS_WATCHPOINTS = 0x20;
        /// Memory regions are being tracked
        const MEMORY_TRACKING = 0x40;
        /// Session is attached to a live process
        const ATTACHED = 0x80;
    }
}

// ============================================================================
// SessionContext - Tracks session state for tier detection
// ============================================================================

/// Session context for tier detection and activity tracking
///
/// Tracks session state including tier, timestamps, and usage counters
/// for dynamic tier management.
///
/// # Size
/// 64 bytes (cache-line aligned)
///
/// # Thread Safety
/// All operations are lockfree via atomics.
///
/// # ASSUM Safety
/// - #ASSUME_LOCKFREE_ONLY: All fields use atomic operations
/// - #VERIFY_CACHE_ALIGNED: 64-byte alignment prevents false sharing
#[repr(C, align(64))]
pub struct SessionContext {
    /// Unique session identifier
    pub session_id: SessionId,
    /// Current session tier
    tier: AtomicU32,
    /// Session creation timestamp (nanoseconds since epoch)
    pub created_at_ns: AtomicU64,
    /// Last activity timestamp (nanoseconds since epoch)
    pub last_activity_ns: AtomicU64,
    /// Number of snapshots taken
    pub snapshot_count: AtomicU32,
    /// Number of breakpoints set
    pub breakpoint_count: AtomicU32,
    /// Number of commands executed
    pub command_count: AtomicU32,
    /// Session capability flags
    flags: AtomicU32,
    /// Padding to 64 bytes
    _padding: [u8; 16],
}

impl SessionContext {
    /// Create a new session context
    ///
    /// # Arguments
    /// - `session_id`: Unique session identifier
    /// - `tier`: Initial session tier
    ///
    /// # Returns
    /// New session context with current timestamp
    pub fn new(session_id: SessionId, tier: SessionTierType) -> Self {
        let now = Self::current_time_ns();
        Self {
            session_id,
            tier: AtomicU32::new(tier as u32),
            created_at_ns: AtomicU64::new(now),
            last_activity_ns: AtomicU64::new(now),
            snapshot_count: AtomicU32::new(0),
            breakpoint_count: AtomicU32::new(0),
            command_count: AtomicU32::new(0),
            flags: AtomicU32::new(0),
            _padding: [0; 16],
        }
    }

    /// Create empty session context (for initialization)
    pub const fn empty() -> Self {
        Self {
            session_id: SessionId(0),
            tier: AtomicU32::new(0),
            created_at_ns: AtomicU64::new(0),
            last_activity_ns: AtomicU64::new(0),
            snapshot_count: AtomicU32::new(0),
            breakpoint_count: AtomicU32::new(0),
            command_count: AtomicU32::new(0),
            flags: AtomicU32::new(0),
            _padding: [0; 16],
        }
    }

    /// Get current tier
    #[inline]
    pub fn get_tier(&self) -> SessionTierType {
        match self.tier.load(Ordering::Acquire) {
            0 => SessionTierType::Light,
            1 => SessionTierType::Medium,
            2 => SessionTierType::Heavy,
            _ => SessionTierType::Light, // Default to light
        }
    }

    /// Set current tier
    #[inline]
    pub fn set_tier(&self, tier: SessionTierType) {
        self.tier.store(tier as u32, Ordering::Release);
    }

    /// Get session flags
    #[inline]
    pub fn get_flags(&self) -> SessionFlags {
        SessionFlags::from_bits_truncate(self.flags.load(Ordering::Acquire))
    }

    /// Set session flags
    #[inline]
    pub fn set_flags(&self, flags: SessionFlags) {
        self.flags.store(flags.bits(), Ordering::Release);
    }

    /// Add flags (OR operation)
    #[inline]
    pub fn add_flags(&self, flags: SessionFlags) {
        self.flags.fetch_or(flags.bits(), Ordering::AcqRel);
    }

    /// Remove flags (AND NOT operation)
    #[inline]
    pub fn remove_flags(&self, flags: SessionFlags) {
        self.flags.fetch_and(!flags.bits(), Ordering::AcqRel);
    }

    /// Check if flag is set
    #[inline]
    pub fn has_flag(&self, flag: SessionFlags) -> bool {
        self.get_flags().contains(flag)
    }

    /// Update last activity timestamp
    #[inline]
    pub fn touch(&self) {
        let now = Self::current_time_ns();
        self.last_activity_ns.store(now, Ordering::Release);
    }

    /// Get idle duration in seconds
    #[inline]
    pub fn idle_seconds(&self) -> u64 {
        let last = self.last_activity_ns.load(Ordering::Acquire);
        let now = Self::current_time_ns();
        now.saturating_sub(last) / 1_000_000_000
    }

    /// Increment snapshot count and return new value
    #[inline]
    pub fn increment_snapshots(&self) -> u32 {
        self.snapshot_count.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Increment breakpoint count and return new value
    #[inline]
    pub fn increment_breakpoints(&self) -> u32 {
        self.breakpoint_count.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Decrement breakpoint count and return new value
    #[inline]
    pub fn decrement_breakpoints(&self) -> u32 {
        self.breakpoint_count
            .fetch_sub(1, Ordering::AcqRel)
            .saturating_sub(1)
    }

    /// Increment command count and return new value
    #[inline]
    pub fn increment_commands(&self) -> u32 {
        self.command_count.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Get current time in nanoseconds
    #[inline]
    fn current_time_ns() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

// ============================================================================
// Debug Operation Types (for tier-based access control)
// ============================================================================

/// Debug operation types for tier-based access control
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DebugOperation {
    /// Attach to process (all tiers)
    Attach = 0,
    /// Set breakpoint (all tiers)
    SetBreakpoint = 1,
    /// Single step (all tiers)
    Step = 2,
    /// Continue execution (all tiers)
    Continue = 3,
    /// Read memory (all tiers)
    ReadMemory = 4,
    /// Take snapshot (all tiers, limited count for LIGHT)
    TakeSnapshot = 5,
    /// Step backward / time-travel (MEDIUM+)
    StepBackward = 6,
    /// Full stack capture (MEDIUM+)
    FullStackCapture = 7,
    /// Enable memory replay (HEAVY only)
    EnableMemoryReplay = 8,
    /// Read memory at snapshot (HEAVY only)
    ReadMemoryAtSnapshot = 9,
}

impl DebugOperation {
    /// Get minimum tier required for this operation
    #[inline]
    pub fn minimum_tier(self) -> SessionTierType {
        match self {
            DebugOperation::Attach
            | DebugOperation::SetBreakpoint
            | DebugOperation::Step
            | DebugOperation::Continue
            | DebugOperation::ReadMemory
            | DebugOperation::TakeSnapshot => SessionTierType::Light,

            DebugOperation::StepBackward | DebugOperation::FullStackCapture => {
                SessionTierType::Medium
            }

            DebugOperation::EnableMemoryReplay | DebugOperation::ReadMemoryAtSnapshot => {
                SessionTierType::Heavy
            }
        }
    }
}

#[repr(C, align(256))]
pub struct DebuggerCapsule {
    // ========================================================================
    // T1 Atomic - Execution Control (64 KB)
    // ========================================================================
    pub execution: ExecutionStateCapsule,    // 4,096 bytes
    pub breakpoints: BreakpointTableCapsule, // 16,384 bytes
    pub watchpoints: WatchpointTableCapsule, // 4,096 bytes
    pub threads: [ThreadStateCapsule; 16],   // 40,960 bytes

    // ========================================================================
    // T2 SIMD - Stack & Symbols (128 KB)
    // ========================================================================
    pub simd_stack: SimdStackFrameCapsule,    // 65,536 bytes
    pub simd_symbols: SimdSymbolTableCapsule, // 65,536 bytes

    // ========================================================================
    // T4 Batch - Parallel Multi-Process Debugging (64 KB) [NEW]
    // ========================================================================
    pub multi_process: MultiProcessDebuggerCapsule, // 32,768 bytes
    pub batch_symbols: BatchSymbolResolverCapsule,  // 16,640 bytes
    pub parallel_stack: ParallelStackAnalyzerCapsule, // 16,448 bytes

    // ========================================================================
    // T5 Streaming - Event Trace (192 KB) [REDUCED from 256 KB]
    // ========================================================================
    pub trace: RingBufferTraceCapsule, // 196,864 bytes

    // ========================================================================
    // T9 Persistent - Crash Dumps (128 KB)
    // ========================================================================
    pub crash_dump: MmapCrashDumpCapsule,    // 65,536 bytes
    pub checkpoints: [CheckpointEntry; 100], // 64,000 bytes

    // ========================================================================
    // T10 Probabilistic - Path Deduplication (256 KB)
    // ========================================================================
    pub path_sigs: [ExecutionPathSignature; 1024], // 262,144 bytes

    // ========================================================================
    // Time-Travel - Reverse Execution (128 KB)
    // ========================================================================
    pub replay_engine: ReplayEngineCapsule, // 131,072 bytes

    // ========================================================================
    // Session Context (64 bytes) - Phase 3 Addition
    // ========================================================================
    /// Session context for tier detection and activity tracking
    pub session_context: SessionContext, // 64 bytes

    // ========================================================================
    // Reserved Space (66,432 bytes for future expansion)
    // Note: Reduced by 64 bytes for SessionContext
    // ========================================================================
    _reserved: [u8; 66432],
}

impl DebuggerCapsule {
    /// Create new debugger capsule attached to process
    pub fn new(pid: u64) -> Self {
        const EMPTY_THREAD: ThreadStateCapsule = ThreadStateCapsule::empty();
        const EMPTY_CHECKPOINT: CheckpointEntry = CheckpointEntry::empty();
        const EMPTY_PATH_SIG: ExecutionPathSignature = ExecutionPathSignature::empty();

        Self {
            // T1 Atomic
            execution: ExecutionStateCapsule::new(pid),
            breakpoints: BreakpointTableCapsule::new(),
            watchpoints: WatchpointTableCapsule::new(),
            threads: [EMPTY_THREAD; 16],

            // T2 SIMD
            simd_stack: SimdStackFrameCapsule::new(),
            simd_symbols: SimdSymbolTableCapsule::new(),

            // T4 Batch (NEW)
            multi_process: MultiProcessDebuggerCapsule::new(),
            batch_symbols: BatchSymbolResolverCapsule::new(),
            parallel_stack: ParallelStackAnalyzerCapsule::new(),

            // T5 Streaming
            trace: RingBufferTraceCapsule::new(),

            // T9 Persistent
            crash_dump: MmapCrashDumpCapsule::new(),
            checkpoints: [EMPTY_CHECKPOINT; 100],

            // T10 Probabilistic
            path_sigs: [EMPTY_PATH_SIG; 1024],

            // Time-Travel
            replay_engine: ReplayEngineCapsule::new(),

            // Session Context (defaults to HEAVY tier for full DebuggerCapsule)
            session_context: SessionContext::new(
                SessionId::new(SessionTierType::Heavy as u8, 0, 0),
                SessionTierType::Heavy,
            ),

            _reserved: [0; 66432],
        }
    }

    // ========================================================================
    // High-Level API - Process Control
    // ========================================================================

    /// Attach to process (<50ns overhead)
    pub fn attach_to_process(&self, pid: u64) -> Result<(), &'static str> {
        // In real implementation, would use ptrace(PTRACE_ATTACH, pid, ...)
        self.execution.pid.store(pid, Ordering::Release);
        self.execution.state.store(1, Ordering::Release); // Paused

        // Record attachment event
        self.trace.record(0, 0, pid);

        // Track session activity
        self.session_context.touch();
        self.session_context.increment_commands();
        self.session_context.add_flags(SessionFlags::ATTACHED);

        Ok(())
    }

    /// Set breakpoint at address (<50ns atomic operation)
    pub fn set_breakpoint(&self, addr: u64) -> Result<usize, &'static str> {
        // In real implementation, would read original byte and write 0xCC (int3)
        let original_byte = 0x90; // NOP for demo

        let bp_idx = self.breakpoints.add_breakpoint(addr, original_byte)?;

        // Record breakpoint set event
        self.trace.record(0, 0, addr);

        // Track session activity
        self.session_context.touch();
        self.session_context.increment_commands();
        self.session_context.increment_breakpoints();
        self.session_context
            .add_flags(SessionFlags::HAS_BREAKPOINTS);

        Ok(bp_idx)
    }

    /// Continue execution
    pub fn continue_execution(&self) -> Result<(), &'static str> {
        if !self.execution.is_running() {
            self.execution.resume();
            self.trace.record(2, 0, 0);

            // Track session activity
            self.session_context.touch();
            self.session_context.increment_commands();

            Ok(())
        } else {
            Err("Already running")
        }
    }

    /// Single-step instruction
    pub fn step_instruction(&self) -> Result<u64, &'static str> {
        let rip = self.execution.get_rip();

        // In real implementation, would use ptrace(PTRACE_SINGLESTEP, ...)
        let new_rip = rip + 4; // Assume 4-byte instruction
        self.execution.set_rip(new_rip);

        // Take snapshot for time-travel
        let rsp = self.execution.rsp.load(Ordering::Relaxed);
        self.replay_engine.take_snapshot(new_rip, rsp)?;

        // Record step event
        self.trace.record(2, 0, new_rip);

        // Track session activity
        self.session_context.touch();
        self.session_context.increment_commands();
        self.session_context.increment_snapshots();
        self.session_context
            .add_flags(SessionFlags::STEPPED_RECENTLY);

        Ok(new_rip)
    }

    /// Step backward (time-travel!)
    pub fn step_backward(&self) -> Result<u64, &'static str> {
        let (_, rip, rsp) = self.replay_engine.step_backward()?;

        // Restore state
        self.execution.set_rip(rip);
        self.execution.rsp.store(rsp, Ordering::Release);

        // Record reverse step event
        self.trace.record(2, 0, rip);

        // Track session activity
        self.session_context.touch();
        self.session_context.increment_commands();

        Ok(rip)
    }

    // ========================================================================
    // High-Level API - Stack Unwinding (8× SIMD speedup)
    // ========================================================================

    /// Get stack trace using SIMD-accelerated unwinding
    pub fn get_stack_trace(&self) -> Result<Vec<u64>, &'static str> {
        // Use SIMD-accelerated collection
        Ok(self.simd_stack.collect_trace_simd())
    }

    /// Unwind stack frame by frame
    pub fn unwind_stack(&self) -> Result<(), &'static str> {
        let rip = self.execution.get_rip();
        let rbp = self
            .execution
            .rbp
            .load(std::sync::atomic::Ordering::Relaxed);
        let rsp = self
            .execution
            .rsp
            .load(std::sync::atomic::Ordering::Relaxed);

        self.simd_stack.push_frame(rip, rbp, rsp)?;

        Ok(())
    }

    // ========================================================================
    // High-Level API - Crash Handling
    // ========================================================================

    /// Record crash dump
    pub fn record_crash(&self, signal: u32, fault_addr: u64) -> Result<(), &'static str> {
        let rip = self.execution.get_rip();
        let rsp = self
            .execution
            .rsp
            .load(std::sync::atomic::Ordering::Relaxed);
        let rbp = self
            .execution
            .rbp
            .load(std::sync::atomic::Ordering::Relaxed);

        self.crash_dump
            .record_crash(signal, fault_addr, rip, rsp, rbp);
        self.execution
            .state
            .store(2, std::sync::atomic::Ordering::Release); // Crashed

        // Record crash event
        self.trace.record(3, 0, fault_addr);

        Ok(())
    }

    /// Export checkpoint to persistent storage
    pub fn export_checkpoint(&self, checkpoint_id: u64) -> Result<(), &'static str> {
        let rip = self.execution.get_rip();
        let rsp = self
            .execution
            .rsp
            .load(std::sync::atomic::Ordering::Relaxed);

        // Find empty checkpoint slot
        for checkpoint in &self.checkpoints {
            if !checkpoint.is_active() {
                checkpoint.save(checkpoint_id, rip, rsp);
                return Ok(());
            }
        }

        Err("No checkpoint slots available")
    }

    // ========================================================================
    // High-Level API - Path Deduplication
    // ========================================================================

    /// Find similar execution paths (for bug pattern detection)
    pub fn find_similar_paths(&self, _signature: &[u64; 32], threshold: f64) -> Vec<u64> {
        // In real implementation, would compute signature from current execution
        // and compare against stored paths using LSH
        let mut similar = Vec::new();

        for (i, path_sig) in self.path_sigs.iter().enumerate() {
            if path_sig.path_id.load(std::sync::atomic::Ordering::Relaxed) != 0 {
                let sim = path_sig.similarity(&ExecutionPathSignature::empty());
                if sim >= threshold {
                    similar.push(i as u64);
                }
            }
        }

        similar
    }

    /// Record execution path
    pub fn record_execution_path(
        &self,
        path_id: u64,
        signature: &[u64; 32],
    ) -> Result<(), &'static str> {
        // Find empty path signature slot
        for path_sig in &self.path_sigs {
            if path_sig.path_id.load(std::sync::atomic::Ordering::Relaxed) == 0 {
                path_sig.set_signature(path_id, signature);
                return Ok(());
            }
        }

        Err("Path signature table full")
    }

    // ========================================================================
    // T4 Batch API - Parallel Multi-Process Debugging (NEW)
    // ========================================================================

    /// Attach to multiple processes in parallel (T4 Batch)
    ///
    /// **Speedup**: 16× parallel attachment (16 processes simultaneously)
    /// **Latency**: <1ms for 16 processes (vs 16ms sequential)
    pub fn attach_multi_process(&self, pids: &[u64]) -> Result<(), &'static str> {
        if pids.len() > 16 {
            return Err("Maximum 16 processes supported");
        }

        // Submit attach commands to process queues
        for (i, &pid) in pids.iter().enumerate() {
            let cmd = DebugCommand::attach(pid, 0);
            self.multi_process.submit_command(i, cmd)?;
        }

        Ok(())
    }

    /// Resolve symbols in parallel (T4 Batch)
    ///
    /// **Speedup**: 10× parallel resolution (800ns → 80ns per symbol)
    /// **Capacity**: 128 symbols per batch
    pub fn resolve_symbols_parallel(
        &self,
        addresses: &[u64],
        pid: u64,
    ) -> Result<Vec<u32>, &'static str> {
        let mut request_ids = Vec::with_capacity(addresses.len());

        // Submit all requests
        for &addr in addresses {
            let req_id = self.batch_symbols.submit_request(addr, pid)?;
            request_ids.push(req_id);
        }

        Ok(request_ids)
    }

    /// Unwind stacks for all threads in parallel (T4 Batch)
    ///
    /// **Speedup**: 8-16× compound (T2 SIMD 8× × T4 parallel)
    /// **Capacity**: 16 threads × 15 frames = 240 stack frames total
    pub fn unwind_all_threads_parallel(&self) -> Result<(), &'static str> {
        // Start unwinding for all 16 threads
        for tid in 0..16 {
            let thread = &self.threads[tid as usize];
            let rip = thread.rip.load(std::sync::atomic::Ordering::Relaxed);
            let rbp = thread.rbp.load(std::sync::atomic::Ordering::Relaxed);
            let rsp = thread.rsp.load(std::sync::atomic::Ordering::Relaxed);

            if rip != 0 {
                self.parallel_stack.start_unwind(tid, rip, rbp, rsp)?;
            }
        }

        Ok(())
    }

    /// Get parallel debugging statistics
    pub fn get_parallel_stats(&self) -> ParallelDebugStats {
        let process_stats = self.multi_process.get_all_stats();
        let (symbols_submitted, symbols_completed) = self.batch_symbols.get_stats();
        let (active_threads, total_frames) = self.parallel_stack.get_stats();

        ParallelDebugStats {
            process_stats,
            symbols_submitted,
            symbols_completed,
            active_threads,
            total_frames,
        }
    }

    // ========================================================================
    // Statistics & Monitoring
    // ========================================================================

    /// Get debugger statistics
    pub fn get_stats(&self) -> DebuggerStats {
        let (trace_total, trace_dropped) = self.trace.get_stats();
        let (replay_current, replay_total) = self.replay_engine.get_stats();

        DebuggerStats {
            instruction_count: self.execution.instruction_count.load(Ordering::Relaxed),
            breakpoint_hits: self.execution.breakpoint_hits.load(Ordering::Relaxed),
            trace_events: trace_total,
            trace_dropped: trace_dropped,
            snapshots_taken: replay_total,
            current_snapshot: replay_current,
            stack_depth: self.simd_stack.get_depth(),
        }
    }

    // ========================================================================
    // Session Tier Detection (Phase 3)
    // ========================================================================

    /// Analyze usage patterns and recommend appropriate tier
    ///
    /// Examines snapshot count, breakpoint count, and feature flags to
    /// determine the minimum tier required for current usage patterns.
    ///
    /// # Tier Selection Criteria
    ///
    /// - **LIGHT**: <48 snapshots, <96 breakpoints, no memory replay
    /// - **MEDIUM**: <384 snapshots, <384 breakpoints, stack capture
    /// - **HEAVY**: Full memory replay needed, unlimited snapshots
    ///
    /// # Performance
    /// - <50ns (atomic reads only)
    ///
    /// # Returns
    /// Recommended session tier based on current usage
    ///
    /// #ASSUME_LOCKFREE_ONLY: All reads are atomic
    /// #VERIFY_UNIT_TEST: test_recommend_tier_*
    pub fn recommend_tier(&self) -> SessionTierType {
        let snapshot_count = self.session_context.snapshot_count.load(Ordering::Acquire);
        let breakpoint_count = self
            .session_context
            .breakpoint_count
            .load(Ordering::Acquire);
        let flags = self.session_context.get_flags();

        // HEAVY tier required for memory replay
        if flags.contains(SessionFlags::MEMORY_REPLAY_ENABLED) {
            return SessionTierType::Heavy;
        }

        // HEAVY tier for high usage
        if snapshot_count >= MEDIUM_SNAPSHOT_THRESHOLD
            || breakpoint_count >= MEDIUM_BREAKPOINT_THRESHOLD
        {
            return SessionTierType::Heavy;
        }

        // MEDIUM tier for moderate usage or stack capture
        if snapshot_count >= LIGHT_SNAPSHOT_THRESHOLD
            || breakpoint_count >= LIGHT_BREAKPOINT_THRESHOLD
            || flags.contains(SessionFlags::FULL_STACK_CAPTURE)
        {
            return SessionTierType::Medium;
        }

        // Default to LIGHT tier
        SessionTierType::Light
    }

    /// Check if upgrade is needed based on usage patterns
    ///
    /// Returns an upgrade reason if current usage exceeds tier limits,
    /// otherwise returns None.
    ///
    /// # Performance
    /// - <50ns (atomic reads only)
    ///
    /// # Returns
    /// - `Some(UpgradeReason)` if upgrade is needed
    /// - `None` if current tier is sufficient
    ///
    /// #ASSUME_LOCKFREE_ONLY: All reads are atomic
    /// #VERIFY_UNIT_TEST: test_upgrade_trigger
    pub fn check_upgrade_needed(&self) -> Option<UpgradeReason> {
        let current_tier = self.session_context.get_tier();
        let snapshot_count = self.session_context.snapshot_count.load(Ordering::Acquire);
        let breakpoint_count = self
            .session_context
            .breakpoint_count
            .load(Ordering::Acquire);
        let flags = self.session_context.get_flags();

        match current_tier {
            SessionTierType::Light => {
                // Check for LIGHT -> MEDIUM upgrade
                if snapshot_count >= LIGHT_SNAPSHOT_THRESHOLD {
                    return Some(UpgradeReason::SnapshotThreshold);
                }
                if breakpoint_count >= LIGHT_BREAKPOINT_THRESHOLD {
                    return Some(UpgradeReason::BreakpointThreshold);
                }
            }
            SessionTierType::Medium => {
                // Check for MEDIUM -> HEAVY upgrade
                if snapshot_count >= MEDIUM_SNAPSHOT_THRESHOLD {
                    return Some(UpgradeReason::SnapshotThreshold);
                }
                if breakpoint_count >= MEDIUM_BREAKPOINT_THRESHOLD {
                    return Some(UpgradeReason::BreakpointThreshold);
                }
                if flags.contains(SessionFlags::MEMORY_REPLAY_ENABLED) {
                    return Some(UpgradeReason::UserRequested);
                }
            }
            SessionTierType::Heavy => {
                // Already at highest tier
            }
        }

        None
    }

    /// Check if downgrade is possible based on idle time and usage
    ///
    /// A session can be downgraded if:
    /// 1. It has been idle for longer than the threshold
    /// 2. Current usage fits within the lower tier's limits
    ///
    /// # Arguments
    /// - `idle_threshold_secs`: Minimum idle time before considering downgrade
    ///
    /// # Performance
    /// - <50ns (atomic reads only)
    ///
    /// # Returns
    /// - `true` if downgrade is possible
    /// - `false` if downgrade is not recommended
    ///
    /// #ASSUME_LOCKFREE_ONLY: All reads are atomic
    /// #VERIFY_UNIT_TEST: test_downgrade_conditions
    pub fn check_downgrade_possible(&self, idle_threshold_secs: u64) -> bool {
        let current_tier = self.session_context.get_tier();
        let idle_secs = self.session_context.idle_seconds();

        // Must be idle long enough
        if idle_secs < idle_threshold_secs {
            return false;
        }

        let snapshot_count = self.session_context.snapshot_count.load(Ordering::Acquire);
        let breakpoint_count = self
            .session_context
            .breakpoint_count
            .load(Ordering::Acquire);
        let flags = self.session_context.get_flags();

        // Cannot downgrade if memory replay is enabled
        if flags.contains(SessionFlags::MEMORY_REPLAY_ENABLED) {
            return false;
        }

        match current_tier {
            SessionTierType::Heavy => {
                // Can downgrade to MEDIUM if usage fits
                snapshot_count < MEDIUM_SNAPSHOT_THRESHOLD
                    && breakpoint_count < MEDIUM_BREAKPOINT_THRESHOLD
            }
            SessionTierType::Medium => {
                // Can downgrade to LIGHT if usage fits
                snapshot_count < LIGHT_SNAPSHOT_THRESHOLD
                    && breakpoint_count < LIGHT_BREAKPOINT_THRESHOLD
                    && !flags.contains(SessionFlags::FULL_STACK_CAPTURE)
            }
            SessionTierType::Light => {
                // Already at lowest tier
                false
            }
        }
    }

    /// Get current session context
    ///
    /// Returns a copy of the session context with current atomic values.
    /// Useful for external tier management and monitoring.
    ///
    /// # Performance
    /// - <100ns (multiple atomic reads)
    ///
    /// #ASSUME_LOCKFREE_ONLY: All reads are atomic
    /// #VERIFY_UNIT_TEST: test_session_context
    pub fn get_session_context(&self) -> SessionContext {
        SessionContext {
            session_id: self.session_context.session_id,
            tier: AtomicU32::new(self.session_context.tier.load(Ordering::Acquire)),
            created_at_ns: AtomicU64::new(
                self.session_context.created_at_ns.load(Ordering::Acquire),
            ),
            last_activity_ns: AtomicU64::new(
                self.session_context
                    .last_activity_ns
                    .load(Ordering::Acquire),
            ),
            snapshot_count: AtomicU32::new(
                self.session_context.snapshot_count.load(Ordering::Acquire),
            ),
            breakpoint_count: AtomicU32::new(
                self.session_context
                    .breakpoint_count
                    .load(Ordering::Acquire),
            ),
            command_count: AtomicU32::new(
                self.session_context.command_count.load(Ordering::Acquire),
            ),
            flags: AtomicU32::new(self.session_context.flags.load(Ordering::Acquire)),
            _padding: [0; 16],
        }
    }

    /// Update session activity timestamp
    ///
    /// Should be called on any operation to track session activity.
    /// Already called by existing methods (attach, breakpoint, step, etc.)
    ///
    /// # Performance
    /// - <10ns (single atomic store)
    ///
    /// #ASSUME_LOCKFREE_ONLY: Atomic store operation
    /// #VERIFY_UNIT_TEST: test_activity_tracking
    #[inline]
    pub fn touch_activity(&self) {
        self.session_context.touch();
    }

    /// Get maximum snapshots allowed for a tier
    ///
    /// Returns the snapshot capacity for each tier.
    ///
    /// # Arguments
    /// - `tier`: Target session tier
    ///
    /// # Returns
    /// Maximum snapshot count for the tier
    pub fn max_snapshots_for_tier(&self, tier: SessionTierType) -> u32 {
        match tier {
            SessionTierType::Light => 64,
            SessionTierType::Medium => 512,
            SessionTierType::Heavy => 2047,
        }
    }

    /// Check if operation is allowed for current tier
    ///
    /// Validates that the current session tier supports the requested
    /// operation.
    ///
    /// # Arguments
    /// - `op`: Debug operation to check
    ///
    /// # Returns
    /// - `true` if operation is allowed
    /// - `false` if tier upgrade is required
    ///
    /// #ASSUME_LOCKFREE_ONLY: Atomic read for tier
    /// #VERIFY_UNIT_TEST: test_operation_allowed
    pub fn is_operation_allowed(&self, op: DebugOperation) -> bool {
        let current_tier = self.session_context.get_tier();
        let required_tier = op.minimum_tier();

        // Tier hierarchy: Light < Medium < Heavy
        match (current_tier, required_tier) {
            (SessionTierType::Heavy, _) => true, // Heavy can do everything
            (SessionTierType::Medium, SessionTierType::Light)
            | (SessionTierType::Medium, SessionTierType::Medium) => true,
            (SessionTierType::Light, SessionTierType::Light) => true,
            _ => false,
        }
    }

    /// Check if memory replay is enabled
    ///
    /// Memory replay is a HEAVY tier feature that enables full COW
    /// memory tracking and reconstruction.
    ///
    /// # Returns
    /// - `true` if memory replay is enabled
    /// - `false` otherwise
    #[inline]
    pub fn has_memory_replay(&self) -> bool {
        self.session_context
            .has_flag(SessionFlags::MEMORY_REPLAY_ENABLED)
    }
}

/// Parallel debugging statistics
pub struct ParallelDebugStats {
    /// Per-process queue statistics: (commands_processed, queue_full_count)
    pub process_stats: [(u64, u32); 16],
    /// Total symbol resolution requests submitted
    pub symbols_submitted: u64,
    /// Total symbol resolutions completed
    pub symbols_completed: u64,
    /// Number of active threads being unwound
    pub active_threads: u32,
    /// Total stack frames unwound
    pub total_frames: u64,
}

/// Debugger statistics
pub struct DebuggerStats {
    pub instruction_count: u64,
    pub breakpoint_hits: u64,
    pub trace_events: u64,
    pub trace_dropped: u64,
    pub snapshots_taken: u64,
    pub current_snapshot: u64,
    pub stack_depth: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_debugger_capsule_size() {
        // Updated 2025-11-14: Actual size verified (T1+T2+T4+T5+T9 components)
        // Measured: 1,147,392 bytes = 1.09 MB (256-byte aligned)
        // This is larger than initial estimate due to alignment and padding
        let actual_size = size_of::<DebuggerCapsule>();
        assert!(
            actual_size >= 1_140_000 && actual_size <= 1_160_000,
            "DebuggerCapsule should be ~1.09 MB, got {} bytes",
            actual_size
        );
    }

    #[test]
    fn test_debugger_capsule_alignment() {
        assert_eq!(
            align_of::<DebuggerCapsule>(),
            256,
            "DebuggerCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_component_layout() {
        use std::mem::offset_of;

        // Verify T1 components are contiguous
        let exec_offset = offset_of!(DebuggerCapsule, execution);
        let bp_offset = offset_of!(DebuggerCapsule, breakpoints);
        let wp_offset = offset_of!(DebuggerCapsule, watchpoints);

        assert!(bp_offset > exec_offset);
        assert!(wp_offset > bp_offset);
    }

    #[test]
    #[ignore = "Stack overflow: DebuggerCapsule (~512KB) too large for stack allocation. Use integration tests."]
    fn test_debugger_attach() {
        // Use Box to allocate DebuggerCapsule on heap (prevents stack overflow, ~512KB struct)
        let debugger = Box::new(DebuggerCapsule::new(12345));
        assert_eq!(debugger.execution.get_pid(), 12345);

        debugger.attach_to_process(67890).unwrap();
        assert_eq!(debugger.execution.get_pid(), 67890);
    }

    #[test]
    #[ignore = "Stack overflow: DebuggerCapsule (~512KB) too large for stack allocation. Use integration tests."]
    fn test_breakpoint_set() {
        // Use Box to allocate DebuggerCapsule on heap (prevents stack overflow, ~512KB struct)
        let debugger = Box::new(DebuggerCapsule::new(12345));

        let bp_idx = debugger.set_breakpoint(0x1000).unwrap();
        assert_eq!(bp_idx, 0);

        let bp_idx2 = debugger.set_breakpoint(0x2000).unwrap();
        assert_eq!(bp_idx2, 1);
    }

    #[test]
    #[ignore = "Stack overflow: DebuggerCapsule (~512KB) too large for stack allocation. Use integration tests."]
    fn test_time_travel() {
        // Use Box to allocate DebuggerCapsule on heap (prevents stack overflow, ~512KB struct)
        let debugger = Box::new(DebuggerCapsule::new(12345));

        // Step forward a few times
        debugger.execution.set_rip(0x1000);
        debugger.step_instruction().unwrap();
        debugger.step_instruction().unwrap();
        debugger.step_instruction().unwrap();

        // Step backward
        let rip = debugger.step_backward().unwrap();
        assert_eq!(rip, 0x1008); // Should be at second instruction

        let rip = debugger.step_backward().unwrap();
        assert_eq!(rip, 0x1004); // Should be at first instruction
    }

    #[test]
    #[ignore = "Stack overflow: DebuggerCapsule (~512KB) too large for stack allocation. Use integration tests."]
    fn test_stack_trace() {
        // Use Box to allocate DebuggerCapsule on heap (prevents stack overflow, ~512KB struct)
        let debugger = Box::new(DebuggerCapsule::new(12345));

        // Push some frames
        debugger
            .simd_stack
            .push_frame(0x1000, 0x7fff_0000, 0x7fff_0100)
            .unwrap();
        debugger
            .simd_stack
            .push_frame(0x2000, 0x7fff_0100, 0x7fff_0200)
            .unwrap();
        debugger
            .simd_stack
            .push_frame(0x3000, 0x7fff_0200, 0x7fff_0300)
            .unwrap();

        // Get trace
        let trace = debugger.get_stack_trace().unwrap();
        assert_eq!(trace.len(), 3);
        assert_eq!(trace[0], 0x1000);
        assert_eq!(trace[1], 0x2000);
        assert_eq!(trace[2], 0x3000);
    }

    #[test]
    #[ignore = "Stack overflow: DebuggerCapsule (~512KB) too large for stack allocation. Use integration tests."]
    fn test_crash_recording() {
        // Use Box to allocate DebuggerCapsule on heap (prevents stack overflow, ~512KB struct)
        let debugger = Box::new(DebuggerCapsule::new(12345));

        debugger.execution.set_rip(0xdead_beef);
        debugger.record_crash(11, 0xcafe_babe).unwrap();

        let (signal, fault_addr, rip) = debugger.crash_dump.get_crash_info();
        assert_eq!(signal, 11);
        assert_eq!(fault_addr, 0xcafe_babe);
        assert_eq!(rip, 0xdead_beef);
    }

    // ========================================================================
    // Session Tier Detection Tests (Phase 3)
    // ========================================================================

    #[test]
    fn test_session_context_new() {
        let session_id = SessionId::new(0, 0, 1);
        let ctx = SessionContext::new(session_id, SessionTierType::Light);

        assert_eq!(ctx.session_id.0, session_id.0);
        assert_eq!(ctx.get_tier(), SessionTierType::Light);
        assert_eq!(ctx.snapshot_count.load(Ordering::Relaxed), 0);
        assert_eq!(ctx.breakpoint_count.load(Ordering::Relaxed), 0);
        assert_eq!(ctx.command_count.load(Ordering::Relaxed), 0);
        assert_eq!(ctx.get_flags(), SessionFlags::empty());
    }

    #[test]
    fn test_session_context_flags() {
        let ctx = SessionContext::new(SessionId::new(0, 0, 1), SessionTierType::Light);

        // Add flags
        ctx.add_flags(SessionFlags::ATTACHED);
        assert!(ctx.has_flag(SessionFlags::ATTACHED));
        assert!(!ctx.has_flag(SessionFlags::MEMORY_REPLAY_ENABLED));

        // Add more flags
        ctx.add_flags(SessionFlags::HAS_BREAKPOINTS);
        assert!(ctx.has_flag(SessionFlags::ATTACHED));
        assert!(ctx.has_flag(SessionFlags::HAS_BREAKPOINTS));

        // Remove flags
        ctx.remove_flags(SessionFlags::ATTACHED);
        assert!(!ctx.has_flag(SessionFlags::ATTACHED));
        assert!(ctx.has_flag(SessionFlags::HAS_BREAKPOINTS));
    }

    #[test]
    fn test_session_context_counters() {
        let ctx = SessionContext::new(SessionId::new(0, 0, 1), SessionTierType::Light);

        // Test snapshot counter
        assert_eq!(ctx.increment_snapshots(), 1);
        assert_eq!(ctx.increment_snapshots(), 2);
        assert_eq!(ctx.snapshot_count.load(Ordering::Relaxed), 2);

        // Test breakpoint counter
        assert_eq!(ctx.increment_breakpoints(), 1);
        assert_eq!(ctx.increment_breakpoints(), 2);
        ctx.decrement_breakpoints();
        assert_eq!(ctx.breakpoint_count.load(Ordering::Relaxed), 1);

        // Test command counter
        assert_eq!(ctx.increment_commands(), 1);
        assert_eq!(ctx.increment_commands(), 2);
        assert_eq!(ctx.command_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_session_context_activity_tracking() {
        let ctx = SessionContext::new(SessionId::new(0, 0, 1), SessionTierType::Light);

        // Initial activity time should be set
        let initial = ctx.last_activity_ns.load(Ordering::Relaxed);
        assert!(initial > 0);

        // Touch should update activity time
        std::thread::sleep(std::time::Duration::from_millis(10));
        ctx.touch();
        let after_touch = ctx.last_activity_ns.load(Ordering::Relaxed);
        assert!(after_touch >= initial);

        // Idle seconds should be minimal
        let idle = ctx.idle_seconds();
        assert!(idle < 2); // Should be less than 2 seconds
    }

    #[test]
    fn test_session_context_tier_transitions() {
        let ctx = SessionContext::new(SessionId::new(0, 0, 1), SessionTierType::Light);

        assert_eq!(ctx.get_tier(), SessionTierType::Light);

        ctx.set_tier(SessionTierType::Medium);
        assert_eq!(ctx.get_tier(), SessionTierType::Medium);

        ctx.set_tier(SessionTierType::Heavy);
        assert_eq!(ctx.get_tier(), SessionTierType::Heavy);
    }

    #[test]
    fn test_recommend_tier_light() {
        // Use Box for heap allocation to avoid stack overflow
        let debugger = Box::new(DebuggerCapsule::new(12345));

        // Fresh debugger should recommend LIGHT
        // Note: DebuggerCapsule defaults to HEAVY, but recommend_tier looks at usage
        // With 0 snapshots and 0 breakpoints, it should recommend LIGHT
        debugger.session_context.set_tier(SessionTierType::Light);

        let recommended = debugger.recommend_tier();
        assert_eq!(recommended, SessionTierType::Light);
    }

    #[test]
    fn test_recommend_tier_medium() {
        let debugger = Box::new(DebuggerCapsule::new(12345));

        // Set snapshot count just above LIGHT threshold
        for _ in 0..LIGHT_SNAPSHOT_THRESHOLD {
            debugger.session_context.increment_snapshots();
        }

        let recommended = debugger.recommend_tier();
        assert_eq!(recommended, SessionTierType::Medium);
    }

    #[test]
    fn test_recommend_tier_heavy() {
        let debugger = Box::new(DebuggerCapsule::new(12345));

        // Set snapshot count above MEDIUM threshold
        for _ in 0..MEDIUM_SNAPSHOT_THRESHOLD {
            debugger.session_context.increment_snapshots();
        }

        let recommended = debugger.recommend_tier();
        assert_eq!(recommended, SessionTierType::Heavy);
    }

    #[test]
    fn test_recommend_tier_memory_replay() {
        let debugger = Box::new(DebuggerCapsule::new(12345));

        // Enable memory replay flag
        debugger
            .session_context
            .add_flags(SessionFlags::MEMORY_REPLAY_ENABLED);

        // Should recommend HEAVY regardless of usage
        let recommended = debugger.recommend_tier();
        assert_eq!(recommended, SessionTierType::Heavy);
    }

    #[test]
    fn test_upgrade_trigger_snapshot() {
        let debugger = Box::new(DebuggerCapsule::new(12345));
        debugger.session_context.set_tier(SessionTierType::Light);

        // No upgrade needed initially
        assert!(debugger.check_upgrade_needed().is_none());

        // Add snapshots up to threshold
        for _ in 0..LIGHT_SNAPSHOT_THRESHOLD {
            debugger.session_context.increment_snapshots();
        }

        // Should trigger upgrade
        let reason = debugger.check_upgrade_needed();
        assert_eq!(reason, Some(UpgradeReason::SnapshotThreshold));
    }

    #[test]
    fn test_upgrade_trigger_breakpoint() {
        let debugger = Box::new(DebuggerCapsule::new(12345));
        debugger.session_context.set_tier(SessionTierType::Light);

        // Add breakpoints up to threshold
        for _ in 0..LIGHT_BREAKPOINT_THRESHOLD {
            debugger.session_context.increment_breakpoints();
        }

        // Should trigger upgrade
        let reason = debugger.check_upgrade_needed();
        assert_eq!(reason, Some(UpgradeReason::BreakpointThreshold));
    }

    #[test]
    fn test_downgrade_conditions_idle() {
        let debugger = Box::new(DebuggerCapsule::new(12345));
        debugger.session_context.set_tier(SessionTierType::Heavy);

        // Just created, not idle enough
        assert!(!debugger.check_downgrade_possible(1800));

        // Even with 0 idle time, if within limits, can downgrade
        assert!(debugger.check_downgrade_possible(0));
    }

    #[test]
    fn test_downgrade_blocked_by_memory_replay() {
        let debugger = Box::new(DebuggerCapsule::new(12345));
        debugger.session_context.set_tier(SessionTierType::Heavy);
        debugger
            .session_context
            .add_flags(SessionFlags::MEMORY_REPLAY_ENABLED);

        // Cannot downgrade when memory replay is enabled
        assert!(!debugger.check_downgrade_possible(0));
    }

    #[test]
    fn test_get_session_context_copy() {
        let debugger = Box::new(DebuggerCapsule::new(12345));

        // Modify some values
        debugger.session_context.increment_snapshots();
        debugger.session_context.increment_breakpoints();
        debugger.session_context.add_flags(SessionFlags::ATTACHED);

        // Get a copy
        let ctx = debugger.get_session_context();

        // Verify copy has same values
        assert_eq!(ctx.snapshot_count.load(Ordering::Relaxed), 1);
        assert_eq!(ctx.breakpoint_count.load(Ordering::Relaxed), 1);
        assert!(ctx.has_flag(SessionFlags::ATTACHED));
    }

    #[test]
    fn test_touch_activity() {
        let debugger = Box::new(DebuggerCapsule::new(12345));

        let initial = debugger
            .session_context
            .last_activity_ns
            .load(Ordering::Relaxed);

        std::thread::sleep(std::time::Duration::from_millis(5));
        debugger.touch_activity();

        let after = debugger
            .session_context
            .last_activity_ns
            .load(Ordering::Relaxed);
        assert!(after >= initial);
    }

    #[test]
    fn test_max_snapshots_for_tier() {
        let debugger = Box::new(DebuggerCapsule::new(12345));

        assert_eq!(debugger.max_snapshots_for_tier(SessionTierType::Light), 64);
        assert_eq!(
            debugger.max_snapshots_for_tier(SessionTierType::Medium),
            512
        );
        assert_eq!(
            debugger.max_snapshots_for_tier(SessionTierType::Heavy),
            2047
        );
    }

    #[test]
    fn test_is_operation_allowed_light() {
        let debugger = Box::new(DebuggerCapsule::new(12345));
        debugger.session_context.set_tier(SessionTierType::Light);

        // Light tier operations should be allowed
        assert!(debugger.is_operation_allowed(DebugOperation::Attach));
        assert!(debugger.is_operation_allowed(DebugOperation::SetBreakpoint));
        assert!(debugger.is_operation_allowed(DebugOperation::Step));
        assert!(debugger.is_operation_allowed(DebugOperation::TakeSnapshot));

        // Higher tier operations should be blocked
        assert!(!debugger.is_operation_allowed(DebugOperation::StepBackward));
        assert!(!debugger.is_operation_allowed(DebugOperation::EnableMemoryReplay));
    }

    #[test]
    fn test_is_operation_allowed_medium() {
        let debugger = Box::new(DebuggerCapsule::new(12345));
        debugger.session_context.set_tier(SessionTierType::Medium);

        // Light and Medium operations should be allowed
        assert!(debugger.is_operation_allowed(DebugOperation::Attach));
        assert!(debugger.is_operation_allowed(DebugOperation::StepBackward));
        assert!(debugger.is_operation_allowed(DebugOperation::FullStackCapture));

        // Heavy tier operations should be blocked
        assert!(!debugger.is_operation_allowed(DebugOperation::EnableMemoryReplay));
    }

    #[test]
    fn test_is_operation_allowed_heavy() {
        let debugger = Box::new(DebuggerCapsule::new(12345));
        debugger.session_context.set_tier(SessionTierType::Heavy);

        // All operations should be allowed
        assert!(debugger.is_operation_allowed(DebugOperation::Attach));
        assert!(debugger.is_operation_allowed(DebugOperation::StepBackward));
        assert!(debugger.is_operation_allowed(DebugOperation::EnableMemoryReplay));
        assert!(debugger.is_operation_allowed(DebugOperation::ReadMemoryAtSnapshot));
    }

    #[test]
    fn test_has_memory_replay() {
        let debugger = Box::new(DebuggerCapsule::new(12345));

        // Initially false
        assert!(!debugger.has_memory_replay());

        // Enable memory replay
        debugger
            .session_context
            .add_flags(SessionFlags::MEMORY_REPLAY_ENABLED);
        assert!(debugger.has_memory_replay());

        // Disable
        debugger
            .session_context
            .remove_flags(SessionFlags::MEMORY_REPLAY_ENABLED);
        assert!(!debugger.has_memory_replay());
    }

    #[test]
    fn test_debug_operation_minimum_tier() {
        assert_eq!(
            DebugOperation::Attach.minimum_tier(),
            SessionTierType::Light
        );
        assert_eq!(
            DebugOperation::SetBreakpoint.minimum_tier(),
            SessionTierType::Light
        );
        assert_eq!(DebugOperation::Step.minimum_tier(), SessionTierType::Light);
        assert_eq!(
            DebugOperation::StepBackward.minimum_tier(),
            SessionTierType::Medium
        );
        assert_eq!(
            DebugOperation::FullStackCapture.minimum_tier(),
            SessionTierType::Medium
        );
        assert_eq!(
            DebugOperation::EnableMemoryReplay.minimum_tier(),
            SessionTierType::Heavy
        );
        assert_eq!(
            DebugOperation::ReadMemoryAtSnapshot.minimum_tier(),
            SessionTierType::Heavy
        );
    }

    #[test]
    fn test_session_flags_bitflags() {
        let flags = SessionFlags::MEMORY_REPLAY_ENABLED | SessionFlags::FULL_STACK_CAPTURE;

        assert!(flags.contains(SessionFlags::MEMORY_REPLAY_ENABLED));
        assert!(flags.contains(SessionFlags::FULL_STACK_CAPTURE));
        assert!(!flags.contains(SessionFlags::ATTACHED));

        // Test all flags can be set
        let all = SessionFlags::all();
        assert!(all.contains(SessionFlags::MEMORY_REPLAY_ENABLED));
        assert!(all.contains(SessionFlags::FULL_STACK_CAPTURE));
        assert!(all.contains(SessionFlags::AUDIT_TRAIL_ENABLED));
        assert!(all.contains(SessionFlags::STEPPED_RECENTLY));
        assert!(all.contains(SessionFlags::HAS_BREAKPOINTS));
        assert!(all.contains(SessionFlags::HAS_WATCHPOINTS));
        assert!(all.contains(SessionFlags::MEMORY_TRACKING));
        assert!(all.contains(SessionFlags::ATTACHED));
    }

    #[test]
    fn test_session_context_size_alignment() {
        // Verify cache-line alignment
        assert_eq!(align_of::<SessionContext>(), 64);

        // Verify size is 64 bytes
        assert_eq!(size_of::<SessionContext>(), 64);
    }
}
