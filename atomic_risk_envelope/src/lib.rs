#![no_std]
//! Atomic Risk Envelope (ARE) packs Topstep-style risk guardrails inside a single
//! `u128` word. The public API is tracked in `docs/api_surface.md` and changes are logged
//! in `CHANGELOG.md` as we march toward a 1.0 release.

use core::fmt;
use core::sync::atomic::Ordering;
use portable_atomic::AtomicU128;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Bitflag markers that can be packed into the ARE `flags` field.
pub mod flag {
    use core::fmt;
    use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

    #[cfg(feature = "serde")]
    use serde::{Deserialize, Serialize};

    const MASK_BITS: u8 = ((1u16 << super::FLAGS_BITS) - 1) as u8;

    /// Wrapper that preserves only the supported flag bits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[cfg_attr(feature = "serde", serde(transparent))]
    pub struct Flags(u8);

    impl Flags {
        /// Empty flag set.
        pub const EMPTY: Self = Self(0);

        /// Construct from raw bits, discarding unsupported flags.
        pub const fn from_bits_truncate(bits: u8) -> Self {
            Self(bits & MASK_BITS)
        }

        /// Retrieve the raw bits represented by this flag set.
        pub const fn bits(self) -> u8 {
            self.0
        }

        /// Check whether every bit in `other` is present.
        pub const fn contains(self, other: Flags) -> bool {
            (self.bits() & other.bits()) == other.bits()
        }

        /// Check whether any overlap exists with `other`.
        pub const fn intersects(self, other: Flags) -> bool {
            (self.bits() & other.bits()) != 0
        }

        /// True when no bits are enabled.
        pub const fn is_empty(self) -> bool {
            self.bits() == 0
        }
    }

    impl BitOr for Flags {
        type Output = Flags;

