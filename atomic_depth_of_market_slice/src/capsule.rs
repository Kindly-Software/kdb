#![allow(clippy::module_name_repetitions)]

use core::sync::atomic::Ordering;
use portable_atomic::AtomicU128;

use crate::layout;

/// Wrapper around the eight packed 128-bit words that form a DOS capsule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedDos1024 {
    words: [u128; layout::WORD_COUNT],
}

impl PackedDos1024 {
    /// Create a new packed representation from raw words.
    #[must_use]
    pub const fn new(words: [u128; layout::WORD_COUNT]) -> Self {
        Self { words }
    }

    /// Access the underlying words.
    #[must_use]
    pub const fn words(&self) -> &[u128; layout::WORD_COUNT] {
        &self.words
    }

    /// Consume the wrapper and return the raw words.
    #[must_use]
    pub const fn into_words(self) -> [u128; layout::WORD_COUNT] {
        self.words
    }

    /// Unpack the words into a structured snapshot.
    #[must_use]
    pub fn unpack(self) -> Dos1024Snapshot {
        unpack_snapshot(self.words)
    }
}

impl From<[u128; layout::WORD_COUNT]> for PackedDos1024 {
    #[inline]
    fn from(words: [u128; layout::WORD_COUNT]) -> Self {
        Self::new(words)
    }
}

impl From<PackedDos1024> for [u128; layout::WORD_COUNT] {
    #[inline]
    fn from(value: PackedDos1024) -> Self {
        value.words
    }
}

/// Header fields stored in `W0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DosHeader {
    /// `commit` flag indicating the snapshot is live.
    pub commit: bool,
    /// `stale` flag published by the writer once the data breaches its age budget.
    pub stale: bool,
    /// Even version (LSB = 0) published in the commit header.
    pub version_even: u8,
    /// Sequence number mirrored in the tail word.
    pub sequence_head: u16,
    /// Instrument identifier for slot A.
    pub sym_a_id: u16,
    /// Instrument identifier for slot B.
    pub sym_b_id: u16,
    /// Coarse timestamp (`ms/4`).
    pub created_ms_coarse: u32,
    /// Minutes-after-open threshold that forbids new positions.
    pub forbid_after_min_ct: u16,
    /// Minutes remaining before end-of-day flattening must complete.
    pub eod_flat_min_ct: u16,
    /// Miscellaneous session flags (14 bits).
    pub flags: u16,
    /// Reserved spare bits (10 bits effective).
    pub spare: u16,
}

/// Per-level depth information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DosLevel {
    /// Price expressed in ticks relative to tick zero.
    pub px_ticks: i16,
    /// Visible size at the level.
    pub qty: u16,
}

/// Per-instrument header metadata stored in `hdrA`/`hdrB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DosInstrumentHeader {
    /// Tick value in cents × 1/16 (Q4).
    pub tick_value_cents_q4: u16,
    /// Reference price tick index (signed S12).
    pub px_ref_ticks: i16,
    /// Local book version (4 bits).
    pub local_ver: u8,
    /// Local book sequence (4 bits).
    pub local_seq: u8,
}

/// Per-instrument snapshot containing five bid/ask levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DosInstrument {
    /// Compact metadata header.
    pub header: DosInstrumentHeader,
    /// Bid levels ordered from L1 to L5.
    pub bids: [DosLevel; 5],
    /// Ask levels ordered from L1 to L5.
    pub asks: [DosLevel; 5],
    /// Sum of bid quantities for levels L1–L3.
    pub sum_bid_l1_3: u16,
    /// Sum of ask quantities for levels L1–L3.
    pub sum_ask_l1_3: u16,
}

/// Derived metrics per instrument stored in `W7`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DosInstrumentDerived {
    /// Spread in ticks (≥ 0).
    pub spread_ticks: u8,
    /// Order-book imbalance Q1.10 (−1024..+1023).
    pub obi_q1_10: i16,
    /// Microprice offset relative to mid (ticks).
    pub micro_off_ticks: i16,
    /// Sweep detection flag with ~200 ms decay.
    pub sweep_flag: bool,
    /// Mid-price trend over ≈200 ms (ticks).
    pub trend_200ms_ticks: i16,
}

