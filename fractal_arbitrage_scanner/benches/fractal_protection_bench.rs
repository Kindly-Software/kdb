//! B32 Framework: Fractal Protection Validation Benchmarks
//!
//! Empirical validation of fractal protection performance improvement claims.
//! Following UCE32 Q30 (Empirical Validation) + Q29 (Practical Constraints) + Kontext27 Hardware Reality.
//!
//! # Protection Hypothesis
//!
//! Fractal protection provides 10-30% performance improvement over baseline through:
//! 1. **Cache-aware fractal memory**: √N storage complexity vs O(N) baseline
//! 2. **O(1) k-NN CAKES manifold**: O(1) vs O(log N) search baseline
//! 3. **Lockfree DualAtomicU64**: No contention vs mutex baseline
//! 4. **SIMD acceleration**: 3-4x vectorized distance calculations
//! 5. **Proof-of-work protection**: One-time overhead, persistent benefit
//!
//! # B32 Framework Compliance
//!
//! - **B1 Fair Baselines**: Compare against optimized implementations (parking_lot, dashmap)
//! - **B2 Statistical Rigor**: 1000+ iterations, 95% confidence intervals
//! - **B3 Realistic Workloads**: Production-scale market data patterns
//! - **B5 Reporting Standards**: P50/P95/P99 percentiles, hardware specs
//! - **Kontext27 Reality**: Expect 10-30% typical gains, validate claims
//!
//! # ASSUM Safety Framework
//!
//! All atomic operations follow ASSUM framework validation:
//! - #ASSUME_MEMORY_ORDERING: Acquire/Release for synchronization
//! - #VERIFY_ATOMIC_TIMING: Validate CAS performance against Kontext27 (15-25ns)
//! - #ASSUME_CACHE_ALIGNMENT: 64-byte single, 128-byte complex coordination

use criterion::{
    black_box, criterion_group, criterion_main,
    BenchmarkId, Criterion, Throughput,
    BatchSize
};
use fractal_arbitrage_scanner::{
    // Core fractal components
    FractalArbitrageScanner, QuantumArbitrageScanner,
    FractalMemoryManager, MarketPoint, DualAtomicU64,
    CakesManifoldEngine,

    // Analysis modules
    MultifractalDFA, WilliamsFractal, WaveletLeaders,

    // Types and utilities
    ArbitrageOpportunity, TemporalArbitrageOpportunity,
    FractalCacheKey, FractalAnalysisType, aid_class, Aid96,

    // Protection system
    DefaultAdaptiveParams, PerformanceMetrics, ProtectionTier
};

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use parking_lot::Mutex as ParkingMutex;
use dashmap::DashMap;

/// Kontext27 Reality Check: Expected performance baselines (Intel Ultra 7 155H)
const TYPICAL_IMPROVEMENT: f64 = 1.15;      // 15% improvement typical
const GOOD_IMPROVEMENT: f64 = 1.30;         // 30% improvement good
const EXCEPTIONAL_IMPROVEMENT: f64 = 2.0;   // 2x exceptional, needs validation
const SUSPICIOUS_THRESHOLD: f64 = 10.0;     // 10x+ suspicious without algorithm change

/// B32 Framework: Statistical validation requirements
const MIN_ITERATIONS: u64 = 1000;
const CONFIDENCE_LEVEL: f64 = 0.95;
const WARMUP_ITERATIONS: u64 = 100;

/// Realistic data sizes for production validation
const DATA_SIZES: [usize; 6] = [100, 500, 1000, 2000, 5000, 10000];
const MARKET_SYMBOLS: [&str; 4] = ["BTC/USD", "ETH/USD", "SOL/USD", "DOGE/USD"];

/// Cache line sizes for hardware-aware testing (Kontext27 K6)
const L1_CACHE_SIZE: usize = 48_000;  // 48KB L1 data cache
const L2_CACHE_SIZE: usize = 2_000_000; // 2MB L2 cache
const L3_CACHE_SIZE: usize = 24_000_000; // 24MB L3 shared cache

