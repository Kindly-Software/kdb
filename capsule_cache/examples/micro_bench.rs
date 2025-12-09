//! Minimal microbench (no external deps). Measures hit/insert latencies.

use capsule_cache::{sharded::ShardedCache, CapsuleCache};
use std::time::{Duration, Instant};

const ITER: usize = 50_000;

fn main() {
    let cache = CapsuleCache::<String>::new();
    cache
        .insert("hot".into(), "v".into(), Duration::from_secs(60))
        .unwrap();
    let mut hit_samples = Vec::with_capacity(ITER);
    for _ in 0..ITER {
        let start = Instant::now();
        let _ = cache.get(&"hot".into());
        hit_samples.push(start.elapsed().as_nanos() as f64);
    }
    hit_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |p: f64| {
        let idx = ((p / 100.0) * (hit_samples.len() as f64 - 1.0)) as usize;
        hit_samples[idx]
    };

    let cache = ShardedCache::<String>::new(4, 8_192);
    let start_ins = Instant::now();
    for i in 0..ITER {
        let _ = cache.insert(format!("k{i}"), "v".into(), Duration::from_secs(60));
    }
    let ins_ns = start_ins.elapsed().as_nanos() as f64 / ITER as f64;

    println!(
        "hit p50: {:.1} ns, p95: {:.1} ns, p99: {:.1} ns | insert avg: {:.1} ns",
        pct(50.0),
        pct(95.0),
        pct(99.0),
        ins_ns
    );
}
