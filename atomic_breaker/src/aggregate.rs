//! Fractal aggregation helpers for breaker snapshots.

use crate::breaker::{AtomicBreakerGuard, LayoutKind, State};
use crate::cause;
use crate::diag::DiagSnapshot;
#[cfg(feature = "compact48")]
use crate::layout::Compact48;
use crate::layout::{Layout, LayoutRaw, Standard64};

/// Aggregation input sample containing a packed breaker word and optional diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sample {
    /// Packed breaker word.
    pub packed: u64,
    /// Optional diagnostics snapshot tied to the breaker.
    pub diag: Option<DiagSnapshot>,
}

impl Sample {
    /// Construct a sample without diagnostics.
    #[must_use]
    pub const fn new(packed: u64) -> Self {
        Self { packed, diag: None }
    }

    /// Construct a sample with diagnostics information.
    #[must_use]
    pub const fn with_diag(packed: u64, diag: DiagSnapshot) -> Self {
        Self {
            packed,
            diag: Some(diag),
        }
    }
}

/// Aggregate a slice of samples into a single summary word.
#[must_use]
pub fn aggregate(
    samples: &[Sample],
    layout: LayoutKind,
    now_ms: Option<u32>,
    stale_window_ms: Option<u32>,
) -> u64 {
    match layout {
        LayoutKind::Standard64 => aggregate_standard64(samples, now_ms, stale_window_ms),
        #[cfg(feature = "compact48")]
        LayoutKind::Compact48 => aggregate_compact48(samples, now_ms, stale_window_ms),
    }
}

/// Aggregate samples using the standard 64-bit layout.
#[must_use]
pub fn aggregate_standard64(
    samples: &[Sample],
    now_ms: Option<u32>,
    stale_window_ms: Option<u32>,
) -> u64 {
    if samples.is_empty() {
        return 0;
    }

    let mut rolling = LayoutRaw::default();
    let mut state_rank = 0u8;
    let mut stale_flag = false;

    for sample in samples {
        let guard = AtomicBreakerGuard::from_layout(sample.packed, LayoutKind::Standard64);
        let state_bits = guard.state().bits();
        let rank = state_rank_value(guard.state());
        if rank > state_rank {
            state_rank = rank;
            rolling.state = state_bits;
        }
        rolling.level = rolling.level.max(guard.level());
        rolling.err = rolling.err.max(guard.err()).min(0x3fff);
        rolling.mu_norm = rolling.mu_norm.max(guard.mu_norm());
        rolling.sg_norm = rolling.sg_norm.max(guard.sg_norm());
        rolling.cause |= guard.cause();
        rolling.backoff = rolling.backoff.max(guard.backoff());

        if let (Some(now), Some(window), Some(diag)) = (now_ms, stale_window_ms, sample.diag) {
            if diag.is_stale(now, window) {
                stale_flag = true;
            }
        }
    }

    if stale_flag {
        rolling.state = State::Open.bits();
        rolling.cause |= cause::TIMEOUT;
    }

    Standard64::pack(rolling)
}

#[cfg(feature = "compact48")]
#[must_use]
/// Aggregate samples using the compact 48-bit layout.
pub fn aggregate_compact48(
    samples: &[Sample],
    now_ms: Option<u32>,
    stale_window_ms: Option<u32>,
) -> u64 {
    if samples.is_empty() {
        return 0;
    }

    let mut rolling = LayoutRaw::default();
    let mut state_rank = 0u8;
    let mut stale_flag = false;

    for sample in samples {
        let guard = AtomicBreakerGuard::from_layout(sample.packed, LayoutKind::Compact48);
        let rank = state_rank_value(guard.state());
        if rank > state_rank {
            state_rank = rank;
            rolling.state = guard.state().bits();
        }
        rolling.level = rolling.level.max(guard.level());
        rolling.err = rolling.err.max(guard.err()).min(0x0fff);
        rolling.mu_norm = rolling.mu_norm.max(guard.mu_norm());
        rolling.sg_norm = rolling.sg_norm.max(guard.sg_norm());

        if let (Some(now), Some(window), Some(diag)) = (now_ms, stale_window_ms, sample.diag) {
            if diag.is_stale(now, window) {
                stale_flag = true;
            }
        }
    }

    if stale_flag {
        rolling.state = State::Open.bits();
    }

    Compact48::pack(rolling)
}

fn state_rank_value(state: State) -> u8 {
    match state {
        State::Closed => 0,
        State::HalfOpen => 1,
        State::Open => 2,
        State::ForcedOpen => 3,
    }
}

