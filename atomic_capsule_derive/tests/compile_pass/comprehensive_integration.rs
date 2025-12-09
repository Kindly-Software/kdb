//! Test: Comprehensive integration test (all features)
//!
//! T28 Q15-Q18 (Integration Testing): All capsule features together
//! UCE34 Q10: Real-world capsule with multiple features
//!
//! Expected: Compilation succeeds with full feature set

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, AtomicU32, AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128, tier = "Mixed")]
#[repr(C, align(128))]
struct ComprehensiveCapsule {
    // Generation counter for TOCTOU prevention
    generation: AtomicU64,

    // Primary state
    state: AtomicU64,

    // Counters
    success_count: AtomicU32,
    error_count: AtomicU32,

    // Flags
    active: AtomicBool,
    shutdown: AtomicBool,

    // Padding to 128 bytes
    _padding: [u8; 102],
}

impl ComprehensiveCapsule {
    fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            state: AtomicU64::new(0),
            success_count: AtomicU32::new(0),
            error_count: AtomicU32::new(0),
            active: AtomicBool::new(true),
            shutdown: AtomicBool::new(false),
            _padding: [0u8; 102],
        }
    }

    fn update_state(&self, new_state: u64) {
        // Atomic update with generation counter
        self.generation.fetch_add(1, Ordering::Release);
        self.state.store(new_state, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.success_count.fetch_add(1, Ordering::Relaxed);
    }

    fn read_state_consistent(&self) -> Option<u64> {
        // Consistent read using generation counter
        for _ in 0..10 {
            let gen_before = self.generation.load(Ordering::Acquire);
            let state = self.state.load(Ordering::Relaxed);
            let gen_after = self.generation.load(Ordering::Acquire);

            if gen_before == gen_after && gen_before % 2 == 0 {
                return Some(state);
            }
        }
        None
    }
}

fn main() {
    use core::mem::{size_of, align_of};

    // Verify size and alignment
    assert_eq!(size_of::<ComprehensiveCapsule>(), 128);
    assert_eq!(align_of::<ComprehensiveCapsule>(), 128);

    // Verify traits
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<ComprehensiveCapsule>();
    assert_sync::<ComprehensiveCapsule>();

    let capsule = Arc::new(ComprehensiveCapsule::new());

    // Concurrent writers
    let writers: Vec<_> = (0..4)
        .map(|i| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for j in 0..100 {
                    c.update_state((i * 100 + j) as u64);
                    thread::yield_now();
                }
            })
        })
        .collect();

    // Concurrent readers
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                let mut successful_reads = 0;
                for _ in 0..1000 {
                    if c.read_state_consistent().is_some() {
                        successful_reads += 1;
                    }
                    thread::yield_now();
                }
                successful_reads
            })
        })
        .collect();

    // Wait for all threads
    for w in writers {
        w.join().unwrap();
    }

    let mut total_reads = 0;
    for r in readers {
        total_reads += r.join().unwrap();
    }

    println!("Comprehensive integration test passed!");
    println!("Success count: {}", capsule.success_count.load(Ordering::Relaxed));
    println!("Successful consistent reads: {}", total_reads);
    println!("Active: {}", capsule.active.load(Ordering::Relaxed));
}
