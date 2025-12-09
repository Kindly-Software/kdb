#![cfg(feature = "std")]

//! Pseudo-random stress workload toggling the breaker.

use atomic_breaker::breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR, State};
use atomic_breaker::policy::{self, Policy};
use atomic_breaker::telemetry::TelemetrySample;
use rand::{rngs::StdRng, Rng, SeedableRng};

fn drive(policy: &Policy, iterations: usize, seed: u64) {
    let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
    let mut last_change = 0u32;
    let mut now = 0u32;
    let mut rng = StdRng::seed_from_u64(seed);

    for step in 0..iterations {
        now = now.wrapping_add(rng.gen_range(1..=32));
        let mu = rng.gen_range(0.0..4.0);
        let sg = rng.gen_range(0.0..4.0);
        let err = rng.gen_range(0..16) as u16;

        let sample = TelemetrySample {
            mu_norm: mu,
            sg_norm: sg,
            err_inc: err,
            cause: 0,
            backoff_hint: None,
        };

        policy::evaluate_with_telemetry(&breaker, &sample, now, &mut last_change, policy);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());

        if step % 64 == 0 {
            println!(
                "step={:04} state={:?} level={} err={} cause=0x{:02X}",
                step,
                guard.state(),
                guard.level(),
                guard.err(),
                guard.cause()
            );
        }
    }
}

fn main() {
    let policy = Policy::ui_holographic();
    drive(&policy, 1_024, 0xD15EA5E);
}