/// Tail word containing derived summary and integrity metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DosSummary {
    /// Derived metrics for instrument A.
    pub instrument_a: DosInstrumentDerived,
    /// Derived metrics for instrument B.
    pub instrument_b: DosInstrumentDerived,
    /// CRC16 computed over `W1..W6`.
    pub checksum16: u16,
    /// Tail version (odd during staging).
    pub ver_tail: u8,
    /// Tail sequence.
    pub seq_tail: u16,
}

/// Human-friendly snapshot of the entire DOS-1024 capsule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dos1024Snapshot {
    /// Commit header (`W0`).
    pub header: DosHeader,
    /// Instrument A block (`W1..W3`).
    pub instrument_a: DosInstrument,
    /// Instrument B block (`W4..W6`).
    pub instrument_b: DosInstrument,
    /// Derived summary (`W7`).
    pub summary: DosSummary,
}

impl Dos1024Snapshot {
    /// Pack the snapshot into eight 128-bit words.
    #[must_use]
    pub fn pack(&self) -> PackedDos1024 {
        PackedDos1024::new(pack_snapshot(self))
    }

    /// Return the dequantised creation timestamp in milliseconds.
    #[must_use]
    #[inline]
    pub fn created_ms(&self) -> u64 {
        layout::dequantise_timestamp_ms(self.header.created_ms_coarse)
    }

    /// Determine whether the snapshot is stale for the supplied clock and budget.
    #[must_use]
    pub fn is_stale(&self, now_ms: u64, budget_ms: u64) -> bool {
        now_ms.saturating_sub(self.created_ms()) > budget_ms
    }

    /// Validate version and sequence coherence between head and tail.
    #[must_use]
    pub fn head_tail_match(&self) -> bool {
        if (self.header.version_even & 1) != 0 {
            return false;
        }
        if self.summary.ver_tail.wrapping_add(1) != self.header.version_even {
            return false;
        }
        self.summary.seq_tail == self.header.sequence_head
    }
}

/// Atomic DOS capsule supporting `SWeMR` semantics.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct Dos1024 {
    words: [AtomicU128; layout::WORD_COUNT],
}

impl Dos1024 {
    /// Create a zero-initialised capsule.
    #[must_use]
    pub const fn new() -> Self {
        const ZERO: AtomicU128 = AtomicU128::new(0);
        Self {
            words: [ZERO; layout::WORD_COUNT],
        }
    }

    /// Seed the capsule from pre-packed words.
    #[must_use]
    pub const fn from_packed(packed: PackedDos1024) -> Self {
        let words = packed.into_words();
        Self {
            words: [
                AtomicU128::new(words[0]),
                AtomicU128::new(words[1]),
                AtomicU128::new(words[2]),
                AtomicU128::new(words[3]),
                AtomicU128::new(words[4]),
                AtomicU128::new(words[5]),
                AtomicU128::new(words[6]),
                AtomicU128::new(words[7]),
            ],
        }
    }

    /// Store the header with relaxed ordering (used to publish odd version).
    pub fn store_header_relaxed(&self, word: u128) {
        self.words[0].store(word, Ordering::Relaxed);
    }

    /// Stage body words (`W1..W7`) using relaxed ordering.
    pub fn store_body_relaxed(&self, index: usize, word: u128) {
        debug_assert!((1..layout::WORD_COUNT).contains(&index));
        self.words[index].store(word, Ordering::Relaxed);
    }

    /// Publish the header with `store(Ordering::Release)` semantics.
    pub fn store_header_release(&self, word: u128) {
        self.words[0].store(word, Ordering::Release);
    }

    /// Convenience helper that publishes all words in order.
    pub fn publish(&self, packed: &PackedDos1024) {
        let words = packed.words();
        for (idx, word) in words.iter().enumerate().skip(1) {
            self.store_body_relaxed(idx, *word);
        }
        self.store_header_release(words[0]);
    }

