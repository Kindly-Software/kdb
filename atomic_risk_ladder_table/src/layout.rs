//! Bit packing layout for Risk Ladder Table 1024 structure.
//! All truncating casts are intentional for fixed-point conversion.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::missing_errors_doc)]

use core::fmt;

/// Describes a contiguous bit-range inside a 128-bit word.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
    /// Bit offset of the field, starting at the least-significant bit.
    pub shift: u8,
    /// Width of the field in bits.
    pub width: u8,
}

impl FieldSpec {
    /// Creates a new [`FieldSpec`].
    #[must_use]
    pub const fn new(shift: u8, width: u8) -> Self {
        Self { shift, width }
    }

    /// Maximum value the field can represent.
    #[must_use]
    pub const fn max_value(self) -> u128 {
        ones(self.width)
    }

    /// Bit-mask covering the full field.
    #[must_use]
    pub const fn mask(self) -> u128 {
        self.max_value() << self.shift
    }
}

const fn ones(width: u8) -> u128 {
    if width >= 128 {
        u128::MAX
    } else if width == 0 {
        0
    } else {
        (1u128 << width) - 1
    }
}

const fn extract(word: u128, field: FieldSpec) -> u128 {
    (word >> field.shift) & ones(field.width)
}

fn write(word: &mut u128, field: FieldSpec, value: u128) {
    debug_assert!(
        value <= field.max_value(),
        "value {} exceeds width {}",
        value,
        field.width
    );
    let mask = field.mask();
    *word = (*word & !mask) | ((value & field.max_value()) << field.shift);
}

fn write_bool(word: &mut u128, field: FieldSpec, value: bool) {
    write(word, field, u128::from(value));
}

const fn read_bool(word: u128, field: FieldSpec) -> bool {
    extract(word, field) != 0
}

/// Q1.7 gain applied to trip thresholds to derive recovery thresholds.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecoverScale(u8);

impl RecoverScale {
    /// Creates a new [`RecoverScale`] from a raw Q1.7 encoded byte.
    #[must_use]
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    /// Raw encoded value (Q1.7).
    #[must_use]
    pub const fn raw(self) -> u8 {
        self.0
    }

    /// Computes the recovery threshold for a given trip value.
    #[must_use]
    pub const fn apply(self, trip: u16) -> u16 {
        let scaled = (trip as u32 * self.0 as u32) >> 7;
        scaled as u16
    }
}

impl fmt::Debug for RecoverScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let numerator = f32::from(self.0);
        let value = numerator / 128.0;
        write!(f, "RecoverScale({value:.3})")
    }
}

/// Header / global policy word helpers.
pub mod header {
    use super::{extract, read_bool, write, write_bool, FieldSpec, RecoverScale};
    use core::fmt;

    /// Layout specification for W0 (header).
    #[allow(missing_docs)]
    pub mod fields {
        use super::FieldSpec;

        pub const COMMIT: FieldSpec = FieldSpec::new(0, 1);
        pub const STALE: FieldSpec = FieldSpec::new(1, 1);
        pub const VER_EVEN: FieldSpec = FieldSpec::new(2, 8);
        pub const SEQ_HEAD: FieldSpec = FieldSpec::new(10, 16);
        pub const POLICY_ID: FieldSpec = FieldSpec::new(26, 16);
        pub const STRATEGY_MASK: FieldSpec = FieldSpec::new(42, 4);
        pub const RECOVER_SCALE: FieldSpec = FieldSpec::new(46, 8);
        pub const DWELL_UP_MS: FieldSpec = FieldSpec::new(54, 12);
        pub const DWELL_DOWN_MS: FieldSpec = FieldSpec::new(66, 12);
        pub const CREATED_MS_COARSE: FieldSpec = FieldSpec::new(78, 24);
        pub const GLOBAL_FLAGS: FieldSpec = FieldSpec::new(102, 14);
        pub const RESERVED: FieldSpec = FieldSpec::new(116, 12);
    }

    /// Mask describing which strategies are encoded in the table.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct StrategyMask(u8);

    impl StrategyMask {
        /// Strategy A bit flag.
        pub const STRATEGY_A: u8 = 0b001;
        /// Strategy B bit flag.
        pub const STRATEGY_B: u8 = 0b010;
        /// Strategy C bit flag.
        pub const STRATEGY_C: u8 = 0b100;

