//! Bit layout definitions and helpers for breaker packing.

use core::fmt;

/// Layout version for the standard 64-bit packing, used for serialization.
pub const STANDARD64_V1: u32 = 0xA1B2_C3D4;
/// Layout version for the compact 48-bit packing (still stored in `u64`).
pub const COMPACT48_V1: u32 = 0x4D5E_6F70;

/// Layout alias selected by the active feature flags.
#[cfg(feature = "circuit-breaker-standard64")]
/// Active layout alias for builds targeting the standard 64-bit packing.
pub type DefaultLayout = Standard64;
#[cfg(all(
    not(feature = "circuit-breaker-standard64"),
    feature = "circuit-breaker-compact48"
))]
/// Active layout alias for builds targeting the compact 48-bit packing.
pub type DefaultLayout = Compact48;

/// Common trait for packing and unpacking layout-specific values.
pub trait Layout: Sized + Copy + fmt::Debug {
    /// Total number of significant bits used by the layout.
    const BITS: u32;

    /// Pack raw components into the underlying word.
    fn pack(raw: LayoutRaw) -> u64;

    /// Unpack the word into raw components.
    fn unpack(word: u64) -> LayoutRaw;
}

/// Raw components shared across layouts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutRaw {
    /// Breaker state (2 bits).
    pub state: u8,
    /// Breaker level (2 bits).
    pub level: u8,
    /// Saturating error count (layout-dependent bit width).
    pub err: u16,
    /// Normalized mean metric in fixed-point.
    pub mu_norm: u16,
    /// Normalized sigma metric in fixed-point.
    pub sg_norm: u16,
    /// Cause bits (standard layout only).
    pub cause: u8,
    /// Backoff index (standard layout only).
    pub backoff: u8,
}

impl LayoutRaw {
    /// Retain the state and level while clearing metric fields for the standard layout.
    #[must_use]
    pub fn state_level_bits(&self) -> u64 {
        u64::from(self.state & 0x3) | (u64::from(self.level & 0x3) << 2)
    }
}

/// Standard 64-bit layout descriptor.
#[derive(Clone, Copy, Debug, Default)]
pub struct Standard64;

/// Compact 48-bit layout descriptor (packed in low bits of `u64`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Compact48;

impl Layout for Standard64 {
    const BITS: u32 = 64;

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn pack(raw: LayoutRaw) -> u64 {
        let mut word = 0u64;
        word |= u64::from(raw.state & 0x3);
        word |= (u64::from(raw.level & 0x3)) << 2;
        word |= (u64::from(raw.err.min(0x3fff))) << 4;
        word |= (u64::from(raw.mu_norm)) << 18;
        word |= (u64::from(raw.sg_norm)) << 34;
        word |= (u64::from(raw.cause)) << 50;
        word |= (u64::from(raw.backoff & 0x3f)) << 58;
        word
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn unpack(word: u64) -> LayoutRaw {
        LayoutRaw {
            state: (word & 0x3) as u8,
            level: ((word >> 2) & 0x3) as u8,
            err: ((word >> 4) & 0x3fff) as u16,
            mu_norm: ((word >> 18) & 0xffff) as u16,
            sg_norm: ((word >> 34) & 0xffff) as u16,
            cause: ((word >> 50) & 0xff) as u8,
            backoff: ((word >> 58) & 0x3f) as u8,
        }
    }
}

impl Layout for Compact48 {
    const BITS: u32 = 48;

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn pack(raw: LayoutRaw) -> u64 {
        let mut word = 0u64;
        word |= u64::from(raw.state & 0x3);
        word |= (u64::from(raw.level & 0x3)) << 2;
        word |= (u64::from(raw.err.min(0x0fff))) << 4;
        word |= (u64::from(raw.mu_norm)) << 16;
        word |= (u64::from(raw.sg_norm)) << 32;
        word
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn unpack(word: u64) -> LayoutRaw {
        LayoutRaw {
            state: (word & 0x3) as u8,
            level: ((word >> 2) & 0x3) as u8,
            err: ((word >> 4) & 0x0fff) as u16,
            mu_norm: ((word >> 16) & 0xffff) as u16,
            sg_norm: ((word >> 32) & 0xffff) as u16,
            cause: 0,
            backoff: 0,
        }
    }
}

/// Masks and shifts for the standard layout (public for reuse).
pub mod standard64 {
    /// State mask (bits 0..=1).
    pub const STATE_MASK: u64 = 0b11;
    /// Level mask (bits 2..=3).
    pub const LEVEL_MASK: u64 = 0b11 << 2;
    /// Error counter mask (bits 4..=17).
    pub const ERR_MASK: u64 = 0x3fff << 4;
    /// Mean metric mask (bits 18..=33).
    pub const MU_MASK: u64 = 0xffff << 18;
    /// Sigma metric mask (bits 34..=49).
    pub const SIGMA_MASK: u64 = 0xffff << 34;
    /// Cause bits mask (bits 50..=57).
    pub const CAUSE_MASK: u64 = 0xff << 50;
    /// Backoff field mask (bits 58..=63).
    pub const BACKOFF_MASK: u64 = 0x3f << 58;
    /// Mask covering all metric fields.
    pub const METRIC_MASK: u64 = ERR_MASK | MU_MASK | SIGMA_MASK | CAUSE_MASK | BACKOFF_MASK;
    /// Mask over the state and level fields.
    pub const STATE_LEVEL_MASK: u64 = STATE_MASK | LEVEL_MASK;

