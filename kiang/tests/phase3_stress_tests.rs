//! Phase 3 Stress Tests - Concurrent Access
//!
//! Following T42 test framework, this file validates:
//! - 100-thread concurrent memory checks
//! - Allocation/deallocation under load
//! - Pipeline throughput with memory pressure
//! - No data races or deadlocks

use kiang::capsules::*;
use kiang::circuit_breaker::*;
use kiang::command::*;
use kiang::context::*;
use kiang::drm_interface::*;
use kiang::fence::*;
use kiang::guc_ctb::*;
use kiang::memory::*;
use kiang::submission_pipeline::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// Memory Capsule Stress Tests (6 tests)
// ============================================================================

#[test]
fn test_stress_memory_capsule_100_concurrent_readers() {
    let capsule = Arc::new(MemoryCapsule::new(8192));

    // Publish initial state
    let state = MemoryState {
        total_vram_mb: 8192,
        used_vram_mb: 4096,
        free_vram_mb: 4096,
        allocation_count: 1000,
        fragment_count: 50,
        largest_free_mb: 2048,
        allocation_gen: 100,
        pressure_pct: 50,
    };
    capsule.publish(state);

    let mut handles = vec![];

    // Spawn 100 reader threads
    for tid in 0..100 {
        let cap = capsule.clone();
        let handle = thread::spawn(move || {
            let mut successful_reads = 0;
            for _ in 0..10000 {
                if let Some(snapshot) = cap.read() {
                    assert!(snapshot.is_valid(), "Thread {} got invalid snapshot", tid);
                    assert_eq!(snapshot.state.total_vram_mb, 8192);
                    assert_eq!(snapshot.state.used_vram_mb, 4096);
                    successful_reads += 1;
                }
            }
            successful_reads
        });
        handles.push(handle);
    }

    // All threads should complete successfully
    for (i, handle) in handles.into_iter().enumerate() {
        let reads = handle.join().expect(&format!("Thread {} panicked", i));
        assert!(
            reads > 9000,
            "Thread {} had too few successful reads: {}",
            i,
            reads
        );
    }
}

#[test]
fn test_stress_memory_capsule_writer_with_readers() {
    let capsule = Arc::new(MemoryCapsule::new(16384));

    // Writer thread
    let writer_cap = capsule.clone();
    let writer = thread::spawn(move || {
        for i in 0..1000 {
            let used_mb = (i * 10) % 16384;
            let free_mb = 16384 - used_mb;

            let state = MemoryState {
                total_vram_mb: 16384,
                used_vram_mb: used_mb,
                free_vram_mb: free_mb,
                allocation_count: i as u32,
                fragment_count: (i % 100) as u32,
                largest_free_mb: free_mb,
                allocation_gen: i,
                pressure_pct: ((used_mb as u32 * 100) / 16384) as u8,
            };

            writer_cap.publish(state);
            thread::sleep(Duration::from_micros(10));
        }
    });

    // Reader threads
    let mut readers = vec![];
    for _ in 0..50 {
        let reader_cap = capsule.clone();
        let handle = thread::spawn(move || {
            let mut valid_reads = 0;
            for _ in 0..5000 {
                if let Some(snapshot) = reader_cap.read() {
                    if snapshot.is_valid() {
                        valid_reads += 1;
                        // Verify invariant
                        assert!(
                            snapshot.state.used_vram_mb as u32 + snapshot.state.free_vram_mb as u32
                                <= snapshot.state.total_vram_mb as u32
                        );
                    }
                }
            }
            valid_reads
        });
        readers.push(handle);
    }

    writer.join().unwrap();

    // All readers should get mostly valid reads
    for reader in readers {
        let valid = reader.join().unwrap();
        assert!(valid > 4000, "Reader had too few valid reads: {}", valid);
    }
}

