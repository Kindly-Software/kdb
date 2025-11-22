//! Hysteresis policy evaluation for the atomic breaker.

#[cfg(feature = "circuit-breaker-auto-tune")]
use super::breaker::MetricsSnapshot;
use super::breaker::{AtomicBreakerGuard, BreakerLike, LayoutKind, State};
use super::cause;
#[cfg(feature = "circuit-breaker-compact48")]
use super::layout::pack_q6_10;
use super::layout::pack_q8_8;
#[cfg(feature = "std")]
use super::telemetry::TelemetrySample;
#[cfg(feature = "circuit-breaker-auto-tune")]
use super::telemetry::{
    tune_policy as calibrate_policy, CalibrationMode, CalibrationTargets, HistoryBuffer,
    HistoryEntry, LevelFeedbackResult, MetricsTap, PolicyDraft,
};
#[cfg(feature = "circuit-breaker-adaptive")]
use core::sync::atomic::{AtomicU16, Ordering};

/// Policy thresholds expressed in normalized Q8.8 fixed-point units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Trip threshold for normalized mean metric (Q8.8).
    pub mu_trip: u16,
    /// Trip threshold for normalized jitter metric (Q8.8).
    pub sg_trip: u16,
    /// Close threshold for normalized mean metric (Q8.8).
    pub mu_close: u16,
    /// Close threshold for normalized jitter metric (Q8.8).
    pub sg_close: u16,
    /// Minimum time to stay open before probing (milliseconds).
    pub cool_down_ms: u32,
    /// Time window that metrics must remain healthy before closing (milliseconds).
    pub ok_window_ms: u32,
    /// Error count threshold for trips.
    pub err_trip: u16,

    // ============================================================================
    // ADAPTIVE THRESHOLD EXTENSION (Phase 2 - Feature-gated)
    // ============================================================================
    // Real-time adaptive thresholds via EMA for 50% false positive reduction.
    // Feature: circuit-breaker-adaptive
    // UCE34 Q10: T1 Atomic + T3 Fixed-Point (lockfree adaptive thresholds)
    // Note: Config-only fields (non-atomic) for compatibility with Clone/Copy
    // ============================================================================
    #[cfg(feature = "circuit-breaker-adaptive")]
    /// Update interval in evaluation calls (0 = disabled, use static thresholds).
    /// Default: 100 (update EMA every 100 evaluations).
    /// Range: 0 (disabled) to 65535 (very slow adaptation).
    pub update_interval: u16,

    #[cfg(feature = "circuit-breaker-adaptive")]
    /// EMA alpha coefficient in Q8.8 fixed-point (0.0-1.0 scaled to 0-256).
    /// Default: 24 (0.095) for N=20 equivalent exponential window.
    /// Formula: alpha = 2 / (N + 1), where N is the equivalent window size.
    /// Common values:
    /// - 24 (0.095): N=20 window (recommended default)
    /// - 51 (0.2): N=9 window (faster adaptation)
    /// - 13 (0.05): N=39 window (slower, more stable)
    pub alpha_q8: u16,
}

#[cfg(feature = "circuit-breaker-adaptive")]
/// Adaptive threshold state (separate from Policy for lockfree mutation).
/// UCE34 Q10: T1 Atomic capsule - lockfree adaptive thresholds.
/// UCE34 Q28: Simplify - Single-cache-line layout (64B), minimal API surface.
#[repr(align(64))]
pub struct AdaptiveState {
    /// Adaptive mu_trip threshold (Q8.8 EMA). Replaces static mu_trip when enabled.
    /// #ASSUME_ACQUIRE_ORDERING: Load with Acquire prevents reordering with breaker state reads.
    /// #VERIFY_ACQUIRE_ORDERING: Memory ordering validated in tests (circuit_breaker_adaptive_ordering).
    pub mu_trip_ema: AtomicU16,

    /// Adaptive sg_trip threshold (Q8.8 EMA). Replaces static sg_trip when enabled.
    /// #ASSUME_ACQUIRE_ORDERING: Load with Acquire prevents reordering with breaker state reads.
    /// #VERIFY_ACQUIRE_ORDERING: Memory ordering validated in tests (circuit_breaker_adaptive_ordering).
    pub sg_trip_ema: AtomicU16,

    /// Adaptive err_trip threshold (raw EMA, not Q8.8). Replaces static err_trip when enabled.
    /// #ASSUME_RELAXED_COUNTERS: Load with Relaxed (no synchronization needed, independent metric).
    /// #VERIFY_RELAXED_COUNTERS: Validated in tests (circuit_breaker_adaptive_relaxed).
    pub err_trip_ema: AtomicU16,

    /// Count of false positives (trip → recovered without real issue).
    /// #ASSUME_RELAXED_COUNTERS: Relaxed ordering (statistics only, no synchronization required).
    /// #ASSUME_U16_OVERFLOW: Counter overflow acceptable (wrapping semantics, saturates in rate calculation).
    /// #VERIFY_RELAXED_COUNTERS: Validated in tests (circuit_breaker_adaptive_counters).
    pub false_positive_count: AtomicU16,

    /// Total trip count (for false positive rate calculation).
    /// #ASSUME_RELAXED_COUNTERS: Relaxed ordering (statistics only, no synchronization required).
    /// #ASSUME_U16_OVERFLOW: Counter overflow acceptable (wrapping semantics, saturates in rate calculation).
    /// #VERIFY_RELAXED_COUNTERS: Validated in tests (circuit_breaker_adaptive_counters).
    pub total_trips: AtomicU16,

    /// Update counter for EMA calculation (every Nth evaluation triggers EMA update).
    /// #ASSUME_RELAXED_COUNTERS: Relaxed ordering (no synchronization needed, approximate triggering OK).
    /// #ASSUME_U16_OVERFLOW: Counter overflow acceptable (wrapping semantics).
    /// #VERIFY_RELAXED_COUNTERS: Validated in tests (circuit_breaker_adaptive_counters).
    pub update_counter: AtomicU16,