        /// Creates a new mask from raw bits.
        #[must_use]
        pub const fn new(raw: u8) -> Self {
            Self(raw & 0b1111)
        }

        /// Returns the raw 4-bit value.
        #[must_use]
        pub const fn raw(self) -> u8 {
            self.0 & 0b1111
        }

        /// Returns whether a given strategy bit is enabled.
        #[must_use]
        pub const fn contains(self, bit: u8) -> bool {
            (self.raw() & bit) != 0
        }
    }

    /// W0 header word wrapper.
    #[repr(transparent)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct HeaderWord {
        raw: u128,
    }

    impl HeaderWord {
        /// All-zero word constant.
        pub const ZERO: Self = Self { raw: 0 };

        /// Creates a new word from the provided raw bits.
        #[must_use]
        pub const fn from_raw(raw: u128) -> Self {
            Self { raw }
        }

        /// Returns the underlying bits.
        #[must_use]
        pub const fn raw(self) -> u128 {
            self.raw
        }

        /// Sets the commit bit (true once the word is ready for publish).
        pub fn set_commit(&mut self, commit: bool) {
            write_bool(&mut self.raw, fields::COMMIT, commit);
        }

        /// Returns the commit bit.
        #[must_use]
        pub const fn commit(self) -> bool {
            read_bool(self.raw, fields::COMMIT)
        }

        /// Sets the stale flag.
        pub fn set_stale(&mut self, stale: bool) {
            write_bool(&mut self.raw, fields::STALE, stale);
        }

        /// Returns the stale flag.
        #[must_use]
        pub const fn stale(self) -> bool {
            read_bool(self.raw, fields::STALE)
        }

        /// Sets the version (must be even).
        pub fn set_version_even(&mut self, version: u8) {
            debug_assert_eq!(version & 1, 0, "version must be even");
            write(&mut self.raw, fields::VER_EVEN, u128::from(version));
        }

        /// Returns the encoded (even) version byte.
        #[must_use]
        pub const fn version_even(self) -> u8 {
            extract(self.raw, fields::VER_EVEN) as u8
        }

        /// Sets the head sequence counter.
        pub fn set_seq_head(&mut self, sequence: u16) {
            write(&mut self.raw, fields::SEQ_HEAD, u128::from(sequence));
        }

        /// Returns the head sequence counter.
        #[must_use]
        pub const fn seq_head(self) -> u16 {
            extract(self.raw, fields::SEQ_HEAD) as u16
        }

        /// Sets the policy identifier.
        pub fn set_policy_id(&mut self, policy_id: u16) {
            write(&mut self.raw, fields::POLICY_ID, u128::from(policy_id));
        }

        /// Returns the policy identifier.
        #[must_use]
        pub const fn policy_id(self) -> u16 {
            extract(self.raw, fields::POLICY_ID) as u16
        }

        /// Configures the strategy mask.
        pub fn set_strategy_mask(&mut self, mask: StrategyMask) {
            write(&mut self.raw, fields::STRATEGY_MASK, u128::from(mask.raw()));
        }

        /// Returns the strategy mask bits.
        #[must_use]
        pub const fn strategy_mask(self) -> StrategyMask {
            StrategyMask::new(extract(self.raw, fields::STRATEGY_MASK) as u8)
        }

        /// Sets the recover scale gain.
        pub fn set_recover_scale(&mut self, scale: RecoverScale) {
            write(
                &mut self.raw,
                fields::RECOVER_SCALE,
                u128::from(scale.raw()),
            );
        }

        /// Returns the encoded recover scale gain.
        #[must_use]
        pub const fn recover_scale(self) -> RecoverScale {
            RecoverScale::new(extract(self.raw, fields::RECOVER_SCALE) as u8)
        }

        /// Configures the default dwell-up time (ms).
        pub fn set_dwell_up_ms(&mut self, dwell_ms: u16) {
            debug_assert!(dwell_ms < (1 << 12));
            write(&mut self.raw, fields::DWELL_UP_MS, u128::from(dwell_ms));
        }

        /// Returns the default dwell-up time (ms).
        #[must_use]
        pub const fn dwell_up_ms(self) -> u16 {
            extract(self.raw, fields::DWELL_UP_MS) as u16
        }

        /// Configures the default dwell-down time (ms).
        pub fn set_dwell_down_ms(&mut self, dwell_ms: u16) {
            debug_assert!(dwell_ms < (1 << 12));
            write(&mut self.raw, fields::DWELL_DOWN_MS, u128::from(dwell_ms));
        }

        /// Returns the default dwell-down time (ms).
        #[must_use]
        pub const fn dwell_down_ms(self) -> u16 {
            extract(self.raw, fields::DWELL_DOWN_MS) as u16
        }

        /// Sets the coarse creation timestamp (ms/4 units).
        pub fn set_created_ms_coarse(&mut self, timestamp: u32) {
            debug_assert!(timestamp < (1 << 24));
            write(
                &mut self.raw,
                fields::CREATED_MS_COARSE,
                u128::from(timestamp),
            );
        }

        /// Returns the creation timestamp (ms/4 units).
        #[must_use]
        pub const fn created_ms_coarse(self) -> u32 {
            extract(self.raw, fields::CREATED_MS_COARSE) as u32
        }

        /// Sets global flag bits.
        pub fn set_global_flags(&mut self, flags: u16) {
            debug_assert!(flags < (1 << 14));
            write(&mut self.raw, fields::GLOBAL_FLAGS, u128::from(flags));
        }

        /// Returns global flag bits.
        #[must_use]
        pub const fn global_flags(self) -> u16 {
            extract(self.raw, fields::GLOBAL_FLAGS) as u16
        }

        /// Zeros the reserved bits to maintain canonical encoding.
        pub fn clear_reserved(&mut self) {
            write(&mut self.raw, fields::RESERVED, 0);
        }
    }

    impl Default for HeaderWord {
        fn default() -> Self {
            Self::ZERO
        }
    }

    impl fmt::Debug for HeaderWord {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("HeaderWord")
                .field("commit", &self.commit())
                .field("stale", &self.stale())
                .field("version_even", &self.version_even())
                .field("seq_head", &self.seq_head())
                .field("policy_id", &self.policy_id())
                .field("strategy_mask", &self.strategy_mask())
                .field("recover_scale", &self.recover_scale())
                .field("dwell_up_ms", &self.dwell_up_ms())
                .field("dwell_down_ms", &self.dwell_down_ms())
                .field("created_ms_coarse", &self.created_ms_coarse())
                .field("global_flags", &self.global_flags())
                .finish()
        }
    }
}

