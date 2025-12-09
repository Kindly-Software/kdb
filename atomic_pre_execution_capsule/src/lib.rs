#![no_std]

//! PEX-1024 (Pre-Executed Playbook) packs four staged plays, shared templates,
//! and router defaults into a single 1024-bit capsule. A writer thread fills W1
//! through W7, computes a checksum, and flips W0 with a release store. Readers
//! obtain a coherent snapshot with one cache-line read and apply trigger/route
//! policies without allocating or copying.

use core::sync::atomic::Ordering;
use portable_atomic::AtomicU128;

#[cfg(feature = "sim")]
extern crate alloc;
#[cfg(test)]
extern crate std;

const WORD_COUNT: usize = 8;
const ATTEMPTS: usize = 8;

#[derive(Clone, Copy)]
struct Field {
    shift: u32,
    bits: u32,
}

impl Field {
    const fn value_mask(self) -> u128 {
        if self.bits == 0 {
            0
        } else if self.bits >= 128 {
            !0
        } else {
            (1u128 << self.bits) - 1
        }
    }

    const fn mask(self) -> u128 {
        self.value_mask() << self.shift
    }

    fn set(self, word: u128, value: u128) -> u128 {
        debug_assert_eq!(value & !self.value_mask(), 0, "value exceeds field width");
        let cleared = word & !self.mask();
        cleared | (value << self.shift)
    }

    fn set_signed(self, word: u128, value: i32) -> u128 {
        debug_assert!(
            self.bits > 0 && self.bits <= 32,
            "signed field width invalid"
        );
        let range = 1i32 << (self.bits - 1);
        debug_assert!(
            value >= -range && value < range,
            "signed value exceeds field width"
        );
        let encoded = (value as i64) & ((1i64 << self.bits) - 1);
        self.set(word, encoded as u128)
    }

    const fn get(self, word: u128) -> u128 {
        (word >> self.shift) & self.value_mask()
    }

    fn get_signed(self, word: u128) -> i32 {
        debug_assert!(
            self.bits > 0 && self.bits <= 32,
            "signed field width invalid"
        );
        let raw = self.get(word) as u32;
        let shift = 32 - self.bits;
        ((raw << shift) as i32) >> shift
    }
}

const W0_COMMIT: Field = Field { shift: 0, bits: 1 };
const W0_STALE: Field = Field { shift: 1, bits: 1 };
const W0_VER_EVEN: Field = Field { shift: 2, bits: 8 };
const W0_SEQ_HEAD: Field = Field {
    shift: 10,
    bits: 16,
};
const W0_ACCOUNT_ID: Field = Field {
    shift: 26,
    bits: 16,
};
const W0_CREATED_MS_COARSE: Field = Field {
    shift: 42,
    bits: 24,
};
const W0_DEFAULT_TTL_MS: Field = Field {
    shift: 66,
    bits: 12,
};
const W0_FORBID_AFTER_MIN_CT: Field = Field {
    shift: 78,
    bits: 11,
};
const W0_EOD_FLAT_MIN_CT: Field = Field {
    shift: 89,
    bits: 11,
};
const W0_PLAY_MASK: Field = Field {
    shift: 100,
    bits: 4,
};
const W0_GLOBAL_FLAGS: Field = Field {
    shift: 104,
    bits: 8,
};
const W0_PORTFOLIO_BREAKER_LEVEL: Field = Field {
    shift: 112,
    bits: 2,
};
const W0_SYMBOL_COUNT: Field = Field {
    shift: 114,
    bits: 4,
};
const W0_SPARE: Field = Field {
    shift: 118,
    bits: 8,
};
const W0_RESERVED: Field = Field {
    shift: 126,
    bits: 2,
};

const PLAY_ENABLE: Field = Field { shift: 0, bits: 1 };
const PLAY_DIR: Field = Field { shift: 1, bits: 1 };
const PLAY_ANCHOR: Field = Field { shift: 2, bits: 2 };
const PLAY_ORDER_TYPE: Field = Field { shift: 4, bits: 3 };
const PLAY_TIF: Field = Field { shift: 7, bits: 3 };
const PLAY_SYM_ID: Field = Field {
    shift: 10,
    bits: 12,
};
const PLAY_QTY: Field = Field {
    shift: 22,
    bits: 18,
};
const PLAY_PX_TICKS: Field = Field {
    shift: 40,
    bits: 16,
};
const PLAY_ROUTE_TMPL_ID: Field = Field { shift: 56, bits: 3 };
const PLAY_BRACKET_TMPL_ID: Field = Field { shift: 59, bits: 3 };
const PLAY_SLIP_CAP_BP: Field = Field {
    shift: 62,
    bits: 10,
};
const PLAY_LAT_BUDGET_US: Field = Field {
    shift: 72,
    bits: 10,
};
const PLAY_TTL_MS: Field = Field {
    shift: 82,
    bits: 10,
};
const PLAY_PRIORITY: Field = Field { shift: 92, bits: 6 };
const PLAY_TRIG_MASK: Field = Field {
    shift: 98,
    bits: 16,
};
const PLAY_TRIG_PARAM: Field = Field {
    shift: 114,
    bits: 8,
};
const PLAY_SPARE: Field = Field {
    shift: 122,
    bits: 6,
};

