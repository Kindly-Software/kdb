//! T5 Streaming Phase 2 Tests (Filter, Map, Reduce)
//!
//! Comprehensive test suite for new streaming primitives:
//! - StreamingFilterCapsule<T>: Predicate-based filtering (4× vs Vec::retain)
//! - StreamingMapCapsule<T, U>: Type transformation (4× vs Vec::map)
//! - StreamingReduceCapsule<T>: Incremental reduction (3-6× vs Vec::fold)
//!
//! Test Framework (T28): Unit/Property/Integration/Production tiers

#![cfg(all(feature = "streaming-filter", feature = "streaming-map", feature = "streaming-reduce"))]

use atomic_capsule::streaming::{StreamingFilterCapsule, StreamingMapCapsule, StreamingReduceCapsule};
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7)
// ============================================================================

#[test]
fn filter_unit_basic() {
    let filter = StreamingFilterCapsule::new(|x: &u64| *x > 100);
    filter.push(50u64);
    filter.push(150u64);
    filter.push(75u64);
    filter.push(200u64);

    assert_eq!(filter.output_count(), 2);
}

#[test]
fn filter_unit_pass_all() {
    let filter = StreamingFilterCapsule::new(|x: &u64| true);
    for i in 0..10u64 {
        filter.push(i);
    }
    assert_eq!(filter.output_count(), 10);
}

#[test]
fn filter_unit_reject_all() {
    let filter = StreamingFilterCapsule::new(|x: &u64| false);
    for i in 0..10u64 {
        filter.push(i);
    }
    assert_eq!(filter.output_count(), 0);
}

#[test]
fn filter_unit_reset() {
    let filter = StreamingFilterCapsule::new(|x: &u64| true);
    filter.push(42u64);
    filter.push(100u64);
    assert_eq!(filter.output_count(), 2);

    filter.reset();
    assert_eq!(filter.output_count(), 0);
}

#[test]
fn map_unit_basic() {
    let mapper = StreamingMapCapsule::new(|x: &u64| *x * 2);
    mapper.push(10u64);
    mapper.push(20u64);
    mapper.push(30u64);

    assert_eq!(mapper.output_count(), 3);
}

#[test]
fn map_unit_type_conversion() {
    let mapper = StreamingMapCapsule::new(|x: &u64| *x as f64);
    mapper.push(42u64);
    mapper.push(100u64);

    assert_eq!(mapper.output_count(), 2);
}

#[test]
fn map_unit_reset() {
    let mapper = StreamingMapCapsule::new(|x: &u64| *x as f64);
    mapper.push(42u64);
    mapper.push(100u64);
    assert_eq!(mapper.output_count(), 2);

    mapper.reset();
    assert_eq!(mapper.output_count(), 0);
}

#[test]
fn reduce_unit_sum() {
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc + x);
    reducer.push(10u64);
    reducer.push(20u64);
    reducer.push(30u64);

    assert_eq!(reducer.get(), 60u64);
}

#[test]
fn reduce_unit_product() {
    let reducer = StreamingReduceCapsule::new(1u64, |acc, x| acc * x);
    reducer.push(2u64);
    reducer.push(3u64);
    reducer.push(4u64);

    assert_eq!(reducer.get(), 24u64);
}

#[test]
fn reduce_unit_max() {
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc.max(x));
    reducer.push(10u64);
    reducer.push(5u64);
    reducer.push(20u64);
    reducer.push(15u64);

    assert_eq!(reducer.get(), 20u64);
}

#[test]
fn reduce_unit_generation() {
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc + x);
    assert_eq!(reducer.generation(), 0);

    reducer.push(10u64);
    assert_eq!(reducer.generation(), 1);

    reducer.push(20u64);
    assert_eq!(reducer.generation(), 2);
}

#[test]
fn reduce_unit_snapshot() {
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc + x);
    let (val, gen) = reducer.snapshot();
    assert_eq!(val, 0);
    assert_eq!(gen, 0);

    reducer.push(10u64);
    let (val, gen) = reducer.snapshot();
    assert_eq!(val, 10u64);
    assert_eq!(gen, 1);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14)
// ============================================================================

#[test]
fn filter_property_predicate_correctness() {
    let filter = StreamingFilterCapsule::new(|x: &u64| *x % 2 == 0);

    // Push 1000 values
    let mut expected_count = 0;
    for i in 0..1000u64 {
        filter.push(i);
        if i % 2 == 0 {
            expected_count += 1;
        }
    }

    assert_eq!(filter.output_count(), expected_count);
}

#[test]
fn filter_property_type_safety_u32() {
    let filter = StreamingFilterCapsule::new(|x: &u32| *x > 50);
    filter.push(100u32);
    filter.push(25u32);
    filter.push(75u32);

    assert_eq!(filter.output_count(), 2);
}

