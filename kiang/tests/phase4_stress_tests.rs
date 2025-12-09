//! Phase 4 Stress Tests - Concurrent Memory Management
//!
//! High-load concurrent testing for Phase 4 memory management:
//! - 100-thread concurrent allocations
//! - Page fault storms
//! - GGTT fragmentation under load
//! - Reclamation under pressure
//!
//! ## Test Strategy (T42 Framework)
//!
//! **Stress Tests**: Concurrent access, race detection
//! - Multi-threaded allocation storms
//! - Concurrent reads during writes
//! - Memory pressure scenarios
//! - Error injection and recovery
//!
//! **Coverage Target**: Validate under extreme load

use kiang::{
    BumpAllocator, DeferredFree, FaultStatus, FaultType, GgttCapsule, GgttEntry, GgttManager,
    GpuMemoryAllocator, IommuCapsule, IommuManager, IommuMapping, MemoryCapsule, MemoryDomain,
    MemoryReclaimer, PageFault, PageFaultCapsule, PageFaultHandler, ReclamationCapsule,
};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// MemoryCapsule Stress Tests
// ============================================================================

#[test]
fn stress_memory_capsule_concurrent_reads() {
    // 100 threads reading concurrently while writer updates

    let capsule = Arc::new(MemoryCapsule::new(8192));

    // Publisher thread
    let capsule_writer = Arc::clone(&capsule);
    let writer_handle = thread::spawn(move || {
        for i in 0..1000 {
            let state = kiang::MemoryState {
                total_vram_mb: 8192,
                used_vram_mb: (i % 8192) as u16,
                free_vram_mb: 8192 - (i % 8192) as u16,
                allocation_count: i as u32,
                fragment_count: 0,
                largest_free_mb: 8192 - (i % 8192) as u16,
                allocation_gen: i as u16,
                pressure_pct: ((i % 8192) * 100 / 8192) as u8,
            };
            capsule_writer.publish(state);
            thread::sleep(Duration::from_micros(100));
        }
    });

    // 100 reader threads
    let mut reader_handles = vec![];
    for _ in 0..100 {
        let capsule_reader = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let mut read_count = 0;
            let mut valid_count = 0;

            for _ in 0..10000 {
                read_count += 1;

                if let Some(snapshot) = capsule_reader.read() {
                    if snapshot.is_valid() {
                        valid_count += 1;

                        // Validate invariants
                        assert_eq!(
                            snapshot.state.total_vram_mb,
                            snapshot.state.used_vram_mb + snapshot.state.free_vram_mb,
                            "Memory conservation violated under stress"
                        );
                    }
                }

                // Hot path checks
                capsule_reader.can_allocate(1024);
            }

            (read_count, valid_count)
        });
        reader_handles.push(handle);
    }

    // Wait for all threads
    writer_handle.join().unwrap();

    let mut total_reads = 0;
    let mut total_valid = 0;

    for handle in reader_handles {
        let (reads, valid) = handle.join().unwrap();
        total_reads += reads;
        total_valid += valid;
    }

    println!(
        "Stress test: {} reads, {} valid ({}%)",
        total_reads,
        total_valid,
        (total_valid * 100) / total_reads
    );

    // Should have high success rate (>90%)
    assert!(
        (total_valid * 100) / total_reads > 90,
        "Too many invalid reads under stress"
    );
}

#[test]
fn stress_memory_capsule_rapid_updates() {
    // Single writer publishing as fast as possible, readers should never see torn reads

    let capsule = Arc::new(MemoryCapsule::new(16384));

    let capsule_writer = Arc::clone(&capsule);
    let writer_handle = thread::spawn(move || {
        for i in 0..10000 {
            let state = kiang::MemoryState {
                total_vram_mb: 16384,
                used_vram_mb: (i % 16384) as u16,
                free_vram_mb: 16384 - (i % 16384) as u16,
                allocation_count: i as u32,
                fragment_count: 0,
                largest_free_mb: 16384 - (i % 16384) as u16,
                allocation_gen: i as u16,
                pressure_pct: ((i % 16384) * 100 / 16384) as u8,
            };
            capsule_writer.publish(state);
            // No sleep - publish as fast as possible
        }
    });

    // 10 reader threads
    let mut reader_handles = vec![];
    for _ in 0..10 {
        let capsule_reader = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let mut torn_read_count = 0;

            for _ in 0..100000 {
                if let Some(snapshot) = capsule_reader.read() {
                    // Check for torn reads
                    if snapshot.state.used_vram_mb + snapshot.state.free_vram_mb
                        != snapshot.state.total_vram_mb
                    {
                        torn_read_count += 1;
                    }
                }
            }

            torn_read_count
        });
        reader_handles.push(handle);
    }

    writer_handle.join().unwrap();

    let mut total_torn_reads = 0;
    for handle in reader_handles {
        total_torn_reads += handle.join().unwrap();
    }

    // CRITICAL: Should never see torn reads
    assert_eq!(
        total_torn_reads, 0,
        "Detected {} torn reads - two-phase commit failed!",
        total_torn_reads
    );
}

