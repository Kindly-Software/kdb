use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use atomic_breaker::{AtomicBreakerSWeMR, breaker::State as BreakerState};

/// Maximum number of per-symbol slices carried in a snapshot.
pub const MAX_SYMBOL_SLICES: usize = 6;

/// Portfolio-level breaker severity.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BreakerLevel {
    L0 = 0,
    L1 = 1,
    L2 = 2,
    L3 = 3,
}

impl BreakerLevel {
    /// Clamp an integer into the supported breaker range.
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::L0,
            1 => Self::L1,
            2 => Self::L2,
            _ => Self::L3,
        }
    }

    /// Return the compact representation encoded in the snapshot.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Convert from atomic_breaker level (2-bit field, 0-3).
    ///
    /// #ASSUME_STATE_VALID: atomic_breaker levels map 1:1 to BreakerLevel
    /// #VERIFY_STATE_MACHINE: Test validates level conversion correctness
    pub const fn from_atomic_breaker_level(level: u8) -> Self {
        Self::from_u8(level & 0x3)
    }

    /// Convert to atomic_breaker level format.
    pub const fn to_atomic_breaker_level(self) -> u8 {
        self.as_u8()
    }

    /// Create an atomic breaker instance with this level and default closed state.
    ///
    /// #ASSUME_LOCKFREE_ONLY: AtomicBreakerSWeMR is lockfree by design
    /// #VERIFY_NO_BLOCKING: atomic_breaker crate guarantees lockfree operation
    pub fn create_atomic_breaker(self) -> AtomicBreakerSWeMR {
        let breaker = AtomicBreakerSWeMR::new(BreakerState::Closed);
        breaker.set_level(self.to_atomic_breaker_level());
        breaker
    }
}

impl Default for BreakerLevel {
    fn default() -> Self {
        Self::L0
    }
}

/// Bitflags that describe portfolio level state.
#[derive(Copy, Clone, Default, Eq, PartialEq)]
pub struct PortfolioFlags(pub u16);

impl PortfolioFlags {
    pub const PAUSED: Self = Self(1 << 0);
    pub const NEWS_LOCKOUT: Self = Self(1 << 1);
    pub const AFTER_FORBID: Self = Self(1 << 2);
    pub const AT_EOD: Self = Self(1 << 3);
    pub const TRAIL_WARN: Self = Self(1 << 4);
    /// Flag reserved for future use.
    pub const RESERVED5: Self = Self(1 << 5);

    /// No flags set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Raw bit representation.
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Check whether the provided flags are present.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Insert additional flags.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Update flags based on atomic breaker state.
    ///
    /// Automatically sets PAUSED flag when breaker is Open or ForcedOpen.
    /// This provides unified risk management between portfolio flags and breaker state.
    ///
    /// #ASSUME_STATE_VALID: BreakerState enum values are stable
    /// #VERIFY_STATE_MACHINE: Test validates flag updates match breaker state
    pub fn with_breaker_state(mut self, breaker_state: BreakerState) -> Self {
        match breaker_state {
            BreakerState::Open | BreakerState::ForcedOpen => {
                self.insert(Self::PAUSED);
            }
            BreakerState::Closed | BreakerState::HalfOpen => {
                // Allow normal operation, don't force PAUSED
                // (PAUSED may still be set by other conditions)
            }
        }
        self
    }

    /// Check if the breaker state should force operations to pause.
    pub fn is_breaker_paused(breaker_state: BreakerState) -> bool {
        matches!(breaker_state, BreakerState::Open | BreakerState::ForcedOpen)
    }
}

impl fmt::Debug for PortfolioFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PortfolioFlags({:#x})", self.0)
    }
}

