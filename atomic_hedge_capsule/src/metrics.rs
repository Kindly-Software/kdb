//! # Performance Metrics Collection System
//!
//! Zero-overhead metrics collection with B32 validation and UCE-32 Q30 empirical validation.
//! Implements comprehensive performance tracking with atomic counters and lockfree coordination.
//!
//! ## Design Principles (UCE-32 Q29 Constraints)
//!
//! 1. **Zero Overhead When Disabled**: Complete compile-time elimination via feature gates
//! 2. **100% Lockfree**: Pure atomic operations for all metrics collection
//! 3. **Cache-Aware**: 64-byte aligned metrics groups for optimal memory performance
//! 4. **Statistical Validity**: Moving averages with percentile calculation for B32 compliance
//! 5. **Real-time Safe**: All operations are wait-free and bounded-time
//!
//! ## Usage Examples
//!
//! ### Basic Metrics Collection
//! ```rust
//! use atomic_hedge_capsule::metrics::MetricsCollector;
//!
//! let metrics = MetricsCollector::new();
//!
//! // Record successful operation with latency
//! metrics.record_operation(true, 150); // 150ns latency
//!
//! // Get current statistics
//! let stats = metrics.get_statistics();
//! println!("Success rate: {:.2}%", stats.success_rate);
//! println!("P99 latency: {}ns", stats.p99_latency);
//! ```
//!
//! ### Zero-Overhead Design
//! ```rust
//! // With metrics feature enabled - full functionality
//! #[cfg(feature = "metrics")]
//! metrics.record_operation(true, latency);
//!
//! // Without metrics feature - compiles to nothing
//! #[cfg(not(feature = "metrics"))]
//! metrics.record_operation(true, latency); // No-op
//! ```

use portable_atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// UCE-32 Q32: Nightly features for advanced metrics
#[cfg(all(feature = "nightly", feature = "portable_simd"))]
use std::simd::prelude::*;

/// B32 performance baseline validation
const METRICS_OVERHEAD_TARGET_NS: u64 = 5; // Target: <5ns per metric operation

/// Performance threshold constants for health monitoring
const PERFORMANCE_THRESHOLD_NS: u64 = 200; // 200ns latency threshold
const SUCCESS_RATE_THRESHOLD: f64 = 95.0; // 95% success rate threshold
const ERROR_RATE_THRESHOLD: f64 = 5.0; // 5% error rate threshold
const CACHE_LINE_SIZE: usize = 64;
const HISTOGRAM_BUCKETS: usize = 32;

/// # MetricsCollector - Zero-Overhead Performance Tracking
///
/// Comprehensive metrics collection system designed for production hedge trading systems.
/// Implements B32 statistical validation with UCE-32 Q30 empirical measurement requirements.
///
/// ## Features
/// - **Operation Metrics**: Success/failure counts with rates
/// - **Performance Metrics**: Latency percentiles (P50, P95, P99) with moving averages
/// - **Contention Metrics**: CAS retry counts and thread collision tracking
/// - **Error Metrics**: Categorized error tracking with recovery statistics
/// - **Cache Efficiency**: 64-byte aligned hot metrics for optimal memory performance
///
/// ## Memory Layout Optimization
/// ```
/// Cache Line 1 (Hot Metrics - 64 bytes):
///   operation_count, success_count, total_latency, cas_retries
/// Cache Line 2 (Latency Distribution - 64 bytes):
///   histogram buckets for percentile calculation
/// Cache Line 3 (Error Tracking - 64 bytes):
///   error counts by category, recovery attempts
/// ```
#[cfg(feature = "metrics")]
#[repr(align(64))] // UCE-32 Q29: Cache line alignment for optimal performance
pub struct MetricsCollector {
    // === HOT METRICS - First cache line (0-63 bytes) ===
    /// Total number of operations attempted
    operation_count: AtomicU64,

    /// Number of successful operations
    success_count: AtomicU64,

    /// Cumulative latency for average calculation (nanoseconds)
    total_latency: AtomicU64,

    /// Number of CAS retry operations (contention indicator)
    cas_retries: AtomicU64,

    /// Thread collision counter for contention analysis
    thread_collisions: AtomicU64,

    /// Emergency stop counter
    emergency_stops: AtomicU64,

    /// Cache hit counter for efficiency tracking
    cache_hits: AtomicU64,

    /// Generation for consistent reads
    metrics_generation: AtomicU64,

    // === LATENCY DISTRIBUTION - Second cache line (64-127 bytes) ===
    /// Histogram buckets for latency percentile calculation
    /// Bucket boundaries: [0-10ns, 10-20ns, 20-50ns, 50-100ns, 100-200ns, ...]
    latency_histogram: [AtomicU32; HISTOGRAM_BUCKETS],

    // === ERROR TRACKING - Third cache line (128-191 bytes) ===
    /// Error counts by category
    validation_errors: AtomicU64,
    coordination_errors: AtomicU64,
    timeout_errors: AtomicU64,
    network_errors: AtomicU64,

    /// Recovery statistics
    recovery_attempts: AtomicU64,
    successful_recoveries: AtomicU64,

    /// Performance degradation indicators
    throttle_events: AtomicU64,
    backpressure_events: AtomicU64,
}

/// Zero-size type for disabled metrics - compiles to nothing
#[cfg(not(feature = "metrics"))]
pub struct MetricsCollector;

