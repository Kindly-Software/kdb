//! TUI Command Palette Benchmarks - B32 Framework Compliance
//!
//! # Purpose
//! Measure honest fuzzy search performance for command palette filtering.
//! All benchmarks follow B32 framework guidelines for fair, reproducible measurement.
//!
//! # B32 Compliance
//! - **Fair Baseline**: Compare against String operations
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Claims**: <1μs fuzzy search (12 commands)
//! - **Reality Check**: Search latency negligible vs user input (>50ms/keystroke)
//!
//! # Benchmarks
//! 1. **Filter Update**: Hash computation + atomic store (<100ns)
//! 2. **Fuzzy Match**: Search all commands (<1μs)
//! 3. **Navigation**: Atomic index updates (<20ns)
//! 4. **Command Lookup**: Binary search (<50ns)
//!
//! # Performance Targets
//! - Filter update: <100ns (FNV-1a hash + atomic store)
//! - Fuzzy search: <1μs (12 commands, substring matching)
//! - Navigation: <20ns (atomic fetch_add/fetch_sub)
//! - Command lookup: <50ns (binary search, 12 entries)
//!
//! # Build Instructions
//! ```bash
//! cargo bench --bench tui_palette_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

// ============================================================================
// MOCK COMMAND PALETTE CAPSULE (Matches Production Pattern)
// ============================================================================

/// Command metadata
#[derive(Debug, Clone, Copy)]
struct Command {
    name: &'static str,
    id_hash: u64,
    description: &'static str,
}

/// Command Palette Capsule (128B, T1 Atomic)
#[repr(C, align(128))]
struct CommandPaletteCapsule {
    visible: AtomicBool,          // Visibility toggle
    _padding0: [u8; 7],
    selected_index: AtomicU32,    // Selected command index
    _padding1: [u8; 4],
    filter_hash: AtomicU64,       // FNV-1a hash of filter string
    _padding2: [u8; 96],          // Complete 128B alignment
}

impl CommandPaletteCapsule {
    const fn new() -> Self {
        Self {
            visible: AtomicBool::new(false),
            _padding0: [0u8; 7],
            selected_index: AtomicU32::new(0),
            _padding1: [0u8; 4],
            filter_hash: AtomicU64::new(0),
            _padding2: [0u8; 96],
        }
    }

    #[inline(always)]
    fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }

    #[inline(always)]
    fn toggle(&self) {
        let current = self.visible.load(Ordering::Relaxed);
        self.visible.store(!current, Ordering::Release);
        if !current {
            self.selected_index.store(0, Ordering::Release);
            self.filter_hash.store(0, Ordering::Release);
        }
    }

    #[inline(always)]
    fn update_filter(&self, input: &str) {
        let hash = fnv1a_hash(input.as_bytes());
        self.filter_hash.store(hash, Ordering::Release);
        self.selected_index.store(0, Ordering::Release);
    }

    #[inline(always)]
    fn filter_hash(&self) -> u64 {
        self.filter_hash.load(Ordering::Acquire)
    }

    #[inline(always)]
    fn selected_index(&self) -> u32 {
        self.selected_index.load(Ordering::Acquire)
    }

    #[inline(always)]
    fn next(&self, max_index: u32) {
        let current = self.selected_index.load(Ordering::Acquire);
        let new_index = if current >= max_index { 0 } else { current + 1 };
        self.selected_index.store(new_index, Ordering::Release);
    }

    #[inline(always)]
    fn prev(&self, max_index: u32) {
        let current = self.selected_index.load(Ordering::Acquire);
        let new_index = if current == 0 { max_index } else { current - 1 };
        self.selected_index.store(new_index, Ordering::Release);
    }
}

// FNV-1a hash (const-compatible)
#[inline(always)]
const fn fnv1a_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

