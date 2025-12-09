#![no_std]

//! ASF-256 (Atomic Slip/Fee Surface) delivers fees and a compact maker / taker
//! slippage model from a single atomic snapshot. Writers stage both 128-bit
//! words, then flip the commit flag with a release-store so readers can extract
//! coefficients with one relaxed load.

use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use core::sync::atomic::Ordering;

use portable_atomic::AtomicU128;

#[cfg(test)]
extern crate std;

#[derive(Clone, Copy)]
struct Field {
    shift: u8,
    bits: u8,
}

impl Field {
    const fn value_mask(self) -> u128 {
        if self.bits == 0 {
            0
        } else if self.bits as u32 >= 128 {
            u128::MAX
        } else {
            (1u128 << self.bits) - 1
        }
    }

    const fn mask(self) -> u128 {
        self.value_mask() << self.shift
    }
}

#[inline]
fn set_field(word: u128, field: Field, value: u128) -> u128 {
    debug_assert_eq!(value & !field.value_mask(), 0, "value exceeds field width");
    let cleared = word & !field.mask();
    cleared | ((value & field.value_mask()) << field.shift)
}

#[inline]
fn set_signed_field(word: u128, field: Field, value: i32) -> u128 {
    debug_assert!(
        field.bits > 0 && field.bits <= 24,
        "signed field width invalid"
    );
    let bits = field.bits as i32;
    let min = -(1i32 << (bits - 1));
    let max = (1i32 << (bits - 1)) - 1;
    debug_assert!(
        value >= min && value <= max,
        "signed value exceeds field width"
    );
    let mask = (1i32 << bits) - 1;
    let encoded = (value & mask) as u128;
    set_field(word, field, encoded)
}

#[inline]
fn get_field(word: u128, field: Field) -> u32 {
    ((word >> field.shift) & field.value_mask()) as u32
}

const W0_COMMIT: Field = Field { shift: 0, bits: 1 };
const W0_STALE: Field = Field { shift: 1, bits: 1 };
const W0_VER_EVEN: Field = Field { shift: 2, bits: 8 };
const W0_SEQ_HEAD: Field = Field {
    shift: 10,
    bits: 16,
};
const W0_SIZE_SCALE: Field = Field {
    shift: 26,
    bits: 16,
};
const W0_MAKER_FEE: Field = Field {
    shift: 42,
    bits: 16,
};
const W0_TAKER_FEE: Field = Field {
    shift: 58,
    bits: 16,
};
const W0_MISC_FEE: Field = Field {
    shift: 74,
    bits: 16,
};
const W0_SHARED_VOL: Field = Field {
    shift: 90,
    bits: 12,
};
const W0_SHARED_SPREAD: Field = Field {
    shift: 102,
    bits: 10,
};
const W0_AGE_BUCKET: Field = Field {
    shift: 112,
    bits: 8,
};
const W0_FLAGS: Field = Field {
    shift: 120,
    bits: 8,
};

const W1_A0_M: Field = Field { shift: 0, bits: 14 };
const W1_A1_M: Field = Field {
    shift: 14,
    bits: 12,
};
const W1_A2_M: Field = Field { shift: 26, bits: 8 };
const W1_C_M: Field = Field {
    shift: 34,
    bits: 12,
};
const W1_A0_T: Field = Field {
    shift: 46,
    bits: 14,
};
const W1_A1_T: Field = Field {
    shift: 60,
    bits: 12,
};
const W1_A2_T: Field = Field { shift: 72, bits: 8 };
const W1_C_T: Field = Field {
    shift: 80,
    bits: 12,
};
const W1_CAP_M: Field = Field {
    shift: 92,
    bits: 10,
};
const W1_CAP_T: Field = Field {
    shift: 102,
    bits: 10,
};
const W1_VER_TAIL: Field = Field {
    shift: 112,
    bits: 8,
};
const W1_RESERVED: Field = Field {
    shift: 120,
    bits: 8,
};

const AGE_BUCKET_MS: u32 = 250;

const ODD_BIT: u8 = 1;
const RESERVED_DEFAULT: u8 = 0;

/// Builder-style wrapper around ASF writer flags.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Flags(u8);

impl Flags {
    /// No flags set.
    pub const EMPTY: Self = Self(0);

    /// Construct from raw bits, discarding unsupported positions.
    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & flag::MASK.bits())
    }

    /// Retrieve the raw bit pattern.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// True when all bits in `other` are present.
    pub const fn contains(self, other: Flags) -> bool {
        (self.bits() & other.bits()) == other.bits()
    }

    /// True when any bit in `other` overlaps.
    pub const fn intersects(self, other: Flags) -> bool {
        (self.bits() & other.bits()) != 0
    }

    /// True when no flag bits are set.
    pub const fn is_empty(self) -> bool {
        self.bits() == 0
    }
}

