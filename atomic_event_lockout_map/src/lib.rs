#![no_std]

//! ECO-1024 (Event Lockout Map) publishes trading session gates, event windows,
//! and breaker guidance in a single eight-word atomic capsule. Writers stage the
//! 512-bit bitmap plus metadata, compute the integrity tail, and commit with a
//! single release-store on W0. Readers obtain a stable snapshot via relaxed
//! loads, validate head/tail parity, then answer "Is this minute safe to trade?"
//! with one bitmap probe.

use core::sync::atomic::Ordering;

use bitflags::bitflags;
use portable_atomic::AtomicU128;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(test)]
extern crate std;

const WORDS: usize = 8;
const BITMAP_WORDS: usize = 4; // W1..W4
const BITMAP_BITS: usize = 512;
const MAX_EVENTS: usize = 8;
const MAX_BASELINE_WINDOWS: usize = 8;
const MINUTES_PER_DAY: u16 = 1_440;
const INVALID_MINUTE: u16 = 0x7ff; // 11-bit all-ones sentinel

/// ECO-1024 capsule aligned to a 64-byte boundary with eight 128-bit lanes.
#[repr(C, align(64))]
pub struct Eco1024 {
    words: [AtomicU128; WORDS],
}

impl Eco1024 {
    /// Creates a new zeroed capsule.
    pub const fn new() -> Self {
        Self {
            words: [
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
            ],
        }
    }

    /// Publishes the staged draft following the odd→even contract.
    pub fn publish(&self, draft: &EcoSnapshotDraft) {
        for (lane, value) in self.words.iter().zip(draft.words.iter()).skip(1) {
            lane.store(*value, Ordering::Relaxed);
        }
        self.words[0].store(draft.words[0], Ordering::Release);
    }

    /// Loads a snapshot with relaxed ordering and validates integrity bits.
    pub fn load_relaxed(&self) -> Option<EcoSnapshot> {
        let head_bits = self.words[0].load(Ordering::Relaxed);
        let head = HeadWord::from_bits(head_bits);
        if !head.commit || head.ver_even % 2 == 1 {
            return None;
        }

        let mut lanes = [0u128; WORDS - 1];
        for (dst, atomic) in lanes.iter_mut().zip(self.words.iter().skip(1)) {
            *dst = atomic.load(Ordering::Relaxed);
        }

        let tail = TailWord::from_bits(lanes[WORDS - 2]);
        if tail.ver_tail != head.ver_even {
            return None;
        }

        let checksum = checksum16(head_bits, &lanes[..lanes.len() - 1]);
        if checksum != tail.checksum16 {
            return None;
        }

        let mut words = [0u128; WORDS];
        words[0] = head_bits;
        for (idx, value) in lanes.into_iter().enumerate() {
            words[idx + 1] = value;
        }
        Some(EcoSnapshot { words })
    }
}

impl Default for Eco1024 {
    fn default() -> Self {
        Self::new()
    }
}

/// Writer-facing draft that stores the staged 1024 bits prior to commit.
#[derive(Clone, Debug, Default)]
pub struct EcoSnapshotDraft {
    words: [u128; WORDS],
}

impl EcoSnapshotDraft {
    /// Returns raw words for inspection or atomic CAS commit.
    pub const fn words(&self) -> &[u128; WORDS] {
        &self.words
    }

    fn set_head(&mut self, head: &HeadWord) {
        self.words[0] = head.encode();
    }

    fn set_bitmap_word(&mut self, lane: usize, bits: u128) {
        self.words[1 + lane] = bits;
    }

    fn set_event_word(&mut self, idx: usize, bits: u128) {
        debug_assert!(idx < 2);
        self.words[5 + idx] = bits;
    }

    fn set_tail(&mut self, tail: &TailWord) {
        self.words[7] = tail.encode();
    }

    fn bake_checksum(&mut self) {
        let checksum = checksum16(self.words[0], &self.words[1..7]);
        self.words[7] &= !0xffff;
        self.words[7] |= checksum as u128;
    }
}

/// Immutable reader snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EcoSnapshot {
    words: [u128; WORDS],
}

impl EcoSnapshot {
    /// Returns decoded head word.
    pub fn head(&self) -> HeadWord {
        HeadWord::from_bits(self.words[0])
    }

    /// Returns decoded tail word.
    pub fn tail(&self) -> TailWord {
        TailWord::from_bits(self.words[7])
    }

    /// Returns view of the 512-bit minute bitmap.
    pub fn bitmap(&self) -> MinuteBitmap {
        MinuteBitmap {
            lanes: [self.words[1], self.words[2], self.words[3], self.words[4]],
        }
    }

    /// Iterates over populated event slots.
    pub fn events(&self) -> EventIter {
        EventIter {
            raw: [self.words[5], self.words[6]],
            index: 0,
        }
    }

    /// Returns true when the current minute is allowed to open new risk.
    pub fn is_allowed_now(&self) -> bool {
        let head = self.head();
        let tail = self.tail();
        if !head.commit || head.stale {
            return false;
        }
        if tail.now_min_ct == INVALID_MINUTE {
            return false;
        }

        if tail.now_min_ct >= head.forbid_after_min_ct || tail.now_min_ct >= head.eod_flat_min_ct {
            return false;
        }

        if let Some(offset) =
            offset_from_origin(head.origin_min_ct, tail.now_min_ct, head.mask_len_min)
        {
            self.bitmap().bit(offset)
        } else {
            false
        }
    }

    /// Returns next lockout minute-of-day if provided.
    pub fn next_lockout_minute(&self) -> Option<u16> {
        let tail = self.tail();
        if tail.next_lockout_min_ct == INVALID_MINUTE {
            None
        } else {
            Some(tail.next_lockout_min_ct)
        }
    }

    /// Returns next resume minute-of-day if provided.
    pub fn next_resume_minute(&self) -> Option<u16> {
        let tail = self.tail();
        if tail.next_resume_min_ct == INVALID_MINUTE {
            None
        } else {
            Some(tail.next_resume_min_ct)
        }
    }
}

