// B32 Benchmark Framework - Performance Validation for kindly-web
//
// Framework Compliance:
// - B32: Statistical rigor, fair baselines, honest claims
// - T28: Performance testing (Q22-Q28)
// - UCE34: Tier 1 (Atomic) capsule validation
//
// Benchmarks:
// 1. AppStateCapsule read (<10ns target)
// 2. AppStateCapsule write (<100ns target)
// 3. BudgetViewCapsule deduct (<100ns target)
// 4. Component render simulation (<500ns target)
// 5. Full app initialization (<10μs target)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// MOCK CAPSULE IMPLEMENTATIONS (for benchmarking)
// ============================================================================
// Note: These are simplified mock implementations to demonstrate expected
// performance characteristics. Actual implementations will use proper
// computational capsule patterns from atomic_capsule crate.

/// AppStateCapsule - Tier 1 (Atomic) - 64 bytes
///
/// Purpose: Global application state (theme, dark mode, locale)
///
/// Memory Layout:
/// [0-7]   theme_id: AtomicU64          // Current theme (0-3)
/// [8-15]  dark_mode: AtomicBool        // Dark mode enabled
/// [16-23] generation: AtomicU64        // TOCTOU prevention
/// [24-63] _padding: [u8; 40]           // Cache alignment
#[repr(C, align(64))]
struct AppStateCapsule {
    theme_id: AtomicU64,
    dark_mode: AtomicBool,
    generation: AtomicU64,
    _padding: [u8; 40],
}

impl AppStateCapsule {
    fn new() -> Self {
        Self {
            theme_id: AtomicU64::new(0),
            dark_mode: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            _padding: [0u8; 40],
        }
    }

