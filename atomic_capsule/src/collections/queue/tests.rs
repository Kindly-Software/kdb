//! Queue tests - T28 comprehensive testing framework

use super::*;
use std::sync::Arc;
use std::thread;

#[test]
fn test_capacity_validation() {
    // Valid capacities
    assert!(QueueCapsule::<u64, SPSC>::new(1).is_ok());
    assert!(QueueCapsule::<u64, SPSC>::new(2).is_ok());
    assert!(QueueCapsule::<u64, SPSC>::new(1024).is_ok());

    // Invalid capacities (not power of 2)
    assert_eq!(
        QueueCapsule::<u64, SPSC>::new(0).unwrap_err(),
        QueueError::InvalidCapacity
    );
    assert_eq!(
        QueueCapsule::<u64, SPSC>::new(3).unwrap_err(),
        QueueError::InvalidCapacity
    );
    assert_eq!(
        QueueCapsule::<u64, SPSC>::new(1000).unwrap_err(),
        QueueError::InvalidCapacity
    );
}

#[test]
fn test_spsc_sequential() {
    let queue = QueueCapsule::<u64, SPSC>::new(16).unwrap();

    // Push and pop sequentially
    for i in 0..10 {
        assert_eq!(queue.push(i), Ok(()));
    }

    for i in 0..10 {
        assert_eq!(queue.pop(), Some(i));
    }

    assert_eq!(queue.pop(), None);
}

#[test]
fn test_mpmc_sequential() {
    let queue = QueueCapsule::<u64, MPMC>::new(16).unwrap();

    // Push and pop sequentially
    for i in 0..10 {
        assert_eq!(queue.push(i), Ok(()));
    }

    for i in 0..10 {
        assert_eq!(queue.pop(), Some(i));
    }

    assert_eq!(queue.pop(), None);
}

#[test]
fn test_spsc_wraparound() {
    let queue = QueueCapsule::<u64, SPSC>::new(4).unwrap();

    // Fill and empty multiple times
    for round in 0..100 {
        for i in 0..3 {
            assert_eq!(queue.push(round * 10 + i), Ok(()));
        }
        for i in 0..3 {
            assert_eq!(queue.pop(), Some(round * 10 + i));
        }
    }
}

#[test]
fn test_mpmc_concurrent_producers() {
    let queue = Arc::new(QueueCapsule::<u64, MPMC>::new(1024).unwrap());
    let num_threads = 4;
    let items_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let q = queue.clone();
            thread::spawn(move || {
                for i in 0..items_per_thread {
                    while q.push(t * 1000 + i).is_err() {
                        thread::yield_now();
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify all items pushed
    let mut count = 0;
    while queue.pop().is_some() {
        count += 1;
    }
    assert_eq!(count, num_threads * items_per_thread);
}

#[test]
fn test_mpmc_concurrent_consumers() {
    let queue = Arc::new(QueueCapsule::<u64, MPMC>::new(1024).unwrap());
    let total_items: usize = 400;

    // Push all items
    for i in 0..total_items {
        queue.push(i as u64).unwrap();
    }

    let num_threads = 4;
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let q = queue.clone();
            thread::spawn(move || {
                let mut count = 0;
                while q.pop().is_some() {
                    count += 1;
                }
                count
            })
        })
        .collect();

    let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(total, total_items);
}

#[test]
fn test_mpmc_concurrent_mixed() {
    let queue = Arc::new(QueueCapsule::<u64, MPMC>::new(1024).unwrap());
    let num_producers = 2;
    let num_consumers = 2;
    let items_per_producer = 500;

    // Spawn producers
    let producer_handles: Vec<_> = (0..num_producers)
        .map(|t| {
            let q = queue.clone();
            thread::spawn(move || {
                for i in 0..items_per_producer {
                    while q.push(t * 10000 + i).is_err() {
                        thread::yield_now();
                    }
                }
            })
        })
        .collect();

    // Spawn consumers
    let consumer_handles: Vec<_> = (0..num_consumers)
        .map(|_| {
            let q = queue.clone();
            thread::spawn(move || {
                let mut count = 0;
                for _ in 0..(num_producers * items_per_producer / num_consumers) {
                    while q.pop().is_none() {
                        thread::yield_now();
                    }
                    count += 1;
                }
                count
            })
        })
        .collect();

    for h in producer_handles {
        h.join().unwrap();
    }

    let total: usize = consumer_handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(total, (num_producers * items_per_producer) as usize);
}

#[test]
fn test_len_and_capacity() {
    let queue = QueueCapsule::<u64, SPSC>::new(8).unwrap();
    assert_eq!(queue.capacity(), 8);
    assert_eq!(queue.len(), 0);
    assert!(queue.is_empty());

    queue.push(1).unwrap();
    queue.push(2).unwrap();
    assert_eq!(queue.len(), 2);
    assert!(!queue.is_empty());

    queue.pop();
    assert_eq!(queue.len(), 1);

    queue.pop();
    assert_eq!(queue.len(), 0);
    assert!(queue.is_empty());
}

#[test]
fn test_drop_initialized_elements() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug)]
    struct DropCounter;
    impl Drop for DropCounter {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    {
        let queue = QueueCapsule::<DropCounter, SPSC>::new(8).unwrap();
        queue.push(DropCounter).unwrap();
        queue.push(DropCounter).unwrap();
        queue.push(DropCounter).unwrap();
        // Drop queue with 3 elements
    }

    assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 3);
}

#[test]
fn test_timeout_stress() {
    use std::time::{Duration, Instant};

    let queue = Arc::new(QueueCapsule::<u64, MPMC>::new(256).unwrap());
    let start = Instant::now();
    let timeout = Duration::from_secs(10);

    let producer = {
        let q = queue.clone();
        thread::spawn(move || {
            let mut i = 0;
            while start.elapsed() < timeout {
                while q.push(i).is_err() {
                    thread::yield_now();
                    if start.elapsed() >= timeout {
                        return;
                    }
                }
                i += 1;
            }
        })
    };

    let consumer = {
        let q = queue.clone();
        thread::spawn(move || {
            let mut count = 0;
            while start.elapsed() < timeout {
                if q.pop().is_some() {
                    count += 1;
                }
            }
            count
        })
    };

    producer.join().unwrap();
    let consumed = consumer.join().unwrap();

    // Verify no crashes and positive throughput
    assert!(consumed > 0, "Should have consumed some items");
}