    /// Read a consistent snapshot using the provided retry budget.
    #[must_use]
    pub fn load_consistent(&self, attempts: usize) -> Option<Dos1024Snapshot> {
        let mut remaining = attempts;
        while remaining > 0 {
            remaining -= 1;
            let head_word = self.words[0].load(Ordering::Relaxed);
            let header = unpack_header(head_word);
            if (header.version_even & 1) != 0 || !header.commit {
                continue;
            }
            let mut words = [0u128; layout::WORD_COUNT];
            words[0] = head_word;
            for (idx, slot) in words.iter_mut().enumerate().skip(1) {
                *slot = self.words[idx].load(Ordering::Relaxed);
            }
            let snapshot = PackedDos1024::new(words).unpack();
            if !snapshot.header.commit {
                continue;
            }
            if !snapshot.head_tail_match() {
                continue;
            }
            if !verify_checksum(&snapshot, &words) {
                continue;
            }
            return Some(snapshot);
        }
        None
    }

    /// Load a raw word with arbitrary ordering (diagnostics/testing).
    #[must_use]
    pub fn load_word(&self, index: usize, ordering: Ordering) -> u128 {
        self.words[index].load(ordering)
    }
}

impl Default for Dos1024 {
    fn default() -> Self {
        Self::new()
    }
}

fn pack_snapshot(snapshot: &Dos1024Snapshot) -> [u128; layout::WORD_COUNT] {
    let mut words = [0u128; layout::WORD_COUNT];
    words[1] = pack_instrument(&snapshot.instrument_a);
    words[2] = pack_instrument_mid(&snapshot.instrument_a);
    words[3] = pack_instrument_tail(&snapshot.instrument_a);
    words[4] = pack_instrument(&snapshot.instrument_b);
    words[5] = pack_instrument_mid(&snapshot.instrument_b);
    words[6] = pack_instrument_tail(&snapshot.instrument_b);
    let checksum = compute_checksum(&words[1..7]);
    words[7] = pack_summary(&snapshot.summary, checksum);
    words[0] = pack_header(&snapshot.header);
    words
}

fn unpack_snapshot(words: [u128; layout::WORD_COUNT]) -> Dos1024Snapshot {
    let header = unpack_header(words[0]);
    let instrument_a = unpack_instrument(words[1], words[2], words[3]);
    let instrument_b = unpack_instrument(words[4], words[5], words[6]);
    let summary = unpack_summary(words[7]);
    Dos1024Snapshot {
        header,
        instrument_a,
        instrument_b,
        summary,
    }
}

pub(crate) fn pack_header(header: &DosHeader) -> u128 {
    let mut word = 0u128;
    if header.commit {
        word = layout::pack_unsigned(word, layout::W0_COMMIT, 1);
    }
    if header.stale {
        word = layout::pack_unsigned(word, layout::W0_STALE, 1);
    }
    word = layout::pack_unsigned(word, layout::W0_VERSION, u128::from(header.version_even));
    word = layout::pack_unsigned(word, layout::W0_SEQ_HEAD, u128::from(header.sequence_head));
    word = layout::pack_unsigned(word, layout::W0_SYM_A_ID, u128::from(header.sym_a_id));
    word = layout::pack_unsigned(word, layout::W0_SYM_B_ID, u128::from(header.sym_b_id));
    word = layout::pack_unsigned(
        word,
        layout::W0_CREATED_MS_COARSE,
        u128::from(header.created_ms_coarse & ((1 << 24) - 1)),
    );
    word = layout::pack_unsigned(
        word,
        layout::W0_FORBID_AFTER_MIN_CT,
        u128::from(header.forbid_after_min_ct & 0x07FF),
    );
    word = layout::pack_unsigned(
        word,
        layout::W0_EOD_FLAT_MIN_CT,
        u128::from(header.eod_flat_min_ct & 0x07FF),
    );
    word = layout::pack_unsigned(word, layout::W0_FLAGS, u128::from(header.flags & 0x3FFF));
    word = layout::pack_unsigned(word, layout::W0_SPARE, u128::from(header.spare & 0x03FF));
    word
}