//
// BASELINE IMPLEMENTATIONS (Fair Comparisons)
//

/// Unprotected baseline using standard synchronization
struct UnprotectedBaseline {
    data: Arc<Mutex<HashMap<String, Vec<f64>>>>,
    access_count: AtomicU64,
}

impl UnprotectedBaseline {
    fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
            access_count: AtomicU64::new(0),
        }
    }

    fn store_data(&self, key: String, values: Vec<f64>) {
        let mut data = self.data.lock().unwrap();
        data.insert(key, values);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    fn retrieve_data(&self, key: &str) -> Option<Vec<f64>> {
        let data = self.data.lock().unwrap();
        self.access_count.fetch_add(1, Ordering::Relaxed);
        data.get(key).cloned()
    }

    fn linear_search(&self, target: f64, data: &[f64]) -> Option<usize> {
        // O(N) linear search baseline
        data.iter().position(|&x| (x - target).abs() < f64::EPSILON)
    }
}

/// Optimized baseline using parking_lot (fair comparison, not strawman)
struct OptimizedBaseline {
    data: Arc<ParkingMutex<HashMap<String, Vec<f64>>>>,
    access_count: AtomicU64,
}

impl OptimizedBaseline {
    fn new() -> Self {
        Self {
            data: Arc::new(ParkingMutex::new(HashMap::new())),
            access_count: AtomicU64::new(0),
        }
    }

    fn store_data(&self, key: String, values: Vec<f64>) {
        let mut data = self.data.lock();
        data.insert(key, values);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    fn retrieve_data(&self, key: &str) -> Option<Vec<f64>> {
        let data = self.data.lock();
        self.access_count.fetch_add(1, Ordering::Relaxed);
        data.get(key).cloned()
    }

    fn binary_search(&self, target: f64, data: &[f64]) -> Option<usize> {
        // O(log N) binary search baseline
        data.binary_search_by(|&x| x.partial_cmp(&target).unwrap()).ok()
    }
}

/// DashMap baseline (lockfree alternative)
struct DashMapBaseline {
    data: Arc<DashMap<String, Vec<f64>>>,
    access_count: AtomicU64,
}

impl DashMapBaseline {
    fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            access_count: AtomicU64::new(0),
        }
    }

    fn store_data(&self, key: String, values: Vec<f64>) {
        self.data.insert(key, values);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    fn retrieve_data(&self, key: &str) -> Option<Vec<f64>> {
        self.access_count.fetch_add(1, Ordering::Relaxed);
        self.data.get(key).map(|entry| entry.value().clone())
    }
}

//
// PROTECTED IMPLEMENTATIONS
//

/// Fractal protected implementation
struct FractalProtected {
    memory_manager: FractalMemoryManager,
    cakes_engine: CakesManifoldEngine,
    dual_atomic: DualAtomicU64,
    generation: AtomicU64,
}

impl FractalProtected {
    fn new() -> Self {
        Self {
            memory_manager: FractalMemoryManager::new(),
            cakes_engine: CakesManifoldEngine::new(),
            dual_atomic: DualAtomicU64::new(0, 0),
            generation: AtomicU64::new(1),
        }
    }

    fn store_with_fractal_protection(&mut self, key: String, values: Vec<f64>) {
        let cache_key = FractalCacheKey::new(key, 300_000, FractalAnalysisType::HurstExponent);
        self.memory_manager.store(cache_key, values);
        self.dual_atomic.fetch_add_primary(1, Ordering::Relaxed);
    }

    fn retrieve_with_protection(&self, key: &str) -> Option<Vec<f64>> {
        let cache_key = FractalCacheKey::new(key.to_string(), 300_000, FractalAnalysisType::HurstExponent);
        let result = self.memory_manager.retrieve(&cache_key);
        self.dual_atomic.fetch_add_secondary(1, Ordering::Relaxed);
        result
    }