/// Trip threshold packing helpers.
pub mod trips {
    use super::{extract, write, FieldSpec};
    use core::fmt;

    /// Layout constants for trip thresholds.
    #[allow(missing_docs)]
    pub mod fields {
        use super::FieldSpec;

        pub const ALT_L1: FieldSpec = FieldSpec::new(0, 10);
        pub const ALT_L2: FieldSpec = FieldSpec::new(10, 10);
        pub const ALT_L3: FieldSpec = FieldSpec::new(20, 10);

        pub const REJ_L1: FieldSpec = FieldSpec::new(30, 10);
        pub const REJ_L2: FieldSpec = FieldSpec::new(40, 10);
        pub const REJ_L3: FieldSpec = FieldSpec::new(50, 10);

        pub const LOSS_L1: FieldSpec = FieldSpec::new(60, 10);
        pub const LOSS_L2: FieldSpec = FieldSpec::new(70, 10);
        pub const LOSS_L3: FieldSpec = FieldSpec::new(80, 10);

        pub const VOL_L1: FieldSpec = FieldSpec::new(90, 12);
        pub const VOL_L2: FieldSpec = FieldSpec::new(102, 12);
        pub const VOL_L3: FieldSpec = FieldSpec::new(114, 12);

        pub const SPARE: FieldSpec = FieldSpec::new(126, 2);
    }