    /// Extract state bits.
    #[must_use]
    pub const fn state(word: u64) -> u8 {
        (word & STATE_MASK) as u8
    }

    /// Extract level bits.
    #[must_use]
    pub const fn level(word: u64) -> u8 {
        ((word & LEVEL_MASK) >> 2) as u8
    }

    /// Extract the error counter.
    #[must_use]
    pub const fn err(word: u64) -> u16 {
        ((word & ERR_MASK) >> 4) as u16
    }

    /// Extract the mean metric.
    #[must_use]
    pub const fn mu(word: u64) -> u16 {
        ((word & MU_MASK) >> 18) as u16
    }

    /// Extract the jitter metric.
    #[must_use]
    pub const fn sigma(word: u64) -> u16 {
        ((word & SIGMA_MASK) >> 34) as u16
    }

    /// Extract cause bits.
    #[must_use]
    pub const fn cause(word: u64) -> u8 {
        ((word & CAUSE_MASK) >> 50) as u8
    }

    /// Extract backoff index.
    #[must_use]
    pub const fn backoff(word: u64) -> u8 {
        ((word & BACKOFF_MASK) >> 58) as u8
    }

    /// Replace the state field.
    #[must_use]
    pub fn with_state(word: u64, state: u8) -> u64 {
        (word & !STATE_MASK) | u64::from(state & 0x3)
    }

    /// Replace the level field.
    #[must_use]
    pub fn with_level(word: u64, level: u8) -> u64 {
        (word & !LEVEL_MASK) | (u64::from(level & 0x3) << 2)
    }

    /// Replace the metrics region with the provided packed bits.
    #[must_use]
    pub fn with_metrics(word: u64, metrics_bits: u64) -> u64 {
        (word & !METRIC_MASK) | (metrics_bits & METRIC_MASK)
    }

    /// Pack metrics and auxiliary fields with saturation.
    #[must_use]
    pub fn pack_metrics(err: u16, mu: u16, sigma: u16, cause: u8, backoff: u8) -> u64 {
        let err_bits = u64::from(err.min(0x3fff)) << 4;
        let mu_bits = u64::from(mu) << 18;
        let sigma_bits = u64::from(sigma) << 34;
        let cause_bits = u64::from(cause) << 50;
        let backoff_bits = u64::from(backoff & 0x3f) << 58;
        err_bits | mu_bits | sigma_bits | cause_bits | backoff_bits
    }
}

/// Masks and shifts for the compact layout.
pub mod compact48 {
    /// State mask (bits 0..=1).
    pub const STATE_MASK: u64 = 0b11;
    /// Level mask (bits 2..=3).
    pub const LEVEL_MASK: u64 = 0b11 << 2;
    /// Error counter mask (bits 4..=15).
    pub const ERR_MASK: u64 = 0x0fff << 4;
    /// Mean metric mask (bits 16..=31).
    pub const MU_MASK: u64 = 0xffff << 16;
    /// Sigma metric mask (bits 32..=47).
    pub const SIGMA_MASK: u64 = 0xffff << 32;
    /// Mask covering all metric fields.
    pub const METRIC_MASK: u64 = ERR_MASK | MU_MASK | SIGMA_MASK;
    /// Mask over the state and level fields.
    pub const STATE_LEVEL_MASK: u64 = STATE_MASK | LEVEL_MASK;

    /// Extract state bits.
    #[must_use]
    pub const fn state(word: u64) -> u8 {
        (word & STATE_MASK) as u8
    }