/// Combine two packed breaker words using the same aggregation rules.
#[must_use]
pub fn lub(packed_a: u64, packed_b: u64, layout: LayoutKind) -> u64 {
    let samples = [Sample::new(packed_a), Sample::new(packed_b)];
    aggregate(&samples, layout, None, None)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR, State};
    use crate::cause;
    use crate::layout::{LayoutRaw, Standard64};
    use proptest::collection::vec;
    use proptest::prelude::*;

    #[test]
    fn aggregator_takes_strongest_state() {
        let closed = AtomicBreakerSWeMR::new_standard64(State::Closed);
        let open = AtomicBreakerSWeMR::new_standard64(State::Closed);
        open.open();
        let samples = [
            Sample::new(closed.load_relaxed()),
            Sample::new(open.load_relaxed()),
        ];
        let packed = aggregate_standard64(&samples, None, None);
        let guard = AtomicBreakerGuard::from_layout(packed, LayoutKind::Standard64);
        assert_eq!(guard.state(), State::Open);
    }

    #[test]
    fn stale_child_sets_timeout_cause() {
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        let diag = DiagSnapshot {
            last_update_ms: 1,
            last_reason: 0,
            long_err: 0,
        };
        let sample = Sample::with_diag(breaker.load_relaxed(), diag);
        let packed = aggregate_standard64(&[sample], Some(100), Some(10));
        let guard = AtomicBreakerGuard::from_layout(packed, LayoutKind::Standard64);
        assert_eq!(guard.state(), State::Open);
        assert!(guard.cause() & cause::TIMEOUT != 0);
    }

    #[test]
    fn aggregate_and_lub_helpers_match() {
        let closed = Sample::new(AtomicBreakerSWeMR::new_standard64(State::Closed).load_relaxed());
        let open = Sample::new(AtomicBreakerSWeMR::new_standard64(State::Open).load_relaxed());
        let empty = aggregate(&[], LayoutKind::Standard64, None, None);
        assert_eq!(empty, 0);

        let summary = aggregate(&[closed, open], LayoutKind::Standard64, None, None);
        let lub_summary = lub(closed.packed, open.packed, LayoutKind::Standard64);
        assert_eq!(summary, lub_summary);

        let guard = AtomicBreakerGuard::from_layout(summary, LayoutKind::Standard64);
        assert_eq!(guard.state(), State::Open);
    }

    fn sample_strategy(now: u32, window: u32) -> impl Strategy<Value = Sample> {
        (
            0u8..=3,
            0u8..=3,
            0u16..=0x3fff,
            any::<u16>(),
            any::<u16>(),
            any::<u8>(),
            0u8..=0x3f,
            prop::option::of(0u32..=window),
            any::<u8>(),
            any::<u64>(),
        )
            .prop_map(
                move |(
                    state,
                    level,
                    err,
                    mu,
                    sg,
                    cause_bits,
                    backoff,
                    age_opt,
                    reason,
                    long_err,
                )| {
                    let raw = LayoutRaw {
                        state,
                        level,
                        err,
                        mu_norm: mu,
                        sg_norm: sg,
                        cause: cause_bits,
                        backoff,
                    };
                    let packed = Standard64::pack(raw);
                    let diag = age_opt.map(|age| {
                        let last_update_ms = now.saturating_sub(age);
                        DiagSnapshot {
                            last_update_ms,
                            last_reason: reason,
                            long_err,
                        }
                    });
                    Sample { packed, diag }
                },
            )
    }

    proptest! {
        #[test]
        fn aggregate_order_invariant(samples in vec(sample_strategy(10_000, 1_000), 1..6)) {
            let now = Some(10_000u32);
            let window = Some(1_000u32);
            let agg_forward = aggregate_standard64(&samples, now, window);

            let mut reversed = samples.clone();
            reversed.reverse();
            let agg_reverse = aggregate_standard64(&reversed, now, window);

            prop_assert_eq!(agg_forward, agg_reverse);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn sample_serializes_via_serde() {
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Open);
        let diag = DiagSnapshot {
            last_update_ms: 7,
            last_reason: cause::LAT,
            long_err: 99,
        };
        let sample = Sample::with_diag(breaker.load_relaxed(), diag);
        let json = serde_json::to_string(&sample).expect("serialize sample");
        let parsed: Sample = serde_json::from_str(&json).expect("deserialize sample");
        assert_eq!(sample.packed, parsed.packed);
        assert_eq!(sample.diag, parsed.diag);
    }
}
