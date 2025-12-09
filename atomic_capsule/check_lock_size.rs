use std::mem::{size_of, align_of};
use std::sync::atomic::AtomicU64;

#[repr(C)]
struct DualAtomicU64 {
    primary: AtomicU64,
    secondary: AtomicU64,
}

#[repr(C, align(64))]
struct DaemonLockCapsule {
    state: DualAtomicU64,
    heartbeat: AtomicU64,
    timeout_ns: u64,
    acquires: AtomicU64,
    contentions: AtomicU64,
    stale_recoveries: AtomicU64,
    _padding: [u8; 8],
}

fn main() {
    println!("DaemonLockCapsule: {} bytes", size_of::<DaemonLockCapsule>());
    println!("DaemonLockCapsule alignment: {} bytes", align_of::<DaemonLockCapsule>());
}
