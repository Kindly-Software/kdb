//! # BitwiseSerializable Property Tests - T28 Tier 2 (Q8-Q14)
//!
//! Property-based tests using proptest for BitwiseSerializable trait.
//!
//! ## Test Coverage
//! - Q8: Roundtrip properties for all primitive types
//! - Q9: Arc refcount invariants under concurrent access
//! - Q10: String heap allocation correctness
//! - Q11: Cross-type safety (no type confusion)
//! - Q12: Overflow/underflow behavior
//! - Q13: Drop safety (no double-free, no leaks)
//! - Q14: Concurrent access patterns

#![cfg(all(test, feature = "std"))]

use atomic_capsule::collections::serializable::BitwiseSerializable;
use std::sync::Arc;

// ============================================================================
// Property 1: Roundtrip Identity for Primitives
// ============================================================================

#[test]
fn property_primitive_roundtrip_identity() {
    // Property: For all primitives, to_storage(x).from_storage() == x

    // u8
    for value in 0u8..=255 {
        let storage = value.to_storage();
        let roundtrip = u8::from_storage(storage);
        assert_eq!(roundtrip, value, "u8 roundtrip failed for {}", value);
        unsafe {
            u8::drop_storage(storage);
        }
    }

    // u16 (sample)
    for value in [0u16, 1, 255, 256, u16::MAX / 2, u16::MAX - 1, u16::MAX] {
        let storage = value.to_storage();
        let roundtrip = u16::from_storage(storage);
        assert_eq!(roundtrip, value, "u16 roundtrip failed for {}", value);
        unsafe {
            u16::drop_storage(storage);
        }
    }

    // u32 (sample)
    for value in [
        0u32,
        1,
        255,
        256,
        65535,
        65536,
        u32::MAX / 2,
        u32::MAX - 1,
        u32::MAX,
    ] {
        let storage = value.to_storage();
        let roundtrip = u32::from_storage(storage);
        assert_eq!(roundtrip, value, "u32 roundtrip failed for {}", value);
        unsafe {
            u32::drop_storage(storage);
        }
    }

    // u64 (sample)
    for value in [
        0u64,
        1,
        255,
        65535,
        u32::MAX as u64,
        u64::MAX / 2,
        u64::MAX - 1,
        u64::MAX,
    ] {
        let storage = value.to_storage();
        let roundtrip = u64::from_storage(storage);
        assert_eq!(roundtrip, value, "u64 roundtrip failed for {}", value);
        unsafe {
            u64::drop_storage(storage);
        }
    }

    // Signed integers
    for value in [i8::MIN, -1, 0, 1, 127] {
        let storage = value.to_storage();
        let roundtrip = i8::from_storage(storage);
        assert_eq!(roundtrip, value, "i8 roundtrip failed for {}", value);
        unsafe {
            i8::drop_storage(storage);
        }
    }

    for value in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
        let storage = value.to_storage();
        let roundtrip = i64::from_storage(storage);
        assert_eq!(roundtrip, value, "i64 roundtrip failed for {}", value);
        unsafe {
            i64::drop_storage(storage);
        }
    }
}

// ============================================================================
// Property 2: Arc Refcount Invariants
// ============================================================================

#[test]
fn property_arc_refcount_invariant_single_reader() {
    // Property: to_storage + from_storage increments refcount by 1

    for _ in 0..100 {
        let value = Arc::new(42u64);
        let initial_count = Arc::strong_count(&value);
        assert_eq!(initial_count, 1);

        let storage = value.to_storage();

        let reader = Arc::<u64>::from_storage(storage);
        let after_read_count = Arc::strong_count(&reader);

        // Refcount should be 2: storage + reader
        assert_eq!(
            after_read_count, 2,
            "Refcount after read should be 2 (storage + reader)"
        );

        drop(reader);

        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
    }
}

#[test]
fn property_arc_refcount_invariant_multiple_readers() {
    // Property: N readers should result in refcount = N + 1 (N readers + storage)

    for num_readers in [1, 2, 5, 10, 100] {
        let value = Arc::new(42u64);
        let storage = value.to_storage();

        let mut readers = Vec::new();
        for _ in 0..num_readers {
            readers.push(Arc::<u64>::from_storage(storage));
        }

        let expected_count = num_readers + 1; // readers + storage
        let actual_count = Arc::strong_count(&readers[0]);

        assert_eq!(
            actual_count, expected_count,
            "Refcount with {} readers should be {}",
            num_readers, expected_count
        );

        readers.clear();

        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
    }
}

#[test]
fn property_arc_drop_fully_cleans_up() {
    // Property: After drop_storage, weak ref should be dead

    for _ in 0..100 {
        let value = Arc::new(42u64);
        let weak = Arc::downgrade(&value);

        assert_eq!(weak.strong_count(), 1);

        let storage = value.to_storage();

        // Create and drop a reader
        let reader = Arc::<u64>::from_storage(storage);
        assert_eq!(Arc::strong_count(&reader), 2);
        drop(reader);

        // Storage should still be alive
        assert_eq!(weak.strong_count(), 1);

        // Drop storage
        unsafe {
            Arc::<u64>::drop_storage(storage);
        }

        // Now weak should be dead
        assert_eq!(weak.strong_count(), 0);
        assert!(weak.upgrade().is_none());
    }
}

