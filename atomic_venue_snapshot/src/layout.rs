//! Bit-field layout and fixed-point helpers for the AVS-128 capsule.

/// Width in bits for the `spread_ticks` field.
pub const SPREAD_TICKS_WIDTH: u32 = 8;
/// Bit offset for the `spread_ticks` field (MSB-aligned).
pub const SPREAD_TICKS_SHIFT: u32 = 120;
/// Maximum representable spread in ticks.
pub const SPREAD_TICKS_MAX: u8 = ((1u32 << SPREAD_TICKS_WIDTH) - 1) as u8;

/// Width in bits for the `obi_q1_10` field.
pub const OBI_Q1_10_WIDTH: u32 = 12;
/// Bit offset for the `obi_q1_10` field.
pub const OBI_Q1_10_SHIFT: u32 = 108;
/// Scaling factor for Q1.10 imbalance values.
pub const OBI_Q1_10_SCALE: i32 = 1 << 10;
/// Minimum raw value for OBI (−1.000 represented as −1024).
pub const OBI_Q1_10_MIN: i16 = -(OBI_Q1_10_SCALE as i16);
/// Maximum raw value for OBI (+0.999 represented as +1023).
pub const OBI_Q1_10_MAX: i16 = (OBI_Q1_10_SCALE as i16) - 1;

/// Width in bits for the `micro_off_ticks` field.
pub const MICRO_OFF_TICKS_WIDTH: u32 = 12;
/// Bit offset for the `micro_off_ticks` field.
pub const MICRO_OFF_TICKS_SHIFT: u32 = 96;
/// Minimum raw microprice offset in ticks.
pub const MICRO_OFF_TICKS_MIN: i16 = (-(1i32 << (MICRO_OFF_TICKS_WIDTH - 1))) as i16;
/// Maximum raw microprice offset in ticks.
pub const MICRO_OFF_TICKS_MAX: i16 = ((1i32 << (MICRO_OFF_TICKS_WIDTH - 1)) - 1) as i16;

/// Width in bits for the `sum_bid_l1_3` field.
pub const SUM_BID_L1_3_WIDTH: u32 = 16;
/// Bit offset for the `sum_bid_l1_3` field.
pub const SUM_BID_L1_3_SHIFT: u32 = 80;
/// Maximum representable cumulative bid size.
pub const SUM_BID_L1_3_MAX: u16 = u16::MAX;

/// Width in bits for the `sum_ask_l1_3` field.
pub const SUM_ASK_L1_3_WIDTH: u32 = 16;
/// Bit offset for the `sum_ask_l1_3` field.
pub const SUM_ASK_L1_3_SHIFT: u32 = 64;
/// Maximum representable cumulative ask size.
pub const SUM_ASK_L1_3_MAX: u16 = u16::MAX;

/// Width in bits for the `vol_bp_q8_8` field.
pub const VOL_BP_Q8_8_WIDTH: u32 = 16;
/// Bit offset for the `vol_bp_q8_8` field.
pub const VOL_BP_Q8_8_SHIFT: u32 = 48;
/// Scaling factor for volatility encoded as Q8.8 basis points.
pub const VOL_BP_Q8_8_SCALE: u32 = 1 << 8;
/// Maximum raw volatility value (≈255.996 bp).
pub const VOL_BP_Q8_8_MAX: u16 = u16::MAX;

/// Bit offset for the single-bit `sweep_flag` field.
pub const SWEEP_FLAG_SHIFT: u32 = 47;

/// Width in bits for the `trend_200ms_ticks` field.
pub const TREND_200MS_TICKS_WIDTH: u32 = 11;
/// Bit offset for the `trend_200ms_ticks` field.
pub const TREND_200MS_TICKS_SHIFT: u32 = 36;
/// Minimum raw mid-price trend (ticks).
pub const TREND_200MS_TICKS_MIN: i16 = (-(1i32 << (TREND_200MS_TICKS_WIDTH - 1))) as i16;
/// Maximum raw mid-price trend (ticks).
pub const TREND_200MS_TICKS_MAX: i16 = ((1i32 << (TREND_200MS_TICKS_WIDTH - 1)) - 1) as i16;

