//! Tests for HttpKeepAliveCapsule (T1 Atomic)
//!
//! **T28 Tier 2: Property-Based Tests**
//!
//! This file contains property-based tests for the HTTP keep-alive connection
//! timeout tracking capsule, exercising the timeout algorithm and state machine.

#[cfg(test)]
mod tests {
    use crate::http::{ConnectionState, HttpKeepAliveCapsule};

    // Test 1: Basic construction
    #[test]
    fn test_new() {
        let capsule = HttpKeepAliveCapsule::new(90_000_000_000);
        assert_eq!(capsule.get_state(), ConnectionState::Active);
        assert_eq!(capsule.get_request_count(), 0);
        assert_eq!(capsule.get_total_bytes_read(), 0);
        assert_eq!(capsule.get_total_bytes_written(), 0);
    }

    // Test 2: Size and alignment
    #[test]
    fn test_size_and_alignment() {
        assert_eq!(
            std::mem::size_of::<HttpKeepAliveCapsule>(),
            64,
            "HttpKeepAliveCapsule must be exactly 64 bytes"
        );
        assert_eq!(
            std::mem::align_of::<HttpKeepAliveCapsule>(),
            64,
            "HttpKeepAliveCapsule must be 64-byte aligned"
        );
    }

    // Test 3: Timeout check with active connection
    #[test]
    fn test_is_timed_out_active() {
        let capsule = HttpKeepAliveCapsule::new(100);
        let now = 1000u64;
        capsule.touch(now);

        // Not timed out yet
        assert!(!capsule.is_timed_out(now + 50));
        // Just at the boundary
        assert!(!capsule.is_timed_out(now + 100));
    }

    // Test 4: Timeout check with expired connection
    #[test]
    fn test_is_timed_out_expired() {
        let capsule = HttpKeepAliveCapsule::new(100);
        let now = 1000u64;
        capsule.touch(now);

        // Timed out
        assert!(capsule.is_timed_out(now + 101));
        // Well past timeout
        assert!(capsule.is_timed_out(now + 1000));
    }

    // Test 5: Touch updates last activity
    #[test]
    fn test_touch_updates_activity() {
        let capsule = HttpKeepAliveCapsule::new(100);
        let now1 = 1000u64;
        capsule.touch(now1);
        assert!(!capsule.is_timed_out(now1 + 50));

        // Touch again later
        let now2 = 2000u64;
        capsule.touch(now2);
        // Should not be timed out even though 1100 ns have passed since first touch
        assert!(!capsule.is_timed_out(now2 + 50));
    }

    // Test 6: Close transitions to CLOSED state
    #[test]
    fn test_close() {
        let capsule = HttpKeepAliveCapsule::new(100);
        assert_eq!(capsule.get_state(), ConnectionState::Active);

        capsule.close();
        assert_eq!(capsule.get_state(), ConnectionState::Closed);
    }

    // Test 7: Mark idle
    #[test]
    fn test_mark_idle() {
        let capsule = HttpKeepAliveCapsule::new(100);
        assert_eq!(capsule.get_state(), ConnectionState::Active);

        capsule.mark_idle();
        assert_eq!(capsule.get_state(), ConnectionState::Idle);

        // Touch should return to ACTIVE
        capsule.touch(1000);
        assert_eq!(capsule.get_state(), ConnectionState::Active);
    }

    // Test 8: Request counting
    #[test]
    fn test_request_count() {
        let capsule = HttpKeepAliveCapsule::new(100);
        assert_eq!(capsule.get_request_count(), 0);

        capsule.increment_request_count();
        assert_eq!(capsule.get_request_count(), 1);

        for _ in 0..99 {
            capsule.increment_request_count();
        }
        assert_eq!(capsule.get_request_count(), 100);
    }

    // Test 9: Bytes tracking
    #[test]
    fn test_bytes_tracking() {
        let capsule = HttpKeepAliveCapsule::new(100);
        assert_eq!(capsule.get_total_bytes_read(), 0);
        assert_eq!(capsule.get_total_bytes_written(), 0);

        capsule.add_bytes_read(256);
        assert_eq!(capsule.get_total_bytes_read(), 256);

        capsule.add_bytes_written(512);
        assert_eq!(capsule.get_total_bytes_written(), 512);

        // Add more
        capsule.add_bytes_read(64);
        capsule.add_bytes_written(128);
        assert_eq!(capsule.get_total_bytes_read(), 320);
        assert_eq!(capsule.get_total_bytes_written(), 640);
    }

    // Test 10: Connection ID
    #[test]
    fn test_connection_id() {
        let capsule = HttpKeepAliveCapsule::new(100);
        assert_eq!(capsule.get_connection_id(), 0);

        capsule.set_connection_id(12345);
        assert_eq!(capsule.get_connection_id(), 12345);

        capsule.set_connection_id(u64::MAX);
        assert_eq!(capsule.get_connection_id(), u64::MAX);
    }

    // Test 11: Time until timeout
    #[test]
    fn test_time_until_timeout() {
        let capsule = HttpKeepAliveCapsule::new(100);
        let now = 1000u64;
        capsule.touch(now);

        // 50ns remaining
        if let Some(remaining) = capsule.time_until_timeout(now + 50) {
            assert_eq!(remaining, 50);
        } else {
            panic!("Should have time remaining");
        }

        // Expired
        assert_eq!(capsule.time_until_timeout(now + 101), None);
    }

    // Test 12: Concurrent activity (multi-threaded)
    #[test]
    fn test_concurrent_activity() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(HttpKeepAliveCapsule::new(1000));
        let mut handles = vec![];

        // Spawn 4 threads that touch the capsule concurrently
        for i in 0..4 {
            let cap = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                for j in 0..25 {
                    let now = (i * 25 + j) as u64 * 10;
                    cap.touch(now);
                    cap.increment_request_count();
                    cap.add_bytes_read(100);
                    cap.add_bytes_written(200);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Check final state
        assert_eq!(capsule.get_request_count(), 100);
        assert_eq!(capsule.get_total_bytes_read(), 10000);
        assert_eq!(capsule.get_total_bytes_written(), 20000);
        assert_eq!(capsule.get_state(), ConnectionState::Active);
    }

    // Test 13: Large timeout values
    #[test]
    fn test_large_timeout() {
        let capsule = HttpKeepAliveCapsule::new((1u64 << 32) - 1);
        assert_eq!(capsule.get_state(), ConnectionState::Active);
    }
}