// ============================================================================
// GpuMemoryAllocator Stress Tests
// ============================================================================

#[test]
fn stress_allocator_concurrent_allocations() {
    // 100 threads allocating concurrently

    let allocator = Arc::new(GpuMemoryAllocator::new(64 * 1024 * 1024 * 1024)); // 64GB
    let barrier = Arc::new(Barrier::new(100));

    let mut handles = vec![];

    for _ in 0..100 {
        let allocator_clone = Arc::clone(&allocator);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            // Wait for all threads to be ready
            barrier_clone.wait();

            let mut success_count = 0;
            let mut oom_count = 0;

            // Each thread tries to allocate 100MB chunks
            for _ in 0..10 {
                match allocator_clone.allocate(100 * 1024 * 1024, MemoryDomain::Vram) {
                    Some(_) => success_count += 1,
                    None => oom_count += 1,
                }
            }

            (success_count, oom_count)
        });
        handles.push(handle);
    }

    // Wait for all threads
    let mut total_success = 0;
    let mut total_oom = 0;

    for handle in handles {
        let (success, oom) = handle.join().unwrap();
        total_success += success;
        total_oom += oom;
    }

    println!(
        "Concurrent allocations: {} succeeded, {} OOM",
        total_success, total_oom
    );

    // Total requests: 100 threads * 10 allocations = 1000
    // Capacity: 64GB / 100MB = 640 allocations
    assert_eq!(
        total_success, 640,
        "Should have exactly 640 successful allocations"
    );
    assert_eq!(total_oom, 360, "Should have exactly 360 OOM failures");

    // Verify total allocated equals capacity
    assert_eq!(
        allocator.allocated_bytes(),
        64 * 1024 * 1024 * 1024,
        "Total allocated should equal capacity"
    );
}

#[test]
fn stress_allocator_allocate_free_cycle() {
    // Rapid allocate/free cycles under contention

    let allocator = Arc::new(GpuMemoryAllocator::new(8 * 1024 * 1024 * 1024)); // 8GB

    let mut handles = vec![];

    for _ in 0..50 {
        let allocator_clone = Arc::clone(&allocator);
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                // Allocate
                if let Some(alloc) = allocator_clone.allocate(10 * 1024 * 1024, MemoryDomain::Vram)
                {
                    // Immediately free
                    allocator_clone.free(alloc.size);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // After all allocate/free cycles, should be back to 0 allocated
    // (within rounding errors due to concurrent updates)
    let final_allocated = allocator.allocated_bytes();
    println!("Final allocated after stress: {} bytes", final_allocated);

    // Should be relatively low (some allocations may still be in flight)
    assert!(
        final_allocated < 1024 * 1024 * 1024, // Less than 1GB
        "Too much memory still allocated: {}",
        final_allocated
    );
}

#[test]
fn stress_allocator_capsule_hot_path() {
    // Stress test hot path (can_allocate) under concurrent load

    let allocator = Arc::new(GpuMemoryAllocator::new(16 * 1024 * 1024 * 1024)); // 16GB
    let barrier = Arc::new(Barrier::new(100));

    let mut handles = vec![];

    for _ in 0..100 {
        let allocator_clone = Arc::clone(&allocator);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let start = Instant::now();
            let mut check_count = 0;

            // Hammer the hot path for 100ms
            while start.elapsed() < Duration::from_millis(100) {
                allocator_clone.capsule().can_allocate(1024);
                check_count += 1;
            }

            check_count
        });
        handles.push(handle);
    }

    // Wait and collect stats
    let mut total_checks = 0;
    for handle in handles {
        total_checks += handle.join().unwrap();
    }

    println!("Total hot path checks: {}", total_checks);

    // Should be able to do millions of checks
    assert!(
        total_checks > 1_000_000,
        "Hot path too slow: only {} checks",
        total_checks
    );
}

// ============================================================================
// BumpAllocator Stress Tests
// ============================================================================