        fn bitor(self, rhs: Flags) -> Flags {
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

        fn bitand(self, rhs: Flags) -> Flags {
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

        fn not(self) -> Flags {
            Flags::from_bits_truncate(!self.bits())
        }
    }

    /// Trading is paused; deny all orders.
    pub const PAUSED: Flags = Flags(1 << 0);
    /// Force flatten; deny new risk and signal liquidation logic.
    pub const EMERGENCY_FLAT: Flags = Flags(1 << 1);
    /// Scheduled or unscheduled news lockout window is active.
    pub const NEWS_LOCKOUT: Flags = Flags(1 << 2);
    /// Session is in liquidation-only mode. Optional downstream semantic.
    pub const LIQUIDATION_ONLY: Flags = Flags(1 << 3);
    /// Reserved for venue-specific halt.
    pub const VENUE_HALT: Flags = Flags(1 << 4);
    /// Reserved for custom guardrails.
    pub const CUSTOM: Flags = Flags(1 << 5);

    /// All supported flag bits for quick masking.
    pub const MASK: Flags = Flags(MASK_BITS);

    /// Pause all trading and trigger flattening.
    pub const HARD_FLAT: Flags = Flags::from_bits_truncate(PAUSED.bits() | EMERGENCY_FLAT.bits());
    /// Combine pause, flatten, and venue halt guards.
    pub const HALT: Flags =
        Flags::from_bits_truncate(PAUSED.bits() | EMERGENCY_FLAT.bits() | VENUE_HALT.bits());
    /// Pause and enforce a news lock window.
    pub const NEWS_LOCK: Flags = Flags::from_bits_truncate(PAUSED.bits() | NEWS_LOCKOUT.bits());
    /// Enable every supported guard bit.
    pub const ALL: Flags = MASK;

    impl From<Flags> for u8 {
        fn from(value: Flags) -> Self {
            value.bits()
        }
    }

    impl From<u8> for Flags {
        fn from(value: u8) -> Self {
            Flags::from_bits_truncate(value)
        }
    }

    const FLAG_LABELS: &[(Flags, &str)] = &[
        (PAUSED, "PAUSED"),
        (EMERGENCY_FLAT, "EMERGENCY_FLAT"),
        (NEWS_LOCKOUT, "NEWS_LOCKOUT"),
        (LIQUIDATION_ONLY, "LIQUIDATION_ONLY"),
        (VENUE_HALT, "VENUE_HALT"),
        (CUSTOM, "CUSTOM"),
    ];

    impl fmt::Display for Flags {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if self.is_empty() {
                return f.write_str("NONE");
            }

            let mut first = true;
            for (flag, label) in FLAG_LABELS.iter() {
                if self.contains(*flag) {
                    if !first {
                        f.write_str("|")?;
                    }
                    f.write_str(label)?;
                    first = false;
                }
            }

            let residual = self.bits() & !MASK.bits();
            if residual != 0 {
                if !first {
                    f.write_str("|")?;
                }
                write!(f, "0x{:02x}", residual)?;
            }

            Ok(())
        }
    }
}

const SEQ_BITS: u32 = 6;
const VER_BITS: u32 = 6;
const FLAGS_BITS: u32 = 6;
const EOD_FLAT_BITS: u32 = 11;
const FORBID_AFTER_BITS: u32 = 11;
const MAX_OPEN_MS_BITS: u32 = 20;
const MAX_CONTRACTS_BITS: u32 = 12;
const MAX_PER_TRADE_BITS: u32 = 24;
const REM_DAILY_BITS: u32 = 32;

const SEQ_SHIFT: u32 = 0;
const VER_SHIFT: u32 = SEQ_SHIFT + SEQ_BITS;
const FLAGS_SHIFT: u32 = VER_SHIFT + VER_BITS;
const EOD_FLAT_SHIFT: u32 = FLAGS_SHIFT + FLAGS_BITS;
const FORBID_AFTER_SHIFT: u32 = EOD_FLAT_SHIFT + EOD_FLAT_BITS;
const MAX_OPEN_MS_SHIFT: u32 = FORBID_AFTER_SHIFT + FORBID_AFTER_BITS;
const MAX_CONTRACTS_SHIFT: u32 = MAX_OPEN_MS_SHIFT + MAX_OPEN_MS_BITS;
const MAX_PER_TRADE_SHIFT: u32 = MAX_CONTRACTS_SHIFT + MAX_CONTRACTS_BITS;
const REM_DAILY_SHIFT: u32 = MAX_PER_TRADE_SHIFT + MAX_PER_TRADE_BITS;

const _: [(); 128] = [(); (REM_DAILY_SHIFT + REM_DAILY_BITS) as usize];

const fn mask(bits: u32) -> u128 {
    if bits == 0 {
        0
    } else {
        (!0u128) >> (128 - bits)
    }
}

const SEQ_MASK: u128 = mask(SEQ_BITS) << SEQ_SHIFT;
const VER_MASK: u128 = mask(VER_BITS) << VER_SHIFT;
const FLAGS_MASK: u128 = mask(FLAGS_BITS) << FLAGS_SHIFT;
const EOD_FLAT_MASK: u128 = mask(EOD_FLAT_BITS) << EOD_FLAT_SHIFT;
const FORBID_AFTER_MASK: u128 = mask(FORBID_AFTER_BITS) << FORBID_AFTER_SHIFT;
const MAX_OPEN_MS_MASK: u128 = mask(MAX_OPEN_MS_BITS) << MAX_OPEN_MS_SHIFT;
const MAX_CONTRACTS_MASK: u128 = mask(MAX_CONTRACTS_BITS) << MAX_CONTRACTS_SHIFT;
const MAX_PER_TRADE_MASK: u128 = mask(MAX_PER_TRADE_BITS) << MAX_PER_TRADE_SHIFT;
const REM_DAILY_MASK: u128 = mask(REM_DAILY_BITS) << REM_DAILY_SHIFT;
const USED_MASK: u128 = SEQ_MASK
    | VER_MASK
    | FLAGS_MASK
    | EOD_FLAT_MASK
    | FORBID_AFTER_MASK
    | MAX_OPEN_MS_MASK
    | MAX_CONTRACTS_MASK
    | MAX_PER_TRADE_MASK
    | REM_DAILY_MASK;

/// Bundle of human-readable fields that can be packed into an ARE word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Fields {
    pub rem_daily_loss_cents: u32,
    pub max_per_trade_cents: u32,
    pub max_contracts: u16,
    pub max_open_ms: u32,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub flags: flag::Flags,
    pub version: u8,
    pub sequence: u8,
}