fn unpack_header(word: u128) -> DosHeader {
    DosHeader {
        commit: layout::unpack_unsigned(word, layout::W0_COMMIT) != 0,
        stale: layout::unpack_unsigned(word, layout::W0_STALE) != 0,
        version_even: layout::unpack_unsigned(word, layout::W0_VERSION) as u8,
        sequence_head: layout::unpack_unsigned(word, layout::W0_SEQ_HEAD) as u16,
        sym_a_id: layout::unpack_unsigned(word, layout::W0_SYM_A_ID) as u16,
        sym_b_id: layout::unpack_unsigned(word, layout::W0_SYM_B_ID) as u16,
        created_ms_coarse: layout::unpack_unsigned(word, layout::W0_CREATED_MS_COARSE) as u32,
        forbid_after_min_ct: layout::unpack_unsigned(word, layout::W0_FORBID_AFTER_MIN_CT) as u16,
        eod_flat_min_ct: layout::unpack_unsigned(word, layout::W0_EOD_FLAT_MIN_CT) as u16,
        flags: layout::unpack_unsigned(word, layout::W0_FLAGS) as u16,
        spare: layout::unpack_unsigned(word, layout::W0_SPARE) as u16,
    }
}

fn pack_instrument(instr: &DosInstrument) -> u128 {
    let mut word = 0u128;
    let header = layout::pack_instrument_header(
        instr.header.tick_value_cents_q4,
        instr.header.px_ref_ticks,
        instr.header.local_ver,
        instr.header.local_seq,
    );
    word |= u128::from(header);
    word |= u128::from(layout::pack_level(
        instr.bids[0].px_ticks,
        instr.bids[0].qty,
    )) << 32;
    word |= u128::from(layout::pack_level(
        instr.asks[0].px_ticks,
        instr.asks[0].qty,
    )) << 64;
    word |= u128::from(layout::pack_level(
        instr.bids[1].px_ticks,
        instr.bids[1].qty,
    )) << 96;
    word
}

fn pack_instrument_mid(instr: &DosInstrument) -> u128 {
    let mut word = 0u128;
    let a2 = layout::pack_level(instr.asks[1].px_ticks, instr.asks[1].qty);
    let b3 = layout::pack_level(instr.bids[2].px_ticks, instr.bids[2].qty);
    let a3 = layout::pack_level(instr.asks[2].px_ticks, instr.asks[2].qty);
    let b4 = layout::pack_level(instr.bids[3].px_ticks, instr.bids[3].qty);
    word |= u128::from(a2);
    word |= u128::from(b3) << 32;
    word |= u128::from(a3) << 64;
    word |= u128::from(b4) << 96;
    word
}

fn pack_instrument_tail(instr: &DosInstrument) -> u128 {
    let mut word = 0u128;
    let a4 = layout::pack_level(instr.asks[3].px_ticks, instr.asks[3].qty);
    let b5 = layout::pack_level(instr.bids[4].px_ticks, instr.bids[4].qty);
    let a5 = layout::pack_level(instr.asks[4].px_ticks, instr.asks[4].qty);
    let sums = layout::pack_sums(instr.sum_bid_l1_3, instr.sum_ask_l1_3);
    word |= u128::from(a4);
    word |= u128::from(b5) << 32;
    word |= u128::from(a5) << 64;
    word |= u128::from(sums) << 96;
    word
}