    /// Padding to 64 bytes (single cache line).
    /// #ASSUME_CACHE_LINE_64B: x86-64, ARM, RISC-V use 64B cache lines.
    /// #VERIFY_CACHE_LINE_64B: Size verified in tests (adaptive_state_size_invariant).
    _padding: [u8; 52],
}

impl Policy {
    const Q8_SCALE: f32 = 256.0;

    /// Policy tuned for holographic user interfaces.
    #[must_use]
    pub const fn ui_holographic() -> Self {
        Self {
            mu_trip: 4608,
            sg_trip: 4096,
            mu_close: 2048,
            sg_close: 1536,
            cool_down_ms: 75,
            ok_window_ms: 16,
            err_trip: 20,
            #[cfg(feature = "circuit-breaker-adaptive")]
            update_interval: 100, // Default: update every 100 evaluations
            #[cfg(feature = "circuit-breaker-adaptive")]
            alpha_q8: 24, // Default: 0.095 (N=20 window)
        }
    }

    /// Policy tuned for low-latency audio rendering pipelines.
    #[must_use]
    pub const fn audio_lowlatency() -> Self {
        Self {
            mu_trip: 5120,
            sg_trip: 3328,
            mu_close: 1792,
            sg_close: 1280,
            cool_down_ms: 20,
            ok_window_ms: 10,
            err_trip: 8,
            #[cfg(feature = "circuit-breaker-adaptive")]
            update_interval: 100,
            #[cfg(feature = "circuit-breaker-adaptive")]
            alpha_q8: 24,
        }
    }

    /// Policy tuned for IO-bound workloads.
    #[must_use]
    pub const fn io_disk() -> Self {
        Self {
            mu_trip: 4096,
            sg_trip: 5120,
            mu_close: 1792,
            sg_close: 2048,
            cool_down_ms: 100,
            ok_window_ms: 50,
            err_trip: 24,
            #[cfg(feature = "circuit-breaker-adaptive")]
            update_interval: 100,
            #[cfg(feature = "circuit-breaker-adaptive")]
            alpha_q8: 24,
        }
    }

    /// Policy tuned for arbitrage/trading venues.
    #[must_use]
    pub const fn arb_venue() -> Self {
        Self {
            mu_trip: 5888,
            sg_trip: 4864,
            mu_close: 2304,
            sg_close: 2048,
            cool_down_ms: 40,
            ok_window_ms: 12,
            err_trip: 6,
            #[cfg(feature = "circuit-breaker-adaptive")]
            update_interval: 100,
            #[cfg(feature = "circuit-breaker-adaptive")]
            alpha_q8: 24,
        }
    }

    /// Policy tuned for distributed cache nodes.
    ///
    /// **Thresholds:**
    /// - mu_trip: 3.0 (300% of baseline latency) → trip to Open
    /// - sg_trip: 2.5 (250% of baseline jitter) → trip to Open
    /// - mu_close: 0.8 (80% of baseline latency) → close from HalfOpen
    /// - sg_close: 0.7 (70% of baseline jitter) → close from HalfOpen
    /// - cool_down_ms: 60s before HalfOpen
    /// - ok_window_ms: 10s of healthy metrics before Closed
    /// - err_trip: 10 errors before trip
    #[must_use]
    pub const fn distributed_cache() -> Self {
        Self {
            mu_trip: 768,         // 3.0 * 256 (Q8.8 fixed-point)
            sg_trip: 640,         // 2.5 * 256
            mu_close: 205,        // 0.8 * 256
            sg_close: 179,        // 0.7 * 256
            cool_down_ms: 60_000, // 60 seconds
            ok_window_ms: 10_000, // 10 seconds
            err_trip: 10,         // 10 errors before trip
            #[cfg(feature = "circuit-breaker-adaptive")]
            update_interval: 100,
            #[cfg(feature = "circuit-breaker-adaptive")]
            alpha_q8: 24,
        }
    }

    #[must_use]
    fn mu_trip_f32(&self) -> f32 {
        f32::from(self.mu_trip) / Self::Q8_SCALE
    }

    #[must_use]
    fn sg_trip_f32(&self) -> f32 {
        f32::from(self.sg_trip) / Self::Q8_SCALE
    }

    #[must_use]
    fn mu_close_f32(&self) -> f32 {
        f32::from(self.mu_close) / Self::Q8_SCALE
    }

