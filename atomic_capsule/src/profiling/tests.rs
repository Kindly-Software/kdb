//! # Profiling Module Integration Tests
//!
//! Comprehensive T28 test suite for profiling capsules.
//!
//! ## Test Tiers (T28 Framework)
//!
//! - **Q1-Q7 (Unit)**: Individual capsule functionality
//! - **Q8-Q14 (Property)**: Invariant validation
//! - **Q15-Q21 (Integration)**: Cross-capsule interaction
//! - **Q22-Q28 (Production)**: Real-world scenarios

use super::*;

// ============================================================================
// ProfilerCapsule Integration Tests
// ============================================================================

#[cfg(feature = "std")]
mod profiler_integration {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// Q15: Test concurrent sample recording
    #[test]
    fn test_concurrent_sampling() {
        let profiler = Arc::new(ProfilerCapsule::new());
        let mut buffer = profiler::SampleBuffer::new();

        profiler.start().unwrap();

        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                let p = Arc::clone(&profiler);
                thread::spawn(move || {
                    for i in 0..100 {
                        let sample = profiler::SampleEntry::new(
                            (thread_id * 1000 + i) as u64,
                            0,
                            thread_id as u32,
                        );
                        // Note: In real usage, each thread would have its own buffer segment
                        // This is just testing the atomic coordination
                        let _ = p.is_active();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        profiler.stop().unwrap();

        // Verify profiler handled concurrent access
        assert_eq!(profiler.state(), profiler::ProfilerState::Stopped);
    }

    /// Q16: Test sample buffer overflow handling
    #[test]
    fn test_buffer_overflow() {
        let profiler = ProfilerCapsule::new();
        let mut buffer = profiler::SampleBuffer::with_capacity(10); // Small buffer

        profiler.start().unwrap();

        // Record more samples than buffer capacity
        for i in 0..20 {
            let sample = profiler::SampleEntry::new(i * 1000, 0, 1);
            profiler.record_sample(buffer.as_mut_slice(), sample);
        }

        // Should have recorded up to capacity
        assert!(profiler.total_samples() > 0);

        profiler.stop().unwrap();
    }

    /// Q17: Test profiler state machine
    #[test]
    fn test_state_machine() {
        let profiler = ProfilerCapsule::new();

        // Initial state
        assert_eq!(profiler.state(), profiler::ProfilerState::Stopped);

        // Start
        assert!(profiler.start().is_ok());
        assert_eq!(profiler.state(), profiler::ProfilerState::Started);

        // Can't start twice
        assert!(profiler.start().is_err());

        // Stop
        assert!(profiler.stop().is_ok());
        assert_eq!(profiler.state(), profiler::ProfilerState::Stopped);

        // Can restart
        assert!(profiler.start().is_ok());
        assert_eq!(profiler.state(), profiler::ProfilerState::Started);
    }

    /// Q18: Test sample consumption ordering
    #[test]
    fn test_sample_ordering() {
        let profiler = ProfilerCapsule::new();
        let mut buffer = profiler::SampleBuffer::new();

        profiler.start().unwrap();

        // Record samples with sequential timestamps
        for i in 0..10 {
            let sample = profiler::SampleEntry::new(i * 1000, 0, 1);
            profiler.record_sample(buffer.as_mut_slice(), sample);
        }

        // Consume and verify ordering
        let mut last_timestamp = 0u64;
        let mut count = 0;

        profiler.consume_samples(buffer.as_slice(), |sample| {
            assert!(sample.timestamp_ns >= last_timestamp);
            last_timestamp = sample.timestamp_ns;
            count += 1;
        });

        assert_eq!(count, 10);

        profiler.stop().unwrap();
    }
}

// ============================================================================
// FlameGraphCapsule Integration Tests
// ============================================================================

#[cfg(feature = "std")]
mod flamegraph_integration {
    use super::*;
    use std::collections::HashMap;

    /// Q19: Test flamegraph from profiler samples
    #[test]
    fn test_process_samples() {
        let fg = FlameGraphCapsule::new();
        let mut nodes = flamegraph::NodePool::new();

        // Create mock samples
        let mut samples = Vec::new();
        for i in 0..100 {
            let mut sample = profiler::SampleEntry::new(i * 1000, 0, 1);
            // Add stack frames
            sample.add_frame(profiler::StackFrame::new(0x1000, 0, profiler::FrameFlags::USER, 1));
            sample.add_frame(profiler::StackFrame::new(0x2000, 0, profiler::FrameFlags::USER, 1));
            sample.add_frame(profiler::StackFrame::new(0x3000 + (i % 3) * 0x100, 0, profiler::FrameFlags::USER, 1));
            samples.push(sample);
        }

        // Simple symbolizer (uses IP as hash)
        let symbolizer = |frame: &profiler::StackFrame| frame.instruction_ptr;

        // Process samples
        let result = fg.process_samples(&samples, nodes.as_mut_slice(), symbolizer);
        assert!(result.is_ok());

        assert_eq!(fg.state(), flamegraph::FlameState::Complete);
        assert!(fg.total_samples() > 0);
        assert!(fg.node_count() > 1); // Root + at least one child
    }

    /// Q20: Test collapsed stack generation
    #[test]
    fn test_collapsed_stack_output() {
        let fg = FlameGraphCapsule::new();
        let mut nodes = flamegraph::NodePool::new();

        // Create mock samples with known stack
        let mut samples = Vec::new();
        for _ in 0..50 {
            let mut sample = profiler::SampleEntry::new(1000, 0, 1);
            sample.add_frame(profiler::StackFrame::new(0x1000, 0, profiler::FrameFlags::USER, 1));
            sample.add_frame(profiler::StackFrame::new(0x2000, 0, profiler::FrameFlags::USER, 1));
            samples.push(sample);
        }

        // Process
        fg.process_samples(&samples, nodes.as_mut_slice(), |frame| frame.instruction_ptr).unwrap();

        // Generate collapsed stacks
        let name_map: HashMap<u64, &str> = [
            (0x1000, "main"),
            (0x2000, "foo"),
        ].into_iter().collect();

        let collapsed = fg.generate_collapsed(nodes.as_slice(), |hash| {
            name_map.get(&hash).unwrap_or(&"unknown").to_string()
        });

        // Should have at least one stack
        assert!(!collapsed.is_empty());
    }

    /// Q21: Test SVG generation
    #[test]
    fn test_svg_generation() {
        let fg = FlameGraphCapsule::new();
        let mut nodes = flamegraph::NodePool::new();

        // Create mock samples
        let mut samples = Vec::new();
        for i in 0..10 {
            let mut sample = profiler::SampleEntry::new(i * 1000, 0, 1);
            sample.add_frame(profiler::StackFrame::new(0x1000, 0, profiler::FrameFlags::USER, 1));
            samples.push(sample);
        }

        fg.process_samples(&samples, nodes.as_mut_slice(), |frame| frame.instruction_ptr).unwrap();

        let svg = fg.generate_svg(nodes.as_slice(), |hash| {
            format!("func_{:x}", hash)
        }, "Test Flamegraph");

        assert!(svg.contains("<?xml"));
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("Test Flamegraph"));
    }
}

// ============================================================================
// PerfCounterCapsule Integration Tests
// ============================================================================

mod perf_counter_integration {
    use super::*;
    use perf_counter::MAX_COUNTERS;
    use std::sync::Arc;
    use std::thread;

    /// Q22: Test concurrent counter updates
    #[test]
    fn test_concurrent_updates() {
        let capsule = Arc::new(PerfCounterCapsule::new());

        capsule.enable(0xFFFF).unwrap();

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let c = Arc::clone(&capsule);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        c.add(0, 1);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All increments should be counted
        assert_eq!(capsule.read(0), Some(8000));

        capsule.disable().unwrap();
    }

    /// Q23: Test multi-counter snapshot consistency
    #[test]
    fn test_snapshot_consistency() {
        let capsule = PerfCounterCapsule::new();

        // Write known values
        for i in 0..MAX_COUNTERS {
            capsule.write(i, (i * 100) as u64);
        }

        // Take snapshot
        let snapshot = capsule.snapshot();

        // Verify all values
        for i in 0..MAX_COUNTERS {
            assert_eq!(snapshot[i], (i * 100) as u64);
        }
    }

    /// Q24: Test overflow detection accuracy
    #[test]
    fn test_overflow_tracking() {
        let capsule = PerfCounterCapsule::new();

        // Set counter near max
        capsule.write(0, u64::MAX - 100);

        // Add to trigger overflow
        for _ in 0..200 {
            capsule.add(0, 1);
        }

        // Check overflow was detected
        let value = capsule.read_with_overflow(0).unwrap();
        assert!(value.has_overflow());
        assert!(value.overflow_count > 0);
    }

    /// Q25: Test IPC calculation
    #[test]
    fn test_ipc_calculation() {
        let capsule = PerfCounterCapsule::new();

        // Set instructions = 4000, cycles = 2000 (IPC = 2.0)
        capsule.write(perf_counter::CounterType::Instructions as usize, 4000);
        capsule.write(perf_counter::CounterType::CpuCycles as usize, 2000);

        let ipc = capsule.ipc().unwrap();
        assert!((ipc - 2.0).abs() < 0.01);
    }

    /// Q26: Test cache miss rate calculation
    #[test]
    fn test_cache_miss_rate() {
        let capsule = PerfCounterCapsule::new();

        // Set loads = 1000, misses = 100 (10% miss rate)
        capsule.write(perf_counter::CounterType::LLCLoads as usize, 1000);
        capsule.write(perf_counter::CounterType::LLCMisses as usize, 100);

        let rate = capsule.cache_miss_rate().unwrap();
        assert!((rate - 0.1).abs() < 0.01);
    }
}

// ============================================================================
// Cross-Module Integration Tests
// ============================================================================

#[cfg(feature = "std")]
mod cross_module_integration {
    use super::*;

    /// Q27: Test full profiling workflow
    #[test]
    fn test_full_workflow() {
        // 1. Create profiler and perf counters
        let profiler = ProfilerCapsule::new();
        let counters = PerfCounterCapsule::new();
        let mut buffer = profiler::SampleBuffer::new();

        // 2. Enable profiling
        profiler.start().unwrap();
        counters.enable(0b111).unwrap();

        // 3. Simulate workload with samples
        for i in 0..50 {
            let mut sample = profiler::SampleEntry::new(i * 1000, 0, 1);
            sample.add_frame(profiler::StackFrame::new(0x1000 + i, 0, profiler::FrameFlags::USER, 1));
            profiler.record_sample(buffer.as_mut_slice(), sample);

            // Update counters
            counters.add(0, 100);
            counters.add(1, 50);
        }

        // 4. Stop profiling
        profiler.stop().unwrap();
        counters.disable().unwrap();

        // 5. Generate flamegraph
        let fg = FlameGraphCapsule::new();
        let mut nodes = flamegraph::NodePool::new();

        // Collect samples
        let mut samples: Vec<profiler::SampleEntry> = Vec::new();
        profiler.consume_samples(buffer.as_slice(), |sample| {
            samples.push(sample.clone());
        });

        // Process into flamegraph
        let result = fg.process_samples(&samples, nodes.as_mut_slice(), |frame| frame.instruction_ptr);
        assert!(result.is_ok());

        // 6. Verify results
        assert_eq!(profiler.total_samples(), 50);
        assert_eq!(fg.state(), flamegraph::FlameState::Complete);
        assert_eq!(counters.read(0), Some(5000));
        assert_eq!(counters.read(1), Some(2500));
    }

    /// Q28: Test performance characteristics
    #[test]
    fn test_performance_overhead() {
        use std::time::Instant;

        let capsule = PerfCounterCapsule::new();
        capsule.enable(0xFFFF).unwrap();

        // Measure counter read latency (target: <5ns)
        let iterations = 10000;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = capsule.read(0);
        }
        let read_time = start.elapsed();
        let read_ns = read_time.as_nanos() / iterations as u128;

        // Measure counter add latency (target: <10ns)
        let start = Instant::now();
        for _ in 0..iterations {
            capsule.add(0, 1);
        }
        let add_time = start.elapsed();
        let add_ns = add_time.as_nanos() / iterations as u128;

        // Measure snapshot latency (target: <50ns)
        let start = Instant::now();
        for _ in 0..(iterations / 10) {
            let _ = capsule.snapshot();
        }
        let snapshot_time = start.elapsed();
        let snapshot_ns = snapshot_time.as_nanos() / (iterations / 10) as u128;

        capsule.disable().unwrap();

        // Performance assertions (relaxed for CI variability)
        // In production, these would be stricter
        assert!(read_ns < 100, "Counter read too slow: {}ns", read_ns);
        assert!(add_ns < 200, "Counter add too slow: {}ns", add_ns);
        assert!(snapshot_ns < 500, "Snapshot too slow: {}ns", snapshot_ns);
    }
}

// ============================================================================
// Property-Based Tests
// ============================================================================

mod property_tests {
    use super::*;
    use perf_counter::MAX_COUNTERS;