fn unpack_instrument(w1: u128, w2: u128, w3: u128) -> DosInstrument {
    let header_word = (w1 & 0xFFFF_FFFF) as u32;
    let (tick_value, px_ref, local_ver, local_seq) = layout::unpack_instrument_header(header_word);
    let bids = [
        unpack_level_from_word(w1, 1),
        unpack_level_from_word(w1, 3),
        unpack_level_from_word(w2, 1),
        unpack_level_from_word(w2, 3),
        unpack_level_from_word(w3, 1),
    ];
    let asks = [
        unpack_level_from_word(w1, 2),
        unpack_level_from_word(w2, 0),
        unpack_level_from_word(w2, 2),
        unpack_level_from_word(w3, 0),
        unpack_level_from_word(w3, 2),
    ];
    let (sum_bid, sum_ask) = layout::unpack_sums((w3 >> 96) as u32);
    DosInstrument {
        header: DosInstrumentHeader {
            tick_value_cents_q4: tick_value,
            px_ref_ticks: px_ref,
            local_ver,
            local_seq,
        },
        bids,
        asks,
        sum_bid_l1_3: sum_bid,
        sum_ask_l1_3: sum_ask,
    }
}

fn unpack_level_from_word(word: u128, slot: u8) -> DosLevel {
    let shift = match slot {
        0 => 0,
        1 => 32,
        2 => 64,
        3 => 96,
        _ => 0,
    };
    let raw = ((word >> shift) & 0xFFFF_FFFF) as u32;
    let (px, qty) = layout::unpack_level(raw);
    DosLevel { px_ticks: px, qty }
}

fn pack_summary(summary: &DosSummary, checksum: u16) -> u128 {
    let mut word = 0u128;
    let a = &summary.instrument_a;
    let b = &summary.instrument_b;
    word |= u128::from(a.spread_ticks);
    word |= u128::from(encode_signed(a.obi_q1_10, 12)) << 8;
    word |= u128::from(encode_signed(a.micro_off_ticks, 12)) << 20;
    if a.sweep_flag {
        word |= 1u128 << 32;
    }
    word |= u128::from(encode_signed(a.trend_200ms_ticks, 11)) << 33;

    word |= u128::from(b.spread_ticks) << 44;
    word |= u128::from(encode_signed(b.obi_q1_10, 12)) << 52;
    word |= u128::from(encode_signed(b.micro_off_ticks, 12)) << 64;
    if b.sweep_flag {
        word |= 1u128 << 76;
    }
    word |= u128::from(encode_signed(b.trend_200ms_ticks, 11)) << 77;

    word |= u128::from(checksum) << 88;
    word |= u128::from(summary.ver_tail) << 104;
    word |= u128::from(summary.seq_tail) << 112;
    word
}

fn unpack_summary(word: u128) -> DosSummary {
    let a_spread = (word & 0xFF) as u8;
    let a_obi = decode_signed((word >> 8) & 0xFFF, 12);
    let a_micro = decode_signed((word >> 20) & 0xFFF, 12);
    let a_sweep = ((word >> 32) & 1) != 0;
    let a_trend = decode_signed((word >> 33) & 0x7FF, 11);

    let b_spread = ((word >> 44) & 0xFF) as u8;
    let b_obi = decode_signed((word >> 52) & 0xFFF, 12);
    let b_micro = decode_signed((word >> 64) & 0xFFF, 12);
    let b_sweep = ((word >> 76) & 1) != 0;
    let b_trend = decode_signed((word >> 77) & 0x7FF, 11);

    DosSummary {
        instrument_a: DosInstrumentDerived {
            spread_ticks: a_spread,
            obi_q1_10: a_obi,
            micro_off_ticks: a_micro,
            sweep_flag: a_sweep,
            trend_200ms_ticks: a_trend,
        },
        instrument_b: DosInstrumentDerived {
            spread_ticks: b_spread,
            obi_q1_10: b_obi,
            micro_off_ticks: b_micro,
            sweep_flag: b_sweep,
            trend_200ms_ticks: b_trend,
        },
        checksum16: ((word >> 88) & 0xFFFF) as u16,
        ver_tail: ((word >> 104) & 0xFF) as u8,
        seq_tail: ((word >> 112) & 0xFFFF) as u16,
    }
}

fn encode_signed(value: i16, width: u32) -> u16 {
    let mask = (1u16 << width) - 1;
    (i32::from(value) as u16) & mask
}

fn decode_signed(raw: u128, width: u32) -> i16 {
    let raw = raw as u32;
    let shift = 32 - width;
    (((raw << shift) as i32) >> shift) as i16
}