impl BitOr for PortfolioFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for PortfolioFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for PortfolioFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for PortfolioFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for PortfolioFlags {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

/// Bitflags stored in each per-symbol slice.
#[derive(Copy, Clone, Default, Eq, PartialEq)]
pub struct SymbolFlags(pub u8);

impl SymbolFlags {
    pub const CAN_SCALE_UP: Self = Self(1 << 0);
    pub const REDUCE_ONLY: Self = Self(1 << 1);
    pub const LOCKOUT: Self = Self(1 << 2);
    pub const NEWS: Self = Self(1 << 3);
    pub const AFTER_FORBID: Self = Self(1 << 4);
    pub const HAS_RISK: Self = Self(1 << 5);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

impl fmt::Debug for SymbolFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SymbolFlags({:#x})", self.0)
    }
}

impl BitOr for SymbolFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for SymbolFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for SymbolFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for SymbolFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for SymbolFlags {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

/// Header word of the portfolio map snapshot.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ApmHeader {
    pub commit: bool,
    pub stale: bool,
    pub version: u8,
    pub seq: u16,
    pub account_id: u16,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub rem_daily_loss_total_cents: u32,
    pub portfolio_breaker: BreakerLevel,
    pub symbol_count: u8,
    pub portfolio_flags: PortfolioFlags,
    pub created_ms_coarse: u16,
}

impl ApmHeader {
    const FORBID_MASK: u16 = (1 << 11) - 1;
    const SYMBOL_COUNT_MASK: u8 = (1 << 4) - 1;
    const FLAGS_MASK: u16 = (1 << 10) - 1;

    pub const fn empty() -> Self {
        Self {
            commit: false,
            stale: false,
            version: 0,
            seq: 0,
            account_id: 0,
            forbid_after_min_ct: 0,
            eod_flat_min_ct: 0,
            rem_daily_loss_total_cents: 0,
            portfolio_breaker: BreakerLevel::L0,
            symbol_count: 0,
            portfolio_flags: PortfolioFlags::empty(),
            created_ms_coarse: 0,
        }
    }

    fn clamp_values(&mut self) {
        self.forbid_after_min_ct = self.forbid_after_min_ct.min(Self::FORBID_MASK);
        self.eod_flat_min_ct = self.eod_flat_min_ct.min(Self::FORBID_MASK);
        self.symbol_count = self.symbol_count.min(Self::SYMBOL_COUNT_MASK);
        self.portfolio_flags.0 &= Self::FLAGS_MASK;
    }

    /// Encode the header into its packed word.
    pub fn encode(&self) -> u128 {
        pack_header(self)
    }

    /// Decode a packed header word.
    pub fn decode(word: u128) -> Self {
        unpack_header(word)
    }
}

impl Default for ApmHeader {
    fn default() -> Self {
        Self::empty()
    }
}

/// Per-symbol slice describing account state for a tradable symbol.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ApmSymbolSlice {
    pub sym_id: u16,
    pub breaker_level: BreakerLevel,
    pub flags: SymbolFlags,
    pub pos_qty: i32,
    pub unreal_cents: i32,
    pub rem_daily_loss_cents: u32,
    pub spread_ticks: u8,
    pub vol_band: u8,
    pub priority: u8,
}

impl ApmSymbolSlice {
    const POS_MIN: i32 = -(1 << 23);
    const POS_MAX: i32 = (1 << 23) - 1;
    const REM_MASK: u32 = (1 << 24) - 1;

    pub const fn empty() -> Self {
        Self {
            sym_id: 0,
            breaker_level: BreakerLevel::L0,
            flags: SymbolFlags::empty(),
            pos_qty: 0,
            unreal_cents: 0,
            rem_daily_loss_cents: 0,
            spread_ticks: 0,
            vol_band: 0,
            priority: 0,
        }
    }

    fn clamp_values(&mut self) {
        self.pos_qty = self.pos_qty.clamp(Self::POS_MIN, Self::POS_MAX);
        self.rem_daily_loss_cents = self.rem_daily_loss_cents.min(Self::REM_MASK);
    }

    /// Encode the slice into its packed word.
    pub fn encode(&self) -> u128 {
        pack_slice(self)
    }