/// Head word fields (W0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeadWord {
    pub commit: bool,
    pub stale: bool,
    pub ver_even: u8,
    pub seq_head: u16,
    pub account_id: u16,
    pub tz_id: u8,
    pub origin_min_ct: u16,
    pub mask_len_min: u16,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub global_flags: GlobalFlag,
    pub created_ms_coarse: u32,
}

impl HeadWord {
    fn encode(&self) -> u128 {
        let mut bits = 0u128;
        bits |= self.commit as u128;
        bits |= (self.stale as u128) << 1;
        bits |= (self.ver_even as u128) << 2;
        bits |= (self.seq_head as u128) << 10;
        bits |= (self.account_id as u128) << 26;
        bits |= (self.tz_id as u128) << 42;
        bits |= (self.origin_min_ct as u128) << 50;
        bits |= ((self.mask_len_min.saturating_sub(1)) as u128) << 61;
        bits |= (self.forbid_after_min_ct as u128) << 70;
        bits |= (self.eod_flat_min_ct as u128) << 81;
        bits |= (self.global_flags.bits() as u128) << 92;
        bits |= (self.created_ms_coarse as u128) << 104;
        bits
    }

    fn from_bits(bits: u128) -> Self {
        Self {
            commit: (bits & 0x1) != 0,
            stale: ((bits >> 1) & 0x1) != 0,
            ver_even: ((bits >> 2) & 0xff) as u8,
            seq_head: ((bits >> 10) & 0xffff) as u16,
            account_id: ((bits >> 26) & 0xffff) as u16,
            tz_id: ((bits >> 42) & 0xff) as u8,
            origin_min_ct: ((bits >> 50) & 0x7ff) as u16,
            mask_len_min: (((bits >> 61) & 0x1ff) as u16) + 1,
            forbid_after_min_ct: ((bits >> 70) & 0x7ff) as u16,
            eod_flat_min_ct: ((bits >> 81) & 0x7ff) as u16,
            global_flags: GlobalFlag::from_bits_truncate(((bits >> 92) & 0xfff) as u16),
            created_ms_coarse: ((bits >> 104) & 0xffffff) as u32,
        }
    }
}

/// Tail word fields (W7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TailWord {
    pub checksum16: u16,
    pub ver_tail: u8,
    pub now_min_ct: u16,
    pub age_8ms: u16,
    pub next_lockout_min_ct: u16,
    pub next_resume_min_ct: u16,
    pub active_severity: EventSeverity,
    pub active_action: EventAction,
    pub day_of_week: u8,
    pub holiday_flag: bool,
}

impl TailWord {
    fn encode(&self) -> u128 {
        let mut bits = 0u128;
        bits |= self.checksum16 as u128;
        bits |= (self.ver_tail as u128) << 16;
        bits |= (self.now_min_ct as u128) << 24;
        bits |= (self.age_8ms as u128) << 35;
        bits |= (self.next_lockout_min_ct as u128) << 47;
        bits |= (self.next_resume_min_ct as u128) << 58;
        bits |= (self.active_severity.bits() as u128) << 69;
        bits |= (self.active_action.bits() as u128) << 71;
        bits |= (self.day_of_week as u128) << 73;
        bits |= (self.holiday_flag as u128) << 76;
        bits
    }

    fn from_bits(bits: u128) -> Self {
        Self {
            checksum16: (bits & 0xffff) as u16,
            ver_tail: ((bits >> 16) & 0xff) as u8,
            now_min_ct: ((bits >> 24) & 0x7ff) as u16,
            age_8ms: ((bits >> 35) & 0xfff) as u16,
            next_lockout_min_ct: ((bits >> 47) & 0x7ff) as u16,
            next_resume_min_ct: ((bits >> 58) & 0x7ff) as u16,
            active_severity: EventSeverity::from_bits(((bits >> 69) & 0x3) as u8),
            active_action: EventAction::from_bits(((bits >> 71) & 0x3) as u8),
            day_of_week: ((bits >> 73) & 0x7) as u8,
            holiday_flag: ((bits >> 76) & 0x1) != 0,
        }
    }
}

/// Reader-facing bitmap view.
pub struct MinuteBitmap {
    lanes: [u128; BITMAP_WORDS],
}

impl MinuteBitmap {
    /// Returns whether the minute offset is allowed.
    pub fn bit(&self, offset: u16) -> bool {
        if offset as usize >= BITMAP_BITS {
            return false;
        }
        let lane = offset as usize / 128;
        let bit = offset as usize % 128;
        (self.lanes[lane] >> bit) & 1 == 1
    }

    /// Finds the first transition 1→0 strictly after `offset`.
    pub fn next_lockout_from(&self, offset: u16) -> Option<u16> {
        if offset as usize >= BITMAP_BITS {
            return None;
        }
        let mut prev = self.bit(offset);
        for idx in (offset as usize + 1)..BITMAP_BITS {
            let state = self.bit(idx as u16);
            if prev && !state {
                return Some(idx as u16);
            }
            prev = state;
        }
        None
    }

    /// Finds the first transition 0→1 strictly after `offset`.
    pub fn next_resume_from(&self, offset: u16) -> Option<u16> {
        if offset as usize >= BITMAP_BITS {
            return None;
        }
        let mut prev = self.bit(offset);
        for idx in (offset as usize + 1)..BITMAP_BITS {
            let state = self.bit(idx as u16);
            if !prev && state {
                return Some(idx as u16);
            }
            prev = state;
        }
        None
    }
}

/// Event slot published in W5/W6 (32 bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventSlot {
    pub start_min_ct: u16,
    pub end_min_ct: u16,
    pub severity: EventSeverity,
    pub action: EventAction,
    pub sym_mask: u8,
    pub kind: EventKind,
}

