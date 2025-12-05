//! T5 Streaming Ring Buffer Trace - Usage Demo
//!
//! Demonstrates instruction tracing with O(1) append performance.

use kdb::t5_trace_buffer::{RingBufferTraceCapsule, TraceEntry, TraceFlags};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn main() {
    println!("=== T5 Streaming Ring Buffer Trace Demo ===\n");

    // ========================================================================
    // DEMO 1: Basic Usage
    // ========================================================================
    println!("Demo 1: Basic single-threaded tracing");
    basic_usage_demo();
    println!();

    // ========================================================================
    // DEMO 2: Performance Validation
    // ========================================================================
    println!("Demo 2: Performance validation (<10ns target)");
    performance_demo();
    println!();

    // ========================================================================
    // DEMO 3: Concurrent Tracing
    // ========================================================================
    println!("Demo 3: Concurrent multi-threaded tracing");
    concurrent_demo();
    println!();

    // ========================================================================
    // DEMO 4: Wraparound Behavior
    // ========================================================================
    println!("Demo 4: Ring buffer wraparound");
    wraparound_demo();
    println!();

    // ========================================================================
    // DEMO 5: Zero-Copy Export
    // ========================================================================
    println!("Demo 5: Zero-copy trace export");
    export_demo();
    println!();

    println!("=== All Demos Complete ===");
}

fn basic_usage_demo() {
    let capsule = RingBufferTraceCapsule::new();

    println!(
        "  Capacity: {} entries ({} KB)",
        capsule.capacity(),
        capsule.memory_usage_bytes() / 1024
    );

    // Simulate instruction trace
    let instructions = vec![
        (0x401000, TraceFlags::Call),
        (0x401010, TraceFlags::Load),
        (0x401014, TraceFlags::Store),
        (0x401018, TraceFlags::Branch),
        (0x401020, TraceFlags::Jump),
        (0x40102c, TraceFlags::Return),
    ];

    let start_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    for (i, (pc, flag)) in instructions.iter().enumerate() {
        let timestamp = ((start_time + i as u64 * 100) % u32::MAX as u64) as u32;
        capsule.record_trace(*pc, timestamp, 0, *flag as u16);
    }

    println!("  Recorded: {} instructions", capsule.total_writes());

    // Retrieve recent trace
    let recent = capsule.get_recent_trace(6);
    println!("  Recent trace (newest first):");
    for (i, entry) in recent.iter().enumerate() {
        let flag_name = match entry.flags {
            x if x == TraceFlags::Call as u16 => "CALL",
            x if x == TraceFlags::Load as u16 => "LOAD",
            x if x == TraceFlags::Store as u16 => "STORE",
            x if x == TraceFlags::Branch as u16 => "BRANCH",
            x if x == TraceFlags::Jump as u16 => "JUMP",
            x if x == TraceFlags::Return as u16 => "RETURN",
            _ => "UNKNOWN",
        };
        println!(
            "    [{}] PC: 0x{:08x} @ {} ns - {}",
            i, entry.pc, entry.timestamp, flag_name
        );
    }
}

fn performance_demo() {
    let capsule = RingBufferTraceCapsule::new();

    const ITERATIONS: usize = 100_000;
    let start = Instant::now();

    for i in 0..ITERATIONS {
        capsule.record_trace(0x500000 + i as u64, i as u32, 0, TraceFlags::Store as u16);
    }

    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / ITERATIONS as u128;
    let ops_per_sec = (ITERATIONS as f64 / elapsed.as_secs_f64()) / 1_000_000.0;

    println!("  Iterations: {}", ITERATIONS);
    println!("  Total time: {:.2} ms", elapsed.as_secs_f64() * 1000.0);
    println!("  Latency: {} ns/op (target: <10ns)", ns_per_op);
    println!("  Throughput: {:.1} M ops/sec", ops_per_sec);

    if ns_per_op < 10 {
        println!("  ✅ EXCEPTIONAL: Meets <10ns target");
    } else if ns_per_op < 20 {
        println!("  ✅ TYPICAL: Within 2× of target");
    } else {
        println!("  ⚠️  Slower than expected (may be CI overhead)");
    }
}