    /// Decode a packed slice word.
    pub fn decode(word: u128) -> Self {
        unpack_slice(word)
    }
}

impl Default for ApmSymbolSlice {
    fn default() -> Self {
        Self::empty()
    }
}

/// Tail word carrying aggregated totals and integrity markers.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ApmTail {
    pub sum_pos_abs_contracts: u16,
    pub net_unreal_cents: i32,
    pub net_realized_cents: i32,
    pub trailing_draw_cents: u16,
    pub version: u8,
    pub seq: u16,
    pub spare: u8,
}

impl ApmTail {
    pub const fn empty() -> Self {
        Self {
            sum_pos_abs_contracts: 0,
            net_unreal_cents: 0,
            net_realized_cents: 0,
            trailing_draw_cents: 0,
            version: 0,
            seq: 0,
            spare: 0,
        }
    }

    /// Encode the tail into its packed word.
    pub fn encode(&self) -> u128 {
        pack_tail(self)
    }

    /// Decode a packed tail word.
    pub fn decode(word: u128) -> Self {
        unpack_tail(word)
    }
}

impl Default for ApmTail {
    fn default() -> Self {
        Self::empty()
    }
}

/// Complete logical view of an APM snapshot.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ApmSnapshot {
    pub header: ApmHeader,
    pub slices: [ApmSymbolSlice; MAX_SYMBOL_SLICES],
    pub tail: ApmTail,
}

impl ApmSnapshot {
    pub const fn empty() -> Self {
        Self {
            header: ApmHeader::empty(),
            slices: [ApmSymbolSlice::empty(); MAX_SYMBOL_SLICES],
            tail: ApmTail::empty(),
        }
    }

    pub fn pack(mut self) -> ApmWords {
        self.header.clamp_values();
        for slice in &mut self.slices {
            slice.clamp_values();
        }
        let mut words = [0u128; 8];
        words[0] = self.header.encode();
        for (idx, slice) in self.slices.iter().enumerate() {
            words[idx + 1] = slice.encode();
        }
        words[7] = self.tail.encode();
        ApmWords { words }
    }

    pub fn unpack(words: &ApmWords) -> Self {
        let header = ApmHeader::decode(words.words[0]);
        let mut slices = [ApmSymbolSlice::empty(); MAX_SYMBOL_SLICES];
        for (idx, slot) in slices.iter_mut().enumerate() {
            *slot = ApmSymbolSlice::decode(words.words[idx + 1]);
        }
        let tail = ApmTail::decode(words.words[7]);
        Self {
            header,
            slices,
            tail,
        }
    }
}

impl Default for ApmSnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

/// Packed representation aligned for atomic publication.
#[repr(C, align(64))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ApmWords {
    words: [u128; 8],
}

impl ApmWords {
    pub const fn zeroed() -> Self {
        Self { words: [0u128; 8] }
    }

    pub const fn as_words(&self) -> &[u128; 8] {
        &self.words
    }

    pub fn as_mut_words(&mut self) -> &mut [u128; 8] {
        &mut self.words
    }

    pub fn from_words(words: [u128; 8]) -> Self {
        Self { words }
    }

    pub fn into_inner(self) -> [u128; 8] {
        self.words
    }
}

fn mask(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    }
}

fn pack_bool(value: bool, shift: u32) -> u128 {
    (value as u128) << shift
}

fn pack_unsigned(value: u128, width: u32, shift: u32) -> u128 {
    let max = mask(width);
    let clamped = if value > max { max } else { value };
    clamped << shift
}

fn pack_signed(value: i64, width: u32, shift: u32) -> u128 {
    let min = -(1i64 << (width - 1));
    let max = (1i64 << (width - 1)) - 1;
    let mut v = value;
    if v < min {
        v = min;
    } else if v > max {
        v = max;
    }
    let modulus = 1i128 << width;
    let raw = if v < 0 {
        (modulus + v as i128) as u128
    } else {
        v as u128
    };
    let masked = raw & mask(width);
    masked << shift
}

fn unpack_bool(word: u128, shift: u32) -> bool {
    ((word >> shift) & 1) != 0
}

fn unpack_unsigned(word: u128, width: u32, shift: u32) -> u64 {
    let masked = (word >> shift) & mask(width);
    masked as u64
}

