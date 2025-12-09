#![no_std]

//! APC-512 (Atomic Position Capsule) publishes live position state, P&L, and
//! risk headroom in a single 64-byte cache line. Writers stage the four 128-bit
//! words, flip the odd→even version in W0 with a release store, and readers
//! obtain an authoritative snapshot with one relaxed load pass.

use core::sync::atomic::Ordering;
use portable_atomic::{AtomicU128, AtomicU64};

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

pub const FLAG_FLAT: u8 = 0b0000_0001;
pub const FLAG_LONG: u8 = 0b0000_0010;
pub const FLAG_SHORT: u8 = 0b0000_0100;
pub const FLAG_LOCKED: u8 = 0b0000_1000;
pub const FLAG_HALT: u8 = 0b0001_0000;

pub const RISK_FLAG_PAUSE_NEWS: u8 = 0b0000_0001;
pub const RISK_FLAG_NEWS_WINDOW: u8 = 0b0000_0010;
pub const RISK_FLAG_STALL_LAT: u8 = 0b0000_0100;

pub const BREAKER_REDUCE_ONLY_LEVEL: u8 = 2;

const W0_POS_QTY: Field = Field { shift: 0, bits: 32 };
const W0_AVG_PX_TICKS: Field = Field {
    shift: 32,
    bits: 24,
};
const W0_REM_DAILY_LOSS: Field = Field {
    shift: 56,
    bits: 32,
};
const W0_FLAGS: Field = Field { shift: 88, bits: 8 };
const W0_VERSION: Field = Field { shift: 96, bits: 8 };
const W0_SEQUENCE: Field = Field {
    shift: 104,
    bits: 16,
};
const W0_PAD: Field = Field {
    shift: 120,
    bits: 8,
};

const W1_REALIZED: Field = Field { shift: 0, bits: 32 };
const W1_UNREAL: Field = Field {
    shift: 32,
    bits: 32,
};
const W1_PEAK_EQUITY: Field = Field {
    shift: 64,
    bits: 32,
};
const W1_TRAILING_DRAW: Field = Field {
    shift: 96,
    bits: 32,
};

const W2_NOW_MIN_CT: Field = Field { shift: 0, bits: 11 };
const W2_FORBID_AFTER_CT: Field = Field {
    shift: 11,
    bits: 11,
};
const W2_EOD_FLAT_CT: Field = Field {
    shift: 22,
    bits: 11,
};
const W2_OPEN_SINCE_MS: Field = Field {
    shift: 33,
    bits: 24,
};
const W2_MAX_OPEN_MS: Field = Field {
    shift: 57,
    bits: 20,
};
const W2_MAX_CONTRACTS: Field = Field {
    shift: 77,
    bits: 12,
};
const W2_MAX_PER_TRADE: Field = Field {
    shift: 89,
    bits: 20,
};
const W2_RISK_FLAGS: Field = Field {
    shift: 109,
    bits: 8,
};
const W2_PAD: Field = Field {
    shift: 117,
    bits: 10,
};
const W2_PAD_HIGH: Field = Field {
    shift: 127,
    bits: 1,
};

const W3_SYMBOL_ID: Field = Field { shift: 0, bits: 16 };
const W3_ACCOUNT_ID: Field = Field {
    shift: 16,
    bits: 16,
};
const W3_LAST_EXEC_ID: Field = Field {
    shift: 32,
    bits: 32,
};
const W3_BREAKER_LEVEL: Field = Field { shift: 64, bits: 2 };
const W3_ALT_HEALTH: Field = Field { shift: 66, bits: 6 };
const W3_VIOLATION_BITS: Field = Field {
    shift: 72,
    bits: 16,
};
const W3_CHECKSUM: Field = Field {
    shift: 88,
    bits: 16,
};
const W3_VER_TAIL: Field = Field {
    shift: 104,
    bits: 8,
};
const W3_SEQ_TAIL: Field = Field {
    shift: 112,
    bits: 16,
};