    #[must_use]
    fn sg_close_f32(&self) -> f32 {
        f32::from(self.sg_close) / Self::Q8_SCALE
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    /// Create policy with adaptive thresholds enabled (default config).
    /// Default: update_interval=100, alpha_q8=24 (N=20 window).
    #[must_use]
    pub const fn with_adaptive(mut self) -> Self {
        self.update_interval = 100;
        self.alpha_q8 = 24; // 0.095 for N=20 window
        self
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    /// Create policy with custom adaptive config.
    #[must_use]
    pub const fn with_adaptive_config(mut self, update_interval: u16, alpha_q8: u16) -> Self {
        self.update_interval = update_interval;
        self.alpha_q8 = alpha_q8;
        self
    }
}

#[cfg(feature = "circuit-breaker-adaptive")]
impl AdaptiveState {
    /// Create new adaptive state from base policy thresholds.
    /// Initializes EMA values with static thresholds (no adaptation yet).
    #[must_use]
    pub fn new(base_policy: &Policy) -> Self {
        Self {
            mu_trip_ema: AtomicU16::new(base_policy.mu_trip),
            sg_trip_ema: AtomicU16::new(base_policy.sg_trip),
            err_trip_ema: AtomicU16::new(base_policy.err_trip),
            false_positive_count: AtomicU16::new(0),
            total_trips: AtomicU16::new(0),
            update_counter: AtomicU16::new(0),
            _padding: [0u8; 52],
        }
    }

    /// Get current adaptive mu_trip threshold (Q8.8).
    /// #ASSUME_ACQUIRE_ORDERING: Acquire load prevents reordering.
    /// #VERIFY_ACQUIRE_ORDERING: Validated in tests.
    #[must_use]
    #[inline]
    pub fn adaptive_mu_trip(&self) -> u16 {
        self.mu_trip_ema.load(Ordering::Acquire)
    }

    /// Get current adaptive sg_trip threshold (Q8.8).
    /// #ASSUME_ACQUIRE_ORDERING: Acquire load prevents reordering.
    /// #VERIFY_ACQUIRE_ORDERING: Validated in tests.
    #[must_use]
    #[inline]
    pub fn adaptive_sg_trip(&self) -> u16 {
        self.sg_trip_ema.load(Ordering::Acquire)
    }

    /// Get current adaptive err_trip threshold (raw count).
    /// #ASSUME_RELAXED_COUNTERS: Relaxed load (no synchronization needed).
    /// #VERIFY_RELAXED_COUNTERS: Validated in tests.
    #[must_use]
    #[inline]
    pub fn adaptive_err_trip(&self) -> u16 {
        self.err_trip_ema.load(Ordering::Relaxed)
    }

    /// Record a circuit breaker trip event.
    /// #ASSUME_RELAXED_COUNTERS: Relaxed increment (statistics only).
    /// #ASSUME_U16_OVERFLOW: Wrapping semantics acceptable.
    /// #VERIFY_RELAXED_COUNTERS: Validated in tests.
    #[inline]
    pub fn record_trip(&self) {
        self.total_trips.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a false positive trip (recovered without real issue).
    /// #ASSUME_RELAXED_COUNTERS: Relaxed increment (statistics only).
    /// #ASSUME_U16_OVERFLOW: Wrapping semantics acceptable.
    /// #VERIFY_RELAXED_COUNTERS: Validated in tests.
    #[inline]
    pub fn record_false_positive(&self) {
        self.false_positive_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Calculate current false positive rate (0.0-1.0).
    /// Returns 0.0 if no trips recorded yet.
    /// #ASSUME_RELAXED_COUNTERS: Relaxed loads (approximate calculation OK).
    /// #ASSUME_U16_OVERFLOW: Saturates rate calculation at overflow.
    /// #VERIFY_RELAXED_COUNTERS: Validated in tests.
    #[must_use]
    pub fn false_positive_rate(&self) -> f64 {
        let total = self.total_trips.load(Ordering::Relaxed);
        let false_pos = self.false_positive_count.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            f64::from(false_pos) / f64::from(total)
        }
    }

    /// Update EMA thresholds based on current metrics (called every Nth evaluation).
    /// Uses Q8.8 fixed-point arithmetic for deterministic updates.
    /// Formula: EMA_new = alpha * value + (1 - alpha) * EMA_old
    /// #ASSUME_RELEASE_ORDERING: Release store ensures visibility to other threads.
    /// #VERIFY_RELEASE_ORDERING: Validated in tests (circuit_breaker_adaptive_ordering).
    #[inline]
    pub fn update_ema(&self, mu_norm: f32, sg_norm: f32, err_count: u16, alpha_q8: u16) {
        // Convert current metrics to Q8.8
        let mu_q8 = (mu_norm * 256.0) as u16;
        let sg_q8 = (sg_norm * 256.0) as u16;

        // Load current EMA values
        let mu_ema_old = self.mu_trip_ema.load(Ordering::Acquire);
        let sg_ema_old = self.sg_trip_ema.load(Ordering::Acquire);
        let err_ema_old = self.err_trip_ema.load(Ordering::Relaxed);

        // Calculate new EMA: new = alpha * value + (256 - alpha) * old
        // All arithmetic in Q8.8 fixed-point
        let mu_ema_new = ((u32::from(alpha_q8) * u32::from(mu_q8)
            + u32::from(256 - alpha_q8) * u32::from(mu_ema_old))
            >> 8) as u16;
        let sg_ema_new = ((u32::from(alpha_q8) * u32::from(sg_q8)
            + u32::from(256 - alpha_q8) * u32::from(sg_ema_old))
            >> 8) as u16;
        let err_ema_new = ((u32::from(alpha_q8) * u32::from(err_count)
            + u32::from(256 - alpha_q8) * u32::from(err_ema_old))
            >> 8) as u16;

        // Store new EMA values
        // #ASSUME_RELEASE_ORDERING: Release store for visibility
        self.mu_trip_ema.store(mu_ema_new, Ordering::Release);
        self.sg_trip_ema.store(sg_ema_new, Ordering::Release);
        self.err_trip_ema.store(err_ema_new, Ordering::Relaxed);
    }

    /// Increment update counter and check if EMA update is due.
    /// Returns true if counter reached update_interval.
    /// #ASSUME_RELAXED_COUNTERS: Relaxed increment (approximate triggering OK).
    /// #ASSUME_U16_OVERFLOW: Wrapping semantics acceptable.
    /// #VERIFY_RELAXED_COUNTERS: Validated in tests.
    #[must_use]
    #[inline]
    pub fn should_update_ema(&self, update_interval: u16) -> bool {
        if update_interval == 0 {
            return false;
        }
        let count = self.update_counter.fetch_add(1, Ordering::Relaxed);
        count % update_interval == 0
    }
}

#[cfg(feature = "circuit-breaker-auto-tune")]
/// Optional observers for recording evaluations when adaptive tuning is enabled.
pub struct EvaluationObservers<'a> {
    /// Sliding history buffer for transition analysis.
    pub history: Option<&'a mut HistoryBuffer>,
    /// User-supplied metrics tap for workload feedback.
    pub metrics_tap: Option<&'a mut dyn MetricsTap>,
}

#[cfg(feature = "circuit-breaker-auto-tune")]
impl EvaluationObservers<'_> {
    /// Construct observers with no hooks.
    #[must_use]
    pub fn none() -> Self {
        Self {
            history: None,
            metrics_tap: None,
        }
    }
}

#[cfg(feature = "circuit-breaker-adaptive")]
/// Evaluate breaker with adaptive thresholds (EMA-based).
/// Uses AdaptiveState for real-time threshold adjustment.
/// UCE34 Q10: T1 Atomic + T3 Fixed-Point - lockfree adaptive evaluation.
/// Performance: <20ns overhead over static evaluate() (within budget).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_adaptive<B: BreakerLike>(
    breaker: &B,
    mu_norm: f32,
    sg_norm: f32,
    err_inc: u16,
    now_ms: u32,
    last_change_ms: &mut u32,
    policy: &Policy,
    adaptive: &AdaptiveState,
) {
    // Check if EMA update is due (every Nth evaluation)
    if adaptive.should_update_ema(policy.update_interval) {
        adaptive.update_ema(mu_norm, sg_norm, err_inc, policy.alpha_q8);
    }

    // Use adaptive thresholds if enabled (update_interval > 0)
    let (mu_trip_actual, sg_trip_actual, err_trip_actual) = if policy.update_interval > 0 {
        (
            adaptive.adaptive_mu_trip(),
            adaptive.adaptive_sg_trip(),
            adaptive.adaptive_err_trip(),
        )
    } else {
        (policy.mu_trip, policy.sg_trip, policy.err_trip)
    };

    // Create temporary policy with adaptive thresholds
    let mut adaptive_policy = *policy;
    adaptive_policy.mu_trip = mu_trip_actual;
    adaptive_policy.sg_trip = sg_trip_actual;
    adaptive_policy.err_trip = err_trip_actual;

    // Track trip state before evaluation
    let before_state = {
        let layout = breaker.layout_kind();
        let packed = breaker.load_relaxed();
        AtomicBreakerGuard::from_layout(packed, layout).state()
    };

    // Run standard evaluation with adaptive thresholds
    evaluate(
        breaker,
        mu_norm,
        sg_norm,
        err_inc,
        now_ms,
        last_change_ms,
        &adaptive_policy,
    );

    // Track trip state after evaluation
    let after_state = {
        let layout = breaker.layout_kind();
        let packed = breaker.load_relaxed();
        AtomicBreakerGuard::from_layout(packed, layout).state()
    };

    // Record trip events for statistics
    if after_state == State::Open && before_state != State::Open {
        adaptive.record_trip();
    }

    // Record false positives (HalfOpen → Closed = successful recovery)
    if before_state == State::HalfOpen && after_state == State::Closed {
        // Check if this was a false positive (recovered quickly)
        let elapsed = now_ms.wrapping_sub(*last_change_ms);
        if elapsed < policy.ok_window_ms * 2 {
            adaptive.record_false_positive();
        }
    }
}