fn compute_checksum(words: &[u128]) -> u16 {
    debug_assert!(words.len() <= 6);
    let mut buf = [0u8; 16 * 6];
    let used = words.len() * 16;
    for (idx, word) in words.iter().enumerate() {
        let bytes = layout::word_to_bytes(*word);
        let base = idx * 16;
        buf[base..base + 16].copy_from_slice(&bytes);
    }
    layout::crc16(&buf[..used])
}

fn verify_checksum(snapshot: &Dos1024Snapshot, words: &[u128; layout::WORD_COUNT]) -> bool {
    let expected = compute_checksum(&words[1..7]);
    snapshot.summary.checksum16 == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_and_unpack_header() {
        let header = DosHeader {
            commit: true,
            stale: false,
            version_even: 4,
            sequence_head: 123,
            sym_a_id: 111,
            sym_b_id: 222,
            created_ms_coarse: 42,
            forbid_after_min_ct: 17,
            eod_flat_min_ct: 29,
            flags: 0x2AA,
            spare: 0x1F,
        };
        let word = pack_header(&header);
        let decoded = unpack_header(word);
        assert_eq!(header, decoded);
    }

    #[test]
    fn instrument_roundtrip_preserves_levels() {
        let instrument = DosInstrument {
            header: DosInstrumentHeader {
                tick_value_cents_q4: 123,
                px_ref_ticks: -12,
                local_ver: 7,
                local_seq: 11,
            },
            bids: [
                DosLevel {
                    px_ticks: 100,
                    qty: 10,
                },
                DosLevel {
                    px_ticks: 99,
                    qty: 9,
                },
                DosLevel {
                    px_ticks: 98,
                    qty: 8,
                },
                DosLevel {
                    px_ticks: 97,
                    qty: 7,
                },
                DosLevel {
                    px_ticks: 96,
                    qty: 6,
                },
            ],
            asks: [
                DosLevel {
                    px_ticks: 101,
                    qty: 11,
                },
                DosLevel {
                    px_ticks: 102,
                    qty: 12,
                },
                DosLevel {
                    px_ticks: 103,
                    qty: 13,
                },
                DosLevel {
                    px_ticks: 104,
                    qty: 14,
                },
                DosLevel {
                    px_ticks: 105,
                    qty: 15,
                },
            ],
            sum_bid_l1_3: 27,
            sum_ask_l1_3: 36,
        };
        let w1 = pack_instrument(&instrument);
        let w2 = pack_instrument_mid(&instrument);
        let w3 = pack_instrument_tail(&instrument);
        let decoded = unpack_instrument(w1, w2, w3);
        assert_eq!(instrument.header, decoded.header);
        assert_eq!(instrument.bids, decoded.bids);
        assert_eq!(instrument.asks, decoded.asks);
        assert_eq!(instrument.sum_bid_l1_3, decoded.sum_bid_l1_3);
        assert_eq!(instrument.sum_ask_l1_3, decoded.sum_ask_l1_3);
    }

    #[test]
    fn checksum_detects_tamper() {
        let snapshot = Dos1024Snapshot {
            header: DosHeader {
                commit: true,
                stale: false,
                version_even: 2,
                sequence_head: 1,
                sym_a_id: 1,
                sym_b_id: 2,
                created_ms_coarse: 0,
                forbid_after_min_ct: 0,
                eod_flat_min_ct: 0,
                flags: 0,
                spare: 0,
            },
            instrument_a: DosInstrument::default(),
            instrument_b: DosInstrument::default(),
            summary: DosSummary {
                instrument_a: DosInstrumentDerived::default(),
                instrument_b: DosInstrumentDerived::default(),
                checksum16: 0,
                ver_tail: 1,
                seq_tail: 1,
            },
        };
        let packed = snapshot.pack();
        let mut words = packed.into_words();
        let unpacked = PackedDos1024::new(words).unpack();
        assert!(verify_checksum(&unpacked, &words));
        words[2] ^= 0x10;
        assert!(!verify_checksum(&unpacked, &words));
    }
}