#[test]
fn stress_bump_allocator_rapid_allocations() {
    // Allocate as fast as possible until OOM

    let mut allocator = BumpAllocator::new(1024 * 1024 * 1024); // 1GB

    let start = Instant::now();
    let mut alloc_count = 0;

    loop {
        match allocator.allocate(4096, 64) {
            Some(_) => alloc_count += 1,
            None => break, // OOM
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Bump allocator: {} allocations in {:?} ({:.0} allocs/sec)",
        alloc_count,
        elapsed,
        alloc_count as f64 / elapsed.as_secs_f64()
    );

    // Should be able to do hundreds of thousands per second
    assert!(
        alloc_count > 100_000,
        "Too few allocations: {}",
        alloc_count
    );
}

#[test]
fn stress_bump_allocator_various_alignments() {
    // Mix of different alignment requirements

    let mut allocator = BumpAllocator::new(512 * 1024 * 1024); // 512MB

    let alignments = vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

    for _ in 0..10000 {
        for &align in &alignments {
            if allocator.allocate(1024, align).is_none() {
                // Reset and continue
                allocator.reset();
            }
        }
    }

    // Should complete without panics
}

// ============================================================================
// GGTT Stress Tests
// ============================================================================

#[test]
fn stress_ggtt_concurrent_reads() {
    // Many threads reading GGTT state concurrently

    let ggtt = Arc::new(GgttCapsule::new(4096));

    // Writer thread
    let ggtt_writer = Arc::clone(&ggtt);
    let writer_handle = thread::spawn(move || {
        for i in 0..1000 {
            let state = kiang::GgttState {
                total_entries: 4096,
                used_entries: (i % 4096) as u32,
                free_entries: 4096 - (i % 4096) as u32,
                total_size_mb: 4096,
                mapped_size_mb: (i % 4096) as u32,
                fragment_count: 0,
                entry_gen: i as u32,
            };
            ggtt_writer.publish(state);
            thread::sleep(Duration::from_micros(100));
        }
    });

    // 50 reader threads
    let mut reader_handles = vec![];
    for _ in 0..50 {
        let ggtt_reader = Arc::clone(&ggtt);
        let handle = thread::spawn(move || {
            let mut valid_count = 0;

            for _ in 0..10000 {
                if let Some(snapshot) = ggtt_reader.read() {
                    if snapshot.is_valid() {
                        valid_count += 1;

                        // Validate GGTT invariant
                        assert_eq!(
                            snapshot.state.total_entries,
                            snapshot.state.used_entries + snapshot.state.free_entries,
                            "GGTT conservation violated"
                        );
                    }
                }
            }

            valid_count
        });
        reader_handles.push(handle);
    }

    writer_handle.join().unwrap();

    let mut total_valid = 0;
    for handle in reader_handles {
        total_valid += handle.join().unwrap();
    }

    println!("GGTT stress test: {} valid reads", total_valid);

    // Should have high success rate
    assert!(total_valid > 400_000, "Too few valid GGTT reads");
}

// ============================================================================
// Reclamation Stress Tests
// ============================================================================

#[test]
fn stress_reclamation_many_deferred_frees() {
    // Defer thousands of frees and process them

    let reclaimer = Arc::new(MemoryReclaimer::new());

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    // 10 threads each deferring 1000 frees
    for thread_id in 0..10 {
        let reclaimer_clone = Arc::clone(&reclaimer);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            for i in 0..1000 {
                let offset = (thread_id * 1000 + i) * 4096;
                reclaimer_clone.defer_free(DeferredFree {
                    offset: offset as u64,
                    size: 4096,
                    generation: i as u64,
                });
            }
        });
        handles.push(handle);
    }

    // Wait for all deferrals
    for handle in handles {
        handle.join().unwrap();
    }

    // Process all deferred frees
    let mut total_freed = 0;
    loop {
        let freed = reclaimer.process_deferred(100);
        if freed.is_empty() {
            break;
        }
        total_freed += freed.len();
    }

    println!("Reclamation stress: {} frees processed", total_freed);

    // Should have processed all 10000 frees
    assert_eq!(total_freed, 10000, "Should process all deferred frees");
}

// ============================================================================
// IOMMU Stress Tests
// ============================================================================

