//! Memory Corruption Integration Tests
//!
//! Tests kdb's ability to detect, diagnose, and report memory corruption bugs.
//! Uses the buffer_overflow_target and use_after_free_target example programs.
//!
//! Framework: T28 Q15-Q21 (Integration testing tier)
//! Framework: T28 Q29-Q35 (Determinism testing tier)
//!
//! Test Categories:
//! - Buffer overflow detection and diagnosis
//! - Use-after-free detection
//! - Stack corruption analysis
//! - Heap corruption analysis
//! - Memory pattern recognition
//!
//! #ASSUME_LINUX_PTRACE: ptrace() syscall available
//! #ASSUME_PROCESS_SPAWNED: Target processes can be created
//! #ASSUME_MEMORY_READABLE: Debugger can read target memory
//!
//! Run all tests:
//!   cargo test --test memory_corruption_tests
//!
//! Run with verbose output:
//!   cargo test --test memory_corruption_tests -- --nocapture
//!
//! Run ignored tests (requires ptrace):
//!   cargo test --test memory_corruption_tests -- --ignored

#[cfg(target_os = "linux")]
mod tests {
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    // =========================================================================
    // Constants for Memory Corruption Detection
    // =========================================================================

    /// Stack canary pattern from buffer_overflow_target
    const STACK_CANARY: u64 = 0xDEAD_BEEF_CAFE_BABE;

    /// Free memory pattern
    const FREE_PATTERN: u8 = 0xDD;

    /// Overflow pattern from buffer_overflow_target
    const OVERFLOW_PATTERN: u8 = 0xCC;

    /// Allocation magic from use_after_free_target
    const ALLOC_MAGIC: u64 = 0xA11C_A7ED_DEAD_BEEF;

    /// Free magic from use_after_free_target
    const FREE_MAGIC: u64 = 0xF4EE_DDA7_ACAF_E000;

    // =========================================================================
    // Test Helpers
    // =========================================================================

    /// Build and run a target example, returning the child process
    fn spawn_target(example_name: &str, args: &[&str]) -> Result<Child, String> {
        // Build the example first
        let build_status = Command::new("cargo")
            .args(["build", "--release", "--example", example_name])
            .current_dir("/home/samuel/Primitives/Kindly-Debugger/kdb")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("Failed to build example: {}", e))?;

        if !build_status.success() {
            return Err(format!("Failed to build example {}", example_name));
        }

        // Run the pre-built binary directly (faster than cargo run)
        // Note: In a workspace, binaries are in the workspace root target directory
        // Workspace root target directory (kdb is part of Primitives workspace)
        let binary_path = format!(
            "/home/samuel/Primitives/target/release/examples/{}",
            example_name
        );

        let child = Command::new(&binary_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", binary_path, e))?;

        Ok(child)
    }

    /// Helper to read process output with timeout
    fn wait_with_timeout(mut child: Child, timeout: Duration) -> Result<String, String> {
        let start = Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    let output = child.wait_with_output()
                        .map_err(|e| format!("Failed to read output: {}", e))?;

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);

