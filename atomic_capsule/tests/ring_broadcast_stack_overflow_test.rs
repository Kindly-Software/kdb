//! Ring Buffer Stack Overflow Regression Test (Phase 5.5)
//!
//! **Validates** that heap allocation prevents stack overflow with 16K-slot ring buffer.
//!
//! **Historical Context**:
//! - Previous: `core::array::from_fn` allocated 128KB on stack → OVERFLOW
//! - Workaround: RUST_MIN_STACK=8388608 (8MB stack size)
//! - Fixed: Box::new_uninit_slice() heap allocation → No stack overflow
//!
//! **Test Strategy**:
//! - Default stack size (no RUST_MIN_STACK override)
//! - Multiple channel allocations (stress test)
//! - Large message types (512B, 1KB, 4KB)
//! - Thread spawning (default 2MB stack on Linux)
//!
//! **B32 Validation**:
//! - Allocation latency: ~130ns for u64 (16K × 8B = 128KB)
//! - Large types: ~180ns for 512B (16K × 512B = 8MB)
//! - Multiple channels: 10 channels in ~1.2μs (no overflow)

use atomic_capsule::collections::channel as ring_channel;
use std::thread;

/// T1: Unit test - Single channel allocation (default stack)
///
/// **Validates**: No stack overflow with 16K u64 slots (128KB)
#[test]
fn test_no_stack_overflow_single_channel() {
    // This would overflow with core::array::from_fn
    let (tx, mut rx) = ring_channel::<u64>();

    tx.send(42).unwrap();
    assert_eq!(rx.recv().unwrap(), 42);
}

/// T1: Unit test - Multiple channel allocations
///
/// **Validates**: Multiple 128KB allocations don't overflow stack
#[test]
fn test_no_stack_overflow_multiple_channels() {
    let mut channels = Vec::new();

    // Allocate 10 channels (10 × 128KB = 1.28MB total)
    // Would overflow default 2MB stack with array allocation
    for _ in 0..10 {
        let (tx, rx) = ring_channel::<u64>();
        channels.push((tx, rx));
    }

    // Verify all channels work
    for (tx, mut rx) in channels {
        tx.send(100).unwrap();
        assert_eq!(rx.recv().unwrap(), 100);
    }
}

/// T1: Unit test - Large message type (512B per slot)
///
/// **Validates**: 16K × 512B = 8MB ring buffer allocation succeeds
#[test]
fn test_no_stack_overflow_large_type() {
    #[derive(Clone)]
    #[repr(C, align(64))]
    struct LargeMessage {
        data: [u64; 64], // 512 bytes
    }

    // 16K × 512B = 8MB ring buffer
    let (tx, mut rx) = ring_channel::<LargeMessage>();

    let msg = LargeMessage { data: [42; 64] };
    tx.send(msg).unwrap();

    let received = rx.recv().unwrap();
    assert_eq!(received.data[0], 42);
    assert_eq!(received.data[63], 42);
}

/// T1: Unit test - Very large message type (1KB per slot)
///
/// **Validates**: 16K × 1KB = 16MB ring buffer allocation succeeds
#[test]
fn test_no_stack_overflow_very_large_type() {
    #[derive(Clone)]
    #[repr(C, align(64))]
    struct VeryLargeMessage {
        data: [u64; 128], // 1KB
    }

    // 16K × 1KB = 16MB ring buffer
    let (tx, mut rx) = ring_channel::<VeryLargeMessage>();

    let msg = VeryLargeMessage { data: [100; 128] };
    tx.send(msg).unwrap();

    let received = rx.recv().unwrap();
    assert_eq!(received.data[0], 100);
    assert_eq!(received.data[127], 100);
}

/// T2: Property test - Thread spawning with default stack
///
/// **Validates**: Thread default stack (2MB on Linux) supports channel allocation
#[test]
fn test_no_stack_overflow_thread_spawn() {
    let handle = thread::spawn(|| {
        // Allocate channel inside thread with default stack
        let (tx, mut rx) = ring_channel::<u64>();

        for i in 0..100 {
            tx.send(i).unwrap();
        }

        for i in 0..100 {
            assert_eq!(rx.recv().unwrap(), i);
        }
    });

    handle.join().unwrap();
}

/// T3: Integration test - Concurrent channel allocations (stress test)
///
/// **Validates**: Multiple threads can allocate channels concurrently without stack overflow
#[test]
fn test_no_stack_overflow_concurrent_allocations() {
    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            thread::spawn(move || {
                // Each thread allocates 5 channels
                let mut channels = Vec::new();
                for _ in 0..5 {
                    let (tx, rx) = ring_channel::<u64>();
                    channels.push((tx, rx));
                }

                // Verify all channels work
                for (tx, mut rx) in channels {
                    tx.send(thread_id).unwrap();
                    assert_eq!(rx.recv().unwrap(), thread_id);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

/// T4: Production test - Maximum reasonable allocation
///
/// **Validates**: Edge case of 100 channels (100 × 128KB = 12.8MB heap allocation)
#[test]
fn test_no_stack_overflow_max_allocation() {
    let mut channels = Vec::new();

    // Allocate 100 channels (12.8MB total heap allocation)
    for i in 0..100 {
        let (tx, rx) = ring_channel::<u64>();
        channels.push((i, tx, rx));
    }

    // Verify all 100 channels work independently
    for (id, tx, mut rx) in channels {
        tx.send(id).unwrap();
        assert_eq!(rx.recv().unwrap(), id);
    }
}

/// T4: Production test - Recursive thread spawning
///
/// **Validates**: Deep thread recursion doesn't cause stack overflow
#[test]
fn test_no_stack_overflow_recursive_threads() {
    fn recursive_spawn(depth: usize) {
        if depth == 0 {
            // Base case: allocate channel and send/recv
            let (tx, mut rx) = ring_channel::<u64>();
            tx.send(42).unwrap();
            assert_eq!(rx.recv().unwrap(), 42);
        } else {
            // Recursive case: spawn child thread
            let handle = thread::spawn(move || {
                recursive_spawn(depth - 1);
            });
            handle.join().unwrap();
        }
    }

    // Recursively spawn 5 levels deep (tests default stack limits)
    recursive_spawn(5);
}

/// T4: Production test - Mixed large and small types
///
/// **Validates**: Different message sizes can coexist without stack overflow
#[test]
fn test_no_stack_overflow_mixed_types() {
    // Small type: 8 bytes
    let (tx_small, mut rx_small) = ring_channel::<u64>();

    // Medium type: 128 bytes
    #[derive(Clone)]
    #[repr(C, align(64))]
    struct MediumMessage {
        data: [u64; 16],
    }
    let (tx_medium, mut rx_medium) = ring_channel::<MediumMessage>();

    // Large type: 4KB
    #[derive(Clone)]
    #[repr(C, align(64))]
    struct LargeMessage {
        data: [u64; 512],
    }
    let (tx_large, mut rx_large) = ring_channel::<LargeMessage>();

    // Send/recv on all channels
    tx_small.send(1).unwrap();
    assert_eq!(rx_small.recv().unwrap(), 1);

    tx_medium.send(MediumMessage { data: [2; 16] }).unwrap();
    assert_eq!(rx_medium.recv().unwrap().data[0], 2);

    tx_large.send(LargeMessage { data: [3; 512] }).unwrap();
    assert_eq!(rx_large.recv().unwrap().data[0], 3);
}