    /// Structured trip thresholds across four axes.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct TripThresholds {
        /// ALT axis thresholds (L1..L3).
        pub alt: [u16; 3],
        /// Reject rate thresholds (basis points, L1..L3).
        pub rej: [u16; 3],
        /// Packet loss thresholds (basis points, L1..L3).
        pub loss: [u16; 3],
        /// Volatility thresholds (Q4.8, L1..L3).
        pub vol: [u16; 3],
    }

    impl TripThresholds {
        /// Baseline configuration recommended by the spec.
        pub const DEFAULT: Self = Self {
            alt: [640, 896, 1023],
            rej: [150, 300, 600],
            loss: [50, 200, 400],
            vol: [384, 640, 1024],
        };
    }

    impl Default for TripThresholds {
        fn default() -> Self {
            Self::DEFAULT
        }
    }

    /// Wtrip wrapper.
    #[repr(transparent)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct TripWord {
        raw: u128,
    }

    impl TripWord {
        /// Zero word constant.
        pub const ZERO: Self = Self { raw: 0 };

        /// Creates a word from raw bits.
        #[must_use]
        pub const fn from_raw(raw: u128) -> Self {
            Self { raw }
        }

        /// Returns the raw bits.
        #[must_use]
        pub const fn raw(self) -> u128 {
            self.raw
        }

        fn set_axis(&mut self, fields: [FieldSpec; 3], values: [u16; 3]) {
            for (field, value) in fields.into_iter().zip(values) {
                write(&mut self.raw, field, u128::from(value));
            }
        }

        fn axis(&self, fields: [FieldSpec; 3]) -> [u16; 3] {
            let mut out = [0u16; 3];
            let mut idx = 0;
            while idx < 3 {
                out[idx] = extract(self.raw, fields[idx]) as u16;
                idx += 1;
            }
            out
        }

        /// Configures every axis according to the provided struct.
        pub fn set_thresholds(&mut self, thresholds: TripThresholds) {
            self.set_axis(
                [fields::ALT_L1, fields::ALT_L2, fields::ALT_L3],
                thresholds.alt,
            );
            self.set_axis(
                [fields::REJ_L1, fields::REJ_L2, fields::REJ_L3],
                thresholds.rej,
            );
            self.set_axis(
                [fields::LOSS_L1, fields::LOSS_L2, fields::LOSS_L3],
                thresholds.loss,
            );
            self.set_axis(
                [fields::VOL_L1, fields::VOL_L2, fields::VOL_L3],
                thresholds.vol,
            );
        }

        /// Returns the decoded thresholds.
        #[must_use]
        pub fn thresholds(self) -> TripThresholds {
            TripThresholds {
                alt: self.axis([fields::ALT_L1, fields::ALT_L2, fields::ALT_L3]),
                rej: self.axis([fields::REJ_L1, fields::REJ_L2, fields::REJ_L3]),
                loss: self.axis([fields::LOSS_L1, fields::LOSS_L2, fields::LOSS_L3]),
                vol: self.axis([fields::VOL_L1, fields::VOL_L2, fields::VOL_L3]),
            }
        }

        /// Clears the spare bits for canonical encoding.
        pub fn clear_spare(&mut self) {
            super::write(&mut self.raw, fields::SPARE, 0);
        }
    }

    impl Default for TripWord {
        fn default() -> Self {
            Self::ZERO
        }
    }

    impl fmt::Debug for TripWord {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let thresholds = self.thresholds();
            f.debug_struct("TripWord")
                .field("alt", &thresholds.alt)
                .field("rej", &thresholds.rej)
                .field("loss", &thresholds.loss)
                .field("vol", &thresholds.vol)
                .finish()
        }
    }
}

/// Action packing helper module.
pub mod actions {
    use super::{extract, write, FieldSpec};
    use core::fmt;

    /// Layout constants for Wact words.
    #[allow(missing_docs)]
    pub mod fields {
        use super::FieldSpec;

        pub const SIZE_L0: FieldSpec = FieldSpec::new(0, 8);
        pub const SIZE_L1: FieldSpec = FieldSpec::new(8, 8);
        pub const SIZE_L2: FieldSpec = FieldSpec::new(16, 8);
        pub const SIZE_L3: FieldSpec = FieldSpec::new(24, 8);

        pub const SLIP_L0: FieldSpec = FieldSpec::new(32, 8);
        pub const SLIP_L1: FieldSpec = FieldSpec::new(40, 8);
        pub const SLIP_L2: FieldSpec = FieldSpec::new(48, 8);
        pub const SLIP_L3: FieldSpec = FieldSpec::new(56, 8);

        pub const LAT_L0: FieldSpec = FieldSpec::new(64, 8);
        pub const LAT_L1: FieldSpec = FieldSpec::new(72, 8);
        pub const LAT_L2: FieldSpec = FieldSpec::new(80, 8);
        pub const LAT_L3: FieldSpec = FieldSpec::new(88, 8);