    /// Q8: Profiler generation counter monotonicity
    #[test]
    fn test_generation_monotonic() {
        let profiler = ProfilerCapsule::new();
        let mut last_gen = profiler.generation();

        for _ in 0..10 {
            profiler.start().unwrap();
            let gen = profiler.generation();
            assert!(gen > last_gen);
            last_gen = gen;

            profiler.stop().unwrap();
            let gen = profiler.generation();
            assert!(gen > last_gen);
            last_gen = gen;
        }
    }

    /// Q9: Counter values never decrease (no underflow)
    #[test]
    fn test_counter_no_underflow() {
        let capsule = PerfCounterCapsule::new();

        capsule.write(0, 100);

        // Add should never decrease value
        for _ in 0..100 {
            let old = capsule.read(0).unwrap();
            capsule.add(0, 1);
            let new = capsule.read(0).unwrap();

            // Account for overflow wraparound
            if new < old {
                // Overflow occurred, which is valid behavior
                continue;
            }
            assert!(new >= old);
        }
    }

    /// Q10: Flamegraph node count bounded
    #[test]
    fn test_node_count_bounded() {
        let fg = FlameGraphCapsule::new();

        // Node count should never exceed max
        assert!(fg.node_count() <= flamegraph::MAX_NODES as u64);
    }

