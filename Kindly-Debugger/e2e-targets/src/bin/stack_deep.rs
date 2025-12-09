//! Deep stack unwinding test target.
//!
//! This binary creates 10+ nested function calls to test:
//! - Stack unwinding accuracy
//! - Frame pointer traversal
//! - Return address extraction
//! - SIMD-accelerated stack walking performance
//!
//! # Stack Structure
//! The call chain is:
//! main -> level_1 -> level_2 -> ... -> level_10 -> wait_loop
//!
//! Each function is marked `#[inline(never)]` to ensure distinct stack frames.

use std::io::Write;

/// Deepest level - waits in an infinite loop.
/// The E2E harness captures stack traces here.
#[inline(never)]
#[no_mangle]
pub fn level_10() {
    eprintln!("stack_deep: reached level_10, entering wait loop");
    loop {
        std::hint::black_box(10u64);
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

#[inline(never)]
#[no_mangle]
pub fn level_9() {
    std::hint::black_box(9u64);
    level_10();
}

#[inline(never)]
#[no_mangle]
pub fn level_8() {
    std::hint::black_box(8u64);
    level_9();
}

#[inline(never)]
#[no_mangle]
pub fn level_7() {
    std::hint::black_box(7u64);
    level_8();
}

#[inline(never)]
#[no_mangle]
pub fn level_6() {
    std::hint::black_box(6u64);
    level_7();
}

#[inline(never)]
#[no_mangle]
pub fn level_5() {
    std::hint::black_box(5u64);
    level_6();
}

#[inline(never)]
#[no_mangle]
pub fn level_4() {
    std::hint::black_box(4u64);
    level_5();
}

#[inline(never)]
#[no_mangle]
pub fn level_3() {
    std::hint::black_box(3u64);
    level_4();
}

#[inline(never)]
#[no_mangle]
pub fn level_2() {
    std::hint::black_box(2u64);
    level_3();
}

#[inline(never)]
#[no_mangle]
pub fn level_1() {
    std::hint::black_box(1u64);
    level_2();
}

fn main() {
    // Print PID for harness detection
    println!("PID: {}", std::process::id());
    let _ = std::io::stdout().flush();

    eprintln!("stack_deep: starting descent through 10 levels");

    // Begin the deep call chain
    level_1();

    // Never reached
    println!("stack_deep: unexpected exit");
}