        pub const ROUTE_L0: FieldSpec = FieldSpec::new(96, 2);
        pub const ROUTE_L1: FieldSpec = FieldSpec::new(98, 2);
        pub const ROUTE_L2: FieldSpec = FieldSpec::new(100, 2);
        pub const ROUTE_L3: FieldSpec = FieldSpec::new(102, 2);

        pub const DWELL_UP: FieldSpec = FieldSpec::new(104, 12);
        pub const DWELL_DOWN: FieldSpec = FieldSpec::new(116, 12);
    }

    #[inline]
    fn decode_q2_6(raw: u8) -> f32 {
        f32::from(raw) / 64.0
    }

    #[inline]
    fn decode_q1_7(raw: u8) -> f32 {
        f32::from(raw) / 128.0
    }

    /// Routing policy flags per level.
    #[repr(u8)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum RoutePolicy {
        /// Normal routing (status quo).
        Normal = 0,
        /// Prefer maker routes.
        MakerPreferred = 1,
        /// Allow taker-only routes (disable maker).
        TakerOnly = 2,
        /// Forbid new risk (reduce-only).
        ForbidNew = 3,
    }

    impl RoutePolicy {
        /// Falls back to [`RoutePolicy::Normal`] when the raw value is out of range.
        #[must_use]
        pub const fn from_raw(raw: u8) -> Self {
            match raw {
                0 => Self::Normal,
                1 => Self::MakerPreferred,
                2 => Self::TakerOnly,
                3 => Self::ForbidNew,
                _ => Self::Normal,
            }
        }
    }

    /// Structured action bundle for easier authoring.
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct ActionsWordDraft {
        /// Q2.6 size multipliers for levels L0..L3.
        pub size_q2_6: [u8; 4],
        /// Q1.7 slip cap multipliers for levels L0..L3.
        pub slip_q1_7: [u8; 4],
        /// Q1.7 latency budget multipliers for levels L0..L3.
        pub latency_q1_7: [u8; 4],
        /// Route policy bits for levels L0..L3.
        pub route: [RoutePolicy; 4],
        /// Optional dwell-up override in milliseconds.
        pub dwell_up_ms: u16,
        /// Optional dwell-down override in milliseconds.
        pub dwell_down_ms: u16,
    }

    impl ActionsWordDraft {
        /// Recommended defaults derived from the spec guidance.
        pub const DEFAULT: Self = Self {
            size_q2_6: [64, 32, 16, 0],
            slip_q1_7: [128, 109, 90, 90],
            latency_q1_7: [128, 109, 90, 64],
            route: [
                RoutePolicy::Normal,
                RoutePolicy::MakerPreferred,
                RoutePolicy::TakerOnly,
                RoutePolicy::ForbidNew,
            ],
            dwell_up_ms: 0,
            dwell_down_ms: 0,
        };

        /// Returns the size multiplier for the requested level as an `f32`.
        #[must_use]
        pub fn size_multiplier(&self, level: u8) -> f32 {
            decode_q2_6(self.size_q2_6[level.min(3) as usize])
        }

        /// Returns the slip-cap multiplier for the requested level.
        #[must_use]
        pub fn slip_multiplier(&self, level: u8) -> f32 {
            decode_q1_7(self.slip_q1_7[level.min(3) as usize])
        }

        /// Returns the latency-budget multiplier for the requested level.
        #[must_use]
        pub fn latency_multiplier(&self, level: u8) -> f32 {
            decode_q1_7(self.latency_q1_7[level.min(3) as usize])
        }

        /// Returns the routing directive for the requested level.
        #[must_use]
        pub fn route_policy(&self, level: u8) -> RoutePolicy {
            self.route[level.min(3) as usize]
        }

        /// Applies the level-specific multipliers to the supplied bases.
        #[must_use]
        pub fn apply_to(&self, level: u8, bases: ActionBases) -> AppliedActionSet {
            AppliedActionSet {
                size: bases.size * self.size_multiplier(level),
                slip_cap: bases.slip_cap * self.slip_multiplier(level),
                latency_budget: bases.latency_budget * self.latency_multiplier(level),
                route: self.route_policy(level),
            }
        }
    }

    impl Default for ActionsWordDraft {
        fn default() -> Self {
            Self::DEFAULT
        }
    }

    /// Base values used when applying action multipliers.
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ActionBases {
        /// Nominal order size.
        pub size: f32,
        /// Baseline slip cap (basis points).
        pub slip_cap: f32,
        /// Baseline latency budget (microseconds or milliseconds, caller-defined units).
        pub latency_budget: f32,
    }

    /// Applied values for a given level after scaling.
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct AppliedActionSet {
        /// Adjusted order size.
        pub size: f32,
        /// Adjusted slip cap.
        pub slip_cap: f32,
        /// Adjusted latency budget.
        pub latency_budget: f32,
        /// Route directive for the level.
        pub route: RoutePolicy,
    }

    /// Wact word wrapper.
    #[repr(transparent)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct ActionWord {
        raw: u128,
    }

    impl ActionWord {
        /// Zero word constant.
        pub const ZERO: Self = Self { raw: 0 };

        /// Creates from raw bits.
        #[must_use]
        pub const fn from_raw(raw: u128) -> Self {
            Self { raw }
        }

        /// Returns the raw bits.
        #[must_use]
        pub const fn raw(self) -> u128 {
            self.raw
        }

        /// Loads a structured draft representation.
        #[must_use]
        pub fn draft(self) -> ActionsWordDraft {
            ActionsWordDraft {
                size_q2_6: self.level_array([
                    fields::SIZE_L0,
                    fields::SIZE_L1,
                    fields::SIZE_L2,
                    fields::SIZE_L3,
                ]),
                slip_q1_7: self.level_array([
                    fields::SLIP_L0,
                    fields::SLIP_L1,
                    fields::SLIP_L2,
                    fields::SLIP_L3,
                ]),
                latency_q1_7: self.level_array([
                    fields::LAT_L0,
                    fields::LAT_L1,
                    fields::LAT_L2,
                    fields::LAT_L3,
                ]),
                route: self.routes(),
                dwell_up_ms: extract(self.raw, fields::DWELL_UP) as u16,
                dwell_down_ms: extract(self.raw, fields::DWELL_DOWN) as u16,
            }
        }

        /// Applies a structured draft.
        pub fn apply_draft(&mut self, draft: ActionsWordDraft) {
            self.set_level_array(
                [
                    fields::SIZE_L0,
                    fields::SIZE_L1,
                    fields::SIZE_L2,
                    fields::SIZE_L3,
                ],
                draft.size_q2_6,
            );
            self.set_level_array(
                [
                    fields::SLIP_L0,
                    fields::SLIP_L1,
                    fields::SLIP_L2,
                    fields::SLIP_L3,
                ],
                draft.slip_q1_7,
            );
            self.set_level_array(
                [
                    fields::LAT_L0,
                    fields::LAT_L1,
                    fields::LAT_L2,
                    fields::LAT_L3,
                ],
                draft.latency_q1_7,
            );
            for (slot, policy) in [
                fields::ROUTE_L0,
                fields::ROUTE_L1,
                fields::ROUTE_L2,
                fields::ROUTE_L3,
            ]
            .into_iter()
            .zip(draft.route)
            {
                write(&mut self.raw, slot, u128::from(policy as u8));
            }
            self.set_dwell_up_ms(draft.dwell_up_ms);
            self.set_dwell_down_ms(draft.dwell_down_ms);
        }

        fn level_array(&self, fields: [FieldSpec; 4]) -> [u8; 4] {
            let mut out = [0u8; 4];
            let mut idx = 0;
            while idx < 4 {
                out[idx] = extract(self.raw, fields[idx]) as u8;
                idx += 1;
            }
            out
        }

        fn set_level_array(&mut self, fields: [FieldSpec; 4], values: [u8; 4]) {
            for (field, value) in fields.into_iter().zip(values) {
                write(&mut self.raw, field, u128::from(value));
            }
        }

        fn routes(&self) -> [RoutePolicy; 4] {
            let raw = self.level_array([
                fields::ROUTE_L0,
                fields::ROUTE_L1,
                fields::ROUTE_L2,
                fields::ROUTE_L3,
            ]);
            [
                RoutePolicy::from_raw(raw[0]),
                RoutePolicy::from_raw(raw[1]),
                RoutePolicy::from_raw(raw[2]),
                RoutePolicy::from_raw(raw[3]),
            ]
        }

        /// Sets the dwell-up override in milliseconds.
        pub fn set_dwell_up_ms(&mut self, value: u16) {
            debug_assert!(value < (1 << 12));
            write(&mut self.raw, fields::DWELL_UP, u128::from(value));
        }

        /// Returns the dwell-up override in milliseconds.
        #[must_use]
        pub const fn dwell_up_ms(self) -> u16 {
            extract(self.raw, fields::DWELL_UP) as u16
        }

        /// Sets the dwell-down override in milliseconds.
        pub fn set_dwell_down_ms(&mut self, value: u16) {
            debug_assert!(value < (1 << 12));
            write(&mut self.raw, fields::DWELL_DOWN, u128::from(value));
        }

        /// Returns the dwell-down override in milliseconds.
        #[must_use]
        pub const fn dwell_down_ms(self) -> u16 {
            extract(self.raw, fields::DWELL_DOWN) as u16
        }
    }

    impl Default for ActionWord {
        fn default() -> Self {
            Self::ZERO
        }
    }

    impl fmt::Debug for ActionWord {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let draft = self.draft();
            f.debug_struct("ActionWord")
                .field("size_q2_6", &draft.size_q2_6)
                .field("slip_q1_7", &draft.slip_q1_7)
                .field("latency_q1_7", &draft.latency_q1_7)
                .field("route", &draft.route)
                .field("dwell_up_ms", &draft.dwell_up_ms)
                .field("dwell_down_ms", &draft.dwell_down_ms)
                .finish()
        }
    }
}