#[test]
fn test_stress_allocator_concurrent_allocations() {
    let allocator = Arc::new(GpuMemoryAllocator::new(16 * 1024 * 1024 * 1024)); // 16GB

    let mut handles = vec![];

    // Spawn 100 threads allocating concurrently
    for tid in 0..100 {
        let alloc = allocator.clone();
        let handle = thread::spawn(move || {
            let mut success_count = 0;
            for i in 0..100 {
                let size = ((tid * 100 + i) % 100 + 1) * 1024 * 1024; // 1-100MB
                if let Some(_) = alloc.allocate(size, MemoryDomain::Vram) {
                    success_count += 1;
                }
            }
            success_count
        });
        handles.push(handle);
    }

    // Wait for all allocations
    let mut total_successes = 0;
    for handle in handles {
        total_successes += handle.join().unwrap();
    }

    // At least some allocations should succeed
    assert!(total_successes > 0, "No allocations succeeded");

    // Total allocated should not exceed capacity
    assert!(allocator.allocated_bytes() <= 16 * 1024 * 1024 * 1024);
}

#[test]
fn test_stress_allocator_alloc_free_cycle() {
    let allocator = Arc::new(GpuMemoryAllocator::new(1024 * 1024 * 1024)); // 1GB

    let mut handles = vec![];

    // Spawn 50 threads doing alloc/free cycles
    for _ in 0..50 {
        let alloc = allocator.clone();
        let handle = thread::spawn(move || {
            for _ in 0..200 {
                let size = 10 * 1024 * 1024; // 10MB
                if let Some(allocation) = alloc.allocate(size, MemoryDomain::Vram) {
                    // Immediately free
                    alloc.free(allocation.size);
                }
            }
        });
        handles.push(handle);
    }

    // All threads should complete
    for handle in handles {
        handle.join().unwrap();
    }

    // After all alloc/free cycles, allocation should be low
    assert!(
        allocator.allocated_bytes() < 100 * 1024 * 1024,
        "Too much memory still allocated: {}",
        allocator.allocated_bytes()
    );
}

#[test]
fn test_stress_memory_capsule_can_allocate_hot_path() {
    let capsule = Arc::new(MemoryCapsule::new(8192));

    let state = MemoryState {
        total_vram_mb: 8192,
        used_vram_mb: 4096,
        free_vram_mb: 4096,
        allocation_count: 100,
        fragment_count: 10,
        largest_free_mb: 4096,
        allocation_gen: 50,
        pressure_pct: 50,
    };
    capsule.publish(state);

    let mut handles = vec![];

    // Spawn 100 threads hammering can_allocate (hot path)
    for _ in 0..100 {
        let cap = capsule.clone();
        let handle = thread::spawn(move || {
            let mut check_count = 0;
            for i in 0..100000 {
                let request_mb = (i % 8192) as u16;
                let can_alloc = cap.can_allocate(request_mb);

                // Verify correctness
                if request_mb <= 4096 {
                    assert!(can_alloc, "Should allow {} MB allocation", request_mb);
                } else {
                    assert!(!can_alloc, "Should reject {} MB allocation", request_mb);
                }
                check_count += 1;
            }
            check_count
        });
        handles.push(handle);
    }

    // All threads should complete all checks
    for handle in handles {
        let count = handle.join().unwrap();
        assert_eq!(count, 100000);
    }
}

#[test]
fn test_stress_memory_pressure_updates() {
    let allocator = Arc::new(GpuMemoryAllocator::new(2 * 1024 * 1024 * 1024)); // 2GB

    let mut handles = vec![];

    // Allocator threads
    for _ in 0..20 {
        let alloc = allocator.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                if let Some(allocation) = alloc.allocate(10 * 1024 * 1024, MemoryDomain::Vram) {
                    thread::sleep(Duration::from_micros(100));
                    alloc.free(allocation.size);
                }
            }
        });
        handles.push(handle);
    }

    // Monitor thread checking pressure
    let monitor_alloc = allocator.clone();
    let monitor = thread::spawn(move || {
        let mut max_pressure = 0u8;
        for _ in 0..2000 {
            let pressure = monitor_alloc.capsule().pressure_pct();
            max_pressure = max_pressure.max(pressure);
            thread::sleep(Duration::from_micros(100));
        }
        max_pressure
    });

    // Wait for allocators
    for handle in handles {
        handle.join().unwrap();
    }

    // Monitor should have seen some pressure
    let max_pressure = monitor.join().unwrap();
    assert!(max_pressure > 0, "No memory pressure observed");
}