impl fmt::Debug for Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("Flags(EMPTY)");
        }

        f.write_str("Flags(")?;
        let mut first = true;
        for (flag, label) in flag::LABELS.iter() {
            if self.contains(*flag) {
                if !first {
                    f.write_str("|")?;
                }
                f.write_str(label)?;
                first = false;
            }
        }

        if first {
            f.write_fmt(format_args!("0x{:02X}", self.bits()))?;
        }
        f.write_str(")")
    }
}

impl BitOr for Flags {
    type Output = Flags;

    fn bitor(self, rhs: Flags) -> Self::Output {
        Flags::from_bits_truncate(self.bits() | rhs.bits())
    }
}

impl BitOrAssign for Flags {
    fn bitor_assign(&mut self, rhs: Flags) {
        *self = *self | rhs;
    }
}

impl BitAnd for Flags {
    type Output = Flags;

    fn bitand(self, rhs: Flags) -> Self::Output {
        Flags::from_bits_truncate(self.bits() & rhs.bits())
    }
}

impl BitAndAssign for Flags {
    fn bitand_assign(&mut self, rhs: Flags) {
        *self = *self & rhs;
    }
}

impl Not for Flags {
    type Output = Flags;

    fn not(self) -> Self::Output {
        Flags::from_bits_truncate(!self.bits())
    }
}

impl From<u8> for Flags {
    fn from(value: u8) -> Self {
        Flags::from_bits_truncate(value)
    }
}

impl From<Flags> for u8 {
    fn from(value: Flags) -> Self {
        value.bits()
    }
}

impl Default for Flags {
    fn default() -> Self {
        Flags::EMPTY
    }
}

/// Exposed flag constants.
pub mod flag {
    use super::Flags;

    /// Estimator has sufficient maker data.
    pub const HAS_DATA_M: Flags = Flags(1 << 0);
    /// Estimator has sufficient taker data.
    pub const HAS_DATA_T: Flags = Flags(1 << 1);
    /// Regime change detected; coefficients in-flight.
    pub const REGIME_SHIFT: Flags = Flags(1 << 2);
    /// Model is relying on conservative fallback settings.
    pub const FALLBACK_MODE: Flags = Flags(1 << 3);
    /// Slip learning recently reset; expect higher variance.
    pub const RESET: Flags = Flags(1 << 4);
    /// Spare bit for venue-specific wiring.
    pub const SPARE_0: Flags = Flags(1 << 5);
    /// Spare bit for downstream policy.
    pub const SPARE_1: Flags = Flags(1 << 6);
    /// Spare bit for future variants.
    pub const SPARE_2: Flags = Flags(1 << 7);

    /// Mask of all supported flags.
    pub const MASK: Flags = Flags(
        HAS_DATA_M.bits()
            | HAS_DATA_T.bits()
            | REGIME_SHIFT.bits()
            | FALLBACK_MODE.bits()
            | RESET.bits()
            | SPARE_0.bits()
            | SPARE_1.bits()
            | SPARE_2.bits(),
    );

    pub(crate) const LABELS: &[(Flags, &str)] = &[
        (HAS_DATA_M, "HAS_DATA_M"),
        (HAS_DATA_T, "HAS_DATA_T"),
        (REGIME_SHIFT, "REGIME_SHIFT"),
        (FALLBACK_MODE, "FALLBACK_MODE"),
        (RESET, "RESET"),
    ];
}

fn quantize_unsigned(value: f32, total_bits: u8, frac_bits: u8) -> u32 {
    if value <= 0.0 {
        return 0;
    }
    let scale = (1u32 << frac_bits) as f32;
    let max = ((1u32 << total_bits) - 1) as f32 / scale;
    let clamped = if value > max { max } else { value };
    (clamped * scale + 0.5) as u32
}

fn quantize_signed(value: f32, total_bits: u8, frac_bits: u8) -> i32 {
    let scale = (1i32 << frac_bits) as f32;
    let max = ((1i32 << (total_bits - 1)) - 1) as f32 / scale;
    let min = (-(1i32 << (total_bits - 1))) as f32 / scale;
    let mut clamped = value;
    if clamped > max {
        clamped = max;
    } else if clamped < min {
        clamped = min;
    }
    let scaled = clamped * scale;
    if scaled >= 0.0 {
        (scaled + 0.5) as i32
    } else {
        (scaled - 0.5) as i32
    }
}

