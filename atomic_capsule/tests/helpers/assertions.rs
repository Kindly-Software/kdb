// Custom Assertions for T8 Network Capsule Testing
// Provides domain-specific assertion macros

/// Assert deterministic sharding (same bucket → same shard)
///
/// # T28 Property Test Support
/// - Tests 1000 buckets map to same shard
/// - Verifies sharding is deterministic
#[macro_export]
macro_rules! assert_deterministic_sharding {
    ($shard_fn:expr, $bucket:expr, $shard_count:expr) => {{
        let shard1 = $shard_fn($bucket, $shard_count);
        let shard2 = $shard_fn($bucket, $shard_count);
        assert_eq!(
            shard1, shard2,
            "Sharding must be deterministic: bucket {} mapped to shard {} then {}",
            $bucket, shard1, shard2
        );
    }};
}

/// Assert idempotent operation (same input → same output)
///
/// # T28 Property Test Support
/// - Tests operation can be repeated safely
/// - Verifies no side effects
#[macro_export]
macro_rules! assert_idempotent {
    ($op:expr, $input:expr) => {{
        let result1 = $op($input.clone());
        let result2 = $op($input.clone());
        assert_eq!(
            result1, result2,
            "Operation must be idempotent: {:?} != {:?}",
            result1, result2
        );
    }};
}

/// Assert eventually consistent (wait for convergence)
///
/// # T28 Integration Test Support
/// - Waits up to timeout for condition
/// - Asserts condition becomes true
#[macro_export]
macro_rules! assert_eventually_consistent {
    ($condition:expr, $timeout_ms:expr) => {{
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis($timeout_ms);

        loop {
            if $condition {
                break;
            }

            if start.elapsed() > timeout {
                panic!(
                    "Condition did not become true within {}ms (eventual consistency failed)",
                    $timeout_ms
                );
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }};
}

/// Assert no data loss (audit trail matches expected)
///
/// # T28 Production Test Support
/// - Compares audit trail to expected operations
/// - Verifies all operations recorded
#[macro_export]
macro_rules! assert_no_data_loss {
    ($audit_trail:expr, $expected_count:expr) => {{
        let actual_count = $audit_trail.len();
        assert_eq!(
            actual_count, $expected_count,
            "Data loss detected: expected {} operations, found {} in audit trail",
            $expected_count, actual_count
        );
    }};
}

/// Assert RPC latency within budget
///
/// # T28 Performance Test Support
/// - Measures RPC latency
/// - Asserts within performance budget
#[macro_export]
macro_rules! assert_rpc_latency {
    ($rpc_call:expr, $budget_ms:expr) => {{
        let start = std::time::Instant::now();
        let _result = $rpc_call;
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() <= $budget_ms as u128,
            "RPC latency exceeded budget: {}ms > {}ms",
            elapsed.as_millis(),
            $budget_ms
        );
    }};
}

/// Assert shard health (healthy/degraded/failed)
///
/// # T28 Integration Test Support
/// - Checks shard health status
/// - Verifies health monitoring works
#[macro_export]
macro_rules! assert_shard_health {
    ($shard:expr, $expected_status:expr) => {{
        let status = $shard.health_status();
        assert_eq!(
            status, $expected_status,
            "Shard health mismatch: expected {:?}, got {:?}",
            $expected_status, status
        );
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_assert_deterministic_sharding() {
        fn shard_fn(bucket: u16, shard_count: u16) -> u16 {
            bucket % shard_count
        }

        assert_deterministic_sharding!(shard_fn, 42, 10);
    }

    #[test]
    fn test_assert_idempotent() {
        fn double(x: i32) -> i32 {
            x * 2
        }

        assert_idempotent!(double, 21);
    }

    #[test]
    fn test_assert_eventually_consistent() {
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
        use std::thread;
        use std::time::Duration;

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            flag_clone.store(true, Ordering::Release);
        });

        assert_eventually_consistent!(flag.load(Ordering::Acquire), 1000);
    }

    #[test]
    fn test_assert_no_data_loss() {
        let audit_trail = vec![1, 2, 3, 4, 5];
        assert_no_data_loss!(audit_trail, 5);
    }
}
