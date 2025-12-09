//! Integration tests for UnboundedQueueCapsule

use atomic_capsule::collections::queue::{UnboundedQueueCapsule, SPSC, MPMC};

#[test]
fn test_spsc_unbounded_basic() {
    let queue = UnboundedQueueCapsule::<u64, SPSC>::new();

    // Basic push/pop
    queue.push(42).unwrap();
    assert_eq!(queue.pop(), Some(42));
    assert_eq!(queue.pop(), None);
}

#[test]
fn test_spsc_unbounded_growth() {
    let queue = UnboundedQueueCapsule::<u64, SPSC>::new();

    // Push 1000 elements (triggers segment growth)
    for i in 0..1000 {
        queue.push(i).unwrap();
    }

    assert_eq!(queue.len(), 1000);

    // Pop all
    for i in 0..1000 {
        assert_eq!(queue.pop(), Some(i));
    }

    assert_eq!(queue.len(), 0);
}

#[test]
fn test_mpmc_unbounded_basic() {
    let queue = UnboundedQueueCapsule::<u64, MPMC>::new();

    // Basic push/pop
    queue.push(42).unwrap();
    assert_eq!(queue.pop(), Some(42));
    assert_eq!(queue.pop(), None);
}

#[test]
fn test_mpmc_unbounded_concurrent() {
    use std::sync::Arc;
    use std::thread;

    let queue = Arc::new(UnboundedQueueCapsule::<u64, MPMC>::new());

    // Spawn 4 producers
    let producers: Vec<_> = (0..4)
        .map(|id| {
            let q = queue.clone();
            thread::spawn(move || {
                for i in 0..100 {
                    q.push(id * 1000 + i).unwrap();
                }
            })
        })
        .collect();

    // Wait for producers
    for handle in producers {
        handle.join().unwrap();
    }

    // Should have 400 elements
    assert_eq!(queue.len(), 400);

    // Spawn 4 consumers
    let consumers: Vec<_> = (0..4)
        .map(|_| {
            let q = queue.clone();
            thread::spawn(move || {
                let mut count = 0;
                for _ in 0..100 {
                    if q.pop().is_some() {
                        count += 1;
                    }
                }
                count
            })
        })
        .collect();

    // Wait for consumers and sum
    let total: usize = consumers
        .into_iter()
        .map(|h| h.join().unwrap())
        .sum();

    // Should have consumed all 400 elements
    assert_eq!(total, 400);
    assert_eq!(queue.len(), 0);
}