/// Errors encountered when encoding or decoding fields into an ARE word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FieldError {
    Range {
        field: &'static str,
        value: u64,
        max: u64,
    },
    Relation {
        field: &'static str,
        other_field: &'static str,
        field_value: u64,
        other_value: u64,
    },
    ResidualBits {
        residual: u128,
    },
}

/// Simplified classification for `FieldError` to aid logging and metrics pipelines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldErrorKind {
    Range,
    Relation,
    ResidualBits,
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldError::Range { field, value, max } => {
                write!(f, "field `{field}` value {value} exceeds maximum {max}")
            }
            FieldError::Relation {
                field,
                other_field,
                field_value,
                other_value,
            } => write!(
                f,
                "field `{field}` value {field_value} violates relation with `{other_field}` ({other_value})"
            ),
            FieldError::ResidualBits { residual } => {
                write!(f, "unused bits set in envelope: {residual:#034x}")
            }
        }
    }
}

impl FieldError {
    /// Identify the class of error without matching on payloads.
    pub const fn kind(&self) -> FieldErrorKind {
        match self {
            FieldError::Range { .. } => FieldErrorKind::Range,
            FieldError::Relation { .. } => FieldErrorKind::Relation,
            FieldError::ResidualBits { .. } => FieldErrorKind::ResidualBits,
        }
    }

    /// Primary field name implicated in the failure, when available.
    pub const fn field(&self) -> Option<&'static str> {
        match self {
            FieldError::Range { field, .. } | FieldError::Relation { field, .. } => Some(field),
            FieldError::ResidualBits { .. } => None,
        }
    }

    /// Secondary field name when a relational constraint is violated.
    pub const fn other_field(&self) -> Option<&'static str> {
        match self {
            FieldError::Relation { other_field, .. } => Some(other_field),
            _ => None,
        }
    }
}

/// Risk envelope packed into a single `u128` word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct RiskEnvelope {
    bits: u128,
}

impl RiskEnvelope {
    /// Construct from raw bits without validation.
    pub const fn from_bits(bits: u128) -> Self {
        Self { bits }
    }

    /// Construct from bits while enforcing layout constraints.
    pub fn try_from_bits(bits: u128) -> Result<Self, FieldError> {
        let residual = bits & !USED_MASK;
        if residual != 0 {
            return Err(FieldError::ResidualBits { residual });
        }
        let candidate = Self::from_bits(bits);
        let fields = candidate.to_fields();
        validate_fields(&fields)?;
        Ok(candidate)
    }

    /// Pack validated fields into a risk envelope.
    pub fn try_from_fields(fields: Fields) -> Result<Self, FieldError> {
        let f = fields;
        validate_fields(&f)?;

        Ok(Self {
            bits: pack_fields(&f),
        })
    }

    /// Decompose the packed word back into field values.
    pub fn to_fields(self) -> Fields {
        Fields {
            rem_daily_loss_cents: self.rem_daily_loss_cents(),
            max_per_trade_cents: self.max_per_trade_cents(),
            max_contracts: self.max_contracts(),
            max_open_ms: self.max_open_ms(),
            forbid_after_min_ct: self.forbid_after_min_ct(),
            eod_flat_min_ct: self.eod_flat_min_ct(),
            flags: self.flags(),
            version: self.version(),
            sequence: self.sequence(),
        }
    }

    #[inline]
    pub const fn bits(self) -> u128 {
        self.bits
    }

    #[inline]
    pub fn sequence(self) -> u8 {
        ((self.bits & SEQ_MASK) >> SEQ_SHIFT) as u8
    }

    #[inline]
    pub fn version(self) -> u8 {
        ((self.bits & VER_MASK) >> VER_SHIFT) as u8
    }

    #[inline]
    pub fn flags(self) -> flag::Flags {
        flag::Flags::from(((self.bits & FLAGS_MASK) >> FLAGS_SHIFT) as u8)
    }

    #[inline]
    pub fn eod_flat_min_ct(self) -> u16 {
        ((self.bits & EOD_FLAT_MASK) >> EOD_FLAT_SHIFT) as u16
    }

    #[inline]
    pub fn forbid_after_min_ct(self) -> u16 {
        ((self.bits & FORBID_AFTER_MASK) >> FORBID_AFTER_SHIFT) as u16
    }

    #[inline]
    pub fn max_open_ms(self) -> u32 {
        ((self.bits & MAX_OPEN_MS_MASK) >> MAX_OPEN_MS_SHIFT) as u32
    }

