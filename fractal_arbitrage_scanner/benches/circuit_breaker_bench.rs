//! Circuit breaker overhead benchmark
//!
//! UCE32 Q30 (Empirical Validation): Verify <10ns overhead per breaker check
//! ASSUM framework validation for ASSUME_BRANCHLESS assumption

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use fractal_arbitrage_scanner::hydra::HydraCoordinationEngine;
use atomic_breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR};
use atomic_breaker::breaker::State as BreakerState;

/// Benchmark circuit breaker overhead (UCE32 Q30: <10ns requirement)
fn bench_circuit_breaker_overhead(c: &mut Criterion) {
    let engine = HydraCoordinationEngine::new();

    let mut group = c.benchmark_group("circuit_breaker");
    group.throughput(Throughput::Elements(1));

    // Benchmark the critical path: breaker check overhead
    group.bench_function("breaker_check_overhead", |b| {
        b.iter(|| {
            // This simulates the exact check in perform_coordinated_analysis
            let breaker_guard = AtomicBreakerGuard::new(black_box(engine.breaker.load_acquire()));
            let _state_check = match breaker_guard.state() {
                BreakerState::Open | BreakerState::ForcedOpen => false,
                _ => true,
            };
            black_box(_state_check)
        })
    });

    // Benchmark baseline without breaker for comparison
    group.bench_function("baseline_no_breaker", |b| {
        b.iter(|| {
            // Minimal work to establish baseline
            black_box(true)
        })
    });

    // Benchmark emergency halt performance
    group.bench_function("emergency_halt", |b| {
        let mut engine = HydraCoordinationEngine::new();
        b.iter(|| {
            engine.emergency_halt().unwrap();
            engine.reset_emergency_halt().unwrap();
        })
    });

    group.finish();
}

criterion_group!(benches, bench_circuit_breaker_overhead);
criterion_main!(benches);