    /// Extract level bits.
    #[must_use]
    pub const fn level(word: u64) -> u8 {
        ((word & LEVEL_MASK) >> 2) as u8
    }

    /// Extract the error counter.
    #[must_use]
    pub const fn err(word: u64) -> u16 {
        ((word & ERR_MASK) >> 4) as u16
    }

    /// Extract the mean metric.
    #[must_use]
    pub const fn mu(word: u64) -> u16 {
        ((word & MU_MASK) >> 16) as u16
    }

    /// Extract the jitter metric.
    #[must_use]
    pub const fn sigma(word: u64) -> u16 {
        ((word & SIGMA_MASK) >> 32) as u16
    }

    /// Replace the state field.
    #[must_use]
    pub fn with_state(word: u64, state: u8) -> u64 {
        (word & !STATE_MASK) | u64::from(state & 0x3)
    }

    /// Replace the level field.
    #[must_use]
    pub fn with_level(word: u64, level: u8) -> u64 {
        (word & !LEVEL_MASK) | (u64::from(level & 0x3) << 2)
    }

    /// Replace the metrics region with the provided packed bits.
    #[must_use]
    pub fn with_metrics(word: u64, metrics_bits: u64) -> u64 {
        (word & !METRIC_MASK) | (metrics_bits & METRIC_MASK)
    }

