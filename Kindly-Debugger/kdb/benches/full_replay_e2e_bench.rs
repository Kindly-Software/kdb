//! End-to-End Benchmark: Full State Replay Pipeline
//!
//! Simulates realistic debugging workflows combining session pool and memory replay.
//!
//! # Scenario Coverage
//!
//! 1. **Quick Debug Session**: Attach, inspect, detach (LIGHT tier)
//! 2. **Step Debug Session**: Attach, set breakpoints, step, snapshot (MEDIUM tier)
//! 3. **Full Replay Session**: Attach, capture memory, time-travel (HEAVY tier)
//! 4. **Concurrent Sessions**: Multiple AI agents debugging simultaneously
//!
//! # Performance Targets (B32 Framework)
//!
//! | Scenario                    | Target    | Notes                        |
//! |-----------------------------|-----------|------------------------------|
//! | Quick attach + detach       | <1ms      | LIGHT session lifecycle      |
//! | Step debug (10 steps)       | <10ms     | MEDIUM session + snapshots   |
//! | Full replay (10 snapshots)  | <100ms    | HEAVY session + memory       |
//! | 100 concurrent sessions     | <1s       | Pool utilization stress      |
//! | Session upgrade chain       | <50ms     | LIGHT -> MEDIUM -> HEAVY     |
//!
//! # Integration Points
//!
//! - SessionPoolCapsule: Tiered session management
//! - MemoryReplayCapsule: COW page tracking
//! - Time-travel ReplayEngineCapsule: Register/stack snapshots

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kdb::session_pool::{
    PoolConfig, SessionId, SessionPoolCapsule, SessionTierType,
};
use kdb::memory_replay::{
    MemoryReplayCapsule, ReplayConfig, PAGE_SIZE,
};
use kdb::time_travel::ReplayEngineCapsule;
use std::sync::Arc;
use std::thread;

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate test page with deterministic pattern
fn generate_test_page(seed: u64) -> [u8; PAGE_SIZE] {
    let mut page = [0u8; PAGE_SIZE];
    for i in 0..PAGE_SIZE {
        page[i] = ((seed.wrapping_mul(1103515245).wrapping_add(12345 + i as u64)) >> 16) as u8;
    }
    page
}

/// Simulate attach/detach latency (ptrace overhead)
fn simulate_ptrace_attach() {
    // Ptrace attach typically takes 5-10μs
    // We simulate with a short spin
    std::hint::spin_loop();
}

// ============================================================================
// Quick Debug Session Benchmarks (LIGHT Tier)
// ============================================================================

/// Benchmark quick attach-inspect-detach workflow
///
/// Simulates: AI agent quickly checking process state
/// Target: <1ms total
fn bench_quick_debug_session(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("e2e_quick_debug_session", |b| {
        b.iter(|| {
            // 1. Allocate LIGHT session
            let session_id = pool.allocate_session(SessionTierType::Light).unwrap();

            // 2. Simulate attach (ptrace overhead)
            simulate_ptrace_attach();

            // 3. Quick register read (simulated)
            let rip = black_box(0x7fff_1234_5678u64);
            let rsp = black_box(0x7fff_aaaa_0000u64);

            // 4. Release session
            pool.release_session(session_id).unwrap();

            black_box((rip, rsp))
        })
    });
}

/// Benchmark multiple quick inspections
fn bench_quick_debug_burst(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("e2e_quick_debug_burst_100", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let session_id = pool.allocate_session(SessionTierType::Light).unwrap();
                simulate_ptrace_attach();
                let rip = black_box(0x7fff_1234_5678u64);
                pool.release_session(session_id).unwrap();
                black_box(rip);
            }
        })
    });
}

// ============================================================================
// Step Debug Session Benchmarks (MEDIUM Tier)
// ============================================================================

/// Benchmark step debugging workflow with snapshots
///
/// Simulates: AI agent stepping through code with register snapshots
/// Target: <10ms for 10 steps
fn bench_step_debug_session(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("e2e_step_debug_10_steps", |b| {
        b.iter(|| {
            // 1. Allocate MEDIUM session
            let session_id = pool.allocate_session(SessionTierType::Medium).unwrap();

            // 2. Create replay engine for time-travel
            let engine = ReplayEngineCapsule::new();

            // 3. Simulate 10 step operations with snapshots
            for step in 0..10 {
                let rip = 0x1000 + step * 4;
                let rsp = 0x7fff_0000 - step * 8;

                // Take snapshot
                engine.take_snapshot(rip, rsp).unwrap();

                // Simulate step latency (ptrace overhead)
                simulate_ptrace_attach();
            }

            // 4. Time-travel back 5 steps
            engine.jump_to_snapshot(5).unwrap();
            let snapshot = engine.step_backward();

            // 5. Release session
            pool.release_session(session_id).unwrap();

            black_box(snapshot)
        })
    });
}

