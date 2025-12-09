use core::mem::size_of;

use static_assertions::const_assert_eq;

/// Magic bytes that identify an ET-1kB tile on disk.
pub const TILE_MAGIC: [u8; 4] = *b"ET1K";
/// Current layout version of the ET-1kB tile definition.
pub const TILE_LAYOUT_VERSION: u8 = 1;
/// Number of symbol slices carried in the `S2` section.
pub const SYMBOL_SLICE_COUNT: usize = 4;
/// Number of mini-log entries in the `L3` section.
pub const MINI_LOG_CAPACITY: usize = 8;
/// Total size of a tile in bytes.
pub const TILE_SIZE: usize = size_of::<EtTile>();

#[repr(C, align(64))]
#[derive(Clone, Debug)]
pub struct EtTile {
    pub header: HeaderSection,
    pub counters: CountersSection,
    pub symbols: SymbolSection,
    pub log: LogSection,
}

impl Default for EtTile {
    fn default() -> Self {
        Self {
            header: HeaderSection::default(),
            counters: CountersSection::default(),
            symbols: SymbolSection::default(),
            log: LogSection::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HeaderSection {
    pub magic: [u8; 4],
    pub layout_version: u8,
    pub commit: u8,
    pub ver_even: u8,
    pub seq_head: u8,
    pub epoch_id: u64,
    pub created_ms: u64,
    pub run_id: u128,
    pub policy_id: u16,
    pub account_id: u16,
    pub tz_id: u8,
    pub symbol_mask: u8,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub applied_level: u8,
    pub global_flags: u8,
    pub prev_tile_hash: [u8; 16],
    pub ale_tail_hash: u64,
    pub capsule_digests: [u64; 8],
    pub checksum32: u32,
    pub reserved: [u8; 100],
}

impl Default for HeaderSection {
    fn default() -> Self {
        let mut header = Self {
            magic: TILE_MAGIC,
            layout_version: TILE_LAYOUT_VERSION,
            commit: 0,
            ver_even: 0,
            seq_head: 0,
            epoch_id: 0,
            created_ms: 0,
            run_id: 0,
            policy_id: 0,
            account_id: 0,
            tz_id: 0,
            symbol_mask: 0,
            forbid_after_min_ct: 0,
            eod_flat_min_ct: 0,
            applied_level: 0,
            global_flags: 0,
            prev_tile_hash: [0; 16],
            ale_tail_hash: 0,
            capsule_digests: [0; 8],
            checksum32: 0,
            reserved: [0; 100],
        };
        header.magic = TILE_MAGIC;
        header.layout_version = TILE_LAYOUT_VERSION;
        header
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CountersSection {
    pub orders_sent: u32,
    pub acks: u32,
    pub fills: u32,
    pub cancels: u32,
    pub rejects: u32,
    pub maker_sends: u32,
    pub taker_sends: u32,
    pub reduce_only: u32,
    pub qty_traded: i32,
    pub trades_won: u32,
    pub trades_lost: u32,
    pub realized_cents: i64,
    pub unreal_cents: i64,
    pub fees_cents: i64,
    pub slip_mbp_sum: i64,
    pub slip_mbp_abs_sum: u64,
    pub peak_equity_cents: i64,
    pub max_draw_cents: i64,
    pub lat_d2a_us_p50: u16,
    pub lat_d2a_us_p90: u16,
    pub lat_d2a_us_p99: u16,
    pub lat_a2f_us_p50: u16,
    pub lat_a2f_us_p90: u16,
    pub lat_a2f_us_p99: u16,
    pub rej_rate_bp: u16,
    pub cxl_rate_bp: u16,
    pub loss_bp: u16,
    pub jitter_us: u16,
    pub lat_hist8: [u32; 8],
    pub slip_hist8: [u32; 8],
    pub reserved: [u8; 68],
}

impl Default for CountersSection {
    fn default() -> Self {
        Self {
            orders_sent: 0,
            acks: 0,
            fills: 0,
            cancels: 0,
            rejects: 0,
            maker_sends: 0,
            taker_sends: 0,
            reduce_only: 0,
            qty_traded: 0,
            trades_won: 0,
            trades_lost: 0,
            realized_cents: 0,
            unreal_cents: 0,
            fees_cents: 0,
            slip_mbp_sum: 0,
            slip_mbp_abs_sum: 0,
            peak_equity_cents: 0,
            max_draw_cents: 0,
            lat_d2a_us_p50: 0,
            lat_d2a_us_p90: 0,
            lat_d2a_us_p99: 0,
            lat_a2f_us_p50: 0,
            lat_a2f_us_p90: 0,
            lat_a2f_us_p99: 0,
            rej_rate_bp: 0,
            cxl_rate_bp: 0,
            loss_bp: 0,
            jitter_us: 0,
            lat_hist8: [0; 8],
            slip_hist8: [0; 8],
            reserved: [0; 68],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SymbolSection {
    pub slots: [SymbolSlice; SYMBOL_SLICE_COUNT],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SymbolSlice {
    pub sym_id: u16,
    pub breaker_level: u8,
    pub flags: u8,
    pub pos_qty: i32,
    pub avg_px_ticks: i32,
    pub realized_cents: i64,
    pub unreal_cents: i64,
    pub rem_daily_loss_cents: u32,
    pub trailing_draw_cents: u32,
    pub spread_ticks: u8,
    pub vol_bp_q8_8: u16,
    pub obi_q1_10: i16,
    pub last_exec_id: u32,
    pub sum_bid_l1_3: u16,
    pub sum_ask_l1_3: u16,
    pub reserved: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct LogSection {
    pub entries: [LogEntry; MINI_LOG_CAPACITY],
    pub tail: LogTail,
}

impl Default for LogSection {
    fn default() -> Self {
        Self {
            entries: [LogEntry::default(); MINI_LOG_CAPACITY],
            tail: LogTail::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LogEntry {
    pub ts_ms: u32,
    pub event: u8,
    pub actor: u8,
    pub sym_id: u16,
    pub code: i32,
    pub aux: i32,
    pub flags: u8,
    pub pad: [u8; 7],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LogTail {
    pub mini_head: u8,
    pub mini_count: u8,
    pub reserved0: [u8; 6],
    pub now_min_ct: u16,
    pub next_lockout_min_ct: u16,
    pub next_resume_min_ct: u16,
    pub eco_action_now: u8,
    pub apm_summary: u32,
    pub ver_tail: u8,
    pub seq_tail: u8,
    pub tile_index: u16,
    pub file_offset: u64,
    pub spare: [u8; 32],
}

impl Default for LogTail {
    fn default() -> Self {
        Self {
            mini_head: 0,
            mini_count: 0,
            reserved0: [0; 6],
            now_min_ct: 0,
            next_lockout_min_ct: 0,
            next_resume_min_ct: 0,
            eco_action_now: 0,
            apm_summary: 0,
            ver_tail: 0,
            seq_tail: 0,
            tile_index: 0,
            file_offset: 0,
            spare: [0; 32],
        }
    }
}

const_assert_eq!(size_of::<HeaderSection>(), 256);
const_assert_eq!(size_of::<CountersSection>(), 256);
const_assert_eq!(size_of::<SymbolSection>(), 256);
const_assert_eq!(size_of::<LogSection>(), 256);
const_assert_eq!(size_of::<EtTile>(), 1024);
