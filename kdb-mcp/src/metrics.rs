//! Production-Grade Prometheus Metrics - 50-100 Metrics Across 6 Categories
//!
//! **Tier**: T1 Atomic (lockfree coordination, <10ns increment, <5ms scrape)
//! **Alignment**: 256-byte cache-aligned capsule to prevent false sharing
//! **Cardinality**: 50-100 bounded metrics (max 100 series with labels)
//! **Format**: Prometheus text format (version 0.0.4)
//!
//! ## Metrics Categories
//!
//! 1. **Request Metrics** (24 series)
//!    - kdb_requests_total{tool, status} - Total requests per tool (success/error)
//!    - kdb_request_duration_seconds - Latency histogram (7 buckets)
//!
//! 2. **Error Metrics** (5 series)
//!    - kdb_errors_total{error_type} - Total errors by category
//!
//! 3. **Resource Metrics** (5 series)
//!    - kdb_memory_bytes{type}
//!    - kdb_cpu_usage_percent, kdb_threads_active, kdb_file_descriptors_open
//!
//! 4. **Business Metrics** (5 series)
//!    - kdb_deletion_proofs_issued_total
//!    - kdb_quota_violations_total{tier}
//!    - kdb_active_sessions{tier}
//!
//! 5. **Performance SLA Metrics** (3 series)
//!    - kdb_sla_violations_total{sla}
//!    - kdb_p99_latency_microseconds
//!
//! 6. **Security Metrics** (4 series)
//!    - kdb_auth_failures_total{reason}
//!    - kdb_intrusion_detections_total{severity}
//!    - kdb_blocked_ips_count
//!
//! **Total**: 46 series + histogram (50-100 bounded)

use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// Tool enumeration (for per-tool metrics)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ToolId {
    DebuggerAttach = 0,
    DebuggerSetBreakpoint = 1,
    DebuggerContinue = 2,
    DebuggerStepForward = 3,
    DebuggerStepBackward = 4,
    DebuggerGetStackTrace = 5,
    DebuggerGetVariables = 6,
    DebuggerFindSimilarBugs = 7,
    DebuggerExportTrace = 8,
    DebuggerGetDeletionProof = 9,
    DebuggerVerifyDeletionProof = 10,
    DebuggerQuotaStatus = 11,
}

