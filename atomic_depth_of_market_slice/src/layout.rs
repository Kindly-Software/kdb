//! Bit layout helpers and fixed-point utilities for the DOS-1024 capsule.

/// Number of 128-bit words in a DOS capsule.
pub const WORD_COUNT: usize = 8;

/// Width of the version field (stored as even values in the header).
pub const VERSION_WIDTH: u32 = 8;
/// Minimum bit-width helper for signed micro price offsets (S12).
pub const MICRO_OFF_TICKS_MIN: i16 = -2048;
/// Maximum representable micro price offset (ticks).
pub const MICRO_OFF_TICKS_MAX: i16 = 2047;
/// Minimum order-book imbalance value encoded as Q1.10.
pub const OBI_Q1_10_MIN: i16 = -1024;
/// Maximum order-book imbalance value encoded as Q1.10.
pub const OBI_Q1_10_MAX: i16 = 1023;
/// Scale factor for converting to/from Q1.10 floats.
pub const OBI_Q1_10_SCALE: i32 = 1 << 10;
/// Minimum 200 ms trend (signed 11-bit field).
pub const TREND_200MS_MIN: i16 = -1024;
/// Maximum 200 ms trend (signed 11-bit field).
pub const TREND_200MS_MAX: i16 = 1023;

/// Field descriptor for packing/unpacking within a 128-bit word.
#[derive(Clone, Copy)]
pub struct Field {
    /// Bit shift from LSB.
    pub shift: u32,
    /// Width in bits.
    pub width: u32,
}

impl Field {
    /// Create a new descriptor.
    #[must_use]
    pub const fn new(shift: u32, width: u32) -> Self {
        Self { shift, width }
    }

    const fn mask(self) -> u128 {
        if self.width == 0 {
            0
        } else if self.width >= 128 {
            !0
        } else {
            ((1u128 << self.width) - 1) << self.shift
        }
    }

    const fn value_mask(self) -> u128 {
        if self.width == 0 {
            0
        } else if self.width >= 128 {
            !0
        } else {
            (1u128 << self.width) - 1
        }
    }
}

/// Header fields (W0).
pub const W0_COMMIT: Field = Field::new(0, 1);
/// Bit-field descriptor for the stale flag.
pub const W0_STALE: Field = Field::new(1, 1);
/// Version field descriptor (even values published to readers).
pub const W0_VERSION: Field = Field::new(2, 8);
/// Sequence field (mirrored in the tail word).
pub const W0_SEQ_HEAD: Field = Field::new(10, 16);
/// Symbol identifier for instrument A.
pub const W0_SYM_A_ID: Field = Field::new(26, 16);
/// Symbol identifier for instrument B.
pub const W0_SYM_B_ID: Field = Field::new(42, 16);
/// Creation timestamp field using coarse ms/4 granularity.
pub const W0_CREATED_MS_COARSE: Field = Field::new(58, 24);
/// Minutes-after-open guard bit-field.
pub const W0_FORBID_AFTER_MIN_CT: Field = Field::new(82, 11);
/// Minutes-to-flatten guard bit-field.
pub const W0_EOD_FLAT_MIN_CT: Field = Field::new(93, 11);
/// Session flags bit-field.
pub const W0_FLAGS: Field = Field::new(104, 14);
/// Spare payload used for future extensions (10 effective bits).
pub const W0_SPARE: Field = Field::new(118, 10);

/// Instrument header (32 bits).
/// Bit shift for the reference price tick field.
pub const HDR_PX_REF_SHIFT: u32 = 12;
/// Bit width for the reference price tick field.
pub const HDR_PX_REF_WIDTH: u32 = 12;
/// Bit shift for the local version nibble.
pub const HDR_LOCAL_VER_SHIFT: u32 = 24;
/// Bit width for the local version nibble.
pub const HDR_LOCAL_VER_WIDTH: u32 = 4;
/// Bit shift for the local sequence nibble.
pub const HDR_LOCAL_SEQ_SHIFT: u32 = 28;
/// Bit width for the local sequence nibble.
pub const HDR_LOCAL_SEQ_WIDTH: u32 = 4;

/// Maximum tick-value (Q4) that fits into the header.
pub const HDR_TICK_VALUE_Q4_MAX: u16 = (1 << 12) - 1;
/// Minimum reference tick index (S12).
pub const HDR_PX_REF_MIN: i16 = -2048;
/// Maximum reference tick index (S12).
pub const HDR_PX_REF_MAX: i16 = 2047;

/// Encode an instrument header into a 32-bit word.
#[must_use]
pub fn pack_instrument_header(
    tick_value_cents_q4: u16,
    px_ref_ticks: i16,
    local_ver: u8,
    local_seq: u8,
) -> u32 {
    let tick = u32::from(tick_value_cents_q4.min(HDR_TICK_VALUE_Q4_MAX));
    let px_ref_clamped = i32::from(px_ref_ticks.clamp(HDR_PX_REF_MIN, HDR_PX_REF_MAX));
    let px_ref_mask = (1 << HDR_PX_REF_WIDTH) - 1;
    let px_ref = (px_ref_clamped & px_ref_mask) as u32;
    let ver = u32::from(local_ver & 0x0F);
    let seq = u32::from(local_seq & 0x0F);
    tick | (px_ref << HDR_PX_REF_SHIFT)
        | (ver << HDR_LOCAL_VER_SHIFT)
        | (seq << HDR_LOCAL_SEQ_SHIFT)
}