    fn manifold_search(&mut self, target: f64, points: &[MarketPoint]) -> Result<Vec<(usize, f32)>, Box<dyn std::error::Error>> {
        // Add points to CAKES engine
        for point in points {
            self.cakes_engine.add_point(*point)?;
        }

        // Build the k-NN graph
        self.cakes_engine.build_graph()?;

        // Create query point
        let query = MarketPoint::new([target, target, target, 1.0], 0, 0, 0);

        // Perform k-NN search
        let result = self.cakes_engine.search_knn(&query, 1)?;
        Ok(result)
    }
}

//
// DATA GENERATION UTILITIES
//

/// Generate realistic market data with fractal properties
fn generate_fractal_market_data(size: usize, hurst: f64) -> Vec<f64> {
    let mut data = Vec::with_capacity(size);
    let mut price = 50000.0; // Starting BTC price

    // Use fractional Brownian motion with specified Hurst exponent
    for i in 0..size {
        let white_noise = (i as f64 * 0.1).sin() * 0.01; // Simplified noise
        let trend = 0.0001 * (i as f64); // Small trend
        let fractal_component = hurst * white_noise + (1.0 - hurst) * trend;

        price += price * fractal_component;
        data.push(price);
    }

    data
}

/// Generate market points for CAKES manifold testing
fn generate_market_points(size: usize) -> Vec<MarketPoint> {
    let mut points = Vec::with_capacity(size);
    let base_price = 50000.0;

    for i in 0..size {
        let bid = base_price + (i as f64 * 0.1);
        let ask = bid + 10.0; // 10 USD spread
        let last = (bid + ask) / 2.0;
        let volume = 1.0 + (i as f64 * 0.01);

        points.push(MarketPoint::new(
            [bid, ask, last, volume],
            i as u64 * 1_000_000, // 1ms intervals
            (i % 4) as u8, // 4 exchanges
            (i % 100) as u16, // 100 markets
        ));
    }

    points
}

//
// BENCHMARK IMPLEMENTATIONS
//

/// B1: Fair baseline comparison - Memory storage operations
fn bench_memory_storage_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_storage");
    group.confidence_level(CONFIDENCE_LEVEL);
    group.sample_size(MIN_ITERATIONS as usize);
    group.warm_up_time(Duration::from_millis(1000));

    for &size in &DATA_SIZES {
        let data = generate_fractal_market_data(size, 0.7);

        // Baseline: Standard mutex
        group.bench_with_input(
            BenchmarkId::new("baseline_mutex", size),
            &size,
            |b, &_size| {
                let baseline = UnprotectedBaseline::new();
                b.iter_batched(
                    || data.clone(),
                    |data| {
                        for (i, symbol) in MARKET_SYMBOLS.iter().enumerate() {
                            let key = format!("{}_{}", symbol, i);
                            baseline.store_data(black_box(key), black_box(data.clone()));
                        }
                    },
                    BatchSize::SmallInput
                );
            },
        );

        // Fair comparison: parking_lot mutex (optimized)
        group.bench_with_input(
            BenchmarkId::new("optimized_parking_lot", size),
            &size,
            |b, &_size| {
                let optimized = OptimizedBaseline::new();
                b.iter_batched(
                    || data.clone(),
                    |data| {
                        for (i, symbol) in MARKET_SYMBOLS.iter().enumerate() {
                            let key = format!("{}_{}", symbol, i);
                            optimized.store_data(black_box(key), black_box(data.clone()));
                        }
                    },
                    BatchSize::SmallInput
                );
            },
        );

        // Fair comparison: DashMap (lockfree)
        group.bench_with_input(
            BenchmarkId::new("dashmap_lockfree", size),
            &size,
            |b, &_size| {
                let dashmap = DashMapBaseline::new();
                b.iter_batched(
                    || data.clone(),
                    |data| {
                        for (i, symbol) in MARKET_SYMBOLS.iter().enumerate() {
                            let key = format!("{}_{}", symbol, i);
                            dashmap.store_data(black_box(key), black_box(data.clone()));
                        }
                    },
                    BatchSize::SmallInput
                );
            },
        );

        // Protected: Fractal memory management
        group.bench_with_input(
            BenchmarkId::new("fractal_protected", size),
            &size,
            |b, &_size| {
                let mut protected = FractalProtected::new();
                b.iter_batched(
                    || data.clone(),
                    |data| {
                        for (i, symbol) in MARKET_SYMBOLS.iter().enumerate() {
                            let key = format!("{}_{}", symbol, i);
                            protected.store_with_fractal_protection(black_box(key), black_box(data.clone()));
                        }
                    },
                    BatchSize::SmallInput
                );
            },
        );
    }

    group.finish();
}