    #[inline]
    pub fn max_contracts(self) -> u16 {
        ((self.bits & MAX_CONTRACTS_MASK) >> MAX_CONTRACTS_SHIFT) as u16
    }

    #[inline]
    pub fn max_per_trade_cents(self) -> u32 {
        ((self.bits & MAX_PER_TRADE_MASK) >> MAX_PER_TRADE_SHIFT) as u32
    }

    #[inline]
    pub fn rem_daily_loss_cents(self) -> u32 {
        ((self.bits & REM_DAILY_MASK) >> REM_DAILY_SHIFT) as u32
    }

    /// Update the sequence field, returning a new packed word.
    pub fn with_sequence(mut self, sequence: u8) -> Result<Self, FieldError> {
        check_range("sequence", sequence as u64, SEQ_BITS)?;
        self.bits &= !SEQ_MASK;
        self.bits |= (sequence as u128) << SEQ_SHIFT;
        Ok(self)
    }

    /// Update the remaining daily loss field, returning a new packed word.
    pub fn with_rem_daily_loss_cents(mut self, value: u32) -> Result<Self, FieldError> {
        check_range("rem_daily_loss_cents", value as u64, REM_DAILY_BITS)?;
        self.bits &= !REM_DAILY_MASK;
        self.bits |= (value as u128) << REM_DAILY_SHIFT;
        Ok(self)
    }

    /// Replace the flag field, returning a new packed word.
    pub fn with_flags(mut self, flags: flag::Flags) -> Result<Self, FieldError> {
        check_range("flags", u8::from(flags) as u64, FLAGS_BITS)?;
        self.bits &= !FLAGS_MASK;
        self.bits |= (u8::from(flags) as u128) << FLAGS_SHIFT;
        Ok(self)
    }

    /// Derive new flags via the provided closure.
    pub fn update_flags<F>(self, op: F) -> Result<Self, FieldError>
    where
        F: FnOnce(flag::Flags) -> flag::Flags,
    {
        let next = op(self.flags());
        self.with_flags(next)
    }

    /// Debit the remaining daily loss; returns `None` if the debit would underflow.
    pub fn debit_daily_loss(mut self, amount_cents: u32) -> Option<Self> {
        let remaining = self.rem_daily_loss_cents();
        remaining.checked_sub(amount_cents).map(|updated| {
            self.bits &= !REM_DAILY_MASK;
            self.bits |= (updated as u128) << REM_DAILY_SHIFT;
            self
        })
    }

    /// Debit the remaining daily loss, saturating at zero.
    pub fn saturating_debit_daily_loss(mut self, amount_cents: u32) -> Self {
        let updated = self.rem_daily_loss_cents().saturating_sub(amount_cents);
        self.bits &= !REM_DAILY_MASK;
        self.bits |= (updated as u128) << REM_DAILY_SHIFT;
        self
    }

    /// Evaluate whether an order can be accepted under this envelope.
    pub fn evaluate_order(&self, order: OrderCheck) -> GateOutcome {
        let flags = self.flags();
        if flags.intersects(flag::PAUSED) {
            return GateOutcome::deny(DenyReason::Paused);
        }
        if flags.intersects(flag::EMERGENCY_FLAT) {
            return GateOutcome::deny(DenyReason::EmergencyFlat);
        }
        if flags.intersects(flag::NEWS_LOCKOUT) {
            return GateOutcome::deny(DenyReason::NewsLockout);
        }

        let cost = order.cost_cents;
        let max_trade = self.max_per_trade_cents();
        if max_trade != 0 && cost > max_trade {
            return GateOutcome::deny(DenyReason::PerTradeLimit {
                cost_cents: cost,
                max_per_trade_cents: max_trade,
            });
        }

        let remaining = self.rem_daily_loss_cents();
        if cost > remaining {
            return GateOutcome::deny(DenyReason::DailyLossLimit {
                cost_cents: cost,
                remaining_daily_loss_cents: remaining,
            });
        }

        let contracts = order.contracts;
        let max_contracts = self.max_contracts();
        if max_contracts != 0 && contracts > max_contracts {
            return GateOutcome::deny(DenyReason::ContractLimit {
                contracts,
                max_contracts,
            });
        }

        let max_open_ms = self.max_open_ms();
        if max_open_ms != 0 && order.open_duration_ms > max_open_ms {
            return GateOutcome::deny(DenyReason::OpenDurationLimit {
                open_duration_ms: order.open_duration_ms,
                max_open_ms,
            });
        }

        let now_min = order.minute_ct;
        let forbid_after = self.forbid_after_min_ct();
        if forbid_after != 0 && now_min >= forbid_after {
            return GateOutcome::deny(DenyReason::SessionClosed {
                now_min_ct: now_min,
                forbid_after_min_ct: forbid_after,
            });
        }

        let eod_flat = self.eod_flat_min_ct();
        if eod_flat != 0 && now_min >= eod_flat {
            return GateOutcome::deny(DenyReason::PastEodFlat {
                now_min_ct: now_min,
                eod_flat_min_ct: eod_flat,
            });
        }

        GateOutcome::Allow
    }
}