// ============================================================================
// Command Capsule Stress Tests (5 tests)
// ============================================================================

#[test]
fn test_stress_command_capsule_100_readers() {
    let cmd = Arc::new(CommandCapsule::with_state(
        1000,
        4096,
        CommandPriority::High,
    ));

    let mut handles = vec![];

    // Spawn 100 reader threads
    for _ in 0..100 {
        let cmd_clone = cmd.clone();
        let handle = thread::spawn(move || {
            let mut read_count = 0;
            for _ in 0..10000 {
                if let Some(snapshot) = cmd_clone.read() {
                    assert_eq!(snapshot.buffer_id, 1000);
                    assert_eq!(snapshot.size_kb, 4096);
                    read_count += 1;
                }
            }
            read_count
        });
        handles.push(handle);
    }

    for handle in handles {
        let count = handle.join().unwrap();
        assert!(count > 9000, "Too few successful reads: {}", count);
    }
}

#[test]
fn test_stress_command_state_transitions_concurrent() {
    let cmd = Arc::new(CommandCapsule::with_state(
        2000,
        1024,
        CommandPriority::Normal,
    ));

    // Writer thread doing state transitions
    let writer_cmd = cmd.clone();
    let writer = thread::spawn(move || {
        for _ in 0..1000 {
            writer_cmd.mark_submitted();
            writer_cmd.mark_executing();
            writer_cmd.mark_completed();
            writer_cmd.reset(2000, 1024, CommandPriority::Normal);
            thread::sleep(Duration::from_micros(10));
        }
    });

    // Reader threads checking state
    let mut readers = vec![];
    for _ in 0..50 {
        let reader_cmd = cmd.clone();
        let handle = thread::spawn(move || {
            let mut valid_reads = 0;
            for _ in 0..5000 {
                if let Some(_) = reader_cmd.read() {
                    valid_reads += 1;
                }
            }
            valid_reads
        });
        readers.push(handle);
    }

    writer.join().unwrap();

    for reader in readers {
        let count = reader.join().unwrap();
        assert!(count > 4000, "Too few valid reads: {}", count);
    }
}

#[test]
fn test_stress_command_readiness_checks() {
    let cmd = Arc::new(CommandCapsule::with_state(
        3000,
        2048,
        CommandPriority::High,
    ));

    let mut handles = vec![];

    // Spawn 100 threads checking readiness
    for _ in 0..100 {
        let cmd_clone = cmd.clone();
        let handle = thread::spawn(move || {
            let mut ready_count = 0;
            for _ in 0..100000 {
                if cmd_clone.is_ready() {
                    ready_count += 1;
                }
            }
            ready_count
        });
        handles.push(handle);
    }

    for handle in handles {
        let count = handle.join().unwrap();
        // Initially all should see ready state
        assert!(count > 0, "No ready checks succeeded");
    }
}