/// Benchmark step debugging with breakpoint hits
fn bench_step_debug_with_breakpoints(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("e2e_step_debug_with_breakpoints", |b| {
        b.iter(|| {
            let session_id = pool.allocate_session(SessionTierType::Medium).unwrap();
            let engine = ReplayEngineCapsule::new();

            // Simulate breakpoint setup
            let breakpoints = [0x1000u64, 0x1100, 0x1200, 0x1300, 0x1400];

            // Run until each breakpoint, take snapshot
            for (i, &bp) in breakpoints.iter().enumerate() {
                engine.take_snapshot(bp, 0x7fff_0000 - (i as u64) * 0x100).unwrap();
                simulate_ptrace_attach();
            }

            // Navigate back to first breakpoint
            engine.jump_to_snapshot(0).unwrap();

            pool.release_session(session_id).unwrap();

            black_box(engine.get_stats())
        })
    });
}

// ============================================================================
// Full Replay Session Benchmarks (HEAVY Tier)
// ============================================================================

/// Benchmark full replay session with memory capture
///
/// Simulates: AI agent using time-travel with memory reconstruction
/// Target: <100ms for 10 snapshots with memory
fn bench_full_replay_session(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("e2e_full_replay_10_snapshots", |b| {
        b.iter(|| {
            // 1. Allocate HEAVY session
            let session_id = pool.allocate_session(SessionTierType::Heavy).unwrap();

            // 2. Create replay engines
            let reg_engine = ReplayEngineCapsule::new();
            let mut mem_replay = MemoryReplayCapsule::with_config(ReplayConfig::minimal());
            mem_replay.attach(12345).unwrap();

            let test_page = generate_test_page(12345);
            let memory_reader = |_: u64| -> Result<[u8; PAGE_SIZE], String> {
                Ok(test_page)
            };

            // 3. Capture 10 snapshots with register + memory state
            for i in 0..10 {
                let rip = 0x1000 + i * 4;
                let rsp = 0x7fff_0000 - i * 8;

                // Register snapshot
                reg_engine.take_snapshot(rip, rsp).unwrap();

                // Memory snapshot (mark 10 pages dirty per snapshot)
                for j in 0..10 {
                    mem_replay.mark_page_dirty((j * PAGE_SIZE) as u64);
                }
                let _ = mem_replay.capture_snapshot(&memory_reader);
            }

            // 4. Time-travel back 5 snapshots
            reg_engine.jump_to_snapshot(5).unwrap();
            let _ = mem_replay.navigate_to_snapshot(5);

            // 5. Read memory at that snapshot
            let mem_result = mem_replay.read_memory_at_snapshot(5, 0, 64);

            // 6. Release session
            pool.release_session(session_id).unwrap();

            black_box((reg_engine.get_stats(), mem_result))
        })
    });
}

/// Benchmark intensive memory capture session
fn bench_memory_intensive_session(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("e2e_memory_intensive_100_pages", |b| {
        b.iter(|| {
            let session_id = pool.allocate_session(SessionTierType::Heavy).unwrap();

            let mut mem_replay = MemoryReplayCapsule::with_config(ReplayConfig::performance());
            mem_replay.attach(12345).unwrap();

            let test_page = generate_test_page(12345);
            let memory_reader = |_: u64| -> Result<[u8; PAGE_SIZE], String> {
                Ok(test_page)
            };

            // Capture 5 snapshots with 100 pages each
            for _ in 0..5 {
                for j in 0..100 {
                    mem_replay.mark_page_dirty((j * PAGE_SIZE) as u64);
                }
                let _ = mem_replay.capture_snapshot(&memory_reader);
            }

            let stats = mem_replay.get_stats();
            pool.release_session(session_id).unwrap();

            black_box(stats)
        })
    });
}

// ============================================================================
// Concurrent Sessions Benchmarks
// ============================================================================