/// Decode the instrument header fields from a packed 32-bit value.
#[must_use]
pub fn unpack_instrument_header(word: u32) -> (u16, i16, u8, u8) {
    let tick = (word & ((1 << HDR_PX_REF_SHIFT) - 1)) as u16;
    let px_ref_raw = (word >> HDR_PX_REF_SHIFT) & ((1 << HDR_PX_REF_WIDTH) - 1);
    let px_ref =
        (((px_ref_raw << (32 - HDR_PX_REF_WIDTH)) as i32) >> (32 - HDR_PX_REF_WIDTH)) as i16;
    let local_ver = ((word >> HDR_LOCAL_VER_SHIFT) & ((1 << HDR_LOCAL_VER_WIDTH) - 1)) as u8;
    let local_seq = ((word >> HDR_LOCAL_SEQ_SHIFT) & ((1 << HDR_LOCAL_SEQ_WIDTH) - 1)) as u8;
    (tick, px_ref, local_ver, local_seq)
}

/// Pack a `(px_ticks_s16, qty_u16)` level into 32 bits.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn pack_level(px_ticks: i16, qty: u16) -> u32 {
    let px = px_ticks as u16;
    (u32::from(px) << 16) | u32::from(qty)
}

/// Unpack a 32-bit level back into signed ticks and raw quantity.
#[must_use]
pub fn unpack_level(word: u32) -> (i16, u16) {
    let px = (word >> 16) as u16;
    let qty = word as u16;
    (px as i16, qty)
}

/// Pack the cumulative L1-3 sums into a 32-bit tail word.
#[must_use]
pub fn pack_sums(sum_bid: u16, sum_ask: u16) -> u32 {
    u32::from(sum_bid) | (u32::from(sum_ask) << 16)
}

/// Unpack the cumulative sums from the tail word.
#[must_use]
pub fn unpack_sums(word: u32) -> (u16, u16) {
    let bid = word as u16;
    let ask = (word >> 16) as u16;
    (bid, ask)
}

/// Clamp helper for signed 12-bit fields.
#[must_use]
pub const fn clamp_s12(value: i32) -> i16 {
    if value < MICRO_OFF_TICKS_MIN as i32 {
        MICRO_OFF_TICKS_MIN
    } else if value > MICRO_OFF_TICKS_MAX as i32 {
        MICRO_OFF_TICKS_MAX
    } else {
        value as i16
    }
}

/// Clamp helper for signed 11-bit fields.
#[must_use]
pub const fn clamp_s11(value: i32) -> i16 {
    if value < TREND_200MS_MIN as i32 {
        TREND_200MS_MIN
    } else if value > TREND_200MS_MAX as i32 {
        TREND_200MS_MAX
    } else {
        value as i16
    }
}

/// Clamp helper for OBI values expressed in Q1.10 scale.
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

/// Convert an imbalance into Q1.10 given bid/ask sums.
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

/// Convert Q1.10 back to a float ratio.
#[must_use]
pub fn obi_to_ratio(raw: i16) -> f32 {
    f32::from(raw) / (OBI_Q1_10_SCALE as f32)
}

/// Quantise a millisecond timestamp using the `ms/4` coarse granularity.
#[must_use]
pub const fn quantise_timestamp_ms(timestamp_ms: u64) -> u32 {
    const MAX: u64 = (1u64 << 24) - 1;
    let coarse = timestamp_ms >> 2;
    if coarse > MAX {
        MAX as u32
    } else {
        coarse as u32
    }
}

/// Dequantise a coarse timestamp back to milliseconds (lower 2 bits lost).
#[must_use]
pub const fn dequantise_timestamp_ms(coarse: u32) -> u64 {
    (coarse as u64) << 2
}

/// Pack an unsigned field into a 128-bit word.
#[must_use]
pub fn pack_unsigned(word: u128, field: Field, value: u128) -> u128 {
    debug_assert!(value <= field.value_mask(), "value exceeds field width");
    let cleared = word & !field.mask();
    cleared | ((value & field.value_mask()) << field.shift)
}

/// Pack a signed value with explicit range checking into a 128-bit word.
#[must_use]
pub fn pack_signed(word: u128, field: Field, value: i32, width: u32) -> u128 {
    debug_assert!(width <= 32 && width > 0, "signed packing width invalid");
    let min = -(1i64 << (width - 1));
    let max = (1i64 << (width - 1)) - 1;
    debug_assert!(
        i64::from(value) >= min && i64::from(value) <= max,
        "signed value exceeds width"
    );
    let mask = (1i128 << width) - 1;
    let encoded = (i128::from(value) & mask) as u128;
    pack_unsigned(word, field, encoded)
}

/// Extract an unsigned field from a 128-bit word.
#[must_use]
pub const fn unpack_unsigned(word: u128, field: Field) -> u128 {
    (word >> field.shift) & field.value_mask()
}

/// Extract a signed field from a 128-bit word.
#[must_use]
pub fn unpack_signed(word: u128, field: Field, width: u32) -> i32 {
    debug_assert!(width <= 32 && width > 0, "signed unpack width invalid");
    let raw = (word >> field.shift) & ((1u128 << width) - 1);
    let raw32 = raw as u32;
    let shift = 32 - width;
    ((raw32 << shift) as i32) >> shift
}

/// Compute CRC16-IBM (polynomial 0xA001) across the supplied bytes.
#[must_use]
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for &byte in data {
        let mut x = crc ^ u16::from(byte);
        for _ in 0..8 {
            if (x & 1) != 0 {
                x = (x >> 1) ^ 0xA001;
            } else {
                x >>= 1;
            }
        }
        crc = x;
    }
    crc
}

/// Helper to view a `u128` as bytes in little-endian order.
#[must_use]
pub fn word_to_bytes(word: u128) -> [u8; 16] {
    word.to_le_bytes()
}

/// Saturating helper for quantities that must fit in `u16`.
#[must_use]
pub const fn clamp_qty(value: u32) -> u16 {
    if value > u16::MAX as u32 {
        u16::MAX
    } else {
        value as u16
    }
}