impl TryFrom<u128> for RiskEnvelope {
    type Error = FieldError;

    fn try_from(value: u128) -> Result<Self, Self::Error> {
        Self::try_from_bits(value)
    }
}

/// Atomic wrapper around the packed ARE word.
#[derive(Debug)]
#[repr(transparent)]
pub struct AtomicRiskEnvelope {
    inner: AtomicU128,
}

impl AtomicRiskEnvelope {
    pub const fn new(initial: RiskEnvelope) -> Self {
        Self {
            inner: AtomicU128::new(initial.bits()),
        }
    }

    pub fn load(&self, order: Ordering) -> RiskEnvelope {
        RiskEnvelope::from_bits(self.inner.load(order))
    }

    pub fn load_validated(&self, order: Ordering) -> Result<RiskEnvelope, FieldError> {
        RiskEnvelope::try_from_bits(self.inner.load(order))
    }

    pub fn store(&self, new: RiskEnvelope, order: Ordering) {
        self.inner.store(new.bits(), order);
    }

    pub fn swap(&self, new: RiskEnvelope, order: Ordering) -> RiskEnvelope {
        RiskEnvelope::from_bits(self.inner.swap(new.bits(), order))
    }

    pub fn compare_exchange(
        &self,
        current: RiskEnvelope,
        new: RiskEnvelope,
        success: Ordering,
        failure: Ordering,
    ) -> Result<RiskEnvelope, RiskEnvelope> {
        self.inner
            .compare_exchange(current.bits(), new.bits(), success, failure)
            .map(RiskEnvelope::from_bits)
            .map_err(RiskEnvelope::from_bits)
    }

    ///
    /// # Stability
    /// Provisional; API may evolve after downstream integration exercises.
    pub fn fetch_update<F>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: F,
    ) -> Result<RiskEnvelope, RiskEnvelope>
    where
        F: FnMut(RiskEnvelope) -> Option<RiskEnvelope>,
    {
        self.inner
            .fetch_update(set_order, fetch_order, |bits| {
                let current = RiskEnvelope::from_bits(bits);
                f(current).map(RiskEnvelope::bits)
            })
            .map(RiskEnvelope::from_bits)
            .map_err(RiskEnvelope::from_bits)
    }

    pub fn debit_daily_loss(
        &self,
        amount_cents: u32,
        set_order: Ordering,
        fetch_order: Ordering,
    ) -> Result<RiskEnvelope, RiskEnvelope> {
        self.fetch_update(set_order, fetch_order, |env| {
            env.debit_daily_loss(amount_cents)
        })
    }
}

/// Inputs required to run the order gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OrderCheck {
    pub cost_cents: u32,
    pub contracts: u16,
    pub minute_ct: u16,
    pub open_duration_ms: u32,
}

impl OrderCheck {
    pub const fn new(
        cost_cents: u32,
        contracts: u16,
        minute_ct: u16,
        open_duration_ms: u32,
    ) -> Self {
        Self {
            cost_cents,
            contracts,
            minute_ct,
            open_duration_ms,
        }
    }
}

/// Outcome produced after evaluating the order gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum GateOutcome {
    Allow,
    Deny(DenyReason),
}

impl GateOutcome {
    pub const fn deny(reason: DenyReason) -> Self {
        GateOutcome::Deny(reason)
    }

    pub const fn is_allow(&self) -> bool {
        matches!(self, GateOutcome::Allow)
    }
}