const _: () = {
    assert!(W0_PAD.shift + W0_PAD.bits <= 128);
    assert!(W1_TRAILING_DRAW.shift + W1_TRAILING_DRAW.bits <= 128);
    assert!(W2_PAD_HIGH.shift + W2_PAD_HIGH.bits <= 128);
    assert!(W3_SEQ_TAIL.shift + W3_SEQ_TAIL.bits <= 128);
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PositionHeadWord {
    pub position_qty: i32,
    pub avg_px_ticks: i32,
    pub remaining_daily_loss_cents: u32,
    pub flags: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EquityWord {
    pub realized_cents: i32,
    pub unrealized_cents: i32,
    pub peak_equity_cents: i32,
    pub trailing_draw_cents: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionWord {
    pub now_min_ct: u16,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub open_since_ms: u32,
    pub max_open_ms: u32,
    pub max_contracts: u16,
    pub max_per_trade_cents: u32,
    pub risk_flags: u8,
    pub reserved_bits: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TailWord {
    pub symbol_id: u16,
    pub account_id: u16,
    pub last_exec_id: u32,
    pub breaker_level: u8,
    pub alt_health: u8,
    pub violation_bits: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(align(64))]
pub struct CapsuleDraft {
    words: [u128; 4],
}

impl CapsuleDraft {
    pub const fn new() -> Self {
        Self { words: [0; 4] }
    }

    pub fn clear(&mut self) {
        self.words = [0; 4];
    }

    pub fn set_head(&mut self, head: PositionHeadWord) -> &mut Self {
        let mut word = 0u128;
        word = set_signed_field(word, W0_POS_QTY, head.position_qty);
        word = set_signed_field(word, W0_AVG_PX_TICKS, head.avg_px_ticks);
        word = set_field(
            word,
            W0_REM_DAILY_LOSS,
            head.remaining_daily_loss_cents as u128,
        );
        word = set_field(word, W0_FLAGS, head.flags as u128);
        word = set_field(word, W0_VERSION, 0);
        word = set_field(word, W0_SEQUENCE, 0);
        word = set_field(word, W0_PAD, 0);
        self.words[0] = word;
        self
    }

    pub fn set_equity(&mut self, equity: EquityWord) -> &mut Self {
        let mut word = 0u128;
        word = set_signed_field(word, W1_REALIZED, equity.realized_cents);
        word = set_signed_field(word, W1_UNREAL, equity.unrealized_cents);
        word = set_signed_field(word, W1_PEAK_EQUITY, equity.peak_equity_cents);
        word = set_field(word, W1_TRAILING_DRAW, equity.trailing_draw_cents as u128);
        self.words[1] = word;
        self
    }

    pub fn set_session(&mut self, session: SessionWord) -> &mut Self {
        debug_assert!(session.now_min_ct < (1 << W2_NOW_MIN_CT.bits));
        debug_assert!(session.forbid_after_min_ct < (1 << W2_FORBID_AFTER_CT.bits));
        debug_assert!(session.eod_flat_min_ct < (1 << W2_EOD_FLAT_CT.bits));
        debug_assert!(session.open_since_ms < (1 << W2_OPEN_SINCE_MS.bits));
        debug_assert!(session.max_open_ms < (1 << W2_MAX_OPEN_MS.bits));
        debug_assert!(session.max_contracts < (1 << W2_MAX_CONTRACTS.bits));
        debug_assert!(session.max_per_trade_cents < (1 << W2_MAX_PER_TRADE.bits));
        debug_assert!(session.reserved_bits < (1 << (W2_PAD.bits + W2_PAD_HIGH.bits)));

        let mut word = 0u128;
        word = set_field(word, W2_NOW_MIN_CT, session.now_min_ct as u128);
        word = set_field(
            word,
            W2_FORBID_AFTER_CT,
            session.forbid_after_min_ct as u128,
        );
        word = set_field(word, W2_EOD_FLAT_CT, session.eod_flat_min_ct as u128);
        word = set_field(word, W2_OPEN_SINCE_MS, session.open_since_ms as u128);
        word = set_field(word, W2_MAX_OPEN_MS, session.max_open_ms as u128);
        word = set_field(word, W2_MAX_CONTRACTS, session.max_contracts as u128);
        word = set_field(word, W2_MAX_PER_TRADE, session.max_per_trade_cents as u128);
        word = set_field(word, W2_RISK_FLAGS, session.risk_flags as u128);
        let pad_low = (session.reserved_bits & 0x03FF) as u128;
        let pad_high = ((session.reserved_bits >> 10) & 0x01) as u128;
        word = set_field(word, W2_PAD, pad_low);
        word = set_field(word, W2_PAD_HIGH, pad_high);
        self.words[2] = word;
        self
    }

    pub fn set_tail(&mut self, tail: TailWord) -> &mut Self {
        debug_assert!(tail.breaker_level < (1 << W3_BREAKER_LEVEL.bits));
        debug_assert!(tail.alt_health < (1 << W3_ALT_HEALTH.bits));
        let mut word = 0u128;
        word = set_field(word, W3_SYMBOL_ID, tail.symbol_id as u128);
        word = set_field(word, W3_ACCOUNT_ID, tail.account_id as u128);
        word = set_field(word, W3_LAST_EXEC_ID, tail.last_exec_id as u128);
        word = set_field(word, W3_BREAKER_LEVEL, tail.breaker_level as u128);
        word = set_field(word, W3_ALT_HEALTH, tail.alt_health as u128);
        word = set_field(word, W3_VIOLATION_BITS, tail.violation_bits as u128);
        word = set_field(word, W3_CHECKSUM, 0);
        word = set_field(word, W3_VER_TAIL, 0);
        word = set_field(word, W3_SEQ_TAIL, 0);
        self.words[3] = word;
        self
    }
}

#[repr(align(64))]
pub struct AtomicPositionCapsule {
    words: [AtomicU128; 4],
}

impl AtomicPositionCapsule {
    pub const fn new() -> Self {
        Self {
            words: [
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
                AtomicU128::new(0),
            ],
        }
    }

    pub fn publish(
        &self,
        head: PositionHeadWord,
        equity: EquityWord,
        session: SessionWord,
        tail: TailWord,
    ) -> Snapshot {
        let mut draft = CapsuleDraft::new();
        draft
            .set_head(head)
            .set_equity(equity)
            .set_session(session)
            .set_tail(tail);
        self.publish_draft(&draft)
    }

    pub fn publish_with<F>(&self, mut builder: F) -> Snapshot
    where
        F: FnMut(&mut CapsuleDraft),
    {
        let mut draft = CapsuleDraft::new();
        builder(&mut draft);
        self.publish_draft(&draft)
    }

    pub fn publish_with_reuse<F>(&self, draft: &mut CapsuleDraft, mut builder: F) -> Snapshot
    where
        F: FnMut(&mut CapsuleDraft),
    {
        draft.clear();
        builder(draft);
        self.publish_draft(draft)
    }

    pub fn publish_draft(&self, draft: &CapsuleDraft) -> Snapshot {
        let old_head = self.words[0].load(Ordering::Relaxed);
        let prev_ver = get_field(old_head, W0_VERSION) as u16;
        let prev_seq = get_field(old_head, W0_SEQUENCE) as u16;

        let ver_mask = W0_VERSION.value_mask() as u16;
        let seq_mask = W0_SEQUENCE.value_mask() as u16;

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

        let mut w0_base = draft.words[0];
        let w1 = draft.words[1];
        let w2 = draft.words[2];
        let mut w3 = draft.words[3];

        w0_base = set_field(w0_base, W0_PAD, 0);

        let mut w0_inflight = set_field(w0_base, W0_VERSION, odd_ver as u128);
        w0_inflight = set_field(w0_inflight, W0_SEQUENCE, new_seq as u128);

        let mut w0_final = set_field(w0_base, W0_VERSION, even_ver as u128);
        w0_final = set_field(w0_final, W0_SEQUENCE, new_seq as u128);

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

        Snapshot {
            words: [w0_final, w1, w2, w3],
        }
    }

    pub fn load(&self) -> Option<Snapshot> {
        self.load_with_diagnostics(None).ok()
    }

    pub fn load_with_diagnostics(
        &self,
        counters: Option<&DenyCounters>,
    ) -> Result<Snapshot, DenyReason> {
        let mut last_reason = DenyReason::AttemptsExhausted;
        for _ in 0..ATTEMPTS {
            let w0_first = self.words[0].load(Ordering::Acquire);
            if get_field(w0_first, W0_VERSION) & 1 != 0 {
                last_reason = DenyReason::OddVersion;
                continue;
            }

            let w1 = self.words[1].load(Ordering::Relaxed);
            let w2 = self.words[2].load(Ordering::Relaxed);
            let w3 = self.words[3].load(Ordering::Relaxed);

            let w0_second = self.words[0].load(Ordering::Acquire);
            if w0_first != w0_second {
                last_reason = DenyReason::InFlightRewrite;
                continue;
            }
            if get_field(w0_second, W0_VERSION) & 1 != 0 {
                last_reason = DenyReason::OddVersion;
                continue;
            }

            let ver = get_field(w0_second, W0_VERSION) as u16;
            let seq = get_field(w0_second, W0_SEQUENCE) as u16;
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

            if let Some(c) = counters {
                c.record_accept();
            }
            return Ok(Snapshot {
                words: [w0_second, w1, w2, w3],
            });
        }
        if let Some(c) = counters {
            if last_reason != DenyReason::AttemptsExhausted {
                c.record(last_reason);
            }
            c.record(DenyReason::AttemptsExhausted);
        }
        Err(last_reason)
    }

    pub fn sequence_pair(&self) -> (u16, u16) {
        let head = get_field(self.words[0].load(Ordering::Acquire), W0_SEQUENCE) as u16;
        let tail = get_field(self.words[3].load(Ordering::Acquire), W3_SEQ_TAIL) as u16;
        (head, tail)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateDecision {
    Allow,
    ReduceOnly,
    Deny(GateDeny),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateDeny {
    DailyLoss,
    ViolationBits,
    SessionForbid,
    SessionPastEod,
    Halted,
    SizeLimit,
}

impl GateDecision {
    pub fn permits(self, current_qty: i32, delta_qty: i32) -> bool {
        match self {
            GateDecision::Allow => true,
            GateDecision::ReduceOnly => reduces_position(current_qty, delta_qty),
            GateDecision::Deny(_) => false,
        }
    }
}

fn reduces_position(current_qty: i32, delta_qty: i32) -> bool {
    if delta_qty == 0 || current_qty == 0 {
        return false;
    }
    if current_qty.signum() == delta_qty.signum() {
        return false;
    }
    current_qty.abs() >= delta_qty.abs()
}

fn extends_position(current_qty: i32, delta_qty: i32) -> bool {
    if delta_qty == 0 {
        return false;
    }
    if current_qty == 0 {
        return true;
    }
    current_qty.signum() == delta_qty.signum()
}

#[derive(Default)]
pub struct GateMetrics {
    allow: AtomicU64,
    reduce_only: AtomicU64,
    deny_daily_loss: AtomicU64,
    deny_violation_bits: AtomicU64,
    deny_session_forbid: AtomicU64,
    deny_session_past_eod: AtomicU64,
    deny_halted: AtomicU64,
    deny_size_limit: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GateMetricsSnapshot {
    pub allow: u64,
    pub reduce_only: u64,
    pub deny_daily_loss: u64,
    pub deny_violation_bits: u64,
    pub deny_session_forbid: u64,
    pub deny_session_past_eod: u64,
    pub deny_halted: u64,
    pub deny_size_limit: u64,
}

impl GateMetrics {
    pub const fn new() -> Self {
        Self {
            allow: AtomicU64::new(0),
            reduce_only: AtomicU64::new(0),
            deny_daily_loss: AtomicU64::new(0),
            deny_violation_bits: AtomicU64::new(0),
            deny_session_forbid: AtomicU64::new(0),
            deny_session_past_eod: AtomicU64::new(0),
            deny_halted: AtomicU64::new(0),
            deny_size_limit: AtomicU64::new(0),
        }
    }

    pub fn record(&self, decision: GateDecision) {
        match decision {
            GateDecision::Allow => {
                let _ = self.allow.fetch_add(1, Ordering::Relaxed);
            }
            GateDecision::ReduceOnly => {
                let _ = self.reduce_only.fetch_add(1, Ordering::Relaxed);
            }
            GateDecision::Deny(reason) => match reason {
                GateDeny::DailyLoss => {
                    let _ = self.deny_daily_loss.fetch_add(1, Ordering::Relaxed);
                }
                GateDeny::ViolationBits => {
                    let _ = self.deny_violation_bits.fetch_add(1, Ordering::Relaxed);
                }
                GateDeny::SessionForbid => {
                    let _ = self.deny_session_forbid.fetch_add(1, Ordering::Relaxed);
                }
                GateDeny::SessionPastEod => {
                    let _ = self.deny_session_past_eod.fetch_add(1, Ordering::Relaxed);
                }
                GateDeny::Halted => {
                    let _ = self.deny_halted.fetch_add(1, Ordering::Relaxed);
                }
                GateDeny::SizeLimit => {
                    let _ = self.deny_size_limit.fetch_add(1, Ordering::Relaxed);
                }
            },
        }
    }

    pub fn snapshot(&self) -> GateMetricsSnapshot {
        GateMetricsSnapshot {
            allow: self.allow.load(Ordering::Relaxed),
            reduce_only: self.reduce_only.load(Ordering::Relaxed),
            deny_daily_loss: self.deny_daily_loss.load(Ordering::Relaxed),
            deny_violation_bits: self.deny_violation_bits.load(Ordering::Relaxed),
            deny_session_forbid: self.deny_session_forbid.load(Ordering::Relaxed),
            deny_session_past_eod: self.deny_session_past_eod.load(Ordering::Relaxed),
            deny_halted: self.deny_halted.load(Ordering::Relaxed),
            deny_size_limit: self.deny_size_limit.load(Ordering::Relaxed),
        }
    }
}

impl GateMetricsSnapshot {
    pub fn total_decisions(&self) -> u64 {
        self.allow
            + self.reduce_only
            + self.deny_daily_loss
            + self.deny_violation_bits
            + self.deny_session_forbid
            + self.deny_session_past_eod
            + self.deny_halted
            + self.deny_size_limit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(align(64))]
pub struct Snapshot {
    words: [u128; 4],
}

impl Snapshot {
    pub fn version(&self) -> u16 {
        get_field(self.words[0], W0_VERSION) as u16
    }

    pub fn sequence(&self) -> u16 {
        get_field(self.words[0], W0_SEQUENCE) as u16
    }

    pub fn flags(&self) -> u8 {
        get_field(self.words[0], W0_FLAGS) as u8
    }

    pub fn head(&self) -> PositionHeadWord {
        PositionHeadWord {
            position_qty: get_signed_field(self.words[0], W0_POS_QTY),
            avg_px_ticks: get_signed_field(self.words[0], W0_AVG_PX_TICKS),
            remaining_daily_loss_cents: get_field(self.words[0], W0_REM_DAILY_LOSS) as u32,
            flags: self.flags(),
        }
    }

    pub fn equity(&self) -> EquityWord {
        EquityWord {
            realized_cents: get_signed_field(self.words[1], W1_REALIZED),
            unrealized_cents: get_signed_field(self.words[1], W1_UNREAL),
            peak_equity_cents: get_signed_field(self.words[1], W1_PEAK_EQUITY),
            trailing_draw_cents: get_field(self.words[1], W1_TRAILING_DRAW) as u32,
        }
    }

    pub fn session(&self) -> SessionWord {
        let pad_low = get_field(self.words[2], W2_PAD) as u16;
        let pad_high = get_field(self.words[2], W2_PAD_HIGH) as u16;
        SessionWord {
            now_min_ct: get_field(self.words[2], W2_NOW_MIN_CT) as u16,
            forbid_after_min_ct: get_field(self.words[2], W2_FORBID_AFTER_CT) as u16,
            eod_flat_min_ct: get_field(self.words[2], W2_EOD_FLAT_CT) as u16,
            open_since_ms: get_field(self.words[2], W2_OPEN_SINCE_MS) as u32,
            max_open_ms: get_field(self.words[2], W2_MAX_OPEN_MS) as u32,
            max_contracts: get_field(self.words[2], W2_MAX_CONTRACTS) as u16,
            max_per_trade_cents: get_field(self.words[2], W2_MAX_PER_TRADE) as u32,
            risk_flags: get_field(self.words[2], W2_RISK_FLAGS) as u8,
            reserved_bits: pad_low | (pad_high << 10),
        }
    }

    pub fn tail(&self) -> TailWord {
        TailWord {
            symbol_id: get_field(self.words[3], W3_SYMBOL_ID) as u16,
            account_id: get_field(self.words[3], W3_ACCOUNT_ID) as u16,
            last_exec_id: get_field(self.words[3], W3_LAST_EXEC_ID) as u32,
            breaker_level: get_field(self.words[3], W3_BREAKER_LEVEL) as u8,
            alt_health: get_field(self.words[3], W3_ALT_HEALTH) as u8,
            violation_bits: get_field(self.words[3], W3_VIOLATION_BITS) as u16,
        }
    }

    pub fn risk_flags(&self) -> u8 {
        get_field(self.words[2], W2_RISK_FLAGS) as u8
    }

    pub fn violation_bits(&self) -> u16 {
        get_field(self.words[3], W3_VIOLATION_BITS) as u16
    }

    fn gate_order_decision(&self, delta_qty: i32) -> GateDecision {
        let head = self.head();
        let session = self.session();
        let tail = self.tail();

        let extend = extends_position(head.position_qty, delta_qty);
        let reduce = reduces_position(head.position_qty, delta_qty);

        if tail.violation_bits != 0 {
            return if reduce {
                GateDecision::ReduceOnly
            } else {
                GateDecision::Deny(GateDeny::ViolationBits)
            };
        }

        if head.remaining_daily_loss_cents == 0 {
            return if reduce {
                GateDecision::ReduceOnly
            } else {
                GateDecision::Deny(GateDeny::DailyLoss)
            };
        }

        if head.flags & FLAG_HALT != 0 {
            return if reduce {
                GateDecision::ReduceOnly
            } else {
                GateDecision::Deny(GateDeny::Halted)
            };
        }

        if session.now_min_ct >= session.eod_flat_min_ct {
            return if reduce {
                GateDecision::ReduceOnly
            } else {
                GateDecision::Deny(GateDeny::SessionPastEod)
            };
        }

        if session.now_min_ct >= session.forbid_after_min_ct {
            return if reduce {
                GateDecision::ReduceOnly
            } else {
                GateDecision::Deny(GateDeny::SessionForbid)
            };
        }

        if session.max_contracts > 0 && extend {
            let target = head.position_qty.saturating_add(delta_qty).unsigned_abs();
            if target > session.max_contracts as u32 {
                return GateDecision::Deny(GateDeny::SizeLimit);
            }
        }

        if session.risk_flags != 0 || (head.flags & FLAG_LOCKED) != 0 {
            return GateDecision::ReduceOnly;
        }

        if session.max_open_ms > 0 && session.open_since_ms >= session.max_open_ms && extend {
            return GateDecision::ReduceOnly;
        }

        if tail.breaker_level >= BREAKER_REDUCE_ONLY_LEVEL {
            return GateDecision::ReduceOnly;
        }

        GateDecision::Allow
    }

    pub fn gate_order(&self, delta_qty: i32) -> GateDecision {
        self.gate_order_decision(delta_qty)
    }

    pub fn gate_order_with_metrics(&self, delta_qty: i32, metrics: &GateMetrics) -> GateDecision {
        let decision = self.gate_order_decision(delta_qty);
        metrics.record(decision);
        decision
    }

    pub fn checksum(&self) -> u16 {
        get_field(self.words[3], W3_CHECKSUM) as u16
    }

    pub fn tail_version(&self) -> u16 {
        get_field(self.words[3], W3_VER_TAIL) as u16
    }

    pub fn tail_sequence(&self) -> u16 {
        get_field(self.words[3], W3_SEQ_TAIL) as u16
    }

    pub fn words(&self) -> [u128; 4] {
        self.words
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DenyReason {
    OddVersion,
    InFlightRewrite,
    SeqMismatch,
    ChecksumMismatch,
    AttemptsExhausted,
}

#[derive(Default)]
pub struct DenyCounters {
    accepts: AtomicU64,
    odd_version: AtomicU64,
    inflight_rewrite: AtomicU64,
    seq_mismatch: AtomicU64,
    checksum_mismatch: AtomicU64,
    attempts_exhausted: AtomicU64,
}

impl DenyCounters {
    pub fn record_accept(&self) {
        let _ = self.accepts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record(&self, reason: DenyReason) {
        match reason {
            DenyReason::OddVersion => {
                let _ = self.odd_version.fetch_add(1, Ordering::Relaxed);
            }
            DenyReason::InFlightRewrite => {
                let _ = self.inflight_rewrite.fetch_add(1, Ordering::Relaxed);
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
            odd_version: self.odd_version.load(Ordering::Relaxed),
            inflight_rewrite: self.inflight_rewrite.load(Ordering::Relaxed),
            seq_mismatch: self.seq_mismatch.load(Ordering::Relaxed),
            checksum_mismatch: self.checksum_mismatch.load(Ordering::Relaxed),
            attempts_exhausted: self.attempts_exhausted.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DenySnapshot {
    pub accepts: u64,
    pub odd_version: u64,
    pub inflight_rewrite: u64,
    pub seq_mismatch: u64,
    pub checksum_mismatch: u64,
    pub attempts_exhausted: u64,
}

const _APC_ATOMIC_ALIGN: [u8; 64] = [0; core::mem::size_of::<AtomicPositionCapsule>()];
const _APC_SNAPSHOT_ALIGN: [u8; 64] = [0; core::mem::size_of::<Snapshot>()];
const _APC_DRAFT_ALIGN: [u8; 64] = [0; core::mem::size_of::<CapsuleDraft>()];

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

impl Default for AtomicPositionCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_round_trip() {
        let capsule = AtomicPositionCapsule::new();
        let snapshot = capsule.publish(
            PositionHeadWord {
                position_qty: 7,
                avg_px_ticks: -12,
                remaining_daily_loss_cents: 25_000,
                flags: 0b1010_0011,
            },
            EquityWord {
                realized_cents: 12_500,
                unrealized_cents: -3_200,
                peak_equity_cents: 15_000,
                trailing_draw_cents: 2_500,
            },
            SessionWord {
                now_min_ct: 820,
                forbid_after_min_ct: 905,
                eod_flat_min_ct: 910,
                open_since_ms: 45_000,
                max_open_ms: 120_000,
                max_contracts: 6,
                max_per_trade_cents: 75_000,
                risk_flags: 0b0101_0010,
                reserved_bits: 0,
            },
            TailWord {
                symbol_id: 17,
                account_id: 301,
                last_exec_id: 0xDEADBEEF,
                breaker_level: 2,
                alt_health: 12,
                violation_bits: 0b1010_1010_1010_1010,
            },
        );

        assert_eq!(snapshot.version() & 1, 0);
        assert_eq!(snapshot.tail_version(), snapshot.version());
        assert_eq!(snapshot.sequence(), snapshot.tail_sequence());

        let head = snapshot.head();
        assert_eq!(head.position_qty, 7);
        assert_eq!(head.avg_px_ticks, -12);
        assert_eq!(head.remaining_daily_loss_cents, 25_000);
        assert_eq!(head.flags, 0b1010_0011);

        let equity = snapshot.equity();
        assert_eq!(equity.realized_cents, 12_500);
        assert_eq!(equity.unrealized_cents, -3_200);
        assert_eq!(equity.peak_equity_cents, 15_000);
        assert_eq!(equity.trailing_draw_cents, 2_500);

        let session = snapshot.session();
        assert_eq!(session.now_min_ct, 820);
        assert_eq!(session.forbid_after_min_ct, 905);
        assert_eq!(session.eod_flat_min_ct, 910);
        assert_eq!(session.open_since_ms, 45_000);
        assert_eq!(session.max_open_ms, 120_000);
        assert_eq!(session.max_contracts, 6);
        assert_eq!(session.max_per_trade_cents, 75_000);
        assert_eq!(session.risk_flags, 0b0101_0010);

        let tail = snapshot.tail();
        assert_eq!(tail.symbol_id, 17);
        assert_eq!(tail.account_id, 301);
        assert_eq!(tail.last_exec_id, 0xDEADBEEF);
        assert_eq!(tail.breaker_level, 2);
        assert_eq!(tail.alt_health, 12);
        assert_eq!(tail.violation_bits, 0b1010_1010_1010_1010);

        let loaded = capsule.load().expect("capsule load");
        assert_eq!(loaded.words(), snapshot.words());
    }

    #[test]
    fn sequence_pair_tracks_publish() {
        let capsule = AtomicPositionCapsule::new();
        let mut draft = CapsuleDraft::new();
        let mut seen = 0u16;
        for idx in 0..16u16 {
            let snap = capsule.publish_with_reuse(&mut draft, |draft| {
                draft
                    .set_head(PositionHeadWord {
                        position_qty: idx as i32,
                        avg_px_ticks: idx as i32,
                        remaining_daily_loss_cents: 10_000,
                        flags: 0,
                    })
                    .set_equity(EquityWord {
                        realized_cents: idx as i32,
                        unrealized_cents: 0,
                        peak_equity_cents: idx as i32,
                        trailing_draw_cents: 0,
                    })
                    .set_session(SessionWord {
                        now_min_ct: 800,
                        forbid_after_min_ct: 900,
                        eod_flat_min_ct: 905,
                        open_since_ms: 1_000,
                        max_open_ms: 10_000,
                        max_contracts: 5,
                        max_per_trade_cents: 50_000,
                        risk_flags: 0,
                        reserved_bits: 0,
                    })
                    .set_tail(TailWord {
                        symbol_id: 1,
                        account_id: 2,
                        last_exec_id: idx as u32,
                        breaker_level: 1,
                        alt_health: 1,
                        violation_bits: 0,
                    });
            });
            let (head, tail) = capsule.sequence_pair();
            assert_eq!(head, tail);
            assert_ne!(head, seen);
            seen = head;
            assert_eq!(snap.sequence(), head);
        }
    }
    #[test]
    fn gate_metrics_recording() {
        let capsule = AtomicPositionCapsule::new();
        let mut draft = CapsuleDraft::new();
        let metrics = GateMetrics::new();

        let allow = capsule.publish_with_reuse(&mut draft, |draft| {
            draft
                .set_head(PositionHeadWord {
                    position_qty: 2,
                    avg_px_ticks: 10,
                    remaining_daily_loss_cents: 20_000,
                    flags: 0,
                })
                .set_equity(EquityWord::default())
                .set_session(SessionWord {
                    now_min_ct: 840,
                    forbid_after_min_ct: 905,
                    eod_flat_min_ct: 910,
                    open_since_ms: 20_000,
                    max_open_ms: 120_000,
                    max_contracts: 5,
                    max_per_trade_cents: 50_000,
                    risk_flags: 0,
                    reserved_bits: 0,
                })
                .set_tail(TailWord {
                    symbol_id: 1,
                    account_id: 1,
                    last_exec_id: 0,
                    breaker_level: 0,
                    alt_health: 0,
                    violation_bits: 0,
                });
        });
        assert_eq!(
            allow.gate_order_with_metrics(1, &metrics),
            GateDecision::Allow
        );

        let reduce = capsule.publish_with_reuse(&mut draft, |draft| {
            draft
                .set_head(PositionHeadWord {
                    position_qty: 2,
                    avg_px_ticks: 10,
                    remaining_daily_loss_cents: 20_000,
                    flags: FLAG_LOCKED,
                })
                .set_equity(EquityWord::default())
                .set_session(SessionWord {
                    now_min_ct: 902,
                    forbid_after_min_ct: 905,
                    eod_flat_min_ct: 910,
                    open_since_ms: 90_000,
                    max_open_ms: 120_000,
                    max_contracts: 5,
                    max_per_trade_cents: 50_000,
                    risk_flags: RISK_FLAG_PAUSE_NEWS,
                    reserved_bits: 0,
                })
                .set_tail(TailWord {
                    symbol_id: 1,
                    account_id: 1,
                    last_exec_id: 0,
                    breaker_level: 0,
                    alt_health: 0,
                    violation_bits: 0,
                });
        });
        assert_eq!(
            reduce.gate_order_with_metrics(1, &metrics),
            GateDecision::ReduceOnly
        );

        let deny = capsule.publish_with_reuse(&mut draft, |draft| {
            draft
                .set_head(PositionHeadWord {
                    position_qty: 2,
                    avg_px_ticks: 10,
                    remaining_daily_loss_cents: 0,
                    flags: 0,
                })
                .set_equity(EquityWord::default())
                .set_session(SessionWord {
                    now_min_ct: 903,
                    forbid_after_min_ct: 905,
                    eod_flat_min_ct: 910,
                    open_since_ms: 95_000,
                    max_open_ms: 120_000,
                    max_contracts: 5,
                    max_per_trade_cents: 50_000,
                    risk_flags: 0,
                    reserved_bits: 0,
                })
                .set_tail(TailWord {
                    symbol_id: 1,
                    account_id: 1,
                    last_exec_id: 0,
                    breaker_level: 0,
                    alt_health: 0,
                    violation_bits: 0,
                });
        });
        assert_eq!(
            deny.gate_order_with_metrics(1, &metrics),
            GateDecision::Deny(GateDeny::DailyLoss)
        );

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.allow, 1);
        assert_eq!(snapshot.reduce_only, 1);
        assert_eq!(snapshot.deny_daily_loss, 1);
        assert_eq!(snapshot.total_decisions(), 3);
    }

    #[test]
    fn gate_decision_transitions() {
        let capsule = AtomicPositionCapsule::new();
        let mut draft = CapsuleDraft::new();

        capsule.publish_with_reuse(&mut draft, |draft| {
            draft
                .set_head(PositionHeadWord {
                    position_qty: 2,
                    avg_px_ticks: 10,
                    remaining_daily_loss_cents: 10_000,
                    flags: 0,
                })
                .set_equity(EquityWord::default())
                .set_session(SessionWord {
                    now_min_ct: 840,
                    forbid_after_min_ct: 905,
                    eod_flat_min_ct: 910,
                    open_since_ms: 30_000,
                    max_open_ms: 120_000,
                    max_contracts: 5,
                    max_per_trade_cents: 50_000,
                    risk_flags: 0,
                    reserved_bits: 0,
                })
                .set_tail(TailWord {
                    symbol_id: 1,
                    account_id: 1,
                    last_exec_id: 0,
                    breaker_level: 0,
                    alt_health: 0,
                    violation_bits: 0,
                });
        });

        let base = capsule.load().unwrap();
        assert_eq!(base.gate_order(1), GateDecision::Allow);

        capsule.publish_with_reuse(&mut draft, |draft| {
            draft
                .set_head(PositionHeadWord {
                    position_qty: 2,
                    avg_px_ticks: 10,
                    remaining_daily_loss_cents: 0,
                    flags: 0,
                })
                .set_equity(EquityWord::default())
                .set_session(SessionWord {
                    now_min_ct: 840,
                    forbid_after_min_ct: 905,
                    eod_flat_min_ct: 910,
                    open_since_ms: 30_000,
                    max_open_ms: 120_000,
                    max_contracts: 5,
                    max_per_trade_cents: 50_000,
                    risk_flags: 0,
                    reserved_bits: 0,
                })
                .set_tail(TailWord {
                    symbol_id: 1,
                    account_id: 1,
                    last_exec_id: 0,
                    breaker_level: 0,
                    alt_health: 0,
                    violation_bits: 0,
                });
        });

        let depleted = capsule.load().unwrap();
        assert_eq!(
            depleted.gate_order(1),
            GateDecision::Deny(GateDeny::DailyLoss)
        );
        let reduce = depleted.gate_order(-2);
        assert_eq!(reduce, GateDecision::ReduceOnly);
        assert!(reduce.permits(depleted.head().position_qty, -2));

        capsule.publish_with_reuse(&mut draft, |draft| {
            draft
                .set_head(PositionHeadWord {
                    position_qty: 2,
                    avg_px_ticks: 10,
                    remaining_daily_loss_cents: 5_000,
                    flags: 0,
                })
                .set_equity(EquityWord::default())
                .set_session(SessionWord {
                    now_min_ct: 840,
                    forbid_after_min_ct: 905,
                    eod_flat_min_ct: 910,
                    open_since_ms: 30_000,
                    max_open_ms: 120_000,
                    max_contracts: 5,
                    max_per_trade_cents: 50_000,
                    risk_flags: 0,
                    reserved_bits: 0,
                })
                .set_tail(TailWord {
                    symbol_id: 1,
                    account_id: 1,
                    last_exec_id: 0,
                    breaker_level: 0,
                    alt_health: 0,
                    violation_bits: 1,
                });
        });

        let violation = capsule.load().unwrap();
        assert_eq!(
            violation.gate_order(1),
            GateDecision::Deny(GateDeny::ViolationBits)
        );
        assert_eq!(violation.gate_order(-2), GateDecision::ReduceOnly);
    }

    #[test]
    fn gate_reduce_only_paths() {
        let capsule = AtomicPositionCapsule::new();
        let mut draft = CapsuleDraft::new();

        capsule.publish_with_reuse(&mut draft, |draft| {
            draft
                .set_head(PositionHeadWord {
                    position_qty: 4,
                    avg_px_ticks: 100,
                    remaining_daily_loss_cents: 20_000,
                    flags: FLAG_LONG,
                })
                .set_equity(EquityWord::default())
                .set_session(SessionWord {
                    now_min_ct: 904,
                    forbid_after_min_ct: 905,
                    eod_flat_min_ct: 910,
                    open_since_ms: 100_000,
                    max_open_ms: 120_000,
                    max_contracts: 6,
                    max_per_trade_cents: 60_000,
                    risk_flags: 0,
                    reserved_bits: 0,
                })
                .set_tail(TailWord {
                    symbol_id: 2,
                    account_id: 3,
                    last_exec_id: 0,
                    breaker_level: 2,
                    alt_health: 0,
                    violation_bits: 0,
                });
        });

        let breaker = capsule.load().unwrap();
        assert_eq!(breaker.gate_order(2), GateDecision::ReduceOnly);
        assert!(!breaker
            .gate_order(2)
            .permits(breaker.head().position_qty, 2));
        assert!(breaker
            .gate_order(-4)
            .permits(breaker.head().position_qty, -4));

        capsule.publish_with_reuse(&mut draft, |draft| {
            draft
                .set_head(PositionHeadWord {
                    position_qty: 4,
                    avg_px_ticks: 100,
                    remaining_daily_loss_cents: 20_000,
                    flags: FLAG_LOCKED,
                })
                .set_equity(EquityWord::default())
                .set_session(SessionWord {
                    now_min_ct: 907,
                    forbid_after_min_ct: 905,
                    eod_flat_min_ct: 910,
                    open_since_ms: 130_000,
                    max_open_ms: 120_000,
                    max_contracts: 6,
                    max_per_trade_cents: 60_000,
                    risk_flags: RISK_FLAG_PAUSE_NEWS,
                    reserved_bits: 0,
                })
                .set_tail(TailWord {
                    symbol_id: 2,
                    account_id: 3,
                    last_exec_id: 0,
                    breaker_level: 1,
                    alt_health: 0,
                    violation_bits: 0,
                });
        });

        let locked = capsule.load().unwrap();
        assert_eq!(
            locked.gate_order(1),
            GateDecision::Deny(GateDeny::SessionForbid)
        );
        assert!(!locked.gate_order(1).permits(locked.head().position_qty, 1));
        assert!(locked
            .gate_order(-4)
            .permits(locked.head().position_qty, -4));
    }

    #[cfg(feature = "checksum")]
    #[test]
    fn checksum_mismatch_rejected() {
        use core::sync::atomic::Ordering as AtomicOrdering;

        let capsule = AtomicPositionCapsule::new();
        let snapshot = capsule.publish(
            PositionHeadWord::default(),
            EquityWord::default(),
            SessionWord::default(),
            TailWord::default(),
        );
        assert!(capsule.load().is_some());
        let corrupt = snapshot.words();
        capsule.words[3].store(corrupt[3] ^ 0x1, AtomicOrdering::Release);
        assert!(capsule.load().is_none());
    }
}
