use std::mem::{size_of, align_of};

#[repr(C)]
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

#[repr(C)]
pub struct RiskComponents {
    pub intrusion_risk: u16,
    pub license_risk: u16,
    pub session_risk: u16,
    pub rate_limit_risk: u16,
    pub anomaly_risk: u16,
    pub totp_risk: u16,
    pub pid_access_risk: u16,
    pub _reserved: u16,
}

#[repr(C)]
pub struct RiskScore {
    pub total_risk: u16,
    pub component_risks: RiskComponents,
    pub _reserved: [u16; 7],
}

fn main() {
    println!("CacheStatsSnapshot: {} bytes, align {}", size_of::<CacheStatsSnapshot>(), align_of::<CacheStatsSnapshot>());
    println!("CacheStatsToolCapsule: {} bytes, align {}", size_of::<CacheStatsToolCapsule>(), align_of::<CacheStatsToolCapsule>());
    println!("RiskComponents: {} bytes, align {}", size_of::<RiskComponents>(), align_of::<RiskComponents>());
    println!("RiskScore: {} bytes, align {}", size_of::<RiskScore>(), align_of::<RiskScore>());
}