/// Benchmark 100 concurrent debug sessions
///
/// Simulates: Multiple AI agents debugging different processes
/// Target: <1s for all sessions to complete
fn bench_concurrent_100_sessions(c: &mut Criterion) {
    let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::default()));

    c.bench_function("e2e_concurrent_100_sessions", |b| {
        b.iter(|| {
            let mut handles = Vec::with_capacity(10);

            // Spawn 10 threads, each handling 10 sessions
            for thread_id in 0..10 {
                let pool_clone = Arc::clone(&pool);
                handles.push(thread::spawn(move || {
                    for i in 0..10 {
                        // Mix of tier usage
                        let tier = match (thread_id + i) % 3 {
                            0 => SessionTierType::Light,
                            1 => SessionTierType::Medium,
                            _ => SessionTierType::Heavy,
                        };

                        if let Ok(session_id) = pool_clone.allocate_session(tier) {
                            // Simulate some work
                            let engine = ReplayEngineCapsule::new();
                            for j in 0..5 {
                                let _ = engine.take_snapshot(
                                    0x1000 + (j * 4),
                                    0x7fff_0000 - (j * 8),
                                );
                            }

                            let _ = pool_clone.release_session(session_id);
                            black_box(engine.get_stats());
                        }
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });
}

/// Benchmark concurrent sessions with upgrades
fn bench_concurrent_with_upgrades(c: &mut Criterion) {
    let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::default()));

    c.bench_function("e2e_concurrent_with_upgrades_4_threads", |b| {
        b.iter(|| {
            let mut handles = Vec::with_capacity(4);

            for _ in 0..4 {
                let pool_clone = Arc::clone(&pool);
                handles.push(thread::spawn(move || {
                    for _ in 0..10 {
                        // Start LIGHT, upgrade based on activity
                        if let Ok(light_id) = pool_clone.allocate_session(SessionTierType::Light) {
                            let engine = ReplayEngineCapsule::new();

                            // Simulate work that triggers upgrade
                            for j in 0..50 {
                                let _ = engine.take_snapshot(0x1000 + j * 4, 0x7fff_0000);
                            }

                            // Upgrade to MEDIUM (simulating threshold reached)
                            if let Ok(medium_id) = pool_clone.upgrade_session(light_id) {
                                // More work
                                for j in 50..100 {
                                    let _ = engine.take_snapshot(0x1000 + j * 4, 0x7fff_0000);
                                }
                                let _ = pool_clone.release_session(medium_id);
                            }
                        }
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });
}

// ============================================================================
// Session Upgrade Chain Benchmarks
// ============================================================================

/// Benchmark full upgrade chain LIGHT -> MEDIUM -> HEAVY
///
/// Target: <50ms for complete upgrade chain
fn bench_upgrade_chain_e2e(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("e2e_upgrade_chain_full", |b| {
        b.iter(|| {
            // Start with LIGHT
            let light_id = pool.allocate_session(SessionTierType::Light).unwrap();

            // Simulate activity that triggers upgrade
            let engine = ReplayEngineCapsule::new();
            for i in 0..48 {
                let _ = engine.take_snapshot(0x1000 + i * 4, 0x7fff_0000);
            }

            // Upgrade to MEDIUM
            let medium_id = pool.upgrade_session(light_id).unwrap();

            // More activity
            for i in 48..96 {
                let _ = engine.take_snapshot(0x1000 + i * 4, 0x7fff_0000);
            }

            // Upgrade to HEAVY
            let heavy_id = pool.upgrade_session(medium_id).unwrap();

            // Create memory replay for HEAVY tier
            let mut mem_replay = MemoryReplayCapsule::with_config(ReplayConfig::minimal());
            mem_replay.attach(12345).unwrap();

            let test_page = generate_test_page(12345);
            let memory_reader = |_: u64| -> Result<[u8; PAGE_SIZE], String> {
                Ok(test_page)
            };

            // Capture memory snapshots
            for _ in 0..5 {
                mem_replay.mark_page_dirty(0);
                let _ = mem_replay.capture_snapshot(&memory_reader);
            }

            // Release
            pool.release_session(heavy_id).unwrap();

            black_box((engine.get_stats(), mem_replay.get_stats()))
        })
    });
}

// ============================================================================
// Pool Utilization Stress Tests
// ============================================================================

/// Benchmark pool at high utilization
fn bench_high_utilization(c: &mut Criterion) {
    let config = PoolConfig {
        light_capacity: 100,
        medium_capacity: 50,
        heavy_capacity: 25,
        ..PoolConfig::default()
    };
    let pool = Arc::new(SessionPoolCapsule::new(config));

    // Pre-fill to 80% capacity
    let mut held_sessions: Vec<SessionId> = Vec::new();
    for _ in 0..80 {
        if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
            held_sessions.push(id);
        }
    }
    for _ in 0..40 {
        if let Ok(id) = pool.allocate_session(SessionTierType::Medium) {
            held_sessions.push(id);
        }
    }
    for _ in 0..20 {
        if let Ok(id) = pool.allocate_session(SessionTierType::Heavy) {
            held_sessions.push(id);
        }
    }

    c.bench_function("e2e_high_utilization_80_percent", |b| {
        b.iter(|| {
            // Try to allocate/release in remaining capacity
            for _ in 0..10 {
                if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
                    let engine = ReplayEngineCapsule::new();
                    let _ = engine.take_snapshot(0x1000, 0x7fff_0000);
                    let _ = pool.release_session(id);
                    black_box(engine.get_stats());
                }
            }
        })
    });

    // Cleanup
    for id in held_sessions {
        let _ = pool.release_session(id);
    }
}

// ============================================================================
// Real-World Workflow Simulations
// ============================================================================

/// Benchmark realistic debugging workflow: Find bug -> Analyze -> Fix
fn bench_realistic_debug_workflow(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("e2e_realistic_debug_workflow", |b| {
        b.iter(|| {
            // Phase 1: Quick inspection (LIGHT)
            let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
            simulate_ptrace_attach();
            let rip = black_box(0x7fff_1234_5678u64);

            // Decide we need more info, upgrade
            let medium_id = pool.upgrade_session(light_id).unwrap();

            // Phase 2: Step through code (MEDIUM)
            let engine = ReplayEngineCapsule::new();
            for i in 0..20 {
                let _ = engine.take_snapshot(rip + i * 4, 0x7fff_0000 - i * 8);
                simulate_ptrace_attach();
            }

            // Found the bug, need memory analysis
            let heavy_id = pool.upgrade_session(medium_id).unwrap();

            // Phase 3: Memory analysis (HEAVY)
            let mut mem_replay = MemoryReplayCapsule::with_config(ReplayConfig::minimal());
            mem_replay.attach(12345).unwrap();

            let test_page = generate_test_page(12345);
            let memory_reader = |_: u64| -> Result<[u8; PAGE_SIZE], String> {
                Ok(test_page)
            };

            mem_replay.mark_page_dirty(0x1000);
            let _ = mem_replay.capture_snapshot(&memory_reader);

            // Time-travel to find root cause
            engine.jump_to_snapshot(10).unwrap();
            let snapshot = engine.step_backward();

            // Done, release
            pool.release_session(heavy_id).unwrap();

            black_box((snapshot, mem_replay.get_stats()))
        })
    });
}

/// Benchmark crash analysis workflow
fn bench_crash_analysis_workflow(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("e2e_crash_analysis_workflow", |b| {
        b.iter(|| {
            // Crash happened, start with HEAVY for full replay
            let heavy_id = pool.allocate_session(SessionTierType::Heavy).unwrap();

            // Initialize replay engines
            let engine = ReplayEngineCapsule::new();
            let mut mem_replay = MemoryReplayCapsule::with_config(ReplayConfig::compliance());
            mem_replay.attach(12345).unwrap();

            let test_page = generate_test_page(12345);
            let memory_reader = |_: u64| -> Result<[u8; PAGE_SIZE], String> {
                Ok(test_page)
            };

            // Capture crash state
            engine.take_snapshot(0xDEAD_BEEF, 0x0).unwrap();
            for i in 0..10 {
                mem_replay.mark_page_dirty((i * PAGE_SIZE) as u64);
            }
            let _ = mem_replay.capture_snapshot(&memory_reader);

            // Verify integrity (Q34 compliance)
            let integrity_ok = mem_replay.verify_integrity();

            // Get stats for report
            let stats = mem_replay.get_stats();

            pool.release_session(heavy_id).unwrap();

            black_box((integrity_ok, stats))
        })
    });
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    quick_debug_benches,
    bench_quick_debug_session,
    bench_quick_debug_burst,
);

criterion_group!(
    step_debug_benches,
    bench_step_debug_session,
    bench_step_debug_with_breakpoints,
);

criterion_group!(
    full_replay_benches,
    bench_full_replay_session,
    bench_memory_intensive_session,
);

criterion_group!(
    concurrent_benches,
    bench_concurrent_100_sessions,
    bench_concurrent_with_upgrades,
);

criterion_group!(
    upgrade_benches,
    bench_upgrade_chain_e2e,
);

criterion_group!(
    stress_benches,
    bench_high_utilization,
);

criterion_group!(
    workflow_benches,
    bench_realistic_debug_workflow,
    bench_crash_analysis_workflow,
);

criterion_main!(
    quick_debug_benches,
    step_debug_benches,
    full_replay_benches,
    concurrent_benches,
    upgrade_benches,
    stress_benches,
    workflow_benches,
);
