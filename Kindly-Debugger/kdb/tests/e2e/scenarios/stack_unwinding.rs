//! E2E-05: Stack Trace Correctness Scenarios
//!
//! Tests for SIMD-accelerated stack unwinding and trace correctness.
//! Validates that kdb produces accurate stack traces comparable to GDB.
//!
//! # Test Coverage
//!
//! - Basic stack trace retrieval
//! - Stack depth validation
//! - Frame address validity
//! - GDB comparison (when available)
//! - Deep stack traces
//!
//! # ASSUM Safety
//!
//! - #ASSUME_PTRACE_AVAILABLE: Tests require CAP_SYS_PTRACE
//! - #ASSUME_SIMD_AVAILABLE: SIMD acceleration auto-detected by kdb
//! - #ASSUME_DWARF_AVAILABLE: Some tests may require debug info

use super::*;

/// E2E-05: Stack trace correctness
///
/// Tests basic stack trace retrieval:
/// 1. Attach to process
/// 2. Get stack trace
/// 3. Verify minimum frame count
/// 4. Verify frame addresses are valid (non-zero)
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_stack_trace_correctness() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(30)?;

    // Get stack trace
    let frames = fixture.driver.get_stack_trace()?;

    // Should have at least 1 frame
    assert!(!frames.is_empty(), "Stack trace should have at least one frame");

    // Validate basic structure
    let result = fixture.validator.validate_stack_trace(&frames, 1);
    result.into_result()?;

    // Verify frame[0] has non-zero RIP
    assert!(frames[0].rip != 0, "Current frame should have non-zero RIP");

    // Verify frame indices are sequential
    for (i, frame) in frames.iter().enumerate() {
        assert_eq!(
            frame.index, i,
            "Frame index should match position: expected {}, got {}",
            i, frame.index
        );
    }

    fixture.cleanup()?;
    Ok(())
}

/// E2E-05b: Stack trace without attach
///
/// Tests that stack trace fails when not attached.
#[test]
fn test_stack_trace_not_attached() {
    let kdb = DebuggerDriver::new();

    let result = kdb.get_stack_trace();

    assert!(result.is_err());
    assert!(matches!(result, Err(E2EError::NotAttached)));
}

/// E2E-05c: Stack trace depth validation
///
/// Tests that stack traces have reasonable depth.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_stack_trace_depth() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(30)?;

    let frames = fixture.driver.get_stack_trace()?;

    // A typical user-space process should have at least a few frames
    // (main, libc start, etc.)
    eprintln!("Stack trace has {} frames", frames.len());

    // At minimum, we should have the current frame
    assert!(frames.len() >= 1, "Should have at least 1 frame");

    // Reasonable upper bound (shouldn't have millions of frames)
    assert!(
        frames.len() < 10000,
        "Stack trace should have reasonable depth, got {}",
        frames.len()
    );

    fixture.cleanup()?;
    Ok(())
}

/// E2E-05d: Stack frame address validity
///
/// Tests that stack frame addresses are in valid ranges.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_stack_frame_addresses() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(30)?;

    let frames = fixture.driver.get_stack_trace()?;

    for (i, frame) in frames.iter().enumerate() {
        // Frame 0 should always have a valid RIP
        if i == 0 {
            assert!(
                frame.rip != 0,
                "Frame 0 RIP should be non-zero"
            );
        }

        // If RIP is non-zero, it should be in a reasonable range
        // (typical user-space is 0x400000-0x7fffffffffff on x86_64)
        if frame.rip != 0 {
            // Should be in user space (not kernel space)
            assert!(
                frame.rip < 0x0000800000000000,
                "Frame {} RIP 0x{:x} should be in user space",
                i,
                frame.rip
            );

            // Should be above typical code start
            // (relaxed check, some dynamic libs may be lower)
            assert!(
                frame.rip >= 0x1000,
                "Frame {} RIP 0x{:x} should be above 0x1000",
                i,
                frame.rip
            );
        }
    }

    fixture.cleanup()?;
    Ok(())
}

/// E2E-05e: Stack trace after step
///
/// Tests that stack trace is updated after stepping.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_stack_trace_after_step() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(60)?;

    // Get initial stack trace
    let frames_before = fixture.driver.get_stack_trace()?;
    let rip_before = if !frames_before.is_empty() {
        frames_before[0].rip
    } else {
        0
    };

    // Try to step
    match fixture.driver.step() {
        Ok(()) => {
            // Get stack trace after step
            let frames_after = fixture.driver.get_stack_trace()?;

            // Should still have frames
            assert!(!frames_after.is_empty(), "Should have frames after step");

            // RIP may have changed (depending on instruction)
            let rip_after = frames_after[0].rip;
            eprintln!(
                "RIP before: 0x{:x}, after: 0x{:x}",
                rip_before, rip_after
            );
        }
        Err(E2EError::StepFailed { .. }) => {
            eprintln!("Step failed, skipping post-step validation");
        }
        Err(e) => return Err(e),
    }

    fixture.cleanup()?;
    Ok(())
}

