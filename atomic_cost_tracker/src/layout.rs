use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

/// Fixed-point helper for Q8.8 signed values (basis points).
#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct FixedQ8_8(i16);

impl FixedQ8_8 {
    const SCALE: f64 = 256.0;
    pub const MIN_BP: f64 = i16::MIN as f64 / Self::SCALE;
    pub const MAX_BP: f64 = i16::MAX as f64 / Self::SCALE;

    /// Construct from a basis-point value, saturating to the representable range.
    pub fn saturating_from_bp(bp: f64) -> Self {
        let clamped = bp.clamp(Self::MIN_BP, Self::MAX_BP);
        let scaled = (clamped * Self::SCALE).round();
        let raw = scaled.clamp(i16::MIN as f64, i16::MAX as f64).trunc() as i16;
        Self(raw)
    }

    /// Construct from a raw Q8.8 bit pattern.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits as i16)
    }

    /// Obtain the raw Q8.8 bit pattern for storage.
    pub const fn to_bits(self) -> u16 {
        self.0 as u16
    }

    /// Access the underlying integer representation.
    pub const fn raw(self) -> i16 {
        self.0
    }

    /// Return the value in basis points as `f64`.
    pub const fn to_bp(self) -> f64 {
        self.0 as f64 / Self::SCALE
    }
}

impl From<f64> for FixedQ8_8 {
    fn from(value: f64) -> Self {
        Self::saturating_from_bp(value)
    }
}

impl From<FixedQ8_8> for f64 {
    fn from(value: FixedQ8_8) -> Self {
        value.to_bp()
    }
}

impl fmt::Debug for FixedQ8_8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}bp", self.to_bp())
    }
}

/// Bitflags carried on every ACT snapshot.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ActFlags(pub u8);

impl ActFlags {
    pub const OK: Self = Self(1 << 0);
    pub const MAKER: Self = Self(1 << 1);
    pub const TAKER: Self = Self(1 << 2);
    pub const HIGH_JITTER: Self = Self(1 << 3);
    pub const WIDE_SPREAD: Self = Self(1 << 4);
    pub const EMERG_BUF: Self = Self(1 << 5);

    /// No flags set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Return the raw bit representation.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Check whether a specific flag (or combination) is present.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl BitOr for ActFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ActFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ActFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for ActFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for ActFlags {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

/// Logical view of the ACT-128 snapshot.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ActSnapshot {
    pub gross: FixedQ8_8,
    pub fees: FixedQ8_8,
    pub slip: FixedQ8_8,
    pub net: FixedQ8_8,
    pub min_required: FixedQ8_8,
    pub sigma: FixedQ8_8,
    pub flags: ActFlags,
    pub version: u8,
    /// Sequence id (u8). Expand the layout if a wider counter is required.
    pub seq: u8,
    pub age_ms_bucket: u8,
}

impl ActSnapshot {
    /// Snapshot with zeroed financial fields and `OK` cleared.
    pub const fn empty() -> Self {
        Self {
            gross: FixedQ8_8::from_bits(0),
            fees: FixedQ8_8::from_bits(0),
            slip: FixedQ8_8::from_bits(0),
            net: FixedQ8_8::from_bits(0),
            min_required: FixedQ8_8::from_bits(0),
            sigma: FixedQ8_8::from_bits(0),
            flags: ActFlags::empty(),
            version: 0,
            seq: 0,
            age_ms_bucket: 0,
        }
    }
}

impl Default for ActSnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

/// Packed 128-bit representation of the snapshot.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ActWord(pub u128);

impl ActWord {
    const FIELD_MASK: u128 = 0xFFFF;
    const BYTE_MASK: u128 = 0xFF;

    const GROSS_SHIFT: u32 = 0;
    const FEES_SHIFT: u32 = 16;
    const SLIP_SHIFT: u32 = 32;
    const NET_SHIFT: u32 = 48;
    const MIN_REQ_SHIFT: u32 = 64;
    const SIGMA_SHIFT: u32 = 80;
    const FLAGS_SHIFT: u32 = 96;
    const VERSION_SHIFT: u32 = 104;
    // Sequence is stored as u8 to keep the total width at 128 bits.
    const SEQ_SHIFT: u32 = 112;
    const AGE_SHIFT: u32 = 120;