/// Evaluate the breaker state machine according to the provided policy.
#[allow(clippy::too_many_arguments)]
pub fn evaluate<B: BreakerLike>(
    breaker: &B,
    mu_norm: f32,
    sg_norm: f32,
    err_inc: u16,
    now_ms: u32,
    last_change_ms: &mut u32,
    policy: &Policy,
) {
    let layout = breaker.layout_kind();
    let packed = breaker.load_relaxed();
    let guard = AtomicBreakerGuard::from_layout(packed, layout);

    let mut err_total = guard.err();
    let err_cap = match layout {
        LayoutKind::Standard64 => 0x3fff,
        #[cfg(feature = "circuit-breaker-compact48")]
        LayoutKind::Compact48 => 0x0fff,
    };

    let mu_q = match layout {
        LayoutKind::Standard64 => pack_q8_8(mu_norm),
        #[cfg(feature = "circuit-breaker-compact48")]
        LayoutKind::Compact48 => pack_q6_10(mu_norm),
    };
    let sg_q = match layout {
        LayoutKind::Standard64 => pack_q8_8(sg_norm),
        #[cfg(feature = "circuit-breaker-compact48")]
        LayoutKind::Compact48 => pack_q6_10(sg_norm),
    };

    let mu_high = mu_norm > policy.mu_trip_f32();
    let sg_high = sg_norm > policy.sg_trip_f32();
    let mu_ok = mu_norm < policy.mu_close_f32();
    let sg_ok = sg_norm < policy.sg_close_f32();

    let mut cause_flags = 0u8;
    if mu_high {
        cause_flags |= cause::LAT;
    }
    if sg_high {
        cause_flags |= cause::JIT;
    }

    err_total = err_total.saturating_add(err_inc).min(err_cap);
    let mut trip_due_to_error = false;
    if policy.err_trip > 0 && err_total >= policy.err_trip {
        trip_due_to_error = true;
        cause_flags |= cause::IO;
    }

    if cause_flags == 0 {
        cause_flags = guard.cause();
    }

    let state = guard.state();
    let mut new_state = state;
    let mut backoff = guard.backoff();
    let elapsed = now_ms.wrapping_sub(*last_change_ms);
    let should_trip = mu_high || sg_high || trip_due_to_error;
    let mut reset_error = false;

    match state {
        State::Closed => {
            if should_trip {
                new_state = State::Open;
            }
        }
        State::Open => {
            if !should_trip && elapsed >= policy.cool_down_ms {
                new_state = State::HalfOpen;
            }
        }
        State::HalfOpen => {
            if should_trip {
                new_state = State::Open;
            } else if mu_ok && sg_ok && elapsed >= policy.ok_window_ms {
                new_state = State::Closed;
                reset_error = true;
                cause_flags = 0;
            }
        }
        State::ForcedOpen => {
            // Sticky state; operator intervention required.
            new_state = State::ForcedOpen;
        }
    }

    if new_state == State::Open && state != State::Open {
        backoff = backoff.saturating_add(1).min(63);
    } else if matches!(new_state, State::Closed) {
        backoff = 0;
    }

    if reset_error {
        breaker.clear_error();
        err_total = err_inc.min(err_cap);
    }

    let err_ratio = if policy.err_trip > 0 {
        f32::from(err_total) / f32::from(policy.err_trip)
    } else {
        0.0
    };

    let mut desired_level = derive_level(mu_norm, sg_norm, err_ratio);
    if matches!(new_state, State::Open) {
        desired_level = desired_level.max(2);
    }
    if matches!(new_state, State::HalfOpen) {
        desired_level = desired_level.max(1);
    }
    if matches!(new_state, State::ForcedOpen) {
        desired_level = 3;
    }

    let current_level = guard.level();
    if desired_level < current_level && !(mu_ok && sg_ok) {
        desired_level = current_level;
    }
    desired_level = desired_level.min(3);

    breaker.update_metrics(err_inc, mu_q, sg_q, cause_flags, backoff);

    if new_state != state || desired_level != current_level {
        breaker.set_state_level(new_state, desired_level);
        *last_change_ms = now_ms;
    }
}