const W5_B0_TP_TICKS: Field = Field { shift: 0, bits: 10 };
const W5_B0_SL_TICKS: Field = Field {
    shift: 10,
    bits: 10,
};
const W5_B0_TRAIL_TICKS: Field = Field {
    shift: 20,
    bits: 10,
};
const W5_B0_TSTOP_MS: Field = Field {
    shift: 30,
    bits: 12,
};
const W5_B0_EXIT_TIF: Field = Field { shift: 42, bits: 2 };
const W5_B0_SCALE_OUT_PCT: Field = Field { shift: 44, bits: 7 };
const W5_B0_FLAGS: Field = Field { shift: 51, bits: 3 };
const W5_B1_TP_TICKS: Field = Field {
    shift: 54,
    bits: 10,
};
const W5_B1_SL_TICKS: Field = Field {
    shift: 64,
    bits: 10,
};
const W5_B1_TRAIL_TICKS: Field = Field {
    shift: 74,
    bits: 10,
};
const W5_B1_TSTOP_MS: Field = Field {
    shift: 84,
    bits: 12,
};
const W5_B1_EXIT_TIF: Field = Field { shift: 96, bits: 2 };
const W5_B1_SCALE_OUT_PCT: Field = Field { shift: 98, bits: 7 };
const W5_B1_FLAGS: Field = Field {
    shift: 105,
    bits: 3,
};
const W5_RESERVED: Field = Field {
    shift: 108,
    bits: 20,
};

const ROUTE_STRIDE: u32 = 25;
const ROUTE_ROUTE_ID: Field = Field { shift: 0, bits: 10 };
const ROUTE_MAKER_TAKER: Field = Field { shift: 10, bits: 1 };
const ROUTE_IOC_FOK: Field = Field { shift: 11, bits: 2 };
const ROUTE_POST_ONLY: Field = Field { shift: 13, bits: 1 };
const ROUTE_ALLOW_PARTIAL: Field = Field { shift: 14, bits: 1 };
const ROUTE_SLIP_CAP_BP: Field = Field {
    shift: 15,
    bits: 10,
};

const W7_CHECKSUM: Field = Field { shift: 0, bits: 16 };
const W7_VER_TAIL: Field = Field { shift: 16, bits: 8 };
const W7_SEQ_TAIL: Field = Field {
    shift: 24,
    bits: 16,
};
const W7_SLIP_CAP_DEFAULT_BP: Field = Field {
    shift: 40,
    bits: 10,
};
const W7_LAT_BUDGET_DEFAULT_US: Field = Field {
    shift: 50,
    bits: 12,
};
const W7_ROUTER_HINTS: Field = Field { shift: 62, bits: 8 };
const W7_SPARE: Field = Field {
    shift: 70,
    bits: 58,
};

const PLAY_COUNT: usize = 4;
const BRACKET_TEMPLATE_COUNT: usize = 2;
const ROUTE_TEMPLATE_COUNT: usize = 4;

/// RLT-1024 adapters for scaling playbooks.
pub mod rlt;

/// Convenience utilities for staging and consuming PEX capsules in a production
/// pipeline. Provides writer/router coordination, lightweight stats, and a
/// priority-aware iterator over staged plays.
pub mod pipeline {
    use super::{
        PexCapsule, PexDraft, PexSnapshot, PexWords, Play, PlayView, TailDefaults, PLAY_COUNT,
    };

    /// Publish counters collected by [`PexWriter`].
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct PublishStats {
        pub publishes: u64,
        pub stale_marks: u64,
    }

    /// Router counters collected by [`PexRouter`].
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct RouterStats {
        pub polls: u64,
        pub accepted_snapshots: u64,
        pub plays_considered: u64,
    }