/// B3: Search operation comparison - O(1) vs O(N) vs O(log N)
fn bench_search_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_operations");
    group.confidence_level(CONFIDENCE_LEVEL);
    group.sample_size(MIN_ITERATIONS as usize);

    for &size in &DATA_SIZES[0..4] { // Limit size for manifold testing
        let data = generate_fractal_market_data(size, 0.7);
        let sorted_data = {
            let mut sorted = data.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            sorted
        };
        let market_points = generate_market_points(size);
        let target = data[size / 2]; // Middle element

        group.throughput(Throughput::Elements(size as u64));

        // Baseline: Linear search O(N)
        group.bench_with_input(
            BenchmarkId::new("linear_search_baseline", size),
            &size,
            |b, &_size| {
                let baseline = UnprotectedBaseline::new();
                b.iter(|| {
                    black_box(baseline.linear_search(black_box(target), black_box(&data)))
                });
            },
        );

        // Optimized baseline: Binary search O(log N)
        group.bench_with_input(
            BenchmarkId::new("binary_search_optimized", size),
            &size,
            |b, &_size| {
                let optimized = OptimizedBaseline::new();
                b.iter(|| {
                    black_box(optimized.binary_search(black_box(target), black_box(&sorted_data)))
                });
            },
        );

        // Protected: CAKES manifold O(1) search
        group.bench_with_input(
            BenchmarkId::new("cakes_manifold_o1", size),
            &size,
            |b, &_size| {
                let mut protected = FractalProtected::new();
                b.iter(|| {
                    let _ = black_box(protected.manifold_search(black_box(target), black_box(&market_points)));
                });
            },
        );
    }

    group.finish();
}

/// Cold start vs warm operation performance
fn bench_cold_start_vs_warm(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_vs_warm");
    group.confidence_level(CONFIDENCE_LEVEL);
    group.measurement_time(Duration::from_secs(10));

    let size = 2000;
    let data = generate_fractal_market_data(size, 0.7);

    // Cold start: No cache warming
    group.bench_function("cold_start_fractal", |b| {
        b.iter_batched(
            || {
                // Create fresh instance each time (cold)
                FractalProtected::new()
            },
            |mut protected| {
                // Perform operations on cold cache
                for (i, symbol) in MARKET_SYMBOLS.iter().enumerate() {
                    let key = format!("cold_{}_{}", symbol, i);
                    protected.store_with_fractal_protection(black_box(key), black_box(data.clone()));
                }
            },
            BatchSize::SmallInput
        );
    });

    // Warm operation: Pre-warmed cache
    group.bench_function("warm_operation_fractal", |b| {
        let mut protected = FractalProtected::new();

        // Pre-warm the cache
        for (i, symbol) in MARKET_SYMBOLS.iter().enumerate() {
            let key = format!("warm_{}_{}", symbol, i);
            protected.store_with_fractal_protection(key, data.clone());
        }

        b.iter(|| {
            // Operations on warm cache
            for (i, symbol) in MARKET_SYMBOLS.iter().enumerate() {
                let key = format!("warm_{}_{}", symbol, i);
                black_box(protected.retrieve_with_protection(black_box(&key)));
            }
        });
    });

    // Proof-of-work validation (one-time cost)
    group.bench_function("proof_of_work_setup", |b| {
        b.iter(|| {
            // Simulate proof-of-work setup cost
            let mut mfdfa = MultifractalDFA::new();
            let _hurst = mfdfa.calculate_hurst(black_box(&data));

            let mut williams = WilliamsFractal::new();
            let _fractals = williams.detect_fractals(black_box(&data));

            black_box(())
        });
    });

    group.finish();
}

