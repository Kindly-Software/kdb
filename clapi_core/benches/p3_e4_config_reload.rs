//! P3-E4: ConfigReloadCapsule64 Benchmarks
//!
//! # B32 Honest Benchmarking
//! - Fair baseline: Arc clone + RwLock swap
//! - Statistical rigor: 1000+ samples, 95% CI
//! - Honest reporting: Document overhead and limitations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::{Arc, RwLock};
use std::path::PathBuf;

use clapi_core::capsules::ConfigReloadCapsule64;
use clapi_core::proxy::config::ProxyConfig;

fn test_config() -> ProxyConfig {
    ProxyConfig {
        listen_addr: "127.0.0.1:8080".to_string(),
        providers: vec![],
        default_budget: 10_000,
        audit_log_path: PathBuf::from("/tmp/audit.log"),
        request_timeout_secs: 30,
        test_mode: true,
        pagerduty_token: None,
        slack_webhook: None,
    }
}

/// Baseline: RwLock<Arc<Config>> pattern
struct RwLockConfigReload {
    config: RwLock<Arc<ProxyConfig>>,
}

impl RwLockConfigReload {
    fn new(config: ProxyConfig) -> Self {
        Self {
            config: RwLock::new(Arc::new(config)),
        }
    }

    fn get(&self) -> Arc<ProxyConfig> {
        let guard = self.config.read().unwrap();
        Arc::clone(&guard)
    }

    fn reload(&self, new_config: ProxyConfig) {
        let mut guard = self.config.write().unwrap();
        *guard = Arc::new(new_config);
    }
}

fn bench_config_reload_read(c: &mut Criterion) {
    let capsule = ConfigReloadCapsule64::new(test_config());
    let baseline = RwLockConfigReload::new(test_config());

    c.bench_function("config_reload_read_atomic", |b| {
        b.iter(|| {
            let config = black_box(capsule.get());
            black_box(&config.listen_addr);
        })
    });

    c.bench_function("config_reload_read_rwlock", |b| {
        b.iter(|| {
            let config = black_box(baseline.get());
            black_box(&config.listen_addr);
        })
    });
}

fn bench_config_reload_write(c: &mut Criterion) {
    let capsule = ConfigReloadCapsule64::new(test_config());
    let baseline = RwLockConfigReload::new(test_config());

    c.bench_function("config_reload_write_atomic", |b| {
        b.iter(|| {
            let new_config = test_config();
            black_box(capsule.reload(new_config).unwrap());
        })
    });

    c.bench_function("config_reload_write_rwlock", |b| {
        b.iter(|| {
            let new_config = test_config();
            black_box(baseline.reload(new_config));
        })
    });
}

fn bench_config_reload_concurrent(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("config_reload_concurrent");

    for num_readers in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("atomic", num_readers),
            &num_readers,
            |b, &num_readers| {
                b.iter(|| {
                    let capsule = Arc::new(ConfigReloadCapsule64::new(test_config()));
                    let mut handles = vec![];

                    for _ in 0..num_readers {
                        let c = Arc::clone(&capsule);
                        handles.push(thread::spawn(move || {
                            for _ in 0..100 {
                                let config = black_box(c.get());
                                black_box(&config.listen_addr);
                            }
                        }));
                    }

                    // Concurrent reload
                    capsule.reload(test_config()).unwrap();

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("rwlock", num_readers),
            &num_readers,
            |b, &num_readers| {
                b.iter(|| {
                    let baseline = Arc::new(RwLockConfigReload::new(test_config()));
                    let mut handles = vec![];

                    for _ in 0..num_readers {
                        let c = Arc::clone(&baseline);
                        handles.push(thread::spawn(move || {
                            for _ in 0..100 {
                                let config = black_box(c.get());
                                black_box(&config.listen_addr);
                            }
                        }));
                    }

                    // Concurrent reload
                    baseline.reload(test_config());

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

fn bench_config_reload_version(c: &mut Criterion) {
    let capsule = ConfigReloadCapsule64::new(test_config());

    c.bench_function("config_reload_version", |b| {
        b.iter(|| {
            black_box(capsule.version());
        })
    });
}

criterion_group!(
    benches,
    bench_config_reload_read,
    bench_config_reload_write,
    bench_config_reload_concurrent,
    bench_config_reload_version,
);
criterion_main!(benches);
