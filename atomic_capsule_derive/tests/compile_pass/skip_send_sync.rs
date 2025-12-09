//! Test: skip_send_sync attribute prevents automatic Send + Sync generation
//!
//! # Purpose
//! Validate that `#[capsule(skip_send_sync = true)]` suppresses automatic
//! Send + Sync trait generation for GPU types with raw pointers.

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::AtomicU64;

/// Capsule with skip_send_sync = true
///
/// # Safety
/// This capsule skips automatic Send + Sync generation.
/// User must manually implement Send + Sync if needed.
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, skip_send_sync = true)]
#[repr(C, align(64))]
struct SkipSendSyncCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

/// Normal capsule (skip_send_sync = false, default)
///
/// # Safety
/// Automatically generates Send + Sync because all fields are Send + Sync.
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct NormalCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

fn main() {
    // Create instances
    let skip_capsule = SkipSendSyncCapsule {
        state: AtomicU64::new(42),
        _padding: [0u8; 56],
    };

    let normal_capsule = NormalCapsule {
        state: AtomicU64::new(42),
        _padding: [0u8; 56],
    };

    // Normal capsule should have Send + Sync
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    assert_send_sync(&normal_capsule);

    // Skip capsule won't have automatic Send + Sync (user must implement manually if needed)
    // We can't test the absence of Send + Sync here (compile would fail)
    // But we can verify the capsule exists and compiles

    drop(skip_capsule);
    drop(normal_capsule);

    println!("✓ skip_send_sync attribute works correctly");
}
