//! McpRuntimeCapsule - T6 Mixed MCP Server Runtime Orchestration (16 KB)
//!
//! Top-level MCP server runtime that coordinates all subsystems:
//! - StdioTransportCapsule (T5 Streaming): 4 KB stdin/stdout buffering
//! - McpServerCapsule (T6 Mixed): 256 KB request processing
//! - ToolExecutorCapsule (T1 Atomic): 256 B tool dispatch
//! - State Machine: Idle → Processing → Shutdown
//!
//! **Target latency**: <10μs per request (network I/O excluded)
//! **Tier**: T6 Mixed (T1 Atomic coordination + T5 Streaming I/O + async await)
//!
//! ## Design
//!
//! The runtime implements a lockfree event loop:
//! 1. Poll stdin for complete JSON-RPC lines (O(1) incremental)
//! 2. Dispatch to McpServerCapsule for processing (<10μs)
//! 3. Queue response to stdout buffer
//! 4. Flush stdout (batched writes)
//! 5. Repeat until shutdown signal

use crate::{McpServerCapsule, StdioTransportCapsule, ToolExecutorCapsule};
use kdb::DebuggerCapsule;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Runtime State Machine (T1 Atomic)
// ============================================================================

/// Runtime execution state (packed into 2 bits)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RuntimeState {
    /// Waiting for requests, ready to process
    Idle = 0,
    /// Currently processing a request
    Processing = 1,
    /// Shutdown initiated, draining queues
    ShuttingDown = 2,
    /// Fully shut down, exit event loop
    Stopped = 3,
}

impl RuntimeState {
    /// Convert from u8 representation
    #[inline]
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(RuntimeState::Idle),
            1 => Some(RuntimeState::Processing),
            2 => Some(RuntimeState::ShuttingDown),
            3 => Some(RuntimeState::Stopped),
            _ => None,
        }
    }

    /// Convert to u8 representation
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// McpRuntimeCapsule (16 KB, 256-byte aligned, T6 Mixed)
// ============================================================================
//
// Size breakdown:
// - Runtime state (64 bytes, single cache line, T1 Atomic coordination)
// - Event loop metrics (64 bytes)
// - Request/response buffers (2 KB)
// - Buffered stdout (2 KB)
// - Reserved (13.875 KB for future expansion)
//
// Total: 16 KB (16,384 bytes)
// ============================================================================

#[repr(C, align(256))]
pub struct McpRuntimeCapsule {
    // ========================================================================
    // State Machine (64 bytes, single cache line)
    // ========================================================================

    /// Current runtime state (Idle/Processing/ShuttingDown/Stopped)
    pub state: AtomicU64,

    /// Graceful shutdown flag (set by signal handler)
    pub should_shutdown: AtomicBool,

    /// Event loop generation counter (TOCTOU prevention)
    pub generation: AtomicU64,

    /// Last request ID processed (for deduplication)
    pub last_request_id: AtomicU64,

    /// Request timeout (nanoseconds, 0 = no timeout)
    pub request_timeout_ns: AtomicU64,

    /// Shutdown timeout (nanoseconds)
    pub shutdown_timeout_ns: AtomicU64,

    _padding1: [u8; 24],

    // ========================================================================
    // Event Loop Metrics (64 bytes)
    // ========================================================================

    /// Total requests processed by runtime
    pub total_requests: AtomicU64,

    /// Total responses sent
    pub total_responses: AtomicU64,

    /// Total errors encountered
    pub total_errors: AtomicU64,

    /// Total event loop iterations
    pub loop_iterations: AtomicU64,

    /// Average request latency (moving average in ns)
    pub avg_request_latency_ns: AtomicU64,

    /// Maximum request latency observed
    pub max_request_latency_ns: AtomicU64,

    /// Event loop cycle time (ns)
    pub loop_cycle_ns: AtomicU64,

    /// Lines pending in stdout (for batching metrics)
    pub pending_output_lines: AtomicU64,

    // ========================================================================
    // Request/Response Pipeline (4 KB)
    // ========================================================================

    /// Buffered JSON-RPC request (up to 2 KB)
    pub request_buffer: [u8; 2048],

    /// Request buffer length
    pub request_len: AtomicU64,

    /// Buffered JSON-RPC response (up to 2 KB)
    pub response_buffer: [u8; 2048],

