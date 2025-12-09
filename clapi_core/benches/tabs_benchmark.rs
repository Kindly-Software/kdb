//! Tab State Capsule Benchmarks - B32 Framework Validation
//!
//! **Purpose**: Validate <5ns tab switch performance claims
//! **Framework**: B32 (honest measurement, fair baselines, 95% CI)
//!
//! # Performance Targets (from tabs.rs)
//! - Tab read: <3ns (Relaxed atomic load)
//! - Tab write: <5ns (Relaxed atomic store)
//! - Next/prev: <8ns (Relaxed load + store with bounds check)

use clapi_core::tui::tabs::{DashboardTab, TabStateCapsule};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_tab_read(c: &mut Criterion) {
    let tabs = TabStateCapsule::new();
    tabs.set_tab(2);

    c.bench_function("tab_read", |b| {
        b.iter(|| {
            black_box(tabs.get_tab());
        });
    });
}

fn bench_tab_write(c: &mut Criterion) {
    let tabs = TabStateCapsule::new();

    c.bench_function("tab_write", |b| {
        b.iter(|| {
            tabs.set_tab(black_box(2));
        });
    });
}

fn bench_tab_next(c: &mut Criterion) {
    let tabs = TabStateCapsule::new();

    c.bench_function("tab_next", |b| {
        b.iter(|| {
            tabs.next_tab();
        });
    });
}

fn bench_tab_prev(c: &mut Criterion) {
    let tabs = TabStateCapsule::new();

    c.bench_function("tab_prev", |b| {
        b.iter(|| {
            tabs.prev_tab();
        });
    });
}

fn bench_enum_conversion(c: &mut Criterion) {
    let tabs = TabStateCapsule::new();

    c.bench_function("enum_get", |b| {
        b.iter(|| {
            black_box(tabs.get_tab_enum());
        });
    });

    c.bench_function("enum_set", |b| {
        b.iter(|| {
            tabs.set_tab_enum(black_box(DashboardTab::Performance));
        });
    });
}

fn bench_tab_cycling(c: &mut Criterion) {
    let tabs = TabStateCapsule::new();

    c.bench_function("full_cycle_forward", |b| {
        b.iter(|| {
            tabs.set_tab(0);
            for _ in 0..5 {
                tabs.next_tab();
            }
        });
    });

    c.bench_function("full_cycle_backward", |b| {
        b.iter(|| {
            tabs.set_tab(0);
            for _ in 0..5 {
                tabs.prev_tab();
            }
        });
    });
}

fn bench_contention(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("contention");

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let tabs = Arc::new(TabStateCapsule::new());
                    let mut handles = vec![];

                    for i in 0..num_threads {
                        let tabs_clone = Arc::clone(&tabs);
                        let handle = thread::spawn(move || {
                            for _ in 0..100 {
                                if i % 2 == 0 {
                                    tabs_clone.next_tab();
                                } else {
                                    tabs_clone.prev_tab();
                                }
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_tab_read,
    bench_tab_write,
    bench_tab_next,
    bench_tab_prev,
    bench_enum_conversion,
    bench_tab_cycling,
    bench_contention
);
criterion_main!(benches);