fn dequantize_unsigned(raw: u32, frac_bits: u8) -> f32 {
    raw as f32 / (1u32 << frac_bits) as f32
}

fn dequantize_signed(raw: u32, total_bits: u8, frac_bits: u8) -> f32 {
    let shift = 32 - total_bits;
    let signed = ((raw << shift) as i32) >> shift;
    signed as f32 / (1u32 << frac_bits) as f32
}

/// Quantized maker / taker lane coefficients.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LaneQuantized {
    pub intercept_q6_8: u16,
    pub size_linear_q4_8: u16,
    pub size_quadratic_q0_8: u8,
    pub latency_q4_8: u16,
    pub slip_cap_bp: u16,
}

impl LaneQuantized {
    fn encode(
        word: u128,
        coeffs: &LanePublish,
        a0_field: Field,
        a1_field: Field,
        a2_field: Field,
        c_field: Field,
        cap_field: Field,
    ) -> u128 {
        let intercept = quantize_signed(coeffs.intercept_bp, a0_field.bits, 8);
        let size_linear = quantize_unsigned(coeffs.size_linear_bp.max(0.0), a1_field.bits, 8);
        let size_quadratic = quantize_unsigned(coeffs.size_quadratic_bp.max(0.0), a2_field.bits, 8);
        let latency = quantize_unsigned(coeffs.latency_coeff_bp.max(0.0), c_field.bits, 8);
        let cap = (coeffs.slip_cap_bp.clamp(0.0, 1023.0) + 0.5) as u32;

        let word = set_signed_field(word, a0_field, intercept);
        let word = set_field(word, a1_field, size_linear as u128);
        let word = set_field(word, a2_field, size_quadratic as u128);
        let word = set_field(word, c_field, latency as u128);
        set_field(word, cap_field, cap as u128)
    }

    fn from_word(
        word: u128,
        a0_field: Field,
        a1_field: Field,
        a2_field: Field,
        c_field: Field,
        cap_field: Field,
    ) -> Self {
        Self {
            intercept_q6_8: get_field(word, a0_field) as u16,
            size_linear_q4_8: get_field(word, a1_field) as u16,
            size_quadratic_q0_8: get_field(word, a2_field) as u8,
            latency_q4_8: get_field(word, c_field) as u16,
            slip_cap_bp: get_field(word, cap_field) as u16,
        }
    }

    fn decode(self) -> LaneCoefficients {
        LaneCoefficients {
            intercept_bp: dequantize_signed(self.intercept_q6_8 as u32, 14, 8),
            size_linear_bp: dequantize_unsigned(self.size_linear_q4_8 as u32, 8),
            size_quadratic_bp: dequantize_unsigned(self.size_quadratic_q0_8 as u32, 8),
            latency_coeff_bp: dequantize_unsigned(self.latency_q4_8 as u32, 8),
            slip_cap_bp: self.slip_cap_bp as f32,
        }
    }
}

/// Lane coefficients in native units (basis points).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LaneCoefficients {
    pub intercept_bp: f32,
    pub size_linear_bp: f32,
    pub size_quadratic_bp: f32,
    pub latency_coeff_bp: f32,
    pub slip_cap_bp: f32,
}

impl LaneCoefficients {
    fn estimate(
        &self,
        size_k: f32,
        shared_vol: f32,
        shared_spread: f32,
        vol_bp: f32,
        spread_ticks: f32,
        lat_metric: f32,
    ) -> f32 {
        let mut value = self.intercept_bp
            + self.size_linear_bp * size_k
            + self.size_quadratic_bp * size_k * size_k
            + shared_vol * vol_bp
            + shared_spread * spread_ticks
            + self.latency_coeff_bp * lat_metric;
        if value > self.slip_cap_bp {
            value = self.slip_cap_bp;
        }
        value
    }
}

/// Writer-facing lane configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LanePublish {
    pub intercept_bp: f32,
    pub size_linear_bp: f32,
    pub size_quadratic_bp: f32,
    pub latency_coeff_bp: f32,
    pub slip_cap_bp: f32,
}

/// Writer payload for a single publish operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AsfPublish {
    pub odd_version: u8,
    pub seq_head: u16,
    pub size_scale: f32,
    pub maker_fee_bp: f32,
    pub taker_fee_bp: f32,
    pub misc_fee_bp: f32,
    pub shared_vol_coeff_bp: f32,
    pub shared_spread_coeff_bp: f32,
    pub age_ms: u32,
    pub flags: Flags,
    pub stale: bool,
    pub commit: bool,
    pub maker: LanePublish,
    pub taker: LanePublish,
}