#[test]
fn test_stress_command_multiple_capsules() {
    let cmds: Vec<_> = (0..100)
        .map(|i| Arc::new(CommandCapsule::with_state(i, 512, CommandPriority::Normal)))
        .collect();

    let mut handles = vec![];

    // Each capsule gets a reader thread
    for (i, cmd) in cmds.iter().enumerate() {
        let cmd_clone = cmd.clone();
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                if let Some(snapshot) = cmd_clone.read() {
                    assert_eq!(snapshot.buffer_id, i as u32);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_stress_command_reset_cycle() {
    let cmd = Arc::new(CommandCapsule::with_state(4000, 1024, CommandPriority::Low));

    let mut handles = vec![];

    // Writer doing reset cycles
    let writer_cmd = cmd.clone();
    let writer = thread::spawn(move || {
        for i in 0..500 {
            writer_cmd.reset(4000, (i % 4096) as u16 + 1, CommandPriority::Normal);
            thread::sleep(Duration::from_micros(20));
        }
    });

    // Readers watching resets
    for _ in 0..20 {
        let reader_cmd = cmd.clone();
        let handle = thread::spawn(move || {
            for _ in 0..2500 {
                let _ = reader_cmd.read();
            }
        });
        handles.push(handle);
    }

    writer.join().unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
}

// ============================================================================
// Pipeline Stress Tests (5 tests)
// ============================================================================

#[test]
fn test_stress_pipeline_concurrent_submissions() {
    let gpu_state = Arc::new(GpuStateCapsule::new());
    let breaker = Arc::new(GpuCircuitBreaker::new());
    let context = Arc::new(ContextCapsule::new());
    let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
    let fence = Arc::new(FenceCapsule::new(0));

    let pipeline = Arc::new(SubmissionPipeline::new(
        gpu_state.clone(),
        breaker,
        context.clone(),
        guc_ctb.clone(),
        fence.clone(),
    ));

    // Set up for success
    gpu_state.publish(GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    });

    context.publish(crate::context::ContextUpdate {
        context_id: 1,
        priority: 0,
        state: crate::context::ContextState::Ready,
        last_fence: 0,
        batch_count: 0,
        error_count: 0,
        timestamp_us: 0,
        resource_gen: 0,
        mem_usage_mb: 0,
        submission_count: 0,
    });

    guc_ctb.publish(crate::guc_ctb::GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 16 * 1024,
        pending_count: 0,
    });

    fence.signal(1000, 1000);

    let mut handles = vec![];

    // Spawn 50 threads submitting commands
    for tid in 0..50 {
        let pipe = pipeline.clone();
        let handle = thread::spawn(move || {
            let mut accepted = 0;
            for i in 0..200 {
                let result = pipe.submit_command(1024, (tid * 200 + i) as u32, 500);
                if matches!(result, SubmissionResult::Accepted { .. }) {
                    accepted += 1;
                }
            }
            accepted
        });
        handles.push(handle);
    }

    // Count total accepted
    let mut total_accepted = 0;
    for handle in handles {
        total_accepted += handle.join().unwrap();
    }

    assert!(total_accepted > 0, "No commands accepted");
    assert_eq!(pipeline.total_submissions(), total_accepted as u64);
}

#[test]
fn test_stress_pipeline_with_rejections() {
    let gpu_state = Arc::new(GpuStateCapsule::new());
    let breaker = Arc::new(GpuCircuitBreaker::new());
    let context = Arc::new(ContextCapsule::new());
    let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
    let fence = Arc::new(FenceCapsule::new(0));

    let pipeline = Arc::new(SubmissionPipeline::new(
        gpu_state.clone(),
        breaker.clone(),
        context,
        guc_ctb,
        fence,
    ));

    // Publish invalid GPU state (too hot)
    gpu_state.publish(GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 100, // Too hot!
        utilization: 50,
        valid: true,
    });

    let mut handles = vec![];

    // Spawn threads attempting submissions (all should be rejected)
    for tid in 0..100 {
        let pipe = pipeline.clone();
        let handle = thread::spawn(move || {
            let mut rejected = 0;
            for i in 0..100 {
                let result = pipe.submit_command(1024, (tid * 100 + i) as u32, 0);
                if matches!(result, SubmissionResult::RejectedGpuState) {
                    rejected += 1;
                }
            }
            rejected
        });
        handles.push(handle);
    }

    // All should be rejected
    for handle in handles {
        let count = handle.join().unwrap();
        assert_eq!(count, 100, "All submissions should be rejected");
    }

    assert_eq!(pipeline.total_rejections(), 10000);
}