    /// Read operations (<10ns target)
    #[inline(always)]
    fn get_theme(&self) -> u64 {
        self.theme_id.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn is_dark_mode(&self) -> bool {
        self.dark_mode.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Write operations (<100ns target)
    #[inline(always)]
    fn set_theme(&self, theme_id: u64) -> Result<(), &'static str> {
        if theme_id > 3 {
            return Err("Invalid theme ID");
        }
        self.theme_id.store(theme_id, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
        Ok(())
    }

    #[inline(always)]
    fn toggle_dark_mode(&self) {
        let current = self.dark_mode.load(Ordering::Relaxed);
        self.dark_mode.store(!current, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }
}

// Compile-time size verification
const _: () = assert!(std::mem::size_of::<AppStateCapsule>() == 64);
const _: () = assert!(std::mem::align_of::<AppStateCapsule>() == 64);

/// BudgetViewCapsule - Tier 1 (Atomic) - 64 bytes
///
/// Purpose: Client-side budget tracking (display-only, not authoritative)
///
/// Memory Layout:
/// [0-7]   budget_cents: AtomicU64       // Current budget (cents)
/// [8-15]  total_spent: AtomicU64        // Total spent (cents)
/// [16-23] generation: AtomicU64         // TOCTOU prevention
/// [24-31] deduction_count: AtomicU64    // Successful deductions
/// [32-63] _padding: [u8; 32]            // Cache alignment
#[repr(C, align(64))]
struct BudgetViewCapsule {
    budget_cents: AtomicU64,
    total_spent: AtomicU64,
    generation: AtomicU64,
    deduction_count: AtomicU64,
    _padding: [u8; 32],
}

impl BudgetViewCapsule {
    fn new(initial_budget_cents: u64) -> Self {
        Self {
            budget_cents: AtomicU64::new(initial_budget_cents),
            total_spent: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            deduction_count: AtomicU64::new(0),
            _padding: [0u8; 32],
        }
    }

    /// Try to deduct budget (<100ns target)
    ///
    /// Note: This is optimistic deduction for UI responsiveness.
    /// Server-side validation is authoritative.
    #[inline(always)]
    fn try_deduct(&self, cost_cents: u64) -> Result<u64, &'static str> {
        // Optimistic CAS loop
        let mut current = self.budget_cents.load(Ordering::Acquire);
        loop {
            if current < cost_cents {
                return Err("Insufficient budget");
            }

            let new_budget = current - cost_cents;
            match self.budget_cents.compare_exchange_weak(
                current,
                new_budget,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.total_spent.fetch_add(cost_cents, Ordering::Relaxed);
                    self.deduction_count.fetch_add(1, Ordering::Relaxed);
                    self.generation.fetch_add(1, Ordering::Release);
                    return Ok(new_budget);
                }
                Err(actual) => current = actual,
            }
        }
    }

    #[inline(always)]
    fn get_budget(&self) -> u64 {
        self.budget_cents.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

const _: () = assert!(std::mem::size_of::<BudgetViewCapsule>() == 64);
const _: () = assert!(std::mem::align_of::<BudgetViewCapsule>() == 64);

/// ComponentStateCapsule - Tier 1 (Atomic) - 64 bytes
///
/// Purpose: Component-level state (button click counts, form validation)
///
/// Memory Layout:
/// [0-7]   click_count: AtomicU64        // Total clicks
/// [8-15]  is_disabled: AtomicBool       // Disabled state
/// [16-23] generation: AtomicU64         // TOCTOU prevention
/// [24-63] _padding: [u8; 40]            // Cache alignment
#[repr(C, align(64))]
struct ComponentStateCapsule {
    click_count: AtomicU64,
    is_disabled: AtomicBool,
    generation: AtomicU64,
    _padding: [u8; 40],
}

impl ComponentStateCapsule {
    fn new() -> Self {
        Self {
            click_count: AtomicU64::new(0),
            is_disabled: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            _padding: [0u8; 40],
        }
    }

    #[inline(always)]
    fn increment_clicks(&self) {
        self.click_count.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    #[inline(always)]
    fn get_click_count(&self) -> u64 {
        self.click_count.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn set_disabled(&self, disabled: bool) {
        self.is_disabled.store(disabled, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }
}

const _: () = assert!(std::mem::size_of::<ComponentStateCapsule>() == 64);
const _: () = assert!(std::mem::align_of::<ComponentStateCapsule>() == 64);

// ============================================================================
// BENCHMARK 1: AppStateCapsule Read Operations
// ============================================================================
// Target: <10ns per read
// Baseline: Direct atomic load (~2-3ns on modern CPUs)
// Reality: 1-2× overhead acceptable (cache-aligned access)

fn bench_app_state_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("AppStateCapsule Read");
    group.throughput(Throughput::Elements(1));

    let capsule = Arc::new(AppStateCapsule::new());

    // Benchmark: get_theme()
    group.bench_function("get_theme", |b| {
        b.iter(|| {
            black_box(capsule.get_theme());
        });
    });

    // Benchmark: is_dark_mode()
    group.bench_function("is_dark_mode", |b| {
        b.iter(|| {
            black_box(capsule.is_dark_mode());
        });
    });

    // Benchmark: generation()
    group.bench_function("generation", |b| {
        b.iter(|| {
            black_box(capsule.generation());
        });
    });

    // Benchmark: full state snapshot (all reads)
    group.bench_function("full_snapshot", |b| {
        b.iter(|| {
            let theme = capsule.get_theme();
            let dark = capsule.is_dark_mode();
            let gen = capsule.generation();
            black_box((theme, dark, gen));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: AppStateCapsule Write Operations
// ============================================================================
// Target: <100ns per write
// Baseline: Atomic store + generation increment (~10-20ns)
// Reality: 2-5× overhead acceptable (cache coherence, fetch_add)

fn bench_app_state_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("AppStateCapsule Write");
    group.throughput(Throughput::Elements(1));

    let capsule = Arc::new(AppStateCapsule::new());

    // Benchmark: set_theme()
    group.bench_function("set_theme", |b| {
        let mut theme_id = 0u64;
        b.iter(|| {
            capsule.set_theme(black_box(theme_id % 4)).unwrap();
            theme_id += 1;
        });
    });

    // Benchmark: toggle_dark_mode()
    group.bench_function("toggle_dark_mode", |b| {
        b.iter(|| {
            capsule.toggle_dark_mode();
        });
    });

    // Benchmark: concurrent writes (4 threads)
    group.bench_function("concurrent_writes_4t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|i| {
                    let capsule = Arc::clone(&capsule);
                    std::thread::spawn(move || {
                        capsule.set_theme(i % 4).unwrap();
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: BudgetViewCapsule Deduct Operations
// ============================================================================
// Target: <100ns per deduction
// Baseline: Atomic CAS loop (~20-40ns uncontended)
// Reality: 2-5× overhead acceptable (CAS retries, cache coherence)

fn bench_budget_deduct(c: &mut Criterion) {
    let mut group = c.benchmark_group("BudgetViewCapsule Deduct");
    group.throughput(Throughput::Elements(1));

    // Benchmark: successful deduction (uncontended)
    group.bench_function("deduct_success", |b| {
        b.iter_batched(
            || BudgetViewCapsule::new(1_000_000_00), // $1,000,000 (plenty of budget)
            |capsule| {
                black_box(capsule.try_deduct(50_00).unwrap()); // $50
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Benchmark: failed deduction (insufficient budget)
    group.bench_function("deduct_failure", |b| {
        let capsule = BudgetViewCapsule::new(10_00); // $10 (low budget)
        b.iter(|| {
            black_box(capsule.try_deduct(100_00).err()); // $100 (exceeds budget)
        });
    });

    // Benchmark: get_budget() read
    group.bench_function("get_budget", |b| {
        let capsule = BudgetViewCapsule::new(1000_00);
        b.iter(|| {
            black_box(capsule.get_budget());
        });
    });

    // Benchmark: concurrent deductions (4 threads, high contention)
    group.bench_function("concurrent_deduct_4t", |b| {
        b.iter_batched(
            || Arc::new(BudgetViewCapsule::new(10_000_00)), // $10,000
            |capsule| {
                let handles: Vec<_> = (0..4)
                    .map(|_| {
                        let capsule = Arc::clone(&capsule);
                        std::thread::spawn(move || {
                            for _ in 0..10 {
                                let _ = capsule.try_deduct(10_00); // $10 each
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Component Render Simulation
// ============================================================================
// Target: <500ns per render
// Baseline: State read + conditional logic (~50-100ns)
// Reality: 5-10× overhead acceptable (DOM updates not included)

fn bench_component_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("Component Render");
    group.throughput(Throughput::Elements(1));

    let app_state = Arc::new(AppStateCapsule::new());
    let comp_state = Arc::new(ComponentStateCapsule::new());

    // Benchmark: Button render (state read + click count)
    group.bench_function("button_render", |b| {
        b.iter(|| {
            let theme = app_state.get_theme();
            let dark = app_state.is_dark_mode();
            let clicks = comp_state.get_click_count();

            // Simulate render decision
            let _disabled = clicks > 100;
            let _color = if dark { "#FFFFFF" } else { "#1A1A1A" };

            black_box((theme, dark, clicks));
        });
    });

    // Benchmark: Button click handler
    group.bench_function("button_click", |b| {
        b.iter(|| {
            comp_state.increment_clicks();
        });
    });

    // Benchmark: Theme switcher render
    group.bench_function("theme_switcher_render", |b| {
        b.iter(|| {
            let theme = app_state.get_theme();
            let dark = app_state.is_dark_mode();
            let gen = app_state.generation();

            // Simulate theme selection
            let _themes = ["Default", "High Contrast", "Deuteranopia", "Protanopia"];
            let _current_theme = _themes[theme as usize % 4];

            black_box((theme, dark, gen));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Full App Initialization
// ============================================================================
// Target: <10μs total initialization
// Baseline: Multiple capsule allocations (~1-2μs)
// Reality: 5-10× overhead acceptable (one-time cost)

fn bench_app_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("App Initialization");
    group.throughput(Throughput::Elements(1));

    // Benchmark: Full app state initialization
    group.bench_function("full_init", |b| {
        b.iter(|| {
            let app_state = Arc::new(AppStateCapsule::new());
            let budget = Arc::new(BudgetViewCapsule::new(1000_00));
            let comp1 = Arc::new(ComponentStateCapsule::new());
            let comp2 = Arc::new(ComponentStateCapsule::new());

            // Initialize default values
            app_state.set_theme(0).unwrap();
            budget.try_deduct(0).ok(); // Trigger generation counter

            black_box((app_state, budget, comp1, comp2));
        });
    });

    // Benchmark: Partial initialization (app state only)
    group.bench_function("app_state_init", |b| {
        b.iter(|| {
            let app_state = Arc::new(AppStateCapsule::new());
            app_state.set_theme(0).unwrap();
            black_box(app_state);
        });
    });

    // Benchmark: Budget capsule initialization
    group.bench_function("budget_init", |b| {
        b.iter(|| {
            let budget = Arc::new(BudgetViewCapsule::new(black_box(1000_00)));
            black_box(budget);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 6: End-to-End Workflows
// ============================================================================
// Target: <1μs per workflow
// Baseline: Multiple capsule operations (~100-300ns)
// Reality: 3-10× overhead acceptable (realistic usage patterns)

fn bench_workflows(c: &mut Criterion) {
    let mut group = c.benchmark_group("End-to-End Workflows");
    group.throughput(Throughput::Elements(1));

    let app_state = Arc::new(AppStateCapsule::new());
    let budget = Arc::new(BudgetViewCapsule::new(1000_00));
    let button = Arc::new(ComponentStateCapsule::new());

    // Workflow: User clicks button → deduct budget → update UI
    group.bench_function("button_click_workflow", |b| {
        b.iter(|| {
            // 1. Increment click count
            button.increment_clicks();

            // 2. Deduct budget (if sufficient)
            let _ = budget.try_deduct(1_00); // $0.01 per click

            // 3. Read state for UI update
            let clicks = button.get_click_count();
            let remaining = budget.get_budget();

            black_box((clicks, remaining));
        });
    });

    // Workflow: Theme change → update all components
    group.bench_function("theme_change_workflow", |b| {
        let mut theme_id = 0u64;
        b.iter(|| {
            // 1. Change theme
            app_state.set_theme(theme_id % 4).unwrap();

            // 2. Read new theme
            let theme = app_state.get_theme();
            let dark = app_state.is_dark_mode();

            // 3. Update generation (simulates component re-render)
            let gen = app_state.generation();

            black_box((theme, dark, gen));
            theme_id += 1;
        });
    });

    // Workflow: Dark mode toggle → UI refresh
    group.bench_function("dark_mode_toggle_workflow", |b| {
        b.iter(|| {
            // 1. Toggle dark mode
            app_state.toggle_dark_mode();

            // 2. Read all state
            let theme = app_state.get_theme();
            let dark = app_state.is_dark_mode();
            let gen = app_state.generation();

            black_box((theme, dark, gen));
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(1000)           // B32: 1000+ iterations
        .measurement_time(Duration::from_secs(5))
        .warm_up_time(Duration::from_secs(2))
        .confidence_level(0.95);     // B32: 95% CI
    targets =
        bench_app_state_read,
        bench_app_state_write,
        bench_budget_deduct,
        bench_component_render,
        bench_app_initialization,
        bench_workflows
);

criterion_main!(benches);