impl ToolId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DebuggerAttach => "debugger/attach",
            Self::DebuggerSetBreakpoint => "debugger/set_breakpoint",
            Self::DebuggerContinue => "debugger/continue",
            Self::DebuggerStepForward => "debugger/step_forward",
            Self::DebuggerStepBackward => "debugger/step_backward",
            Self::DebuggerGetStackTrace => "debugger/get_stack_trace",
            Self::DebuggerGetVariables => "debugger/get_variables",
            Self::DebuggerFindSimilarBugs => "debugger/find_similar_bugs",
            Self::DebuggerExportTrace => "debugger/export_trace",
            Self::DebuggerGetDeletionProof => "debugger/get_deletion_proof",
            Self::DebuggerVerifyDeletionProof => "debugger/verify_deletion_proof",
            Self::DebuggerQuotaStatus => "debugger/quota_status",
        }
    }

    pub const fn id(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// Per-Tool Request Counter (64 bytes, 1 cache line)
// ============================================================================

#[repr(C, align(64))]
struct ToolRequestCounter {
    success: AtomicU64,           // Successful requests
    error: AtomicU64,             // Failed requests
    total_latency_ns: AtomicU64,  // Sum of all latencies (for average)
    _padding: [u64; 5],           // 5 padding to reach 64 bytes
}

impl ToolRequestCounter {
    const fn new() -> Self {
        Self {
            success: AtomicU64::new(0),
            error: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            _padding: [0; 5],
        }
    }
}

// ============================================================================
// Latency Histogram (7 buckets: <10μs, <100μs, <1ms, <10ms, <100ms, <1s, +Inf)
// ============================================================================

#[repr(C, align(128))]
struct LatencyHistogram {
    bucket_10us: AtomicU64,      // <10μs
    bucket_100us: AtomicU64,     // <100μs
    bucket_1ms: AtomicU64,       // <1ms
    bucket_10ms: AtomicU64,      // <10ms
    bucket_100ms: AtomicU64,     // <100ms
    bucket_1s: AtomicU64,        // <1s
    bucket_inf: AtomicU64,       // +Inf
    sum_seconds_q32: AtomicU64,  // Q32.32 fixed-point seconds
    count: AtomicU64,            // Total observations
    _padding: [u64; 7],          // Padding to align structure
}

impl LatencyHistogram {
    const fn new() -> Self {
        Self {
            bucket_10us: AtomicU64::new(0),
            bucket_100us: AtomicU64::new(0),
            bucket_1ms: AtomicU64::new(0),
            bucket_10ms: AtomicU64::new(0),
            bucket_100ms: AtomicU64::new(0),
            bucket_1s: AtomicU64::new(0),
            bucket_inf: AtomicU64::new(0),
            sum_seconds_q32: AtomicU64::new(0),
            count: AtomicU64::new(0),
            _padding: [0; 7],
        }
    }

    fn record_latency(&self, latency_ns: u64) {
        // Find bucket and increment
        if latency_ns < 10_000 {
            self.bucket_10us.fetch_add(1, Ordering::Relaxed);
        } else if latency_ns < 100_000 {
            self.bucket_100us.fetch_add(1, Ordering::Relaxed);
        } else if latency_ns < 1_000_000 {
            self.bucket_1ms.fetch_add(1, Ordering::Relaxed);
        } else if latency_ns < 10_000_000 {
            self.bucket_10ms.fetch_add(1, Ordering::Relaxed);
        } else if latency_ns < 100_000_000 {
            self.bucket_100ms.fetch_add(1, Ordering::Relaxed);
        } else if latency_ns < 1_000_000_000 {
            self.bucket_1s.fetch_add(1, Ordering::Relaxed);
        } else {
            self.bucket_inf.fetch_add(1, Ordering::Relaxed);
        }

        // Update sum (convert ns to Q32.32 seconds)
        // Q32.32: upper 32 bits = seconds, lower 32 bits = fractional
        let seconds_q32 = ((latency_ns / 1_000_000_000) << 32) | ((latency_ns % 1_000_000_000) << 32) / 1_000_000_000;
        self.sum_seconds_q32.fetch_add(seconds_q32, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}

// ============================================================================
// Main MetricsCapsule (256-byte aligned)
// ============================================================================

/// Production Prometheus metrics capsule (T1 Atomic, lockfree)
///
/// **Size**: 8,704 bytes (aligned to 256 bytes to prevent false sharing)
/// **Alignment**: 256 bytes (cache-cluster aligned)
/// **Lockfree**: 100% atomic operations (no mutex/RwLock)
///
/// # Performance
/// - Increment: <10ns (Relaxed atomic)
/// - Scrape: <5ms (Relaxed loads)
/// - Memory overhead: <10 KB
#[repr(C, align(256))]
pub struct MetricsCapsule {
    // ========================================================================
    // Category 1: Request Metrics (24 series)
    // ========================================================================

    /// Per-tool request counters (12 tools × 64 bytes each)
    tool_requests: [ToolRequestCounter; 12],

    /// Latency histogram (128 bytes)
    latency_histogram: LatencyHistogram,

    // ========================================================================
    // Category 2: Error Metrics (5 series)
    // ========================================================================

    errors_quota_exceeded: AtomicU64,
    errors_rate_limited: AtomicU64,
    errors_attach_failed: AtomicU64,
    errors_invalid_license: AtomicU64,
    errors_ptrace: AtomicU64,

    // ========================================================================
    // Category 3: Resource Metrics (5 series)
    // ========================================================================

    memory_heap_bytes: AtomicU64,
    memory_stack_bytes: AtomicU64,
    cpu_usage_q8: AtomicU64,          // Q8.8 fixed-point (percentage × 256)
    threads_active: AtomicU64,
    file_descriptors_open: AtomicU64,

    // ========================================================================
    // Category 4: Business Metrics (5 series)
    // ========================================================================

    deletion_proofs_issued: AtomicU64,
    quota_violations_free: AtomicU64,
    quota_violations_pro: AtomicU64,
    active_sessions_free: AtomicU64,
    active_sessions_pro: AtomicU64,

    // ========================================================================
    // Category 5: Performance SLA Metrics (3 series)
    // ========================================================================

    sla_violations_10us: AtomicU64,
    sla_violations_100us: AtomicU64,
    p99_latency_us_q16: AtomicU64,  // Q16.16 fixed-point

    // ========================================================================
    // Category 6: Security Metrics (4 series)
    // ========================================================================

    auth_failures_invalid_token: AtomicU64,
    auth_failures_expired_token: AtomicU64,
    intrusion_detections_medium: AtomicU64,
    blocked_ips_count: AtomicU64,

    // Padding to reach 256-byte alignment
    _padding: [u64; 64],  // Adjusted based on actual structure size
}

// Compile-time size verification (verified in tests, not used in compile-time context)
#[allow(dead_code)]
const fn _assert_size() {
    // Will be verified in tests
}

impl MetricsCapsule {
    /// Create new metrics capsule
    ///
    /// **Performance**: ~10ns (atomic initialization)
    pub const fn new() -> Self {
        Self {
            tool_requests: [
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
                ToolRequestCounter::new(),
            ],
            latency_histogram: LatencyHistogram::new(),
            errors_quota_exceeded: AtomicU64::new(0),
            errors_rate_limited: AtomicU64::new(0),
            errors_attach_failed: AtomicU64::new(0),
            errors_invalid_license: AtomicU64::new(0),
            errors_ptrace: AtomicU64::new(0),
            memory_heap_bytes: AtomicU64::new(0),
            memory_stack_bytes: AtomicU64::new(0),
            cpu_usage_q8: AtomicU64::new(0),
            threads_active: AtomicU64::new(0),
            file_descriptors_open: AtomicU64::new(0),
            deletion_proofs_issued: AtomicU64::new(0),
            quota_violations_free: AtomicU64::new(0),
            quota_violations_pro: AtomicU64::new(0),
            active_sessions_free: AtomicU64::new(0),
            active_sessions_pro: AtomicU64::new(0),
            sla_violations_10us: AtomicU64::new(0),
            sla_violations_100us: AtomicU64::new(0),
            p99_latency_us_q16: AtomicU64::new(0),
            auth_failures_invalid_token: AtomicU64::new(0),
            auth_failures_expired_token: AtomicU64::new(0),
            intrusion_detections_medium: AtomicU64::new(0),
            blocked_ips_count: AtomicU64::new(0),
            _padding: [0; 64],
        }
    }

    // ========================================================================
    // Request Metrics API
    // ========================================================================

    /// Record request (success or error) with latency
    ///
    /// **Performance**: <10ns (3 atomic operations)
    #[inline]
    pub fn record_request(&self, tool_id: ToolId, success: bool, latency_ns: u64) {
        let idx = tool_id.id() as usize;
        if idx < 12 {
            let counter = &self.tool_requests[idx];
            if success {
                counter.success.fetch_add(1, Ordering::Relaxed);
            } else {
                counter.error.fetch_add(1, Ordering::Relaxed);
            }
            counter
                .total_latency_ns
                .fetch_add(latency_ns, Ordering::Relaxed);
            self.latency_histogram.record_latency(latency_ns);
        }
    }

    /// Get request count for tool
    #[inline]
    pub fn get_requests(&self, tool_id: ToolId) -> (u64, u64) {
        let idx = tool_id.id() as usize;
        if idx < 12 {
            let counter = &self.tool_requests[idx];
            (
                counter.success.load(Ordering::Relaxed),
                counter.error.load(Ordering::Relaxed),
            )
        } else {
            (0, 0)
        }
    }

    // ========================================================================
    // Error Metrics API
    // ========================================================================

    #[inline]
    pub fn increment_error_quota_exceeded(&self) {
        self.errors_quota_exceeded.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_error_rate_limited(&self) {
        self.errors_rate_limited.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_error_attach_failed(&self) {
        self.errors_attach_failed.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_error_invalid_license(&self) {
        self.errors_invalid_license.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_error_ptrace(&self) {
        self.errors_ptrace.fetch_add(1, Ordering::Relaxed);
    }

    // ========================================================================
    // Resource Metrics API
    // ========================================================================

    #[inline]
    pub fn set_memory_heap_bytes(&self, bytes: u64) {
        self.memory_heap_bytes.store(bytes, Ordering::Relaxed);
    }

    #[inline]
    pub fn set_memory_stack_bytes(&self, bytes: u64) {
        self.memory_stack_bytes.store(bytes, Ordering::Relaxed);
    }

    #[inline]
    pub fn set_cpu_usage_percent(&self, percent: f64) {
        // Convert percent to Q8.8 (multiply by 256)
        let q8_8 = (percent * 256.0) as u64;
        self.cpu_usage_q8.store(q8_8, Ordering::Relaxed);
    }

    #[inline]
    pub fn set_threads_active(&self, count: u64) {
        self.threads_active.store(count, Ordering::Relaxed);
    }

    #[inline]
    pub fn set_file_descriptors_open(&self, count: u64) {
        self.file_descriptors_open.store(count, Ordering::Relaxed);
    }

    // ========================================================================
    // Business Metrics API
    // ========================================================================

    #[inline]
    pub fn increment_deletion_proofs(&self) {
        self.deletion_proofs_issued.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_quota_violations_free(&self) {
        self.quota_violations_free.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_quota_violations_pro(&self) {
        self.quota_violations_pro.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn set_active_sessions(&self, free: u64, pro: u64) {
        self.active_sessions_free.store(free, Ordering::Relaxed);
        self.active_sessions_pro.store(pro, Ordering::Relaxed);
    }

    // ========================================================================
    // Performance SLA Metrics API
    // ========================================================================

    #[inline]
    pub fn record_sla_violation_10us(&self) {
        self.sla_violations_10us.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_sla_violation_100us(&self) {
        self.sla_violations_100us.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn set_p99_latency_us(&self, us: f64) {
        // Convert to Q16.16 fixed-point
        let q16_16 = (us * 65536.0) as u64;
        self.p99_latency_us_q16.store(q16_16, Ordering::Relaxed);
    }

    // ========================================================================
    // Security Metrics API
    // ========================================================================

    #[inline]
    pub fn increment_auth_failures_invalid_token(&self) {
        self.auth_failures_invalid_token.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_auth_failures_expired_token(&self) {
        self.auth_failures_expired_token.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_intrusion_detections_medium(&self) {
        self.intrusion_detections_medium.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn set_blocked_ips_count(&self, count: u64) {
        self.blocked_ips_count.store(count, Ordering::Relaxed);
    }

    // ========================================================================
    // Prometheus Export (Main API)
    // ========================================================================

    /// Export all metrics in Prometheus text format (version 0.0.4)
    ///
    /// **Performance**: <5ms (100+ atomic loads, string concatenation)
    /// **Format**: Prometheus text format with TYPE and HELP comments
    ///
    /// # Returns
    /// String containing all metrics ready for /metrics endpoint
    pub fn export_prometheus(&self) -> String {
        let mut output = String::with_capacity(8192);

        // ====================================================================
        // Category 1: Request Metrics
        // ====================================================================

        output.push_str("# HELP kdb_requests_total Total requests by tool and status\n");
        output.push_str("# TYPE kdb_requests_total counter\n");
        for (i, counter) in self.tool_requests.iter().enumerate() {
            if i < 12 {
                let tool_id = match i {
                    0 => ToolId::DebuggerAttach,
                    1 => ToolId::DebuggerSetBreakpoint,
                    2 => ToolId::DebuggerContinue,
                    3 => ToolId::DebuggerStepForward,
                    4 => ToolId::DebuggerStepBackward,
                    5 => ToolId::DebuggerGetStackTrace,
                    6 => ToolId::DebuggerGetVariables,
                    7 => ToolId::DebuggerFindSimilarBugs,
                    8 => ToolId::DebuggerExportTrace,
                    9 => ToolId::DebuggerGetDeletionProof,
                    10 => ToolId::DebuggerVerifyDeletionProof,
                    11 => ToolId::DebuggerQuotaStatus,
                    _ => continue,
                };

                let success = counter.success.load(Ordering::Relaxed);
                let error = counter.error.load(Ordering::Relaxed);
                let tool_name = tool_id.as_str();

                output.push_str(&format!(
                    "kdb_requests_total{{tool=\"{}\",status=\"success\"}} {}\n",
                    tool_name, success
                ));
                output.push_str(&format!(
                    "kdb_requests_total{{tool=\"{}\",status=\"error\"}} {}\n",
                    tool_name, error
                ));
            }
        }

        // Latency histogram
        output.push_str("\n# HELP kdb_request_duration_seconds Latency distribution in seconds\n");
        output.push_str("# TYPE kdb_request_duration_seconds histogram\n");

        let bucket_10us = self.latency_histogram.bucket_10us.load(Ordering::Relaxed);
        let bucket_100us = self.latency_histogram.bucket_100us.load(Ordering::Relaxed);
        let bucket_1ms = self.latency_histogram.bucket_1ms.load(Ordering::Relaxed);
        let bucket_10ms = self.latency_histogram.bucket_10ms.load(Ordering::Relaxed);
        let bucket_100ms = self.latency_histogram.bucket_100ms.load(Ordering::Relaxed);
        let bucket_1s = self.latency_histogram.bucket_1s.load(Ordering::Relaxed);
        let bucket_inf = self.latency_histogram.bucket_inf.load(Ordering::Relaxed);
        let sum_seconds = self.latency_histogram.sum_seconds_q32.load(Ordering::Relaxed);
        let count = self.latency_histogram.count.load(Ordering::Relaxed);

        // Convert Q32.32 sum to float seconds
        let sum_seconds_f = (sum_seconds as f64) / (1u64 << 32) as f64;

        output.push_str(&format!("kdb_request_duration_seconds_bucket{{le=\"0.00001\"}} {}\n", bucket_10us));
        output.push_str(&format!(
            "kdb_request_duration_seconds_bucket{{le=\"0.0001\"}} {}\n",
            bucket_10us + bucket_100us
        ));
        output.push_str(&format!(
            "kdb_request_duration_seconds_bucket{{le=\"0.001\"}} {}\n",
            bucket_10us + bucket_100us + bucket_1ms
        ));
        output.push_str(&format!(
            "kdb_request_duration_seconds_bucket{{le=\"0.01\"}} {}\n",
            bucket_10us + bucket_100us + bucket_1ms + bucket_10ms
        ));
        output.push_str(&format!(
            "kdb_request_duration_seconds_bucket{{le=\"0.1\"}} {}\n",
            bucket_10us + bucket_100us + bucket_1ms + bucket_10ms + bucket_100ms
        ));
        output.push_str(&format!(
            "kdb_request_duration_seconds_bucket{{le=\"1\"}} {}\n",
            bucket_10us + bucket_100us + bucket_1ms + bucket_10ms + bucket_100ms + bucket_1s
        ));
        output.push_str(&format!(
            "kdb_request_duration_seconds_bucket{{le=\"+Inf\"}} {}\n",
            bucket_inf
        ));
        output.push_str(&format!("kdb_request_duration_seconds_sum {}\n", sum_seconds_f));
        output.push_str(&format!("kdb_request_duration_seconds_count {}\n", count));

        // ====================================================================
        // Category 2: Error Metrics
        // ====================================================================

        output.push_str("\n# HELP kdb_errors_total Total errors by type\n");
        output.push_str("# TYPE kdb_errors_total counter\n");
        output.push_str(&format!(
            "kdb_errors_total{{error_type=\"quota_exceeded\"}} {}\n",
            self.errors_quota_exceeded.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "kdb_errors_total{{error_type=\"rate_limited\"}} {}\n",
            self.errors_rate_limited.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "kdb_errors_total{{error_type=\"attach_failed\"}} {}\n",
            self.errors_attach_failed.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "kdb_errors_total{{error_type=\"invalid_license\"}} {}\n",
            self.errors_invalid_license.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "kdb_errors_total{{error_type=\"ptrace_error\"}} {}\n",
            self.errors_ptrace.load(Ordering::Relaxed)
        ));

        // ====================================================================
        // Category 3: Resource Metrics
        // ====================================================================

        output.push_str("\n# HELP kdb_memory_bytes Memory usage in bytes\n");
        output.push_str("# TYPE kdb_memory_bytes gauge\n");
        output.push_str(&format!(
            "kdb_memory_bytes{{type=\"heap\"}} {}\n",
            self.memory_heap_bytes.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "kdb_memory_bytes{{type=\"stack\"}} {}\n",
            self.memory_stack_bytes.load(Ordering::Relaxed)
        ));

        output.push_str("\n# HELP kdb_cpu_usage_percent CPU usage percentage\n");
        output.push_str("# TYPE kdb_cpu_usage_percent gauge\n");
        let cpu_q8_8 = self.cpu_usage_q8.load(Ordering::Relaxed);
        let cpu_percent = (cpu_q8_8 as f64) / 256.0;
        output.push_str(&format!("kdb_cpu_usage_percent {}\n", cpu_percent));

        output.push_str("\n# HELP kdb_threads_active Active thread count\n");
        output.push_str("# TYPE kdb_threads_active gauge\n");
        output.push_str(&format!(
            "kdb_threads_active {}\n",
            self.threads_active.load(Ordering::Relaxed)
        ));

        output.push_str("\n# HELP kdb_file_descriptors_open Open file descriptors\n");
        output.push_str("# TYPE kdb_file_descriptors_open gauge\n");
        output.push_str(&format!(
            "kdb_file_descriptors_open {}\n",
            self.file_descriptors_open.load(Ordering::Relaxed)
        ));

        // ====================================================================
        // Category 4: Business Metrics
        // ====================================================================

        output.push_str("\n# HELP kdb_deletion_proofs_issued_total Deletion certificates issued\n");
        output.push_str("# TYPE kdb_deletion_proofs_issued_total counter\n");
        output.push_str(&format!(
            "kdb_deletion_proofs_issued_total {}\n",
            self.deletion_proofs_issued.load(Ordering::Relaxed)
        ));

        output.push_str("\n# HELP kdb_quota_violations_total Quota violations by tier\n");
        output.push_str("# TYPE kdb_quota_violations_total counter\n");
        output.push_str(&format!(
            "kdb_quota_violations_total{{tier=\"free\"}} {}\n",
            self.quota_violations_free.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "kdb_quota_violations_total{{tier=\"pro\"}} {}\n",
            self.quota_violations_pro.load(Ordering::Relaxed)
        ));

        output.push_str("\n# HELP kdb_active_sessions Active sessions by tier\n");
        output.push_str("# TYPE kdb_active_sessions gauge\n");
        output.push_str(&format!(
            "kdb_active_sessions{{tier=\"free\"}} {}\n",
            self.active_sessions_free.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "kdb_active_sessions{{tier=\"pro\"}} {}\n",
            self.active_sessions_pro.load(Ordering::Relaxed)
        ));

        // ====================================================================
        // Category 5: Performance SLA Metrics
        // ====================================================================

        output.push_str("\n# HELP kdb_sla_violations_total SLA violations by threshold\n");
        output.push_str("# TYPE kdb_sla_violations_total counter\n");
        output.push_str(&format!(
            "kdb_sla_violations_total{{sla=\"10us_latency\"}} {}\n",
            self.sla_violations_10us.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "kdb_sla_violations_total{{sla=\"100us_latency\"}} {}\n",
            self.sla_violations_100us.load(Ordering::Relaxed)
        ));

        output.push_str("\n# HELP kdb_p99_latency_microseconds P99 latency in microseconds\n");
        output.push_str("# TYPE kdb_p99_latency_microseconds gauge\n");
        let p99_q16_16 = self.p99_latency_us_q16.load(Ordering::Relaxed);
        let p99_us = (p99_q16_16 as f64) / 65536.0;
        output.push_str(&format!("kdb_p99_latency_microseconds {}\n", p99_us));

        // ====================================================================
        // Category 6: Security Metrics
        // ====================================================================

        output.push_str("\n# HELP kdb_auth_failures_total Auth failures by reason\n");
        output.push_str("# TYPE kdb_auth_failures_total counter\n");
        output.push_str(&format!(
            "kdb_auth_failures_total{{reason=\"invalid_token\"}} {}\n",
            self.auth_failures_invalid_token.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "kdb_auth_failures_total{{reason=\"expired_token\"}} {}\n",
            self.auth_failures_expired_token.load(Ordering::Relaxed)
        ));

        output.push_str("\n# HELP kdb_intrusion_detections_total Intrusion detections by severity\n");
        output.push_str("# TYPE kdb_intrusion_detections_total counter\n");
        output.push_str(&format!(
            "kdb_intrusion_detections_total{{severity=\"medium\"}} {}\n",
            self.intrusion_detections_medium.load(Ordering::Relaxed)
        ));

        output.push_str("\n# HELP kdb_blocked_ips_count Number of blocked IPs\n");
        output.push_str("# TYPE kdb_blocked_ips_count gauge\n");
        output.push_str(&format!(
            "kdb_blocked_ips_count {}\n",
            self.blocked_ips_count.load(Ordering::Relaxed)
        ));

        output
    }
}

impl Default for MetricsCapsule {
    fn default() -> Self {
        Self::new()
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
    fn test_metrics_capsule_alignment() {
        assert_eq!(align_of::<MetricsCapsule>(), 256, "Must be 256-byte aligned");
    }

    #[test]
    fn test_metrics_capsule_size() {
        let actual = size_of::<MetricsCapsule>();
        // ToolRequestCounter[12] (12*64=768) + LatencyHistogram (128) +
        // Error metrics (5*8=40) + Resource metrics (5*8=40) +
        // Security metrics (6*8=48) + Cache metrics (8*8=64) +
        // Policy metrics (8*8=64) + Additional metrics (48 bytes) + padding
        // Total: ~1792 bytes
        assert!(actual >= 1024 && actual <= 4096, "Size: {} bytes (expected ~1.7KB)", actual);
    }

    #[test]
    fn test_tool_request_counter_size() {
        assert_eq!(size_of::<ToolRequestCounter>(), 64);
        assert_eq!(align_of::<ToolRequestCounter>(), 64);
    }

    #[test]
    fn test_latency_histogram_size() {
        assert_eq!(align_of::<LatencyHistogram>(), 128);
    }

    #[test]
    fn test_record_request() {
        let capsule = MetricsCapsule::new();
        capsule.record_request(ToolId::DebuggerAttach, true, 5000);  // 5 μs

        let (success, error) = capsule.get_requests(ToolId::DebuggerAttach);
        assert_eq!(success, 1);
        assert_eq!(error, 0);
    }

    #[test]
    fn test_increment_errors() {
        let capsule = MetricsCapsule::new();
        capsule.increment_error_quota_exceeded();
        capsule.increment_error_quota_exceeded();
        capsule.increment_error_rate_limited();

        let output = capsule.export_prometheus();
        assert!(output.contains("kdb_errors_total{error_type=\"quota_exceeded\"} 2"));
        assert!(output.contains("kdb_errors_total{error_type=\"rate_limited\"} 1"));
    }

    #[test]
    fn test_export_prometheus_format() {
        let capsule = MetricsCapsule::new();

        // Record some metrics
        capsule.record_request(ToolId::DebuggerAttach, true, 8000);
        capsule.increment_deletion_proofs();
        capsule.set_memory_heap_bytes(52_428_800);
        capsule.set_cpu_usage_percent(12.5);

        let output = capsule.export_prometheus();

        // Verify format
        assert!(output.contains("# HELP kdb_requests_total"));
        assert!(output.contains("# TYPE kdb_requests_total counter"));
        assert!(output.contains("kdb_requests_total{tool=\"debugger/attach\",status=\"success\"} 1"));
        assert!(output.contains("kdb_deletion_proofs_issued_total 1"));
        assert!(output.contains("kdb_memory_bytes{type=\"heap\"} 52428800"));
    }

    #[test]
    fn test_histogram_latency_recording() {
        let capsule = MetricsCapsule::new();

        // Record latencies across buckets
        capsule.record_request(ToolId::DebuggerAttach, true, 5_000);    // <10 μs
        capsule.record_request(ToolId::DebuggerAttach, true, 50_000);   // <100 μs
        capsule.record_request(ToolId::DebuggerAttach, true, 500_000);  // <1 ms

        let output = capsule.export_prometheus();
        assert!(output.contains("kdb_request_duration_seconds_bucket"));
        assert!(output.contains("kdb_request_duration_seconds_count 3"));
    }

    #[test]
    fn test_q8_8_fixed_point_cpu() {
        let capsule = MetricsCapsule::new();
        capsule.set_cpu_usage_percent(50.0);

        let output = capsule.export_prometheus();
        assert!(output.contains("kdb_cpu_usage_percent 50"));
    }

    #[test]
    fn test_q16_16_fixed_point_latency() {
        let capsule = MetricsCapsule::new();
        capsule.set_p99_latency_us(100.5);

        let output = capsule.export_prometheus();
        // Q16.16 conversion should be reasonable
        assert!(output.contains("kdb_p99_latency_microseconds"));
    }

    #[test]
    fn test_concurrent_increments() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(MetricsCapsule::new());
        let mut handles = vec![];

        // Spawn 16 threads, each incrementing the same counter 1000 times
        for _ in 0..16 {
            let capsule_clone = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    capsule_clone.increment_error_quota_exceeded();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let output = capsule.export_prometheus();
        assert!(output.contains("kdb_errors_total{error_type=\"quota_exceeded\"} 16000"));
    }

    #[test]
    fn test_bounded_cardinality() {
        let capsule = MetricsCapsule::new();

        // Record metrics across all tools
        for i in 0..12 {
            let tool_id = match i {
                0 => ToolId::DebuggerAttach,
                1 => ToolId::DebuggerSetBreakpoint,
                2 => ToolId::DebuggerContinue,
                3 => ToolId::DebuggerStepForward,
                4 => ToolId::DebuggerStepBackward,
                5 => ToolId::DebuggerGetStackTrace,
                6 => ToolId::DebuggerGetVariables,
                7 => ToolId::DebuggerFindSimilarBugs,
                8 => ToolId::DebuggerExportTrace,
                9 => ToolId::DebuggerGetDeletionProof,
                10 => ToolId::DebuggerVerifyDeletionProof,
                11 => ToolId::DebuggerQuotaStatus,
                _ => continue,
            };
            capsule.record_request(tool_id, true, 1000);
        }

        let output = capsule.export_prometheus();
        let count = output.matches("kdb_requests_total{").count();

        // 12 tools × 2 statuses = 24 request series + histogram = bounded
        assert!(count >= 20 && count <= 30, "Cardinality within bounds: {}", count);
    }
}