    /// Stateful helper that owns a reusable draft and publishes snapshots with
    /// monotonically increasing versions and sequence numbers.
    pub struct PexWriter<'a> {
        capsule: &'a PexCapsule,
        draft: PexDraft,
        next_odd_version: u8,
        next_seq: u16,
        stats: PublishStats,
    }

    impl<'a> PexWriter<'a> {
        /// Create a writer with the provided capsule reference.
        pub fn new(capsule: &'a PexCapsule) -> Self {
            Self {
                capsule,
                draft: PexDraft::default(),
                next_odd_version: 1,
                next_seq: 0,
                stats: PublishStats::default(),
            }
        }

        /// Access the mutable draft before publishing.
        pub fn draft_mut(&mut self) -> &mut PexDraft {
            &mut self.draft
        }

        /// Replace the draft entirely (useful when cloning from templates).
        pub fn set_draft(&mut self, draft: PexDraft) {
            self.draft = draft;
        }

        /// Seed the writer with explicit version/sequence counters (e.g. after restart).
        pub fn with_counters(mut self, odd_version: u8, seq: u16) -> Self {
            self.next_odd_version = if odd_version & 1 == 1 {
                odd_version
            } else {
                odd_version | 1
            };
            self.next_seq = seq;
            self
        }

        /// Publish the current draft, updating version and sequence numbers.
        pub fn publish(&mut self) -> PexWords {
            self.draft.commit = true;
            self.draft.stale = false;
            self.draft.odd_version = self.next_odd_version;
            self.draft.seq = self.next_seq;
            let words = self.capsule.publish(&self.draft);
            self.stats.publishes = self.stats.publishes.saturating_add(1);
            self.next_odd_version = self.next_odd_version.wrapping_add(2);
            if self.next_odd_version & 1 == 0 {
                self.next_odd_version = self.next_odd_version.wrapping_add(1);
            }
            self.next_seq = self.next_seq.wrapping_add(1);
            words
        }

        /// Mark the capsule stale and bump the stale counter.
        pub fn mark_stale(&mut self) {
            self.capsule.mark_stale();
            self.stats.stale_marks = self.stats.stale_marks.saturating_add(1);
        }

        /// Current publish counters.
        pub fn stats(&self) -> PublishStats {
            self.stats
        }

        /// Immutable access to the underlying capsule for direct inspection.
        pub fn capsule(&self) -> &'a PexCapsule {
            self.capsule
        }
    }

    /// Reader-side helper that tracks the last accepted snapshot and iterates
    /// plays in priority order.
    pub struct PexRouter<'a> {
        capsule: &'a PexCapsule,
        last_ver: u8,
        last_seq: u16,
        stats: RouterStats,
    }

    impl<'a> PexRouter<'a> {
        /// Create a router for the provided capsule.
        pub fn new(capsule: &'a PexCapsule) -> Self {
            Self {
                capsule,
                last_ver: 0,
                last_seq: 0,
                stats: RouterStats::default(),
            }
        }

        /// Attempt to obtain the next coherent snapshot. Returns `None` when
        /// there is no new publish, when the capsule is stale, or when commit is unset.
        pub fn poll_snapshot(&mut self) -> Option<PexSnapshot> {
            self.stats.polls = self.stats.polls.saturating_add(1);
            let snapshot = self.capsule.load_snapshot()?;
            let header = snapshot.header();
            if header.ver_even == self.last_ver && header.seq_head == self.last_seq {
                return None;
            }
            self.last_ver = header.ver_even;
            self.last_seq = header.seq_head;
            self.stats.accepted_snapshots = self.stats.accepted_snapshots.saturating_add(1);
            Some(snapshot)
        }

        /// Iterate over plays in descending priority order for the latest snapshot.
        /// The callback returns `true` to stop iteration early (after firing a play).
        pub fn for_each_play<F>(&mut self, mut f: F)
        where
            F: FnMut(usize, PlayView, &PexSnapshot) -> bool,
        {
            let snapshot = match self.poll_snapshot() {
                Some(s) => s,
                None => return,
            };
            let header = snapshot.header();
            let mut indices = [0usize, 1, 2, 3];
            sort_indices_by_priority(&mut indices, &snapshot);

            for idx in indices.iter().copied().take(PLAY_COUNT) {
                if header.play_mask & (1 << idx) == 0 {
                    continue;
                }
                let play = snapshot.play(idx);
                if !play.enable {
                    continue;
                }
                self.stats.plays_considered = self.stats.plays_considered.saturating_add(1);
                if f(idx, play, &snapshot) {
                    break;
                }
            }
        }

        /// Router counters collected so far.
        pub fn stats(&self) -> RouterStats {
            self.stats
        }

        /// Reset the deduplication cursor (e.g. after fast-forwarding during rollback).
        pub fn reset_cursor(&mut self) {
            self.last_ver = 0;
            self.last_seq = 0;
        }
    }

    /// Utility to construct a default Topstep-style draft for examples/tests.
    pub fn topstep_default_playbook() -> PexDraft {
        let mut draft = PexDraft::default();
        draft.header.account_id = 77;
        draft.header.symbol_count = 2;
        draft.header.global_flags = 0;
        draft.defaults = TailDefaults {
            slip_cap_default_bp: 6,
            lat_budget_default_us: 2_500,
            router_hints: 0,
        };

        let plays: [Play; PLAY_COUNT] = [
            Play {
                enable: true,
                dir_sell: false,
                anchor: 2,
                order_type: 0,
                tif: 1,
                sym_id: 17,
                qty: 120,
                px_ticks: -1,
                route_template_id: 0,
                bracket_template_id: 0,
                slip_cap_bp: 6,
                lat_budget_us: 900,
                ttl_ms: 900,
                priority: 10,
                trig_mask: 0b0011_1111,
                trig_param: 3,
                spare: 0,
            },
            Play {
                enable: true,
                dir_sell: true,
                anchor: 3,
                order_type: 0,
                tif: 1,
                sym_id: 18,
                qty: 120,
                px_ticks: 1,
                route_template_id: 0,
                bracket_template_id: 0,
                slip_cap_bp: 6,
                lat_budget_us: 900,
                ttl_ms: 900,
                priority: 9,
                trig_mask: 0b0011_1111,
                trig_param: 3,
                spare: 0,
            },
            Play {
                enable: true,
                dir_sell: false,
                anchor: 3,
                order_type: 1,
                tif: 0,
                sym_id: 42,
                qty: 60,
                px_ticks: 0,
                route_template_id: 2,
                bracket_template_id: 1,
                slip_cap_bp: 10,
                lat_budget_us: 700,
                ttl_ms: 300,
                priority: 6,
                trig_mask: 0b0001_1111,
                trig_param: 12,
                spare: 0,
            },
            Play {
                enable: true,
                dir_sell: true,
                anchor: 1,
                order_type: 1,
                tif: 0,
                sym_id: 43,
                qty: 60,
                px_ticks: 0,
                route_template_id: 3,
                bracket_template_id: 1,
                slip_cap_bp: 10,
                lat_budget_us: 700,
                ttl_ms: 300,
                priority: 5,
                trig_mask: 0b0001_1111,
                trig_param: 12,
                spare: 0,
            },
        ];
        draft.plays = plays;
        draft
    }

    fn sort_indices_by_priority(indices: &mut [usize; PLAY_COUNT], snapshot: &PexSnapshot) {
        for i in 0..PLAY_COUNT {
            let mut best = i;
            for j in (i + 1)..PLAY_COUNT {
                let jp = snapshot.play(indices[j]).priority;
                let bp = snapshot.play(indices[best]).priority;
                if jp > bp {
                    best = j;
                }
            }
            if best != i {
                indices.swap(i, best);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::TailDefaults;

        #[test]
        fn writer_advances_versions_and_seq() {
            let capsule = PexCapsule::new();
            let mut writer = PexWriter::new(&capsule);
            writer.draft_mut().header.account_id = 1;
            writer.draft_mut().defaults = TailDefaults {
                slip_cap_default_bp: 4,
                lat_budget_default_us: 900,
                router_hints: 0,
            };
            writer.publish();
            writer.publish();
            let stats = writer.stats();
            assert_eq!(stats.publishes, 2);
            let snapshot = capsule.load_snapshot().expect("snapshot");
            assert_eq!(snapshot.header().seq_head, 1);
            assert!(snapshot.header().ver_even & 1 == 0);
        }

        #[test]
        fn router_iterates_in_priority_order() {
            let capsule = PexCapsule::new();
            let mut writer = PexWriter::new(&capsule);
            let draft = writer.draft_mut();
            draft.plays[0].enable = true;
            draft.plays[0].priority = 1;
            draft.plays[1].enable = true;
            draft.plays[1].priority = 15;
            draft.plays[2].enable = true;
            draft.plays[2].priority = 7;
            draft.plays[3].enable = true;
            draft.plays[3].priority = 3;
            writer.publish();

            let mut router = PexRouter::new(&capsule);
            let mut visited = [0u8; PLAY_COUNT];
            let mut idx = 0usize;
            router.for_each_play(|_lane, play, _snapshot| {
                visited[idx] = play.priority;
                idx += 1;
                false
            });
            assert_eq!(visited[0], 15);
            assert_eq!(visited[1], 7);
            assert_eq!(visited[2], 3);
        }

        #[test]
        fn stale_capsule_skips_router_iteration() {
            let capsule = PexCapsule::new();
            let mut writer = PexWriter::new(&capsule);
            writer.publish();
            writer.mark_stale();
            let mut router = PexRouter::new(&capsule);
            router.for_each_play(|_, _, _| panic!("unexpected play iteration"));
            assert_eq!(router.stats().accepted_snapshots, 0);
        }
    }
}

/// Capsule-level header values outside of commit/stale/version sequencing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    pub account_id: u16,
    pub created_ms_coarse: u32,
    pub default_ttl_ms: u16,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub global_flags: u8,
    pub portfolio_breaker_level: u8,
    pub symbol_count: u8,
    pub play_mask_override: Option<u8>,
}