impl Default for AsfPublish {
    fn default() -> Self {
        Self {
            odd_version: 1,
            seq_head: 0,
            size_scale: 1.0,
            maker_fee_bp: 0.0,
            taker_fee_bp: 0.0,
            misc_fee_bp: 0.0,
            shared_vol_coeff_bp: 0.0,
            shared_spread_coeff_bp: 0.0,
            age_ms: 0,
            flags: Flags::EMPTY,
            stale: false,
            commit: true,
            maker: LanePublish::default(),
            taker: LanePublish::default(),
        }
    }
}

/// Builder for `AsfPublish`.
pub struct AsfSnapshotBuilder {
    inner: AsfPublish,
}

impl AsfSnapshotBuilder {
    pub fn new() -> Self {
        Self {
            inner: AsfPublish::default(),
        }
    }

    pub fn builder() -> Self {
        Self::new()
    }

    pub fn with_size_scale(mut self, size_scale: f32) -> Self {
        self.inner.size_scale = size_scale;
        self
    }

    pub fn with_maker_fee_bp(mut self, fee: f32) -> Self {
        self.inner.maker_fee_bp = fee;
        self
    }

    pub fn with_taker_fee_bp(mut self, fee: f32) -> Self {
        self.inner.taker_fee_bp = fee;
        self
    }

    pub fn with_misc_fee_bp(mut self, fee: f32) -> Self {
        self.inner.misc_fee_bp = fee;
        self
    }

    pub fn with_shared_vol_coeff(mut self, coeff: f32) -> Self {
        self.inner.shared_vol_coeff_bp = coeff;
        self
    }

    pub fn with_shared_spread_coeff(mut self, coeff: f32) -> Self {
        self.inner.shared_spread_coeff_bp = coeff;
        self
    }

    pub fn with_age_ms(mut self, age_ms: u32) -> Self {
        self.inner.age_ms = age_ms;
        self
    }

    pub fn with_flags(mut self, flags: Flags) -> Self {
        self.inner.flags = flags;
        self
    }

    pub fn with_stale(mut self, stale: bool) -> Self {
        self.inner.stale = stale;
        self
    }

    pub fn with_commit(mut self, commit: bool) -> Self {
        self.inner.commit = commit;
        self
    }

    pub fn with_versions(mut self, odd_version: u8, seq_head: u16) -> Self {
        self.inner.odd_version = odd_version;
        self.inner.seq_head = seq_head;
        self
    }

    pub fn with_maker_lane<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(LanePublish) -> LanePublish,
    {
        self.inner.maker = configure(self.inner.maker);
        self
    }

    pub fn with_taker_lane<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(LanePublish) -> LanePublish,
    {
        self.inner.taker = configure(self.inner.taker);
        self
    }

    pub fn build(self) -> AsfPublish {
        self.inner
    }
}

impl Default for AsfSnapshotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AsfQuantized {
    pub commit: bool,
    pub stale: bool,
    pub version_even: u8,
    pub version_tail: u8,
    pub seq_head: u16,
    pub size_scale_q8_8: u16,
    pub maker_fee_q8_8: u16,
    pub taker_fee_q8_8: u16,
    pub misc_fee_q8_8: u16,
    pub shared_vol_q4_8: u16,
    pub shared_spread_q2_8: u16,
    pub age_bucket: u8,
    pub flags: Flags,
    pub maker: LaneQuantized,
    pub taker: LaneQuantized,
    pub reserved: u8,
}

impl AsfQuantized {
    fn decode(self) -> AsfSnapshot {
        AsfSnapshot {
            commit: self.commit,
            stale: self.stale,
            version_even: self.version_even,
            version_tail: self.version_tail,
            seq_head: self.seq_head,
            size_scale: dequantize_unsigned(self.size_scale_q8_8 as u32, 8),
            maker_fee_bp: dequantize_unsigned(self.maker_fee_q8_8 as u32, 8),
            taker_fee_bp: dequantize_unsigned(self.taker_fee_q8_8 as u32, 8),
            misc_fee_bp: dequantize_unsigned(self.misc_fee_q8_8 as u32, 8),
            shared_vol_coeff_bp: dequantize_unsigned(self.shared_vol_q4_8 as u32, 8),
            shared_spread_coeff_bp: dequantize_unsigned(self.shared_spread_q2_8 as u32, 8),
            age_bucket: self.age_bucket,
            flags: self.flags,
            maker: self.maker.decode(),
            taker: self.taker.decode(),
        }
    }
}