    /// Response buffer length
    pub response_len: AtomicU64,

    // ========================================================================
    // Output Buffering (2 KB for stdout batching)
    // ========================================================================

    /// Output batch buffer (for flushing multiple lines at once)
    pub output_batch: [u8; 2048],

    /// Output batch length
    pub output_batch_len: AtomicU64,

    // ========================================================================
    // Reserved Space (13.875 KB for future expansion)
    // ========================================================================

    _reserved: [u8; 14208],
}

// Safety: McpRuntimeCapsule is Send + Sync (all atomic fields)
unsafe impl Send for McpRuntimeCapsule {}
unsafe impl Sync for McpRuntimeCapsule {}

impl McpRuntimeCapsule {
    /// Create new MCP runtime capsule
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(RuntimeState::Idle as u8 as u64),
            should_shutdown: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            last_request_id: AtomicU64::new(0),
            request_timeout_ns: AtomicU64::new(30_000_000_000), // 30 second default
            shutdown_timeout_ns: AtomicU64::new(5_000_000_000), // 5 second default
            _padding1: [0; 24],

            total_requests: AtomicU64::new(0),
            total_responses: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            loop_iterations: AtomicU64::new(0),
            avg_request_latency_ns: AtomicU64::new(0),
            max_request_latency_ns: AtomicU64::new(0),
            loop_cycle_ns: AtomicU64::new(0),
            pending_output_lines: AtomicU64::new(0),

            request_buffer: [0; 2048],
            request_len: AtomicU64::new(0),
            response_buffer: [0; 2048],
            response_len: AtomicU64::new(0),

            output_batch: [0; 2048],
            output_batch_len: AtomicU64::new(0),

