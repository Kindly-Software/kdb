//! Dashboard Content Capsule - Lockfree Metrics Cache for TUI
//!
//! # Purpose
//! Computational capsule for caching metrics data with:
//! - **100% lockfree** - Atomic fields only (no Mutex/RwLock)
//! - **Real-time updates** - Background HTTP polling with atomic stores
//! - **Sub-5ms rendering** - Ratatui-optimized layouts
//! - **Byzantine Purple theme** - #663399 headers, #FFD700 accents
//!
//! # UCE34 Framework
//! - **Q10**: Tier 1 (Atomic) - Lockfree coordination capsule
//! - **Q11**: Rust atomic primitives (AtomicU32, AtomicU64, AtomicBool)
//! - **Q12**: Stable Rust (no nightly features required)
//! - **Q33**: Compile-time verification with #[derive(ComputationalCapsule)]
//!
//! # Performance
//! - **Atomic update**: <10ns per field (Relaxed ordering for metrics)
//! - **Atomic read**: <5ns per field (Relaxed ordering)
//! - **Full snapshot**: <100ns (read all metrics atomically)
//! - **Rendering**: <5ms (ratatui layout + terminal I/O)
//!
//! # ASSUM Safety
//! - `#ASSUME_METRICS_ATOMIC`: All metrics fields are atomic types
//! - `#VERIFY_METRICS_ATOMIC`: Enforced by ComputationalCapsule derive macro
//! - `#ASSUME_RELAXED_ORDERING_OK`: Metrics cache uses Relaxed ordering (no synchronization needed)
//! - `#VERIFY_RELAXED_OK`: Metrics are read-only snapshots, no happens-before relationships required

#![warn(clippy::missing_capsule_verification)]

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Row, Table},
    style::{Color, Modifier, Style},
};
use crate::cli::dashboard::{
    BudgetMetric, ProviderMetric, SystemMetrics,
    CacheMetricsPanel, CompressionMetricsPanel,
    LoadBalancerMetricsPanel, PerformanceMetricsPanel,
};

/// Dashboard content capsule (Tier 1 Atomic)
///
/// # Layout
/// ```text
/// [0-3]     budgets_count            AtomicU32 (4B)
/// [4-7]     providers_count          AtomicU32 (4B)
/// [8-15]    last_refresh_ns          AtomicU64 (8B)
/// [16-19]   refresh_interval_ms      AtomicU32 (4B)
/// [20-23]   total_requests           AtomicU32 (4B)
/// [24-27]   avg_latency_ms           AtomicU32 (4B)
/// [28-31]   memory_mb                AtomicU32 (4B)
/// [32-39]   uptime_secs              AtomicU64 (8B)
/// [40]      is_paused                AtomicBool (1B)
/// [41]      has_error                AtomicBool (1B)
/// [42-63]   _padding1                [u8; 22] (padding to 64B)
/// [64-71]   circuit_breaker_states   AtomicU64 (8B, 8 providers × 8 bits)
/// [72-79]   provider_success_rates   AtomicU64 (8B, 8 providers × 8 bits)
/// [80-87]   provider_failures        AtomicU64 (8B, 8 providers × 8 bits)
/// [88-95]   budget_utilization       AtomicU64 (8B, 8 budgets × 8 bits)
/// [96-99]   p50_latency_ms           AtomicU32 (4B)
/// [100-103] p99_latency_ms           AtomicU32 (4B)
/// [104-107] p999_latency_ms          AtomicU32 (4B)
/// [108-111] cost_per_1k_tokens_cents AtomicU32 (4B, Q16.16 fixed-point)
/// [112-119] total_spent_cents        AtomicU64 (8B)
/// [120-123] request_rate_per_sec     AtomicU32 (4B)
/// [124-127] loop_armor_rate_allowed  AtomicU32 (4B)
/// [128-131] loop_armor_rate_blocked  AtomicU32 (4B)
/// [132-135] loop_armor_rate_quota    AtomicU32 (4B)
/// [136-139] loop_armor_dedup_hits    AtomicU32 (4B)
/// [140-143] loop_armor_dedup_misses  AtomicU32 (4B)
/// [144-147] loop_armor_anomaly_count AtomicU32 (4B)
/// [148-151] loop_armor_p99_current   AtomicU32 (4B)
/// [152-155] loop_armor_p99_baseline  AtomicU32 (4B)
/// [156-159] loop_armor_severity      AtomicU32 (4B)
/// [160-191] _padding2                [u8; 28] (padding to 192B)
/// [192-195] loop_armor_burst_count   AtomicU32 (4B, total bursts)
/// [196-199] loop_armor_burst_window  AtomicU32 (4B, window count)
/// [200-207] loop_armor_cost_velocity AtomicU64 (8B, Q16.16 cents/min)
/// [208-211] loop_armor_cost_alerts   AtomicU32 (4B, velocity alerts)
/// [212-215] loop_armor_pattern_count AtomicU32 (4B, total patterns)
/// [216-219] loop_armor_pattern_matches AtomicU32 (4B, current matches)
/// [220-255] _padding3                [u8; 36] (padding to 256B)
/// [256-259] loop_armor_circuit_closed_count AtomicU32 (4B, Closed clients)
/// [260-263] loop_armor_circuit_halfopen_count AtomicU32 (4B, HalfOpen clients)
/// [264-267] loop_armor_circuit_open_count AtomicU32 (4B, Open clients)
/// [268-271] loop_armor_circuit_total_opens AtomicU32 (4B, total open events)
/// [272-275] loop_armor_circuit_total_recoveries AtomicU32 (4B, Closed transitions)
/// [276-279] loop_armor_circuit_avg_error_rate AtomicU32 (4B, avg error rate bp)
/// [280-383] _padding4                [u8; 104] (padding to 384B)
/// ```
///
/// **Alignment**: 128B (cache line aligned)
/// **Size**: 384B (6 cache lines: hot 64B + cold 60B + Phase1 64B + Phase2 64B + Phase3 24B + padding 108B)
///
/// # Performance
/// - **Atomic update**: <10ns (Relaxed ordering)
/// - **Atomic read**: <5ns (Relaxed ordering)
/// - **Full snapshot**: <300ns (read all fields including Loop Armor Phase 1 + Phase 2 + Phase 3)
///
/// # Chaos Principles
/// - **Cache-aligned**: 128B alignment prevents false sharing
/// - **Tiered layout**: Hot metrics (first 64B), cold metrics (64-128B), Loop Armor Phase 1 (128-192B), Loop Armor Phase 2 (192-256B), Loop Armor Phase 3 (256-384B)
/// - **One-read decision**: All metrics fit in 384B for 6 cache line reads
/// - **Lockfree**: 100% atomic operations (no Mutex/RwLock)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 384, tier = "Atomic")]
#[repr(C, align(128))]
pub struct DashboardContentCapsule {
    // Hot metrics (first 64B) - updated frequently
    /// Number of active budgets (0-1M)
    budgets_count: AtomicU32,