/// Snapshot exposed to readers in physical units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AsfSnapshot {
    pub commit: bool,
    pub stale: bool,
    pub version_even: u8,
    pub version_tail: u8,
    pub seq_head: u16,
    pub size_scale: f32,
    pub maker_fee_bp: f32,
    pub taker_fee_bp: f32,
    pub misc_fee_bp: f32,
    pub shared_vol_coeff_bp: f32,
    pub shared_spread_coeff_bp: f32,
    pub age_bucket: u8,
    pub flags: Flags,
    pub maker: LaneCoefficients,
    pub taker: LaneCoefficients,
}

impl AsfSnapshot {
    /// Compute milliseconds represented by the stored age bucket (250 ms each).
    pub fn age_ms(&self) -> u32 {
        self.age_bucket as u32 * AGE_BUCKET_MS
    }

    /// True when the snapshot is considered stale for the supplied budget.
    pub fn is_stale_for(&self, budget_ms: u32) -> bool {
        self.stale || self.age_ms() > budget_ms
    }

    /// Maker fee including misc components.
    pub fn maker_total_fee_bp(&self) -> f32 {
        self.maker_fee_bp + self.misc_fee_bp
    }

    /// Taker fee including misc components.
    pub fn taker_total_fee_bp(&self) -> f32 {
        self.taker_fee_bp + self.misc_fee_bp
    }

    /// Estimate maker slip in basis points.
    pub fn estimate_maker_slip_bp(
        &self,
        size_contracts: f32,
        vol_bp: f32,
        spread_ticks: f32,
        rtt_ms: f32,
        jitter_ms: f32,
        jitter_weight: f32,
    ) -> f32 {
        self.estimate_slip_bp(
            Lane::Maker,
            size_contracts,
            vol_bp,
            spread_ticks,
            rtt_ms,
            jitter_ms,
            jitter_weight,
        )
    }

    /// Estimate taker slip in basis points.
    pub fn estimate_taker_slip_bp(
        &self,
        size_contracts: f32,
        vol_bp: f32,
        spread_ticks: f32,
        rtt_ms: f32,
        jitter_ms: f32,
        jitter_weight: f32,
    ) -> f32 {
        self.estimate_slip_bp(
            Lane::Taker,
            size_contracts,
            vol_bp,
            spread_ticks,
            rtt_ms,
            jitter_ms,
            jitter_weight,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn estimate_slip_bp(
        &self,
        lane: Lane,
        size_contracts: f32,
        vol_bp: f32,
        spread_ticks: f32,
        rtt_ms: f32,
        jitter_ms: f32,
        jitter_weight: f32,
    ) -> f32 {
        let size_k = size_contracts * self.size_scale;
        let lat_metric = rtt_ms + jitter_weight * jitter_ms;
        let shared_vol = self.shared_vol_coeff_bp;
        let shared_spread = self.shared_spread_coeff_bp;
        let coeffs = match lane {
            Lane::Maker => &self.maker,
            Lane::Taker => &self.taker,
        };
        coeffs.estimate(
            size_k,
            shared_vol,
            shared_spread,
            vol_bp,
            spread_ticks,
            lat_metric,
        )
    }
}

/// Lane selection for slip estimation.
#[derive(Clone, Copy, Debug)]
pub enum Lane {
    Maker,
    Taker,
}

/// Packed ASF payload (two 128-bit words).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AsfPacked {
    w0: u128,
    w1: u128,
}

impl AsfPacked {
    pub const fn from_words(w0: u128, w1: u128) -> Self {
        Self { w0, w1 }
    }

    pub const fn words(self) -> (u128, u128) {
        (self.w0, self.w1)
    }