fn unpack_signed(word: u128, width: u32, shift: u32) -> i64 {
    let raw = unpack_unsigned(word, width, shift) as u64;
    let sign_bit = 1u64 << (width - 1);
    if raw & sign_bit != 0 {
        let extended = raw | (!0u64 << width);
        extended as i64
    } else {
        raw as i64
    }
}

fn pack_header(header: &ApmHeader) -> u128 {
    let mut word = 0u128;
    word |= pack_bool(header.commit, 0);
    word |= pack_bool(header.stale, 1);
    word |= pack_unsigned(header.version as u128, 8, 2);
    word |= pack_unsigned(header.seq as u128, 16, 10);
    word |= pack_unsigned(header.account_id as u128, 16, 26);
    word |= pack_unsigned(header.forbid_after_min_ct as u128, 11, 42);
    word |= pack_unsigned(header.eod_flat_min_ct as u128, 11, 53);
    word |= pack_unsigned(header.rem_daily_loss_total_cents as u128, 32, 64);
    word |= pack_unsigned(header.portfolio_breaker.as_u8() as u128, 2, 96);
    word |= pack_unsigned(header.symbol_count as u128, 4, 98);
    word |= pack_unsigned(header.portfolio_flags.bits() as u128, 10, 102);
    word |= pack_unsigned(header.created_ms_coarse as u128, 16, 112);
    word
}

fn unpack_header(word: u128) -> ApmHeader {
    ApmHeader {
        commit: unpack_bool(word, 0),
        stale: unpack_bool(word, 1),
        version: unpack_unsigned(word, 8, 2) as u8,
        seq: unpack_unsigned(word, 16, 10) as u16,
        account_id: unpack_unsigned(word, 16, 26) as u16,
        forbid_after_min_ct: unpack_unsigned(word, 11, 42) as u16,
        eod_flat_min_ct: unpack_unsigned(word, 11, 53) as u16,
        rem_daily_loss_total_cents: unpack_unsigned(word, 32, 64) as u32,
        portfolio_breaker: BreakerLevel::from_u8(unpack_unsigned(word, 2, 96) as u8),
        symbol_count: unpack_unsigned(word, 4, 98) as u8,
        portfolio_flags: PortfolioFlags(unpack_unsigned(word, 10, 102) as u16),
        created_ms_coarse: unpack_unsigned(word, 16, 112) as u16,
    }
}

fn pack_slice(slice: &ApmSymbolSlice) -> u128 {
    let mut word = 0u128;
    word |= pack_unsigned(slice.sym_id as u128, 16, 0);
    word |= pack_unsigned(slice.breaker_level.as_u8() as u128, 2, 16);
    word |= pack_unsigned(slice.flags.bits() as u128, 6, 18);
    word |= pack_signed(slice.pos_qty as i64, 24, 24);
    word |= pack_signed(slice.unreal_cents as i64, 32, 48);
    word |= pack_unsigned(slice.rem_daily_loss_cents as u128, 24, 80);
    word |= pack_unsigned(slice.spread_ticks as u128, 8, 104);
    word |= pack_unsigned(slice.vol_band as u128, 8, 112);
    word |= pack_unsigned(slice.priority as u128, 8, 120);
    word
}

fn unpack_slice(word: u128) -> ApmSymbolSlice {
    ApmSymbolSlice {
        sym_id: unpack_unsigned(word, 16, 0) as u16,
        breaker_level: BreakerLevel::from_u8(unpack_unsigned(word, 2, 16) as u8),
        flags: SymbolFlags(unpack_unsigned(word, 6, 18) as u8),
        pos_qty: unpack_signed(word, 24, 24) as i32,
        unreal_cents: unpack_signed(word, 32, 48) as i32,
        rem_daily_loss_cents: unpack_unsigned(word, 24, 80) as u32,
        spread_ticks: unpack_unsigned(word, 8, 104) as u8,
        vol_band: unpack_unsigned(word, 8, 112) as u8,
        priority: unpack_unsigned(word, 8, 120) as u8,
    }
}