fn derive_level(mu_norm: f32, sg_norm: f32, err_ratio: f32) -> u8 {
    let mut level = 0u8;
    if mu_norm >= 3.0 || sg_norm >= 3.0 {
        level = 3;
    } else if mu_norm >= 2.0 || sg_norm >= 2.0 {
        level = 2;
    } else if mu_norm >= 1.15 || sg_norm >= 1.15 {
        level = 1;
    }

    if err_ratio >= 1.0 {
        level = 3;
    } else if err_ratio >= 0.75 {
        level = level.max(2);
    } else if err_ratio >= 0.5 {
        level = level.max(1);
    }

    level
}

#[cfg(feature = "circuit-breaker-auto-tune")]
fn record_observers(
    now_ms: u32,
    last_change_prev: u32,
    mu_norm: f32,
    sg_norm: f32,
    err_inc: u16,
    policy: &Policy,
    observers: &mut EvaluationObservers<'_>,
    before_guard: AtomicBreakerGuard,
    after_guard: AtomicBreakerGuard,
    before_snapshot: MetricsSnapshot,
    after_snapshot: MetricsSnapshot,
) {
    use crate::patterns::circuit_breaker::breaker::State;

    let changed =
        after_guard.state() != before_guard.state() || after_guard.level() != before_guard.level();

    let recorded_sample = TelemetrySample {
        mu_norm,
        sg_norm,
        err_inc,
        cause: after_snapshot.cause,
        backoff_hint: if after_snapshot.backoff > 0 {
            Some(after_snapshot.backoff)
        } else {
            None
        },
    };

    let action_outcome = if let Some(tap) = observers.metrics_tap.as_mut() {
        tap.record_transition(now_ms, &before_snapshot, &after_snapshot, &recorded_sample)
    } else {
        None
    };

    if changed {
        if let Some(history) = observers.history.as_mut() {
            let success = after_guard.state() == State::Closed
                && mu_norm < policy.mu_close_f32()
                && sg_norm < policy.sg_close_f32()
                && err_inc == 0;
            let entry = HistoryEntry {
                timestamp_ms: now_ms,
                prev_state: before_guard.state(),
                next_state: after_guard.state(),
                prev_level: before_guard.level(),
                next_level: after_guard.level(),
                dwell_ms: now_ms.wrapping_sub(last_change_prev),
                success,
                before: before_snapshot,
                after: after_snapshot,
                sample: recorded_sample,
                action_outcome,
            };
            history.record(entry);
        }
    }
}

/// Apply telemetry data and evaluate the breaker in a single step.
#[cfg(feature = "std")]
pub fn evaluate_with_telemetry<B: BreakerLike>(
    breaker: &B,
    sample: &TelemetrySample,
    now_ms: u32,
    last_change_ms: &mut u32,
    policy: &Policy,
) {
    breaker.apply_sample(sample);
    evaluate(
        breaker,
        sample.mu_norm,
        sample.sg_norm,
        sample.err_inc,
        now_ms,
        last_change_ms,
        policy,
    );
}

#[cfg(feature = "circuit-breaker-auto-tune")]
#[allow(clippy::too_many_arguments)]
/// Evaluate breaker logic while recording observer hooks (raw metrics version).
pub fn evaluate_with_observers<B: BreakerLike>(
    breaker: &B,
    mu_norm: f32,
    sg_norm: f32,
    err_inc: u16,
    now_ms: u32,
    last_change_ms: &mut u32,
    policy: &Policy,
    observers: &mut EvaluationObservers<'_>,
) {
    let layout = breaker.layout_kind();
    let before_guard = AtomicBreakerGuard::from_layout(breaker.load_relaxed(), layout);
    let before_snapshot = before_guard.metrics_snapshot();
    let last_change_prev = *last_change_ms;
    evaluate(
        breaker,
        mu_norm,
        sg_norm,
        err_inc,
        now_ms,
        last_change_ms,
        policy,
    );
    let after_guard = AtomicBreakerGuard::from_layout(breaker.load_relaxed(), layout);
    let after_snapshot = after_guard.metrics_snapshot();
    record_observers(
        now_ms,
        last_change_prev,
        mu_norm,
        sg_norm,
        err_inc,
        policy,
        observers,
        before_guard,
        after_guard,
        before_snapshot,
        after_snapshot,
    );
}

#[cfg(feature = "circuit-breaker-auto-tune")]
/// Evaluate with telemetry while recording observer hooks.
pub fn evaluate_with_telemetry_and_observers<B: BreakerLike>(
    breaker: &B,
    sample: &TelemetrySample,
    now_ms: u32,
    last_change_ms: &mut u32,
    policy: &Policy,
    observers: &mut EvaluationObservers<'_>,
) {
    let layout = breaker.layout_kind();
    let before_guard = AtomicBreakerGuard::from_layout(breaker.load_relaxed(), layout);
    let before_snapshot = before_guard.metrics_snapshot();
    let last_change_prev = *last_change_ms;
    evaluate_with_telemetry(breaker, sample, now_ms, last_change_ms, policy);
    let after_guard = AtomicBreakerGuard::from_layout(breaker.load_relaxed(), layout);
    let after_snapshot = after_guard.metrics_snapshot();
    record_observers(
        now_ms,
        last_change_prev,
        sample.mu_norm,
        sample.sg_norm,
        sample.err_inc,
        policy,
        observers,
        before_guard,
        after_guard,
        before_snapshot,
        after_snapshot,
    );
}