    pub fn quantized(self) -> AsfQuantized {
        let maker_q =
            LaneQuantized::from_word(self.w1, W1_A0_M, W1_A1_M, W1_A2_M, W1_C_M, W1_CAP_M);
        let taker_q =
            LaneQuantized::from_word(self.w1, W1_A0_T, W1_A1_T, W1_A2_T, W1_C_T, W1_CAP_T);
        AsfQuantized {
            commit: get_field(self.w0, W0_COMMIT) != 0,
            stale: get_field(self.w0, W0_STALE) != 0,
            version_even: get_field(self.w0, W0_VER_EVEN) as u8,
            version_tail: get_field(self.w1, W1_VER_TAIL) as u8,
            seq_head: get_field(self.w0, W0_SEQ_HEAD) as u16,
            size_scale_q8_8: get_field(self.w0, W0_SIZE_SCALE) as u16,
            maker_fee_q8_8: get_field(self.w0, W0_MAKER_FEE) as u16,
            taker_fee_q8_8: get_field(self.w0, W0_TAKER_FEE) as u16,
            misc_fee_q8_8: get_field(self.w0, W0_MISC_FEE) as u16,
            shared_vol_q4_8: get_field(self.w0, W0_SHARED_VOL) as u16,
            shared_spread_q2_8: get_field(self.w0, W0_SHARED_SPREAD) as u16,
            age_bucket: get_field(self.w0, W0_AGE_BUCKET) as u8,
            flags: Flags::from(get_field(self.w0, W0_FLAGS) as u8),
            maker: maker_q,
            taker: taker_q,
            reserved: get_field(self.w1, W1_RESERVED) as u8,
        }
    }

    pub fn snapshot(self) -> AsfSnapshot {
        self.quantized().decode()
    }

    pub fn from_publish(publish: &AsfPublish) -> Self {
        let mut w0 = 0u128;
        let mut w1 = 0u128;

        let odd_version = publish.odd_version | ODD_BIT;
        let even_version = odd_version.wrapping_add(1);

        if publish.commit {
            w0 = set_field(w0, W0_COMMIT, 1);
        }
        if publish.stale {
            w0 = set_field(w0, W0_STALE, 1);
        }
        w0 = set_field(w0, W0_VER_EVEN, even_version as u128);
        w0 = set_field(w0, W0_SEQ_HEAD, publish.seq_head as u128);
        w0 = set_field(
            w0,
            W0_SIZE_SCALE,
            quantize_unsigned(publish.size_scale.max(0.0), W0_SIZE_SCALE.bits, 8) as u128,
        );
        w0 = set_field(
            w0,
            W0_MAKER_FEE,
            quantize_unsigned(publish.maker_fee_bp.max(0.0), W0_MAKER_FEE.bits, 8) as u128,
        );
        w0 = set_field(
            w0,
            W0_TAKER_FEE,
            quantize_unsigned(publish.taker_fee_bp.max(0.0), W0_TAKER_FEE.bits, 8) as u128,
        );
        w0 = set_field(
            w0,
            W0_MISC_FEE,
            quantize_unsigned(publish.misc_fee_bp.max(0.0), W0_MISC_FEE.bits, 8) as u128,
        );
        w0 = set_field(
            w0,
            W0_SHARED_VOL,
            quantize_unsigned(publish.shared_vol_coeff_bp.max(0.0), W0_SHARED_VOL.bits, 8) as u128,
        );
        w0 = set_field(
            w0,
            W0_SHARED_SPREAD,
            quantize_unsigned(
                publish.shared_spread_coeff_bp.max(0.0),
                W0_SHARED_SPREAD.bits,
                8,
            ) as u128,
        );
        let age_bucket = (publish.age_ms / AGE_BUCKET_MS).min(u32::from(u8::MAX));
        w0 = set_field(w0, W0_AGE_BUCKET, age_bucket as u128);
        w0 = set_field(w0, W0_FLAGS, publish.flags.bits() as u128);

        w1 = LaneQuantized::encode(
            w1,
            &publish.maker,
            W1_A0_M,
            W1_A1_M,
            W1_A2_M,
            W1_C_M,
            W1_CAP_M,
        );
        w1 = LaneQuantized::encode(
            w1,
            &publish.taker,
            W1_A0_T,
            W1_A1_T,
            W1_A2_T,
            W1_C_T,
            W1_CAP_T,
        );
        w1 = set_field(w1, W1_VER_TAIL, odd_version as u128);
        w1 = set_field(w1, W1_RESERVED, RESERVED_DEFAULT as u128);

        Self { w0, w1 }
    }
}

/// Atomic ASF capsule (two 128-bit words, 64-byte aligned).
#[repr(C, align(64))]
pub struct Asf256 {
    word0: AtomicU128,
    word1: AtomicU128,
}

impl Asf256 {
    pub const fn new() -> Self {
        Self {
            word0: AtomicU128::new(0),
            word1: AtomicU128::new(0),
        }
    }

    pub fn publish(&self, publish: &AsfPublish) -> AsfSnapshot {
        let packed = AsfPacked::from_publish(publish);
        let (w0_final, w1) = packed.words();
        let w0_inflight = set_field(w0_final, W0_COMMIT, 0);

        self.word1.store(w1, Ordering::Relaxed);
        self.word0.store(w0_inflight, Ordering::Relaxed);
        self.word0.store(w0_final, Ordering::Release);
        packed.snapshot()
    }

