use atomic_capsule::collections::queue::{QueueCapsule, MPMC};
use std::time::Instant;

#[test]
fn test_mpmc_wraparound_complete() {
    let start = Instant::now();

    let queue = QueueCapsule::<u32, MPMC>::new(4).unwrap();

    // 8 cycles = 2 full rotations through capacity=4 queue
    for i in 0..8 {
        queue.push(i).unwrap();
        assert_eq!(queue.pop(), Some(i));
    }

    // Verify post-wraparound
    queue.push(999).unwrap();
    assert_eq!(queue.pop(), Some(999));

    let elapsed = start.elapsed();
    println!("Test completed in {:?}", elapsed);
    assert!(elapsed.as_secs() < 10, "Test took too long: {:?}", elapsed);
}
