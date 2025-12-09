//! Verify Arc<T> size for inline storage
//!
//! NOTE: Requires `arc_support` feature - AtomicCapsuleMap currently requires V: Copy
//! To enable: `cargo test --features arc_support`

#![cfg(all(test, feature = "arc_support"))]

use std::sync::Arc;

#[test]
fn verify_arc_is_pointer_sized() {
    assert_eq!(std::mem::size_of::<Arc<String>>(), 8);
    assert_eq!(std::mem::size_of::<Arc<Vec<u64>>>(), 8);
    assert_eq!(std::mem::size_of::<Arc<[u8; 1024]>>(), 8);

    // Arc is always pointer-sized regardless of T
    println!(
        "Arc<String> size: {} bytes",
        std::mem::size_of::<Arc<String>>()
    );
}