    pub fn load_relaxed(&self) -> Option<AsfSnapshot> {
        for _ in 0..8 {
            let w0_first = self.word0.load(Ordering::Relaxed);
            if get_field(w0_first, W0_COMMIT) == 0 {
                return None;
            }
            if get_field(w0_first, W0_VER_EVEN) & 1 != 0 {
                continue;
            }

            let w1 = self.word1.load(Ordering::Relaxed);
            let w0_second = self.word0.load(Ordering::Acquire);
            if w0_first != w0_second {
                continue;
            }
            if get_field(w0_second, W0_COMMIT) == 0 {
                continue;
            }
            let ver_even = get_field(w0_second, W0_VER_EVEN) as u8;
            if ver_even & 1 != 0 {
                continue;
            }
            let ver_tail = get_field(w1, W1_VER_TAIL) as u8;
            if ver_even != (ver_tail.wrapping_add(1)) {
                continue;
            }

            let packed = AsfPacked::from_words(w0_second, w1);
            return Some(packed.snapshot());
        }
        None
    }
}

impl Default for Asf256 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sample_publish() -> AsfPublish {
        AsfSnapshotBuilder::builder()
            .with_size_scale(0.25)
            .with_maker_fee_bp(0.42)
            .with_taker_fee_bp(0.42)
            .with_misc_fee_bp(0.10)
            .with_shared_vol_coeff(0.50)
            .with_shared_spread_coeff(0.60)
            .with_age_ms(1_250)
            .with_flags(flag::HAS_DATA_M | flag::HAS_DATA_T)
            .with_versions(1, 77)
            .with_maker_lane(|_lane| LanePublish {
                intercept_bp: 0.40,
                size_linear_bp: 0.15,
                size_quadratic_bp: 0.05,
                latency_coeff_bp: 0.20,
                slip_cap_bp: 6.0,
            })
            .with_taker_lane(|_lane| LanePublish {
                intercept_bp: 0.25,
                size_linear_bp: 0.08,
                size_quadratic_bp: 0.02,
                latency_coeff_bp: 0.10,
                slip_cap_bp: 10.0,
            })
            .build()
    }

    #[test]
    fn pack_roundtrip_snapshot() {
        let publish = sample_publish();
        let packed = AsfPacked::from_publish(&publish);
        let snapshot = packed.snapshot();

        assert!(snapshot.commit);
        assert_eq!(snapshot.version_even & 1, 0);
        assert_eq!(
            snapshot.version_even,
            (publish.odd_version | 1).wrapping_add(1)
        );
        assert_eq!(snapshot.seq_head, publish.seq_head);
        assert!(snapshot.flags.contains(flag::HAS_DATA_M));
        assert!(snapshot.flags.contains(flag::HAS_DATA_T));

        let epsilon = 0.01;
        assert!((snapshot.size_scale - publish.size_scale).abs() < epsilon);
        assert!((snapshot.maker_fee_bp - publish.maker_fee_bp).abs() < epsilon);
        assert!((snapshot.taker_fee_bp - publish.taker_fee_bp).abs() < epsilon);
        assert!((snapshot.misc_fee_bp - publish.misc_fee_bp).abs() < epsilon);
        assert!((snapshot.shared_vol_coeff_bp - publish.shared_vol_coeff_bp).abs() < epsilon);
        assert!((snapshot.shared_spread_coeff_bp - publish.shared_spread_coeff_bp).abs() < epsilon);
        assert_eq!(snapshot.age_bucket as u32, publish.age_ms / AGE_BUCKET_MS);
        assert!((snapshot.maker.intercept_bp - publish.maker.intercept_bp).abs() < epsilon);
        assert!((snapshot.taker.intercept_bp - publish.taker.intercept_bp).abs() < epsilon);
    }

    #[test]
    fn publish_and_load_snapshot() {
        let slot = Asf256::new();
        let publish = sample_publish();
        slot.publish(&publish);
        let snapshot = slot.load_relaxed().expect("snapshot");
        assert!(snapshot.commit);
        assert!(!snapshot.is_stale_for(10_000));
        assert!(snapshot.flags.contains(flag::HAS_DATA_M));
    }