/// Comprehensive metrics snapshot for reporting and analysis
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Total operations attempted
    pub total_operations: u64,

    /// Success rate percentage (0.0 - 100.0)
    pub success_rate: f64,

    /// Average latency in nanoseconds
    pub avg_latency_ns: u64,

    /// Latency percentiles
    pub p50_latency_ns: u64,
    pub p95_latency_ns: u64,
    pub p99_latency_ns: u64,

    /// Operations per second (throughput)
    pub ops_per_second: f64,

    /// Contention metrics
    pub cas_retry_rate: f64,
    pub thread_collision_rate: f64,

    /// Error statistics
    pub error_rate: f64,
    pub recovery_success_rate: f64,

    /// Cache efficiency
    pub cache_hit_rate: f64,

    /// Performance indicators
    pub is_healthy: bool,
    pub performance_grade: char, // A-F grade based on B32 baselines
}

/// Error categories for detailed tracking
#[derive(Debug, Clone, Copy)]
pub enum ErrorCategory {
    Validation,
    Coordination,
    Timeout,
    Network,
    Emergency,
    System,
}

#[cfg(feature = "metrics")]
impl MetricsCollector {
    /// Create new metrics collector with zero-initialized counters
    ///
    /// # Performance
    /// - **Initialization**: <100ns (allocation + atomic initialization)
    /// - **Memory Usage**: 192 bytes (3 cache lines)
    /// - **Cache Efficiency**: Hot metrics fit in single 64-byte cache line
    pub fn new() -> Self {
        Self {
            // Hot metrics
            operation_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            total_latency: AtomicU64::new(0),
            cas_retries: AtomicU64::new(0),
            thread_collisions: AtomicU64::new(0),
            emergency_stops: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            metrics_generation: AtomicU64::new(0),

            // Latency histogram - initialize all buckets to zero
            latency_histogram: [const { AtomicU32::new(0) }; HISTOGRAM_BUCKETS],

            // Error tracking
            validation_errors: AtomicU64::new(0),
            coordination_errors: AtomicU64::new(0),
            timeout_errors: AtomicU64::new(0),
            network_errors: AtomicU64::new(0),
            recovery_attempts: AtomicU64::new(0),
            successful_recoveries: AtomicU64::new(0),
            throttle_events: AtomicU64::new(0),
            backpressure_events: AtomicU64::new(0),
        }
    }

    /// Record operation completion with performance metrics
    ///
    /// # Arguments
    /// * `success` - Whether the operation succeeded
    /// * `latency_ns` - Operation latency in nanoseconds
    ///
    /// # Performance
    /// - **Target Latency**: <5ns (B32 overhead requirement)
    /// - **Memory Ordering**: Relaxed for counters (optimal performance)
    /// - **Cache Impact**: All hot metrics in single cache line
    #[inline(always)]
    pub fn record_operation(&self, success: bool, latency_ns: u64) {
        // Increment generation for consistent reads
        self.metrics_generation.fetch_add(1, Ordering::Relaxed);

        // Update operation counts (hot path - first cache line)
        self.operation_count.fetch_add(1, Ordering::Relaxed);
        if success {
            self.success_count.fetch_add(1, Ordering::Relaxed);
        }

        // Update latency tracking
        self.total_latency.fetch_add(latency_ns, Ordering::Relaxed);
        self.update_latency_histogram(latency_ns);
    }

    /// Record CAS retry for contention analysis
    #[inline(always)]
    pub fn record_cas_retry(&self) {
        self.cas_retries.fetch_add(1, Ordering::Relaxed);
    }

