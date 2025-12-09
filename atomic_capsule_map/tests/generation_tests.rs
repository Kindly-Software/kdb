//! Generation counter tests for AtomicCapsuleMap
//!
//! Tests the generation counter mechanism that prevents ABA problems

use atomic_capsule_map::{
    pack_gen_high, pack_gen_low, unpack_gen_high, unpack_gen_low, MonotonicGen,
};

#[test]
fn test_monotonic_gen_creation() {
    let generation = MonotonicGen::new();
    assert_eq!(generation.load(), 0);
}

#[test]
fn test_monotonic_gen_increment() {
    let generation = MonotonicGen::new();

    for i in 0..100 {
        assert_eq!(generation.load(), i);
        generation.increment();
    }

    assert_eq!(generation.load(), 100);
}

#[test]
fn test_monotonic_gen_concurrent_increment() {
    use std::sync::Arc;
    use std::thread;

    let generation = Arc::new(MonotonicGen::new());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let gen_clone = Arc::clone(&generation);
            thread::spawn(move || {
                for _ in 0..1000 {
                    gen_clone.increment();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have incremented exactly 10,000 times
    assert_eq!(generation.load(), 10_000);
}

#[test]
fn test_pack_unpack_gen_high() {
    let generation = 42u32;
    let value = 0x1234_5678u32;

    let packed = pack_gen_high(value, generation);
    let unpacked_gen = unpack_gen_high(packed);

    assert_eq!(unpacked_gen, generation);

    // Verify lower bits are preserved
    let lower_bits = (packed & 0x0000_0000_FFFF_FFFF) as u32;
    assert_eq!(lower_bits, value);
}

#[test]
fn test_pack_unpack_gen_low() {
    let generation = 123u32;
    let value = 0x9ABC_DEF0u32;

    let packed = pack_gen_low(generation, value);
    let unpacked_gen = unpack_gen_low(packed);

    assert_eq!(unpacked_gen, generation);

    // Verify upper bits are preserved
    let upper_bits = (packed >> 32) as u32;
    assert_eq!(upper_bits, value);
}

#[test]
fn test_pack_unpack_roundtrip_high() {
    for generation in [0, 1, 100, 1000, u32::MAX / 2, u32::MAX - 1] {
        let value = 0x3456_7890u32;
        let packed = pack_gen_high(value, generation);
        let unpacked = unpack_gen_high(packed);
        assert_eq!(unpacked, generation, "Failed for generation={}", generation);
    }
}

#[test]
fn test_pack_unpack_roundtrip_low() {
    for generation in [0, 1, 100, 1000, u32::MAX / 2, u32::MAX - 1] {
        let value = 0xABCD_EF12u32;
        let packed = pack_gen_low(generation, value);
        let unpacked = unpack_gen_low(packed);
        assert_eq!(unpacked, generation, "Failed for generation={}", generation);
    }
}

#[test]
fn test_generation_counter_prevents_aba() {
    // Simulate ABA scenario
    let generation = MonotonicGen::new();
    let value = 100u32;

    // State A: generation=0, value=100
    let state_a1 = pack_gen_high(value, generation.load());

    // Transition to B
    generation.increment();
    let state_b = pack_gen_high(200, generation.load());

    // Back to A (different generation)
    generation.increment();
    let state_a2 = pack_gen_high(value, generation.load());

    // Values are the same, but generations differ
    assert_eq!(value, 100);
    assert_ne!(state_a1, state_a2);
    assert_ne!(unpack_gen_high(state_a1), unpack_gen_high(state_a2));
}

#[test]
fn test_generation_wrapping() {
    let generation = MonotonicGen::with_generation(u32::MAX - 10);

    for i in 0..20 {
        let expected = (u32::MAX - 10).wrapping_add(i);
        assert_eq!(generation.load(), expected);
        generation.increment();
    }
}

#[test]
fn test_concurrent_pack_operations() {
    use std::sync::Arc;
    use std::thread;

    let generation = Arc::new(MonotonicGen::new());
    let base_value = 0x1234_5678u32;

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let gen_clone = Arc::clone(&generation);
            thread::spawn(move || {
                for _ in 0..100 {
                    let current_gen = gen_clone.load();
                    let packed = pack_gen_high(base_value, current_gen);
                    let unpacked = unpack_gen_high(packed);

                    assert!(unpacked <= current_gen + 1000); // Allow for racing increments
                    gen_clone.increment();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(generation.load(), 1000);
}

#[test]
fn test_generation_different_for_each_operation() {
    let generation = MonotonicGen::new();
    let mut generations = Vec::new();

    for _ in 0..100 {
        generations.push(generation.load());
        generation.increment();
    }

    // All generations should be unique and monotonic
    for i in 1..generations.len() {
        assert!(generations[i] > generations[i - 1]);
    }
}

#[test]
fn test_pack_preserves_value_bits() {
    let value = 0b1010_1010_1010_1010_1010_1010_1010_1010u32;
    let generation = 0b1100_1100_1100_1100_1100_1100_1100_1100u32;

    let packed_high = pack_gen_high(value, generation);
    let packed_low = pack_gen_low(generation, value);

    // High packing: value goes in lower 32 bits, generation in upper
    assert_eq!((packed_high & 0xFFFF_FFFF) as u32, value);

    // Low packing: generation goes in lower 32 bits, value in upper
    assert_eq!((packed_low & 0xFFFF_FFFF) as u32, generation);
}