/// Width in bits for the coarse millisecond timestamp.
pub const TS_COARSE_MS_WIDTH: u32 = 24;
/// Bit offset for the coarse millisecond timestamp.
pub const TS_COARSE_MS_SHIFT: u32 = 12;
/// Timestamp granularity in milliseconds (`ms / 4`).
pub const TS_COARSE_MS_GRANULARITY: u32 = 4;
/// Maximum raw coarse timestamp.
pub const TS_COARSE_MS_MAX: u32 = (1u32 << TS_COARSE_MS_WIDTH) - 1;

/// Width in bits for the `ver` field.
pub const VERSION_WIDTH: u32 = 8;
/// Bit offset for the `ver` field.
pub const VERSION_SHIFT: u32 = 4;

/// Width in bits for the `seq` field.
pub const SEQUENCE_WIDTH: u32 = 4;
/// Bit offset for the `seq` field.
pub const SEQUENCE_SHIFT: u32 = 0;
/// Maximum raw sequence counter value.
pub const SEQUENCE_MAX: u8 = ((1u32 << SEQUENCE_WIDTH) - 1) as u8;

/// Clamp an imbalance ratio (scaled by `OBI_Q1_10_SCALE`) to the encodable range.
#[inline]
#[must_use] 
pub const fn clamp_obi(raw: i32) -> i16 {
    if raw < OBI_Q1_10_MIN as i32 {
        OBI_Q1_10_MIN
    } else if raw > OBI_Q1_10_MAX as i32 {
        OBI_Q1_10_MAX
    } else {
        raw as i16
    }
}

/// Compute signed Q1.10 order-book imbalance from depth sums.
///
/// Returns zero when both sums are zero.
#[inline]
#[must_use] 
pub fn obi_from_depths(sum_bid: u64, sum_ask: u64) -> i16 {
    if sum_bid == 0 && sum_ask == 0 {
        return 0;
    }

    let num = i128::from(sum_bid) - i128::from(sum_ask);
    let den = i128::from(sum_bid) + i128::from(sum_ask);
    let scaled = num << 10;
    let rounded = if num >= 0 {
        (scaled + (den >> 1)) / den
    } else {
        (scaled - (den >> 1)) / den
    };
    clamp_obi(rounded as i32)
}

/// Convert a raw Q1.10 imbalance value back to a floating point ratio.
#[inline]
#[must_use] 
pub fn obi_to_ratio(raw: i16) -> f32 {
    f32::from(raw) / (OBI_Q1_10_SCALE as f32)
}

/// Encode a volatility value expressed in basis points (bp) into Q8.8 form.
#[inline]
#[must_use] 
pub fn encode_vol_bp_q8_8(bp: f32) -> u16 {
    if !bp.is_finite() || bp <= 0.0 {
        return 0;
    }

    let scale = VOL_BP_Q8_8_SCALE as f32;
    let capped = (bp * scale).min(f32::from(VOL_BP_Q8_8_MAX));
    let rounded = (capped + 0.5) as u32;
    rounded.min(u32::from(u16::MAX)) as u16
}

/// Decode a Q8.8 basis point volatility reading back into floating point.
#[inline]
#[must_use] 
pub fn decode_vol_bp_q8_8(raw: u16) -> f32 {
    f32::from(raw) / (VOL_BP_Q8_8_SCALE as f32)
}

/// Quantise a millisecond timestamp into the coarse `ms/4` domain.
#[inline]
#[must_use] 
pub const fn quantise_timestamp_ms(timestamp_ms: u64) -> u32 {
    let quantised = (timestamp_ms >> 2);
    if quantised > TS_COARSE_MS_MAX as u64 {
        TS_COARSE_MS_MAX
    } else {
        quantised as u32
    }
}

/// Expand a coarse timestamp back into milliseconds (lower bits lost during quantisation).
#[inline]
#[must_use] 
pub const fn dequantise_timestamp_ms(coarse: u32) -> u64 {
    (coarse as u64) << 2
}