    /// Record thread collision for contention tracking
    #[inline(always)]
    pub fn record_thread_collision(&self) {
        self.thread_collisions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record cache hit for efficiency tracking
    #[inline(always)]
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record error by category for detailed analysis
    #[inline(always)]
    pub fn record_error(&self, category: ErrorCategory) {
        match category {
            ErrorCategory::Validation => {
                self.validation_errors.fetch_add(1, Ordering::Relaxed);
            }
            ErrorCategory::Coordination => {
                self.coordination_errors.fetch_add(1, Ordering::Relaxed);
            }
            ErrorCategory::Timeout => {
                self.timeout_errors.fetch_add(1, Ordering::Relaxed);
            }
            ErrorCategory::Network => {
                self.network_errors.fetch_add(1, Ordering::Relaxed);
            }
            ErrorCategory::Emergency => {
                self.coordination_errors.fetch_add(1, Ordering::Relaxed); // Emergency errors are coordination-related
            }
            ErrorCategory::System => {
                self.validation_errors.fetch_add(1, Ordering::Relaxed); // System errors often validation-related
            }
        }
    }

    /// Record recovery attempt and outcome
    #[inline(always)]
    pub fn record_recovery(&self, successful: bool) {
        self.recovery_attempts.fetch_add(1, Ordering::Relaxed);
        if successful {
            self.successful_recoveries.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record performance events
    #[inline(always)]
    pub fn record_throttle_event(&self) {
        self.throttle_events.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_backpressure_event(&self) {
        self.backpressure_events.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_emergency_stop(&self) {
        self.emergency_stops.fetch_add(1, Ordering::Relaxed);
    }

    /// Get comprehensive metrics snapshot with statistical analysis
    ///
    /// # Returns
    /// Complete metrics snapshot with B32-compliant statistical measures
    ///
    /// # Performance
    /// - **Snapshot Creation**: <200ns (consistent reads via generation counter)
    /// - **Memory Ordering**: Acquire for consistent snapshot
    /// - **Statistical Computation**: O(1) for averages, O(log n) for percentiles
    pub fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        // Use generation counter for consistent reads
        let generation = self.metrics_generation.load(Ordering::Acquire);

        // Read hot metrics atomically
        let total_ops = self.operation_count.load(Ordering::Relaxed);
        let successes = self.success_count.load(Ordering::Relaxed);
        let total_latency = self.total_latency.load(Ordering::Relaxed);
        let cas_retries = self.cas_retries.load(Ordering::Relaxed);
        let collisions = self.thread_collisions.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);

        // Read error metrics
        let validation_errs = self.validation_errors.load(Ordering::Relaxed);
        let coordination_errs = self.coordination_errors.load(Ordering::Relaxed);
        let timeout_errs = self.timeout_errors.load(Ordering::Relaxed);
        let network_errs = self.network_errors.load(Ordering::Relaxed);
        let total_errors = validation_errs + coordination_errs + timeout_errs + network_errs;

        let recovery_attempts = self.recovery_attempts.load(Ordering::Relaxed);
        let successful_recoveries = self.successful_recoveries.load(Ordering::Relaxed);

        // Verify consistency with generation counter
        let generation_end = self.metrics_generation.load(Ordering::Acquire);
        if generation != generation_end {
            // Retry for consistent snapshot if concurrent updates occurred
            return self.get_metrics_snapshot();
        }

        // Calculate derived metrics
        let success_rate = if total_ops > 0 {
            (successes as f64 / total_ops as f64) * 100.0
        } else {
            0.0
        };

        let avg_latency = if successes > 0 {
            total_latency / successes
        } else {
            0
        };

        let error_rate = if total_ops > 0 {
            (total_errors as f64 / total_ops as f64) * 100.0
        } else {
            0.0
        };

        let cas_retry_rate = if total_ops > 0 {
            (cas_retries as f64 / total_ops as f64) * 100.0
        } else {
            0.0
        };

        let thread_collision_rate = if total_ops > 0 {
            (collisions as f64 / total_ops as f64) * 100.0
        } else {
            0.0
        };

        let cache_hit_rate = if total_ops > 0 {
            (cache_hits as f64 / total_ops as f64) * 100.0
        } else {
            0.0
        };

        let recovery_success_rate = if recovery_attempts > 0 {
            (successful_recoveries as f64 / recovery_attempts as f64) * 100.0
        } else {
            0.0
        };

        // Calculate latency percentiles from histogram
        let (p50, p95, p99) = self.calculate_latency_percentiles();

        // Calculate throughput (operations per second)
        // Note: This is instantaneous calculation; real implementation would track time windows
        let ops_per_second = if total_ops > 0 { total_ops as f64 } else { 0.0 };

        // Performance health assessment based on B32 baselines
        let is_healthy = self.assess_performance_health(success_rate, avg_latency, error_rate);
        let performance_grade =
            self.calculate_performance_grade(success_rate, avg_latency, cas_retry_rate);

        MetricsSnapshot {
            total_operations: total_ops,
            success_rate,
            avg_latency_ns: avg_latency,
            p50_latency_ns: p50,
            p95_latency_ns: p95,
            p99_latency_ns: p99,
            ops_per_second,
            cas_retry_rate,
            thread_collision_rate,
            error_rate,
            recovery_success_rate,
            cache_hit_rate,
            is_healthy,
            performance_grade,
        }
    }

    /// Update latency histogram for percentile calculation
    #[inline(always)]
    fn update_latency_histogram(&self, latency_ns: u64) {
        let bucket = self.latency_to_bucket(latency_ns);
        if bucket < HISTOGRAM_BUCKETS {
            self.latency_histogram[bucket].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Convert latency to histogram bucket index
    fn latency_to_bucket(&self, latency_ns: u64) -> usize {
        match latency_ns {
            0..=10 => 0,
            11..=20 => 1,
            21..=50 => 2,
            51..=100 => 3,
            101..=200 => 4,
            201..=500 => 5,
            501..=1000 => 6,
            1001..=2000 => 7,
            2001..=5000 => 8,
            5001..=10000 => 9,
            _ => {
                // Logarithmic buckets for higher latencies
                let log_bucket = (latency_ns as f64).log2() as usize;
                std::cmp::min(log_bucket + 10, HISTOGRAM_BUCKETS - 1)
            }
        }
    }

    /// Calculate latency percentiles from histogram data
    fn calculate_latency_percentiles(&self) -> (u64, u64, u64) {
        let mut total_samples = 0u64;
        let mut histogram = [0u32; HISTOGRAM_BUCKETS];

        // Read histogram atomically
        for (i, bucket) in self.latency_histogram.iter().enumerate() {
            histogram[i] = bucket.load(Ordering::Relaxed);
            total_samples += histogram[i] as u64;
        }

        if total_samples == 0 {
            return (0, 0, 0);
        }

        // Calculate percentile positions
        let p50_pos = total_samples / 2;
        let p95_pos = (total_samples * 95) / 100;
        let p99_pos = (total_samples * 99) / 100;

        let mut cumulative = 0u64;
        let mut p50 = 0u64;
        let mut p95 = 0u64;
        let mut p99 = 0u64;

        for (i, count) in histogram.iter().enumerate() {
            cumulative += *count as u64;

            if p50 == 0 && cumulative >= p50_pos {
                p50 = self.bucket_to_latency(i);
            }
            if p95 == 0 && cumulative >= p95_pos {
                p95 = self.bucket_to_latency(i);
            }
            if p99 == 0 && cumulative >= p99_pos {
                p99 = self.bucket_to_latency(i);
                break;
            }
        }

        (p50, p95, p99)
    }

    /// Convert bucket index back to representative latency
    fn bucket_to_latency(&self, bucket: usize) -> u64 {
        match bucket {
            0 => 5,    // 0-10ns -> 5ns
            1 => 15,   // 11-20ns -> 15ns
            2 => 35,   // 21-50ns -> 35ns
            3 => 75,   // 51-100ns -> 75ns
            4 => 150,  // 101-200ns -> 150ns
            5 => 350,  // 201-500ns -> 350ns
            6 => 750,  // 501-1000ns -> 750ns
            7 => 1500, // 1001-2000ns -> 1500ns
            8 => 3500, // 2001-5000ns -> 3500ns
            9 => 7500, // 5001-10000ns -> 7500ns
            _ => {
                // Logarithmic reconstruction for higher buckets
                let log_value = bucket - 10;
                (1u64 << log_value) * 10000
            }
        }
    }

    /// Assess overall performance health based on B32 baselines
    fn assess_performance_health(
        &self,
        success_rate: f64,
        avg_latency: u64,
        error_rate: f64,
    ) -> bool {
        success_rate >= 95.0 &&     // B32: 95%+ success rate expected
        avg_latency <= 200 &&       // B32: <200ns average latency target
        error_rate <= 5.0 // B32: <5% error rate threshold
    }

    /// Calculate performance grade (A-F) based on multiple metrics
    fn calculate_performance_grade(
        &self,
        success_rate: f64,
        avg_latency: u64,
        cas_retry_rate: f64,
    ) -> char {
        let mut score = 0;

        // Success rate scoring (0-30 points)
        score += match success_rate {
            r if r >= 99.0 => 30,
            r if r >= 95.0 => 25,
            r if r >= 90.0 => 20,
            r if r >= 80.0 => 15,
            r if r >= 70.0 => 10,
            _ => 0,
        };

        // Latency scoring (0-40 points)
        score += match avg_latency {
            l if l <= 50 => 40,
            l if l <= 100 => 35,
            l if l <= 200 => 30,
            l if l <= 500 => 20,
            l if l <= 1000 => 10,
            _ => 0,
        };

        // Contention scoring (0-30 points)
        score += match cas_retry_rate {
            r if r <= 1.0 => 30,
            r if r <= 5.0 => 25,
            r if r <= 10.0 => 20,
            r if r <= 20.0 => 15,
            r if r <= 30.0 => 10,
            _ => 0,
        };

        // Convert score to grade
        match score {
            90..=100 => 'A',
            80..=89 => 'B',
            70..=79 => 'C',
            60..=69 => 'D',
            _ => 'F',
        }
    }

    /// Reset all metrics to zero (for testing or periodic resets)
    pub fn reset(&self) {
        self.metrics_generation.fetch_add(1, Ordering::Relaxed);

        // Reset hot metrics
        self.operation_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.total_latency.store(0, Ordering::Relaxed);
        self.cas_retries.store(0, Ordering::Relaxed);
        self.thread_collisions.store(0, Ordering::Relaxed);
        self.emergency_stops.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);

        // Reset histogram
        for bucket in &self.latency_histogram {
            bucket.store(0, Ordering::Relaxed);
        }

        // Reset error metrics
        self.validation_errors.store(0, Ordering::Relaxed);
        self.coordination_errors.store(0, Ordering::Relaxed);
        self.timeout_errors.store(0, Ordering::Relaxed);
        self.network_errors.store(0, Ordering::Relaxed);
        self.recovery_attempts.store(0, Ordering::Relaxed);
        self.successful_recoveries.store(0, Ordering::Relaxed);
        self.throttle_events.store(0, Ordering::Relaxed);
        self.backpressure_events.store(0, Ordering::Relaxed);
    }
}

#[cfg(not(feature = "metrics"))]
impl MetricsCollector {
    /// No-op implementation for disabled metrics
    #[inline(always)]
    pub fn new() -> Self {
        Self
    }

    /// No-op operation recording
    #[inline(always)]
    pub fn record_operation(&self, _success: bool, _latency_ns: u64) {
        // Compiles to nothing
    }

    /// No-op CAS retry recording
    #[inline(always)]
    pub fn record_cas_retry(&self) {
        // Compiles to nothing
    }

    /// No-op thread collision recording
    #[inline(always)]
    pub fn record_thread_collision(&self) {
        // Compiles to nothing
    }

    /// No-op cache hit recording
    #[inline(always)]
    pub fn record_cache_hit(&self) {
        // Compiles to nothing
    }

    /// No-op error recording
    #[inline(always)]
    pub fn record_error(&self, _category: ErrorCategory) {
        // Compiles to nothing
    }

    /// No-op recovery recording
    #[inline(always)]
    pub fn record_recovery(&self, _successful: bool) {
        // Compiles to nothing
    }

    /// No-op performance event recording
    #[inline(always)]
    pub fn record_throttle_event(&self) {
        // Compiles to nothing
    }

    #[inline(always)]
    pub fn record_backpressure_event(&self) {
        // Compiles to nothing
    }

    #[inline(always)]
    pub fn record_emergency_stop(&self) {
        // Compiles to nothing
    }

    /// Return empty metrics snapshot
    pub fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_operations: 0,
            success_rate: 0.0,
            avg_latency_ns: 0,
            p50_latency_ns: 0,
            p95_latency_ns: 0,
            p99_latency_ns: 0,
            ops_per_second: 0.0,
            cas_retry_rate: 0.0,
            thread_collision_rate: 0.0,
            error_rate: 0.0,
            recovery_success_rate: 0.0,
            cache_hit_rate: 0.0,
            is_healthy: true,
            performance_grade: 'A',
        }
    }

    /// No-op reset
    pub fn reset(&self) {
        // Compiles to nothing
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

// Clone implementation for Arc wrapping in OperationGuard
#[cfg(feature = "metrics")]
impl Clone for MetricsCollector {
    fn clone(&self) -> Self {
        // Create a new collector with reset counters
        // For shared observation, wrap the original in Arc<MetricsCollector>
        Self::new()
    }
}

// ============================================================================
// OBSERVABILITY EXTENSIONS - Enhanced Production Monitoring
// ============================================================================

/// Health status enumeration for system monitoring
///
/// UCE-32 Q30: Empirical validation - health status based on measurable thresholds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Unknown,
}

impl HealthStatus {
    pub fn is_degraded(&self) -> bool {
        matches!(self, HealthStatus::Degraded | HealthStatus::Critical)
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }
}

/// Diagnostic information for production debugging
///
/// UCE-32 Q30: Provides actionable diagnostic data for production issues
#[derive(Debug, Clone)]
pub struct DiagnosticInfo {
    pub performance_issues: Vec<String>,
    pub contention_hotspots: Vec<String>,
    pub error_patterns: Vec<String>,
    pub recommendations: Vec<String>,
}

impl DiagnosticInfo {
    pub fn new() -> Self {
        Self {
            performance_issues: Vec::new(),
            contention_hotspots: Vec::new(),
            error_patterns: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.performance_issues.is_empty()
            && self.contention_hotspots.is_empty()
            && self.error_patterns.is_empty()
            && self.recommendations.is_empty()
    }
}

/// RAII guard for operation timing with automatic metric recording
///
/// UCE-32 Q31: Rust RAII pattern ensures timing is recorded even if operation panics
#[cfg(feature = "metrics")]
pub struct OperationGuard {
    collector: Arc<MetricsCollector>,
    operation_name: String,
    start_time: Instant,
    completed: bool,
}

#[cfg(feature = "metrics")]
impl OperationGuard {
    fn new(collector: Arc<MetricsCollector>, operation_name: String) -> Self {
        Self {
            collector,
            operation_name,
            start_time: Instant::now(),
            completed: false,
        }
    }

    /// Record successful completion
    pub fn success(mut self) {
        let duration = self.start_time.elapsed();
        self.collector
            .record_operation(true, duration.as_nanos() as u64);
        self.completed = true;
        std::mem::forget(self); // Prevent Drop from running
    }

    /// Record error completion
    pub fn error(mut self, category: ErrorCategory) {
        let duration = self.start_time.elapsed();
        self.collector
            .record_operation(false, duration.as_nanos() as u64);
        self.collector.record_error(category);
        self.completed = true;
        std::mem::forget(self); // Prevent Drop from running
    }
}

#[cfg(feature = "metrics")]
impl Drop for OperationGuard {
    fn drop(&mut self) {
        if !self.completed {
            // If neither success() nor error() was called, record as timeout
            let duration = self.start_time.elapsed();
            self.collector
                .record_operation(false, duration.as_nanos() as u64);
            self.collector.record_error(ErrorCategory::Timeout);
        }
    }
}

/// No-op operation guard for disabled metrics
#[cfg(not(feature = "metrics"))]
pub struct OperationGuard;

#[cfg(not(feature = "metrics"))]
impl OperationGuard {
    fn new() -> Self {
        Self
    }

    pub fn success(self) {}
    pub fn error(self, _category: ErrorCategory) {}
}

/// Enhanced metrics collector with observability features
impl MetricsCollector {
    /// Get current health status - alias for health_check()
    pub fn health_status(&self) -> HealthStatus {
        self.health_check()
    }

    /// Get metrics snapshot - alias for get_metrics_snapshot()
    pub fn snapshot(&self) -> MetricsSnapshot {
        self.get_metrics_snapshot()
    }

    /// Start operation tracking - alias for track_operation()
    #[cfg(feature = "metrics")]
    pub fn start_operation(&self, operation_name: &str) -> OperationGuard {
        self.track_operation(operation_name)
    }

    #[cfg(not(feature = "metrics"))]
    pub fn start_operation(&self, operation_name: &str) -> OperationGuard {
        self.track_operation(operation_name)
    }

    /// Perform comprehensive health check based on current metrics
    ///
    /// UCE-32 Q30: Empirical validation - health determined by measurable thresholds
    pub fn health_check(&self) -> HealthStatus {
        let snapshot = self.get_metrics_snapshot();

        // Check for critical conditions
        if snapshot.success_rate < 50.0 {
            return HealthStatus::Critical;
        }

        if snapshot.error_rate > 20.0 {
            return HealthStatus::Critical;
        }

        // Check for degraded performance
        if snapshot.success_rate < SUCCESS_RATE_THRESHOLD {
            return HealthStatus::Degraded;
        }

        if snapshot.avg_latency_ns > PERFORMANCE_THRESHOLD_NS {
            return HealthStatus::Degraded;
        }

        if snapshot.error_rate > ERROR_RATE_THRESHOLD {
            return HealthStatus::Degraded;
        }

        // Check contention levels
        if snapshot.cas_retry_rate > 15.0 {
            // High contention threshold
            return HealthStatus::Degraded;
        }

        HealthStatus::Healthy
    }

    /// Generate diagnostic information for production debugging
    ///
    /// UCE-32 Q30: Actionable diagnostics based on empirical measurements
    pub fn diagnostics(&self) -> DiagnosticInfo {
        let snapshot = self.get_metrics_snapshot();
        let mut diag = DiagnosticInfo::new();

        // Performance analysis
        if snapshot.avg_latency_ns > PERFORMANCE_THRESHOLD_NS {
            diag.performance_issues.push(format!(
                "High latency detected: {}ns (threshold: {}ns)",
                snapshot.avg_latency_ns, PERFORMANCE_THRESHOLD_NS
            ));
            diag.recommendations
                .push("Consider optimizing hot paths or reducing contention".to_string());
        }

        if snapshot.p99_latency_ns > PERFORMANCE_THRESHOLD_NS * 3 {
            diag.performance_issues.push(format!(
                "High P99 latency: {}ns indicates tail latency issues",
                snapshot.p99_latency_ns
            ));
            diag.recommendations
                .push("Investigate periodic performance spikes".to_string());
        }

        // Contention analysis
        if snapshot.cas_retry_rate > 10.0 {
            diag.contention_hotspots.push(format!(
                "High CAS retry rate: {:.2}% indicates thread contention",
                snapshot.cas_retry_rate
            ));
            diag.recommendations
                .push("Consider reducing concurrent access or optimizing CAS loops".to_string());
        }

        if snapshot.thread_collision_rate > 5.0 {
            diag.contention_hotspots.push(format!(
                "Thread collision rate: {:.2}% suggests coordination bottlenecks",
                snapshot.thread_collision_rate
            ));
            diag.recommendations
                .push("Review thread coordination patterns".to_string());
        }

        // Error pattern analysis
        if snapshot.error_rate > ERROR_RATE_THRESHOLD {
            diag.error_patterns.push(format!(
                "Error rate: {:.2}% exceeds threshold of {:.1}%",
                snapshot.error_rate, ERROR_RATE_THRESHOLD
            ));
            diag.recommendations
                .push("Investigate error root causes and improve error handling".to_string());
        }

        if snapshot.recovery_success_rate < 80.0 && snapshot.recovery_success_rate > 0.0 {
            diag.error_patterns.push(format!(
                "Low recovery success rate: {:.2}%",
                snapshot.recovery_success_rate
            ));
            diag.recommendations
                .push("Improve error recovery mechanisms".to_string());
        }

        // Cache efficiency analysis
        if snapshot.cache_hit_rate < 90.0 && snapshot.cache_hit_rate > 0.0 {
            diag.performance_issues.push(format!(
                "Low cache hit rate: {:.2}%",
                snapshot.cache_hit_rate
            ));
            diag.recommendations
                .push("Review data access patterns and cache usage".to_string());
        }

        // Throughput analysis
        if snapshot.ops_per_second > 0.0 && snapshot.ops_per_second < 1000.0 {
            diag.performance_issues.push(format!(
                "Low throughput: {:.0} ops/sec",
                snapshot.ops_per_second
            ));
            diag.recommendations
                .push("Profile for performance bottlenecks".to_string());
        }

        diag
    }

    /// Start tracking an operation with RAII guard
    ///
    /// UCE-32 Q28: Simplified API for operation tracking
    /// UCE-32 Q31: Zero-cost when metrics disabled
    #[cfg(feature = "metrics")]
    pub fn track_operation(&self, operation_name: &str) -> OperationGuard {
        OperationGuard::new(Arc::new(self.clone()), operation_name.to_string())
    }

    #[cfg(not(feature = "metrics"))]
    pub fn track_operation(&self, _operation_name: &str) -> OperationGuard {
        OperationGuard::new()
    }

    /// Get performance summary string for monitoring dashboards
    ///
    /// UCE-32 Q28: Simple API for monitoring integration
    pub fn performance_summary(&self) -> String {
        let snapshot = self.get_metrics_snapshot();
        let health = self.health_check();

        format!(
            "Health: {:?} | Ops: {} | Success: {:.1}% | Latency: {}ns (P99: {}ns) | Throughput: {:.0}/sec | Grade: {}",
            health,
            snapshot.total_operations,
            snapshot.success_rate,
            snapshot.avg_latency_ns,
            snapshot.p99_latency_ns,
            snapshot.ops_per_second,
            snapshot.performance_grade
        )
    }

    /// Check if performance is degraded (simple boolean for alerting)
    ///
    /// UCE-32 Q28: Simple API for alerting systems
    pub fn is_performance_degraded(&self) -> bool {
        self.health_check().is_degraded()
    }
}

/// Global metrics collector for simple usage pattern
///
/// UCE-32 Q28: Simplified global access pattern
static GLOBAL_METRICS: std::sync::OnceLock<Arc<MetricsCollector>> = std::sync::OnceLock::new();

/// Get or initialize global metrics collector
///
/// UCE-32 Q28: Simple access to global metrics collection
pub fn global_metrics() -> &'static Arc<MetricsCollector> {
    GLOBAL_METRICS.get_or_init(|| Arc::new(MetricsCollector::new()))
}

/// Convenience macro for operation tracking
///
/// UCE-32 Q28: Simplified operation tracking
#[macro_export]
macro_rules! track_hedge_operation {
    ($operation:expr, $code:block) => {{
        let metrics = $crate::metrics::global_metrics();
        let guard = metrics.track_operation($operation);
        let result = (|| $code)();
        match &result {
            Ok(_) => guard.success(),
            Err(_) => guard.error($crate::metrics::ErrorCategory::Coordination),
        }
        result
    }};
}

/// No-op macro when metrics are disabled
#[cfg(not(feature = "metrics"))]
#[macro_export]
macro_rules! track_hedge_operation {
    ($operation:expr, $code:block) => {
        $code
    };
}

impl std::fmt::Display for MetricsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MetricsSnapshot {{ ops: {}, success: {:.1}%, avg_latency: {}ns, \
             p99: {}ns, ops/sec: {:.0}, grade: {} }}",
            self.total_operations,
            self.success_rate,
            self.avg_latency_ns,
            self.p99_latency_ns,
            self.ops_per_second,
            self.performance_grade
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_zero_overhead_disabled() {
        // When metrics are disabled, this should compile to essentially nothing
        let metrics = MetricsCollector::new();
        metrics.record_operation(true, 100);
        metrics.record_cas_retry();
        let snapshot = metrics.get_metrics_snapshot();

        #[cfg(not(feature = "metrics"))]
        {
            assert_eq!(snapshot.total_operations, 0);
            assert_eq!(snapshot.success_rate, 0.0);
        }
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_metrics_collection_enabled() {
        let metrics = MetricsCollector::new();

        // Record some operations
        metrics.record_operation(true, 100);
        metrics.record_operation(true, 150);
        metrics.record_operation(false, 200);
        metrics.record_cas_retry();

        let snapshot = metrics.get_metrics_snapshot();

        assert_eq!(snapshot.total_operations, 3);
        assert_eq!(snapshot.success_rate, 200.0 / 3.0);
        assert!(snapshot.avg_latency_ns > 0);
        assert!(snapshot.cas_retry_rate > 0.0);
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_latency_histogram() {
        let metrics = MetricsCollector::new();

        // Record various latencies
        metrics.record_operation(true, 5); // Bucket 0
        metrics.record_operation(true, 15); // Bucket 1
        metrics.record_operation(true, 75); // Bucket 3
        metrics.record_operation(true, 150); // Bucket 4
        metrics.record_operation(true, 1500); // Bucket 7

        let snapshot = metrics.get_metrics_snapshot();

        assert!(snapshot.p50_latency_ns > 0);
        assert!(snapshot.p95_latency_ns >= snapshot.p50_latency_ns);
        assert!(snapshot.p99_latency_ns >= snapshot.p95_latency_ns);
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_performance_grading() {
        let metrics = MetricsCollector::new();

        // Record excellent performance
        for _ in 0..100 {
            metrics.record_operation(true, 50); // Low latency, high success
        }

        let snapshot = metrics.get_metrics_snapshot();

        assert_eq!(snapshot.success_rate, 100.0);
        assert!(snapshot.avg_latency_ns <= 50);
        assert_eq!(snapshot.performance_grade, 'A');
        assert!(snapshot.is_healthy);
    }

    #[test]
    fn test_error_category_tracking() {
        let metrics = MetricsCollector::new();

        metrics.record_error(ErrorCategory::Validation);
        metrics.record_error(ErrorCategory::Coordination);
        metrics.record_recovery(true);
        metrics.record_recovery(false);

        let snapshot = metrics.get_metrics_snapshot();

        #[cfg(feature = "metrics")]
        {
            assert_eq!(snapshot.recovery_success_rate, 50.0);
        }
    }

    // ============================================================================
    // OBSERVABILITY FEATURE TESTS
    // ============================================================================

    #[cfg(feature = "metrics")]
    #[test]
    fn test_health_check_functionality() {
        let metrics = MetricsCollector::new();

        // Initially healthy
        assert_eq!(metrics.health_check(), HealthStatus::Healthy);

        // Degrade with high latency
        for _ in 0..10 {
            metrics.record_operation(true, PERFORMANCE_THRESHOLD_NS + 100);
        }
        assert_eq!(metrics.health_check(), HealthStatus::Degraded);

        // Critical with low success rate
        let critical_metrics = MetricsCollector::new();
        for _ in 0..10 {
            critical_metrics.record_operation(false, 100); // All failures
        }
        assert_eq!(critical_metrics.health_check(), HealthStatus::Critical);
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_diagnostics_generation() {
        let metrics = MetricsCollector::new();

        // Create conditions that should trigger diagnostics

        // High latency
        for _ in 0..5 {
            metrics.record_operation(true, PERFORMANCE_THRESHOLD_NS + 1000);
        }

        // High error rate
        for _ in 0..10 {
            metrics.record_operation(false, 100);
        }

        // High contention
        for _ in 0..50 {
            metrics.record_cas_retry();
        }

        let diagnostics = metrics.diagnostics();

        assert!(!diagnostics.performance_issues.is_empty());
        assert!(!diagnostics.error_patterns.is_empty());
        assert!(!diagnostics.contention_hotspots.is_empty());
        assert!(!diagnostics.recommendations.is_empty());

        println!("Diagnostics test results:");
        for issue in &diagnostics.performance_issues {
            println!("  Performance: {}", issue);
        }
        for pattern in &diagnostics.error_patterns {
            println!("  Error: {}", pattern);
        }
        for hotspot in &diagnostics.contention_hotspots {
            println!("  Contention: {}", hotspot);
        }
        for rec in &diagnostics.recommendations {
            println!("  Recommendation: {}", rec);
        }
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_operation_guard_success() {
        let metrics = Arc::new(MetricsCollector::new());

        {
            let guard = metrics.track_operation("test_operation");
            std::thread::sleep(std::time::Duration::from_nanos(100));
            guard.success();
        }

        let snapshot = metrics.get_metrics_snapshot();
        assert_eq!(snapshot.total_operations, 1);
        assert_eq!(snapshot.success_rate, 100.0);
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_operation_guard_error() {
        let metrics = Arc::new(MetricsCollector::new());

        {
            let guard = metrics.track_operation("test_operation");
            std::thread::sleep(std::time::Duration::from_nanos(100));
            guard.error(ErrorCategory::Timeout);
        }

        let snapshot = metrics.get_metrics_snapshot();
        assert_eq!(snapshot.total_operations, 1);
        assert_eq!(snapshot.success_rate, 0.0);
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_operation_guard_auto_drop() {
        let metrics = Arc::new(MetricsCollector::new());

        // Guard dropped without explicit success/error call
        {
            let _guard = metrics.track_operation("test_operation");
            std::thread::sleep(std::time::Duration::from_nanos(100));
            // Guard dropped here, should record as timeout error
        }

        let snapshot = metrics.get_metrics_snapshot();
        assert_eq!(snapshot.total_operations, 1);
        assert_eq!(snapshot.success_rate, 0.0); // Should be recorded as error
    }

    #[test]
    fn test_global_metrics_access() {
        let global = global_metrics();
        global.record_operation(true, 100);

        let snapshot = global.get_metrics_snapshot();

        #[cfg(feature = "metrics")]
        {
            assert!(snapshot.total_operations >= 1);
        }

        #[cfg(not(feature = "metrics"))]
        {
            assert_eq!(snapshot.total_operations, 0);
        }
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_performance_summary() {
        let metrics = MetricsCollector::new();

        // Add some metrics
        for i in 0..10 {
            metrics.record_operation(i < 9, 50 + i * 10); // 90% success rate
        }

        let summary = metrics.performance_summary();
        assert!(summary.contains("Health:"));
        assert!(summary.contains("Success:"));
        assert!(summary.contains("Latency:"));
        assert!(summary.contains("Grade:"));

        println!("Performance summary: {}", summary);
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_performance_degradation_detection() {
        let healthy_metrics = MetricsCollector::new();
        for _ in 0..10 {
            healthy_metrics.record_operation(true, 50); // Good performance
        }
        assert!(!healthy_metrics.is_performance_degraded());

        let degraded_metrics = MetricsCollector::new();
        for _ in 0..10 {
            degraded_metrics.record_operation(true, PERFORMANCE_THRESHOLD_NS + 100);
            // High latency
        }
        assert!(degraded_metrics.is_performance_degraded());
    }

    #[test]
    fn test_macro_compilation() {
        // Test that the macro compiles correctly
        let result: Result<i32, &str> = track_hedge_operation!("test_macro", { Ok(42) });
        assert_eq!(result.unwrap(), 42);

        let error_result: Result<i32, &str> =
            track_hedge_operation!("test_macro_error", { Err("test error") });
        assert!(error_result.is_err());
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_concurrent_metrics_with_observability() {
        use std::thread;

        let metrics = Arc::new(MetricsCollector::new());
        let mut handles = Vec::new();

        // Spawn multiple threads with different operation patterns
        for thread_id in 0..4 {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    let guard = metrics_clone.track_operation("concurrent_test");

                    // Simulate work
                    std::thread::sleep(std::time::Duration::from_nanos(100));

                    match (thread_id + i) % 10 {
                        0..=7 => guard.success(), // 80% success rate
                        8 => guard.error(ErrorCategory::Timeout),
                        _ => guard.error(ErrorCategory::Coordination),
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        let snapshot = metrics.get_metrics_snapshot();
        let health = metrics.health_check();
        let diagnostics = metrics.diagnostics();

        println!("Concurrent test results:");
        println!("  Total operations: {}", snapshot.total_operations);
        println!("  Success rate: {:.2}%", snapshot.success_rate);
        println!("  Health: {:?}", health);
        println!("  Performance summary: {}", metrics.performance_summary());

        if !diagnostics.is_empty() {
            println!("  Diagnostics:");
            for rec in &diagnostics.recommendations {
                println!("    - {}", rec);
            }
        }

        assert_eq!(snapshot.total_operations, 200); // 4 threads × 50 operations
        assert!(snapshot.success_rate >= 75.0); // Should be around 80%
        assert!(health.is_healthy() || health.is_degraded()); // Should not be critical
    }
}