impl EventSlot {
    const fn empty() -> Self {
        Self {
            start_min_ct: 0,
            end_min_ct: 0,
            severity: EventSeverity::Low,
            action: EventAction::None,
            sym_mask: 0,
            kind: EventKind::Econ,
        }
    }

    fn encode(&self) -> u32 {
        ((self.start_min_ct as u32) & 0x7ff)
            | (((self.end_min_ct as u32) & 0x7ff) << 11)
            | ((self.severity.bits() as u32) << 22)
            | ((self.action.bits() as u32) << 24)
            | (((self.sym_mask & 0x0f) as u32) << 26)
            | ((self.kind.bits() as u32) << 30)
    }

    fn from_bits(bits: u32) -> Self {
        let kind = match (bits >> 30) & 0x3 {
            0 => EventKind::Econ,
            1 => EventKind::Maintenance,
            2 => EventKind::Session,
            _ => EventKind::Other,
        };
        Self {
            start_min_ct: (bits & 0x7ff) as u16,
            end_min_ct: ((bits >> 11) & 0x7ff) as u16,
            severity: EventSeverity::from_bits(((bits >> 22) & 0x3) as u8),
            action: EventAction::from_bits(((bits >> 24) & 0x3) as u8),
            sym_mask: ((bits >> 26) & 0x0f) as u8,
            kind,
        }
    }

    fn is_empty(&self) -> bool {
        *self == Self::empty()
    }
}

/// Iterator over non-empty event slots.
pub struct EventIter {
    raw: [u128; 2],
    index: usize,
}

impl Iterator for EventIter {
    type Item = EventSlot;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < MAX_EVENTS {
            let lane = self.index / 4;
            let slot = self.index % 4;
            let bits = (self.raw[lane] >> (slot * 32)) as u32;
            self.index += 1;
            let decoded = EventSlot::from_bits(bits);
            if decoded.is_empty() {
                continue;
            }
            return Some(decoded);
        }
        None
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub struct GlobalFlag: u16 {
        const ALLOWED_NOW = 0b0000_0000_0001;
        const PAUSED      = 0b0000_0000_0010;
        const NEWS_LOCKOUT= 0b0000_0000_0100;
        const REDUCE_ONLY = 0b0000_0000_1000;
        const AT_EOD      = 0b0000_0001_0000;
        const MANUAL      = 0b0000_0010_0000;
        const SESSION_OFF = 0b0000_0100_0000;
    }
}

/// Breaker action recommendation carried in event windows and tail.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventAction {
    None = 0,
    Degrade = 1,
    ForbidNew = 2,
    Lock = 3,
}

impl EventAction {
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0x3 {
            0 => EventAction::None,
            1 => EventAction::Degrade,
            2 => EventAction::ForbidNew,
            _ => EventAction::Lock,
        }
    }

    pub fn bits(self) -> u8 {
        self as u8
    }

    /// Returns true when the action severity meets or exceeds `other`.
    pub fn at_least(self, other: EventAction) -> bool {
        self >= other
    }
}

/// Event severity enumerations (2-bit lane).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventSeverity {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

impl EventSeverity {
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0x3 {
            0 => EventSeverity::Low,
            1 => EventSeverity::Medium,
            2 => EventSeverity::High,
            _ => EventSeverity::Critical,
        }
    }

    pub fn bits(self) -> u8 {
        self as u8
    }
}

/// Event kind (2-bit lane).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Econ = 0,
    Maintenance = 1,
    Session = 2,
    Other = 3,
}

impl EventKind {
    fn bits(self) -> u8 {
        self as u8
    }
}

/// Writer event input accepted by the aggregator.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventWindow {
    pub start_min_ct: u16,
    pub end_min_ct: u16,
    pub severity: EventSeverity,
    pub action: EventAction,
    pub sym_mask: u8,
    pub kind: EventKind,
}

impl EventWindow {
    /// Creates a new window covering `[start, end)` minutes (wrapping allowed).
    pub fn new(
        start_min_ct: u16,
        end_min_ct: u16,
        severity: EventSeverity,
        action: EventAction,
        sym_mask: u8,
        kind: EventKind,
    ) -> Self {
        Self {
            start_min_ct: start_min_ct % MINUTES_PER_DAY,
            end_min_ct: end_min_ct % MINUTES_PER_DAY,
            severity,
            action,
            sym_mask: sym_mask & 0x0f,
            kind,
        }
    }

    /// Helper for news/economic data windows.
    pub fn econ(
        start_min_ct: u16,
        end_min_ct: u16,
        severity: EventSeverity,
        action: EventAction,
    ) -> Self {
        Self::new(
            start_min_ct,
            end_min_ct,
            severity,
            action,
            0,
            EventKind::Econ,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountScope {
    pub account_id: u16,
    pub tz_id: u8,
}

impl AccountScope {
    pub const fn new(account_id: u16, tz_id: u8) -> Self {
        Self { account_id, tz_id }
    }
}

/// Request parameters for building a snapshot.
pub struct BuildRequest<'a> {
    pub now_min_ct: u16,
    pub age_8ms: u16,
    pub created_ms_coarse: u32,
    pub events: &'a [EventWindow],
    pub global_flags: GlobalFlag,
    pub manual_pause: bool,
    pub day_of_week: u8,
    pub holiday_flag: bool,
}

/// Session clamps that may vary intraday.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionClamps {
    pub forbid_after_min_ct: Option<u16>,
    pub eod_flat_min_ct: Option<u16>,
}

impl SessionClamps {
    pub const fn new(forbid_after_min_ct: Option<u16>, eod_flat_min_ct: Option<u16>) -> Self {
        Self {
            forbid_after_min_ct,
            eod_flat_min_ct,
        }
    }
}

/// Configuration for integrating `EcoWriter` with live feeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublisherConfig {
    pub account: AccountScope,
    pub origin_min_ct: u16,
    pub mask_len_min: u16,
    pub session_clamps: SessionClamps,
}

