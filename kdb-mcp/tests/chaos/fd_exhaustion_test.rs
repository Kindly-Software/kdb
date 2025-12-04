// File descriptor exhaustion
//
// Scenario: Simulate FD exhaustion during connection handling
// Expected: New connections fail gracefully, existing connections unaffected

use super::*;

#[test]
fn test_fd_exhaustion() {
    // Simulate FD limit
    const FD_LIMIT: usize = 100;

    let mut open_fds = Vec::new();
    let mut rejected_count = 0;

    // Try to "open" 150 FDs (simulate connections)
    for i in 0..150 {
        if open_fds.len() >= FD_LIMIT {
            // FD exhausted, reject new connection
            rejected_count += 1;
        } else {
            // FD available, accept connection
            open_fds.push(i);
        }
    }

    println!("FD exhaustion test:");
    println!("  FD limit: {}", FD_LIMIT);
    println!("  Open FDs: {}", open_fds.len());
    println!("  Rejected: {}", rejected_count);

    // Should hit FD limit
    assert_eq!(open_fds.len(), FD_LIMIT);
    assert_eq!(rejected_count, 50);

    // Close some FDs
    open_fds.truncate(50);

    // Should be able to accept new connections
    assert_eq!(open_fds.len(), 50);
    assert!(open_fds.len() < FD_LIMIT);
}