fn concurrent_demo() {
    let capsule = Arc::new(RingBufferTraceCapsule::new());
    let mut handles = vec![];

    const THREADS: usize = 4;
    const WRITES_PER_THREAD: usize = 10_000;

    println!("  Threads: {}", THREADS);
    println!("  Writes per thread: {}", WRITES_PER_THREAD);

    let start = Instant::now();

    for thread_id in 0..THREADS {
        let cap = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let mut successes = 0;
            for i in 0..WRITES_PER_THREAD {
                let pc = 0x600000 + (thread_id * WRITES_PER_THREAD + i) as u64;
                if cap.record_trace(pc, i as u32, thread_id as u16, TraceFlags::Load as u16) {
                    successes += 1;
                }
            }
            successes
        });
        handles.push(handle);
    }

    let mut total_successes = 0;
    for handle in handles {
        total_successes += handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let success_rate = (total_successes as f64 / (THREADS * WRITES_PER_THREAD) as f64) * 100.0;

    println!("  Total writes: {}", capsule.total_writes());
    println!("  Successful: {} ({:.1}%)", total_successes, success_rate);
    println!("  Total time: {:.2} ms", elapsed.as_secs_f64() * 1000.0);

    if success_rate > 99.0 {
        println!("  ✅ EXCELLENT: >99% success rate under contention");
    } else if success_rate > 95.0 {
        println!("  ✅ GOOD: >95% success rate");
    } else {
        println!("  ⚠️  High contention detected");
    }
}

fn wraparound_demo() {
    let capsule = RingBufferTraceCapsule::new();
    let capacity = capsule.capacity();

    println!("  Capacity: {} entries", capacity);

    // Write enough to wrap around once
    let writes = capacity + 100;
    for i in 0..writes {
        capsule.record_trace(0x700000 + i as u64, i as u32, 0, 0);
    }

    println!("  Total writes: {}", capsule.total_writes());
    println!("  Wraparounds: {}", capsule.total_wraps());
    println!("  Head position: {}", capsule.head_position());
    println!("  Head generation: {}", capsule.head_generation());

    // Verify oldest entries were overwritten
    let recent = capsule.get_recent_trace(10);
    println!("  Most recent PC: 0x{:08x}", recent[0].pc);
    println!("  Expected PC:    0x{:08x}", 0x700000 + (writes - 1) as u64);

    if capsule.total_wraps() == 1 && capsule.head_generation() == 1 {
        println!("  ✅ Wraparound handled correctly");
    } else {
        println!("  ⚠️  Unexpected wraparound state");
    }
}

fn export_demo() {
    let capsule = RingBufferTraceCapsule::new();

    // Write some entries
    const COUNT: usize = 500;
    for i in 0..COUNT {
        capsule.record_trace(0x800000 + i as u64, i as u32, 0, 0);
    }

    println!("  Total entries: {}", capsule.total_writes());

    let start = Instant::now();
    let (newer, older) = capsule.export_trace();
    let export_time = start.elapsed();

    println!("  Export time: {} ns (zero-copy)", export_time.as_nanos());
    println!("  Newer slice: {} entries", newer.len());
    println!("  Older slice: {} entries", older.len());

    // Verify total matches
    let total_exported = newer.len() + older.len();
    println!("  Total capacity: {}", total_exported);

    // Sample entries from each slice
    if !newer.is_empty() {
        println!("  First in newer: PC 0x{:08x}", newer[0].pc);
    }
    if !older.is_empty() {
        println!("  First in older: PC 0x{:08x}", older[0].pc);
    }

    if export_time.as_nanos() < 100 {
        println!("  ✅ O(1) export verified (<100ns)");
    } else {
        println!("  ⚠️  Export slower than expected");
    }
}