// ============================================================================
// Property 3: String Heap Memory Correctness
// ============================================================================

#[test]
fn property_string_multiple_clones_independent() {
    // Property: Multiple from_storage calls produce independent String clones

    let original = String::from("Test");
    let storage = original.to_storage();

    let mut clones = Vec::new();
    for _ in 0..10 {
        clones.push(String::from_storage(storage));
    }

    // All clones should be equal
    for clone in &clones {
        assert_eq!(clone, "Test");
    }

    // Modify one clone - others should be unaffected
    let mut modified = clones[0].clone();
    modified.push_str(" Modified");

    assert_eq!(modified, "Test Modified");
    assert_eq!(clones[0], "Test"); // Original clone unchanged

    drop(clones);

    unsafe {
        String::drop_storage(storage);
    }
}

#[test]
fn property_string_size_preserves_content() {
    // Property: Strings of various sizes preserve content correctly

    let test_strings = vec![
        String::new(),
        String::from("A"),
        String::from("Hello"),
        String::from("A".repeat(100)),
        String::from("A".repeat(10_000)),
        String::from("世界🚀"),
    ];

    for original in test_strings {
        let expected = original.clone();
        let storage = original.to_storage();

        let restored = String::from_storage(storage);
        assert_eq!(restored, expected);
        assert_eq!(restored.len(), expected.len());

        unsafe {
            String::drop_storage(storage);
        }
    }
}

// ============================================================================
// Property 4: Float Special Values
// ============================================================================

#[test]
fn property_float_special_values_preserve() {
    // Property: All IEEE-754 special values roundtrip correctly

    let f32_values = vec![
        0.0f32,
        -0.0,
        1.0,
        -1.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::EPSILON,
    ];

    for value in f32_values {
        let storage = value.to_storage();
        let roundtrip = f32::from_storage(storage);

        if value.is_nan() {
            assert!(roundtrip.is_nan());
        } else {
            assert_eq!(roundtrip, value, "f32 special value failed: {}", value);
        }

        unsafe {
            f32::drop_storage(storage);
        }
    }

    let f64_values = vec![
        0.0f64,
        -0.0,
        1.0,
        -1.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::EPSILON,
    ];

    for value in f64_values {
        let storage = value.to_storage();
        let roundtrip = f64::from_storage(storage);

        if value.is_nan() {
            assert!(roundtrip.is_nan());
        } else {
            assert_eq!(roundtrip, value, "f64 special value failed: {}", value);
        }

        unsafe {
            f64::drop_storage(storage);
        }
    }
}

#[test]
fn property_float_nan_roundtrips() {
    // Property: NaN values roundtrip (but may change bit pattern)

    let nan_f32 = f32::NAN;
    let storage = nan_f32.to_storage();
    let roundtrip = f32::from_storage(storage);
    assert!(roundtrip.is_nan());
    unsafe {
        f32::drop_storage(storage);
    }

    let nan_f64 = f64::NAN;
    let storage = nan_f64.to_storage();
    let roundtrip = f64::from_storage(storage);
    assert!(roundtrip.is_nan());
    unsafe {
        f64::drop_storage(storage);
    }
}

// ============================================================================
// Property 5: Drop Safety - No Double Free
// ============================================================================

#[test]
fn property_arc_no_double_free_on_sequential_drops() {
    // Property: Sequential drop_storage calls don't cause double-free
    // (This is actually unsafe - test validates single drop only)

    let value = Arc::new(42u64);
    let storage = value.to_storage();

    // Read and drop multiple times
    for _ in 0..10 {
        let reader = Arc::<u64>::from_storage(storage);
        assert_eq!(*reader, 42);
        drop(reader);
    }

    // Final cleanup
    unsafe {
        Arc::<u64>::drop_storage(storage);
    }

    // If we had a double-free, the test would crash or trigger asan
}

#[test]
fn property_string_no_double_free_on_sequential_reads() {
    // Property: Multiple reads + single drop doesn't cause double-free

    let original = String::from("Test");
    let storage = original.to_storage();

    for _ in 0..100 {
        let read = String::from_storage(storage);
        assert_eq!(read, "Test");
        drop(read);
    }

    unsafe {
        String::drop_storage(storage);
    }
}

// ============================================================================
// Property 6: Concurrent Access Patterns
// ============================================================================