fn pack_tail(tail: &ApmTail) -> u128 {
    let mut word = 0u128;
    word |= pack_unsigned(tail.sum_pos_abs_contracts as u128, 16, 0);
    word |= pack_signed(tail.net_unreal_cents as i64, 32, 16);
    word |= pack_signed(tail.net_realized_cents as i64, 32, 48);
    word |= pack_unsigned(tail.trailing_draw_cents as u128, 16, 80);
    word |= pack_unsigned(tail.version as u128, 8, 96);
    word |= pack_unsigned(tail.seq as u128, 16, 104);
    word |= pack_unsigned(tail.spare as u128, 8, 120);
    word
}

fn unpack_tail(word: u128) -> ApmTail {
    ApmTail {
        sum_pos_abs_contracts: unpack_unsigned(word, 16, 0) as u16,
        net_unreal_cents: unpack_signed(word, 32, 16) as i32,
        net_realized_cents: unpack_signed(word, 32, 48) as i32,
        trailing_draw_cents: unpack_unsigned(word, 16, 80) as u16,
        version: unpack_unsigned(word, 8, 96) as u8,
        seq: unpack_unsigned(word, 16, 104) as u16,
        spare: unpack_unsigned(word, 8, 120) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_round_trip() {
        let mut snapshot = ApmSnapshot::empty();
        snapshot.header = ApmHeader {
            commit: true,
            stale: false,
            version: 4,
            seq: 1_234,
            account_id: 77,
            forbid_after_min_ct: 900,
            eod_flat_min_ct: 915,
            rem_daily_loss_total_cents: 1_000_000,
            portfolio_breaker: BreakerLevel::L2,
            symbol_count: 3,
            portfolio_flags: PortfolioFlags::PAUSED | PortfolioFlags::TRAIL_WARN,
            created_ms_coarse: 42_000,
        };

        snapshot.slices[0] = ApmSymbolSlice {
            sym_id: 1001,
            breaker_level: BreakerLevel::L1,
            flags: SymbolFlags::CAN_SCALE_UP | SymbolFlags::HAS_RISK,
            pos_qty: 125,
            unreal_cents: -25_000,
            rem_daily_loss_cents: 250_000,
            spread_ticks: 4,
            vol_band: 2,
            priority: 192,
        };
        snapshot.slices[1] = ApmSymbolSlice {
            sym_id: 1002,
            breaker_level: BreakerLevel::L0,
            flags: SymbolFlags::REDUCE_ONLY,
            pos_qty: -640,
            unreal_cents: 75_000,
            rem_daily_loss_cents: 0,
            spread_ticks: 6,
            vol_band: 3,
            priority: 64,
        };

        snapshot.tail = ApmTail {
            sum_pos_abs_contracts: 765,
            net_unreal_cents: 50_000,
            net_realized_cents: 120_000,
            trailing_draw_cents: 12_500,
            version: 4,
            seq: 1_234,
            spare: 0,
        };

        let words = snapshot.pack();
        let unpacked = ApmSnapshot::unpack(&words);
        assert_eq!(snapshot.header, unpacked.header);
        assert_eq!(snapshot.slices[0], unpacked.slices[0]);
        assert_eq!(snapshot.slices[1], unpacked.slices[1]);
        assert_eq!(snapshot.tail, unpacked.tail);
    }

    #[test]
    fn clamps_out_of_range_values() {
        let mut snapshot = ApmSnapshot::empty();
        snapshot.header.symbol_count = 250; // exceeds mask
        snapshot.slices[0].pos_qty = ApmSymbolSlice::POS_MIN - 10;
        snapshot.slices[0].rem_daily_loss_cents = (1 << 24) + 100;
        let packed = snapshot.pack();
        let unpacked = ApmSnapshot::unpack(&packed);
        assert_eq!(unpacked.header.symbol_count, 0b1111);
        assert_eq!(unpacked.slices[0].pos_qty, ApmSymbolSlice::POS_MIN);
        assert_eq!(unpacked.slices[0].rem_daily_loss_cents, (1 << 24) - 1);
    }

    #[test]
    fn breaker_level_atomic_conversion() {
        // Test conversion from atomic breaker levels
        assert_eq!(BreakerLevel::from_atomic_breaker_level(0), BreakerLevel::L0);
        assert_eq!(BreakerLevel::from_atomic_breaker_level(1), BreakerLevel::L1);
        assert_eq!(BreakerLevel::from_atomic_breaker_level(2), BreakerLevel::L2);
        assert_eq!(BreakerLevel::from_atomic_breaker_level(3), BreakerLevel::L3);

        // Test clamping of out-of-range values
        assert_eq!(BreakerLevel::from_atomic_breaker_level(4), BreakerLevel::L0);
        assert_eq!(BreakerLevel::from_atomic_breaker_level(7), BreakerLevel::L3);

        // Test conversion to atomic breaker levels
        assert_eq!(BreakerLevel::L0.to_atomic_breaker_level(), 0);
        assert_eq!(BreakerLevel::L1.to_atomic_breaker_level(), 1);
        assert_eq!(BreakerLevel::L2.to_atomic_breaker_level(), 2);
        assert_eq!(BreakerLevel::L3.to_atomic_breaker_level(), 3);
    }

    #[test]
    fn breaker_creation_and_level_setting() {
        // Test creating atomic breaker from level
        let breaker = BreakerLevel::L2.create_atomic_breaker();
        assert_eq!(breaker.state(), BreakerState::Closed);
        assert_eq!(breaker.level(), 2);

        // Test all levels
        for level in [BreakerLevel::L0, BreakerLevel::L1, BreakerLevel::L2, BreakerLevel::L3] {
            let breaker = level.create_atomic_breaker();
            assert_eq!(breaker.level(), level.to_atomic_breaker_level());
            assert_eq!(breaker.state(), BreakerState::Closed);
        }
    }

    #[test]
    fn portfolio_flags_breaker_state_integration() {
        use BreakerState::*;

        // Test empty flags with different breaker states
        let empty_flags = PortfolioFlags::empty();

        assert!(!empty_flags.with_breaker_state(Closed).contains(PortfolioFlags::PAUSED));
        assert!(!empty_flags.with_breaker_state(HalfOpen).contains(PortfolioFlags::PAUSED));
        assert!(empty_flags.with_breaker_state(Open).contains(PortfolioFlags::PAUSED));
        assert!(empty_flags.with_breaker_state(ForcedOpen).contains(PortfolioFlags::PAUSED));

        // Test flags that already have PAUSED set
        let paused_flags = PortfolioFlags::PAUSED;
        assert!(paused_flags.with_breaker_state(Closed).contains(PortfolioFlags::PAUSED));
        assert!(paused_flags.with_breaker_state(Open).contains(PortfolioFlags::PAUSED));

        // Test breaker pause detection
        assert!(!PortfolioFlags::is_breaker_paused(Closed));
        assert!(!PortfolioFlags::is_breaker_paused(HalfOpen));
        assert!(PortfolioFlags::is_breaker_paused(Open));
        assert!(PortfolioFlags::is_breaker_paused(ForcedOpen));
    }

    #[test]
    fn portfolio_flags_preserve_existing_flags() {
        use BreakerState::*;

        // Test that existing flags are preserved when adding breaker state
        let flags = PortfolioFlags::TRAIL_WARN | PortfolioFlags::NEWS_LOCKOUT;

        let closed_flags = flags.with_breaker_state(Closed);
        assert!(closed_flags.contains(PortfolioFlags::TRAIL_WARN));
        assert!(closed_flags.contains(PortfolioFlags::NEWS_LOCKOUT));
        assert!(!closed_flags.contains(PortfolioFlags::PAUSED));

        let open_flags = flags.with_breaker_state(Open);
        assert!(open_flags.contains(PortfolioFlags::TRAIL_WARN));
        assert!(open_flags.contains(PortfolioFlags::NEWS_LOCKOUT));
        assert!(open_flags.contains(PortfolioFlags::PAUSED));
    }
}