/// Tail word helpers.
pub mod tail {
    use super::{extract, write, FieldSpec};
    use core::fmt;

    /// Layout constants for W7 (tail word).
    #[allow(missing_docs)]
    pub mod fields {
        use super::FieldSpec;

        pub const CHECKSUM16: FieldSpec = FieldSpec::new(0, 16);
        pub const VERSION_TAIL: FieldSpec = FieldSpec::new(16, 8);
        pub const SEQ_TAIL: FieldSpec = FieldSpec::new(24, 16);
        pub const ALT_IDX_SCALE_HINT: FieldSpec = FieldSpec::new(40, 10);
        pub const ECO_ACTION_BIND: FieldSpec = FieldSpec::new(50, 2);
        pub const STRAT_ENABLE_MASK: FieldSpec = FieldSpec::new(52, 4);
        pub const SPARE: FieldSpec = FieldSpec::new(56, 72);
    }

    /// Tail word wrapper providing integrity helpers.
    #[repr(transparent)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct TailWord {
        raw: u128,
    }

    impl TailWord {
        /// Zero tail constant.
        pub const ZERO: Self = Self { raw: 0 };

        /// Instantiates from raw bits.
        #[must_use]
        pub const fn from_raw(raw: u128) -> Self {
            Self { raw }
        }

        /// Returns the raw bits.
        #[must_use]
        pub const fn raw(self) -> u128 {
            self.raw
        }

        /// Stores the checksum value.
        pub fn set_checksum(&mut self, checksum: u16) {
            write(&mut self.raw, fields::CHECKSUM16, u128::from(checksum));
        }

        /// Returns the checksum field.
        #[must_use]
        pub const fn checksum(self) -> u16 {
            extract(self.raw, fields::CHECKSUM16) as u16
        }

        /// Sets the tail version byte (should match header version).
        pub fn set_version(&mut self, version: u8) {
            write(&mut self.raw, fields::VERSION_TAIL, u128::from(version));
        }

        /// Returns the tail version byte.
        #[must_use]
        pub const fn version(self) -> u8 {
            extract(self.raw, fields::VERSION_TAIL) as u8
        }

        /// Sets the tail sequence counter.
        pub fn set_seq_tail(&mut self, seq: u16) {
            write(&mut self.raw, fields::SEQ_TAIL, u128::from(seq));
        }

        /// Returns the tail sequence counter.
        #[must_use]
        pub const fn seq_tail(self) -> u16 {
            extract(self.raw, fields::SEQ_TAIL) as u16
        }

        /// Encodes the ALT index scaling hint.
        pub fn set_alt_idx_scale_hint(&mut self, hint: u16) {
            debug_assert!(hint < (1 << 10));
            write(&mut self.raw, fields::ALT_IDX_SCALE_HINT, u128::from(hint));
        }

        /// Returns the ALT index scaling hint.
        #[must_use]
        pub const fn alt_idx_scale_hint(self) -> u16 {
            extract(self.raw, fields::ALT_IDX_SCALE_HINT) as u16
        }

        /// Configures the ECO action bind mapping.
        pub fn set_eco_action_bind(&mut self, value: u8) {
            debug_assert!(value < 4);
            write(&mut self.raw, fields::ECO_ACTION_BIND, u128::from(value));
        }

        /// Returns the ECO action bind.
        #[must_use]
        pub const fn eco_action_bind(self) -> u8 {
            extract(self.raw, fields::ECO_ACTION_BIND) as u8
        }

        /// Configures per-strategy tail enable mask.
        pub fn set_strategy_enable_mask(&mut self, mask: u8) {
            debug_assert!(mask < 16);
            write(&mut self.raw, fields::STRAT_ENABLE_MASK, u128::from(mask));
        }

        /// Returns the strategy enable mask bits.
        #[must_use]
        pub const fn strategy_enable_mask(self) -> u8 {
            extract(self.raw, fields::STRAT_ENABLE_MASK) as u8
        }

        /// Zeros the spare bits for canonical encoding.
        pub fn clear_spare(&mut self) {
            super::write(&mut self.raw, fields::SPARE, 0);
        }
    }

    impl Default for TailWord {
        fn default() -> Self {
            Self::ZERO
        }
    }

    impl fmt::Debug for TailWord {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TailWord")
                .field("checksum", &self.checksum())
                .field("version", &self.version())
                .field("seq_tail", &self.seq_tail())
                .field("alt_idx_scale_hint", &self.alt_idx_scale_hint())
                .field("eco_action_bind", &self.eco_action_bind())
                .field("strategy_enable_mask", &self.strategy_enable_mask())
                .finish()
        }
    }
}