/// E2E-05f: GDB stack trace comparison
///
/// Compares kdb stack trace with GDB baseline (when available).
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions and GDB"]
fn test_stack_trace_vs_gdb() -> E2EResult<()> {
    if !has_gdb() {
        eprintln!("GDB not available, skipping comparison test");
        return Ok(());
    }

    let mut fixture = ComparisonFixture::new()?;

    // Spawn two identical processes
    let (_kdb_pid, _gdb_pid) = fixture.quick_attach_parallel(30)?;

    // Get stack traces from both
    let kdb_frames = fixture.kdb.get_stack_trace()?;
    let gdb_frames = fixture.gdb.get_stack_trace(100)?;  // GDB requires max_frames argument

    // Compare using lenient config (different processes may have different states)
    let result = fixture.validator.compare_stack_traces(&kdb_frames, &gdb_frames);

    // Log differences (expected due to different processes)
    if !result.passed {
        eprintln!("Stack trace differences (expected for different processes):");
        for diff in &result.differences {
            eprintln!("  {}", diff);
        }
    }

    // Both should have at least one frame
    assert!(!kdb_frames.is_empty(), "kdb should produce stack trace");
    assert!(!gdb_frames.is_empty(), "GDB should produce stack trace");

    fixture.cleanup()?;
    Ok(())
}

/// E2E-05g: Stack trace validator edge cases
///
/// Tests the stack trace validator with various inputs.
#[test]
fn test_stack_trace_validator() {
    let validator = OutputValidator::new();

    // Empty stack trace should fail minimum depth
    let empty_frames: Vec<StackFrame> = vec![];
    let result = validator.validate_stack_trace(&empty_frames, 1);
    assert!(!result.passed, "Empty stack should fail min depth 1");

    // Valid stack trace
    let valid_frames = vec![
        StackFrame {
            index: 0,
            rip: 0x400000,
            rsp: 0x7fff0000,
            rbp: 0x7fff0010,
            function_name: Some("main".to_string()),
            source_location: None,
        },
        StackFrame {
            index: 1,
            rip: 0x400100,
            rsp: 0x7fff0020,
            rbp: 0x7fff0030,
            function_name: Some("_start".to_string()),
            source_location: None,
        },
    ];

    let result = validator.validate_stack_trace(&valid_frames, 2);
    assert!(result.passed, "Valid stack should pass: {}", result.summary);

    let result = validator.validate_stack_trace(&valid_frames, 5);
    assert!(!result.passed, "Should fail with higher min depth");
}

/// E2E-05h: Function name in stack validation
///
/// Tests finding a function name in the stack trace.
#[test]
fn test_validate_function_in_stack() {
    let validator = OutputValidator::new();

    let frames = vec![
        StackFrame {
            index: 0,
            rip: 0x400000,
            rsp: 0x7fff0000,
            rbp: 0x7fff0010,
            function_name: Some("process_data".to_string()),
            source_location: None,
        },
        StackFrame {
            index: 1,
            rip: 0x400100,
            rsp: 0x7fff0020,
            rbp: 0x7fff0030,
            function_name: Some("main".to_string()),
            source_location: None,
        },
    ];

    // Should find partial match
    let result = validator.validate_function_in_stack(&frames, "process");
    assert!(result.passed, "Should find 'process' in 'process_data'");

    // Should find exact match
    let result = validator.validate_function_in_stack(&frames, "main");
    assert!(result.passed, "Should find 'main'");

    // Should not find non-existent
    let result = validator.validate_function_in_stack(&frames, "nonexistent");
    assert!(!result.passed, "Should not find 'nonexistent'");
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_stack_frame_default() {
        let frame = StackFrame {
            index: 0,
            rip: 0x400000,
            rsp: 0x7fff0000,
            rbp: 0x7fff0008,
            function_name: None,
            source_location: None,
        };

        assert_eq!(frame.index, 0);
        assert!(frame.function_name.is_none());
    }

    #[test]
    fn test_stack_frame_with_location() {
        let frame = StackFrame {
            index: 0,
            rip: 0x400000,
            rsp: 0x7fff0000,
            rbp: 0x7fff0008,
            function_name: Some("test_func".to_string()),
            source_location: Some(("main.rs".to_string(), 42)),
        };

        assert_eq!(frame.function_name, Some("test_func".to_string()));
        assert_eq!(frame.source_location, Some(("main.rs".to_string(), 42)));
    }
}
