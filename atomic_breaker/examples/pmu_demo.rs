//! Demonstrates updating a breaker using PMU-derived telemetry samples.

#[cfg(all(feature = "pmu", target_os = "linux"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::thread;
    use std::time::Duration;

    use atomic_breaker::breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR, State};
    use atomic_breaker::policy::{self, Policy};
    use atomic_breaker::telemetry::{PmuCollector, PmuConfig, TelemetrySource};

    let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
    let policy = Policy::io_disk();
    let mut last_change = 0u32;
    let mut collector = PmuCollector::new(PmuConfig::default())?;

    for _ in 0..10 {
        let sample = collector.poll();
        policy::evaluate_with_telemetry(
            &breaker,
            &sample,
            monotonic_ms(),
            &mut last_change,
            &policy,
        );
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        println!(
            "state={:?} level={} err={} mu_norm={} sg_norm={}",
            guard.state(),
            guard.level(),
            guard.err(),
            sample.mu_norm,
            sample.sg_norm
        );
        thread::sleep(collector.config().interval);
    }

    Ok(())
}

#[cfg(all(feature = "pmu", target_os = "linux"))]
fn monotonic_ms() -> u32 {
    use std::time::Instant;
    static START: once_cell::sync::Lazy<Instant> = once_cell::sync::Lazy::new(Instant::now);
    START.elapsed().as_millis() as u32
}

#[cfg(not(all(feature = "pmu", target_os = "linux")))]
fn main() {
    eprintln!("pmu_demo requires --features \"pmu\" on a Linux target");
}
