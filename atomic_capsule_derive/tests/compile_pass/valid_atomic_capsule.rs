//! Valid atomic capsule - should compile successfully

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct CircuitBreakerCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

fn main() {
    let capsule = CircuitBreakerCapsule {
        state: AtomicU64::new(0),
        _padding: [0u8; 56],
    };

    // Verify traits are implemented
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<CircuitBreakerCapsule>();
    assert_sync::<CircuitBreakerCapsule>();

    println!("Valid atomic capsule compiled successfully!");
}
