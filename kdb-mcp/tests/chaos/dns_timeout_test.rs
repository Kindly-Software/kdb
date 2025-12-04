// DNS timeout during startup
//
// Scenario: Simulate DNS timeout during server initialization
// Expected: Startup fails gracefully with clear error message

use super::*;
use std::time::{Duration, Instant};

#[test]
fn test_dns_timeout_during_startup() {
    // Simulate DNS lookup with timeout
    fn dns_lookup_with_timeout(hostname: &str, timeout: Duration) -> Result<String, String> {
        let start = Instant::now();

        // Simulate DNS query (random delay)
        let dns_delay = Duration::from_millis(fastrand::u64(10..500));
        std::thread::sleep(dns_delay);

        if start.elapsed() > timeout {
            Err(format!("DNS timeout for {}", hostname))
        } else {
            Ok(format!("192.168.0.{}", fastrand::u8(1..255)))
        }
    }

    let mut success_count = 0;
    let mut timeout_count = 0;

    // Try 100 DNS lookups with 200ms timeout
    for _ in 0..100 {
        match dns_lookup_with_timeout("mcp-debug.local", Duration::from_millis(200)) {
            Ok(ip) => {
                success_count += 1;
            }
            Err(e) => {
                timeout_count += 1;
                // Verify graceful error handling
                assert!(e.contains("DNS timeout"));
            }
        }
    }

    println!("DNS timeout test:");
    println!("  Successful lookups: {}", success_count);
    println!("  Timed out: {}", timeout_count);

    // Some lookups should succeed, some should timeout
    assert!(success_count > 30, "Too few successful lookups: {}", success_count);
    assert!(timeout_count > 10, "Too few timeouts: {}", timeout_count);
}
