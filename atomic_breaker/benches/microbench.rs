#![cfg(feature = "std")]

use std::sync::Arc;
use std::thread;
use std::time::Instant;

#[cfg(feature = "mpmc")]
use atomic_breaker::breaker::AtomicBreakerMPMC;
use atomic_breaker::breaker::{AtomicBreakerSWeMR, State};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_load_store(c: &mut Criterion) {
    let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
    c.bench_function("load_relaxed", |b| {
        b.iter(|| breaker.load_relaxed());
    });
    c.bench_function("store_release", |b| {
        b.iter(|| breaker.store_release(breaker.load_relaxed()));
    });
}

fn bench_multi_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpmc-lite");
    for writers in [1usize, 4, 8, 16] {
        group.bench_with_input(BenchmarkId::from_parameter(writers), &writers, |b, &w| {
            #[cfg(feature = "mpmc")]
            let breaker = Arc::new(AtomicBreakerMPMC::new(State::Closed));
            #[cfg(not(feature = "mpmc"))]
            let breaker = Arc::new(AtomicBreakerSWeMR::new_standard64(State::Closed));
            b.iter_custom(|iters| {
                let mut handles = Vec::new();
                for _ in 0..w {
                    let b = breaker.clone();
                    handles.push(thread::spawn(move || {
                        for _ in 0..iters {
                            b.store_release(b.load_relaxed());
                        }
                    }));
                }
                let start = Instant::now();
                for h in handles {
                    h.join().unwrap();
                }
                start.elapsed()
            });
        });
    }
    group.finish();
}

#[cfg(feature = "compact48")]
fn bench_compact_layout(c: &mut Criterion) {
    use atomic_breaker::breaker::AtomicBreakerSWeMR as CompactBreaker;
    let breaker = CompactBreaker::new_compact48(State::Closed);
    let mut group = c.benchmark_group("compact48");
    group.throughput(Throughput::Elements(1));
    group.bench_function("load_relaxed", |b| b.iter(|| breaker.load_relaxed()));
    group.bench_function("store_release", |b| {
        b.iter(|| breaker.store_release(breaker.load_relaxed()))
    });
    group.finish();
}

#[cfg(not(feature = "compact48"))]
fn bench_compact_layout(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_load_store,
    bench_multi_thread,
    bench_compact_layout
);
criterion_main!(benches);