    /// Pack a snapshot into a single 128-bit word.
    pub fn pack(snapshot: &ActSnapshot) -> Self {
        let mut word = 0u128;
        word |= (snapshot.gross.to_bits() as u128) << Self::GROSS_SHIFT;
        word |= (snapshot.fees.to_bits() as u128) << Self::FEES_SHIFT;
        word |= (snapshot.slip.to_bits() as u128) << Self::SLIP_SHIFT;
        word |= (snapshot.net.to_bits() as u128) << Self::NET_SHIFT;
        word |= (snapshot.min_required.to_bits() as u128) << Self::MIN_REQ_SHIFT;
        word |= (snapshot.sigma.to_bits() as u128) << Self::SIGMA_SHIFT;
        word |= (snapshot.flags.bits() as u128) << Self::FLAGS_SHIFT;
        word |= (snapshot.version as u128) << Self::VERSION_SHIFT;
        word |= (snapshot.seq as u128) << Self::SEQ_SHIFT;
        word |= (snapshot.age_ms_bucket as u128) << Self::AGE_SHIFT;
        Self(word)
    }

    /// Unpack the raw word into structured fields.
    pub fn unpack(self) -> ActSnapshot {
        ActSnapshot {
            gross: Self::extract_q8(self.0, Self::GROSS_SHIFT),
            fees: Self::extract_q8(self.0, Self::FEES_SHIFT),
            slip: Self::extract_q8(self.0, Self::SLIP_SHIFT),
            net: Self::extract_q8(self.0, Self::NET_SHIFT),
            min_required: Self::extract_q8(self.0, Self::MIN_REQ_SHIFT),
            sigma: Self::extract_q8(self.0, Self::SIGMA_SHIFT),
            flags: ActFlags(Self::extract_byte(self.0, Self::FLAGS_SHIFT) as u8),
            version: Self::extract_byte(self.0, Self::VERSION_SHIFT) as u8,
            seq: Self::extract_byte(self.0, Self::SEQ_SHIFT) as u8,
            age_ms_bucket: Self::extract_byte(self.0, Self::AGE_SHIFT) as u8,
        }
    }

    /// Access the raw packed representation.
    pub const fn raw(self) -> u128 {
        self.0
    }

    /// Construct from a raw packed representation.
    pub const fn from_raw(raw: u128) -> Self {
        Self(raw)
    }

    const fn extract_q8(raw: u128, shift: u32) -> FixedQ8_8 {
        let bits = ((raw >> shift) & Self::FIELD_MASK) as u16;
        FixedQ8_8::from_bits(bits)
    }

    const fn extract_byte(raw: u128, shift: u32) -> u16 {
        ((raw >> shift) & Self::BYTE_MASK) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_q8_round_trip() {
        let values = [-64.125, -1.5, 0.0, 0.125, 6.75, 42.5, 120.0];
        for bp in values {
            let q = FixedQ8_8::saturating_from_bp(bp);
            let actual = q.to_bp();
            assert!((actual - bp.clamp(FixedQ8_8::MIN_BP, FixedQ8_8::MAX_BP)).abs() < 1e-3);
        }
    }

    #[test]
    fn act_word_pack_unpack() {
        let snapshot = ActSnapshot {
            gross: FixedQ8_8::saturating_from_bp(5.5),
            fees: FixedQ8_8::saturating_from_bp(1.25),
            slip: FixedQ8_8::saturating_from_bp(0.75),
            net: FixedQ8_8::saturating_from_bp(3.5),
            min_required: FixedQ8_8::saturating_from_bp(2.0),
            sigma: FixedQ8_8::saturating_from_bp(0.5),
            flags: ActFlags::OK | ActFlags::MAKER,
            version: 7,
            seq: 222,
            age_ms_bucket: 18,
        };

        let word = ActWord::pack(&snapshot);
        let unpacked = word.unpack();
        assert_eq!(snapshot, unpacked);
    }

    #[test]
    fn saturates_when_out_of_range() {
        let snapshot = ActSnapshot {
            gross: FixedQ8_8::saturating_from_bp(500.0),
            fees: FixedQ8_8::saturating_from_bp(-500.0),
            slip: FixedQ8_8::saturating_from_bp(500.0),
            net: FixedQ8_8::saturating_from_bp(500.0),
            min_required: FixedQ8_8::saturating_from_bp(500.0),
            sigma: FixedQ8_8::saturating_from_bp(500.0),
            flags: ActFlags::OK,
            version: 0,
            seq: 0,
            age_ms_bucket: 0,
        };

        let word = ActWord::pack(&snapshot);
        let unpacked = word.unpack();
        assert_eq!(unpacked.gross.to_bp(), FixedQ8_8::MAX_BP);
        assert_eq!(unpacked.fees.to_bp(), FixedQ8_8::MIN_BP);
    }
}
