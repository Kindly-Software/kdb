//! Atomic Debugger Demo
//!
//! Demonstrates all 6 tiers working together in the T6 Mixed DebuggerCapsule.

use kdb::DebuggerCapsule;

fn main() {
    println!("=== Atomic Debugger Demo ===\n");

    // Create debugger attached to mock process
    let pid = 12345;
    println!("Creating debugger for PID {}...", pid);
    let debugger = DebuggerCapsule::new(pid);
    println!("✓ Debugger created (1 MB capsule)\n");

    // Verify size
    println!("Size verification:");
    println!(
        "  DebuggerCapsule: {} bytes (1 MB)",
        std::mem::size_of::<DebuggerCapsule>()
    );
    println!(
        "  Alignment: {} bytes\n",
        std::mem::align_of::<DebuggerCapsule>()
    );

    // =======================================================================
    // T1 Atomic: Breakpoints & Execution Control
    // =======================================================================
    println!("--- T1 Atomic: Breakpoints & Execution Control ---");

    debugger.attach_to_process(pid).unwrap();
    println!("✓ Attached to process {}", pid);

    let bp1 = debugger.set_breakpoint(0x1000).unwrap();
    println!("✓ Breakpoint {} set at 0x1000", bp1);

    let bp2 = debugger.set_breakpoint(0x2000).unwrap();
    println!("✓ Breakpoint {} set at 0x2000", bp2);

    debugger.continue_execution().unwrap();
    println!("✓ Execution continued\n");

    // =======================================================================
    // T2 SIMD: Stack Unwinding (8× speedup)
    // =======================================================================
    println!("--- T2 SIMD: Stack Unwinding (8× speedup) ---");

    // Simulate some stack frames
    debugger
        .simd_stack
        .push_frame(0x1000, 0x7fff_0000, 0x7fff_0100)
        .unwrap();
    debugger
        .simd_stack
        .push_frame(0x2000, 0x7fff_0100, 0x7fff_0200)
        .unwrap();
    debugger
        .simd_stack
        .push_frame(0x3000, 0x7fff_0200, 0x7fff_0300)
        .unwrap();
    debugger
        .simd_stack
        .push_frame(0x4000, 0x7fff_0300, 0x7fff_0400)
        .unwrap();
    println!("✓ Pushed 4 stack frames");

    let trace = debugger.get_stack_trace().unwrap();
    println!("✓ Stack trace (SIMD-accelerated):");
    for (i, rip) in trace.iter().enumerate() {
        println!("    Frame {}: 0x{:x}", i, rip);
    }
    println!();

    // =======================================================================
    // T5 Streaming: Event Tracing (O(1) overhead)
    // =======================================================================
    println!("--- T5 Streaming: Event Tracing (O(1) overhead) ---");

    debugger.trace.record(0, 1234, 0x1000); // Breakpoint hit
    debugger.trace.record(1, 1234, 0xdeadbeef); // Watchpoint hit
    debugger.trace.record(2, 1234, 0x1004); // Step
    debugger.trace.record(3, 1234, 11); // Signal
    println!("✓ Recorded 4 trace events");

    let (total, dropped) = debugger.trace.get_stats();
    println!("✓ Trace stats: {} total, {} dropped", total, dropped);

    let recent = debugger.trace.drain_recent(10);
    println!("✓ Recent events: {}", recent.len());
    for (i, (event_type, tid, _ts, data)) in recent.iter().enumerate() {
        let event_name = match event_type {
            0 => "Breakpoint",
            1 => "Watchpoint",
            2 => "Step",
            3 => "Signal",
            _ => "Unknown",
        };
        println!(
            "    Event {}: {} (tid={}, data=0x{:x})",
            i, event_name, tid, data
        );
    }
    println!();

    // =======================================================================
    // T9 Persistent: Crash Dumps & Checkpoints
    // =======================================================================
    println!("--- T9 Persistent: Crash Dumps & Checkpoints ---");

    // Export checkpoint
    debugger.execution.set_rip(0x5000);
    debugger.export_checkpoint(100).unwrap();
    println!("✓ Checkpoint 100 exported");

    // Simulate crash
    debugger.execution.set_rip(0xdead_beef);
    debugger.record_crash(11, 0xcafe_babe).unwrap();
    println!("✓ Crash recorded (signal=11, fault_addr=0xcafe_babe)");

    let (signal, fault_addr, rip) = debugger.crash_dump.get_crash_info();
    println!(
        "✓ Crash info: signal={}, fault=0x{:x}, rip=0x{:x}\n",
        signal, fault_addr, rip
    );

    // =======================================================================
    // T10 Probabilistic: Path Deduplication
    // =======================================================================
    println!("--- T10 Probabilistic: Path Deduplication ---");

    // Record some execution paths
    let sig1: [u64; 32] = [1; 32];
    let sig2: [u64; 32] = [2; 32];
    let sig3: [u64; 32] = [1; 32]; // Duplicate of sig1

    debugger.record_execution_path(1, &sig1).unwrap();
    debugger.record_execution_path(2, &sig2).unwrap();
    debugger.record_execution_path(3, &sig3).unwrap();
    println!("✓ Recorded 3 execution paths");

    // Find similar paths
    let similar = debugger.find_similar_paths(&sig1, 0.85);
    println!("✓ Found {} similar paths (threshold=0.85)\n", similar.len());

    // =======================================================================
    // Time-Travel: Reverse Execution
    // =======================================================================
    println!("--- Time-Travel: Reverse Execution ---");

    // Take snapshots while stepping
    debugger.execution.set_rip(0x1000);
    let rip1 = debugger.step_instruction().unwrap();
    println!("✓ Step forward to 0x{:x}", rip1);

    let rip2 = debugger.step_instruction().unwrap();
    println!("✓ Step forward to 0x{:x}", rip2);

    let rip3 = debugger.step_instruction().unwrap();
    println!("✓ Step forward to 0x{:x}", rip3);

    // Now step backward!
    let rip_back = debugger.step_backward().unwrap();
    println!("✓ Step BACKWARD to 0x{:x}", rip_back);

    let rip_back2 = debugger.step_backward().unwrap();
    println!("✓ Step BACKWARD to 0x{:x}", rip_back2);

    let (current, total) = debugger.replay_engine.get_stats();
    println!("✓ Replay stats: snapshot {}/{}\n", current + 1, total);

    // =======================================================================
    // Final Statistics
    // =======================================================================
    println!("--- Final Statistics ---");
    let stats = debugger.get_stats();
    println!("  Instructions: {}", stats.instruction_count);
    println!("  Breakpoint hits: {}", stats.breakpoint_hits);
    println!(
        "  Trace events: {} (dropped: {})",
        stats.trace_events, stats.trace_dropped
    );
    println!("  Snapshots: {}", stats.snapshots_taken);
    println!("  Stack depth: {}", stats.stack_depth);

    println!("\n=== Demo Complete ===");
    println!("All 6 tiers integrated successfully in 1 MB T6 Mixed capsule!");
}