impl PublisherConfig {
    pub const fn new(
        account: AccountScope,
        origin_min_ct: u16,
        mask_len_min: u16,
        session_clamps: SessionClamps,
    ) -> Self {
        Self {
            account,
            origin_min_ct,
            mask_len_min,
            session_clamps,
        }
    }
}

/// Inputs collected from calendars, ops notices, and manual overrides.
pub struct SnapshotInputs<'a> {
    pub baseline_windows: &'a [MinuteRange],
    pub events: &'a [EventWindow],
    pub now_min_ct: u16,
    pub age_8ms: u16,
    pub created_ms_coarse: u32,
    pub global_flags: GlobalFlag,
    pub manual_pause: bool,
    pub session_clamps: SessionClamps,
    pub day_of_week: u8,
    pub holiday_flag: bool,
}

/// Difference between flag states, useful for logging/telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlagDiff {
    pub set: GlobalFlag,
    pub cleared: GlobalFlag,
}

impl FlagDiff {
    pub fn compute(prev: GlobalFlag, next: GlobalFlag) -> Self {
        Self {
            set: next & !prev,
            cleared: prev & !next,
        }
    }

    pub fn has_changes(&self) -> bool {
        !(self.set.is_empty() && self.cleared.is_empty())
    }
}

/// Outcome of publishing a new snapshot, including flag deltas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublishOutcome {
    pub snapshot: EcoSnapshot,
    pub flag_diff: FlagDiff,
}

/// High-level publisher that hydrates `EcoWriter` from feed inputs.
pub struct EcoPublisher {
    writer: EcoWriter,
}

impl EcoPublisher {
    pub fn new(config: PublisherConfig) -> Self {
        let writer = EcoWriter::new(config.account)
            .with_origin_minute(config.origin_min_ct)
            .with_mask_length(config.mask_len_min);
        let mut this = Self { writer };
        this.writer.configure_session_clamps(config.session_clamps);
        this
    }

    pub fn slot(&self) -> &Eco1024 {
        self.writer.slot()
    }

    pub fn publish(&mut self, inputs: SnapshotInputs<'_>) -> PublishOutcome {
        let prev_flags = self
            .writer
            .slot()
            .load_relaxed()
            .map(|snap| snap.head().global_flags)
            .unwrap_or_else(GlobalFlag::empty);

        self.writer.configure_session_clamps(inputs.session_clamps);
        self.writer.set_baseline_windows(inputs.baseline_windows);

        let snapshot = self.writer.build_and_publish(BuildRequest {
            now_min_ct: inputs.now_min_ct,
            age_8ms: inputs.age_8ms,
            created_ms_coarse: inputs.created_ms_coarse,
            events: inputs.events,
            global_flags: inputs.global_flags,
            manual_pause: inputs.manual_pause,
            day_of_week: inputs.day_of_week,
            holiday_flag: inputs.holiday_flag,
        });

        let diff = FlagDiff::compute(prev_flags, snapshot.head().global_flags);
        PublishOutcome {
            snapshot,
            flag_diff: diff,
        }
    }
}

/// Aggregator responsible for building ECO-1024 snapshots.
pub struct EcoWriter {
    slot: Eco1024,
    account: AccountScope,
    origin_min_ct: u16,
    mask_len_min: u16,
    forbid_after_min_ct: u16,
    eod_flat_min_ct: u16,
    baseline: [Option<MinuteRange>; MAX_BASELINE_WINDOWS],
    seq_head: u16,
    version_even: u8,
}

impl EcoWriter {
    pub fn new(account: AccountScope) -> Self {
        Self {
            slot: Eco1024::new(),
            account,
            origin_min_ct: 0,
            mask_len_min: BITMAP_BITS as u16,
            forbid_after_min_ct: INVALID_MINUTE,
            eod_flat_min_ct: INVALID_MINUTE,
            baseline: [None; MAX_BASELINE_WINDOWS],
            seq_head: 0,
            version_even: 0,
        }
    }

    /// Returns the capsule slot used for publishing.
    pub fn slot(&self) -> &Eco1024 {
        &self.slot
    }

    /// Sets the bitmap anchor minute.
    pub fn with_origin_minute(mut self, origin_min_ct: u16) -> Self {
        self.origin_min_ct = origin_min_ct % MINUTES_PER_DAY;
        self
    }

    /// Sets the number of minutes covered by the bitmap (1..=512).
    pub fn with_mask_length(mut self, mask_len_min: u16) -> Self {
        self.mask_len_min = mask_len_min.clamp(1, BITMAP_BITS as u16);
        self
    }

    /// Configures the forbid-after and EOD-flat clamps.
    pub fn with_session_clamps(mut self, forbid_after_min_ct: u16, eod_flat_min_ct: u16) -> Self {
        self.configure_session_clamps(SessionClamps::new(
            Some(forbid_after_min_ct),
            Some(eod_flat_min_ct),
        ));
        self
    }

    /// Adds a baseline trading window `[start, end)` (wrapping permitted).
    pub fn with_baseline_window(mut self, start_min_ct: u16, end_min_ct: u16) -> Self {
        let window = MinuteRange::new(start_min_ct, end_min_ct);
        for slot in &mut self.baseline {
            if slot.is_none() {
                *slot = Some(window);
                break;
            }
        }
        self
    }

    /// Applies session clamps (optional) in-place.
    pub fn configure_session_clamps(&mut self, clamps: SessionClamps) {
        self.forbid_after_min_ct = clamps
            .forbid_after_min_ct
            .map(|m| m % MINUTES_PER_DAY)
            .unwrap_or(INVALID_MINUTE);
        self.eod_flat_min_ct = clamps
            .eod_flat_min_ct
            .map(|m| m % MINUTES_PER_DAY)
            .unwrap_or(INVALID_MINUTE);
    }