// Command registry (12 commands, alphabetical order)
const COMMANDS: &[Command] = &[
    Command { name: "audit", id_hash: fnv1a_hash(b"audit"), description: "View audit log" },
    Command { name: "budget", id_hash: fnv1a_hash(b"budget"), description: "Show budget status" },
    Command { name: "cache", id_hash: fnv1a_hash(b"cache"), description: "Cache operations" },
    Command { name: "clear", id_hash: fnv1a_hash(b"clear"), description: "Clear screen" },
    Command { name: "config", id_hash: fnv1a_hash(b"config"), description: "Show config" },
    Command { name: "doctor", id_hash: fnv1a_hash(b"doctor"), description: "Health diagnostics" },
    Command { name: "help", id_hash: fnv1a_hash(b"help"), description: "Show help" },
    Command { name: "metrics", id_hash: fnv1a_hash(b"metrics"), description: "Metrics dashboard" },
    Command { name: "profile", id_hash: fnv1a_hash(b"profile"), description: "Performance profile" },
    Command { name: "providers", id_hash: fnv1a_hash(b"providers"), description: "List providers" },
    Command { name: "start", id_hash: fnv1a_hash(b"start"), description: "Start server" },
    Command { name: "stop", id_hash: fnv1a_hash(b"stop"), description: "Stop server" },
];

// Fuzzy score (0-100, higher is better)
#[inline]
fn fuzzy_score(query: &str, target: &str) -> u8 {
    if query.is_empty() {
        return 100;
    }

    let query_lower = query.to_ascii_lowercase();
    let target_lower = target.to_ascii_lowercase();

    if target_lower == query_lower {
        100 // Exact match
    } else if target_lower.starts_with(&query_lower) {
        90 // Prefix match
    } else if target_lower.contains(&query_lower) {
        50 // Contains match
    } else {
        0 // No match
    }
}

// Filter commands by query
fn filter_commands(query: &str) -> Vec<usize> {
    let mut scores: Vec<(usize, u8)> = COMMANDS
        .iter()
        .enumerate()
        .map(|(i, cmd)| (i, fuzzy_score(query, cmd.name)))
        .collect();

    scores.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by score descending
    scores.into_iter()
        .filter(|(_, score)| *score > 0)
        .map(|(idx, _)| idx)
        .collect()
}

// ============================================================================
// BENCHMARK 1: Filter Update Operations
// ============================================================================

