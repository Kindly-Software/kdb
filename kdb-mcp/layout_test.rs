use std::mem::{size_of, align_of, offset_of};

#[repr(C, align(32))]
pub struct CacheStatsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub total_bytes: u64,
    pub entry_count: u32,
    pub hit_ratio: u32,
}

#[repr(C, align(32))]
pub struct CacheStatsToolCapsule {
    pub generation: u64,
    pub snapshot_timestamp: u64,
    pub stats: CacheStatsSnapshot,
    pub _reserved: [u8; 0],
}

fn main() {
    println!("CacheStatsSnapshot: size={}, align={}", size_of::<CacheStatsSnapshot>(), align_of::<CacheStatsSnapshot>());
    println!("CacheStatsToolCapsule: size={}, align={}", size_of::<CacheStatsToolCapsule>(), align_of::<CacheStatsToolCapsule>());
    // Manual layout calculation
    // offset 0-7: generation (u64 = 8)
    // offset 8-15: snapshot_timestamp (u64 = 8)
    // offset 16-47: stats (CacheStatsSnapshot = 32, align 32 -> pad to 32 boundary = 16 padding + 32 = 48 total)
    println!("Manual calc: 16 + 32 = 48, padded to 64 with align(32) = 64");
}