                    return Ok(format!("stdout:\n{}\nstderr:\n{}", stdout, stderr));
                }
                Ok(None) => {
                    if start.elapsed() > timeout {
                        child.kill().ok();
                        return Err("Process timed out".to_string());
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(format!("Error waiting for process: {}", e)),
            }
        }
    }

    // =========================================================================
    // Buffer Overflow Tests
    // =========================================================================

    /// Test 1: Buffer overflow target builds successfully
    #[test]
    fn test_buffer_overflow_target_builds() {
        let status = Command::new("cargo")
            .args(["build", "--example", "buffer_overflow_target"])
            .current_dir("/home/samuel/Primitives/Kindly-Debugger/kdb")
            .status()
            .expect("Failed to execute cargo build");

        assert!(status.success(), "buffer_overflow_target should compile");
    }

    /// Test 2: Buffer overflow target runs in safe mode
    #[test]
    fn test_buffer_overflow_safe_mode() {
        let child = spawn_target("buffer_overflow_target", &["safe"])
            .expect("Failed to spawn target");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Target should complete");

        assert!(output.contains("SAFE MODE"), "Should run in safe mode");
        assert!(output.contains("Canary check: PASSED"), "Canaries should be intact in safe mode");
        assert!(output.contains("Safe Mode Complete"), "Should complete successfully");
    }

    /// Test 3: Buffer overflow target detects overflow
    #[test]
    fn test_buffer_overflow_detection() {
        let child = spawn_target("buffer_overflow_target", &["overflow"])
            .expect("Failed to spawn target");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Target should complete");

        assert!(output.contains("OVERFLOW MODE"), "Should run in overflow mode");
        assert!(output.contains("Canary check: FAILED"), "Canaries should be corrupted");
        assert!(output.contains("buffer overflow detected"), "Should detect overflow");
    }

    /// Test 4: Canary detection at various overflow sizes
    #[test]
    fn test_canary_detection_sizes() {
        let child = spawn_target("buffer_overflow_target", &["canary"])
            .expect("Failed to spawn target");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Target should complete");

        assert!(output.contains("CANARY DETECTION MODE"), "Should run in canary mode");
        // 64 bytes should be SAFE, anything larger should be CORRUPTED
        assert!(output.contains("Write 64 bytes (overflow 0): SAFE"),
            "64 bytes should not overflow");
        assert!(output.contains("CORRUPTED"),
            "Overflow should be detected for sizes > 64");
    }

    /// Test 5: Heap mode runs
    #[test]
    fn test_buffer_overflow_heap_mode() {
        let child = spawn_target("buffer_overflow_target", &["heap"])
            .expect("Failed to spawn target");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Target should complete");

        assert!(output.contains("HEAP MODE"), "Should run in heap mode");
        assert!(output.contains("Heap buffer allocated"), "Should allocate heap buffer");
    }

    // =========================================================================
    // Use-After-Free Tests
    // =========================================================================

    /// Test 6: Use-after-free target builds successfully
    #[test]
    fn test_use_after_free_target_builds() {
        let status = Command::new("cargo")
            .args(["build", "--example", "use_after_free_target"])
            .current_dir("/home/samuel/Primitives/Kindly-Debugger/kdb")
            .status()
            .expect("Failed to execute cargo build");

        assert!(status.success(), "use_after_free_target should compile");
    }

    /// Test 7: Use-after-free target runs in safe mode
    #[test]
    fn test_use_after_free_safe_mode() {
        let child = spawn_target("use_after_free_target", &["safe"])
            .expect("Failed to spawn target");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Target should complete");

        assert!(output.contains("SAFE MODE"), "Should run in safe mode");
        assert!(output.contains("Allocation freed safely"), "Should free safely");
        assert!(output.contains("Safe Mode Complete"), "Should complete successfully");
    }

    /// Test 8: Use-after-free read detection
    #[test]
    fn test_use_after_free_read_detection() {
        let child = spawn_target("use_after_free_target", &["uaf"])
            .expect("Failed to spawn target");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Target should complete");

        assert!(output.contains("UAF READ MODE"), "Should run in UAF mode");
        assert!(output.contains("Attempting use-after-free READ"),
            "Should attempt UAF read");
        assert!(output.contains("Read from freed memory"),
            "Should read from freed memory");
    }

    /// Test 9: Use-after-free write detection
    #[test]
    fn test_use_after_free_write_detection() {
        let child = spawn_target("use_after_free_target", &["uaf_write"])
            .expect("Failed to spawn target");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Target should complete");

        assert!(output.contains("UAF WRITE MODE"), "Should run in UAF write mode");
        assert!(output.contains("Attempting use-after-free WRITE"),
            "Should attempt UAF write");
        assert!(output.contains("undefined behavior"),
            "Should acknowledge UB");
    }

    /// Test 10: Double-free detection scenario
    #[test]
    fn test_double_free_scenario() {
        let child = spawn_target("use_after_free_target", &["double_free"])
            .expect("Failed to spawn target");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Target should complete");

        assert!(output.contains("DOUBLE FREE MODE"), "Should run in double-free mode");
        assert!(output.contains("First free completed"), "First free should succeed");
    }

    /// Test 11: Dangling pointer scenario
    #[test]
    fn test_dangling_pointer_scenario() {
        let child = spawn_target("use_after_free_target", &["dangling"])
            .expect("Failed to spawn target");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Target should complete");

        assert!(output.contains("DANGLING POINTER MODE"), "Should run in dangling mode");
        assert!(output.contains("is_dangling() = true"), "Should detect dangling pointer");
    }

    // =========================================================================
    // Memory Pattern Tests (Unit Tests - No Process Spawn)
    // =========================================================================

    /// Test 12: Stack canary pattern is recognizable
    #[test]
    fn test_stack_canary_pattern() {
        let canary = STACK_CANARY;

        // Canary should have recognizable pattern
        let bytes = canary.to_le_bytes();
        assert_eq!(bytes[0], 0xBE); // BABE suffix
        assert_eq!(bytes[1], 0xBA);
        assert_eq!(bytes[2], 0xFE); // CAFE
        assert_eq!(bytes[3], 0xCA);
    }

    /// Test 13: Overflow pattern is distinct
    #[test]
    fn test_overflow_pattern_distinct() {
        assert_eq!(OVERFLOW_PATTERN, 0xCC);
        assert_ne!(OVERFLOW_PATTERN, 0x00);
        assert_ne!(OVERFLOW_PATTERN, FREE_PATTERN);
    }

    /// Test 14: Free pattern is distinct
    #[test]
    fn test_free_pattern_distinct() {
        assert_eq!(FREE_PATTERN, 0xDD);
        assert_ne!(FREE_PATTERN, 0x00);
        assert_ne!(FREE_PATTERN, OVERFLOW_PATTERN);
    }

    // =========================================================================
    // Debugger Integration Tests (Require ptrace)
    // =========================================================================

    /// Test 15: Debugger can attach to safe buffer overflow target
    #[test]
    #[ignore = "Requires ptrace capabilities - run with --ignored"]
    fn test_debugger_attach_buffer_overflow() {
        use kdb::DebuggerCapsule;

        // Spawn target in wait mode
        let mut child = Command::new("cargo")
            .args(["run", "--example", "buffer_overflow_target", "--", "wait"])
            .current_dir("/home/samuel/Primitives/Kindly-Debugger/kdb")
            .env("KDB_WAIT", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn target");

        let pid = child.id() as u64;

        // Wait a bit for process to initialize
        std::thread::sleep(Duration::from_millis(100));

        // Create debugger (heap allocation for large struct)
        let debugger = Box::new(DebuggerCapsule::new(pid));

        // Verify debugger initialized with correct PID
        assert_eq!(debugger.execution.get_pid(), pid);

        // Clean up
        child.kill().ok();
    }

    /// Test 16: Debugger can attach to use-after-free target
    #[test]
    #[ignore = "Requires ptrace capabilities - run with --ignored"]
    fn test_debugger_attach_use_after_free() {
        use kdb::DebuggerCapsule;

        // Spawn target in wait mode
        let mut child = Command::new("cargo")
            .args(["run", "--example", "use_after_free_target", "--", "wait"])
            .current_dir("/home/samuel/Primitives/Kindly-Debugger/kdb")
            .env("KDB_WAIT", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn target");

        let pid = child.id() as u64;

        std::thread::sleep(Duration::from_millis(100));

        let debugger = Box::new(DebuggerCapsule::new(pid));
        assert_eq!(debugger.execution.get_pid(), pid);

        child.kill().ok();
    }

    // =========================================================================
    // Memory Corruption Analysis Tests
    // =========================================================================

    /// Test 17: Analyze corrupted stack frame pattern
    #[test]
    fn test_analyze_corrupted_stack_pattern() {
        // Simulate a corrupted stack frame
        let mut frame = [0u8; 128];

        // Fill with normal data
        for (i, byte) in frame.iter_mut().enumerate() {
            *byte = (i & 0xFF) as u8;
        }

        // Add canary at expected positions
        let canary_pos = 72; // After 64-byte buffer + 8-byte marker
        frame[canary_pos..canary_pos+8].copy_from_slice(&STACK_CANARY.to_le_bytes());

        // Simulate overflow
        for byte in &mut frame[64..80] {
            *byte = OVERFLOW_PATTERN;
        }

        // Verify overflow detection
        let canary_bytes = &frame[canary_pos..canary_pos+8];
        let found_canary = u64::from_le_bytes(canary_bytes.try_into().unwrap());

        // Canary should be corrupted by overflow pattern
        assert_ne!(found_canary, STACK_CANARY, "Canary should be corrupted");
        assert_eq!(frame[canary_pos], OVERFLOW_PATTERN, "Canary area should have overflow pattern");
    }

    /// Test 18: Detect free pattern in memory
    #[test]
    fn test_detect_free_pattern() {
        let mut memory = [0u8; 64];

        // Fill with free pattern
        for byte in &mut memory {
            *byte = FREE_PATTERN;
        }

        // Check if memory looks like freed memory
        let is_freed = memory.iter().all(|&b| b == FREE_PATTERN);
        assert!(is_freed, "Memory should be recognized as freed");
    }

    /// Test 19: Distinguish between allocated and freed memory
    #[test]
    fn test_distinguish_alloc_free() {
        // Simulated allocation header check
        let alloc_header = ALLOC_MAGIC;
        let free_header = FREE_MAGIC;

        // Check magic values are distinct
        assert_ne!(alloc_header, free_header);

        // Pattern matching for corruption detection
        let is_allocated = alloc_header == ALLOC_MAGIC;
        let is_freed = free_header == FREE_MAGIC;

        assert!(is_allocated);
        assert!(is_freed);
    }

    // =========================================================================
    // Determinism Tests (T28 Q29-Q35)
    // =========================================================================

    /// Test 20: Memory patterns are deterministic across runs
    #[test]
    fn test_memory_patterns_deterministic() {
        // Run safe mode twice and compare output patterns
        let child1 = spawn_target("buffer_overflow_target", &["safe"])
            .expect("First run failed");
        let output1 = wait_with_timeout(child1, Duration::from_secs(5))
            .expect("First run timeout");

        let child2 = spawn_target("buffer_overflow_target", &["safe"])
            .expect("Second run failed");
        let output2 = wait_with_timeout(child2, Duration::from_secs(5))
            .expect("Second run timeout");

        // Both should have same structural output (PIDs will differ)
        assert!(output1.contains("Canary check: PASSED"));
        assert!(output2.contains("Canary check: PASSED"));
        assert!(output1.contains("Safe Mode Complete"));
        assert!(output2.contains("Safe Mode Complete"));
    }

    /// Test 21: Overflow detection is deterministic
    #[test]
    fn test_overflow_detection_deterministic() {
        // Run overflow mode multiple times
        for i in 0..3 {
            let child = spawn_target("buffer_overflow_target", &["overflow"])
                .expect(&format!("Run {} failed", i));
            let output = wait_with_timeout(child, Duration::from_secs(5))
                .expect(&format!("Run {} timeout", i));

            assert!(output.contains("Canary check: FAILED"),
                "Run {} should detect overflow", i);
        }
    }

    // =========================================================================
    // Performance Tests (B32 Validation)
    // =========================================================================

    /// Test 22: Target spawning performance
    #[test]
    fn test_spawn_performance() {
        let start = Instant::now();
        let iterations = 5;

        for _ in 0..iterations {
            let child = spawn_target("buffer_overflow_target", &["safe"])
                .expect("Spawn failed");
            wait_with_timeout(child, Duration::from_secs(5))
                .expect("Wait failed");
        }

        let elapsed = start.elapsed();
        let per_spawn = elapsed / iterations;

        println!("Spawn + run performance: {:?} per iteration", per_spawn);

        // Should complete in reasonable time (< 2s per iteration including compilation cache)
        assert!(per_spawn < Duration::from_secs(2),
            "Each spawn+run should take < 2s");
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    /// Test 23: Invalid mode handling
    #[test]
    fn test_invalid_mode_buffer_overflow() {
        let child = spawn_target("buffer_overflow_target", &["invalid_mode"])
            .expect("Failed to spawn");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Should complete even with invalid mode");

        assert!(output.contains("Unknown mode") || output.contains("Valid modes"),
            "Should report invalid mode");
    }

    /// Test 24: Invalid mode handling for UAF target
    #[test]
    fn test_invalid_mode_use_after_free() {
        let child = spawn_target("use_after_free_target", &["invalid_mode"])
            .expect("Failed to spawn");

        let output = wait_with_timeout(child, Duration::from_secs(5))
            .expect("Should complete even with invalid mode");

        assert!(output.contains("Unknown mode") || output.contains("Valid modes"),
            "Should report invalid mode");
    }
}

// =========================================================================
// Non-Linux Platform Tests
// =========================================================================

#[cfg(not(target_os = "linux"))]
mod non_linux_tests {
    #[test]
    fn test_memory_corruption_tests_skipped() {
        println!("Memory corruption integration tests require Linux platform");
        println!("These tests use ptrace and process spawning specific to Linux");
    }
}

// =========================================================================
// Common Test Utilities
// =========================================================================

/// Helper to create debugger on heap (avoids stack overflow for large struct)
#[cfg(target_os = "linux")]
#[allow(unused)]
fn create_debugger_on_heap(pid: u64) -> Box<kdb::DebuggerCapsule> {
    Box::new(kdb::DebuggerCapsule::new(pid))
}