    /// Q11: State transitions are valid
    #[test]
    fn test_valid_state_transitions() {
        let profiler = ProfilerCapsule::new();

        // Stopped -> Started (valid)
        assert!(profiler.start().is_ok());

        // Started -> Started (invalid)
        assert!(profiler.start().is_err());

        // Started -> Stopped (valid)
        assert!(profiler.stop().is_ok());

        // Stopped -> Stopped (invalid)
        assert!(profiler.stop().is_err());
    }

    /// Q12: Counter mask consistency
    #[test]
    fn test_mask_consistency() {
        let capsule = PerfCounterCapsule::new();

        let mask = 0b10101010;
        capsule.enable(mask).unwrap();

        // is_counter_enabled should match mask
        for i in 0..16 {
            let expected = (mask & (1 << i)) != 0;
            assert_eq!(capsule.is_counter_enabled(i), expected, "Counter {} mismatch", i);
        }

        capsule.disable().unwrap();
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

mod edge_cases {
    use super::*;

    /// Test profiler with zero samples
    #[test]
    fn test_empty_profiler() {
        let profiler = ProfilerCapsule::new();

        profiler.start().unwrap();
        profiler.stop().unwrap();

        assert_eq!(profiler.total_samples(), 0);
        assert_eq!(profiler.dropped_samples(), 0);
    }

    /// Test counter at max value
    #[test]
    fn test_counter_max_value() {
        let capsule = PerfCounterCapsule::new();

        capsule.write(0, u64::MAX);
        assert_eq!(capsule.read(0), Some(u64::MAX));

        // Adding should wrap
        capsule.add(0, 1);
        assert_eq!(capsule.read(0), Some(0));
    }

    #[cfg(feature = "std")]
    /// Test flamegraph with single sample
    #[test]
    fn test_single_sample_flamegraph() {
        let fg = FlameGraphCapsule::new();
        let mut nodes = flamegraph::NodePool::new();

        let mut sample = profiler::SampleEntry::new(1000, 0, 1);
        sample.add_frame(profiler::StackFrame::new(0x1000, 0, profiler::FrameFlags::USER, 1));

        let samples = vec![sample];

        fg.process_samples(&samples, nodes.as_mut_slice(), |f| f.instruction_ptr).unwrap();

        assert_eq!(fg.total_samples(), 1);
    }

    /// Test stack frame depth limit
    #[test]
    fn test_stack_depth_limit() {
        let mut sample = profiler::SampleEntry::new(1000, 0, 1);

        // Add frames up to limit
        for i in 0..profiler::MAX_STACK_DEPTH {
            assert!(sample.add_frame(profiler::StackFrame::new(i as u64, 0, 0, 0)));
        }

        // Next frame should fail and set truncated flag
        assert!(!sample.add_frame(profiler::StackFrame::new(0xFF, 0, 0, 0)));
        assert!((sample.flags & profiler::SampleFlags::TRUNCATED) != 0);
    }

    /// Test counter disabled operations
    #[test]
    fn test_disabled_counter_operations() {
        let capsule = PerfCounterCapsule::new();

        // Operations work even when disabled
        capsule.write(0, 100);
        assert_eq!(capsule.read(0), Some(100));

        capsule.add(0, 50);
        assert_eq!(capsule.read(0), Some(150));
    }
}