/// Per-play staged specification matching the 128-bit packed layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Play {
    pub enable: bool,
    pub dir_sell: bool,
    pub anchor: u8,
    pub order_type: u8,
    pub tif: u8,
    pub sym_id: u16,
    pub qty: u32,
    pub px_ticks: i16,
    pub route_template_id: u8,
    pub bracket_template_id: u8,
    pub slip_cap_bp: u16,
    pub lat_budget_us: u16,
    pub ttl_ms: u16,
    pub priority: u8,
    pub trig_mask: u16,
    pub trig_param: u8,
    pub spare: u8,
}

/// Bracket template lane (two slots in W5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BracketTemplate {
    pub tp_ticks: i16,
    pub sl_ticks: i16,
    pub trail_ticks: i16,
    pub tstop_ms: u16,
    pub exit_tif: u8,
    pub scale_out_pct: u8,
    pub flags: u8,
}

/// Route template lane (four slots in W6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RouteTemplate {
    pub route_id: u16,
    pub maker_taker: bool,
    pub ioc_fok: u8,
    pub post_only: bool,
    pub allow_partial: bool,
    pub slip_cap_bp: u16,
}

/// Tail defaults shared by router (W7 fields outside integrity tokens).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TailDefaults {
    pub slip_cap_default_bp: u16,
    pub lat_budget_default_us: u16,
    pub router_hints: u8,
}

/// Draft assembly for a full PEX-1024 publish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PexDraft {
    pub commit: bool,
    pub stale: bool,
    pub odd_version: u8,
    pub seq: u16,
    pub header: Header,
    pub plays: [Play; PLAY_COUNT],
    pub bracket_templates: [BracketTemplate; BRACKET_TEMPLATE_COUNT],
    pub route_templates: [RouteTemplate; ROUTE_TEMPLATE_COUNT],
    pub defaults: TailDefaults,
}

impl Default for PexDraft {
    fn default() -> Self {
        Self {
            commit: true,
            stale: false,
            odd_version: 1,
            seq: 0,
            header: Header::default(),
            plays: [Play::default(); PLAY_COUNT],
            bracket_templates: [BracketTemplate::default(); BRACKET_TEMPLATE_COUNT],
            route_templates: [RouteTemplate::default(); ROUTE_TEMPLATE_COUNT],
            defaults: TailDefaults::default(),
        }
    }
}

/// Eight contiguous 128-bit words holding the live capsule.
#[repr(C, align(64))]
pub struct PexCapsule {
    words: [AtomicU128; WORD_COUNT],
}

impl PexCapsule {
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

    #[inline]
    fn word(&self, idx: usize) -> &AtomicU128 {
        &self.words[idx]
    }

    /// Stage the provided draft into the capsule, flipping W0 with a release store.
    pub fn publish(&self, draft: &PexDraft) -> PexWords {
        let assembled = assemble_words(draft);
        for idx in 1..7 {
            self.word(idx)
                .store(assembled.words[idx], Ordering::Relaxed);
        }
        self.word(7).store(assembled.words[7], Ordering::Relaxed);
        self.word(0).store(assembled.words[0], Ordering::Release);
        assembled
    }

    /// Mark the capsule stale without touching payload words (Release store on W0).
    pub fn mark_stale(&self) {
        let mut head = self.word(0).load(Ordering::Relaxed);
        head = W0_STALE.set(head, 1);
        head = W0_COMMIT.set(head, 1);
        self.word(0).store(head, Ordering::Release);
    }

    /// Attempt to read a coherent snapshot using the accept rule.
    pub fn load_snapshot(&self) -> Option<PexSnapshot> {
        for _ in 0..ATTEMPTS {
            let head = self.word(0).load(Ordering::Acquire);
            if W0_COMMIT.get(head) == 0 || W0_STALE.get(head) != 0 {
                return None;
            }
            let ver_even = W0_VER_EVEN.get(head) as u8;
            if ver_even & 1 != 0 {
                return None;
            }
            let mut words = [0u128; WORD_COUNT];
            words[0] = head;
            for idx in 1..WORD_COUNT {
                words[idx] = self.word(idx).load(Ordering::Relaxed);
            }
            let tail = words[7];
            let checksum = checksum16(&words[1..7]);
            if checksum != W7_CHECKSUM.get(tail) as u16 {
                continue;
            }
            if ver_even != W7_VER_TAIL.get(tail) as u8 {
                continue;
            }
            if W0_SEQ_HEAD.get(head) as u16 != W7_SEQ_TAIL.get(tail) as u16 {
                continue;
            }
            let verify = self.word(0).load(Ordering::Acquire);
            if verify != head {
                continue;
            }
            return Some(PexSnapshot { words });
        }
        None
    }
}

impl Default for PexCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Raw words returned during publish or by a reader snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PexWords {
    words: [u128; WORD_COUNT],
}