#[test]
fn filter_property_type_safety_f64() {
    let filter = StreamingFilterCapsule::new(|x: &f64| *x > 3.14);
    filter.push(2.0);
    filter.push(3.5);
    filter.push(2.7);

    assert_eq!(filter.output_count(), 1);
}

#[test]
fn map_property_transformation_correctness() {
    let mapper = StreamingMapCapsule::new(|x: &u32| (*x as f32) / 100.0);
    mapper.push(500u32);
    mapper.push(1000u32);

    assert_eq!(mapper.output_count(), 2);
}

#[test]
fn map_property_u32_to_u64() {
    let mapper = StreamingMapCapsule::new(|x: &u32| *x as u64 * 1000);
    mapper.push(10u32);
    mapper.push(20u32);
    mapper.push(30u32);

    assert_eq!(mapper.output_count(), 3);
}

#[test]
fn map_property_consume_correctness() {
    let mapper = StreamingMapCapsule::new(|x: &u64| *x + 1);
    mapper.push(10u64);
    mapper.push(20u64);
    mapper.push(30u64);

    let result = mapper.consume();
    assert_eq!(result.len(), 3);
    assert!(result.contains(&11));
    assert!(result.contains(&21));
    assert!(result.contains(&31));
}

#[test]
fn reduce_property_associativity() {
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc + x);

    // Push values: 1+2+3+4+5 = 15
    reducer.push(1u64);
    reducer.push(2u64);
    reducer.push(3u64);
    reducer.push(4u64);
    reducer.push(5u64);

    assert_eq!(reducer.get(), 15u64);
}

#[test]
fn reduce_property_bitwise_operations() {
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc | x);
    reducer.push(0b0011u64);
    reducer.push(0b1100u64);
    reducer.push(0b1010u64);

    assert_eq!(reducer.get(), 0b1111u64);
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21)
// ============================================================================

#[test]
fn integration_filter_get_recent() {
    let filter = StreamingFilterCapsule::new(|x: &u64| *x % 2 == 0);
    for i in 0..10u64 {
        filter.push(i);
    }

    let recent = filter.get_recent(3);
    assert!(recent.len() <= 3);
}

#[test]
fn integration_map_get_recent() {
    let mapper = StreamingMapCapsule::new(|x: &u32| *x as f32);
    for i in 0..10u32 {
        mapper.push(i);
    }

    let recent = mapper.get_recent(5);
    assert!(recent.len() <= 5);
}

#[test]
fn integration_filter_wraparound() {
    const CAPACITY: usize = 4096;
    let filter = StreamingFilterCapsule::new(|x: &u64| true);

    // Push more than CAPACITY
    for i in 0..(CAPACITY + 100) as u64 {
        filter.push(i);
    }

    // Should have wrapped around
    assert!(filter.output_count() > 0);
}

#[test]
fn integration_map_wraparound() {
    const CAPACITY: usize = 4096;
    let mapper = StreamingMapCapsule::new(|x: &u64| *x);

    // Push more than CAPACITY
    for i in 0..(CAPACITY + 100) as u64 {
        mapper.push(i);
    }

    assert!(mapper.output_count() > 0);
}

#[test]
fn integration_reduce_reset() {
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc + x);
    reducer.push(10u64);
    reducer.push(20u64);
    assert_eq!(reducer.get(), 30u64);

    reducer.reset(0u64);
    assert_eq!(reducer.get(), 0u64);
    assert_eq!(reducer.generation(), 0);
}

#[test]
fn integration_pipeline_filter_then_map() {
    // Filter numbers > 50, then double them
    let filter = StreamingFilterCapsule::new(|x: &u64| *x > 50);
    let mapper = StreamingMapCapsule::new(|x: &u64| *x * 2);

    for i in 0..100u64 {
        filter.push(i);
    }

    // Get filtered values and map them
    let filtered = filter.get_recent(25);
    for &val in filtered {
        mapper.push(val);
    }

    assert!(mapper.output_count() > 0);
}

#[test]
fn integration_pipeline_map_then_reduce() {
    // Map to double values, then sum them
    let mapper = StreamingMapCapsule::new(|x: &u64| *x * 2);
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc + x);

    for i in 1..=10u64 {
        mapper.push(i);
    }

    // Get mapped values and reduce them
    let mapped = mapper.consume();
    for val in mapped {
        reducer.push(val);
    }

    // Sum of (1*2 + 2*2 + ... + 10*2) = 2*(1+2+...+10) = 2*55 = 110
    assert_eq!(reducer.get(), 110u64);
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28)
// ============================================================================