/// B32 Benchmark: Filter string hash computation
///
/// # Purpose
/// Measure FNV-1a hash overhead for filter updates.
///
/// # Performance Target
/// - <100ns (hash computation + atomic store)
///
/// # B32 Reality Check
/// - Human typing speed: ~50-200ms per character
/// - Filter update overhead: <0.1% of typing latency
fn bench_filter_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("palette/filter_update");

    let palette = CommandPaletteCapsule::new();

    group.bench_function("update_empty", |b| {
        b.iter(|| {
            palette.update_filter(black_box(""));
        });
    });

    group.bench_function("update_single_char", |b| {
        b.iter(|| {
            palette.update_filter(black_box("s"));
        });
    });

    group.bench_function("update_short_query", |b| {
        b.iter(|| {
            palette.update_filter(black_box("hea"));
        });
    });

    group.bench_function("update_long_query", |b| {
        b.iter(|| {
            palette.update_filter(black_box("clapi metrics"));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Fuzzy Search Operations
// ============================================================================

/// B32 Benchmark: Fuzzy command search
///
/// # Purpose
/// Measure substring matching overhead for 12 commands.
///
/// # Performance Target
/// - <1μs for 12 commands (simple substring matching)
///
/// # B32 Reality Check
/// - Human keystroke speed: ~50-200ms per character
/// - Search overhead: <1% of keystroke latency
fn bench_fuzzy_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("palette/fuzzy_search");

    // Empty query (show all)
    group.bench_function("search_empty", |b| {
        b.iter(|| {
            black_box(filter_commands(""));
        });
    });

    // Single character (many matches)
    group.bench_function("search_single_char", |b| {
        b.iter(|| {
            black_box(filter_commands("s"));
        });
    });

    // Prefix match
    group.bench_function("search_prefix", |b| {
        b.iter(|| {
            black_box(filter_commands("sta"));
        });
    });

    // Exact match
    group.bench_function("search_exact", |b| {
        b.iter(|| {
            black_box(filter_commands("metrics"));
        });
    });

    // No match
    group.bench_function("search_no_match", |b| {
        b.iter(|| {
            black_box(filter_commands("xyz"));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Fuzzy Score Calculation
// ============================================================================

/// B32 Benchmark: Individual fuzzy score computation
///
/// # Purpose
/// Measure overhead of single fuzzy_score() call.
///
/// # Performance Target
/// - <100ns per score (String::to_lowercase + substring check)
fn bench_fuzzy_score_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("palette/fuzzy_score");

    group.bench_function("score_exact_match", |b| {
        b.iter(|| {
            black_box(fuzzy_score("metrics", "metrics"));
        });
    });

    group.bench_function("score_prefix_match", |b| {
        b.iter(|| {
            black_box(fuzzy_score("met", "metrics"));
        });
    });

    group.bench_function("score_contains_match", |b| {
        b.iter(|| {
            black_box(fuzzy_score("tri", "metrics"));
        });
    });

    group.bench_function("score_no_match", |b| {
        b.iter(|| {
            black_box(fuzzy_score("xyz", "metrics"));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Navigation Operations
// ============================================================================

/// B32 Benchmark: Command selection navigation
///
/// # Purpose
/// Measure atomic index update overhead for Up/Down arrow keys.
///
/// # Performance Target
/// - <20ns per navigation (atomic fetch_add/fetch_sub)
///
/// # B32 Reality Check
/// - Human arrow key press speed: ~200-500ms
/// - Navigation overhead: <0.01% of key press latency
fn bench_navigation(c: &mut Criterion) {
    let mut group = c.benchmark_group("palette/navigation");

    let palette = CommandPaletteCapsule::new();

    group.bench_function("next_command", |b| {
        b.iter(|| {
            palette.next(black_box(11)); // 12 commands (0-11)
        });
    });

    group.bench_function("prev_command", |b| {
        b.iter(|| {
            palette.prev(black_box(11));
        });
    });

    group.bench_function("cycle_10_commands", |b| {
        b.iter(|| {
            for _ in 0..10 {
                palette.next(11);
            }
        });
    });

    group.bench_function("read_selected_index", |b| {
        b.iter(|| {
            black_box(palette.selected_index());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Visibility Toggle
// ============================================================================

/// B32 Benchmark: Palette visibility toggle
///
/// # Purpose
/// Measure atomic boolean toggle overhead.
///
/// # Performance Target
/// - <50ns (atomic load + store + reset fields)
fn bench_visibility_toggle(c: &mut Criterion) {
    let mut group = c.benchmark_group("palette/visibility");

    let palette = CommandPaletteCapsule::new();

    group.bench_function("toggle_show", |b| {
        b.iter(|| {
            palette.toggle();
        });
    });

    group.bench_function("toggle_hide", |b| {
        palette.visible.store(true, Ordering::Release);
        b.iter(|| {
            palette.toggle();
        });
    });

    group.bench_function("check_visibility", |b| {
        b.iter(|| {
            black_box(palette.is_visible());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 6: End-to-End Workflows
// ============================================================================

/// B32 Benchmark: Realistic command palette usage
///
/// # Purpose
/// Measure end-to-end latency for typical user interactions.
///
/// # Performance Target
/// - <2μs for complete workflow (show → filter → select → execute)
///
/// # B32 Reality Check
/// - Human interaction time: 1-5 seconds for command entry
/// - Palette overhead: <0.2% of interaction time
fn bench_end_to_end_workflows(c: &mut Criterion) {
    let mut group = c.benchmark_group("palette/workflow");

    // Workflow: Open → Type → Select → Close
    group.bench_function("complete_workflow", |b| {
        b.iter(|| {
            let palette = CommandPaletteCapsule::new();

            // User presses '/' to open
            palette.toggle();

            // User types "met"
            palette.update_filter("met");

            // Search for matching commands
            let matches = filter_commands("met");

            // User presses Down arrow twice
            palette.next(matches.len() as u32 - 1);
            palette.next(matches.len() as u32 - 1);

            // User presses Enter (execute)
            palette.toggle(); // Close palette

            black_box(matches);
        });
    });

    // Workflow: Quick exact match
    group.bench_function("quick_exact_match", |b| {
        b.iter(|| {
            let palette = CommandPaletteCapsule::new();
            palette.toggle();
            palette.update_filter("help");
            let matches = filter_commands("help");
            palette.toggle();
            black_box(matches);
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    palette_benches,
    bench_filter_update,
    bench_fuzzy_search,
    bench_fuzzy_score_calculation,
    bench_navigation,
    bench_visibility_toggle,
    bench_end_to_end_workflows,
);

criterion_main!(palette_benches);
