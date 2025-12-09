//! Audio pipeline demo adjusting processing depending on breaker state.

use atomic_breaker::breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR, State};
use atomic_breaker::cause;
use atomic_breaker::layout::standard64;

fn main() {
    let base = standard64::with_level(standard64::with_state(0, State::Open.bits()), 3);
    let metrics = standard64::pack_metrics(24, 5200, 4300, cause::JIT | cause::LAT, 5);
    let word = standard64::with_metrics(base, metrics);
    let breaker = AtomicBreakerSWeMR::from_packed(word);
    let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
    let action = match guard.state() {
        State::Closed => "Full oversampling",
        State::HalfOpen => "Shorter IR",
        State::Open => "Bypass heavy FX",
        State::ForcedOpen => "Mute output",
    };
    println!(
        "Audio breaker {:?} => {} (level={}, cause=0x{:02X})",
        guard.state(),
        action,
        guard.level(),
        guard.cause()
    );
}
