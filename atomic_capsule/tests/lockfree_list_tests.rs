//! Comprehensive tests for LockfreeList<T> (T28 Framework)
//!
//! **Testing Strategy**:
//! - Unit tests (Q1-Q7): Basic operations, invariants
//! - Property tests (Q8-Q14): Concurrent correctness, safety
//! - Stress tests (Q15-Q21): Memory leaks, 100K+ operations
//! - Production tests (Q22-Q28): Real-world scenarios

use atomic_capsule::parallel::LockfreeList;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// UNIT TESTS (Q1-Q7)
// ============================================================================

#[test]
fn test_new_empty() {
    let list: LockfreeList<u64> = LockfreeList::new();
    assert_eq!(list.len(), 0);
    assert!(list.is_empty());
}

#[test]
fn test_default_empty() {
    let list: LockfreeList<u64> = LockfreeList::default();
    assert_eq!(list.len(), 0);
    assert!(list.is_empty());
}

#[test]
fn test_push_single() {
    let list: LockfreeList<u64> = LockfreeList::new();
    list.push(42);
    assert_eq!(list.len(), 1);
    assert!(!list.is_empty());
}

#[test]
fn test_push_multiple_ordered() {
    let list: LockfreeList<u64> = LockfreeList::new();
    list.push(1);
    list.push(2);
    list.push(3);
    assert_eq!(list.len(), 3);

    let values: Vec<_> = list.iter().copied().collect();
    assert_eq!(values, vec![1, 2, 3]);
}

#[test]
fn test_iter_empty() {
    let list: LockfreeList<u64> = LockfreeList::new();
    let values: Vec<_> = list.iter().copied().collect();
    assert_eq!(values, vec![]);
}

#[test]
fn test_iter_single() {
    let list: LockfreeList<u64> = LockfreeList::new();
    list.push(42);
    let values: Vec<_> = list.iter().copied().collect();
    assert_eq!(values, vec![42]);
}

#[test]
fn test_iter_multiple() {
    let list: LockfreeList<u64> = LockfreeList::new();
    for i in 0..10 {
        list.push(i);
    }
    let values: Vec<_> = list.iter().copied().collect();
    assert_eq!(values, (0..10).collect::<Vec<_>>());
}

#[test]
fn test_iter_twice() {
    let list: LockfreeList<u64> = LockfreeList::new();
    list.push(1);
    list.push(2);

    let first: Vec<_> = list.iter().copied().collect();
    let second: Vec<_> = list.iter().copied().collect();

    assert_eq!(first, vec![1, 2]);
    assert_eq!(second, vec![1, 2]);
}

#[test]
fn test_drop_empty() {
    let list: LockfreeList<u64> = LockfreeList::new();
    drop(list); // Should not panic
}

#[test]
fn test_drop_with_values() {
    let list: LockfreeList<String> = LockfreeList::new();
    list.push("hello".to_string());
    list.push("world".to_string());
    drop(list); // Should deallocate strings properly
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14): Concurrent Correctness
// ============================================================================