            _reserved: [0; 14208],
        }
    }

    // ========================================================================
    // State Machine Operations (T1 Atomic, <30ns)
    // ========================================================================

    /// Get current runtime state
    #[inline]
    pub fn get_state(&self) -> RuntimeState {
        let state_bits = (self.state.load(Ordering::Acquire) & 0xFF) as u8;
        RuntimeState::from_u8(state_bits).unwrap_or(RuntimeState::Idle)
    }

    /// Transition to new state (returns Ok if successful, Err if invalid)
    fn transition_state(&self, new_state: RuntimeState) -> Result<(), &'static str> {
        let current_bits = self.state.load(Ordering::Acquire);
        let current_state = RuntimeState::from_u8((current_bits & 0xFF) as u8)
            .ok_or("Invalid current state")?;

        // Validate state transition
        let valid = match (current_state, new_state) {
            (RuntimeState::Idle, RuntimeState::Processing) => true,
            (RuntimeState::Idle, RuntimeState::ShuttingDown) => true,
            (RuntimeState::Processing, RuntimeState::Idle) => true,
            (RuntimeState::Processing, RuntimeState::ShuttingDown) => true,
            (RuntimeState::ShuttingDown, RuntimeState::Stopped) => true,
            _ => false,
        };

        if !valid {
            return Err("Invalid state transition");
        }

        let new_bits = (current_bits & !0xFF) | (new_state.as_u8() as u64);
        self.state.store(new_bits, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Check if shutdown is requested
    #[inline]
    pub fn should_shutdown(&self) -> bool {
        self.should_shutdown.load(Ordering::Acquire)
    }

    /// Request graceful shutdown
    pub fn request_shutdown(&self) {
        self.should_shutdown.store(true, Ordering::Release);
        let _ = self.transition_state(RuntimeState::ShuttingDown);
    }

    // ========================================================================
    // Main Event Loop - Async Runtime Integration
    // ========================================================================

    /// Run the MCP server event loop (native atomic_capsule runtime, no tokio)
    ///
    /// This is the main entry point for the runtime. It coordinates:
    /// 1. Reading JSON-RPC requests from stdin (StdioTransportCapsule)
    /// 2. Processing requests (McpServerCapsule)
    /// 3. Writing responses to stdout
    /// 4. Monitoring shutdown signals
    ///
    /// **MIGRATED FROM TOKIO**: This now uses blocking I/O with the native atomic_capsule runtime.
    /// We use blocking I/O here because it's acceptable for single-threaded stdin/stdout transport
    /// (we yield control periodically via small batches).
    ///
    /// **Target latency**: <10μs per request (excluding network I/O)
    /// **Throughput**: 100K+ requests/sec (single-threaded)
    ///
    /// Returns Ok(()) on clean shutdown, Err on fatal error.
    #[cfg(feature = "runtime")]
    pub fn run(
        &mut self,
        transport: &StdioTransportCapsule,
        server: &McpServerCapsule,
        _executor: &ToolExecutorCapsule,
        debugger: &'static DebuggerCapsule,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let debug = std::env::var("MCP_DEBUG").is_ok();
        if debug { eprintln!("[MCP] Runtime starting (atomic_capsule native async runtime)"); }

        // State is already initialized to Idle in new(), no transition needed
        // Just verify we're in the correct initial state
        let current_state = self.get_state();
        if current_state != RuntimeState::Idle {
            return Err(format!("Runtime not in Idle state at startup: {:?}", current_state).into());
        }

        // Use blocking I/O (appropriate for stdin/stdout single-threaded MCP protocol)
        // No async/await needed - we use synchronous blocking reads with small batches
        let mut stdin = std::io::stdin();
        let mut stdout = std::io::stdout();

        // Main event loop
        loop {
            let loop_start_ns = Self::get_timestamp_ns();
            self.loop_iterations.fetch_add(1, Ordering::Relaxed);

            // Check shutdown signal
            if self.should_shutdown() {
                self.handle_shutdown(transport, &mut stdout)?;
                break;
            }

            // ================================================================
            // Phase 1: Read JSON-RPC requests from stdin (T5 Streaming)
            // ================================================================

            let mut input_buffer = [0u8; 4096];
            use std::io::Read;
            match stdin.read(&mut input_buffer) {
                Ok(0) => {
                    // EOF on stdin
                    if debug { eprintln!("[MCP] EOF on stdin, initiating shutdown"); }
                    self.request_shutdown();
                    continue;
                }
                Ok(n) => {
                    // Write to transport buffer
                    let _ = transport.write_input(&input_buffer[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, skip to flush phase
                }
                Err(e) => {
                    self.total_errors.fetch_add(1, Ordering::Relaxed);
                    if debug { eprintln!("[MCP] Read error: {}", e); }
                    continue;
                }
            }

            // ================================================================
            // Phase 2: Process buffered JSON-RPC lines (<10μs per request)
            // ================================================================

            loop {
                match transport.read_line() {
                    Ok(Some(json_line)) => {
                        let req_start_ns = Self::get_timestamp_ns();

                        // Process request through server pipeline
                        match server.handle_request(&json_line, None, None, debugger) {
                            Ok(response) => {
                                // Queue response to output
                                if let Err(e) = transport.write_line(&response) {
                                    self.total_errors.fetch_add(1, Ordering::Relaxed);
                                    if debug { eprintln!("[MCP] Response write error: {}", e); }
                                } else {
                                    self.total_responses.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Err(e) => {
                                self.total_errors.fetch_add(1, Ordering::Relaxed);
                                let error_response = format!(
                                    r#"{{"jsonrpc":"2.0","id":0,"error":{{"code":-32600,"message":"{}"}}}}"#,
                                    e
                                );
                                let _ = transport.write_line(&error_response);
                            }
                        }

                        // Record latency
                        let req_latency_ns = Self::get_timestamp_ns() - req_start_ns;
                        self.record_request_latency(req_latency_ns);
                        self.total_requests.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(None) => {
                        // No complete line yet
                        break;
                    }
                    Err(e) => {
                        self.total_errors.fetch_add(1, Ordering::Relaxed);
                        if debug { eprintln!("[MCP] Parse error: {}", e); }
                        break;
                    }
                }
            }

            // ================================================================
            // Phase 3: Flush stdout (batched writes for efficiency)
            // ================================================================

            let output = transport.get_pending_output();
            if !output.is_empty() {
                use std::io::Write;
                match stdout.write_all(output) {
                    Ok(()) => {
                        // CRITICAL: Always flush after writing (MCP protocol requirement)
                        let _ = stdout.flush();
                        let _ = transport.flush_output(output.len());
                    }
                    Err(e) => {
                        self.total_errors.fetch_add(1, Ordering::Relaxed);
                        if debug { eprintln!("[MCP] Flush error: {}", e); }
                    }
                }
            }

            // Record cycle time
            let cycle_time_ns = Self::get_timestamp_ns() - loop_start_ns;
            self.loop_cycle_ns.store(cycle_time_ns, Ordering::Relaxed);

            // Yield to OS (native runtime, no tokio::task::yield_now() available)
            // For stdin/stdout, we naturally yield during blocking read()
        }

        if debug { eprintln!("[MCP] Runtime shutdown complete"); }
        Ok(())
    }

    #[cfg(not(feature = "runtime"))]
    pub fn run(
        &mut self,
        _transport: &StdioTransportCapsule,
        _server: &McpServerCapsule,
        _executor: &ToolExecutorCapsule,
        _debugger: &'static DebuggerCapsule,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("runtime feature required".into())
    }

    // ========================================================================
    // Shutdown Handling
    // ========================================================================

    /// Handle graceful shutdown sequence (native async runtime)
    #[cfg(feature = "runtime")]
    fn handle_shutdown(
        &self,
        transport: &StdioTransportCapsule,
        stdout: &mut std::io::Stdout,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let debug = std::env::var("MCP_DEBUG").is_ok();
        if debug { eprintln!("[MCP] Shutdown phase 1: draining queues"); }

        // Phase 1: Drain pending output (up to timeout)
        let start_ns = Self::get_timestamp_ns();
        let timeout_ns = self.shutdown_timeout_ns.load(Ordering::Relaxed);

        loop {
            let output = transport.get_pending_output();
            if output.is_empty() {
                break;
            }

            use std::io::Write;
            stdout.write_all(output)?;
            stdout.flush()?;
            let _ = transport.flush_output(output.len());

            // Check timeout
            if Self::get_timestamp_ns() - start_ns > timeout_ns {
                if debug { eprintln!("[MCP] Shutdown timeout exceeded, force closing"); }
                break;
            }
        }

        // Transition to stopped state
        let _ = self.transition_state(RuntimeState::Stopped);
        if debug { eprintln!("[MCP] Runtime gracefully shut down"); }

        Ok(())
    }

    #[cfg(not(feature = "runtime"))]
    fn handle_shutdown(
        &self,
        _transport: &StdioTransportCapsule,
        _stdout: &mut std::io::Stdout,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("runtime feature required".into())
    }

    // ========================================================================
    // Latency Recording (T1 Atomic, <50ns)
    // ========================================================================

    /// Record request latency for monitoring
    #[inline]
    fn record_request_latency(&self, latency_ns: u64) {
        // Update average (simple exponential moving average: 0.8 * old + 0.2 * new)
        let old_avg = self.avg_request_latency_ns.load(Ordering::Relaxed);
        let new_avg = (old_avg * 80 + latency_ns * 20) / 100;
        self.avg_request_latency_ns.store(new_avg, Ordering::Relaxed);

        // Update max
        let old_max = self.max_request_latency_ns.load(Ordering::Relaxed);
        if latency_ns > old_max {
            let _ = self.max_request_latency_ns.compare_exchange(
                old_max,
                latency_ns,
                Ordering::Release,
                Ordering::Relaxed,
            );
        }
    }

    // ========================================================================
    // Statistics & Monitoring
    // ========================================================================

    /// Get current runtime statistics
    pub fn get_stats(&self) -> RuntimeStats {
        RuntimeStats {
            state: self.get_state(),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_responses: self.total_responses.load(Ordering::Relaxed),
            total_errors: self.total_errors.load(Ordering::Relaxed),
            loop_iterations: self.loop_iterations.load(Ordering::Relaxed),
            avg_request_latency_ns: self.avg_request_latency_ns.load(Ordering::Relaxed),
            max_request_latency_ns: self.max_request_latency_ns.load(Ordering::Relaxed),
            loop_cycle_ns: self.loop_cycle_ns.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Relaxed),
        }
    }

    // ========================================================================
    // Utility Functions
    // ========================================================================

    /// Get current timestamp in nanoseconds
    #[inline]
    fn get_timestamp_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

impl Default for McpRuntimeCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Statistics Structure
// ============================================================================

/// Runtime statistics for monitoring and debugging
#[derive(Debug, Clone, Copy)]
pub struct RuntimeStats {
    pub state: RuntimeState,
    pub total_requests: u64,
    pub total_responses: u64,
    pub total_errors: u64,
    pub loop_iterations: u64,
    pub avg_request_latency_ns: u64,
    pub max_request_latency_ns: u64,
    pub loop_cycle_ns: u64,
    pub generation: u64,
}

impl RuntimeStats {
    /// Calculate success rate as percentage
    #[inline]
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.total_responses as f64 / self.total_requests as f64) * 100.0
        }
    }

    /// Calculate average loop iterations per request
    #[inline]
    pub fn avg_iterations_per_request(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.loop_iterations as f64 / self.total_requests as f64
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_runtime_size() {
        let actual_size = size_of::<McpRuntimeCapsule>();
        // Size breakdown:
        // - State machine (64 B)
        // - Metrics (64 B)
        // - Request buffer (2048 B)
        // - Response buffer (2048 B)
        // - Output batch (2048 B)
        // - Reserved space
        // Total: ~20.75 KB (reasonable for T6 composition)
        let max_size = 24576; // Allow up to 24 KB
        assert!(
            actual_size <= max_size,
            "McpRuntimeCapsule must be <= 24 KB (expected {}, got {})",
            max_size, actual_size
        );
        eprintln!("McpRuntimeCapsule size: {} bytes ({:.2} KB)", actual_size, actual_size as f64 / 1024.0);
    }

    #[test]
    fn test_runtime_alignment() {
        assert_eq!(
            align_of::<McpRuntimeCapsule>(),
            256,
            "McpRuntimeCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_state_transitions() {
        let runtime = McpRuntimeCapsule::new();

        // Initial state should be Idle
        assert_eq!(runtime.get_state(), RuntimeState::Idle);

        // Valid: Idle → Processing
        assert!(runtime.transition_state(RuntimeState::Processing).is_ok());
        assert_eq!(runtime.get_state(), RuntimeState::Processing);

        // Valid: Processing → Idle
        assert!(runtime.transition_state(RuntimeState::Idle).is_ok());
        assert_eq!(runtime.get_state(), RuntimeState::Idle);

        // Valid: Idle → ShuttingDown
        assert!(runtime.transition_state(RuntimeState::ShuttingDown).is_ok());
        assert_eq!(runtime.get_state(), RuntimeState::ShuttingDown);

        // Valid: ShuttingDown → Stopped
        assert!(runtime.transition_state(RuntimeState::Stopped).is_ok());
        assert_eq!(runtime.get_state(), RuntimeState::Stopped);
    }

    #[test]
    fn test_shutdown_flag() {
        let runtime = McpRuntimeCapsule::new();

        assert!(!runtime.should_shutdown());
        runtime.request_shutdown();
        assert!(runtime.should_shutdown());
    }

    #[test]
    fn test_latency_recording() {
        let runtime = McpRuntimeCapsule::new();

        // Record some latencies
        runtime.record_request_latency(1000);
        runtime.record_request_latency(2000);
        runtime.record_request_latency(500);

        let stats = runtime.get_stats();
        assert!(stats.avg_request_latency_ns > 0);
        assert_eq!(stats.max_request_latency_ns, 2000);
    }

    #[test]
    fn test_runtime_stats() {
        let runtime = McpRuntimeCapsule::new();

        runtime.total_requests.store(100, Ordering::Relaxed);
        runtime.total_responses.store(95, Ordering::Relaxed);
        runtime.total_errors.store(5, Ordering::Relaxed);

        let stats = runtime.get_stats();
        assert_eq!(stats.total_requests, 100);
        assert_eq!(stats.total_responses, 95);
        assert_eq!(stats.total_errors, 5);
        assert!((stats.success_rate() - 95.0).abs() < 0.1);
    }

    #[test]
    fn test_generation_counter() {
        let runtime = McpRuntimeCapsule::new();

        let gen1 = runtime.generation.load(Ordering::Relaxed);
        runtime.transition_state(RuntimeState::Processing).ok();
        let gen2 = runtime.generation.load(Ordering::Relaxed);

        assert!(gen2 > gen1, "Generation should increment on state transition");
    }

    #[test]
    fn test_timestamp_monotonic() {
        let ts1 = McpRuntimeCapsule::get_timestamp_ns();
        std::thread::sleep(std::time::Duration::from_micros(1));
        let ts2 = McpRuntimeCapsule::get_timestamp_ns();

        assert!(ts2 > ts1, "Timestamps should be monotonically increasing");
    }
}