impl PexWords {
    pub const fn words(&self) -> &[u128; WORD_COUNT] {
        &self.words
    }

    pub const fn header_word(&self) -> u128 {
        self.words[0]
    }

    pub const fn play_word(&self, idx: usize) -> u128 {
        self.words[1 + idx]
    }

    pub const fn bracket_word(&self) -> u128 {
        self.words[5]
    }

    pub const fn route_word(&self) -> u128 {
        self.words[6]
    }

    pub const fn tail_word(&self) -> u128 {
        self.words[7]
    }
}

/// Fully decoded snapshot view for downstream router logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PexSnapshot {
    words: [u128; WORD_COUNT],
}

impl PexSnapshot {
    pub const fn words(&self) -> &[u128; WORD_COUNT] {
        &self.words
    }

    pub fn header(&self) -> HeaderView {
        let word = self.words[0];
        HeaderView {
            commit: W0_COMMIT.get(word) != 0,
            stale: W0_STALE.get(word) != 0,
            ver_even: W0_VER_EVEN.get(word) as u8,
            seq_head: W0_SEQ_HEAD.get(word) as u16,
            account_id: W0_ACCOUNT_ID.get(word) as u16,
            created_ms_coarse: W0_CREATED_MS_COARSE.get(word) as u32,
            default_ttl_ms: W0_DEFAULT_TTL_MS.get(word) as u16,
            forbid_after_min_ct: W0_FORBID_AFTER_MIN_CT.get(word) as u16,
            eod_flat_min_ct: W0_EOD_FLAT_MIN_CT.get(word) as u16,
            play_mask: W0_PLAY_MASK.get(word) as u8,
            global_flags: W0_GLOBAL_FLAGS.get(word) as u8,
            portfolio_breaker_level: W0_PORTFOLIO_BREAKER_LEVEL.get(word) as u8,
            symbol_count: W0_SYMBOL_COUNT.get(word) as u8,
        }
    }

    pub fn play(&self, idx: usize) -> PlayView {
        let word = self.words[1 + idx];
        PlayView {
            enable: PLAY_ENABLE.get(word) != 0,
            dir_sell: PLAY_DIR.get(word) != 0,
            anchor: PLAY_ANCHOR.get(word) as u8,
            order_type: PLAY_ORDER_TYPE.get(word) as u8,
            tif: PLAY_TIF.get(word) as u8,
            sym_id: PLAY_SYM_ID.get(word) as u16,
            qty: PLAY_QTY.get(word) as u32,
            px_ticks: PLAY_PX_TICKS.get_signed(word) as i16,
            route_template_id: PLAY_ROUTE_TMPL_ID.get(word) as u8,
            bracket_template_id: PLAY_BRACKET_TMPL_ID.get(word) as u8,
            slip_cap_bp: PLAY_SLIP_CAP_BP.get(word) as u16,
            lat_budget_us: PLAY_LAT_BUDGET_US.get(word) as u16,
            ttl_ms: PLAY_TTL_MS.get(word) as u16,
            priority: PLAY_PRIORITY.get(word) as u8,
            trig_mask: PLAY_TRIG_MASK.get(word) as u16,
            trig_param: PLAY_TRIG_PARAM.get(word) as u8,
            spare: PLAY_SPARE.get(word) as u8,
        }
    }

    pub fn bracket_template(&self, idx: usize) -> BracketTemplateView {
        let word = self.words[5];
        match idx {
            0 => BracketTemplateView {
                tp_ticks: W5_B0_TP_TICKS.get_signed(word) as i16,
                sl_ticks: W5_B0_SL_TICKS.get_signed(word) as i16,
                trail_ticks: W5_B0_TRAIL_TICKS.get_signed(word) as i16,
                tstop_ms: W5_B0_TSTOP_MS.get(word) as u16,
                exit_tif: W5_B0_EXIT_TIF.get(word) as u8,
                scale_out_pct: W5_B0_SCALE_OUT_PCT.get(word) as u8,
                flags: W5_B0_FLAGS.get(word) as u8,
            },
            1 => BracketTemplateView {
                tp_ticks: W5_B1_TP_TICKS.get_signed(word) as i16,
                sl_ticks: W5_B1_SL_TICKS.get_signed(word) as i16,
                trail_ticks: W5_B1_TRAIL_TICKS.get_signed(word) as i16,
                tstop_ms: W5_B1_TSTOP_MS.get(word) as u16,
                exit_tif: W5_B1_EXIT_TIF.get(word) as u8,
                scale_out_pct: W5_B1_SCALE_OUT_PCT.get(word) as u8,
                flags: W5_B1_FLAGS.get(word) as u8,
            },
            _ => panic!("bracket idx out of range"),
        }
    }

    pub fn route_template(&self, idx: usize) -> RouteTemplateView {
        let word = self.words[6];
        let base = (idx as u32) * ROUTE_STRIDE;
        RouteTemplateView {
            route_id: Field {
                shift: base + ROUTE_ROUTE_ID.shift,
                bits: ROUTE_ROUTE_ID.bits,
            }
            .get(word) as u16,
            maker_taker: Field {
                shift: base + ROUTE_MAKER_TAKER.shift,
                bits: ROUTE_MAKER_TAKER.bits,
            }
            .get(word)
                != 0,
            ioc_fok: Field {
                shift: base + ROUTE_IOC_FOK.shift,
                bits: ROUTE_IOC_FOK.bits,
            }
            .get(word) as u8,
            post_only: Field {
                shift: base + ROUTE_POST_ONLY.shift,
                bits: ROUTE_POST_ONLY.bits,
            }
            .get(word)
                != 0,
            allow_partial: Field {
                shift: base + ROUTE_ALLOW_PARTIAL.shift,
                bits: ROUTE_ALLOW_PARTIAL.bits,
            }
            .get(word)
                != 0,
            slip_cap_bp: Field {
                shift: base + ROUTE_SLIP_CAP_BP.shift,
                bits: ROUTE_SLIP_CAP_BP.bits,
            }
            .get(word) as u16,
        }
    }