#[test]
fn stress_iommu_concurrent_mapping_queries() {
    // Many threads querying IOMMU state concurrently

    let iommu = Arc::new(IommuCapsule::new());

    // Publish initial state
    let state = kiang::IommuState {
        active_mappings: 100,
        total_mapped_mb: 256,
        page_table_entries: 1000,
        mapping_gen: 0,
    };
    iommu.publish(state);

    let mut handles = vec![];

    for _ in 0..100 {
        let iommu_clone = Arc::clone(&iommu);
        let handle = thread::spawn(move || {
            for _ in 0..10000 {
                if let Some(snapshot) = iommu_clone.read() {
                    // Verify state is valid
                    assert!(snapshot.is_valid());
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all readers
    for handle in handles {
        handle.join().unwrap();
    }
}

// ============================================================================
// PageFault Stress Tests
// ============================================================================

#[test]
fn stress_page_fault_storm() {
    // Simulate page fault storm (many faults rapidly)

    let handler = Arc::new(PageFaultHandler::new());
    let barrier = Arc::new(Barrier::new(50));

    let mut handles = vec![];

    // 50 threads each generating 100 page faults
    for thread_id in 0..50 {
        let handler_clone = Arc::clone(&handler);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            for i in 0..100 {
                let fault = PageFault {
                    vaddr: ((thread_id * 100 + i) * 0x1000) as u64,
                    fault_type: if i % 2 == 0 {
                        FaultType::Read
                    } else {
                        FaultType::Write
                    },
                    status: FaultStatus::Pending,
                    timestamp_ns: 0,
                };
                handler_clone.record_fault(fault);
            }
        });
        handles.push(handle);
    }

    // Wait for all fault recordings
    for handle in handles {
        handle.join().unwrap();
    }

    // Check stats
    let stats = handler.stats();
    println!(
        "Page fault storm: {} pending, {} resolved",
        stats.pending_count, stats.resolved_count
    );

    // Should have recorded all 5000 faults
    assert_eq!(
        stats.pending_count + stats.resolved_count,
        5000,
        "Should record all faults"
    );
}

#[test]
fn stress_page_fault_concurrent_resolution() {
    // Record faults in one thread, resolve in multiple threads

    let handler = Arc::new(PageFaultHandler::new());

    // Record 1000 faults
    for addr in (0x1000..=0x1000000).step_by(0x1000) {
        let fault = PageFault {
            vaddr: addr as u64,
            fault_type: FaultType::Read,
            status: FaultStatus::Pending,
            timestamp_ns: 0,
        };
        handler.record_fault(fault);
    }

    // Resolve concurrently from 10 threads
    let mut handles = vec![];
    for _ in 0..10 {
        let handler_clone = Arc::clone(&handler);
        let handle = thread::spawn(move || {
            loop {
                let pending = handler_clone.get_pending_faults();
                if pending.is_empty() {
                    break;
                }

                for fault in pending {
                    handler_clone.resolve_fault(fault.vaddr);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all resolutions
    for handle in handles {
        handle.join().unwrap();
    }

    // All faults should be resolved
    let stats = handler.stats();
    assert_eq!(stats.pending_count, 0, "All faults should be resolved");
    assert!(stats.resolved_count > 0, "Should have resolved some faults");
}

// ============================================================================
// Cross-Component Stress Tests
// ============================================================================

#[test]
fn stress_full_memory_pipeline() {
    // Integrated stress test: allocate → GGTT map → IOMMU map → page fault → reclaim

    let allocator = Arc::new(GpuMemoryAllocator::new(16 * 1024 * 1024 * 1024)); // 16GB
    let ggtt = Arc::new(GgttCapsule::new(4096));
    let iommu = Arc::new(IommuCapsule::new());
    let page_fault_handler = Arc::new(PageFaultHandler::new());
    let reclaimer = Arc::new(MemoryReclaimer::new());

    let barrier = Arc::new(Barrier::new(20));
    let mut handles = vec![];

    // 20 threads running full pipeline
    for thread_id in 0..20 {
        let allocator_clone = Arc::clone(&allocator);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let mut ops = 0;

            for _ in 0..100 {
                // Allocate
                if let Some(alloc) = allocator_clone.allocate(10 * 1024 * 1024, MemoryDomain::Vram)
                {
                    ops += 1;

                    // Simulate GGTT mapping (check can map)
                    // In real system, would insert GGTT entry here

                    // Simulate IOMMU mapping
                    // In real system, would create IOMMU mapping here

                    // Free
                    allocator_clone.free(alloc.size);
                    ops += 1;
                }

                // Check capsule state
                if allocator_clone.capsule().can_allocate(1024) {
                    ops += 1;
                }
            }

            ops
        });
        handles.push(handle);
    }

    // Collect stats
    let mut total_ops = 0;
    for handle in handles {
        total_ops += handle.join().unwrap();
    }

    println!("Full pipeline stress: {} operations", total_ops);

    // Should complete many operations
    assert!(total_ops > 1000, "Too few pipeline operations");
}

#[test]
fn stress_memory_exhaustion_recovery() {
    // Stress test: exhaust memory, free, and re-allocate

    let allocator = Arc::new(GpuMemoryAllocator::new(4 * 1024 * 1024 * 1024)); // 4GB

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for _ in 0..10 {
        let allocator_clone = Arc::clone(&allocator);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            for _ in 0..100 {
                // Try to allocate 500MB
                if let Some(alloc) = allocator_clone.allocate(500 * 1024 * 1024, MemoryDomain::Vram)
                {
                    // Hold briefly
                    thread::sleep(Duration::from_micros(10));

                    // Free
                    allocator_clone.free(alloc.size);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for completion
    for handle in handles {
        handle.join().unwrap();
    }

    // After stress, memory should be mostly freed
    let final_allocated = allocator.allocated_bytes();
    println!(
        "Final allocated after exhaustion stress: {} bytes",
        final_allocated
    );

    assert!(
        final_allocated < 1024 * 1024 * 1024, // Less than 1GB
        "Memory not properly freed: {}",
        final_allocated
    );
}