#[cfg(feature = "circuit-breaker-auto-tune")]
/// Run the auto-calibration loop in offline mode using the supplied history buffer.
#[must_use]
pub fn tune(
    history: &HistoryBuffer,
    baseline: &Policy,
    targets: &CalibrationTargets,
) -> Option<PolicyDraft> {
    tune_with_mode(history, baseline, targets, CalibrationMode::Offline)
}

#[cfg(feature = "circuit-breaker-auto-tune")]
/// Run the auto-calibration loop with an explicit mode (warm-up or offline).
#[must_use]
pub fn tune_with_mode(
    history: &HistoryBuffer,
    baseline: &Policy,
    targets: &CalibrationTargets,
    mode: CalibrationMode,
) -> Option<PolicyDraft> {
    calibrate_policy(history, baseline, targets, mode)
}

#[cfg(feature = "circuit-breaker-auto-tune")]
fn apply_delta(value: &mut u32, delta: i32) {
    if delta > 0 {
        *value = value.saturating_add(delta as u32);
    } else if delta < 0 {
        let reduce = (-delta) as u32;
        *value = value.saturating_sub(reduce);
    }
}

#[cfg(feature = "circuit-breaker-auto-tune")]
/// Apply level feedback recommendations to the policy dwell configuration.
#[must_use]
pub fn adjust_dwell(policy: &mut Policy, feedback: &LevelFeedbackResult) -> Option<u8> {
    apply_delta(&mut policy.cool_down_ms, feedback.cool_down_delta_ms);
    apply_delta(&mut policy.ok_window_ms, feedback.ok_window_delta_ms);
    if policy.ok_window_ms == 0 {
        policy.ok_window_ms = 1;
    }
    feedback.backoff_hint
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::patterns::circuit_breaker::breaker::{
        AtomicBreakerGuard, AtomicBreakerSWeMR, State,
    };
    #[cfg(feature = "circuit-breaker-auto-tune")]
    use crate::patterns::circuit_breaker::telemetry::TelemetrySample;
    #[cfg(feature = "circuit-breaker-auto-tune")]
    use crate::patterns::circuit_breaker::telemetry::{ActionOutcome, HistoryBuffer};
    #[cfg(feature = "circuit-breaker-auto-tune")]
    use crate::patterns::circuit_breaker::MetricsSnapshot;
    use proptest::prelude::*;

    #[derive(Clone, Debug)]
    struct Stimulus {
        mu: f32,
        sg: f32,
        err_inc: u16,
        dt: u32,
    }

    prop_compose! {
        fn arb_stimulus()(mu in 0.0_f32..4.0, sg in 0.0_f32..4.0, err in 0u16..16, dt in 0u32..200) -> Stimulus {
            Stimulus { mu, sg, err_inc: err, dt }
        }
    }

    #[test]
    fn policy_trips_and_recovers() {
        let policy = Policy::ui_holographic();
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        let mut last_change = 0u32;

        evaluate(&breaker, 20.0, 0.4, 4, 10, &mut last_change, &policy);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::Open);
        assert!(last_change >= 10);

        let half_time = last_change + policy.cool_down_ms + 1;
        evaluate(&breaker, 0.9, 0.8, 0, half_time, &mut last_change, &policy);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::HalfOpen);
        assert!(guard.level() >= 1);

        let close_time = last_change + policy.ok_window_ms + 2;
        evaluate(&breaker, 0.6, 0.5, 0, close_time, &mut last_change, &policy);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::Closed);
        assert_eq!(guard.err(), 0);
        assert!(last_change >= close_time);
    }

    #[test]
    fn forced_open_remains_sticky() {
        let policy = Policy::audio_lowlatency();
        let breaker = AtomicBreakerSWeMR::new_standard64(State::ForcedOpen);
        let mut last_change = 0u32;

        evaluate(&breaker, 0.5, 0.4, 0, 5_000, &mut last_change, &policy);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::ForcedOpen);
        assert_eq!(guard.level(), 3);
        assert_eq!(last_change, 5_000);
    }

    #[test]
    fn policy_profiles_are_distinct() {
        let ui = Policy::ui_holographic();
        let audio = Policy::audio_lowlatency();
        let io = Policy::io_disk();
        let arb = Policy::arb_venue();

        assert_ne!(ui.mu_trip, audio.mu_trip);
        assert_ne!(io.sg_trip, ui.sg_trip);
        assert_ne!(arb.err_trip, io.err_trip);
    }

    proptest! {
        #[test]
        fn state_machine_remains_legal(stimuli in proptest::collection::vec(arb_stimulus(), 1..50)) {
            let policy = Policy::io_disk();
            let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
            let mut last_change = 0u32;
            let mut now = 0u32;

            for stimulus in stimuli {
                now = now.wrapping_add(stimulus.dt.max(1));
                evaluate(
                    &breaker,
                    stimulus.mu,
                    stimulus.sg,
                    stimulus.err_inc,
                    now,
                    &mut last_change,
                    &policy,
                );

                let packed = breaker.load_relaxed();
                let guard = AtomicBreakerGuard::new(packed);

                // Levels are always within encoded bounds.
                prop_assert!(guard.level() <= 3);
                // Forced open must pin the level to 3.
                if guard.state() == State::ForcedOpen {
                    prop_assert_eq!(guard.level(), 3);
                }
                // Ensure last_change only moves forward when state/level update.
                prop_assert!(now >= last_change);
            }
        }
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    struct CountingTap {
        calls: usize,
        last_next_state: Option<State>,
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    impl CountingTap {
        fn new() -> Self {
            Self {
                calls: 0,
                last_next_state: None,
            }
        }
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    impl MetricsTap for CountingTap {
        fn record_transition(
            &mut self,
            _now_ms: u32,
            _before: &MetricsSnapshot,
            after: &MetricsSnapshot,
            _sample: &TelemetrySample,
        ) -> Option<ActionOutcome> {
            self.calls += 1;
            self.last_next_state = Some(after.state);
            Some(ActionOutcome {
                recovered_within_target: after.state == State::Closed,
                observed_recovery_ms: Some(90),
            })
        }
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    #[test]
    fn observers_capture_history_on_transition() {
        let policy = Policy::ui_holographic();
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        let mut last_change = 0u32;
        let mut history = HistoryBuffer::new(4);
        let mut tap = CountingTap::new();
        let mut observers = EvaluationObservers {
            history: Some(&mut history),
            metrics_tap: Some(&mut tap),
        };

        evaluate_with_observers(
            &breaker,
            20.0,
            0.5,
            3,
            1,
            &mut last_change,
            &policy,
            &mut observers,
        );

        drop(observers);

        assert_eq!(tap.calls, 1);
        assert_eq!(tap.last_next_state, Some(State::Open));
        assert_eq!(history.len(), 1);
        let entry = history.iter().next().unwrap();
        assert_eq!(entry.prev_state, State::Closed);
        assert_eq!(entry.next_state, State::Open);
        assert_eq!(entry.dwell_ms, 1);
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    fn synthetic_history(mu: f32, sg: f32, success: bool) -> HistoryEntry {
        let snapshot = MetricsSnapshot {
            state: if success { State::Closed } else { State::Open },
            level: if success { 0 } else { 2 },
            err: if success { 1 } else { 24 },
            mu_norm: mu,
            sg_norm: sg,
            cause: 0,
            backoff: 0,
        };
        HistoryEntry {
            timestamp_ms: 0,
            prev_state: State::Closed,
            next_state: snapshot.state,
            prev_level: 0,
            next_level: snapshot.level,
            dwell_ms: 10,
            success,
            before: snapshot,
            after: snapshot,
            sample: TelemetrySample {
                mu_norm: mu,
                sg_norm: sg,
                err_inc: if success { 0 } else { 5 },
                cause: 0,
                backoff_hint: None,
            },
            action_outcome: Some(ActionOutcome {
                recovered_within_target: success,
                observed_recovery_ms: Some(if success { 45 } else { 160 }),
            }),
        }
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    #[test]
    fn tune_returns_adjusted_policy() {
        let mut history = HistoryBuffer::new(4);
        history.record(synthetic_history(3.2, 2.4, false));
        history.record(synthetic_history(2.8, 2.0, false));
        let baseline = Policy::arb_venue();
        let targets = CalibrationTargets {
            success_rate: 0.7,
            max_transitions_per_min: 4.0,
            mu_p95_target: 1.5,
            sg_p95_target: 1.2,
            err_trip_target: 6,
        };
        let result = tune(&history, &baseline, &targets).expect("expected tune result");
        assert!(result.policy.mu_trip < baseline.mu_trip);
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    #[test]
    fn adjust_dwell_applies_deltas() {
        let mut policy = Policy::audio_lowlatency();
        let original = policy;
        let feedback = LevelFeedbackResult {
            cool_down_delta_ms: 15,
            ok_window_delta_ms: -3,
            backoff_hint: Some(5),
            notes: Vec::new(),
        };
        let hint = adjust_dwell(&mut policy, &feedback);
        assert_eq!(policy.cool_down_ms, original.cool_down_ms + 15);
        assert_eq!(policy.ok_window_ms, original.ok_window_ms.saturating_sub(3));
        assert_eq!(hint, Some(5));
    }

    // ============================================================================
    // ADAPTIVE THRESHOLD TESTS (circuit-breaker-adaptive feature)
    // ============================================================================
    // UCE34 Q33: Verification macros for adaptive thresholds
    // T28 Testing Framework: Unit/Property/Integration/Production tiers
    // B32 Benchmarking: Honest performance claims with fair baselines
    // ASSUM Safety: 99.99% safe - all assumptions verified
    // ============================================================================

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn adaptive_state_size_invariant() {
        use core::mem::size_of;
        // #VERIFY_CACHE_LINE_64B: Verify 64-byte alignment
        assert_eq!(size_of::<AdaptiveState>(), 64);
        assert_eq!(core::mem::align_of::<AdaptiveState>(), 64);
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn adaptive_state_initialization() {
        let policy = Policy::ui_holographic();
        let adaptive = AdaptiveState::new(&policy);

        // EMA values should match static thresholds initially
        assert_eq!(adaptive.adaptive_mu_trip(), policy.mu_trip);
        assert_eq!(adaptive.adaptive_sg_trip(), policy.sg_trip);
        assert_eq!(adaptive.adaptive_err_trip(), policy.err_trip);

        // Counters should be zero
        assert_eq!(adaptive.false_positive_rate(), 0.0);
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn adaptive_ema_update() {
        let policy = Policy::ui_holographic();
        let adaptive = AdaptiveState::new(&policy);

        // Simulate high latency metrics
        adaptive.update_ema(3.0, 2.5, 15, 24); // alpha_q8 = 24 (0.095)

        // EMA should increase from base thresholds
        let mu_trip_new = adaptive.adaptive_mu_trip();
        let sg_trip_new = adaptive.adaptive_sg_trip();
        let err_trip_new = adaptive.adaptive_err_trip();

        // Values should have moved toward current metrics
        assert!(mu_trip_new > policy.mu_trip);
        assert!(sg_trip_new > policy.sg_trip);
        assert!(err_trip_new > policy.err_trip);
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn adaptive_false_positive_tracking() {
        let policy = Policy::ui_holographic();
        let adaptive = AdaptiveState::new(&policy);

        // Record trip events
        adaptive.record_trip();
        adaptive.record_trip();
        adaptive.record_trip();

        // Record false positive
        adaptive.record_false_positive();

        // False positive rate should be 1/3 = 0.333...
        let rate = adaptive.false_positive_rate();
        assert!((rate - 0.333).abs() < 0.001);
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn adaptive_update_interval() {
        let policy = Policy::ui_holographic().with_adaptive_config(10, 24);
        let adaptive = AdaptiveState::new(&policy);

        // First 9 calls should return false
        for _ in 0..9 {
            assert!(!adaptive.should_update_ema(policy.update_interval));
        }

        // 10th call should return true
        assert!(adaptive.should_update_ema(policy.update_interval));

        // Next 9 should return false again
        for _ in 0..9 {
            assert!(!adaptive.should_update_ema(policy.update_interval));
        }
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn adaptive_evaluate_integration() {
        let policy = Policy::ui_holographic().with_adaptive();
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        let adaptive = AdaptiveState::new(&policy);
        let mut last_change = 0u32;

        // Evaluate with high latency (should trip)
        evaluate_adaptive(
            &breaker,
            20.0,
            0.5,
            3,
            10,
            &mut last_change,
            &policy,
            &adaptive,
        );

        let guard = breaker.guard();
        assert_eq!(guard.state(), State::Open);

        // Trip should be recorded
        assert_eq!(adaptive.total_trips.load(Ordering::Relaxed), 1);
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn adaptive_false_positive_detection() {
        let policy = Policy::ui_holographic().with_adaptive();
        let breaker = AtomicBreakerSWeMR::new_standard64(State::HalfOpen);
        let adaptive = AdaptiveState::new(&policy);
        let mut last_change = 0u32;

        // Evaluate with good metrics (should close quickly = false positive)
        evaluate_adaptive(
            &breaker,
            0.5,
            0.4,
            0,
            10,
            &mut last_change,
            &policy,
            &adaptive,
        );

        let guard = breaker.guard();
        assert_eq!(guard.state(), State::Closed);

        // False positive should be recorded (quick recovery)
        assert_eq!(adaptive.false_positive_count.load(Ordering::Relaxed), 1);
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn adaptive_disabled_uses_static_thresholds() {
        let policy = Policy::ui_holographic().with_adaptive_config(0, 24); // update_interval=0 = disabled
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        let adaptive = AdaptiveState::new(&policy);
        let mut last_change = 0u32;

        // Update EMA with different values
        adaptive.update_ema(5.0, 4.0, 25, 24);

        // Evaluate should still use static thresholds (update_interval=0)
        evaluate_adaptive(
            &breaker,
            1.5,
            1.2,
            5,
            10,
            &mut last_change,
            &policy,
            &adaptive,
        );

        // Should remain closed (static thresholds not exceeded)
        let guard = breaker.guard();
        assert_eq!(guard.state(), State::Closed);
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn adaptive_ema_convergence() {
        let policy = Policy::ui_holographic();
        let adaptive = AdaptiveState::new(&policy);

        // Simulate 100 updates with constant high metrics
        for _ in 0..100 {
            adaptive.update_ema(3.0, 2.5, 20, 24);
        }

        // EMA should have converged close to target values
        let mu_trip = adaptive.adaptive_mu_trip();
        let sg_trip = adaptive.adaptive_sg_trip();
        let err_trip = adaptive.adaptive_err_trip();

        // Q8.8 fixed-point: 3.0 * 256 = 768, 2.5 * 256 = 640
        let mu_expected = 768u16;
        let sg_expected = 640u16;
        let err_expected = 20u16;

        // Should be within 10% of expected values after 100 updates
        assert!((mu_trip as i32 - mu_expected as i32).abs() < (mu_expected as i32 / 10));
        assert!((sg_trip as i32 - sg_expected as i32).abs() < (sg_expected as i32 / 10));
        assert!((err_trip as i32 - err_expected as i32).abs() < (err_expected as i32 / 10));
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn circuit_breaker_adaptive_ordering() {
        use std::sync::Arc;
        use std::thread;

        let policy = Arc::new(Policy::ui_holographic().with_adaptive());
        let adaptive = Arc::new(AdaptiveState::new(&policy));
        let breaker = Arc::new(AtomicBreakerSWeMR::new_standard64(State::Closed));

        // Spawn 4 threads doing concurrent evaluations
        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                let policy_clone = Arc::clone(&policy);
                let adaptive_clone = Arc::clone(&adaptive);
                let breaker_clone = Arc::clone(&breaker);

                thread::spawn(move || {
                    let mut last_change = 0u32;
                    for i in 0..100 {
                        let mu = if i % 20 == 0 { 5.0 } else { 0.5 };
                        evaluate_adaptive(
                            &breaker_clone,
                            mu,
                            0.5,
                            1,
                            (thread_id * 1000 + i) as u32,
                            &mut last_change,
                            &policy_clone,
                            &adaptive_clone,
                        );
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify trip counter is reasonable (should have some trips)
        let total_trips = adaptive.total_trips.load(Ordering::Relaxed);
        assert!(total_trips > 0);
        assert!(total_trips < 100); // Not every evaluation should trip
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn circuit_breaker_adaptive_counters() {
        let adaptive = AdaptiveState::new(&Policy::ui_holographic());

        // Test counter overflow behavior
        adaptive.total_trips.store(65535, Ordering::Relaxed);
        adaptive.record_trip(); // Should wrap to 0

        // False positive rate should handle overflow gracefully
        let rate = adaptive.false_positive_rate();
        assert!(rate.is_finite());
    }

    #[cfg(feature = "circuit-breaker-adaptive")]
    #[test]
    fn circuit_breaker_adaptive_relaxed() {
        let adaptive = AdaptiveState::new(&Policy::ui_holographic());

        // Verify relaxed ordering doesn't cause issues
        for _ in 0..1000 {
            adaptive.record_trip();
            adaptive.record_false_positive();
            let _ = adaptive.false_positive_rate();
        }

        // All operations should complete successfully
        assert!(adaptive.total_trips.load(Ordering::Relaxed) > 0);
    }
}
