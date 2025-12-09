#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::must_use_candidate)]
#![doc = "RLT-1024 risk ladder table primitive."]
#![doc = ""]
#![doc = "Provides a lock-free policy capsule for breaker/strategy/router coordination"]
#![doc = "across levels L0-L3 using a single, atomic 1024-bit publish."]

use core::fmt;

/// Bit layout helpers and field definitions.
pub mod layout;

pub use layout::{
    actions::RoutePolicy,
    actions::{ActionBases, ActionWord, ActionsWordDraft, AppliedActionSet},
    header::{HeaderWord, StrategyMask},
    tail::TailWord,
    trips::{TripThresholds, TripWord},
    FieldSpec, RecoverScale,
};

/// Number of 128-bit words in the capsule.
pub const WORD_COUNT: usize = 8;

/// Default recover scale (0.70 in Q1.7).
pub const DEFAULT_RECOVER_SCALE_Q1_7: RecoverScale = RecoverScale::new(0b0100_0110);

/// Container for the full RLT-1024 capsule.
#[repr(C, align(64))]
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Rlt1024 {
    /// Word 0 - header/global policy fields.
    pub header: HeaderWord,
    /// Word 1 - strategy A trip thresholds.
    pub strat_a_trips: TripWord,
    /// Word 2 - strategy A actions.
    pub strat_a_actions: ActionWord,
    /// Word 3 - strategy B trip thresholds.
    pub strat_b_trips: TripWord,
    /// Word 4 - strategy B actions.
    pub strat_b_actions: ActionWord,
    /// Word 5 - strategy C trip thresholds.
    pub strat_c_trips: TripWord,
    /// Word 6 - strategy C actions.
    pub strat_c_actions: ActionWord,
    /// Word 7 - policy tail / integrity word.
    pub tail: TailWord,
}

impl Rlt1024 {
    /// Returns a zero-initialised capsule ready for authoring.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            header: HeaderWord::ZERO,
            strat_a_trips: TripWord::ZERO,
            strat_a_actions: ActionWord::ZERO,
            strat_b_trips: TripWord::ZERO,
            strat_b_actions: ActionWord::ZERO,
            strat_c_trips: TripWord::ZERO,
            strat_c_actions: ActionWord::ZERO,
            tail: TailWord::ZERO,
        }
    }

    /// Overwrites the capsule with the provided words.
    ///
    /// The ordering is expected to follow `[W0, W1, ..., W7]`.
    #[must_use] 
    pub fn from_words(words: [u128; WORD_COUNT]) -> Self {
        Self {
            header: HeaderWord::from_raw(words[0]),
            strat_a_trips: TripWord::from_raw(words[1]),
            strat_a_actions: ActionWord::from_raw(words[2]),
            strat_b_trips: TripWord::from_raw(words[3]),
            strat_b_actions: ActionWord::from_raw(words[4]),
            strat_c_trips: TripWord::from_raw(words[5]),
            strat_c_actions: ActionWord::from_raw(words[6]),
            tail: TailWord::from_raw(words[7]),
        }
    }

    /// Returns the underlying words in write order.
    #[must_use]
    pub fn into_words(self) -> [u128; WORD_COUNT] {
        [
            self.header.raw(),
            self.strat_a_trips.raw(),
            self.strat_a_actions.raw(),
            self.strat_b_trips.raw(),
            self.strat_b_actions.raw(),
            self.strat_c_trips.raw(),
            self.strat_c_actions.raw(),
            self.tail.raw(),
        ]
    }

    /// Computes a 16-bit checksum across the first seven words.
    ///
    /// The checksum is a simple Fletcher-like fold that maintains determinism
    /// while avoiding a dependency on the standard library.
    #[must_use]
    pub fn checksum16(&self) -> u16 {
        let mut sum1: u32 = 0;
        let mut sum2: u32 = 0;
        let words = self.into_words();
        for word in words.iter().take(WORD_COUNT - 1) {
            let lower = *word as u64;
            let upper = (word >> 64) as u64;
            for chunk in [lower, upper] {
                let low = (chunk & 0xFFFF) as u32;
                let mid = ((chunk >> 16) & 0xFFFF) as u32;
                let hi = ((chunk >> 32) & 0xFFFF) as u32;
                let top = ((chunk >> 48) & 0xFFFF) as u32;
                sum1 = (sum1 + low) & 0xFFFF;
                sum2 = (sum2 + sum1) & 0xFFFF;
                sum1 = (sum1 + mid) & 0xFFFF;
                sum2 = (sum2 + sum1) & 0xFFFF;
                sum1 = (sum1 + hi) & 0xFFFF;
                sum2 = (sum2 + sum1) & 0xFFFF;
                sum1 = (sum1 + top) & 0xFFFF;
                sum2 = (sum2 + sum1) & 0xFFFF;
            }
        }
        ((sum2 << 8) | (sum1 & 0xFF)) as u16
    }
}

impl fmt::Debug for Rlt1024 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rlt1024")
            .field("header", &self.header)
            .field("strat_a_trips", &self.strat_a_trips)
            .field("strat_a_actions", &self.strat_a_actions)
            .field("strat_b_trips", &self.strat_b_trips)
            .field("strat_b_actions", &self.strat_b_actions)
            .field("strat_c_trips", &self.strat_c_trips)
            .field("strat_c_actions", &self.strat_c_actions)
            .field("tail", &self.tail)
            .finish()
    }
}

#[cfg(test)]
mod tests;
