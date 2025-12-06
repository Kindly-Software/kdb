//! T9 Persistent - Crash dumps and checkpoints
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[repr(C, align(256))]
pub struct MmapCrashDumpCapsule {
    pub crash_timestamp_ns: AtomicU64,
    pub signal: AtomicU32,
    pub fault_addr: AtomicU64,
    pub rip: AtomicU64,
    pub rsp: AtomicU64,
    pub rbp: AtomicU64,
    pub pid: AtomicU64,
    pub tid: AtomicU64,
    pub registers: [AtomicU64; 32],
    pub stack_data: [AtomicU64; 2048],
    pub memory_dump: [AtomicU64; 6144],
}

impl MmapCrashDumpCapsule {
    pub fn new() -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            crash_timestamp_ns: AtomicU64::new(0),
            signal: AtomicU32::new(0),
            fault_addr: AtomicU64::new(0),
            rip: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
            rbp: AtomicU64::new(0),
            pid: AtomicU64::new(0),
            tid: AtomicU64::new(0),
            registers: [ZERO; 32],
            stack_data: [ZERO; 2048],
            memory_dump: [ZERO; 6144],
        }
    }

    pub fn record_crash(&self, signal: u32, fault_addr: u64, rip: u64, rsp: u64, rbp: u64) {
        self.crash_timestamp_ns.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            Ordering::Release,
        );
        self.signal.store(signal, Ordering::Release);
        self.fault_addr.store(fault_addr, Ordering::Release);
        self.rip.store(rip, Ordering::Release);
        self.rsp.store(rsp, Ordering::Release);
        self.rbp.store(rbp, Ordering::Release);
    }

    pub fn get_crash_info(&self) -> (u32, u64, u64) {
        (
            self.signal.load(Ordering::Acquire),
            self.fault_addr.load(Ordering::Acquire),
            self.rip.load(Ordering::Acquire),
        )
    }
}

#[repr(C, align(64))]
pub struct CheckpointEntry {
    pub checkpoint_id: AtomicU64,
    pub timestamp_ns: AtomicU64,
    pub rip: AtomicU64,
    pub rsp: AtomicU64,
    pub registers: [AtomicU64; 64],
    pub flags: AtomicU32,
    _padding: [u8; 640 - (4 + 64) * 8 - 4 - 4],
}

impl CheckpointEntry {
    pub const fn empty() -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            checkpoint_id: AtomicU64::new(0),
            timestamp_ns: AtomicU64::new(0),
            rip: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
            registers: [ZERO; 64],
            flags: AtomicU32::new(0),
            _padding: [0; 640 - (4 + 64) * 8 - 4 - 4],
        }
    }

    pub fn save(&self, checkpoint_id: u64, rip: u64, rsp: u64) {
        self.checkpoint_id.store(checkpoint_id, Ordering::Release);
        self.timestamp_ns.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            Ordering::Relaxed,
        );
        self.rip.store(rip, Ordering::Release);
        self.rsp.store(rsp, Ordering::Release);
        self.flags.store(1, Ordering::Release);
    }

    pub fn is_active(&self) -> bool {
        self.flags.load(Ordering::Acquire) != 0
    }
}