/// Computes the recovery threshold for a given trip value using the supplied scale.
#[must_use]
pub const fn recover_threshold(trip: u16, scale: RecoverScale) -> u16 {
    scale.apply(trip)
}

/// Validates that a publish snapshot is consistent with the odd→even commit protocol.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapsuleValidationError {
    /// The header version was not even.
    HeaderVersionNotEven,
    /// Header and tail versions diverged.
    VersionMismatch {
        /// Version observed in W0.
        header: u8,
        /// Version observed in W7.
        tail: u8,
    },
    /// Head and tail sequence counters diverged.
    SequenceMismatch {
        /// Sequence head (W0).
        head: u16,
        /// Sequence tail (W7).
        tail: u16,
    },
    /// The checksum did not match the words.
    ChecksumMismatch {
        /// Checksum recomputed from W0..W6.
        expected: u16,
        /// Checksum stored in W7.
        observed: u16,
    },
}

impl core::fmt::Display for CapsuleValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderVersionNotEven => write!(f, "header version byte must be even"),
            Self::VersionMismatch { header, tail } => {
                write!(f, "header version {header} != tail version {tail}")
            }
            Self::SequenceMismatch { head, tail } => {
                write!(f, "seq_head {head} != seq_tail {tail}")
            }
            Self::ChecksumMismatch { expected, observed } => {
                write!(
                    f,
                    "checksum expected {expected:#06x}, observed {observed:#06x}"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CapsuleValidationError {}

/// Validates header/tail coherence and checksum.
pub fn validate_snapshot(table: &crate::Rlt1024) -> Result<(), CapsuleValidationError> {
    let header = table.header;
    let tail = table.tail;

    let header_version = header.version_even();
    if header_version & 1 != 0 {
        return Err(CapsuleValidationError::HeaderVersionNotEven);
    }

    let tail_version = tail.version();
    if header_version != tail_version {
        return Err(CapsuleValidationError::VersionMismatch {
            header: header_version,
            tail: tail_version,
        });
    }

    let seq_head = header.seq_head();
    let seq_tail = tail.seq_tail();
    if seq_head != seq_tail {
        return Err(CapsuleValidationError::SequenceMismatch {
            head: seq_head,
            tail: seq_tail,
        });
    }

    let expected = table.checksum16();
    let observed = tail.checksum();
    if expected != observed {
        return Err(CapsuleValidationError::ChecksumMismatch { expected, observed });
    }

    Ok(())
}