#[test]
fn test_stress_pipeline_fast_path() {
    let gpu_state = Arc::new(GpuStateCapsule::new());
    let breaker = Arc::new(GpuCircuitBreaker::new());
    let context = Arc::new(ContextCapsule::new());
    let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
    let fence = Arc::new(FenceCapsule::new(0));

    let pipeline = Arc::new(SubmissionPipeline::new(
        gpu_state,
        breaker,
        context.clone(),
        guc_ctb,
        fence,
    ));

    context.publish(crate::context::ContextUpdate {
        context_id: 1,
        priority: 0,
        state: crate::context::ContextState::Ready,
        last_fence: 0,
        batch_count: 0,
        error_count: 0,
        timestamp_us: 0,
        resource_gen: 0,
        mem_usage_mb: 0,
        submission_count: 0,
    });

    let mut handles = vec![];

    // Hammer fast path with 100 threads
    for _ in 0..100 {
        let pipe = pipeline.clone();
        let handle = thread::spawn(move || {
            let mut check_count = 0;
            for _ in 0..100000 {
                let _ = pipe.can_submit_fast();
                check_count += 1;
            }
            check_count
        });
        handles.push(handle);
    }

    for handle in handles {
        let count = handle.join().unwrap();
        assert_eq!(count, 100000);
    }
}

#[test]
fn test_stress_integrated_memory_and_pipeline() {
    let allocator = Arc::new(GpuMemoryAllocator::new(1024 * 1024 * 1024)); // 1GB

    let gpu_state = Arc::new(GpuStateCapsule::new());
    let breaker = Arc::new(GpuCircuitBreaker::new());
    let context = Arc::new(ContextCapsule::new());
    let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
    let fence = Arc::new(FenceCapsule::new(0));

    let pipeline = Arc::new(SubmissionPipeline::new(
        gpu_state.clone(),
        breaker,
        context.clone(),
        guc_ctb.clone(),
        fence.clone(),
    ));

    // Set up pipeline for success
    gpu_state.publish(GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    });

    context.publish(crate::context::ContextUpdate {
        context_id: 1,
        priority: 0,
        state: crate::context::ContextState::Ready,
        last_fence: 0,
        batch_count: 0,
        error_count: 0,
        timestamp_us: 0,
        resource_gen: 0,
        mem_usage_mb: 0,
        submission_count: 0,
    });

    guc_ctb.publish(crate::guc_ctb::GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 16 * 1024,
        pending_count: 0,
    });

    fence.signal(1000, 1000);

    let mut handles = vec![];

    // Threads doing allocations and submissions
    for tid in 0..50 {
        let alloc = allocator.clone();
        let pipe = pipeline.clone();

        let handle = thread::spawn(move || {
            for i in 0..100 {
                // Try to allocate
                if let Some(_allocation) = alloc.allocate(10 * 1024 * 1024, MemoryDomain::Vram) {
                    // Try to submit command
                    let _ = pipe.submit_command(1024, (tid * 100 + i) as u32, 500);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify system state
    assert!(allocator.allocated_bytes() <= 1024 * 1024 * 1024);
    assert!(pipeline.total_submissions() > 0 || pipeline.total_rejections() > 0);
}

#[test]
fn test_stress_drm_concurrent_gem_creation() {
    use std::os::unix::io::FromRawFd;

    let device = Arc::new(DrmDevice {
        file: unsafe { std::fs::File::from_raw_fd(0) },
        card_path: "/dev/dri/card0".to_string(),
        generation: Arc::new(std::sync::atomic::AtomicU64::new(1)),
    });

    let mut handles = vec![];

    // Spawn 20 threads creating GEM objects
    for _ in 0..20 {
        let dev = device.clone();
        let handle = thread::spawn(move || {
            let mut gems = vec![];
            for i in 0..50 {
                let size = ((i + 1) * 4096) as u64;
                if let Ok(gem) = GemObject::create(&dev, size) {
                    gems.push(gem);
                }
            }
            gems.len()
        });
        handles.push(handle);
    }

    // Count total GEM objects created
    let mut total_gems = 0;
    for handle in handles {
        total_gems += handle.join().unwrap();
    }

    assert_eq!(total_gems, 20 * 50, "All GEM objects should be created");

    // Verify generation advanced
    assert!(device.generation() > 1);
}