/// Different fractal depth tier testing
fn bench_fractal_depth_tiers(c: &mut Criterion) {
    let mut group = c.benchmark_group("fractal_depth_tiers");
    group.confidence_level(CONFIDENCE_LEVEL);

    let data = generate_fractal_market_data(1000, 0.7);

    // Tier 1: Basic fractal analysis
    group.bench_function("tier1_basic", |b| {
        b.iter(|| {
            let mut mfdfa = MultifractalDFA::new();
            black_box(mfdfa.calculate_hurst(black_box(&data)))
        });
    });

    // Tier 2: Williams fractal detection
    group.bench_function("tier2_williams", |b| {
        b.iter(|| {
            let mut williams = WilliamsFractal::new();
            black_box(williams.detect_fractals(black_box(&data)))
        });
    });

    // Tier 3: Complete fractal analysis pipeline
    group.bench_function("tier3_complete", |b| {
        b.iter(|| {
            let mut mfdfa = MultifractalDFA::new();
            let mut williams = WilliamsFractal::new();
            let wl = WaveletLeaders::new();

            let hurst = mfdfa.calculate_hurst(black_box(&data));
            let fractals = williams.detect_fractals(black_box(&data));
            let spectrum = wl.calculate_spectrum(black_box(&data));

            black_box((hurst, fractals, spectrum))
        });
    });

    // Tier 4: Adaptive parameters
    group.bench_function("tier4_adaptive", |b| {
        b.iter(|| {
            let mut mfdfa = MultifractalDFA::new_adaptive();
            let start = Instant::now();
            let hurst = mfdfa.calculate_hurst(black_box(&data));
            let latency = start.elapsed().as_micros() as u64;

            // Update performance metrics
            let _ = mfdfa.update_performance(latency, 0.85);

            black_box(hurst)
        });
    });

    group.finish();
}

/// DualAtomicU64 coordination performance
fn bench_atomic_coordination(c: &mut Criterion) {
    let mut group = c.benchmark_group("atomic_coordination");
    group.confidence_level(CONFIDENCE_LEVEL);

    // Standard atomic operations
    group.bench_function("standard_atomic_u64", |b| {
        let atomic = AtomicU64::new(0);
        b.iter(|| {
            black_box(atomic.fetch_add(black_box(1), Ordering::Relaxed))
        });
    });

    // DualAtomicU64 operations
    group.bench_function("dual_atomic_u64", |b| {
        let dual = DualAtomicU64::new(0, 0);
        b.iter(|| {
            black_box(dual.fetch_add_primary(black_box(1), Ordering::Relaxed))
        });
    });

    // Cache-separated coordination
    group.bench_function("cache_separated_coordination", |b| {
        let dual = DualAtomicU64::new(0, 0);
        b.iter(|| {
            // Simulate coordination between channels
            let primary = dual.fetch_add_primary(1, Ordering::Acquire);
            let secondary = dual.load_secondary(Ordering::Relaxed);
            black_box((primary, secondary))
        });
    });

    group.finish();
}

