//! UI pipeline demo showcasing breaker level degrade table.

use atomic_breaker::breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR, State};
use atomic_breaker::cause;
use atomic_breaker::layout::standard64;

fn main() {
    let base = standard64::with_level(standard64::with_state(0, State::HalfOpen.bits()), 2);
    let metrics = standard64::pack_metrics(12, 4800, 3100, cause::LAT, 3);
    let word = standard64::with_metrics(base, metrics);
    let breaker = AtomicBreakerSWeMR::from_packed(word);
    let guard = AtomicBreakerGuard::new(breaker.load_acquire());
    let level = guard.level();
    let action = match level {
        0 => "Full fidelity",
        1 => "Reduce bloom",
        2 => "Disable parallax",
        _ => "Cap frame rate",
    };
    println!(
        "UI breaker {:?} level {} => {} (cause=0x{:02X})",
        guard.state(),
        level,
        action,
        guard.cause()
    );
}