    pub fn tail(&self) -> TailView {
        let word = self.words[7];
        TailView {
            checksum: W7_CHECKSUM.get(word) as u16,
            ver_tail: W7_VER_TAIL.get(word) as u8,
            seq_tail: W7_SEQ_TAIL.get(word) as u16,
            slip_cap_default_bp: W7_SLIP_CAP_DEFAULT_BP.get(word) as u16,
            lat_budget_default_us: W7_LAT_BUDGET_DEFAULT_US.get(word) as u16,
            router_hints: W7_ROUTER_HINTS.get(word) as u8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderView {
    pub commit: bool,
    pub stale: bool,
    pub ver_even: u8,
    pub seq_head: u16,
    pub account_id: u16,
    pub created_ms_coarse: u32,
    pub default_ttl_ms: u16,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub play_mask: u8,
    pub global_flags: u8,
    pub portfolio_breaker_level: u8,
    pub symbol_count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayView {
    pub enable: bool,
    pub dir_sell: bool,
    pub anchor: u8,
    pub order_type: u8,
    pub tif: u8,
    pub sym_id: u16,
    pub qty: u32,
    pub px_ticks: i16,
    pub route_template_id: u8,
    pub bracket_template_id: u8,
    pub slip_cap_bp: u16,
    pub lat_budget_us: u16,
    pub ttl_ms: u16,
    pub priority: u8,
    pub trig_mask: u16,
    pub trig_param: u8,
    pub spare: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BracketTemplateView {
    pub tp_ticks: i16,
    pub sl_ticks: i16,
    pub trail_ticks: i16,
    pub tstop_ms: u16,
    pub exit_tif: u8,
    pub scale_out_pct: u8,
    pub flags: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteTemplateView {
    pub route_id: u16,
    pub maker_taker: bool,
    pub ioc_fok: u8,
    pub post_only: bool,
    pub allow_partial: bool,
    pub slip_cap_bp: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TailView {
    pub checksum: u16,
    pub ver_tail: u8,
    pub seq_tail: u16,
    pub slip_cap_default_bp: u16,
    pub lat_budget_default_us: u16,
    pub router_hints: u8,
}

fn assemble_words(draft: &PexDraft) -> PexWords {
    debug_assert_eq!(draft.odd_version & 1, 1, "odd version required");

    let ver_even = draft.odd_version.wrapping_add(1);
    let mut words = [0u128; WORD_COUNT];

    for (idx, play) in draft.plays.iter().enumerate() {
        words[1 + idx] = encode_play(play);
    }

    words[5] = encode_bracket_templates(&draft.bracket_templates);
    words[6] = encode_route_templates(&draft.route_templates);

    let checksum = checksum16(&words[1..7]);
    words[7] = encode_tail(checksum, ver_even, draft.seq, &draft.defaults);

    let play_mask = draft
        .header
        .play_mask_override
        .unwrap_or_else(|| compute_play_mask(&draft.plays));

    words[0] = encode_header(
        draft.commit,
        draft.stale,
        ver_even,
        draft.seq,
        play_mask,
        &draft.header,
    );

    PexWords { words }
}

fn encode_header(
    commit: bool,
    stale: bool,
    ver_even: u8,
    seq: u16,
    play_mask: u8,
    header: &Header,
) -> u128 {
    debug_assert!(header.portfolio_breaker_level < 4);
    debug_assert!(header.symbol_count < 16);
    let mut word = 0u128;
    word = W0_COMMIT.set(word, commit as u128);
    word = W0_STALE.set(word, stale as u128);
    word = W0_VER_EVEN.set(word, ver_even as u128);
    word = W0_SEQ_HEAD.set(word, seq as u128);
    word = W0_ACCOUNT_ID.set(word, header.account_id as u128);
    word = W0_CREATED_MS_COARSE.set(word, header.created_ms_coarse as u128);
    word = W0_DEFAULT_TTL_MS.set(word, header.default_ttl_ms as u128);
    word = W0_FORBID_AFTER_MIN_CT.set(word, header.forbid_after_min_ct as u128);
    word = W0_EOD_FLAT_MIN_CT.set(word, header.eod_flat_min_ct as u128);
    word = W0_PLAY_MASK.set(word, (play_mask & 0x0f) as u128);
    word = W0_GLOBAL_FLAGS.set(word, header.global_flags as u128);
    word = W0_PORTFOLIO_BREAKER_LEVEL.set(word, header.portfolio_breaker_level as u128);
    word = W0_SYMBOL_COUNT.set(word, header.symbol_count as u128);
    word = W0_SPARE.set(word, 0);
    word = W0_RESERVED.set(word, 0);
    word
}

fn encode_play(play: &Play) -> u128 {
    debug_assert!(play.anchor < 4);
    debug_assert!(play.order_type < 8);
    debug_assert!(play.tif < 8);
    debug_assert!(play.sym_id < (1 << 12));
    debug_assert!(play.qty < (1 << 18));
    debug_assert!(play.route_template_id < 8);
    debug_assert!(play.bracket_template_id < 8);
    debug_assert!(play.slip_cap_bp < (1 << 10));
    debug_assert!(play.lat_budget_us < (1 << 10));
    debug_assert!(play.ttl_ms < (1 << 10));
    debug_assert!(play.priority < (1 << 6));
    let mut word = 0u128;
    word = PLAY_ENABLE.set(word, play.enable as u128);
    word = PLAY_DIR.set(word, play.dir_sell as u128);
    word = PLAY_ANCHOR.set(word, play.anchor as u128);
    word = PLAY_ORDER_TYPE.set(word, play.order_type as u128);
    word = PLAY_TIF.set(word, play.tif as u128);
    word = PLAY_SYM_ID.set(word, play.sym_id as u128);
    word = PLAY_QTY.set(word, play.qty as u128);
    word = PLAY_PX_TICKS.set_signed(word, play.px_ticks as i32);
    word = PLAY_ROUTE_TMPL_ID.set(word, play.route_template_id as u128);
    word = PLAY_BRACKET_TMPL_ID.set(word, play.bracket_template_id as u128);
    word = PLAY_SLIP_CAP_BP.set(word, play.slip_cap_bp as u128);
    word = PLAY_LAT_BUDGET_US.set(word, play.lat_budget_us as u128);
    word = PLAY_TTL_MS.set(word, play.ttl_ms as u128);
    word = PLAY_PRIORITY.set(word, play.priority as u128);
    word = PLAY_TRIG_MASK.set(word, play.trig_mask as u128);
    word = PLAY_TRIG_PARAM.set(word, play.trig_param as u128);
    word = PLAY_SPARE.set(word, play.spare as u128);
    word
}

fn encode_bracket_templates(templates: &[BracketTemplate; BRACKET_TEMPLATE_COUNT]) -> u128 {
    let mut word = 0u128;
    let t0 = &templates[0];
    debug_assert!(t0.exit_tif < 4);
    debug_assert!(t0.scale_out_pct < (1 << 7));
    debug_assert!(t0.flags < (1 << 3));
    word = W5_B0_TP_TICKS.set_signed(word, t0.tp_ticks as i32);
    word = W5_B0_SL_TICKS.set_signed(word, t0.sl_ticks as i32);
    word = W5_B0_TRAIL_TICKS.set_signed(word, t0.trail_ticks as i32);
    word = W5_B0_TSTOP_MS.set(word, t0.tstop_ms as u128);
    word = W5_B0_EXIT_TIF.set(word, t0.exit_tif as u128);
    word = W5_B0_SCALE_OUT_PCT.set(word, t0.scale_out_pct as u128);
    word = W5_B0_FLAGS.set(word, t0.flags as u128);

    let t1 = &templates[1];
    debug_assert!(t1.exit_tif < 4);
    debug_assert!(t1.scale_out_pct < (1 << 7));
    debug_assert!(t1.flags < (1 << 3));
    word = W5_B1_TP_TICKS.set_signed(word, t1.tp_ticks as i32);
    word = W5_B1_SL_TICKS.set_signed(word, t1.sl_ticks as i32);
    word = W5_B1_TRAIL_TICKS.set_signed(word, t1.trail_ticks as i32);
    word = W5_B1_TSTOP_MS.set(word, t1.tstop_ms as u128);
    word = W5_B1_EXIT_TIF.set(word, t1.exit_tif as u128);
    word = W5_B1_SCALE_OUT_PCT.set(word, t1.scale_out_pct as u128);
    word = W5_B1_FLAGS.set(word, t1.flags as u128);

    word = W5_RESERVED.set(word, 0);
    word
}

fn encode_route_templates(templates: &[RouteTemplate; ROUTE_TEMPLATE_COUNT]) -> u128 {
    let mut word = 0u128;
    for (idx, tmpl) in templates.iter().enumerate() {
        debug_assert!(tmpl.route_id < (1 << 10));
        debug_assert!(tmpl.ioc_fok < 4);
        debug_assert!(tmpl.slip_cap_bp < (1 << 10));
        let base = (idx as u32) * ROUTE_STRIDE;
        let route_id_field = Field {
            shift: base + ROUTE_ROUTE_ID.shift,
            bits: ROUTE_ROUTE_ID.bits,
        };
        let maker_field = Field {
            shift: base + ROUTE_MAKER_TAKER.shift,
            bits: ROUTE_MAKER_TAKER.bits,
        };
        let ioc_field = Field {
            shift: base + ROUTE_IOC_FOK.shift,
            bits: ROUTE_IOC_FOK.bits,
        };
        let post_field = Field {
            shift: base + ROUTE_POST_ONLY.shift,
            bits: ROUTE_POST_ONLY.bits,
        };
        let allow_field = Field {
            shift: base + ROUTE_ALLOW_PARTIAL.shift,
            bits: ROUTE_ALLOW_PARTIAL.bits,
        };
        let slip_field = Field {
            shift: base + ROUTE_SLIP_CAP_BP.shift,
            bits: ROUTE_SLIP_CAP_BP.bits,
        };

        word = route_id_field.set(word, tmpl.route_id as u128);
        word = maker_field.set(word, tmpl.maker_taker as u128);
        word = ioc_field.set(word, tmpl.ioc_fok as u128);
        word = post_field.set(word, tmpl.post_only as u128);
        word = allow_field.set(word, tmpl.allow_partial as u128);
        word = slip_field.set(word, tmpl.slip_cap_bp as u128);
    }
    word
}

fn encode_tail(checksum: u16, ver_even: u8, seq: u16, defaults: &TailDefaults) -> u128 {
    debug_assert!(defaults.slip_cap_default_bp < (1 << 10));
    debug_assert!(defaults.lat_budget_default_us < (1 << 12));
    let mut word = 0u128;
    word = W7_CHECKSUM.set(word, checksum as u128);
    word = W7_VER_TAIL.set(word, ver_even as u128);
    word = W7_SEQ_TAIL.set(word, seq as u128);
    word = W7_SLIP_CAP_DEFAULT_BP.set(word, defaults.slip_cap_default_bp as u128);
    word = W7_LAT_BUDGET_DEFAULT_US.set(word, defaults.lat_budget_default_us as u128);
    word = W7_ROUTER_HINTS.set(word, defaults.router_hints as u128);
    word = W7_SPARE.set(word, 0);
    word
}

fn compute_play_mask(plays: &[Play; PLAY_COUNT]) -> u8 {
    let mut mask = 0u8;
    for (idx, play) in plays.iter().enumerate() {
        if play.enable {
            mask |= 1 << idx;
        }
    }
    mask
}

fn checksum16(words: &[u128]) -> u16 {
    let mut sum = 0u32;
    for &word in words {
        let mut value = word;
        for _ in 0..8 {
            sum = sum.wrapping_add((value & 0xffff) as u32);
            value >>= 16;
        }
    }
    // fold to 16 bits (ones' complement style)
    sum = (sum & 0xffff) + (sum >> 16);
    sum = (sum & 0xffff) + (sum >> 16);
    sum as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_draft() -> PexDraft {
        let plays = [
            Play {
                enable: true,
                dir_sell: false,
                anchor: 2, // BID1
                order_type: 0,
                tif: 1,
                sym_id: 17,
                qty: 120,
                px_ticks: -1,
                route_template_id: 0,
                bracket_template_id: 0,
                slip_cap_bp: 6,
                lat_budget_us: 900,
                ttl_ms: 900,
                priority: 10,
                trig_mask: 0b0011_1111,
                trig_param: 3,
                spare: 0,
            },
            Play {
                enable: true,
                dir_sell: true,
                anchor: 3, // ASK1
                order_type: 0,
                tif: 1,
                sym_id: 18,
                qty: 120,
                px_ticks: 1,
                route_template_id: 0,
                bracket_template_id: 0,
                slip_cap_bp: 6,
                lat_budget_us: 900,
                ttl_ms: 900,
                priority: 9,
                trig_mask: 0b0011_1111,
                trig_param: 3,
                spare: 0,
            },
            Play {
                enable: true,
                dir_sell: false,
                anchor: 3,
                order_type: 1,
                tif: 0,
                sym_id: 42,
                qty: 60,
                px_ticks: 0,
                route_template_id: 2,
                bracket_template_id: 1,
                slip_cap_bp: 10,
                lat_budget_us: 700,
                ttl_ms: 300,
                priority: 6,
                trig_mask: 0b0001_1111,
                trig_param: 12,
                spare: 0,
            },
            Play {
                enable: true,
                dir_sell: true,
                anchor: 1,
                order_type: 1,
                tif: 0,
                sym_id: 43,
                qty: 60,
                px_ticks: 0,
                route_template_id: 3,
                bracket_template_id: 1,
                slip_cap_bp: 10,
                lat_budget_us: 700,
                ttl_ms: 300,
                priority: 5,
                trig_mask: 0b0001_1111,
                trig_param: 12,
                spare: 0,
            },
        ];

        let brackets = [
            BracketTemplate {
                tp_ticks: 1,
                sl_ticks: -2,
                trail_ticks: 0,
                tstop_ms: 1500,
                exit_tif: 1,
                scale_out_pct: 0,
                flags: 0b001,
            },
            BracketTemplate {
                tp_ticks: 2,
                sl_ticks: -2,
                trail_ticks: 0,
                tstop_ms: 1000,
                exit_tif: 0,
                scale_out_pct: 0,
                flags: 0b010,
            },
        ];

        let routes = [
            RouteTemplate {
                route_id: 512,
                maker_taker: false,
                ioc_fok: 0,
                post_only: true,
                allow_partial: true,
                slip_cap_bp: 6,
            },
            RouteTemplate {
                route_id: 520,
                maker_taker: false,
                ioc_fok: 1,
                post_only: true,
                allow_partial: true,
                slip_cap_bp: 6,
            },
            RouteTemplate {
                route_id: 32,
                maker_taker: true,
                ioc_fok: 2,
                post_only: false,
                allow_partial: false,
                slip_cap_bp: 10,
            },
            RouteTemplate {
                route_id: 40,
                maker_taker: true,
                ioc_fok: 2,
                post_only: false,
                allow_partial: false,
                slip_cap_bp: 10,
            },
        ];

        PexDraft {
            commit: true,
            stale: false,
            odd_version: 5,
            seq: 42,
            header: Header {
                account_id: 77,
                created_ms_coarse: 1_234_567,
                default_ttl_ms: 1_500,
                forbid_after_min_ct: 120,
                eod_flat_min_ct: 90,
                global_flags: 0b0001_0001,
                portfolio_breaker_level: 1,
                symbol_count: 2,
                play_mask_override: None,
            },
            plays,
            bracket_templates: brackets,
            route_templates: routes,
            defaults: TailDefaults {
                slip_cap_default_bp: 6,
                lat_budget_default_us: 2_500,
                router_hints: 0b0000_0010,
            },
        }
    }

    #[test]
    fn encode_layout_matches_spec() {
        let draft = sample_draft();
        let words = assemble_words(&draft);
        let header = words.header_word();
        assert_eq!(W0_COMMIT.get(header), 1);
        assert_eq!(W0_STALE.get(header), 0);
        assert_eq!(W0_VER_EVEN.get(header), 6);
        assert_eq!(W0_SEQ_HEAD.get(header), 42);
        assert_eq!(W0_PLAY_MASK.get(header), 0b1111);

        let play0 = words.play_word(0);
        assert_eq!(PLAY_ENABLE.get(play0), 1);
        assert_eq!(PLAY_DIR.get(play0), 0);
        assert_eq!(PLAY_PX_TICKS.get_signed(play0), -1);

        let tail = words.tail_word();
        assert_eq!(W7_VER_TAIL.get(tail), 6);
        assert_eq!(W7_SEQ_TAIL.get(tail), 42);
        assert_eq!(W7_CHECKSUM.get(tail) as u16, checksum16(&words.words[1..7]));
    }

    #[test]
    fn publish_and_read_snapshot() {
        let draft = sample_draft();
        let capsule = PexCapsule::new();
        capsule.publish(&draft);
        let snapshot = capsule.load_snapshot().expect("snapshot");
        let header = snapshot.header();
        assert_eq!(header.ver_even, 6);
        assert_eq!(header.seq_head, 42);
        let play = snapshot.play(2);
        assert_eq!(play.route_template_id, 2);
        assert_eq!(play.trig_mask, 0b0001_1111);
    }

    #[test]
    fn stale_capsule_returns_none() {
        let draft = sample_draft();
        let capsule = PexCapsule::new();
        capsule.publish(&draft);
        capsule.mark_stale();
        assert!(capsule.load_snapshot().is_none());
    }
}
