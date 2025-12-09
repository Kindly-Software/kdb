#![no_std]

//! AEB-512 (Atomic Execution Bundle) packs an entry leg, paired exits, routing,
//! and risk/time budgets into a single 64-byte capsule. Writers stage the four
//! 128-bit words, flip the commit bit with a release store, and readers obtain a
//! coherent snapshot with one cache-line read.

use atomic_breaker::{breaker::State as BreakerState, AtomicBreakerSWeMR};
use core::sync::atomic::Ordering;
use portable_atomic::{AtomicU128, AtomicU64};

#[cfg(feature = "sim")]
extern crate alloc;
#[cfg(test)]
extern crate std;

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
}

fn set_field(word: u128, field: Field, value: u128) -> u128 {
    debug_assert_eq!(value & !field.value_mask(), 0, "value exceeds field width");
    let cleared = word & !field.mask();
    cleared | ((value & field.value_mask()) << field.shift)
}

fn set_signed_field(word: u128, field: Field, value: i32) -> u128 {
    debug_assert!(
        field.bits > 0 && field.bits <= 32,
        "signed field width invalid"
    );
    let bits = field.bits;
    let min = -(1i64 << (bits - 1));
    let max = (1i64 << (bits - 1)) - 1;
    let value64 = value as i64;
    debug_assert!(
        value64 >= min && value64 <= max,
        "signed value exceeds field width"
    );
    let mask = (1i64 << bits) - 1;
    let encoded = value64 & mask;
    set_field(word, field, encoded as u128)
}

fn get_field(word: u128, field: Field) -> u128 {
    (word >> field.shift) & field.value_mask()
}

fn get_signed_field(word: u128, field: Field) -> i32 {
    debug_assert!(
        field.bits > 0 && field.bits <= 32,
        "signed field width invalid"
    );
    let raw = get_field(word, field) as u32;
    let shift = 32 - field.bits;
    ((raw << shift) as i32) >> shift
}

const W0_COMMIT: Field = Field { shift: 0, bits: 1 };
const W0_STALE: Field = Field { shift: 1, bits: 1 };
const W0_VER: Field = Field { shift: 2, bits: 8 };
const W0_SEQ: Field = Field {
    shift: 10,
    bits: 16,
};
const W0_SYMBOL_ID: Field = Field {
    shift: 26,
    bits: 16,
};
const W0_STRATEGY_ID: Field = Field { shift: 42, bits: 8 };
const W0_ACCOUNT_ID: Field = Field {
    shift: 50,
    bits: 16,
};
const W0_PAIR_ID: Field = Field {
    shift: 66,
    bits: 12,
};
const W0_CREATED_MS_COARSE: Field = Field {
    shift: 78,
    bits: 24,
};
const W0_TTL_MS: Field = Field {
    shift: 102,
    bits: 16,
};
const W0_HDR_STATE: Field = Field {
    shift: 118,
    bits: 2,
};
const W0_HDR_KIND: Field = Field {
    shift: 120,
    bits: 2,
};
const W0_HDR_HAS_BRACKET: Field = Field {
    shift: 122,
    bits: 1,
};
const W0_HDR_REDUCE_ONLY: Field = Field {
    shift: 123,
    bits: 1,
};
const W0_HDR_SPARE: Field = Field {
    shift: 124,
    bits: 4,
};

const W1_SIDE: Field = Field { shift: 0, bits: 1 };
const W1_ANCHOR: Field = Field { shift: 1, bits: 2 };
const W1_ORDER_TYPE: Field = Field { shift: 3, bits: 3 };
const W1_TIF: Field = Field { shift: 6, bits: 3 };
const W1_QTY: Field = Field { shift: 9, bits: 24 };
const W1_PX_TICKS: Field = Field {
    shift: 33,
    bits: 24,
};
const W1_ROUTE_ID: Field = Field {
    shift: 57,
    bits: 10,
};
const W1_SLIP_CAP_BP: Field = Field {
    shift: 67,
    bits: 12,
};
const W1_POST_ONLY: Field = Field { shift: 79, bits: 1 };
const W1_REDUCE_ONLY: Field = Field { shift: 80, bits: 1 };
const W1_ALLOW_PARTIAL: Field = Field { shift: 81, bits: 1 };
const W1_RISK_TAG: Field = Field {
    shift: 82,
    bits: 10,
};
const W1_SEQ: Field = Field {
    shift: 92,
    bits: 24,
};
const W1_SPARE: Field = Field {
    shift: 116,
    bits: 12,
};

const W2_TP_TICKS: Field = Field { shift: 0, bits: 12 };
const W2_SL_TICKS: Field = Field {
    shift: 12,
    bits: 12,
};
const W2_TRAIL_TICKS: Field = Field {
    shift: 24,
    bits: 12,
};
const W2_TSTOP_MS: Field = Field {
    shift: 36,
    bits: 14,
};
const W2_EXIT_ROUTE: Field = Field {
    shift: 50,
    bits: 10,
};
const W2_EXIT_TIF: Field = Field { shift: 60, bits: 3 };
const W2_TP_KIND: Field = Field { shift: 63, bits: 2 };
const W2_SL_KIND: Field = Field { shift: 65, bits: 2 };
const W2_REARM: Field = Field { shift: 67, bits: 1 };
const W2_SCALE_OUT: Field = Field { shift: 68, bits: 8 };
const W2_SLIP_CAP_EXIT: Field = Field {
    shift: 76,
    bits: 12,
};
const W2_LAT_BUDGET_US: Field = Field {
    shift: 88,
    bits: 12,
};
const W2_FLAGS: Field = Field {
    shift: 100,
    bits: 8,
};
const W2_OCO_GROUP: Field = Field {
    shift: 108,
    bits: 12,
};
const W2_SPARE: Field = Field {
    shift: 120,
    bits: 8,
};

const W3_MAX_OPEN_MS: Field = Field { shift: 0, bits: 20 };
const W3_MAX_ADVERSE_CENTS: Field = Field {
    shift: 20,
    bits: 24,
};
const W3_EXIT_ON_BREAKER: Field = Field { shift: 44, bits: 2 };
const W3_EXIT_ON_JITTER: Field = Field { shift: 46, bits: 1 };
const W3_EXIT_ON_COST: Field = Field { shift: 47, bits: 1 };
const W3_FORBID_AFTER_MIN: Field = Field {
    shift: 48,
    bits: 11,
};
const W3_EOD_FLAT_MIN: Field = Field {
    shift: 59,
    bits: 11,
};
const W3_ROUTE_B: Field = Field {
    shift: 70,
    bits: 10,
};
const W3_ON_FAIL: Field = Field { shift: 80, bits: 3 };
const W3_CHECKSUM: Field = Field {
    shift: 83,
    bits: 16,
};
const W3_VER_TAIL: Field = Field { shift: 99, bits: 8 };
const W3_SEQ_TAIL: Field = Field {
    shift: 107,
    bits: 16,
};
const W3_SPARE: Field = Field {
    shift: 123,
    bits: 5,
};

const COARSE_WRAP: u32 = 1 << W0_CREATED_MS_COARSE.bits;
const COARSE_MASK: u32 = COARSE_WRAP - 1;