    /// Replaces baseline windows with the provided slice (truncated to capacity).
    pub fn set_baseline_windows(&mut self, windows: &[MinuteRange]) {
        self.baseline = [None; MAX_BASELINE_WINDOWS];
        for (slot, window) in self.baseline.iter_mut().zip(windows.iter()) {
            *slot = Some(*window);
        }
    }

    fn next_versions(&mut self) {
        self.seq_head = self.seq_head.wrapping_add(1);
        self.version_even = self.version_even.wrapping_add(2);
        if self.version_even % 2 == 1 {
            self.version_even = self.version_even.wrapping_add(1);
        }
    }

    /// Builds a snapshot draft per the ECO-1024 contract.
    pub fn build(&mut self, request: BuildRequest<'_>) -> EcoSnapshotDraft {
        self.next_versions();

        let mut draft = EcoSnapshotDraft::default();

        let mut bitmap = BitmapDraft::new(self.origin_min_ct, self.mask_len_min);
        for &window in self.baseline.iter().flatten() {
            bitmap.allow_window(window);
        }

        let mut global_flags = request.global_flags;
        if request.manual_pause {
            bitmap.clear();
            global_flags.insert(GlobalFlag::PAUSED | GlobalFlag::MANUAL);
        }

        let mut event_slots = [EventSlot::empty(); MAX_EVENTS];
        let mut active_action = EventAction::None;
        let mut active_severity = EventSeverity::Low;

        for (idx, window) in request.events.iter().take(MAX_EVENTS).enumerate() {
            let slot = EventSlot {
                start_min_ct: window.start_min_ct,
                end_min_ct: window.end_min_ct,
                severity: window.severity,
                action: window.action,
                sym_mask: window.sym_mask,
                kind: window.kind,
            };
            event_slots[idx] = slot;

            if window.action.at_least(EventAction::ForbidNew) {
                bitmap.forbid_window(MinuteRange::new(window.start_min_ct, window.end_min_ct));
            }

            if minute_in_window(request.now_min_ct, window.start_min_ct, window.end_min_ct) {
                if window.action > active_action {
                    active_action = window.action;
                }
                if window.severity > active_severity {
                    active_severity = window.severity;
                }
            }
        }

        if active_action.at_least(EventAction::ForbidNew) {
            global_flags.insert(GlobalFlag::NEWS_LOCKOUT | GlobalFlag::REDUCE_ONLY);
        } else if active_action == EventAction::Degrade {
            global_flags.insert(GlobalFlag::REDUCE_ONLY);
        } else {
            global_flags.remove(GlobalFlag::REDUCE_ONLY | GlobalFlag::NEWS_LOCKOUT);
        }

        if request.manual_pause {
            active_action = EventAction::Lock;
            active_severity = EventSeverity::Critical;
        }

        let now_offset =
            offset_from_origin(self.origin_min_ct, request.now_min_ct, self.mask_len_min);
        let allowed_bit = now_offset.map(|off| bitmap.bit(off)).unwrap_or(false);
        let before_forbid = self.forbid_after_min_ct == INVALID_MINUTE
            || request.now_min_ct < self.forbid_after_min_ct;
        let before_eod_flat =
            self.eod_flat_min_ct == INVALID_MINUTE || request.now_min_ct < self.eod_flat_min_ct;

        if allowed_bit && before_forbid && before_eod_flat {
            global_flags.insert(GlobalFlag::ALLOWED_NOW);
        } else {
            global_flags.remove(GlobalFlag::ALLOWED_NOW);
        }

        if (self.forbid_after_min_ct != INVALID_MINUTE
            && request.now_min_ct >= self.forbid_after_min_ct)
            || (self.eod_flat_min_ct != INVALID_MINUTE
                && request.now_min_ct >= self.eod_flat_min_ct)
        {
            global_flags.insert(GlobalFlag::AT_EOD);
        } else {
            global_flags.remove(GlobalFlag::AT_EOD);
        }

        // W1..W4
        for (idx, lane) in bitmap.lanes.iter().enumerate() {
            draft.set_bitmap_word(idx, *lane);
        }

        // W5..W6 (events)
        draft.set_event_word(0, pack_events(&event_slots[0..4]));
        draft.set_event_word(1, pack_events(&event_slots[4..8]));

        // Head (W0)
        let head = HeadWord {
            commit: true,
            stale: request.manual_pause,
            ver_even: self.version_even,
            seq_head: self.seq_head,
            account_id: self.account.account_id,
            tz_id: self.account.tz_id,
            origin_min_ct: self.origin_min_ct,
            mask_len_min: self.mask_len_min,
            forbid_after_min_ct: self.forbid_after_min_ct,
            eod_flat_min_ct: self.eod_flat_min_ct,
            global_flags,
            created_ms_coarse: request.created_ms_coarse & 0x00ff_ffff,
        };
        draft.set_head(&head);

        let lockout_offset = now_offset
            .and_then(|off| bitmap.next_lockout_from(off))
            .map(|off| bitmap.absolute_minute(off))
            .unwrap_or(INVALID_MINUTE);
        let resume_offset = now_offset
            .and_then(|off| bitmap.next_resume_from(off))
            .map(|off| bitmap.absolute_minute(off))
            .unwrap_or(INVALID_MINUTE);

        // Tail (W7)
        let tail = TailWord {
            checksum16: 0,
            ver_tail: self.version_even,
            now_min_ct: request.now_min_ct % MINUTES_PER_DAY,
            age_8ms: request.age_8ms & 0x0fff,
            next_lockout_min_ct: lockout_offset,
            next_resume_min_ct: resume_offset,
            active_severity,
            active_action,
            day_of_week: request.day_of_week.min(6),
            holiday_flag: request.holiday_flag,
        };
        draft.set_tail(&tail);

        // Final checksum
        draft.bake_checksum();

        draft
    }

    /// Publishes a freshly built snapshot in one step.
    pub fn build_and_publish(&mut self, request: BuildRequest<'_>) -> EcoSnapshot {
        let draft = self.build(request);
        self.slot.publish(&draft);
        self.slot.load_relaxed().expect("fresh snapshot must load")
    }
}

