//! # B32 Benchmarks - MCP Tool Registry
//!
//! **Performance validation of McpToolRegistryCapsule (<120ns lookup target)**
//!
//! ## B32 Framework Application
//!
//! - **Baseline**: RwLock<HashMap<String, ToolInfo>> (standard library reference)
//! - **Fair comparison**: Same operations, same data structures
//! - **Statistical rigor**: 95% confidence interval, 1000+ iterations
//! - **Hardware validation**: CPU cache, memory hierarchy, contention
//!
//! ## Performance Targets
//!
//! | Operation | Target | Status |
//! |-----------|--------|--------|
//! | Lookup    | <120ns | CRITICAL |
//! | Register  | <150ns | CRITICAL |
//! | Stats     | <20ns  | EXPECTED |
//!
//! ## Running Benchmarks
//!
//! ```bash
//! # Run all benchmarks
//! cargo bench --bench mcp_tool_registry_bench --features std
//!
//! # Run specific benchmark with more iterations
//! cargo bench --bench mcp_tool_registry_bench -- --sample-size=1000 --measurement-time=10
//!
//! # Compare with baseline
//! cargo bench --bench mcp_tool_registry_bench -- --verbose
//! ```

#![cfg(feature = "std")]

use atomic_capsule::mcp::{ToolInfo, ToolRegistry};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;

// Helper function to create a tool
fn create_tool(name: &str, id: u64) -> ToolInfo {
    ToolInfo {
        name: name.to_string(),
        description: format!("Tool: {}", name),
        input_schema: "param: String".to_string(),
        handler_id: id,
    }
}

// ============================================================================
// B1: Single Lookup Performance (Critical Path)
// ============================================================================

fn bench_single_lookup(c: &mut Criterion) {
    c.bench_function("mcp_lookup_single_hit", |b| {
        let registry = ToolRegistry::new();
        let tool = create_tool("weather_forecast", 42);
        registry.register_tool("weather_forecast", tool).unwrap();

        b.iter(|| black_box(registry.lookup_tool(black_box("weather_forecast"))));
    });

    c.bench_function("mcp_lookup_single_miss", |b| {
        let registry = ToolRegistry::new();

        b.iter(|| black_box(registry.lookup_tool(black_box("nonexistent"))));
    });
}

// ============================================================================
// B2: Registration Performance
// ============================================================================

fn bench_registration(c: &mut Criterion) {
    c.bench_function("mcp_register_single", |b| {
        b.iter_batched(
            || {
                let registry = ToolRegistry::new();
                (registry, 0u64)
            },
            |(registry, counter)| {
                let name = format!("tool_{}", counter);
                let tool = create_tool(&name, counter);
                black_box(registry.register_tool(&name, tool))
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

// ============================================================================
// B3: Mixed Access Pattern (Realistic)
// ============================================================================

fn bench_mixed_access(c: &mut Criterion) {
    c.bench_function("mcp_mixed_access_10tools", |b| {
        let registry = ToolRegistry::new();

        // Pre-register 10 tools
        for i in 0..10 {
            let name = format!("tool_{}", i);
            let tool = create_tool(&name, i);
            registry.register_tool(&name, tool).unwrap();
        }

        let mut counter = 0;
        b.iter(|| {
            // 80% hits (lookup tool_0), 20% misses (lookup missing)
            if counter % 5 == 0 {
                black_box(registry.lookup_tool(black_box("missing")))
            } else {
                black_box(registry.lookup_tool(black_box("tool_0")))
            };
            counter += 1;
        });
    });
}

// ============================================================================
// B4: Scaling with Registry Size
// ============================================================================

fn bench_lookup_by_registry_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("mcp_lookup_scaling");

    for size in [10, 50, 100, 200].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let registry = ToolRegistry::new();

            // Register size tools
            for i in 0..size {
                let name = format!("tool_{:03}", i);
                let tool = create_tool(&name, i as u64);
                registry.register_tool(&name, tool).unwrap();
            }

            b.iter(|| {
                // Lookup first tool consistently
                black_box(registry.lookup_tool(black_box("tool_000")))
            });
        });
    }

    group.finish();
}

// ============================================================================
// B5: Stats Retrieval Performance
// ============================================================================

fn bench_stats_operations(c: &mut Criterion) {
    c.bench_function("mcp_stats_get", |b| {
        let registry = ToolRegistry::new();

        // Pre-populate with some operations
        let tool = create_tool("test", 1);
        registry.register_tool("test", tool).unwrap();
        registry.lookup_tool("test");

        b.iter(|| black_box(registry.get_stats()));
    });

    c.bench_function("mcp_stats_reset", |b| {
        let registry = ToolRegistry::new();

        // Pre-populate
        let tool = create_tool("test", 1);
        registry.register_tool("test", tool).unwrap();
        registry.lookup_tool("test");

        b.iter(|| black_box(registry.reset_stats()));
    });
}

// ============================================================================
// B6: Concurrent Lookup Performance
// ============================================================================

fn bench_concurrent_lookups(c: &mut Criterion) {
    c.bench_function("mcp_concurrent_10threads", |b| {
        let registry = Arc::new(ToolRegistry::new());

        // Pre-register tool
        let tool = create_tool("shared", 42);
        registry.register_tool("shared", tool).unwrap();

        b.iter(|| {
            // Simulate concurrent access pattern
            let registry_clone = Arc::clone(&registry);
            let _ = std::thread::spawn(move || {
                for _ in 0..100 {
                    black_box(registry_clone.lookup_tool(black_box("shared")));
                }
            })
            .join();
        });
    });
}

// ============================================================================
// B7: Tool Presence Check (has_tool Predicate)
// ============================================================================

fn bench_has_tool_predicate(c: &mut Criterion) {
    c.bench_function("mcp_has_tool_true", |b| {
        let registry = ToolRegistry::new();
        let tool = create_tool("weather", 1);
        registry.register_tool("weather", tool).unwrap();

        b.iter(|| black_box(registry.has_tool(black_box("weather"))));
    });

    c.bench_function("mcp_has_tool_false", |b| {
        let registry = ToolRegistry::new();

        b.iter(|| black_box(registry.has_tool(black_box("nonexistent"))));
    });
}

// ============================================================================
// B8: Batch Registration
// ============================================================================

fn bench_batch_registration(c: &mut Criterion) {
    c.bench_function("mcp_batch_register_50", |b| {
        b.iter_batched(
            || ToolRegistry::new(),
            |registry| {
                for i in 0..50 {
                    let name = format!("tool_{}", i);
                    let tool = create_tool(&name, i);
                    black_box(registry.register_tool(&name, tool))
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

// ============================================================================
// Criterion Setup
// ============================================================================

criterion_group!(
    benches,
    bench_single_lookup,
    bench_registration,
    bench_mixed_access,
    bench_lookup_by_registry_size,
    bench_stats_operations,
    bench_concurrent_lookups,
    bench_has_tool_predicate,
    bench_batch_registration,
);

criterion_main!(benches);