/// Comprehensive arbitrage pipeline performance
fn bench_arbitrage_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("arbitrage_pipeline");
    group.confidence_level(CONFIDENCE_LEVEL);
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(50);

    // Baseline arbitrage detection
    group.bench_function("baseline_arbitrage", |b| {
        b.iter(|| {
            let scanner = QuantumArbitrageScanner::new(42);

            let mut opportunities = Vec::new();
            for symbol in &MARKET_SYMBOLS {
                if let Ok(arb) = scanner.scan_arbitrage(
                    black_box(symbol),
                    black_box("binance"),
                    black_box("coinbase"),
                    black_box(50_000.0),
                    black_box(50_100.0),
                    black_box(1.0),
                ) {
                    opportunities.push(arb);
                }
            }
            black_box(opportunities)
        });
    });

    // Protected arbitrage with fractal analysis
    group.bench_function("fractal_protected_arbitrage", |b| {
        b.iter(|| {
            let scanner = FractalArbitrageScanner::new(42);
            let data = generate_fractal_market_data(500, 0.7);

            // Fractal analysis
            let mut mfdfa = MultifractalDFA::new();
            let hurst = mfdfa.calculate_hurst(&data);

            // Enhanced arbitrage detection with fractal insights
            let mut opportunities = Vec::new();
            for symbol in &MARKET_SYMBOLS {
                if let Ok(arb) = scanner.scan_arbitrage(
                    black_box(symbol),
                    black_box("binance"),
                    black_box("coinbase"),
                    black_box(50_000.0 * (1.0 + hurst * 0.1)),
                    black_box(50_100.0 * (1.0 + hurst * 0.1)),
                    black_box(1.0),
                ) {
                    opportunities.push(arb);
                }
            }
            black_box((opportunities, hurst))
        });
    });

    group.finish();
}

/// Adaptive parameter evolution benchmarks
fn bench_adaptive_parameter_evolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_parameter_evolution");
    group.confidence_level(CONFIDENCE_LEVEL);

    let data = generate_fractal_market_data(1000, 0.7);

    // Static parameters (baseline)
    group.bench_function("static_parameters", |b| {
        b.iter(|| {
            let mut mfdfa = MultifractalDFA::new();
            black_box(mfdfa.calculate_hurst(black_box(&data)))
        });
    });

    // Adaptive parameters with evolution
    group.bench_function("adaptive_evolution", |b| {
        b.iter(|| {
            let mut mfdfa = MultifractalDFA::new_adaptive();

            // Simulate parameter evolution
            for iteration in 0..10 {
                let start = Instant::now();
                let hurst = mfdfa.calculate_hurst(black_box(&data));
                let latency = start.elapsed().as_micros() as u64;

                // Simulate accuracy based on iteration
                let accuracy = 0.7 + (iteration as f64 * 0.02);

                // Update performance for adaptation
                let _ = mfdfa.update_performance(latency, accuracy);

                if iteration == 9 {
                    black_box(hurst);
                }
            }
        });
    });

    group.finish();
}

/// SIMD acceleration validation (Q32 nightly features)
#[cfg(feature = "portable_simd")]
fn bench_simd_acceleration(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_acceleration");
    group.confidence_level(CONFIDENCE_LEVEL);

    let points = generate_market_points(1000);
    let target = MarketPoint::new([50000.0, 50010.0, 50005.0, 1.0], 0, 0, 0);

    // Scalar distance calculations
    group.bench_function("scalar_distance", |b| {
        b.iter(|| {
            let mut distances = Vec::new();
            for point in &points {
                // Scalar implementation
                let mut sum = 0.0;
                for i in 0..4 {
                    let diff = target.prices[i] - point.prices[i];
                    sum += diff * diff;
                }
                distances.push(sum.sqrt());
            }
            black_box(distances)
        });
    });

    // SIMD distance calculations
    group.bench_function("simd_distance", |b| {
        b.iter(|| {
            let mut distances = Vec::new();
            for point in &points {
                let distance = target.euclidean_distance(point);
                distances.push(distance);
            }
            black_box(distances)
        });
    });

    group.finish();
}

#[cfg(not(feature = "portable_simd"))]
fn bench_simd_acceleration(_c: &mut Criterion) {
    // SIMD features not available
}

criterion_group!(
    fractal_protection_benches,
    bench_memory_storage_comparison,
    bench_search_operations,
    bench_cold_start_vs_warm,
    bench_fractal_depth_tiers,
    bench_atomic_coordination,
    bench_arbitrage_pipeline,
    bench_adaptive_parameter_evolution,
    bench_simd_acceleration
);

criterion_main!(fractal_protection_benches);