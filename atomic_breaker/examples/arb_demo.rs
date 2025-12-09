//! Trading venue demo that adapts gating based on breaker readings.

use atomic_breaker::breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR, State};
use atomic_breaker::cause;
use atomic_breaker::layout::standard64;

fn main() {
    let base = standard64::with_level(standard64::with_state(0, State::HalfOpen.bits()), 1);
    let metrics = standard64::pack_metrics(6, 3600, 2200, cause::LAT, 1);
    let word = standard64::with_metrics(base, metrics);
    let breaker = AtomicBreakerSWeMR::from_packed(word);
    let guard = AtomicBreakerGuard::new(breaker.load_acquire());
    let routing = match guard.level() {
        0 => "Route all venues",
        1 => "Prefer primary venue",
        2 => "Halve clip size",
        _ => "Pause venue",
    };
    println!(
        "Venue state {:?} level {} => {} (cause=0x{:02X})",
        guard.state(),
        guard.level(),
        routing,
        guard.cause()
    );
}