/// Inclusive-exclusive minute window (start inclusive, end exclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinuteRange {
    pub start: u16,
    pub end: u16,
}

impl MinuteRange {
    pub fn new(start: u16, end: u16) -> Self {
        Self {
            start: start % MINUTES_PER_DAY,
            end: end % MINUTES_PER_DAY,
        }
    }
}

/// Writer-side bitmap draft.
struct BitmapDraft {
    origin_min_ct: u16,
    mask_len_min: u16,
    lanes: [u128; BITMAP_WORDS],
}

impl BitmapDraft {
    fn new(origin_min_ct: u16, mask_len_min: u16) -> Self {
        Self {
            origin_min_ct,
            mask_len_min,
            lanes: [0; BITMAP_WORDS],
        }
    }

    fn clear(&mut self) {
        self.lanes = [0; BITMAP_WORDS];
    }

    fn allow_window(&mut self, window: MinuteRange) {
        iterate_minutes(window.start, window.end, |minute| {
            if let Some(offset) = offset_from_origin(self.origin_min_ct, minute, self.mask_len_min)
            {
                self.set_bit(offset, true);
            }
        });
    }

    fn forbid_window(&mut self, window: MinuteRange) {
        iterate_minutes(window.start, window.end, |minute| {
            if let Some(offset) = offset_from_origin(self.origin_min_ct, minute, self.mask_len_min)
            {
                self.set_bit(offset, false);
            }
        });
    }

    fn bit(&self, offset: u16) -> bool {
        if offset as usize >= BITMAP_BITS {
            return false;
        }
        let lane = offset as usize / 128;
        let bit = offset as usize % 128;
        (self.lanes[lane] >> bit) & 1 == 1
    }

    fn set_bit(&mut self, offset: u16, value: bool) {
        if offset as usize >= BITMAP_BITS {
            return;
        }
        let lane = offset as usize / 128;
        let bit = offset as usize % 128;
        if value {
            self.lanes[lane] |= 1u128 << bit;
        } else {
            self.lanes[lane] &= !(1u128 << bit);
        }
    }

    fn next_lockout_from(&self, offset: u16) -> Option<u16> {
        if offset as usize >= BITMAP_BITS {
            return None;
        }
        let mut prev = self.bit(offset);
        for idx in (offset as usize + 1)..BITMAP_BITS {
            let state = self.bit(idx as u16);
            if prev && !state {
                return Some(idx as u16);
            }
            prev = state;
        }
        None
    }

    fn next_resume_from(&self, offset: u16) -> Option<u16> {
        if offset as usize >= BITMAP_BITS {
            return None;
        }
        let mut prev = self.bit(offset);
        for idx in (offset as usize + 1)..BITMAP_BITS {
            let state = self.bit(idx as u16);
            if !prev && state {
                return Some(idx as u16);
            }
            prev = state;
        }
        None
    }

    fn absolute_minute(&self, offset: u16) -> u16 {
        (self.origin_min_ct + offset) % MINUTES_PER_DAY
    }
}

impl core::ops::Deref for BitmapDraft {
    type Target = [u128; BITMAP_WORDS];

    fn deref(&self) -> &Self::Target {
        &self.lanes
    }
}

fn pack_events(slots: &[EventSlot]) -> u128 {
    let mut word = 0u128;
    for (idx, slot) in slots.iter().enumerate() {
        word |= (slot.encode() as u128) << (idx * 32);
    }
    word
}

fn iterate_minutes<F>(start: u16, end: u16, mut f: F)
where
    F: FnMut(u16),
{
    if start == end {
        for minute in 0..MINUTES_PER_DAY {
            f(minute);
        }
        return;
    }

    let mut minute = start % MINUTES_PER_DAY;
    loop {
        f(minute);
        minute = (minute + 1) % MINUTES_PER_DAY;
        if minute == end % MINUTES_PER_DAY {
            break;
        }
    }
}

fn minute_in_window(minute: u16, start: u16, end: u16) -> bool {
    if start % MINUTES_PER_DAY == end % MINUTES_PER_DAY {
        return true;
    }
    if start <= end {
        minute >= start && minute < end
    } else {
        minute >= start || minute < end
    }
}

fn offset_from_origin(origin: u16, minute: u16, mask_len: u16) -> Option<u16> {
    let diff = (minute + MINUTES_PER_DAY - origin) % MINUTES_PER_DAY;
    if diff < mask_len {
        Some(diff)
    } else {
        None
    }
}

fn checksum16(head: u128, body: &[u128]) -> u16 {
    let mut sum = fold_u128(head);
    for word in body {
        sum = sum.wrapping_add(fold_u128(*word));
    }
    let hi = (sum >> 16) & 0xffff;
    let lo = sum & 0xffff;
    (hi + lo) as u16
}