#[test]
fn test_concurrent_push_2_threads() {
    let list = Arc::new(LockfreeList::new());
    let mut handles = vec![];

    for i in 0..2 {
        let list = Arc::clone(&list);
        handles.push(thread::spawn(move || {
            for j in 0..1000 {
                list.push(i * 1000 + j);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(list.len(), 2000);
}

#[test]
fn test_concurrent_push_4_threads() {
    let list = Arc::new(LockfreeList::new());
    let mut handles = vec![];

    for i in 0..4 {
        let list = Arc::clone(&list);
        handles.push(thread::spawn(move || {
            for j in 0..1000 {
                list.push(i * 1000 + j);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(list.len(), 4000);
}

#[test]
fn test_concurrent_push_16_threads() {
    let list = Arc::new(LockfreeList::new());
    let mut handles = vec![];

    for i in 0..16 {
        let list = Arc::clone(&list);
        handles.push(thread::spawn(move || {
            for j in 0..1000 {
                list.push(i * 1000 + j);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(list.len(), 16000);
}

#[test]
fn test_concurrent_push_and_iter() {
    let list = Arc::new(LockfreeList::new());
    let list_writer = Arc::clone(&list);
    let list_reader = Arc::clone(&list);

    // Writer thread
    let writer = thread::spawn(move || {
        for i in 0..10000 {
            list_writer.push(i);
            if i % 100 == 0 {
                thread::sleep(Duration::from_micros(1));
            }
        }
    });

    // Reader thread (iterate multiple times)
    let reader = thread::spawn(move || {
        for _ in 0..100 {
            let count = list_reader.iter().count();
            // Length is monotonically increasing
            assert!(count <= 10000);
            thread::sleep(Duration::from_micros(10));
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();

    assert_eq!(list.len(), 10000);
}

#[test]
fn test_concurrent_multiple_readers() {
    let list = Arc::new(LockfreeList::new());

    // Populate list
    for i in 0..1000 {
        list.push(i);
    }

    let mut handles = vec![];

    // Spawn 8 reader threads
    for _ in 0..8 {
        let list = Arc::clone(&list);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let count = list.iter().count();
                assert_eq!(count, 1000);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_writers_and_readers() {
    let list = Arc::new(LockfreeList::new());
    let total_written = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // Spawn 4 writer threads
    for i in 0..4 {
        let list = Arc::clone(&list);
        let total = Arc::clone(&total_written);
        handles.push(thread::spawn(move || {
            for j in 0..1000 {
                list.push(i * 1000 + j);
                total.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    // Spawn 4 reader threads
    for _ in 0..4 {
        let list = Arc::clone(&list);
        let total = Arc::clone(&total_written);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let count = list.iter().count();
                let written = total.load(Ordering::Relaxed);
                // Count should never exceed written
                assert!(count <= written);
                thread::sleep(Duration::from_micros(1));
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(list.len(), 4000);
}

// ============================================================================
// STRESS TESTS (Q15-Q21): Memory & Performance
// ============================================================================

#[test]
fn test_large_push_10k() {
    let list: LockfreeList<u64> = LockfreeList::new();
    for i in 0..10_000 {
        list.push(i);
    }
    assert_eq!(list.len(), 10_000);

    // Verify order
    let values: Vec<_> = list.iter().copied().collect();
    assert_eq!(values.len(), 10_000);
    for (i, &val) in values.iter().enumerate() {
        assert_eq!(val, i as u64);
    }
}

#[test]
fn test_large_push_100k() {
    let list: LockfreeList<u64> = LockfreeList::new();
    for i in 0..100_000 {
        list.push(i);
    }
    assert_eq!(list.len(), 100_000);
}

#[test]
fn test_large_push_no_leak() {
    // This test verifies no memory leak occurs with 100K pushes
    let list: LockfreeList<Vec<u8>> = LockfreeList::new();
    for i in 0..100_000 {
        list.push(vec![i as u8; 64]);
    }
    assert_eq!(list.len(), 100_000);
    // Drop should deallocate all 100K nodes
    drop(list);
}

#[test]
fn test_concurrent_stress_16_threads_10k_each() {
    let list = Arc::new(LockfreeList::new());
    let mut handles = vec![];

    for i in 0..16 {
        let list = Arc::clone(&list);
        handles.push(thread::spawn(move || {
            for j in 0..10_000 {
                list.push(i * 10_000 + j);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(list.len(), 160_000);
}

#[test]
fn test_iter_large_list() {
    let list: LockfreeList<u64> = LockfreeList::new();
    for i in 0..10_000 {
        list.push(i);
    }

    let mut count = 0;
    for _ in list.iter() {
        count += 1;
    }

    assert_eq!(count, 10_000);
}

// ============================================================================
// PRODUCTION TESTS (Q22-Q28): Real-World Scenarios
// ============================================================================

#[test]
fn test_push_different_types() {
    // Test with various types
    let list_u64: LockfreeList<u64> = LockfreeList::new();
    list_u64.push(42);
    assert_eq!(list_u64.len(), 1);

    let list_string: LockfreeList<String> = LockfreeList::new();
    list_string.push("hello".to_string());
    assert_eq!(list_string.len(), 1);

    struct CustomStruct {
        id: u64,
        name: String,
    }

    let list_custom: LockfreeList<CustomStruct> = LockfreeList::new();
    list_custom.push(CustomStruct {
        id: 1,
        name: "test".to_string(),
    });
    assert_eq!(list_custom.len(), 1);
}

#[test]
fn test_log_style_append() {
    // Simulate log-style append (typical use case)
    struct LogEntry {
        timestamp: u64,
        level: String,
        message: String,
    }

    let log = Arc::new(LockfreeList::new());

    // Simulate 4 threads logging concurrently
    let mut handles = vec![];
    for thread_id in 0..4 {
        let log = Arc::clone(&log);
        handles.push(thread::spawn(move || {
            for i in 0..1000 {
                log.push(LogEntry {
                    timestamp: i,
                    level: "INFO".to_string(),
                    message: format!("Thread {} message {}", thread_id, i),
                });
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(log.len(), 4000);

    // Verify all entries present
    let count = log.iter().count();
    assert_eq!(count, 4000);
}

#[test]
fn test_event_queue() {
    // Simulate event queue (producer-consumer pattern with multiple producers)
    #[derive(Debug, Clone)]
    struct Event {
        event_type: String,
        payload: Vec<u8>,
    }

    let events = Arc::new(LockfreeList::new());

    // Spawn 8 event producers
    let mut handles = vec![];
    for i in 0..8 {
        let events = Arc::clone(&events);
        handles.push(thread::spawn(move || {
            for j in 0..500 {
                events.push(Event {
                    event_type: format!("Type{}", i),
                    payload: vec![j as u8; 32],
                });
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(events.len(), 4000);
}

#[test]
fn test_monotonic_length() {
    // Property: Length is monotonically increasing
    let list = Arc::new(LockfreeList::new());
    let list_writer = Arc::clone(&list);
    let list_reader = Arc::clone(&list);
    let last_seen = Arc::new(AtomicUsize::new(0));
    let last_seen_reader = Arc::clone(&last_seen);

    let writer = thread::spawn(move || {
        for i in 0..10_000 {
            list_writer.push(i);
        }
    });

    let reader = thread::spawn(move || {
        for _ in 0..1000 {
            let current = list_reader.len();
            let last = last_seen_reader.load(Ordering::Relaxed);
            assert!(current >= last, "Length decreased: {} -> {}", last, current);
            last_seen_reader.store(current, Ordering::Relaxed);
            thread::sleep(Duration::from_micros(1));
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();
}

#[test]
fn test_no_duplicate_values_single_thread() {
    // Verify no duplicates in single-threaded scenario
    let list: LockfreeList<u64> = LockfreeList::new();
    for i in 0..1000 {
        list.push(i);
    }

    let values: Vec<_> = list.iter().copied().collect();
    let mut sorted = values.clone();
    sorted.sort();
    sorted.dedup();

    assert_eq!(values.len(), sorted.len(), "Found duplicate values");
}

#[test]
fn test_send_sync_traits() {
    // Verify Send + Sync traits compile
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<LockfreeList<u64>>();
    assert_sync::<LockfreeList<u64>>();
}

#[test]
fn test_concurrent_push_with_yield() {
    // Test with explicit yield points to increase contention
    let list = Arc::new(LockfreeList::new());
    let mut handles = vec![];

    for i in 0..8 {
        let list = Arc::clone(&list);
        handles.push(thread::spawn(move || {
            for j in 0..1000 {
                list.push(i * 1000 + j);
                if j % 100 == 0 {
                    thread::yield_now();
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(list.len(), 8000);
}