    proptest! {
        #[test]
        fn prop_pack_unpack(
            size_scale in 0f32..4f32,
            maker_fee in 0f32..10f32,
            taker_fee in 0f32..10f32,
            misc_fee in 0f32..5f32,
            shared_vol in 0f32..8f32,
            shared_spread in 0f32..4f32,
            age_ms in 0u32..80_000u32,
            intercept_m in -8f32..8f32,
            intercept_t in -8f32..8f32,
            size_linear_m in 0f32..4f32,
            size_linear_t in 0f32..4f32,
            size_quad_m in 0f32..1f32,
            size_quad_t in 0f32..1f32,
            latency_m in 0f32..4f32,
            latency_t in 0f32..4f32,
            cap_m in 0f32..64f32,
            cap_t in 0f32..64f32,
        ) {
            let publish = AsfSnapshotBuilder::builder()
                .with_size_scale(size_scale)
                .with_maker_fee_bp(maker_fee)
                .with_taker_fee_bp(taker_fee)
                .with_misc_fee_bp(misc_fee)
                .with_shared_vol_coeff(shared_vol)
                .with_shared_spread_coeff(shared_spread)
                .with_age_ms(age_ms)
                .with_versions(1, 0)
                .with_maker_lane(|_| LanePublish {
                    intercept_bp: intercept_m,
                    size_linear_bp: size_linear_m,
                    size_quadratic_bp: size_quad_m,
                    latency_coeff_bp: latency_m,
                    slip_cap_bp: cap_m,
                })
                .with_taker_lane(|_| LanePublish {
                    intercept_bp: intercept_t,
                    size_linear_bp: size_linear_t,
                    size_quadratic_bp: size_quad_t,
                    latency_coeff_bp: latency_t,
                    slip_cap_bp: cap_t,
                })
                .build();

            let packed = AsfPacked::from_publish(&publish);
            let snapshot = packed.snapshot();

            let epsilon = 0.05;
            prop_assert!((snapshot.size_scale - publish.size_scale).abs() < epsilon);
            prop_assert!((snapshot.maker_fee_bp - publish.maker_fee_bp).abs() < epsilon);
            prop_assert!((snapshot.taker_fee_bp - publish.taker_fee_bp).abs() < epsilon);
            prop_assert!((snapshot.misc_fee_bp - publish.misc_fee_bp).abs() < epsilon);
            prop_assert!((snapshot.shared_vol_coeff_bp - publish.shared_vol_coeff_bp).abs() < epsilon);
            prop_assert!((snapshot.shared_spread_coeff_bp - publish.shared_spread_coeff_bp).abs() < epsilon);
            prop_assert!((snapshot.maker.intercept_bp - publish.maker.intercept_bp).abs() < epsilon * 2.0);
            prop_assert!((snapshot.taker.intercept_bp - publish.taker.intercept_bp).abs() < epsilon * 2.0);
            prop_assert!(snapshot.maker.slip_cap_bp >= 0.0);
            prop_assert!(snapshot.taker.slip_cap_bp >= 0.0);
        }
    }

    #[test]
    fn slip_estimation_matches_formula() {
        let publish = AsfSnapshotBuilder::builder()
            .with_size_scale(0.5)
            .with_shared_vol_coeff(0.3)
            .with_shared_spread_coeff(0.2)
            .with_maker_lane(|_| LanePublish {
                intercept_bp: 1.0,
                size_linear_bp: 0.6,
                size_quadratic_bp: 0.1,
                latency_coeff_bp: 0.4,
                slip_cap_bp: 20.0,
            })
            .with_taker_lane(|_| LanePublish {
                intercept_bp: 1.4,
                size_linear_bp: 0.7,
                size_quadratic_bp: 0.1,
                latency_coeff_bp: 0.5,
                slip_cap_bp: 20.0,
            })
            .build();
        let snapshot = AsfPacked::from_publish(&publish).snapshot();

        let contracts = 8.0;
        let vol_bp = 12.0;
        let spread_ticks = 3.0;
        let rtt_ms = 4.0;
        let jitter_ms = 2.0;
        let lambda = 0.5;

        let size_k = contracts * snapshot.size_scale;
        let lat_metric = rtt_ms + lambda * jitter_ms;
        let expected = snapshot.maker.intercept_bp
            + snapshot.maker.size_linear_bp * size_k
            + snapshot.maker.size_quadratic_bp * size_k * size_k
            + snapshot.shared_vol_coeff_bp * vol_bp
            + snapshot.shared_spread_coeff_bp * spread_ticks
            + snapshot.maker.latency_coeff_bp * lat_metric;

        let estimate = snapshot.estimate_maker_slip_bp(
            contracts,
            vol_bp,
            spread_ticks,
            rtt_ms,
            jitter_ms,
            lambda,
        );

        assert!((estimate - expected).abs() < 0.25);
    }
}