// Network extension fields using spare bit allocations
// #ASSUME: Network fields use spare bits without breaking existing layout
// #VERIFY: All existing tests pass with network feature disabled
#[cfg(feature = "network")]
const W0_SEND_TIMESTAMP: Field = Field {
    shift: 126, // Use 2 bits from W0_HDR_SPARE (4 bits available, keep 2 as spare)
    bits: 2,    // Limited to 2 bits for timestamp (0-3 range)
};
#[cfg(feature = "network")]
const W0_VENUE_SESSION_ID: Field = Field {
    shift: 116, // Use part of W1_SPARE space (12 bits available)
    bits: 8,
};
#[cfg(feature = "network")]
const W1_NETWORK_ROUTE: Field = Field {
    shift: 124, // Use another part of W1_SPARE space
    bits: 4,    // Limit to 4 bits for route (0-15 range)
};

const _: () = {
    assert!(W0_HDR_SPARE.shift + W0_HDR_SPARE.bits <= 128);
    assert!(W1_SPARE.shift + W1_SPARE.bits <= 128);
    assert!(W2_SPARE.shift + W2_SPARE.bits <= 128);
    assert!(W3_SPARE.shift + W3_SPARE.bits <= 128);
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeaderWord {
    pub stale: bool,
    pub state: u8,
    pub kind: u8,
    pub has_bracket: bool,
    pub reduce_only_bundle: bool,
    pub spare_flags: u8,
    pub symbol_id: u16,
    pub strategy_id: u8,
    pub account_id: u16,
    pub pair_id: u16,
    pub created_ms_coarse: u32,
    pub ttl_ms: u16,
    /// Network timestamp when order was sent (optional, requires 'network' feature)
    /// Uses 16 bits from venue_session_id allocation
    #[cfg(feature = "network")]
    pub send_timestamp: u16,
    /// Venue session identifier (optional, requires 'network' feature)
    /// Uses remaining spare_flags bits
    #[cfg(feature = "network")]
    pub venue_session_id: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryLegWord {
    pub side_is_buy: bool,
    pub anchor: u8,
    pub order_type: u8,
    pub tif: u8,
    pub quantity: u32,
    pub price_ticks: i32,
    pub route_id: u16,
    pub slip_cap_bp: u16,
    pub post_only: bool,
    pub reduce_only: bool,
    pub allow_partial: bool,
    pub risk_tag: u16,
    pub seq_hint: u32,
    /// Network route identifier for order routing (optional, requires 'network' feature)
    /// Uses 8 bits from W1_SPARE allocation
    #[cfg(feature = "network")]
    pub network_route: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BracketsWord {
    pub take_profit_ticks: i16,
    pub stop_loss_ticks: i16,
    pub trailing_ticks: i16,
    pub time_stop_ms: u16,
    pub exit_route_id: u16,
    pub exit_tif: u8,
    pub take_profit_kind: u8,
    pub stop_loss_kind: u8,
    pub rearm_on_reentry: bool,
    pub scale_out_pct: u8,
    pub slip_cap_exit_bp: u16,
    pub latency_budget_us: u16,
    pub flags: u8,
    pub oco_group: u16,
    pub spare: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RiskWord {
    pub max_open_ms: u32,
    pub max_adverse_cents: u32,
    pub exit_on_breaker_ge_level: u8,
    pub exit_on_jitter: bool,
    pub exit_on_cost_gt: bool,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub fallback_route_id: u16,
    pub on_fail: u8,
    pub spare: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(align(64))]
pub struct BundleDraft {
    words: [u128; 4],
}

impl BundleDraft {
    pub const fn new() -> Self {
        Self { words: [0; 4] }
    }

    pub fn clear(&mut self) {
        self.words = [0; 4];
    }

    pub fn set_header(&mut self, header: HeaderWord) -> &mut Self {
        let mut word = 0u128;
        if header.stale {
            word = set_field(word, W0_STALE, 1);
        }
        debug_assert!(header.state < 4);
        debug_assert!(header.kind < 4);
        debug_assert!(header.spare_flags < 16);
        word = set_field(word, W0_HDR_STATE, header.state as u128);
        word = set_field(word, W0_HDR_KIND, header.kind as u128);
        if header.has_bracket {
            word = set_field(word, W0_HDR_HAS_BRACKET, 1);
        }
        if header.reduce_only_bundle {
            word = set_field(word, W0_HDR_REDUCE_ONLY, 1);
        }
        word = set_field(word, W0_HDR_SPARE, header.spare_flags as u128);
        #[cfg(feature = "network")]
        {
            word = set_field(word, W0_SEND_TIMESTAMP, header.send_timestamp as u128);
            word = set_field(word, W0_VENUE_SESSION_ID, header.venue_session_id as u128);
        }
        word = set_field(word, W0_SYMBOL_ID, header.symbol_id as u128);
        word = set_field(word, W0_STRATEGY_ID, header.strategy_id as u128);
        word = set_field(word, W0_ACCOUNT_ID, header.account_id as u128);
        debug_assert!((header.pair_id as u32) < (1u32 << W0_PAIR_ID.bits));
        word = set_field(word, W0_PAIR_ID, header.pair_id as u128);
        debug_assert!(header.created_ms_coarse < (1u32 << W0_CREATED_MS_COARSE.bits));
        word = set_field(word, W0_CREATED_MS_COARSE, header.created_ms_coarse as u128);
        word = set_field(word, W0_TTL_MS, header.ttl_ms as u128);
        self.words[0] = word;
        self
    }

    pub fn set_entry(&mut self, entry: EntryLegWord) -> &mut Self {
        let mut word = 0u128;
        if entry.side_is_buy {
            word = set_field(word, W1_SIDE, 1);
        }
        debug_assert!(entry.anchor < 4);
        debug_assert!(entry.order_type < 8);
        debug_assert!(entry.tif < 8);
        word = set_field(word, W1_ANCHOR, entry.anchor as u128);
        word = set_field(word, W1_ORDER_TYPE, entry.order_type as u128);
        word = set_field(word, W1_TIF, entry.tif as u128);
        debug_assert!(entry.quantity < (1u32 << W1_QTY.bits));
        word = set_field(word, W1_QTY, entry.quantity as u128);
        word = set_signed_field(word, W1_PX_TICKS, entry.price_ticks);
        debug_assert!((entry.route_id as u32) < (1u32 << W1_ROUTE_ID.bits));
        word = set_field(word, W1_ROUTE_ID, entry.route_id as u128);
        debug_assert!((entry.slip_cap_bp as u32) < (1u32 << W1_SLIP_CAP_BP.bits));
        word = set_field(word, W1_SLIP_CAP_BP, entry.slip_cap_bp as u128);
        if entry.post_only {
            word = set_field(word, W1_POST_ONLY, 1);
        }
        if entry.reduce_only {
            word = set_field(word, W1_REDUCE_ONLY, 1);
        }
        if entry.allow_partial {
            word = set_field(word, W1_ALLOW_PARTIAL, 1);
        }
        debug_assert!((entry.risk_tag as u32) < (1u32 << W1_RISK_TAG.bits));
        word = set_field(word, W1_RISK_TAG, entry.risk_tag as u128);
        debug_assert!(entry.seq_hint < (1u32 << W1_SEQ.bits));
        word = set_field(word, W1_SEQ, entry.seq_hint as u128);
        #[cfg(feature = "network")]
        {
            word = set_field(word, W1_NETWORK_ROUTE, entry.network_route as u128);
        }
        self.words[1] = set_field(word, W1_SPARE, 0);
        self
    }

    pub fn set_brackets(&mut self, brackets: BracketsWord) -> &mut Self {
        let mut word = 0u128;
        word = set_signed_field(word, W2_TP_TICKS, brackets.take_profit_ticks as i32);
        word = set_signed_field(word, W2_SL_TICKS, brackets.stop_loss_ticks as i32);
        word = set_signed_field(word, W2_TRAIL_TICKS, brackets.trailing_ticks as i32);
        debug_assert!((brackets.time_stop_ms as u32) < (1u32 << W2_TSTOP_MS.bits));
        word = set_field(word, W2_TSTOP_MS, brackets.time_stop_ms as u128);
        debug_assert!((brackets.exit_route_id as u32) < (1u32 << W2_EXIT_ROUTE.bits));
        word = set_field(word, W2_EXIT_ROUTE, brackets.exit_route_id as u128);
        debug_assert!((brackets.exit_tif as u32) < (1u32 << W2_EXIT_TIF.bits));
        word = set_field(word, W2_EXIT_TIF, brackets.exit_tif as u128);
        debug_assert!((brackets.take_profit_kind as u32) < (1u32 << W2_TP_KIND.bits));
        debug_assert!((brackets.stop_loss_kind as u32) < (1u32 << W2_SL_KIND.bits));
        word = set_field(word, W2_TP_KIND, brackets.take_profit_kind as u128);
        word = set_field(word, W2_SL_KIND, brackets.stop_loss_kind as u128);
        if brackets.rearm_on_reentry {
            word = set_field(word, W2_REARM, 1);
        }
        word = set_field(word, W2_SCALE_OUT, brackets.scale_out_pct as u128);
        debug_assert!((brackets.slip_cap_exit_bp as u32) < (1u32 << W2_SLIP_CAP_EXIT.bits));
        word = set_field(word, W2_SLIP_CAP_EXIT, brackets.slip_cap_exit_bp as u128);
        debug_assert!((brackets.latency_budget_us as u32) < (1u32 << W2_LAT_BUDGET_US.bits));
        word = set_field(word, W2_LAT_BUDGET_US, brackets.latency_budget_us as u128);
        word = set_field(word, W2_FLAGS, brackets.flags as u128);
        debug_assert!((brackets.oco_group as u32) < (1u32 << W2_OCO_GROUP.bits));
        word = set_field(word, W2_OCO_GROUP, brackets.oco_group as u128);
        word = set_field(word, W2_SPARE, brackets.spare as u128);
        self.words[2] = word;
        self
    }

    pub fn set_risk(&mut self, risk: RiskWord) -> &mut Self {
        let mut word = 0u128;
        debug_assert!(risk.max_open_ms < (1u32 << W3_MAX_OPEN_MS.bits));
        word = set_field(word, W3_MAX_OPEN_MS, risk.max_open_ms as u128);
        debug_assert!(risk.max_adverse_cents < (1u32 << W3_MAX_ADVERSE_CENTS.bits));
        word = set_field(word, W3_MAX_ADVERSE_CENTS, risk.max_adverse_cents as u128);
        debug_assert!((risk.exit_on_breaker_ge_level as u32) < (1u32 << W3_EXIT_ON_BREAKER.bits));
        word = set_field(
            word,
            W3_EXIT_ON_BREAKER,
            risk.exit_on_breaker_ge_level as u128,
        );
        if risk.exit_on_jitter {
            word = set_field(word, W3_EXIT_ON_JITTER, 1);
        }
        if risk.exit_on_cost_gt {
            word = set_field(word, W3_EXIT_ON_COST, 1);
        }
        debug_assert!((risk.forbid_after_min_ct as u32) < (1u32 << W3_FORBID_AFTER_MIN.bits));
        word = set_field(word, W3_FORBID_AFTER_MIN, risk.forbid_after_min_ct as u128);
        debug_assert!((risk.eod_flat_min_ct as u32) < (1u32 << W3_EOD_FLAT_MIN.bits));
        word = set_field(word, W3_EOD_FLAT_MIN, risk.eod_flat_min_ct as u128);
        debug_assert!((risk.fallback_route_id as u32) < (1u32 << W3_ROUTE_B.bits));
        word = set_field(word, W3_ROUTE_B, risk.fallback_route_id as u128);
        debug_assert!((risk.on_fail as u32) < (1u32 << W3_ON_FAIL.bits));
        word = set_field(word, W3_ON_FAIL, risk.on_fail as u128);
        debug_assert!((risk.spare as u32) < (1u32 << W3_SPARE.bits));
        word = set_field(word, W3_SPARE, risk.spare as u128);
        self.words[3] = set_field(word, W3_CHECKSUM, 0);
        self
    }
}

#[repr(align(64))]
pub struct AtomicExecutionBundle {
    words: [AtomicU128; 4],
    breaker: AtomicBreakerSWeMR,
}

impl Default for AtomicExecutionBundle {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicExecutionBundle {
    pub const fn new() -> Self {
        Self {
            words: [
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
            ],
            breaker: AtomicBreakerSWeMR::new(BreakerState::Closed),
        }
    }

    /// Create a new execution bundle with a specific breaker state.
    pub const fn with_breaker_state(state: BreakerState) -> Self {
        Self {
            words: [
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
            ],
            breaker: AtomicBreakerSWeMR::new(state),
        }
    }

    /// Access the breaker for external control and monitoring.
    #[inline]
    pub const fn breaker(&self) -> &AtomicBreakerSWeMR {
        &self.breaker
    }

    pub fn publish(
        &self,
        header: HeaderWord,
        entry: EntryLegWord,
        brackets: BracketsWord,
        risk: RiskWord,
    ) -> ExecutionResult {
        let mut draft = BundleDraft::new();
        draft.set_header(header);
        draft.set_entry(entry);
        draft.set_brackets(brackets);
        draft.set_risk(risk);
        self.publish_draft(&draft)
    }

    pub fn publish_with<F>(&self, mut builder: F) -> ExecutionResult
    where
        F: FnMut(&mut BundleDraft),
    {
        let mut draft = BundleDraft::new();
        builder(&mut draft);
        self.publish_draft(&draft)
    }

    pub fn publish_with_reuse<F>(&self, draft: &mut BundleDraft, mut builder: F) -> ExecutionResult
    where
        F: FnMut(&mut BundleDraft),
    {
        draft.clear();
        builder(draft);
        self.publish_draft(draft)
    }

    pub fn publish_draft(&self, draft: &BundleDraft) -> ExecutionResult {
        // #ASSUME: Breaker check is fast (<5ns) and lockfree
        // #VERIFY: No performance regression in benchmarks
        let breaker_state = self.breaker.state();
        if breaker_state == BreakerState::Open || breaker_state == BreakerState::ForcedOpen {
            return Err(ExecutionError::BreakerHalt);
        }
        let old_header = self.words[0].load(Ordering::Relaxed);
        let prev_ver = get_field(old_header, W0_VER) as u16;
        let prev_seq = get_field(old_header, W0_SEQ) as u16;

        let ver_mask = W0_VER.value_mask() as u16;
        let seq_mask = W0_SEQ.value_mask() as u16;

        let mut odd_ver = prev_ver.wrapping_add(1) & ver_mask;
        if odd_ver & 1 == 0 {
            odd_ver = odd_ver.wrapping_add(1) & ver_mask;
        }
        if odd_ver == 0 {
            odd_ver = 1;
        }
        let mut even_ver = odd_ver.wrapping_add(1) & ver_mask;
        if even_ver & 1 != 0 {
            even_ver = even_ver.wrapping_add(1) & ver_mask;
        }
        if even_ver == 0 {
            even_ver = 2;
        }

        let new_seq = prev_seq.wrapping_add(1) & seq_mask;

        let w0_base = draft.words[0];
        let w1 = draft.words[1];
        let w2 = draft.words[2];
        let mut w3 = draft.words[3];

        let mut w0_inflight = w0_base;
        w0_inflight = set_field(w0_inflight, W0_COMMIT, 0);
        w0_inflight = set_field(w0_inflight, W0_VER, odd_ver as u128);
        w0_inflight = set_field(w0_inflight, W0_SEQ, new_seq as u128);

        let mut w0_final = w0_base;
        w0_final = set_field(w0_final, W0_COMMIT, 1);
        w0_final = set_field(w0_final, W0_VER, even_ver as u128);
        w0_final = set_field(w0_final, W0_SEQ, new_seq as u128);

        w3 = set_field(w3, W3_VER_TAIL, even_ver as u128);
        w3 = set_field(w3, W3_SEQ_TAIL, new_seq as u128);

        #[cfg(feature = "checksum")]
        {
            w3 = set_field(w3, W3_CHECKSUM, 0);
            let checksum = checksum_words([w1, w2, w3]);
            w3 = set_field(w3, W3_CHECKSUM, checksum as u128);
        }

        #[cfg(not(feature = "checksum"))]
        {
            w3 = set_field(w3, W3_CHECKSUM, 0);
        }

        self.words[0].store(w0_inflight, Ordering::Relaxed);
        self.words[1].store(w1, Ordering::Relaxed);
        self.words[2].store(w2, Ordering::Relaxed);
        self.words[3].store(w3, Ordering::Relaxed);
        self.words[0].store(w0_final, Ordering::Release);

        Ok(Snapshot {
            words: [w0_final, w1, w2, w3],
        })
    }

    #[inline]
    fn load_fast(&self) -> Option<Snapshot> {
        for _ in 0..ATTEMPTS {
            let w0_first = self.words[0].load(Ordering::Relaxed);
            if get_field(w0_first, W0_COMMIT) == 0 {
                return None;
            }
            if get_field(w0_first, W0_VER) & 1 != 0 {
                continue;
            }

            let w0_second = self.words[0].load(Ordering::Acquire);
            if w0_first != w0_second {
                continue;
            }
            if get_field(w0_second, W0_STALE) != 0 {
                return None;
            }
            if get_field(w0_second, W0_COMMIT) == 0 {
                continue;
            }
            if get_field(w0_second, W0_VER) & 1 != 0 {
                continue;
            }

            let w1 = self.words[1].load(Ordering::Relaxed);
            let w2 = self.words[2].load(Ordering::Relaxed);
            let w3 = self.words[3].load(Ordering::Relaxed);

            let ver = get_field(w0_second, W0_VER) as u16;
            let seq = get_field(w0_second, W0_SEQ) as u16;
            let ver_tail = get_field(w3, W3_VER_TAIL) as u16;
            let seq_tail = get_field(w3, W3_SEQ_TAIL) as u16;
            if ver != ver_tail || seq != seq_tail {
                continue;
            }

            #[cfg(feature = "checksum")]
            {
                let stored_checksum = get_field(w3, W3_CHECKSUM) as u16;
                let w3_zero = set_field(w3, W3_CHECKSUM, 0);
                if checksum_words([w1, w2, w3_zero]) != stored_checksum {
                    continue;
                }
            }

            return Some(Snapshot {
                words: [w0_second, w1, w2, w3],
            });
        }
        None
    }

    pub fn load(&self) -> Option<Snapshot> {
        self.load_fast()
    }

    /// Attempt to load a committed snapshot while recording structured denial metrics.
    ///
    /// When `counters` is supplied, the method records accepts and the specific
    /// failure reasons observed (including exhausted retry loops).
    pub fn load_with_diagnostics(
        &self,
        counters: Option<&DenyCounters>,
    ) -> Result<Snapshot, DenyReason> {
        if let Some(counters) = counters {
            return self.load_with_counters(counters);
        }
        self.load_fast().ok_or(DenyReason::AttemptsExhausted)
    }

    fn load_with_counters(&self, counters: &DenyCounters) -> Result<Snapshot, DenyReason> {
        let mut last_reason = DenyReason::AttemptsExhausted;
        for _ in 0..ATTEMPTS {
            let w0_first = self.words[0].load(Ordering::Relaxed);
            if get_field(w0_first, W0_COMMIT) == 0 {
                let reason = DenyReason::NotCommitted;
                counters.record(reason);
                return Err(reason);
            }
            if get_field(w0_first, W0_VER) & 1 != 0 {
                last_reason = DenyReason::OddVersion;
                continue;
            }

            let w0_second = self.words[0].load(Ordering::Acquire);
            if w0_first != w0_second {
                last_reason = DenyReason::InFlightRewrite;
                continue;
            }
            if get_field(w0_second, W0_STALE) != 0 {
                let reason = DenyReason::Stale;
                counters.record(reason);
                return Err(reason);
            }
            if get_field(w0_second, W0_COMMIT) == 0 {
                last_reason = DenyReason::NotCommitted;
                continue;
            }
            if get_field(w0_second, W0_VER) & 1 != 0 {
                last_reason = DenyReason::OddVersion;
                continue;
            }

            let w1 = self.words[1].load(Ordering::Relaxed);
            let w2 = self.words[2].load(Ordering::Relaxed);
            let w3 = self.words[3].load(Ordering::Relaxed);

            let ver = get_field(w0_second, W0_VER) as u16;
            let seq = get_field(w0_second, W0_SEQ) as u16;
            let ver_tail = get_field(w3, W3_VER_TAIL) as u16;
            let seq_tail = get_field(w3, W3_SEQ_TAIL) as u16;
            if ver != ver_tail || seq != seq_tail {
                last_reason = DenyReason::SeqMismatch;
                continue;
            }

            #[cfg(feature = "checksum")]
            {
                let stored_checksum = get_field(w3, W3_CHECKSUM) as u16;
                let w3_zero = set_field(w3, W3_CHECKSUM, 0);
                if checksum_words([w1, w2, w3_zero]) != stored_checksum {
                    last_reason = DenyReason::ChecksumMismatch;
                    continue;
                }
            }

            counters.record_accept();
            return Ok(Snapshot {
                words: [w0_second, w1, w2, w3],
            });
        }
        if last_reason != DenyReason::AttemptsExhausted {
            counters.record(last_reason);
        }
        counters.record(DenyReason::AttemptsExhausted);
        Err(last_reason)
    }

    pub fn mark_stale(&self) {
        let mask = W0_STALE.mask();
        self.words[0].fetch_or(mask, Ordering::Release);
    }

    pub fn sequence_pair(&self) -> (u16, u16) {
        let head = get_field(self.words[0].load(Ordering::Relaxed), W0_SEQ) as u16;
        let tail = get_field(self.words[3].load(Ordering::Relaxed), W3_SEQ_TAIL) as u16;
        (head, tail)
    }

    #[inline]
    pub fn tail_sequence(&self) -> u16 {
        get_field(self.words[3].load(Ordering::Relaxed), W3_SEQ_TAIL) as u16
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(align(64))]
pub struct Snapshot {
    words: [u128; 4],
}

impl Snapshot {
    pub fn commit(&self) -> bool {
        get_field(self.words[0], W0_COMMIT) != 0
    }

    pub fn stale(&self) -> bool {
        get_field(self.words[0], W0_STALE) != 0
    }

    pub fn version(&self) -> u16 {
        get_field(self.words[0], W0_VER) as u16
    }

    pub fn sequence(&self) -> u16 {
        get_field(self.words[0], W0_SEQ) as u16
    }

    pub fn header(&self) -> HeaderWord {
        HeaderWord {
            stale: self.stale(),
            state: get_field(self.words[0], W0_HDR_STATE) as u8,
            kind: get_field(self.words[0], W0_HDR_KIND) as u8,
            has_bracket: get_field(self.words[0], W0_HDR_HAS_BRACKET) != 0,
            reduce_only_bundle: get_field(self.words[0], W0_HDR_REDUCE_ONLY) != 0,
            spare_flags: get_field(self.words[0], W0_HDR_SPARE) as u8,
            symbol_id: get_field(self.words[0], W0_SYMBOL_ID) as u16,
            strategy_id: get_field(self.words[0], W0_STRATEGY_ID) as u8,
            account_id: get_field(self.words[0], W0_ACCOUNT_ID) as u16,
            pair_id: get_field(self.words[0], W0_PAIR_ID) as u16,
            created_ms_coarse: get_field(self.words[0], W0_CREATED_MS_COARSE) as u32,
            ttl_ms: get_field(self.words[0], W0_TTL_MS) as u16,
            #[cfg(feature = "network")]
            send_timestamp: get_field(self.words[0], W0_SEND_TIMESTAMP) as u16,
            #[cfg(feature = "network")]
            venue_session_id: get_field(self.words[0], W0_VENUE_SESSION_ID) as u8,
        }
    }

    pub fn entry(&self) -> EntryLegWord {
        EntryLegWord {
            side_is_buy: get_field(self.words[1], W1_SIDE) != 0,
            anchor: get_field(self.words[1], W1_ANCHOR) as u8,
            order_type: get_field(self.words[1], W1_ORDER_TYPE) as u8,
            tif: get_field(self.words[1], W1_TIF) as u8,
            quantity: get_field(self.words[1], W1_QTY) as u32,
            price_ticks: get_signed_field(self.words[1], W1_PX_TICKS),
            route_id: get_field(self.words[1], W1_ROUTE_ID) as u16,
            slip_cap_bp: get_field(self.words[1], W1_SLIP_CAP_BP) as u16,
            post_only: get_field(self.words[1], W1_POST_ONLY) != 0,
            reduce_only: get_field(self.words[1], W1_REDUCE_ONLY) != 0,
            allow_partial: get_field(self.words[1], W1_ALLOW_PARTIAL) != 0,
            risk_tag: get_field(self.words[1], W1_RISK_TAG) as u16,
            seq_hint: get_field(self.words[1], W1_SEQ) as u32,
            #[cfg(feature = "network")]
            network_route: get_field(self.words[1], W1_NETWORK_ROUTE) as u8,
        }
    }

    pub fn brackets(&self) -> BracketsWord {
        BracketsWord {
            take_profit_ticks: get_signed_field(self.words[2], W2_TP_TICKS) as i16,
            stop_loss_ticks: get_signed_field(self.words[2], W2_SL_TICKS) as i16,
            trailing_ticks: get_signed_field(self.words[2], W2_TRAIL_TICKS) as i16,
            time_stop_ms: get_field(self.words[2], W2_TSTOP_MS) as u16,
            exit_route_id: get_field(self.words[2], W2_EXIT_ROUTE) as u16,
            exit_tif: get_field(self.words[2], W2_EXIT_TIF) as u8,
            take_profit_kind: get_field(self.words[2], W2_TP_KIND) as u8,
            stop_loss_kind: get_field(self.words[2], W2_SL_KIND) as u8,
            rearm_on_reentry: get_field(self.words[2], W2_REARM) != 0,
            scale_out_pct: get_field(self.words[2], W2_SCALE_OUT) as u8,
            slip_cap_exit_bp: get_field(self.words[2], W2_SLIP_CAP_EXIT) as u16,
            latency_budget_us: get_field(self.words[2], W2_LAT_BUDGET_US) as u16,
            flags: get_field(self.words[2], W2_FLAGS) as u8,
            oco_group: get_field(self.words[2], W2_OCO_GROUP) as u16,
            spare: get_field(self.words[2], W2_SPARE) as u8,
        }
    }

    pub fn risk(&self) -> RiskWord {
        RiskWord {
            max_open_ms: get_field(self.words[3], W3_MAX_OPEN_MS) as u32,
            max_adverse_cents: get_field(self.words[3], W3_MAX_ADVERSE_CENTS) as u32,
            exit_on_breaker_ge_level: get_field(self.words[3], W3_EXIT_ON_BREAKER) as u8,
            exit_on_jitter: get_field(self.words[3], W3_EXIT_ON_JITTER) != 0,
            exit_on_cost_gt: get_field(self.words[3], W3_EXIT_ON_COST) != 0,
            forbid_after_min_ct: get_field(self.words[3], W3_FORBID_AFTER_MIN) as u16,
            eod_flat_min_ct: get_field(self.words[3], W3_EOD_FLAT_MIN) as u16,
            fallback_route_id: get_field(self.words[3], W3_ROUTE_B) as u16,
            on_fail: get_field(self.words[3], W3_ON_FAIL) as u8,
            spare: get_field(self.words[3], W3_SPARE) as u8,
        }
    }

    pub fn checksum(&self) -> u16 {
        get_field(self.words[3], W3_CHECKSUM) as u16
    }

    pub fn words(&self) -> [u128; 4] {
        self.words
    }

    /// Return the coarse expiry deadline in milliseconds (modulo 2^24).
    pub fn ttl_deadline_coarse(&self) -> u32 {
        let created = get_field(self.words[0], W0_CREATED_MS_COARSE) as u32;
        let ttl = get_field(self.words[0], W0_TTL_MS) as u32;
        (created.wrapping_add(ttl)) & COARSE_MASK
    }

    /// Check whether the bundle has exceeded its TTL using a coarse millisecond counter.
    ///
    /// The caller must supply the same wrapping counter used to populate
    /// `created_ms_coarse`. TTL values of zero are treated as indefinite.
    pub fn ttl_expired(&self, now_ms_coarse: u32) -> bool {
        let ttl = get_field(self.words[0], W0_TTL_MS) as u32;
        if ttl == 0 {
            return false;
        }
        let created = get_field(self.words[0], W0_CREATED_MS_COARSE) as u32;
        let elapsed = now_ms_coarse.wrapping_sub(created) & COARSE_MASK;
        elapsed >= ttl
    }
}

/// Execution result type for publish operations.
pub type ExecutionResult = Result<Snapshot, ExecutionError>;

/// Errors that can occur during execution bundle publishing.
#[cfg_attr(feature = "sim", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "sim", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionError {
    /// Circuit breaker halted execution due to risk conditions.
    BreakerHalt,
}

/// Structured reasons for rejecting a capsule snapshot during load.
#[cfg_attr(feature = "sim", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "sim", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DenyReason {
    NotCommitted = 1,
    OddVersion = 2,
    InFlightRewrite = 3,
    Stale = 4,
    SeqMismatch = 5,
    ChecksumMismatch = 6,
    AttemptsExhausted = 7,
}

impl DenyReason {
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            DenyReason::NotCommitted => "not_committed",
            DenyReason::OddVersion => "odd_version",
            DenyReason::InFlightRewrite => "inflight_rewrite",
            DenyReason::Stale => "stale",
            DenyReason::SeqMismatch => "seq_mismatch",
            DenyReason::ChecksumMismatch => "checksum_mismatch",
            DenyReason::AttemptsExhausted => "attempts_exhausted",
        }
    }
}

/// Snapshot of denial counters suitable for metrics/logging.
#[cfg_attr(feature = "sim", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DenySnapshot {
    pub accepts: u64,
    pub not_committed: u64,
    pub odd_version: u64,
    pub inflight_rewrite: u64,
    pub stale: u64,
    pub seq_mismatch: u64,
    pub checksum_mismatch: u64,
    pub attempts_exhausted: u64,
}

/// Atomic counters tracking accept/deny tallies from `load_with_diagnostics`.
#[derive(Debug)]
pub struct DenyCounters {
    accepts: AtomicU64,
    not_committed: AtomicU64,
    odd_version: AtomicU64,
    inflight_rewrite: AtomicU64,
    stale: AtomicU64,
    seq_mismatch: AtomicU64,
    checksum_mismatch: AtomicU64,
    attempts_exhausted: AtomicU64,
}

impl Default for DenyCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl DenyCounters {
    pub const fn new() -> Self {
        Self {
            accepts: AtomicU64::new(0),
            not_committed: AtomicU64::new(0),
            odd_version: AtomicU64::new(0),
            inflight_rewrite: AtomicU64::new(0),
            stale: AtomicU64::new(0),
            seq_mismatch: AtomicU64::new(0),
            checksum_mismatch: AtomicU64::new(0),
            attempts_exhausted: AtomicU64::new(0),
        }
    }

    pub fn record_accept(&self) {
        let _ = self.accepts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record(&self, reason: DenyReason) {
        match reason {
            DenyReason::NotCommitted => {
                let _ = self.not_committed.fetch_add(1, Ordering::Relaxed);
            }
            DenyReason::OddVersion => {
                let _ = self.odd_version.fetch_add(1, Ordering::Relaxed);
            }
            DenyReason::InFlightRewrite => {
                let _ = self.inflight_rewrite.fetch_add(1, Ordering::Relaxed);
            }
            DenyReason::Stale => {
                let _ = self.stale.fetch_add(1, Ordering::Relaxed);
            }
            DenyReason::SeqMismatch => {
                let _ = self.seq_mismatch.fetch_add(1, Ordering::Relaxed);
            }
            DenyReason::ChecksumMismatch => {
                let _ = self.checksum_mismatch.fetch_add(1, Ordering::Relaxed);
            }
            DenyReason::AttemptsExhausted => {
                let _ = self.attempts_exhausted.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn snapshot(&self) -> DenySnapshot {
        DenySnapshot {
            accepts: self.accepts.load(Ordering::Relaxed),
            not_committed: self.not_committed.load(Ordering::Relaxed),
            odd_version: self.odd_version.load(Ordering::Relaxed),
            inflight_rewrite: self.inflight_rewrite.load(Ordering::Relaxed),
            stale: self.stale.load(Ordering::Relaxed),
            seq_mismatch: self.seq_mismatch.load(Ordering::Relaxed),
            checksum_mismatch: self.checksum_mismatch.load(Ordering::Relaxed),
            attempts_exhausted: self.attempts_exhausted.load(Ordering::Relaxed),
        }
    }
}

#[cfg(feature = "sim")]
pub mod sim {
    use super::*;
    use alloc::vec::Vec;
    use core::fmt;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct BundleSpec {
        pub header: HeaderSpec,
        pub entry: EntrySpec,
        pub brackets: BracketsSpec,
        pub risk: RiskSpec,
    }

    impl BundleSpec {
        pub fn publish(&self, capsule: &AtomicExecutionBundle) -> ExecutionResult {
            capsule.publish(
                self.header.clone().into(),
                self.entry.clone().into(),
                self.brackets.clone().into(),
                self.risk.clone().into(),
            )
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct HeaderSpec {
        pub stale: bool,
        pub state: u8,
        pub kind: u8,
        pub has_bracket: bool,
        pub reduce_only_bundle: bool,
        pub spare_flags: u8,
        pub symbol_id: u16,
        pub strategy_id: u8,
        pub account_id: u16,
        pub pair_id: u16,
        pub created_ms_coarse: u32,
        pub ttl_ms: u16,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct EntrySpec {
        pub side_is_buy: bool,
        pub anchor: u8,
        pub order_type: u8,
        pub tif: u8,
        pub quantity: u32,
        pub price_ticks: i32,
        pub route_id: u16,
        pub slip_cap_bp: u16,
        pub post_only: bool,
        pub reduce_only: bool,
        pub allow_partial: bool,
        pub risk_tag: u16,
        pub seq_hint: u32,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct BracketsSpec {
        pub take_profit_ticks: i16,
        pub stop_loss_ticks: i16,
        pub trailing_ticks: i16,
        pub time_stop_ms: u16,
        pub exit_route_id: u16,
        pub exit_tif: u8,
        pub take_profit_kind: u8,
        pub stop_loss_kind: u8,
        pub rearm_on_reentry: bool,
        pub scale_out_pct: u8,
        pub slip_cap_exit_bp: u16,
        pub latency_budget_us: u16,
        pub flags: u8,
        pub oco_group: u16,
        pub spare: u8,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct RiskSpec {
        pub max_open_ms: u32,
        pub max_adverse_cents: u32,
        pub exit_on_breaker_ge_level: u8,
        pub exit_on_jitter: bool,
        pub exit_on_cost_gt: bool,
        pub forbid_after_min_ct: u16,
        pub eod_flat_min_ct: u16,
        pub fallback_route_id: u16,
        pub on_fail: u8,
        pub spare: u8,
    }

    impl From<HeaderSpec> for HeaderWord {
        fn from(spec: HeaderSpec) -> Self {
            HeaderWord {
                stale: spec.stale,
                state: spec.state,
                kind: spec.kind,
                has_bracket: spec.has_bracket,
                reduce_only_bundle: spec.reduce_only_bundle,
                spare_flags: spec.spare_flags,
                symbol_id: spec.symbol_id,
                strategy_id: spec.strategy_id,
                account_id: spec.account_id,
                pair_id: spec.pair_id,
                created_ms_coarse: spec.created_ms_coarse,
                ttl_ms: spec.ttl_ms,
            }
        }
    }

    impl From<EntrySpec> for EntryLegWord {
        fn from(spec: EntrySpec) -> Self {
            EntryLegWord {
                side_is_buy: spec.side_is_buy,
                anchor: spec.anchor,
                order_type: spec.order_type,
                tif: spec.tif,
                quantity: spec.quantity,
                price_ticks: spec.price_ticks,
                route_id: spec.route_id,
                slip_cap_bp: spec.slip_cap_bp,
                post_only: spec.post_only,
                reduce_only: spec.reduce_only,
                allow_partial: spec.allow_partial,
                risk_tag: spec.risk_tag,
                seq_hint: spec.seq_hint,
            }
        }
    }

    impl From<BracketsSpec> for BracketsWord {
        fn from(spec: BracketsSpec) -> Self {
            BracketsWord {
                take_profit_ticks: spec.take_profit_ticks,
                stop_loss_ticks: spec.stop_loss_ticks,
                trailing_ticks: spec.trailing_ticks,
                time_stop_ms: spec.time_stop_ms,
                exit_route_id: spec.exit_route_id,
                exit_tif: spec.exit_tif,
                take_profit_kind: spec.take_profit_kind,
                stop_loss_kind: spec.stop_loss_kind,
                rearm_on_reentry: spec.rearm_on_reentry,
                scale_out_pct: spec.scale_out_pct,
                slip_cap_exit_bp: spec.slip_cap_exit_bp,
                latency_budget_us: spec.latency_budget_us,
                flags: spec.flags,
                oco_group: spec.oco_group,
                spare: spec.spare,
            }
        }
    }

    impl From<RiskSpec> for RiskWord {
        fn from(spec: RiskSpec) -> Self {
            RiskWord {
                max_open_ms: spec.max_open_ms,
                max_adverse_cents: spec.max_adverse_cents,
                exit_on_breaker_ge_level: spec.exit_on_breaker_ge_level,
                exit_on_jitter: spec.exit_on_jitter,
                exit_on_cost_gt: spec.exit_on_cost_gt,
                forbid_after_min_ct: spec.forbid_after_min_ct,
                eod_flat_min_ct: spec.eod_flat_min_ct,
                fallback_route_id: spec.fallback_route_id,
                on_fail: spec.on_fail,
                spare: spec.spare,
            }
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Scenario {
        pub steps: Vec<Step>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(tag = "op", rename_all = "snake_case")]
    pub enum Step {
        Publish { bundle: BundleSpec },
        MarkStale,
        Load { expect: LoadExpectation },
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(tag = "outcome", rename_all = "snake_case")]
    pub enum LoadExpectation {
        Accept {
            #[serde(default)]
            now_ms_coarse: Option<u32>,
            #[serde(default)]
            ttl_expired: Option<bool>,
        },
        Deny {
            reason: DenyReason,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ScenarioError {
        ExpectedAccept(DenyReason),
        ExpectedDeny {
            expected: DenyReason,
            actual: Option<DenyReason>,
        },
        TtlMismatch {
            expected: bool,
            actual: bool,
        },
    }

    impl fmt::Display for ScenarioError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                ScenarioError::ExpectedAccept(actual) => {
                    write!(f, "expected accept but observed deny ({:?})", actual)
                }
                ScenarioError::ExpectedDeny { expected, actual } => {
                    if let Some(actual) = actual {
                        write!(f, "expected deny {:?} but observed {:?}", expected, actual)
                    } else {
                        write!(f, "expected deny {:?} but observed accept", expected)
                    }
                }
                ScenarioError::TtlMismatch { expected, actual } => {
                    write!(
                        f,
                        "TTL expectation mismatch (expected {}, actual {})",
                        expected, actual
                    )
                }
            }
        }
    }

    impl Scenario {
        pub fn execute(
            &self,
            capsule: &AtomicExecutionBundle,
            counters: &DenyCounters,
        ) -> Result<DenySnapshot, ScenarioError> {
            for step in &self.steps {
                match step {
                    Step::Publish { bundle } => {
                        bundle
                            .publish(capsule)
                            .expect("publish should succeed in scenario");
                    }
                    Step::MarkStale => capsule.mark_stale(),
                    Step::Load { expect } => match capsule.load_with_diagnostics(Some(counters)) {
                        Ok(snapshot) => match expect {
                            LoadExpectation::Accept {
                                now_ms_coarse,
                                ttl_expired,
                            } => {
                                if let Some(expected) = ttl_expired {
                                    let now = (*now_ms_coarse)
                                        .unwrap_or(snapshot.header().created_ms_coarse);
                                    let actual = snapshot.ttl_expired(now);
                                    if actual != *expected {
                                        return Err(ScenarioError::TtlMismatch {
                                            expected: *expected,
                                            actual,
                                        });
                                    }
                                }
                            }
                            LoadExpectation::Deny { reason } => {
                                return Err(ScenarioError::ExpectedDeny {
                                    expected: *reason,
                                    actual: None,
                                });
                            }
                        },
                        Err(actual) => match expect {
                            LoadExpectation::Accept { .. } => {
                                return Err(ScenarioError::ExpectedAccept(actual));
                            }
                            LoadExpectation::Deny { reason } => {
                                if actual != *reason {
                                    return Err(ScenarioError::ExpectedDeny {
                                        expected: *reason,
                                        actual: Some(actual),
                                    });
                                }
                            }
                        },
                    },
                }
            }

            Ok(counters.snapshot())
        }
    }
}

#[cfg(feature = "sim")]
pub use sim::{BundleSpec, Scenario, ScenarioError};

const _AEB_ATOMIC_ALIGN: [u8; 128] = [0; core::mem::size_of::<AtomicExecutionBundle>()];
const _AEB_SNAPSHOT_ALIGN: [u8; 64] = [0; core::mem::size_of::<Snapshot>()];
const _AEB_DRAFT_ALIGN: [u8; 64] = [0; core::mem::size_of::<BundleDraft>()];

#[cfg(feature = "checksum")]
fn checksum_words(words: [u128; 3]) -> u16 {
    let mut acc: u32 = 0;
    for word in words {
        let mut lane = word;
        for _ in 0..8 {
            acc = acc.wrapping_add((lane & 0xFFFF) as u32);
            lane >>= 16;
        }
    }
    (acc & 0xFFFF) as u16
}

#[cfg(not(feature = "checksum"))]
#[allow(dead_code)]
fn checksum_words(_: [u128; 3]) -> u16 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_round_trip() {
        let bundle = AtomicExecutionBundle::new();
        let snapshot = bundle
            .publish(
                HeaderWord {
                    stale: false,
                    state: 2,
                    kind: 1,
                    has_bracket: true,
                    reduce_only_bundle: false,
                    spare_flags: 3,
                    symbol_id: 42,
                    strategy_id: 7,
                    account_id: 512,
                    pair_id: 321,
                    created_ms_coarse: 0xABCDE,
                    ttl_ms: 1500,
                },
                EntryLegWord {
                    side_is_buy: true,
                    anchor: 2,
                    order_type: 3,
                    tif: 4,
                    quantity: 55,
                    price_ticks: -1_234,
                    route_id: 777,
                    slip_cap_bp: 120,
                    post_only: true,
                    reduce_only: false,
                    allow_partial: true,
                    risk_tag: 777,
                    seq_hint: 12_345,
                },
                BracketsWord {
                    take_profit_ticks: 48,
                    stop_loss_ticks: -29,
                    trailing_ticks: 17,
                    time_stop_ms: 12_000,
                    exit_route_id: 256,
                    exit_tif: 2,
                    take_profit_kind: 1,
                    stop_loss_kind: 2,
                    rearm_on_reentry: true,
                    scale_out_pct: 20,
                    slip_cap_exit_bp: 350,
                    latency_budget_us: 3_000,
                    flags: 0x24,
                    oco_group: 321,
                    spare: 0,
                },
                RiskWord {
                    max_open_ms: 90_000,
                    max_adverse_cents: 2_500,
                    exit_on_breaker_ge_level: 2,
                    exit_on_jitter: true,
                    exit_on_cost_gt: false,
                    forbid_after_min_ct: 900,
                    eod_flat_min_ct: 1_100,
                    fallback_route_id: 512,
                    on_fail: 3,
                    spare: 0,
                },
            )
            .expect("publish should succeed with closed breaker");

        assert!(snapshot.commit());
        assert!(!snapshot.stale());
        assert_eq!(snapshot.header().pair_id, 321);
        assert_eq!(snapshot.header().account_id, 512);
        assert_eq!(snapshot.entry().route_id, 777);
        assert_eq!(snapshot.entry().price_ticks, -1_234);
        assert!(snapshot.brackets().rearm_on_reentry);
        assert_eq!(snapshot.brackets().latency_budget_us, 3_000);
        assert_eq!(snapshot.risk().fallback_route_id, 512);
        assert_eq!(snapshot.risk().on_fail, 3);

        let loaded = bundle.load().expect("bundle load");
        assert_eq!(loaded.sequence(), snapshot.sequence());
        assert_eq!(loaded.version(), snapshot.version());
        assert_eq!(loaded.entry().seq_hint, 12_345);
        assert_eq!(loaded.brackets().stop_loss_ticks, -29);
        assert_eq!(loaded.checksum(), snapshot.checksum());
    }

    #[test]
    fn stale_flag_invalidation() {
        let bundle = AtomicExecutionBundle::new();
        bundle
            .publish(
                HeaderWord {
                    stale: false,
                    state: 0,
                    kind: 0,
                    has_bracket: false,
                    reduce_only_bundle: false,
                    spare_flags: 0,
                    symbol_id: 1,
                    strategy_id: 2,
                    account_id: 3,
                    pair_id: 4,
                    created_ms_coarse: 20,
                    ttl_ms: 10,
                },
                EntryLegWord::default(),
                BracketsWord::default(),
                RiskWord::default(),
            )
            .expect("publish should succeed with closed breaker");
        bundle.mark_stale();
        assert!(bundle.load().is_none());
    }

    #[test]
    fn diagnostics_counters() {
        let bundle = AtomicExecutionBundle::new();
        let counters = DenyCounters::new();

        assert_eq!(
            bundle.load_with_diagnostics(Some(&counters)),
            Err(DenyReason::NotCommitted)
        );

        let accepted = bundle
            .publish(
                HeaderWord {
                    stale: false,
                    state: 0,
                    kind: 0,
                    has_bracket: true,
                    reduce_only_bundle: false,
                    spare_flags: 0,
                    symbol_id: 101,
                    strategy_id: 2,
                    account_id: 3,
                    pair_id: 8,
                    created_ms_coarse: 500,
                    ttl_ms: 250,
                },
                EntryLegWord {
                    side_is_buy: true,
                    anchor: 1,
                    order_type: 2,
                    tif: 1,
                    quantity: 10,
                    price_ticks: 12,
                    route_id: 55,
                    slip_cap_bp: 40,
                    post_only: false,
                    reduce_only: false,
                    allow_partial: true,
                    risk_tag: 5,
                    seq_hint: 42,
                },
                BracketsWord::default(),
                RiskWord::default(),
            )
            .expect("publish should succeed with closed breaker");
        assert!(accepted.commit());
        assert!(bundle.load_with_diagnostics(Some(&counters)).is_ok());

        bundle.mark_stale();
        assert_eq!(
            bundle.load_with_diagnostics(Some(&counters)),
            Err(DenyReason::Stale)
        );

        // Republish a fresh capsule and tamper with the mirrored tail to trigger seq mismatch.
        let snapshot = bundle
            .publish(
                HeaderWord {
                    stale: false,
                    state: 0,
                    kind: 0,
                    has_bracket: true,
                    reduce_only_bundle: false,
                    spare_flags: 0,
                    symbol_id: 7,
                    strategy_id: 1,
                    account_id: 9,
                    pair_id: 12,
                    created_ms_coarse: 1_200,
                    ttl_ms: 300,
                },
                EntryLegWord::default(),
                BracketsWord::default(),
                RiskWord::default(),
            )
            .expect("publish should succeed with closed breaker");
        assert!(snapshot.commit());
        let w3 = bundle.words[3].load(Ordering::Relaxed);
        let seq_tail = get_field(w3, W3_SEQ_TAIL);
        let tampered = set_field(
            w3,
            W3_SEQ_TAIL,
            (seq_tail.wrapping_add(1)) & W3_SEQ_TAIL.value_mask(),
        );
        bundle.words[3].store(tampered, Ordering::Relaxed);
        assert_eq!(
            bundle.load_with_diagnostics(Some(&counters)),
            Err(DenyReason::SeqMismatch)
        );

        let snap = counters.snapshot();
        assert_eq!(snap.accepts, 1);
        assert_eq!(snap.not_committed, 1);
        assert_eq!(snap.stale, 1);
        assert_eq!(snap.seq_mismatch, 1);
        assert_eq!(snap.attempts_exhausted, 1);
    }

    #[test]
    fn ttl_expiry_checks() {
        let bundle = AtomicExecutionBundle::new();
        let fresh = bundle
            .publish(
                HeaderWord {
                    stale: false,
                    state: 0,
                    kind: 0,
                    has_bracket: true,
                    reduce_only_bundle: false,
                    spare_flags: 0,
                    symbol_id: 9,
                    strategy_id: 1,
                    account_id: 2,
                    pair_id: 3,
                    created_ms_coarse: 1_000,
                    ttl_ms: 150,
                },
                EntryLegWord::default(),
                BracketsWord::default(),
                RiskWord::default(),
            )
            .expect("publish should succeed with closed breaker");
        assert!(!fresh.ttl_expired(1_120));
        assert!(fresh.ttl_expired(1_210));

        let wrap_created = (COARSE_MASK - 10) & COARSE_MASK;
        let wrap_snapshot = bundle
            .publish(
                HeaderWord {
                    stale: false,
                    state: 0,
                    kind: 0,
                    has_bracket: false,
                    reduce_only_bundle: false,
                    spare_flags: 0,
                    symbol_id: 9,
                    strategy_id: 1,
                    account_id: 2,
                    pair_id: 4,
                    created_ms_coarse: wrap_created,
                    ttl_ms: 20,
                },
                EntryLegWord::default(),
                BracketsWord::default(),
                RiskWord::default(),
            )
            .expect("publish should succeed with closed breaker");
        assert!(!wrap_snapshot.ttl_expired((wrap_created + 5) & COARSE_MASK));
        assert!(wrap_snapshot.ttl_expired(15));

        let indefinite = bundle
            .publish(
                HeaderWord {
                    stale: false,
                    state: 0,
                    kind: 0,
                    has_bracket: false,
                    reduce_only_bundle: false,
                    spare_flags: 0,
                    symbol_id: 9,
                    strategy_id: 1,
                    account_id: 2,
                    pair_id: 5,
                    created_ms_coarse: 500,
                    ttl_ms: 0,
                },
                EntryLegWord::default(),
                BracketsWord::default(),
                RiskWord::default(),
            )
            .expect("publish should succeed with closed breaker");
        assert!(!indefinite.ttl_expired(50_000));
    }

    #[test]
    fn publish_with_reuse_builder() {
        let bundle = AtomicExecutionBundle::new();
        let mut draft = BundleDraft::new();
        let mut qty = 10;
        for seq in 1u16..=3 {
            let snapshot = bundle
                .publish_with_reuse(&mut draft, |draft| {
                    draft
                        .set_header(HeaderWord {
                            stale: false,
                            state: 0,
                            kind: 0,
                            has_bracket: true,
                            reduce_only_bundle: false,
                            spare_flags: 0,
                            symbol_id: 7,
                            strategy_id: 1,
                            account_id: 99,
                            pair_id: seq,
                            created_ms_coarse: 11,
                            ttl_ms: 25,
                        })
                        .set_entry(EntryLegWord {
                            side_is_buy: true,
                            anchor: 1,
                            order_type: 1,
                            tif: 1,
                            quantity: qty,
                            price_ticks: 100,
                            route_id: 1,
                            slip_cap_bp: 5,
                            post_only: false,
                            reduce_only: false,
                            allow_partial: false,
                            risk_tag: 0,
                            seq_hint: qty,
                        })
                        .set_brackets(BracketsWord::default())
                        .set_risk(RiskWord::default());
                })
                .expect("publish should succeed with closed breaker");
            assert_eq!(snapshot.entry().quantity, qty);
            assert_eq!(snapshot.header().pair_id, seq);
            qty += 1;
        }
    }
}