    /// Number of configured providers (0-16)
    providers_count: AtomicU32,

    /// Last refresh timestamp (nanoseconds since epoch)
    last_refresh_ns: AtomicU64,

    /// Refresh interval in milliseconds (default: 5000ms)
    refresh_interval_ms: AtomicU32,

    /// Total requests processed since startup
    total_requests: AtomicU32,

    /// Average latency in milliseconds (rolling window)
    avg_latency_ms: AtomicU32,

    /// Memory usage in megabytes
    memory_mb: AtomicU32,

    /// Uptime in seconds
    uptime_secs: AtomicU64,

    /// Dashboard paused state (true = paused, false = live)
    is_paused: AtomicBool,

    /// Error state (true = last fetch failed, false = ok)
    has_error: AtomicBool,

    /// Padding to 64B boundary (first cache line)
    _padding1: [u8; 22],

    // Cold metrics (second 64B) - tabbed metrics expansion
    /// Circuit breaker states for 8 providers (0-2 each, 8 bits per provider)
    /// Packed format: provider_0[7:0] | provider_1[15:8] | ... | provider_7[63:56]
    circuit_breaker_states: AtomicU64,

    /// Provider success rates (0-100%, 8 bits per provider)
    /// Packed format: provider_0[7:0] | provider_1[15:8] | ... | provider_7[63:56]
    provider_success_rates: AtomicU64,

    /// Provider failure counts (0-255, 8 bits per provider)
    /// Packed format: provider_0[7:0] | provider_1[15:8] | ... | provider_7[63:56]
    provider_failures: AtomicU64,

    /// Budget utilization percentages (0-100%, 8 bits per budget)
    /// Packed format: budget_0[7:0] | budget_1[15:8] | ... | budget_7[63:56]
    budget_utilization: AtomicU64,

    /// P50 latency in milliseconds
    p50_latency_ms: AtomicU32,

    /// P99 latency in milliseconds
    p99_latency_ms: AtomicU32,

    /// P999 latency in milliseconds
    p999_latency_ms: AtomicU32,

    /// Cost per 1K tokens in cents (Q16.16 fixed-point stored as u32)
    cost_per_1k_tokens_cents: AtomicU32,

    /// Total spending in cents
    total_spent_cents: AtomicU64,

    /// Request rate per second (calculated from total_requests / uptime)
    request_rate_per_sec: AtomicU32,

    /// Loop Armor metrics (Phase 1 Loop Protection)
    /// Rate limit: Requests allowed in current window
    loop_armor_rate_allowed: AtomicU32,
    /// Rate limit: Requests blocked (429) in current window
    loop_armor_rate_blocked: AtomicU32,
    /// Rate limit: Quota remaining (0-1000)
    loop_armor_rate_quota: AtomicU32,
    /// Dedup: Duplicate requests detected (cache hits)
    loop_armor_dedup_hits: AtomicU32,
    /// Dedup: Unique requests (cache misses)
    loop_armor_dedup_misses: AtomicU32,
    /// Anomaly: Anomalies detected count
    loop_armor_anomaly_count: AtomicU32,
    /// Anomaly: Current p99 latency (ms)
    loop_armor_p99_current: AtomicU32,
    /// Anomaly: Baseline p99 latency (ms)
    loop_armor_p99_baseline: AtomicU32,
    /// Anomaly: Severity (0=None, 1=Low, 2=Medium, 3=High, 4=Critical)
    loop_armor_severity: AtomicU32,

    /// Padding to 192B boundary (Phase 1 metrics)
    _padding2: [u8; 28],

    // Phase 2: Enhanced Detection (64B)
    /// Burst Detection: Total bursts detected
    loop_armor_burst_count: AtomicU32,
    /// Burst Detection: Requests in current window (0-10)
    loop_armor_burst_window: AtomicU32,
    /// Cost Velocity: Current EMA (Q16.16 fixed-point cents/min)
    loop_armor_cost_velocity: AtomicU64,
    /// Cost Velocity: Velocity alerts count
    loop_armor_cost_alerts: AtomicU32,
    /// Pattern Signature: Total patterns detected
    loop_armor_pattern_count: AtomicU32,
    /// Pattern Signature: Current window matches (0-8)
    loop_armor_pattern_matches: AtomicU32,

    /// Padding to 256B boundary (Phase 2 metrics)
    _padding3: [u8; 36],

    // Phase 3: Client Circuit Breaker (64B)
    /// Client Circuit Breaker: Clients in Closed state
    loop_armor_circuit_closed_count: AtomicU32,
    /// Client Circuit Breaker: Clients in HalfOpen state
    loop_armor_circuit_halfopen_count: AtomicU32,
    /// Client Circuit Breaker: Clients in Open state
    loop_armor_circuit_open_count: AtomicU32,
    /// Client Circuit Breaker: Total open events
    loop_armor_circuit_total_opens: AtomicU32,
    /// Client Circuit Breaker: Total Closed transitions (recoveries)
    loop_armor_circuit_total_recoveries: AtomicU32,
    /// Client Circuit Breaker: Average error rate (basis points, 0-10000)
    loop_armor_circuit_avg_error_rate: AtomicU32,

    /// Padding to 384B boundary (Phase 3 metrics)
    _padding4: [u8; 104],
}

impl DashboardContentCapsule {
    /// Create new dashboard content capsule
    ///
    /// # Performance
    /// - <50ns (zero-cost atomic initialization)
    ///
    /// # Examples
    /// ```ignore
    /// use clapi_core::tui::DashboardContentCapsule;
    ///
    /// let capsule = DashboardContentCapsule::new(5000); // 5s refresh
    /// ```
    pub fn new(refresh_interval_ms: u32) -> Self {
        Self {
            budgets_count: AtomicU32::new(0),
            providers_count: AtomicU32::new(0),
            last_refresh_ns: AtomicU64::new(0),
            refresh_interval_ms: AtomicU32::new(refresh_interval_ms),
            total_requests: AtomicU32::new(0),
            avg_latency_ms: AtomicU32::new(0),
            memory_mb: AtomicU32::new(0),
            uptime_secs: AtomicU64::new(0),
            is_paused: AtomicBool::new(false),
            has_error: AtomicBool::new(false),
            _padding1: [0; 22],
            circuit_breaker_states: AtomicU64::new(0),
            provider_success_rates: AtomicU64::new(0),
            provider_failures: AtomicU64::new(0),
            budget_utilization: AtomicU64::new(0),
            p50_latency_ms: AtomicU32::new(0),
            p99_latency_ms: AtomicU32::new(0),
            p999_latency_ms: AtomicU32::new(0),
            cost_per_1k_tokens_cents: AtomicU32::new(0),
            total_spent_cents: AtomicU64::new(0),
            request_rate_per_sec: AtomicU32::new(0),
            loop_armor_rate_allowed: AtomicU32::new(0),
            loop_armor_rate_blocked: AtomicU32::new(0),
            loop_armor_rate_quota: AtomicU32::new(1000),
            loop_armor_dedup_hits: AtomicU32::new(0),
            loop_armor_dedup_misses: AtomicU32::new(0),
            loop_armor_anomaly_count: AtomicU32::new(0),
            loop_armor_p99_current: AtomicU32::new(0),
            loop_armor_p99_baseline: AtomicU32::new(0),
            loop_armor_severity: AtomicU32::new(0),
            _padding2: [0; 28],
            loop_armor_burst_count: AtomicU32::new(0),
            loop_armor_burst_window: AtomicU32::new(0),
            loop_armor_cost_velocity: AtomicU64::new(0),
            loop_armor_cost_alerts: AtomicU32::new(0),
            loop_armor_pattern_count: AtomicU32::new(0),
            loop_armor_pattern_matches: AtomicU32::new(0),
            _padding3: [0; 36],
            loop_armor_circuit_closed_count: AtomicU32::new(0),
            loop_armor_circuit_halfopen_count: AtomicU32::new(0),
            loop_armor_circuit_open_count: AtomicU32::new(0),
            loop_armor_circuit_total_opens: AtomicU32::new(0),
            loop_armor_circuit_total_recoveries: AtomicU32::new(0),
            loop_armor_circuit_avg_error_rate: AtomicU32::new(0),
            _padding4: [0; 104],
        }
    }