/// Reasons why the ARE denied an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DenyReason {
    Paused,
    EmergencyFlat,
    NewsLockout,
    PerTradeLimit {
        cost_cents: u32,
        max_per_trade_cents: u32,
    },
    DailyLossLimit {
        cost_cents: u32,
        remaining_daily_loss_cents: u32,
    },
    ContractLimit {
        contracts: u16,
        max_contracts: u16,
    },
    OpenDurationLimit {
        open_duration_ms: u32,
        max_open_ms: u32,
    },
    SessionClosed {
        now_min_ct: u16,
        forbid_after_min_ct: u16,
    },
    PastEodFlat {
        now_min_ct: u16,
        eod_flat_min_ct: u16,
    },
}

impl DenyReason {
    /// Short code identifier useful for logging and metrics labels.
    pub const fn code(&self) -> &'static str {
        match self {
            DenyReason::Paused => "PAUSED",
            DenyReason::EmergencyFlat => "EMERGENCY_FLAT",
            DenyReason::NewsLockout => "NEWS_LOCKOUT",
            DenyReason::PerTradeLimit { .. } => "PER_TRADE_LIMIT",
            DenyReason::DailyLossLimit { .. } => "DAILY_LOSS_LIMIT",
            DenyReason::ContractLimit { .. } => "CONTRACT_LIMIT",
            DenyReason::OpenDurationLimit { .. } => "OPEN_DURATION_LIMIT",
            DenyReason::SessionClosed { .. } => "SESSION_CLOSED",
            DenyReason::PastEodFlat { .. } => "PAST_EOD_FLAT",
        }
    }
}

fn validate_fields(f: &Fields) -> Result<(), FieldError> {
    check_range(
        "rem_daily_loss_cents",
        f.rem_daily_loss_cents as u64,
        REM_DAILY_BITS,
    )?;
    check_range(
        "max_per_trade_cents",
        f.max_per_trade_cents as u64,
        MAX_PER_TRADE_BITS,
    )?;
    check_range("max_contracts", f.max_contracts as u64, MAX_CONTRACTS_BITS)?;
    check_range("max_open_ms", f.max_open_ms as u64, MAX_OPEN_MS_BITS)?;
    check_range(
        "forbid_after_min_ct",
        f.forbid_after_min_ct as u64,
        FORBID_AFTER_BITS,
    )?;
    check_range("eod_flat_min_ct", f.eod_flat_min_ct as u64, EOD_FLAT_BITS)?;
    check_range("flags", u8::from(f.flags) as u64, FLAGS_BITS)?;
    check_range("version", f.version as u64, VER_BITS)?;
    check_range("sequence", f.sequence as u64, SEQ_BITS)?;

    if f.forbid_after_min_ct != 0
        && f.eod_flat_min_ct != 0
        && f.forbid_after_min_ct > f.eod_flat_min_ct
    {
        return Err(FieldError::Relation {
            field: "forbid_after_min_ct",
            other_field: "eod_flat_min_ct",
            field_value: f.forbid_after_min_ct as u64,
            other_value: f.eod_flat_min_ct as u64,
        });
    }

    if f.rem_daily_loss_cents != 0 && f.max_per_trade_cents > f.rem_daily_loss_cents {
        return Err(FieldError::Relation {
            field: "max_per_trade_cents",
            other_field: "rem_daily_loss_cents",
            field_value: f.max_per_trade_cents as u64,
            other_value: f.rem_daily_loss_cents as u64,
        });
    }

    Ok(())
}

fn pack_fields(f: &Fields) -> u128 {
    ((f.rem_daily_loss_cents as u128) << REM_DAILY_SHIFT)
        | ((f.max_per_trade_cents as u128) << MAX_PER_TRADE_SHIFT)
        | ((f.max_contracts as u128) << MAX_CONTRACTS_SHIFT)
        | ((f.max_open_ms as u128) << MAX_OPEN_MS_SHIFT)
        | ((f.forbid_after_min_ct as u128) << FORBID_AFTER_SHIFT)
        | ((f.eod_flat_min_ct as u128) << EOD_FLAT_SHIFT)
        | ((u8::from(f.flags) as u128) << FLAGS_SHIFT)
        | ((f.version as u128) << VER_SHIFT)
        | ((f.sequence as u128) << SEQ_SHIFT)
}

