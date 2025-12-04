// Test visibility of AuthGuard methods
use kdb_mcp::AuthGuard;

fn main() {
    let guard = AuthGuard::default();
    guard.test_set_total_requests(42);
    println!("Success!");
}