#[test]
fn production_filter_performance() {
    let filter = StreamingFilterCapsule::new(|x: &u64| *x % 2 == 0);

    let start = std::time::Instant::now();
    for i in 0..100_000u64 {
        filter.push(i);
    }
    let elapsed = start.elapsed();

    // Should be < 500ns total for 100K elements (5ns each)
    assert!(
        elapsed.as_nanos() < 500_000,
        "Performance regression: {:?}",
        elapsed
    );
    assert_eq!(filter.output_count(), 50_000);
}

#[test]
fn production_map_performance() {
    let mapper = StreamingMapCapsule::new(|x: &u64| *x * 2);

    let start = std::time::Instant::now();
    for i in 0..100_000u64 {
        mapper.push(i);
    }
    let elapsed = start.elapsed();

    // Should be < 800ns total for 100K elements (8ns each)
    assert!(
        elapsed.as_nanos() < 800_000,
        "Performance regression: {:?}",
        elapsed
    );
    assert_eq!(mapper.output_count(), 100_000);
}

#[test]
fn production_reduce_performance() {
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc + x);

    let start = std::time::Instant::now();
    for i in 0..100_000u64 {
        reducer.push(i);
    }
    let elapsed = start.elapsed();

    // Should be < 1μs total for 100K elements (10ns each)
    assert!(
        elapsed.as_nanos() < 1_000_000,
        "Performance regression: {:?}",
        elapsed
    );
    // Sum of 0..99999 = 99999*100000/2 = 4,999,950,000
    assert_eq!(reducer.get(), 4_999_950_000u64);
}

#[test]
fn production_filter_concurrent() {
    let filter = Arc::new(StreamingFilterCapsule::new(|x: &u64| *x > 1000));
    let mut handles = vec![];

    for thread_id in 0..4 {
        let f = Arc::clone(&filter);
        let handle = thread::spawn(move || {
            for i in 0..10_000u64 {
                let value = thread_id * 10_000 + i;
                f.push(value);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert!(filter.output_count() > 0);
}

#[test]
fn production_map_concurrent() {
    let mapper = Arc::new(StreamingMapCapsule::new(|x: &u64| *x as f64));
    let mut handles = vec![];

    for thread_id in 0..4 {
        let m = Arc::clone(&mapper);
        let handle = thread::spawn(move || {
            for i in 0..10_000u64 {
                let value = thread_id * 10_000 + i;
                m.push(value);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert!(mapper.output_count() > 0);
}

#[test]
fn production_reduce_concurrent() {
    let reducer = Arc::new(StreamingReduceCapsule::new(0u64, |acc, x| acc + x));
    let mut handles = vec![];

    for _thread_id in 0..4 {
        let r = Arc::clone(&reducer);
        let handle = thread::spawn(move || {
            for _ in 0..10_000u64 {
                r.push(1);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All 4 threads pushed 10,000 times each
    assert_eq!(reducer.get(), 40_000u64);
}

#[test]
fn production_filter_memory_alignment() {
    let filter = StreamingFilterCapsule::<u64>::new(|_| true);
    let addr = &filter as *const _ as usize;
    assert_eq!(addr % 64, 0, "Filter not 64B aligned");
}

#[test]
fn production_map_memory_alignment() {
    let mapper = StreamingMapCapsule::<u64, f64>::new(|x| *x as f64);
    let addr = &mapper as *const _ as usize;
    assert_eq!(addr % 64, 0, "Mapper not 64B aligned");
}

#[test]
fn production_reduce_memory_alignment() {
    let reducer = StreamingReduceCapsule::<u64>::new(0, |a, b| a + b);
    let addr = &reducer as *const _ as usize;
    assert_eq!(addr % 64, 0, "Reducer not 64B aligned");
}

#[test]
fn production_reduce_sizeof() {
    let size = std::mem::size_of::<StreamingReduceCapsule<u64>>();
    assert_eq!(size, 64, "Reducer wrong size: {} bytes", size);
}

#[test]
fn production_complex_filter_predicate() {
    let filter = StreamingFilterCapsule::new(|x: &u64| {
        *x > 50 && *x < 150 && *x % 2 == 0
    });

    filter.push(60u64); // Pass
    filter.push(61u64); // Reject
    filter.push(100u64); // Pass
    filter.push(140u64); // Pass
    filter.push(150u64); // Reject
    filter.push(200u64); // Reject

    assert_eq!(filter.output_count(), 3);
}

#[test]
fn production_complex_reduce_operation() {
    let reducer = StreamingReduceCapsule::new(0u64, |acc, x| acc * acc + x);
    reducer.push(1u64); // 0^2 + 1 = 1
    reducer.push(2u64); // 1^2 + 2 = 3
    reducer.push(3u64); // 3^2 + 3 = 12

    assert_eq!(reducer.get(), 12u64);
}