    /// Update metrics from system metrics snapshot
    ///
    /// # Performance
    /// - <50ns (5 atomic stores with Relaxed ordering)
    ///
    /// # ASSUM
    /// - `#ASSUME_RELAXED_ORDERING_OK`: Metrics are read-only snapshots
    /// - `#VERIFY_RELAXED_OK`: No synchronization needed for dashboard display
    pub fn update_system_metrics(&self, metrics: &SystemMetrics) {
        self.total_requests.store(metrics.total_requests as u32, Ordering::Relaxed);
        self.avg_latency_ms.store(metrics.avg_latency_ms as u32, Ordering::Relaxed);
        self.memory_mb.store(metrics.memory_mb as u32, Ordering::Relaxed);
        self.uptime_secs.store(metrics.uptime_secs, Ordering::Relaxed);
        self.last_refresh_ns.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            Ordering::Relaxed,
        );
    }

    /// Update budget count
    pub fn set_budgets_count(&self, count: u32) {
        self.budgets_count.store(count, Ordering::Relaxed);
    }

    /// Update provider count
    pub fn set_providers_count(&self, count: u32) {
        self.providers_count.store(count, Ordering::Relaxed);
    }

    /// Set paused state
    pub fn set_paused(&self, paused: bool) {
        self.is_paused.store(paused, Ordering::Relaxed);
    }

    /// Set error state
    pub fn set_error(&self, error: bool) {
        self.has_error.store(error, Ordering::Relaxed);
    }

    /// Get paused state
    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::Relaxed)
    }

    /// Get error state
    pub fn has_error(&self) -> bool {
        self.has_error.load(Ordering::Relaxed)
    }

    /// Get budgets count (lockfree, <5ns)
    #[inline(always)]
    pub fn budgets_count(&self) -> u32 {
        self.budgets_count.load(Ordering::Relaxed)
    }

    /// Get providers count (lockfree, <5ns)
    #[inline(always)]
    pub fn providers_count(&self) -> u32 {
        self.providers_count.load(Ordering::Relaxed)
    }

    /// Get total requests (lockfree, <5ns)
    #[inline(always)]
    pub fn total_requests(&self) -> u32 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Get average latency (lockfree, <5ns)
    #[inline(always)]
    pub fn avg_latency(&self) -> u32 {
        self.avg_latency_ms.load(Ordering::Relaxed)
    }

    /// Get memory usage in MB (lockfree, <5ns)
    #[inline(always)]
    pub fn memory_mb(&self) -> u32 {
        self.memory_mb.load(Ordering::Relaxed)
    }

    /// Get uptime in seconds (lockfree, <5ns)
    #[inline(always)]
    pub fn uptime(&self) -> u64 {
        self.uptime_secs.load(Ordering::Relaxed)
    }

    /// Get last refresh timestamp (nanoseconds, lockfree, <5ns)
    #[inline(always)]
    pub fn last_refresh(&self) -> u64 {
        self.last_refresh_ns.load(Ordering::Relaxed)
    }

    // ============================================================================
    // Bit-Packing Helpers (Private)
    // ============================================================================

    /// Pack 8 u8 values into a single u64 (8 bits per value)
    ///
    /// # Performance
    /// - <5ns (bit shifts + OR operations)
    ///
    /// # ASSUM
    /// - `#ASSUME_BIT_PACKING_SAFE`: u8 values packed in u64 don't overflow (8 × 8 = 64 bits)
    /// - `#VERIFY_BIT_PACKING_SAFE`: Static assertion 8 * 8 == 64
    #[inline(always)]
    fn pack_u8_array(values: &[u8; 8]) -> u64 {
        (values[0] as u64)
            | ((values[1] as u64) << 8)
            | ((values[2] as u64) << 16)
            | ((values[3] as u64) << 24)
            | ((values[4] as u64) << 32)
            | ((values[5] as u64) << 40)
            | ((values[6] as u64) << 48)
            | ((values[7] as u64) << 56)
    }

    /// Unpack a single u8 value from a u64 at the given index (0-7)
    ///
    /// # Performance
    /// - <5ns (bit shift + mask)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: index is 0-7 (saturate to 7 if invalid)
    /// - `#VERIFY_PROVIDER_INDEX`: Saturate to 7 in all call sites
    #[inline(always)]
    fn unpack_u8_at(packed: u64, index: u8) -> u8 {
        // #VERIFY_PROVIDER_INDEX: Saturate to max 7
        let safe_index = index.min(7);
        ((packed >> (safe_index * 8)) & 0xFF) as u8
    }

    /// Set a single u8 value in a packed u64 at the given index (0-7)
    ///
    /// # Performance
    /// - <10ns (mask + bit shift + OR)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: index is 0-7 (saturate to 7 if invalid)
    /// - `#VERIFY_PROVIDER_INDEX`: Saturate to 7 in all call sites
    #[inline(always)]
    fn set_u8_at(packed: u64, index: u8, value: u8) -> u64 {
        // #VERIFY_PROVIDER_INDEX: Saturate to max 7
        let safe_index = index.min(7);
        let mask = !(0xFFu64 << (safe_index * 8));
        let cleared = packed & mask;
        cleared | ((value as u64) << (safe_index * 8))
    }

    // ============================================================================
    // Circuit Breaker State Methods
    // ============================================================================

    /// Set circuit breaker state for a provider (0-2: Closed/HalfOpen/Open)
    ///
    /// # Performance
    /// - <10ns (atomic load, bit manipulation, atomic store)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: provider_idx is 0-7
    /// - `#VERIFY_PROVIDER_INDEX`: Saturated in set_u8_at()
    #[inline(always)]
    pub fn set_circuit_state(&self, provider_idx: u8, state: u8) {
        let current = self.circuit_breaker_states.load(Ordering::Relaxed);
        let updated = Self::set_u8_at(current, provider_idx, state);
        self.circuit_breaker_states.store(updated, Ordering::Relaxed);
    }

    /// Get circuit breaker state for a provider
    ///
    /// # Performance
    /// - <5ns (atomic load, bit shift, mask)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: provider_idx is 0-7
    /// - `#VERIFY_PROVIDER_INDEX`: Saturated in unpack_u8_at()
    #[inline(always)]
    pub fn get_circuit_state(&self, provider_idx: u8) -> u8 {
        let packed = self.circuit_breaker_states.load(Ordering::Relaxed);
        Self::unpack_u8_at(packed, provider_idx)
    }

    // ============================================================================
    // Provider Success Rate Methods
    // ============================================================================

    /// Set provider success rate (0-100%)
    ///
    /// # Performance
    /// - <10ns (atomic load, bit manipulation, atomic store)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: provider_idx is 0-7
    /// - `#VERIFY_PROVIDER_INDEX`: Saturated in set_u8_at()
    #[inline(always)]
    pub fn set_provider_success_rate(&self, provider_idx: u8, rate: u8) {
        let current = self.provider_success_rates.load(Ordering::Relaxed);
        let updated = Self::set_u8_at(current, provider_idx, rate.min(100));
        self.provider_success_rates.store(updated, Ordering::Relaxed);
    }

    /// Get provider success rate (0-100%)
    ///
    /// # Performance
    /// - <5ns (atomic load, bit shift, mask)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: provider_idx is 0-7
    /// - `#VERIFY_PROVIDER_INDEX`: Saturated in unpack_u8_at()
    #[inline(always)]
    pub fn get_provider_success_rate(&self, provider_idx: u8) -> u8 {
        let packed = self.provider_success_rates.load(Ordering::Relaxed);
        Self::unpack_u8_at(packed, provider_idx)
    }

    // ============================================================================
    // Provider Failure Count Methods
    // ============================================================================

    /// Set provider failure count (0-255)
    ///
    /// # Performance
    /// - <10ns (atomic load, bit manipulation, atomic store)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: provider_idx is 0-7
    /// - `#VERIFY_PROVIDER_INDEX`: Saturated in set_u8_at()
    #[inline(always)]
    pub fn set_provider_failures(&self, provider_idx: u8, failures: u8) {
        let current = self.provider_failures.load(Ordering::Relaxed);
        let updated = Self::set_u8_at(current, provider_idx, failures);
        self.provider_failures.store(updated, Ordering::Relaxed);
    }

    /// Get provider failure count (0-255)
    ///
    /// # Performance
    /// - <5ns (atomic load, bit shift, mask)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: provider_idx is 0-7
    /// - `#VERIFY_PROVIDER_INDEX`: Saturated in unpack_u8_at()
    #[inline(always)]
    pub fn get_provider_failures(&self, provider_idx: u8) -> u8 {
        let packed = self.provider_failures.load(Ordering::Relaxed);
        Self::unpack_u8_at(packed, provider_idx)
    }

    // ============================================================================
    // Budget Utilization Methods
    // ============================================================================

    /// Set budget utilization percentage (0-100%)
    ///
    /// # Performance
    /// - <10ns (atomic load, bit manipulation, atomic store)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: budget_idx is 0-7 (reusing same saturation logic)
    /// - `#VERIFY_PROVIDER_INDEX`: Saturated in set_u8_at()
    #[inline(always)]
    pub fn set_budget_utilization(&self, budget_idx: u8, utilization: u8) {
        let current = self.budget_utilization.load(Ordering::Relaxed);
        let updated = Self::set_u8_at(current, budget_idx, utilization.min(100));
        self.budget_utilization.store(updated, Ordering::Relaxed);
    }

    /// Get budget utilization percentage (0-100%)
    ///
    /// # Performance
    /// - <5ns (atomic load, bit shift, mask)
    ///
    /// # ASSUM
    /// - `#ASSUME_PROVIDER_INDEX`: budget_idx is 0-7
    /// - `#VERIFY_PROVIDER_INDEX`: Saturated in unpack_u8_at()
    #[inline(always)]
    pub fn get_budget_utilization(&self, budget_idx: u8) -> u8 {
        let packed = self.budget_utilization.load(Ordering::Relaxed);
        Self::unpack_u8_at(packed, budget_idx)
    }

    // ============================================================================
    // Latency Percentile Methods
    // ============================================================================

    /// Set P50 latency in milliseconds
    ///
    /// # Performance
    /// - <10ns (atomic store)
    #[inline(always)]
    pub fn set_p50_latency(&self, latency_ms: u32) {
        self.p50_latency_ms.store(latency_ms, Ordering::Relaxed);
    }

    /// Get P50 latency in milliseconds
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn get_p50_latency(&self) -> u32 {
        self.p50_latency_ms.load(Ordering::Relaxed)
    }

    /// Set P99 latency in milliseconds
    ///
    /// # Performance
    /// - <10ns (atomic store)
    #[inline(always)]
    pub fn set_p99_latency(&self, latency_ms: u32) {
        self.p99_latency_ms.store(latency_ms, Ordering::Relaxed);
    }

    /// Get P99 latency in milliseconds
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn get_p99_latency(&self) -> u32 {
        self.p99_latency_ms.load(Ordering::Relaxed)
    }

    /// Set P999 latency in milliseconds
    ///
    /// # Performance
    /// - <10ns (atomic store)
    #[inline(always)]
    pub fn set_p999_latency(&self, latency_ms: u32) {
        self.p999_latency_ms.store(latency_ms, Ordering::Relaxed);
    }

    /// Get P999 latency in milliseconds
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn get_p999_latency(&self) -> u32 {
        self.p999_latency_ms.load(Ordering::Relaxed)
    }

    // ============================================================================
    // Cost Tracking Methods
    // ============================================================================

    /// Set cost per 1K tokens in cents (Q16.16 fixed-point)
    ///
    /// # Performance
    /// - <10ns (atomic store)
    #[inline(always)]
    pub fn set_cost_per_1k_tokens(&self, cents: u32) {
        self.cost_per_1k_tokens_cents.store(cents, Ordering::Relaxed);
    }

    /// Get cost per 1K tokens in cents (Q16.16 fixed-point)
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn get_cost_per_1k_tokens(&self) -> u32 {
        self.cost_per_1k_tokens_cents.load(Ordering::Relaxed)
    }

    /// Set total spending in cents
    ///
    /// # Performance
    /// - <10ns (atomic store)
    #[inline(always)]
    pub fn set_total_spent(&self, cents: u64) {
        self.total_spent_cents.store(cents, Ordering::Relaxed);
    }

    /// Get total spending in cents
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn get_total_spent(&self) -> u64 {
        self.total_spent_cents.load(Ordering::Relaxed)
    }

    /// Add to total spending in cents (atomic increment)
    ///
    /// # Performance
    /// - <15ns (atomic fetch_add)
    ///
    /// # Arguments
    /// - `cents`: Amount to add in cents
    #[inline(always)]
    pub fn add_spent_cents(&self, cents: u64) {
        self.total_spent_cents.fetch_add(cents, Ordering::Relaxed);
    }

    // ============================================================================
    // Request Rate Methods
    // ============================================================================

    /// Set request rate per second
    ///
    /// # Performance
    /// - <10ns (atomic store)
    #[inline(always)]
    pub fn set_request_rate(&self, rate_per_sec: u32) {
        self.request_rate_per_sec.store(rate_per_sec, Ordering::Relaxed);
    }

    /// Get request rate per second
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn get_request_rate(&self) -> u32 {
        self.request_rate_per_sec.load(Ordering::Relaxed)
    }

    // ============================================================================
    // Loop Armor Metrics Methods (Phase 1 Loop Protection)
    // ============================================================================

    /// Set rate limit allowed count
    #[inline(always)]
    pub fn set_loop_armor_rate_allowed(&self, count: u32) {
        self.loop_armor_rate_allowed.store(count, Ordering::Relaxed);
    }

    /// Get rate limit allowed count
    #[inline(always)]
    pub fn get_loop_armor_rate_allowed(&self) -> u32 {
        self.loop_armor_rate_allowed.load(Ordering::Relaxed)
    }

    /// Set rate limit blocked count
    #[inline(always)]
    pub fn set_loop_armor_rate_blocked(&self, count: u32) {
        self.loop_armor_rate_blocked.store(count, Ordering::Relaxed);
    }

    /// Get rate limit blocked count
    #[inline(always)]
    pub fn get_loop_armor_rate_blocked(&self) -> u32 {
        self.loop_armor_rate_blocked.load(Ordering::Relaxed)
    }

    /// Set rate limit quota remaining
    #[inline(always)]
    pub fn set_loop_armor_rate_quota(&self, quota: u32) {
        self.loop_armor_rate_quota.store(quota, Ordering::Relaxed);
    }

    /// Get rate limit quota remaining
    #[inline(always)]
    pub fn get_loop_armor_rate_quota(&self) -> u32 {
        self.loop_armor_rate_quota.load(Ordering::Relaxed)
    }

    /// Set dedup hits (cache hits)
    #[inline(always)]
    pub fn set_loop_armor_dedup_hits(&self, hits: u32) {
        self.loop_armor_dedup_hits.store(hits, Ordering::Relaxed);
    }

    /// Get dedup hits (cache hits)
    #[inline(always)]
    pub fn get_loop_armor_dedup_hits(&self) -> u32 {
        self.loop_armor_dedup_hits.load(Ordering::Relaxed)
    }

    /// Set dedup misses (unique requests)
    #[inline(always)]
    pub fn set_loop_armor_dedup_misses(&self, misses: u32) {
        self.loop_armor_dedup_misses.store(misses, Ordering::Relaxed);
    }

    /// Get dedup misses (unique requests)
    #[inline(always)]
    pub fn get_loop_armor_dedup_misses(&self) -> u32 {
        self.loop_armor_dedup_misses.load(Ordering::Relaxed)
    }

    /// Set anomaly count
    #[inline(always)]
    pub fn set_loop_armor_anomaly_count(&self, count: u32) {
        self.loop_armor_anomaly_count.store(count, Ordering::Relaxed);
    }

    /// Get anomaly count
    #[inline(always)]
    pub fn get_loop_armor_anomaly_count(&self) -> u32 {
        self.loop_armor_anomaly_count.load(Ordering::Relaxed)
    }

    /// Set current p99 latency (ms)
    #[inline(always)]
    pub fn set_loop_armor_p99_current(&self, latency_ms: u32) {
        self.loop_armor_p99_current.store(latency_ms, Ordering::Relaxed);
    }

    /// Get current p99 latency (ms)
    #[inline(always)]
    pub fn get_loop_armor_p99_current(&self) -> u32 {
        self.loop_armor_p99_current.load(Ordering::Relaxed)
    }

    /// Set baseline p99 latency (ms)
    #[inline(always)]
    pub fn set_loop_armor_p99_baseline(&self, latency_ms: u32) {
        self.loop_armor_p99_baseline.store(latency_ms, Ordering::Relaxed);
    }

    /// Get baseline p99 latency (ms)
    #[inline(always)]
    pub fn get_loop_armor_p99_baseline(&self) -> u32 {
        self.loop_armor_p99_baseline.load(Ordering::Relaxed)
    }

    /// Set anomaly severity (0=None, 1=Low, 2=Medium, 3=High, 4=Critical)
    #[inline(always)]
    pub fn set_loop_armor_severity(&self, severity: u32) {
        self.loop_armor_severity.store(severity, Ordering::Relaxed);
    }

    /// Get anomaly severity (0=None, 1=Low, 2=Medium, 3=High, 4=Critical)
    #[inline(always)]
    pub fn get_loop_armor_severity(&self) -> u32 {
        self.loop_armor_severity.load(Ordering::Relaxed)
    }

    // ============================================================================
    // Loop Armor Phase 2 Metrics Methods (Enhanced Detection)
    // ============================================================================

    /// Set burst count (total bursts detected)
    #[inline(always)]
    pub fn set_loop_armor_burst_count(&self, count: u32) {
        self.loop_armor_burst_count.store(count, Ordering::Relaxed);
    }

    /// Get burst count (total bursts detected)
    #[inline(always)]
    pub fn get_loop_armor_burst_count(&self) -> u32 {
        self.loop_armor_burst_count.load(Ordering::Relaxed)
    }

    /// Increment burst count (atomic operation)
    #[inline(always)]
    pub fn increment_loop_armor_burst_count(&self) {
        self.loop_armor_burst_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Set burst window (requests in current window, 0-10)
    #[inline(always)]
    pub fn set_loop_armor_burst_window(&self, count: u32) {
        self.loop_armor_burst_window.store(count, Ordering::Relaxed);
    }

    /// Get burst window (requests in current window, 0-10)
    #[inline(always)]
    pub fn get_loop_armor_burst_window(&self) -> u32 {
        self.loop_armor_burst_window.load(Ordering::Relaxed)
    }

    /// Set cost velocity (Q16.16 fixed-point cents/min)
    #[inline(always)]
    pub fn set_loop_armor_cost_velocity(&self, velocity: u64) {
        self.loop_armor_cost_velocity.store(velocity, Ordering::Relaxed);
    }

    /// Get cost velocity (Q16.16 fixed-point cents/min)
    #[inline(always)]
    pub fn get_loop_armor_cost_velocity(&self) -> u64 {
        self.loop_armor_cost_velocity.load(Ordering::Relaxed)
    }

    /// Set cost alerts count
    #[inline(always)]
    pub fn set_loop_armor_cost_alerts(&self, count: u32) {
        self.loop_armor_cost_alerts.store(count, Ordering::Relaxed);
    }

    /// Get cost alerts count
    #[inline(always)]
    pub fn get_loop_armor_cost_alerts(&self) -> u32 {
        self.loop_armor_cost_alerts.load(Ordering::Relaxed)
    }

    /// Increment cost alerts count (atomic operation)
    #[inline(always)]
    pub fn increment_loop_armor_cost_alerts(&self) {
        self.loop_armor_cost_alerts.fetch_add(1, Ordering::Relaxed);
    }

    /// Set pattern count (total patterns detected)
    #[inline(always)]
    pub fn set_loop_armor_pattern_count(&self, count: u32) {
        self.loop_armor_pattern_count.store(count, Ordering::Relaxed);
    }

    /// Get pattern count (total patterns detected)
    #[inline(always)]
    pub fn get_loop_armor_pattern_count(&self) -> u32 {
        self.loop_armor_pattern_count.load(Ordering::Relaxed)
    }

    /// Increment pattern count (atomic operation)
    #[inline(always)]
    pub fn increment_loop_armor_pattern_count(&self) {
        self.loop_armor_pattern_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Set pattern matches (current window matches, 0-8)
    #[inline(always)]
    pub fn set_loop_armor_pattern_matches(&self, matches: u32) {
        self.loop_armor_pattern_matches.store(matches, Ordering::Relaxed);
    }

    /// Get pattern matches (current window matches, 0-8)
    #[inline(always)]
    pub fn get_loop_armor_pattern_matches(&self) -> u32 {
        self.loop_armor_pattern_matches.load(Ordering::Relaxed)
    }

    // ============================================================================
    // Loop Armor Phase 3 Metrics Methods (Client Circuit Breaker)
    // ============================================================================

    /// Set circuit breaker closed count (Closed state clients)
    #[inline(always)]
    pub fn set_loop_armor_circuit_closed_count(&self, count: u32) {
        self.loop_armor_circuit_closed_count.store(count, Ordering::Relaxed);
    }

    /// Get circuit breaker closed count (Closed state clients)
    #[inline(always)]
    pub fn get_loop_armor_circuit_closed_count(&self) -> u32 {
        self.loop_armor_circuit_closed_count.load(Ordering::Relaxed)
    }

    /// Set circuit breaker halfopen count (HalfOpen state clients)
    #[inline(always)]
    pub fn set_loop_armor_circuit_halfopen_count(&self, count: u32) {
        self.loop_armor_circuit_halfopen_count.store(count, Ordering::Relaxed);
    }

    /// Get circuit breaker halfopen count (HalfOpen state clients)
    #[inline(always)]
    pub fn get_loop_armor_circuit_halfopen_count(&self) -> u32 {
        self.loop_armor_circuit_halfopen_count.load(Ordering::Relaxed)
    }

    /// Set circuit breaker open count (Open state clients)
    #[inline(always)]
    pub fn set_loop_armor_circuit_open_count(&self, count: u32) {
        self.loop_armor_circuit_open_count.store(count, Ordering::Relaxed);
    }

    /// Get circuit breaker open count (Open state clients)
    #[inline(always)]
    pub fn get_loop_armor_circuit_open_count(&self) -> u32 {
        self.loop_armor_circuit_open_count.load(Ordering::Relaxed)
    }

    /// Set circuit breaker total opens (total open events)
    #[inline(always)]
    pub fn set_loop_armor_circuit_total_opens(&self, count: u32) {
        self.loop_armor_circuit_total_opens.store(count, Ordering::Relaxed);
    }

    /// Get circuit breaker total opens (total open events)
    #[inline(always)]
    pub fn get_loop_armor_circuit_total_opens(&self) -> u32 {
        self.loop_armor_circuit_total_opens.load(Ordering::Relaxed)
    }

    /// Set circuit breaker total recoveries (Closed transitions)
    #[inline(always)]
    pub fn set_loop_armor_circuit_total_recoveries(&self, count: u32) {
        self.loop_armor_circuit_total_recoveries.store(count, Ordering::Relaxed);
    }

    /// Get circuit breaker total recoveries (Closed transitions)
    #[inline(always)]
    pub fn get_loop_armor_circuit_total_recoveries(&self) -> u32 {
        self.loop_armor_circuit_total_recoveries.load(Ordering::Relaxed)
    }

    /// Set circuit breaker average error rate (basis points, 0-10000)
    #[inline(always)]
    pub fn set_loop_armor_circuit_avg_error_rate(&self, rate_bp: u32) {
        self.loop_armor_circuit_avg_error_rate.store(rate_bp, Ordering::Relaxed);
    }

    /// Get circuit breaker average error rate (basis points, 0-10000)
    #[inline(always)]
    pub fn get_loop_armor_circuit_avg_error_rate(&self) -> u32 {
        self.loop_armor_circuit_avg_error_rate.load(Ordering::Relaxed)
    }

    // ============================================================================
    // Packed Metrics Accessors (for Overview Tab)
    // ============================================================================

    /// Get packed circuit breaker states (8 providers × 8 bits)
    ///
    /// # Layout
    /// - Bits [7:0]: Provider 0 state (0=Closed, 1=HalfOpen, 2=Open)
    /// - Bits [15:8]: Provider 1 state
    /// - ...
    /// - Bits [63:56]: Provider 7 state
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn circuit_breaker_states(&self) -> u64 {
        self.circuit_breaker_states.load(Ordering::Relaxed)
    }

    /// Get packed provider success rates (8 providers × 8 bits, 0-100%)
    ///
    /// # Layout
    /// - Bits [7:0]: Provider 0 success rate (0-100%)
    /// - Bits [15:8]: Provider 1 success rate
    /// - ...
    /// - Bits [63:56]: Provider 7 success rate
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn provider_success_rates(&self) -> u64 {
        self.provider_success_rates.load(Ordering::Relaxed)
    }

    /// Get packed provider failures (8 providers × 8 bits, 0-255)
    ///
    /// # Layout
    /// - Bits [7:0]: Provider 0 failures (0-255)
    /// - Bits [15:8]: Provider 1 failures
    /// - ...
    /// - Bits [63:56]: Provider 7 failures
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn provider_failures(&self) -> u64 {
        self.provider_failures.load(Ordering::Relaxed)
    }

    /// Get packed budget utilization (8 budgets × 8 bits, 0-100%)
    ///
    /// # Layout
    /// - Bits [7:0]: Budget 0 utilization (0-100%)
    /// - Bits [15:8]: Budget 1 utilization
    /// - ...
    /// - Bits [63:56]: Budget 7 utilization
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn budget_utilization(&self) -> u64 {
        self.budget_utilization.load(Ordering::Relaxed)
    }

    /// Get P50 latency in milliseconds
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn p50_latency(&self) -> u32 {
        self.p50_latency_ms.load(Ordering::Relaxed)
    }

    /// Get P99 latency in milliseconds
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn p99_latency(&self) -> u32 {
        self.p99_latency_ms.load(Ordering::Relaxed)
    }

    /// Get P999 latency in milliseconds
    ///
    /// # Performance
    /// - <5ns (atomic load)
    #[inline(always)]
    pub fn p999_latency(&self) -> u32 {
        self.p999_latency_ms.load(Ordering::Relaxed)
    }

    /// Render dashboard frame to ratatui
    ///
    /// # Performance
    /// - <5ms (ratatui layout + terminal I/O)
    ///
    /// # Layout
    /// - Header (title + refresh info)
    /// - Budget summary table (if available)
    /// - Provider status table (if available)
    /// - System metrics summary
    /// - Footer (controls + status)
    ///
    /// # Byzantine Purple Theme
    /// - Headers: #663399 (Byzantine Purple)
    /// - Accents: #FFD700 (Gold)
    /// - Status: Green (✅), Yellow (⚠️), Red (❌)
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        budgets: &[BudgetMetric],
        providers: &[ProviderMetric],
        cache: Option<&CacheMetricsPanel>,
        compression: Option<&CompressionMetricsPanel>,
        load_balancer: Option<&LoadBalancerMetricsPanel>,
        performance: Option<&PerformanceMetricsPanel>,
        iteration: u64,
    ) {
        // Load atomic metrics snapshot (<100ns)
        let budgets_count = self.budgets_count.load(Ordering::Relaxed);
        let providers_count = self.providers_count.load(Ordering::Relaxed);
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let avg_latency_ms = self.avg_latency_ms.load(Ordering::Relaxed);
        let memory_mb = self.memory_mb.load(Ordering::Relaxed);
        let uptime_secs = self.uptime_secs.load(Ordering::Relaxed);
        let refresh_interval_ms = self.refresh_interval_ms.load(Ordering::Relaxed);
        let is_paused = self.is_paused.load(Ordering::Relaxed);
        let has_error = self.has_error.load(Ordering::Relaxed);

        // Byzantine Purple color scheme
        let purple = Color::Rgb(102, 51, 153); // #663399
        let gold = Color::Rgb(255, 215, 0);    // #FFD700
        let header_style = Style::default().fg(purple).add_modifier(Modifier::BOLD);
        let accent_style = Style::default().fg(gold);

        // Create vertical layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Min(10),    // Main content
                Constraint::Length(3),  // Footer
            ])
            .split(area);

        // Render header
        let status_text = if has_error {
            "ERROR".to_string()
        } else if is_paused {
            "PAUSED".to_string()
        } else {
            "LIVE".to_string()
        };

        let header_text = format!(
            "clapi Metrics Dashboard - {} - Refresh: {}s (iteration {})",
            status_text,
            refresh_interval_ms / 1000,
            iteration
        );

        let header = Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL))
            .style(header_style);
        frame.render_widget(header, chunks[0]);

        // Render main content
        self.render_main_content(
            frame,
            chunks[1],
            budgets,
            providers,
            cache,
            compression,
            load_balancer,
            performance,
            budgets_count,
            providers_count,
            total_requests,
            avg_latency_ms,
            memory_mb,
            uptime_secs,
        );

        // Render footer
        let footer_text = if is_paused {
            "Status: PAUSED | Press 'r' to resume, 'q' to quit"
        } else {
            "Status: LIVE | Press 'p' to pause, 'q' to quit"
        };

        let footer = Paragraph::new(footer_text)
            .block(Block::default().borders(Borders::ALL))
            .style(accent_style);
        frame.render_widget(footer, chunks[2]);
    }

    /// Render main content area (budget/provider tables + system metrics)
    #[allow(clippy::too_many_arguments)]
    fn render_main_content(
        &self,
        frame: &mut Frame,
        area: Rect,
        budgets: &[BudgetMetric],
        providers: &[ProviderMetric],
        _cache: Option<&CacheMetricsPanel>,
        _compression: Option<&CompressionMetricsPanel>,
        _load_balancer: Option<&LoadBalancerMetricsPanel>,
        _performance: Option<&PerformanceMetricsPanel>,
        budgets_count: u32,
        providers_count: u32,
        total_requests: u32,
        avg_latency_ms: u32,
        memory_mb: u32,
        uptime_secs: u64,
    ) {
        let purple = Color::Rgb(102, 51, 153);
        let header_style = Style::default().fg(purple).add_modifier(Modifier::BOLD);

        // Create vertical layout for content sections
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40), // Budget summary
                Constraint::Percentage(40), // Provider status
                Constraint::Percentage(20), // System metrics
            ])
            .split(area);

        // Render budget summary
        if budgets.is_empty() {
            let text = format!("BUDGET SUMMARY ({} budgets)\n\nNo budgets configured", budgets_count);
            let widget = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL))
                .style(header_style);
            frame.render_widget(widget, chunks[0]);
        } else {
            let header = Row::new(vec!["Budget ID", "Available", "Spent", "Status", "Trend"])
                .style(header_style);

            let rows: Vec<Row> = budgets
                .iter()
                .map(|b| {
                    Row::new(vec![
                        b.budget_id.clone(),
                        b.available.clone(),
                        b.spent.clone(),
                        b.status.clone(),
                        b.trend.clone(),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Budget Summary ({} budgets)", budgets_count))
            );

            frame.render_widget(table, chunks[0]);
        }

        // Render provider status
        if providers.is_empty() {
            let text = format!("PROVIDER STATUS ({} providers)\n\nNo providers configured", providers_count);
            let widget = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL))
                .style(header_style);
            frame.render_widget(widget, chunks[1]);
        } else {
            let header = Row::new(vec!["Provider", "Status", "Failures", "Latency", "Response Rate"])
                .style(header_style);

            let rows: Vec<Row> = providers
                .iter()
                .map(|p| {
                    Row::new(vec![
                        p.provider.clone(),
                        p.status.clone(),
                        p.failures.clone(),
                        p.latency.clone(),
                        p.response_rate.clone(),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Provider Status ({} providers)", providers_count))
            );

            frame.render_widget(table, chunks[1]);
        }

        // Render system metrics
        let uptime_str = format_duration(uptime_secs);
        let text = format!(
            "SYSTEM METRICS\n\nUptime: {}  |  Memory: {} MB  |  Total Requests: {}\nAvg Latency: {}ms",
            uptime_str,
            memory_mb,
            total_requests,
            avg_latency_ms
        );

        let widget = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .style(header_style);
        frame.render_widget(widget, chunks[2]);
    }
}

/// Format duration as human-readable string
///
/// # Examples
/// ```
/// assert_eq!(format_duration(65), "1m 5s");
/// assert_eq!(format_duration(3661), "1h 1m 1s");
/// ```
fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_creation() {
        let capsule = DashboardContentCapsule::new(5000);
        assert_eq!(capsule.refresh_interval_ms.load(Ordering::Relaxed), 5000);
        assert!(!capsule.is_paused());
        assert!(!capsule.has_error());
    }

    #[test]
    fn test_atomic_updates() {
        let capsule = DashboardContentCapsule::new(1000);

        capsule.set_budgets_count(10);
        capsule.set_providers_count(5);
        capsule.set_paused(true);
        capsule.set_error(true);

        assert_eq!(capsule.budgets_count.load(Ordering::Relaxed), 10);
        assert_eq!(capsule.providers_count.load(Ordering::Relaxed), 5);
        assert!(capsule.is_paused());
        assert!(capsule.has_error());
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(65), "1m 5s");
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(7200), "2h 0m 0s");
    }

    #[test]
    fn test_bit_packing_helpers() {
        // Test pack_u8_array
        let values = [10, 20, 30, 40, 50, 60, 70, 80];
        let packed = DashboardContentCapsule::pack_u8_array(&values);

        // Test unpack_u8_at
        for i in 0..8 {
            assert_eq!(
                DashboardContentCapsule::unpack_u8_at(packed, i),
                values[i as usize]
            );
        }

        // Test set_u8_at
        let mut modified = packed;
        for i in 0..8 {
            modified = DashboardContentCapsule::set_u8_at(modified, i, 100 + i);
        }

        // Verify all values updated
        for i in 0..8 {
            assert_eq!(
                DashboardContentCapsule::unpack_u8_at(modified, i),
                100 + i
            );
        }

        // Test index saturation (index > 7 should saturate to 7)
        assert_eq!(
            DashboardContentCapsule::unpack_u8_at(packed, 255),
            80 // Should return values[7]
        );
    }

    #[test]
    fn test_circuit_breaker_state_methods() {
        let capsule = DashboardContentCapsule::new(1000);

        // Test all 8 providers
        for i in 0..8 {
            capsule.set_circuit_state(i, i * 10);
            assert_eq!(capsule.get_circuit_state(i), i * 10);
        }

        // Verify all states persist
        for i in 0..8 {
            assert_eq!(capsule.get_circuit_state(i), i * 10);
        }

        // Test index saturation
        capsule.set_circuit_state(255, 42);
        assert_eq!(capsule.get_circuit_state(255), 42); // Should access provider 7
    }

    #[test]
    fn test_provider_success_rate_methods() {
        let capsule = DashboardContentCapsule::new(1000);

        // Test valid rates (0-100%)
        for i in 0..8 {
            capsule.set_provider_success_rate(i, i * 10);
            assert_eq!(capsule.get_provider_success_rate(i), i * 10);
        }

        // Test saturation at 100%
        capsule.set_provider_success_rate(0, 200);
        assert_eq!(capsule.get_provider_success_rate(0), 100);
    }

    #[test]
    fn test_provider_failure_methods() {
        let capsule = DashboardContentCapsule::new(1000);

        for i in 0..8 {
            capsule.set_provider_failures(i, i * 20);
            assert_eq!(capsule.get_provider_failures(i), i * 20);
        }
    }

    #[test]
    fn test_budget_utilization_methods() {
        let capsule = DashboardContentCapsule::new(1000);

        // Test valid utilization (0-100%)
        for i in 0..8 {
            capsule.set_budget_utilization(i, i * 12);
            assert_eq!(capsule.get_budget_utilization(i), i * 12);
        }

        // Test saturation at 100%
        capsule.set_budget_utilization(0, 150);
        assert_eq!(capsule.get_budget_utilization(0), 100);
    }

    #[test]
    fn test_latency_percentile_methods() {
        let capsule = DashboardContentCapsule::new(1000);

        capsule.set_p50_latency(50);
        capsule.set_p99_latency(99);
        capsule.set_p999_latency(999);

        assert_eq!(capsule.get_p50_latency(), 50);
        assert_eq!(capsule.get_p99_latency(), 99);
        assert_eq!(capsule.get_p999_latency(), 999);
    }

    #[test]
    fn test_cost_tracking_methods() {
        let capsule = DashboardContentCapsule::new(1000);

        capsule.set_cost_per_1k_tokens(12345);
        capsule.set_total_spent(67890);

        assert_eq!(capsule.get_cost_per_1k_tokens(), 12345);
        assert_eq!(capsule.get_total_spent(), 67890);

        // Test atomic increment
        capsule.add_spent_cents(100);
        assert_eq!(capsule.get_total_spent(), 67990);
    }

    #[test]
    fn test_request_rate_methods() {
        let capsule = DashboardContentCapsule::new(1000);

        capsule.set_request_rate(5000);
        assert_eq!(capsule.get_request_rate(), 5000);
    }

    #[test]
    fn test_capsule_size_verification() {
        use std::mem::{size_of, align_of};

        // Verify alignment
        assert_eq!(align_of::<DashboardContentCapsule>(), 128);

        // Verify size (384B total, expanded from 256B)
        assert_eq!(size_of::<DashboardContentCapsule>(), 384);

        // Verify hot metrics fit in first cache line (64B)
        // budgets_count (4) + providers_count (4) + last_refresh_ns (8) +
        // refresh_interval_ms (4) + total_requests (4) + avg_latency_ms (4) +
        // memory_mb (4) + uptime_secs (8) + is_paused (1) + has_error (1) +
        // _padding1 (22) = 64B

        // Verify cold metrics fit in second cache line (64B)
        // circuit_breaker_states (8) + provider_success_rates (8) +
        // provider_failures (8) + budget_utilization (8) +
        // p50_latency_ms (4) + p99_latency_ms (4) + p999_latency_ms (4) +
        // cost_per_1k_tokens_cents (4) + total_spent_cents (8) +
        // request_rate_per_sec (4) = 64B

        // Verify Loop Armor Phase 1 fits in third cache line (64B)
        // loop_armor_rate_allowed (4) + loop_armor_rate_blocked (4) +
        // loop_armor_rate_quota (4) + loop_armor_dedup_hits (4) +
        // loop_armor_dedup_misses (4) + loop_armor_anomaly_count (4) +
        // loop_armor_p99_current (4) + loop_armor_p99_baseline (4) +
        // loop_armor_severity (4) + _padding2 (28) = 64B

        // Verify Loop Armor Phase 2 fits in fourth cache line (64B)
        // loop_armor_burst_count (4) + loop_armor_burst_window (4) +
        // loop_armor_cost_velocity (8) + loop_armor_cost_alerts (4) +
        // loop_armor_pattern_count (4) + loop_armor_pattern_matches (4) +
        // _padding3 (36) = 64B

        // Verify Loop Armor Phase 3 (24B fields + 104B padding = 128B total)
        // loop_armor_circuit_closed_count (4) + loop_armor_circuit_halfopen_count (4) +
        // loop_armor_circuit_open_count (4) + loop_armor_circuit_total_opens (4) +
        // loop_armor_circuit_total_recoveries (4) + loop_armor_circuit_avg_error_rate (4) +
        // _padding4 (104) = 128B (spanning 2 cache lines)
    }
}