fn fold_u128(word: u128) -> u32 {
    let mut tmp = word;
    let mut acc = 0u32;
    for _ in 0..8 {
        acc = acc.wrapping_add((tmp & 0xffff) as u32);
        tmp >>= 16;
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::Ordering;
    use proptest::prelude::*;

    #[test]
    fn event_slot_roundtrip() {
        let slot = EventSlot {
            start_min_ct: 450,
            end_min_ct: 455,
            severity: EventSeverity::High,
            action: EventAction::ForbidNew,
            sym_mask: 5,
            kind: EventKind::Maintenance,
        };
        let packed = slot.encode();
        let unpacked = EventSlot::from_bits(packed);
        assert_eq!(slot, unpacked);
    }

    #[test]
    fn bitmap_range_sets_bits() {
        let mut bitmap = BitmapDraft::new(480, 512);
        bitmap.allow_window(MinuteRange::new(510, 515));
        assert!(bitmap.bit(30)); // 480 + 30 = 510
        assert!(bitmap.bit(34));
        assert!(!bitmap.bit(35));
    }

    #[test]
    fn iterate_minutes_wraps() {
        let mut minutes = std::vec::Vec::new();
        iterate_minutes(1438, 2, |m| minutes.push(m));
        assert_eq!(minutes.len(), 4);
        assert_eq!(minutes, [1438, 1439, 0, 1]);
    }

    #[test]
    fn offset_from_origin_respects_mask() {
        assert_eq!(offset_from_origin(480, 500, 512), Some(20));
        assert_eq!(offset_from_origin(480, 480, 512), Some(0));
        assert_eq!(offset_from_origin(480, 480 + 600, 512), None);
    }

    #[test]
    fn minute_in_window_handles_wrap() {
        assert!(minute_in_window(5, 1430, 10));
        assert!(minute_in_window(1435, 1430, 10));
        assert!(!minute_in_window(20, 1430, 10));
    }

    #[test]
    fn writer_builds_baseline_bitmap() {
        let mut writer = EcoWriter::new(AccountScope::new(42, 5))
            .with_origin_minute(480)
            .with_mask_length(512)
            .with_session_clamps(905, 910)
            .with_baseline_window(510, 905);

        let draft = writer.build(BuildRequest {
            now_min_ct: 531,
            age_8ms: 12,
            created_ms_coarse: 12_345,
            events: &[],
            global_flags: GlobalFlag::empty(),
            manual_pause: false,
            day_of_week: 2,
            holiday_flag: false,
        });

        writer.slot().publish(&draft);
        let snapshot = writer.slot().load_relaxed().unwrap();
        let head = snapshot.head();
        let tail = snapshot.tail();

        assert_eq!(head.account_id, 42);
        assert!(head.global_flags.contains(GlobalFlag::ALLOWED_NOW));
        assert_eq!(tail.now_min_ct, 531);

        let offset = offset_from_origin(480, 531, 512).unwrap();
        assert!(snapshot.bitmap().bit(offset));
        assert!(snapshot.is_allowed_now());
    }

    #[test]
    fn lockout_event_punches_hole() {
        let mut writer = EcoWriter::new(AccountScope::new(7, 0))
            .with_origin_minute(480)
            .with_mask_length(512)
            .with_baseline_window(480, 600);

        let events = [EventWindow::econ(
            500,
            505,
            EventSeverity::High,
            EventAction::ForbidNew,
        )];

        let snapshot = writer.build_and_publish(BuildRequest {
            now_min_ct: 502,
            age_8ms: 8,
            created_ms_coarse: 9_999,
            events: &events,
            global_flags: GlobalFlag::empty(),
            manual_pause: false,
            day_of_week: 3,
            holiday_flag: false,
        });

        let offset = offset_from_origin(480, 502, 512).unwrap();
        assert!(!snapshot.bitmap().bit(offset));
        assert!(!snapshot.is_allowed_now());
        assert_eq!(snapshot.tail().active_action, EventAction::ForbidNew);
        assert!(snapshot
            .head()
            .global_flags
            .contains(GlobalFlag::NEWS_LOCKOUT | GlobalFlag::REDUCE_ONLY));
    }

    #[test]
    fn degrade_preserves_bit_and_sets_action() {
        let mut writer = EcoWriter::new(AccountScope::new(1, 0))
            .with_origin_minute(480)
            .with_mask_length(512)
            .with_baseline_window(500, 520);

        let events = [EventWindow::econ(
            505,
            510,
            EventSeverity::Medium,
            EventAction::Degrade,
        )];

        let snapshot = writer.build_and_publish(BuildRequest {
            now_min_ct: 506,
            age_8ms: 4,
            created_ms_coarse: 1_234,
            events: &events,
            global_flags: GlobalFlag::empty(),
            manual_pause: false,
            day_of_week: 1,
            holiday_flag: false,
        });

        let offset = offset_from_origin(480, 506, 512).unwrap();
        assert!(snapshot.bitmap().bit(offset));
        assert_eq!(snapshot.tail().active_action, EventAction::Degrade);
        assert_eq!(snapshot.tail().active_severity, EventSeverity::Medium);
        assert!(snapshot
            .head()
            .global_flags
            .contains(GlobalFlag::REDUCE_ONLY));
    }

    #[test]
    fn manual_pause_zeroes_bitmap() {
        let mut writer = EcoWriter::new(AccountScope::new(9, 0))
            .with_origin_minute(480)
            .with_mask_length(512)
            .with_baseline_window(480, 540);

        let snapshot = writer.build_and_publish(BuildRequest {
            now_min_ct: 500,
            age_8ms: 1,
            created_ms_coarse: 500,
            events: &[],
            global_flags: GlobalFlag::empty(),
            manual_pause: true,
            day_of_week: 4,
            holiday_flag: true,
        });

        let bitmap = snapshot.bitmap();
        for offset in 0..BITMAP_BITS as u16 {
            assert!(!bitmap.bit(offset));
        }
        assert!(snapshot
            .head()
            .global_flags
            .contains(GlobalFlag::PAUSED | GlobalFlag::MANUAL));
        assert_eq!(snapshot.tail().active_action, EventAction::Lock);
        assert_eq!(snapshot.tail().active_severity, EventSeverity::Critical);
    }

    #[test]
    fn next_lockout_and_resume_computed() {
        let mut writer = EcoWriter::new(AccountScope::new(5, 0))
            .with_origin_minute(480)
            .with_mask_length(512)
            .with_baseline_window(480, 520);

        let events = [EventWindow::econ(
            500,
            505,
            EventSeverity::High,
            EventAction::ForbidNew,
        )];

        let snapshot = writer.build_and_publish(BuildRequest {
            now_min_ct: 498,
            age_8ms: 3,
            created_ms_coarse: 777,
            events: &events,
            global_flags: GlobalFlag::empty(),
            manual_pause: false,
            day_of_week: 0,
            holiday_flag: false,
        });

        assert_eq!(snapshot.next_lockout_minute(), Some(500));
        assert_eq!(snapshot.next_resume_minute(), Some(505));
    }

    #[test]
    fn snapshot_rejects_bad_checksum() {
        let mut writer = EcoWriter::new(AccountScope::new(1, 0));
        let draft = writer.build(BuildRequest {
            now_min_ct: 0,
            age_8ms: 0,
            created_ms_coarse: 0,
            events: &[],
            global_flags: GlobalFlag::empty(),
            manual_pause: false,
            day_of_week: 0,
            holiday_flag: false,
        });
        writer.slot().publish(&draft);
        let slot = writer.slot();
        slot.words[7].store(0, Ordering::Relaxed);
        assert!(slot.load_relaxed().is_none());
    }

    #[test]
    fn publisher_emits_flag_diff() {
        let config = PublisherConfig::new(
            AccountScope::new(42, 1),
            480,
            512,
            SessionClamps::new(Some(905), Some(910)),
        );
        let mut publisher = EcoPublisher::new(config);
        let baseline = [MinuteRange::new(480, 905)];

        let initial = publisher.publish(SnapshotInputs {
            baseline_windows: &baseline,
            events: &[],
            now_min_ct: 500,
            age_8ms: 4,
            created_ms_coarse: 1_000,
            global_flags: GlobalFlag::empty(),
            manual_pause: false,
            session_clamps: SessionClamps::new(Some(905), Some(910)),
            day_of_week: 1,
            holiday_flag: false,
        });

        assert!(initial.flag_diff.set.contains(GlobalFlag::ALLOWED_NOW));
        assert!(initial.flag_diff.cleared.is_empty());

        let paused = publisher.publish(SnapshotInputs {
            baseline_windows: &baseline,
            events: &[],
            now_min_ct: 906,
            age_8ms: 6,
            created_ms_coarse: 2_000,
            global_flags: GlobalFlag::empty(),
            manual_pause: true,
            session_clamps: SessionClamps::new(Some(905), Some(910)),
            day_of_week: 1,
            holiday_flag: false,
        });

        assert!(paused
            .flag_diff
            .set
            .contains(GlobalFlag::PAUSED | GlobalFlag::MANUAL));
        assert!(paused.flag_diff.cleared.contains(GlobalFlag::ALLOWED_NOW));
        assert!(paused
            .snapshot
            .head()
            .global_flags
            .contains(GlobalFlag::AT_EOD));
    }

    proptest! {
        #[test]
        fn overlapping_events_escalate_action(
            base_start in 0u16..MINUTES_PER_DAY,
            len_a in 1u16..=60,
            len_b in 1u16..=60,
            severity_a in 0u8..=3,
            severity_b in 0u8..=3,
            action_a in 0u8..=3,
            action_b in 0u8..=3,
        ) {
            let start_a = base_start;
            let end_a = (base_start + len_a) % MINUTES_PER_DAY;
            let start_b = (base_start + len_a / 2) % MINUTES_PER_DAY;
            let end_b = (start_b + len_b) % MINUTES_PER_DAY;

            let events = [
                EventWindow::new(
                    start_a,
                    end_a,
                    EventSeverity::from_bits(severity_a),
                    EventAction::from_bits(action_a),
                    0,
                    EventKind::Econ,
                ),
                EventWindow::new(
                    start_b,
                    end_b,
                    EventSeverity::from_bits(severity_b),
                    EventAction::from_bits(action_b),
                    0,
                    EventKind::Other,
                ),
            ];

            let mut writer = EcoWriter::new(AccountScope::new(99, 0))
                .with_origin_minute(base_start)
                .with_mask_length(512)
                .with_baseline_window(base_start, base_start.wrapping_add(512));

            let now = start_b;
            let snapshot = writer.build_and_publish(BuildRequest {
                now_min_ct: now,
                age_8ms: 0,
                created_ms_coarse: 0,
                events: &events,
                global_flags: GlobalFlag::empty(),
                manual_pause: false,
                day_of_week: 0,
                holiday_flag: false,
            });

            let target_action = core::cmp::max(
                EventAction::from_bits(action_a),
                EventAction::from_bits(action_b),
            );
            let target_severity = core::cmp::max(
                EventSeverity::from_bits(severity_a),
                EventSeverity::from_bits(severity_b),
            );

            prop_assert_eq!(snapshot.tail().active_action, target_action);
            prop_assert_eq!(snapshot.tail().active_severity, target_severity);
        }

        #[test]
        fn allowed_flag_matches_snapshot(
            origin in 0u16..MINUTES_PER_DAY,
            mask in 1u16..=BITMAP_BITS as u16,
            now in 0u16..MINUTES_PER_DAY,
            manual_pause in any::<bool>()
        ) {
            let mut writer = EcoWriter::new(AccountScope::new(1, 0))
                .with_origin_minute(origin)
                .with_mask_length(mask)
                .with_baseline_window(origin, origin.wrapping_add(mask));

            let snapshot = writer.build_and_publish(BuildRequest {
                now_min_ct: now,
                age_8ms: 0,
                created_ms_coarse: 0,
                events: &[],
                global_flags: GlobalFlag::empty(),
                manual_pause,
                day_of_week: 0,
                holiday_flag: false,
            });

            let allowed_flag = snapshot.head().global_flags.contains(GlobalFlag::ALLOWED_NOW);
            prop_assert_eq!(snapshot.is_allowed_now(), allowed_flag);
        }

        #[test]
        fn flag_diff_properties(prev_bits in any::<u16>(), next_bits in any::<u16>()) {
            let prev = GlobalFlag::from_bits_truncate(prev_bits);
            let next = GlobalFlag::from_bits_truncate(next_bits);
            let diff = FlagDiff::compute(prev, next);

            prop_assert!(diff.set & diff.cleared == GlobalFlag::empty());
            let recomposed = (prev | diff.set) & !diff.cleared;
            prop_assert_eq!(recomposed, next);
        }
    }
}