    /// Pack metrics and auxiliary fields with saturation.
    #[must_use]
    pub fn pack_metrics(err: u16, mu: u16, sigma: u16) -> u64 {
        let err_bits = u64::from(err.min(0x0fff)) << 4;
        let mu_bits = u64::from(mu) << 16;
        let sigma_bits = u64::from(sigma) << 32;
        err_bits | mu_bits | sigma_bits
    }
}

/// Pack a normalized value using Q8.8 fixed-point representation for the standard layout.
#[must_use]
pub fn pack_q8_8(value: f32) -> u16 {
    let scaled = value * 256.0;
    if !scaled.is_finite() || scaled <= 0.0 {
        0
    } else if scaled >= 65535.0 {
        u16::MAX
    } else {
        let adjusted = scaled + 0.5;
        let rounded = adjusted as u32;
        rounded.min(u32::from(u16::MAX)) as u16
    }
}

/// Convert a packed Q8.8 value back into a floating-point ratio.
#[must_use]
pub fn unpack_q8_8(value: u16) -> f32 {
    f32::from(value) / 256.0
}

/// Pack a normalized value using Q6.10 fixed-point representation for the compact layout.
#[must_use]
pub fn pack_q6_10(value: f32) -> u16 {
    let scaled = value * 1024.0;
    if !scaled.is_finite() || scaled <= 0.0 {
        0
    } else if scaled >= 65535.0 {
        u16::MAX
    } else {
        let adjusted = scaled + 0.5;
        let rounded = adjusted as u32;
        rounded.min(u32::from(u16::MAX)) as u16
    }
}

/// Convert a packed Q6.10 value back into a floating-point ratio.
#[must_use]
pub fn unpack_q6_10(value: u16) -> f32 {
    f32::from(value) / 1024.0
}

/// Serialize a packed word using network byte order (big-endian).
#[must_use]
pub fn to_network_bytes(word: u64) -> [u8; 8] {
    word.to_be_bytes()
}

/// Deserialize a packed word from network byte order (big-endian).
#[must_use]
pub fn from_network_bytes(bytes: [u8; 8]) -> u64 {
    u64::from_be_bytes(bytes)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_standard_raw() -> impl Strategy<Value = LayoutRaw> {
        (
            0u8..=3,
            0u8..=3,
            0u16..=0x3fff,
            0u16..=u16::MAX,
            0u16..=u16::MAX,
            0u8..=u8::MAX,
            0u8..=0x3f,
        )
            .prop_map(|(state, level, err, mu, sg, cause, backoff)| LayoutRaw {
                state,
                level,
                err,
                mu_norm: mu,
                sg_norm: sg,
                cause,
                backoff,
            })
    }

    proptest! {
        #[test]
        fn standard_round_trip(raw in arb_standard_raw()) {
            let packed = Standard64::pack(raw);
            let unpacked = Standard64::unpack(packed);
            prop_assert_eq!(unpacked.state, raw.state & 0x3);
            prop_assert_eq!(unpacked.level, raw.level & 0x3);
            prop_assert_eq!(unpacked.err, raw.err.min(0x3fff));
            prop_assert_eq!(unpacked.mu_norm, raw.mu_norm);
            prop_assert_eq!(unpacked.sg_norm, raw.sg_norm);
            prop_assert_eq!(unpacked.cause, raw.cause);
            prop_assert_eq!(unpacked.backoff, raw.backoff & 0x3f);
        }
    }

    #[cfg(feature = "circuit-breaker-compact48")]
    fn arb_compact_raw() -> impl Strategy<Value = LayoutRaw> {
        (
            0u8..=3,
            0u8..=3,
            0u16..=0x0fff,
            0u16..=u16::MAX,
            0u16..=u16::MAX,
        )
            .prop_map(|(state, level, err, mu, sg)| LayoutRaw {
                state,
                level,
                err,
                mu_norm: mu,
                sg_norm: sg,
                cause: 0,
                backoff: 0,
            })
    }

    #[cfg(feature = "circuit-breaker-compact48")]
    proptest! {
        #[test]
        fn compact_round_trip(raw in arb_compact_raw()) {
            let packed = Compact48::pack(raw);
            let unpacked = Compact48::unpack(packed);
            prop_assert_eq!(unpacked.state, raw.state & 0x3);
            prop_assert_eq!(unpacked.level, raw.level & 0x3);
            prop_assert_eq!(unpacked.err, raw.err.min(0x0fff));
            prop_assert_eq!(unpacked.mu_norm, raw.mu_norm);
            prop_assert_eq!(unpacked.sg_norm, raw.sg_norm);
        }
    }

    #[test]
    fn q8_8_clamps_bounds() {
        assert_eq!(pack_q8_8(-1.0), 0);
        assert_eq!(pack_q8_8(f32::NAN), 0);
        assert_eq!(pack_q8_8(0.0), 0);
        assert_eq!(pack_q8_8(1.0), 256);
        assert_eq!(pack_q8_8(300.0), u16::MAX);
    }

    #[test]
    fn q6_10_clamps_bounds() {
        assert_eq!(pack_q6_10(-0.5), 0);
        assert_eq!(pack_q6_10(0.0), 0);
        assert_eq!(pack_q6_10(1.0), 1024);
        assert_eq!(pack_q6_10(100.0), u16::MAX);
    }

    #[test]
    fn unpack_q_formats_restore_ratios() {
        assert!((unpack_q8_8(256) - 1.0).abs() < f32::EPSILON);
        assert!((unpack_q6_10(1024) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn standard_helpers_cover_all_masks() {
        use standard64::*;

        let raw = LayoutRaw {
            state: 2,
            level: 1,
            err: 0,
            mu_norm: 0,
            sg_norm: 0,
            cause: 0,
            backoff: 0,
        };
        let base = raw.state_level_bits();
        assert_eq!(state(base), 2);
        assert_eq!(level(base), 1);

        let metrics = pack_metrics(9, 1234, 4321, 0xAB, 17);
        let combined = with_metrics(base, metrics);
        assert_eq!(err(combined), 9);
        assert_eq!(mu(combined), 1234);
        assert_eq!(sigma(combined), 4321);
        assert_eq!(cause(combined), 0xAB);
        assert_eq!(backoff(combined), 17);

        let changed_state = with_state(combined, 3);
        assert_eq!(state(changed_state), 3);
        let changed_level = with_level(changed_state, 0);
        assert_eq!(level(changed_level), 0);
    }

    #[cfg(feature = "circuit-breaker-compact48")]
    #[test]
    fn compact_helpers_cover_all_masks() {
        use compact48::*;

        let word = with_level(with_state(0, 1), 2);
        assert_eq!(state(word), 1);
        assert_eq!(level(word), 2);
        let metrics = pack_metrics(0x0fff, 200, 400);
        let combined = with_metrics(word, metrics);
        assert_eq!(err(combined), 0x0fff);
        assert_eq!(mu(combined), 200);
        assert_eq!(sigma(combined), 400);
    }

    #[test]
    fn network_round_trip_matches_native() {
        let raw = LayoutRaw {
            state: 3,
            level: 2,
            err: 1234,
            mu_norm: 5678,
            sg_norm: 8765,
            cause: 0x5A,
            backoff: 13,
        };
        let packed = Standard64::pack(raw);
        let be = to_network_bytes(packed);
        assert_eq!(be, packed.to_be_bytes());
        let round = from_network_bytes(be);
        assert_eq!(round, packed);
    }
}