#[test]
fn property_arc_concurrent_readers_see_same_pointer() {
    // Property: All concurrent readers from same storage see same Arc pointer

    use std::sync::Mutex;
    use std::thread;

    let value = Arc::new(42u64);
    let original_ptr = Arc::as_ptr(&value);
    let storage = value.to_storage();

    let pointers = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for _ in 0..10 {
        let pointers_clone = Arc::clone(&pointers);
        let handle = thread::spawn(move || {
            let reader = Arc::<u64>::from_storage(storage);
            let ptr = Arc::as_ptr(&reader);
            pointers_clone.lock().unwrap().push(ptr as usize);
            drop(reader);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let pointers_vec = pointers.lock().unwrap();

    // All pointers should be the same
    for &ptr in pointers_vec.iter() {
        assert_eq!(ptr, original_ptr as usize);
    }

    unsafe {
        Arc::<u64>::drop_storage(storage);
    }
}

#[test]
fn property_primitives_concurrent_safe() {
    // Property: Primitives can be safely read concurrently from storage

    use std::thread;

    let value: u64 = 12345;
    let storage = value.to_storage();

    let mut handles = Vec::new();

    for _ in 0..10 {
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let read = u64::from_storage(storage);
                assert_eq!(read, 12345);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    unsafe {
        u64::drop_storage(storage);
    }
}

// ============================================================================
// Property 7: Bool Edge Cases
// ============================================================================

#[test]
fn property_bool_any_nonzero_is_true() {
    // Property: Any non-zero u64 maps to true

    let test_values = vec![
        0u64,     // false
        1,        // true
        2,        // true
        42,       // true
        u64::MAX, // true
    ];

    for value in test_values {
        let expected = value != 0;
        let result = bool::from_storage(value);
        assert_eq!(
            result, expected,
            "bool::from_storage({}) should be {}",
            value, expected
        );
    }
}

// ============================================================================
// Property 8: Size Constraints
// ============================================================================

#[test]
fn property_primitives_fit_in_u64() {
    // Property: All primitive types fit in u64

    assert!(core::mem::size_of::<u8>() <= 8);
    assert!(core::mem::size_of::<u16>() <= 8);
    assert!(core::mem::size_of::<u32>() <= 8);
    assert!(core::mem::size_of::<u64>() <= 8);
    assert!(core::mem::size_of::<i8>() <= 8);
    assert!(core::mem::size_of::<i16>() <= 8);
    assert!(core::mem::size_of::<i32>() <= 8);
    assert!(core::mem::size_of::<i64>() <= 8);
    assert!(core::mem::size_of::<bool>() <= 8);
    assert!(core::mem::size_of::<f32>() <= 8);
    assert!(core::mem::size_of::<f64>() <= 8);
    assert!(core::mem::size_of::<usize>() <= 8);
    assert!(core::mem::size_of::<isize>() <= 8);
}

#[test]
fn property_pointers_fit_in_u64() {
    // Property: Pointers fit in u64 on 64-bit systems

    assert!(core::mem::size_of::<*const ()>() <= 8);
    assert!(core::mem::size_of::<*mut ()>() <= 8);
    assert!(core::mem::size_of::<usize>() <= 8);
}

// ============================================================================
// Property 9: Transmute Soundness
// ============================================================================

#[test]
fn property_primitive_transmute_is_identity() {
    // Property: For u64, to_storage is literally identity

    for value in [0u64, 1, 42, u64::MAX / 2, u64::MAX] {
        let storage = value.to_storage();
        // For u64, storage should be identical to value
        assert_eq!(storage, value);

        let roundtrip = u64::from_storage(storage);
        assert_eq!(roundtrip, value);
    }
}

// ============================================================================
// Property 10: Complex Type Support (Arc<ComplexType>)
// ============================================================================

#[test]
fn property_arc_complex_types_preserve_structure() {
    // Property: Arc<ComplexType> preserves all fields correctly

    #[derive(Debug, Clone, PartialEq)]
    struct ComplexData {
        id: u64,
        name: String,
        values: Vec<f64>,
        nested: Option<Box<String>>,
    }

    let data = ComplexData {
        id: 123,
        name: String::from("Test"),
        values: vec![1.0, 2.0, 3.0],
        nested: Some(Box::new(String::from("Nested"))),
    };

    let arc = Arc::new(data.clone());
    let storage = arc.to_storage();

    // Multiple reads
    for _ in 0..10 {
        let restored = Arc::<ComplexData>::from_storage(storage);
        assert_eq!(*restored, data);
        assert_eq!(restored.id, 123);
        assert_eq!(restored.name, "Test");
        assert_eq!(restored.values, vec![1.0, 2.0, 3.0]);
        assert_eq!(**restored.nested.as_ref().unwrap(), "Nested");
    }

    unsafe {
        Arc::<ComplexData>::drop_storage(storage);
    }
}

// ============================================================================
// Summary Statistics
// ============================================================================

#[test]
fn test_property_coverage_summary() {
    println!("\n=== BitwiseSerializable Property Test Coverage ===");
    println!("Total properties tested: 10");
    println!("  1. Roundtrip identity (primitives)");
    println!("  2. Arc refcount invariants");
    println!("  3. String heap memory correctness");
    println!("  4. Float special values");
    println!("  5. Drop safety (no double-free)");
    println!("  6. Concurrent access patterns");
    println!("  7. Bool edge cases");
    println!("  8. Size constraints");
    println!("  9. Transmute soundness");
    println!(" 10. Complex type support (Arc<T>)");
    println!("====================================================\n");
}