fn check_range(name: &'static str, value: u64, bits: u32) -> Result<(), FieldError> {
    let max = mask(bits) as u64;
    if value > max {
        return Err(FieldError::Range {
            field: name,
            value,
            max,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn fields_fixture() -> Fields {
        Fields {
            rem_daily_loss_cents: 18_000,
            max_per_trade_cents: 4_000,
            max_contracts: 12,
            max_open_ms: 120_000,
            forbid_after_min_ct: 900,
            eod_flat_min_ct: 910,
            flags: flag::PAUSED | flag::NEWS_LOCKOUT,
            version: 3,
            sequence: 17,
        }
    }

    #[test]
    fn pack_round_trip() {
        let fields = fields_fixture();
        let packed = RiskEnvelope::try_from_fields(fields).expect("encode");
        assert_eq!(packed.to_fields(), fields);
    }

    #[test]
    fn range_guard_trips() {
        let mut fields = fields_fixture();
        fields.max_per_trade_cents = 1 << 24; // one more than fits
        let err = RiskEnvelope::try_from_fields(fields).unwrap_err();
        match err {
            FieldError::Range { field, .. } => assert_eq!(field, "max_per_trade_cents"),
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn relation_guard_trips() {
        let mut fields = fields_fixture();
        fields.forbid_after_min_ct = 920;
        let err = RiskEnvelope::try_from_fields(fields).unwrap_err();
        match err {
            FieldError::Relation {
                field, other_field, ..
            } => {
                assert_eq!(field, "forbid_after_min_ct");
                assert_eq!(other_field, "eod_flat_min_ct");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn max_per_trade_not_above_daily() {
        let mut fields = fields_fixture();
        fields.max_per_trade_cents = fields.rem_daily_loss_cents + 1;
        let err = RiskEnvelope::try_from_fields(fields).unwrap_err();
        match err {
            FieldError::Relation {
                field, other_field, ..
            } => {
                assert_eq!(field, "max_per_trade_cents");
                assert_eq!(other_field, "rem_daily_loss_cents");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn evaluate_flags_paused() {
        let fields = fields_fixture();
        let env = RiskEnvelope::try_from_fields(fields).unwrap();
        let outcome = env.evaluate_order(OrderCheck::new(1_000, 1, 100, 0));
        assert!(!outcome.is_allow());
        assert!(matches!(outcome, GateOutcome::Deny(DenyReason::Paused)));
    }

    #[test]
    fn evaluate_contract_guard() {
        let mut fields = fields_fixture();
        fields.flags = flag::Flags::EMPTY;
        let env = RiskEnvelope::try_from_fields(fields).unwrap();
        let outcome = env.evaluate_order(OrderCheck::new(500, 13, 100, 0));
        assert!(matches!(
            outcome,
            GateOutcome::Deny(DenyReason::ContractLimit { contracts: 13, .. })
        ));
    }

    #[test]
    fn evaluate_allows() {
        let mut fields = fields_fixture();
        fields.flags = flag::Flags::EMPTY;
        let env = RiskEnvelope::try_from_fields(fields).unwrap();
        let outcome = env.evaluate_order(OrderCheck::new(3_500, 5, 800, 60_000));
        assert!(outcome.is_allow());
    }

    #[test]
    fn open_duration_guard_trips() {
        let mut fields = fields_fixture();
        fields.flags = flag::Flags::EMPTY;
        fields.max_open_ms = 30_000;
        let env = RiskEnvelope::try_from_fields(fields).unwrap();
        let outcome = env.evaluate_order(OrderCheck::new(1_000, 1, 100, 45_000));
        assert!(matches!(
            outcome,
            GateOutcome::Deny(DenyReason::OpenDurationLimit {
                open_duration_ms: 45_000,
                ..
            })
        ));
    }

    #[test]
    fn atomic_round_trip() {
        let env = RiskEnvelope::try_from_fields(fields_fixture()).unwrap();
        let atomic = AtomicRiskEnvelope::new(env);
        let loaded = atomic.load(Ordering::Relaxed);
        assert_eq!(loaded.bits(), env.bits());
    }

    #[test]
    fn try_from_bits_validates() {
        let mut fields = fields_fixture();
        fields.forbid_after_min_ct = 920;
        let bits = pack_fields(&fields);
        let err = RiskEnvelope::try_from_bits(bits).unwrap_err();
        assert!(matches!(err, FieldError::Relation { .. }));
    }

    #[test]
    fn debit_daily_loss_succeeds() {
        let fields = fields_fixture();
        let env = RiskEnvelope::try_from_fields(fields).unwrap();
        let updated = env.debit_daily_loss(1_000).expect("balance covers fill");
        assert_eq!(
            updated.rem_daily_loss_cents(),
            env.rem_daily_loss_cents() - 1_000
        );
    }

    #[test]
    fn debit_daily_loss_rejects_underflow() {
        let fields = fields_fixture();
        let env = RiskEnvelope::try_from_fields(fields).unwrap();
        assert!(env.debit_daily_loss(1_000_000).is_none());
    }

    #[test]
    fn atomic_debit_daily_loss_updates_word() {
        let fields = fields_fixture();
        let env = RiskEnvelope::try_from_fields(fields).unwrap();
        let atomic = AtomicRiskEnvelope::new(env);
        let prev = atomic
            .debit_daily_loss(2_000, Ordering::SeqCst, Ordering::SeqCst)
            .expect("update should succeed");
        assert_eq!(prev.rem_daily_loss_cents(), env.rem_daily_loss_cents());
        let current = atomic.load(Ordering::SeqCst);
        assert_eq!(
            current.rem_daily_loss_cents(),
            env.rem_daily_loss_cents() - 2_000
        );

        let err = atomic.debit_daily_loss(1_000_000, Ordering::SeqCst, Ordering::SeqCst);
        assert!(err.is_err());
        let snapshot = atomic.load(Ordering::SeqCst);
        assert_eq!(
            snapshot.rem_daily_loss_cents(),
            current.rem_daily_loss_cents()
        );
    }

    const REM_MAX: u32 = super::mask(super::REM_DAILY_BITS) as u32;
    const TRADE_MAX: u32 = super::mask(super::MAX_PER_TRADE_BITS) as u32;
    const CONTRACT_MAX: u16 = super::mask(super::MAX_CONTRACTS_BITS) as u16;
    const OPEN_MS_MAX: u32 = super::mask(super::MAX_OPEN_MS_BITS) as u32;
    const FORBID_MAX: u16 = super::mask(super::FORBID_AFTER_BITS) as u16;
    const EOD_MAX: u16 = super::mask(super::EOD_FLAT_BITS) as u16;
    const FLAGS_MAX: u8 = super::mask(super::FLAGS_BITS) as u8;
    const VER_MAX: u8 = super::mask(super::VER_BITS) as u8;
    const SEQ_MAX: u8 = super::mask(super::SEQ_BITS) as u8;

    proptest! {
        #[test]
        fn prop_pack_round_trip_random(
            rem in 0u32..=REM_MAX,
            trade in 0u32..=TRADE_MAX,
            contracts in 0u16..=CONTRACT_MAX,
            max_open in 0u32..=OPEN_MS_MAX,
            eod in 0u16..=EOD_MAX,
            forbid in 0u16..=FORBID_MAX,
            flags in 0u8..=FLAGS_MAX,
            version in 0u8..=VER_MAX,
            sequence in 0u8..=SEQ_MAX,
        ) {
            let mut fields = Fields {
                rem_daily_loss_cents: rem,
                max_per_trade_cents: trade,
                max_contracts: contracts,
                max_open_ms: max_open,
                forbid_after_min_ct: forbid,
                eod_flat_min_ct: eod,
                flags: flag::Flags::from(flags),
                version,
                sequence,
            };

            if fields.rem_daily_loss_cents != 0 {
                fields.max_per_trade_cents = fields
                    .max_per_trade_cents
                    .min(fields.rem_daily_loss_cents);
            }

            if fields.eod_flat_min_ct != 0 {
                fields.forbid_after_min_ct = fields
                    .forbid_after_min_ct
                    .min(fields.eod_flat_min_ct);
            }

            let packed = RiskEnvelope::try_from_fields(fields).unwrap();
            let round = packed.to_fields();
            prop_assert_eq!(round, fields);
            let validated = RiskEnvelope::try_from_bits(packed.bits()).unwrap();
            prop_assert_eq!(validated.bits(), packed.bits());
        }
    }
}
