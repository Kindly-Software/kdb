//! Buffer Overflow Target - Memory Corruption Test Target
//!
//! This program demonstrates a controlled stack buffer overflow for testing
//! kdb's memory corruption detection capabilities. The overflow is designed
//! to be:
//! - Deterministic: Same behavior on every run
//! - Observable: Clear memory patterns for debugger detection
//! - Safe: Controlled crash rather than exploitation
//!
//! Framework: T28 Q15-Q21 (Integration testing tier)
//! Purpose: Validate kdb can detect and diagnose buffer overflow crashes
//!
//! Usage:
//!   cargo run --example buffer_overflow_target [mode]
//!
//! Modes:
//!   safe       - No overflow, normal execution (default)
//!   overflow   - Trigger stack buffer overflow
//!   canary     - Overflow with stack canary detection
//!   heap       - Heap buffer overflow variant

use std::env;
use std::sync::atomic::{AtomicU64, Ordering};

/// Canary value for stack corruption detection
/// Pattern: 0xDEADBEEF_CAFEBABE (easily recognizable in memory dumps)
const STACK_CANARY: u64 = 0xDEAD_BEEF_CAFE_BABE;

/// Heap canary for heap corruption detection
const HEAP_CANARY: u64 = 0xFEED_FACE_DEAD_C0DE;

/// Marker bytes for buffer boundaries
const BUFFER_START_MARKER: u8 = 0xAA;
const BUFFER_END_MARKER: u8 = 0xBB;
const OVERFLOW_PATTERN: u8 = 0xCC;

/// Stack frame with vulnerable buffer
/// Layout designed for clear memory inspection:
/// - pre_canary: Detects underflow
/// - buffer: Vulnerable 64-byte buffer
/// - post_canary: Detects overflow
/// - saved_rbp: Simulated saved frame pointer
/// - return_addr: Simulated return address
#[repr(C)]
struct VulnerableStackFrame {
    /// Pre-buffer canary (detects buffer underflow)
    pre_canary: u64,
    /// Marker before buffer
    start_marker: [u8; 8],
    /// Vulnerable buffer (64 bytes)
    buffer: [u8; 64],
    /// Marker after buffer
    end_marker: [u8; 8],
    /// Post-buffer canary (detects buffer overflow)
    post_canary: u64,
    /// Simulated saved RBP (would be corrupted in real overflow)
    saved_rbp: u64,
    /// Simulated return address (would be corrupted for ROP)
    return_addr: u64,
}

impl VulnerableStackFrame {
    /// Create new frame with canaries and markers
    fn new() -> Self {
        let mut frame = Self {
            pre_canary: STACK_CANARY,
            start_marker: [BUFFER_START_MARKER; 8],
            buffer: [0u8; 64],
            end_marker: [BUFFER_END_MARKER; 8],
            post_canary: STACK_CANARY,
            saved_rbp: 0x7FFF_FFFF_0000,
            return_addr: 0x0040_1000,
        };
        // Initialize buffer with pattern for easy identification
        for (i, byte) in frame.buffer.iter_mut().enumerate() {
            *byte = (i & 0xFF) as u8;
        }
        frame
    }

    /// Check if canaries are intact
    fn verify_canaries(&self) -> Result<(), &'static str> {
        if self.pre_canary != STACK_CANARY {
            return Err("Pre-buffer canary corrupted (buffer underflow detected)");
        }
        if self.post_canary != STACK_CANARY {
            return Err("Post-buffer canary corrupted (buffer overflow detected)");
        }
        Ok(())
    }

    /// Get buffer address for debugging
    #[allow(unused)]
    fn buffer_address(&self) -> usize {
        self.buffer.as_ptr() as usize
    }

    /// Get frame layout info for debugging
    fn layout_info(&self) -> FrameLayoutInfo {
        FrameLayoutInfo {
            pre_canary_addr: &self.pre_canary as *const u64 as usize,
            buffer_start: self.buffer.as_ptr() as usize,
            buffer_end: unsafe { self.buffer.as_ptr().add(64) as usize },
            post_canary_addr: &self.post_canary as *const u64 as usize,
            saved_rbp_addr: &self.saved_rbp as *const u64 as usize,
            return_addr_addr: &self.return_addr as *const u64 as usize,
        }
    }
}

/// Frame layout information for debugger inspection
struct FrameLayoutInfo {
    pre_canary_addr: usize,
    buffer_start: usize,
    buffer_end: usize,
    post_canary_addr: usize,
    saved_rbp_addr: usize,
    return_addr_addr: usize,
}

impl std::fmt::Display for FrameLayoutInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Stack Frame Layout:")?;
        writeln!(f, "  Pre-canary:   0x{:016x}", self.pre_canary_addr)?;
        writeln!(f, "  Buffer start: 0x{:016x}", self.buffer_start)?;
        writeln!(f, "  Buffer end:   0x{:016x}", self.buffer_end)?;
        writeln!(f, "  Post-canary:  0x{:016x}", self.post_canary_addr)?;
        writeln!(f, "  Saved RBP:    0x{:016x}", self.saved_rbp_addr)?;
        writeln!(f, "  Return addr:  0x{:016x}", self.return_addr_addr)?;
        Ok(())
    }
}

/// Heap buffer structure for heap overflow testing
struct HeapBuffer {
    /// Pre-buffer canary
    pre_canary: u64,
    /// Heap-allocated buffer
    buffer: Vec<u8>,
    /// Post-buffer canary (stored separately since Vec manages its own memory)
    post_canary: AtomicU64,
    /// Allocation metadata
    alloc_size: usize,
}

impl HeapBuffer {
    fn new(size: usize) -> Self {
        let mut buffer = vec![0u8; size];
        // Initialize with pattern
        for (i, byte) in buffer.iter_mut().enumerate() {
            *byte = (i & 0xFF) as u8;
        }
        Self {
            pre_canary: HEAP_CANARY,
            buffer,
            post_canary: AtomicU64::new(HEAP_CANARY),
            alloc_size: size,
        }
    }

    fn verify_canaries(&self) -> Result<(), &'static str> {
        if self.pre_canary != HEAP_CANARY {
            return Err("Heap pre-canary corrupted");
        }
        if self.post_canary.load(Ordering::Relaxed) != HEAP_CANARY {
            return Err("Heap post-canary corrupted");
        }
        Ok(())
    }
}

/// Simulate vulnerable copy function (like strcpy)
/// This function intentionally overflows the buffer
///
/// # Safety
/// This function is intentionally unsafe and will corrupt memory.
/// Only use in controlled test environments.
#[inline(never)]
unsafe fn vulnerable_copy(dst: *mut u8, src: &[u8], _dst_size: usize) -> usize {
    // Intentionally copy more than dst_size (buffer overflow!)
    let copy_len = src.len(); // Should check against dst_size but doesn't

    for i in 0..copy_len {
        // This will overflow if src.len() > dst_size
        *dst.add(i) = src[i];
    }

    copy_len
}

/// Safe copy function for comparison
#[inline(never)]
fn safe_copy(dst: &mut [u8], src: &[u8]) -> usize {
    let copy_len = dst.len().min(src.len());
    dst[..copy_len].copy_from_slice(&src[..copy_len]);
    copy_len
}

/// Run in safe mode (no overflow)
fn run_safe_mode() {
    println!("=== Buffer Overflow Target: SAFE MODE ===");
    println!("PID: {}", std::process::id());
    println!();

    let mut frame = VulnerableStackFrame::new();
    println!("{}", frame.layout_info());

    // Safe operation: copy within bounds
    let data = b"Hello, this is safe data that fits in the buffer!";
    let copied = safe_copy(&mut frame.buffer, data);

    println!("Copied {} bytes (buffer size: 64)", copied);
    println!("Buffer contents: {:?}", &frame.buffer[..copied.min(64)]);

    // Verify canaries are intact
    match frame.verify_canaries() {
        Ok(()) => println!("\nCanary check: PASSED (no corruption)"),
        Err(e) => println!("\nCanary check: FAILED - {}", e),
    }

    println!("\n=== Safe Mode Complete ===");
}

/// Run in overflow mode (trigger buffer overflow)
fn run_overflow_mode() {
    println!("=== Buffer Overflow Target: OVERFLOW MODE ===");
    println!("PID: {}", std::process::id());
    println!("WARNING: This will cause memory corruption!");
    println!();

    let mut frame = VulnerableStackFrame::new();
    println!("{}", frame.layout_info());

    println!("Pre-overflow canary value:  0x{:016x}", frame.post_canary);
    println!("Pre-overflow saved_rbp:     0x{:016x}", frame.saved_rbp);
    println!("Pre-overflow return_addr:   0x{:016x}", frame.return_addr);
    println!();

    // Create overflow data (96 bytes into 64-byte buffer)
    // This will overflow into: end_marker (8) + post_canary (8) + saved_rbp (8) + return_addr (8) = 32 bytes overflow
    let overflow_data: Vec<u8> = (0..96).map(|i| {
        if i < 64 {
            (i & 0xFF) as u8  // Normal buffer data
        } else {
            OVERFLOW_PATTERN  // Overflow pattern (0xCC)
        }
    }).collect();

    println!("Triggering overflow: writing {} bytes to 64-byte buffer...", overflow_data.len());

    // INTENTIONAL BUFFER OVERFLOW
    unsafe {
        let copied = vulnerable_copy(
            frame.buffer.as_mut_ptr(),
            &overflow_data,
            64,  // Buffer is only 64 bytes
        );
        println!("Wrote {} bytes", copied);
    }

    println!();
    println!("Post-overflow canary value: 0x{:016x}", frame.post_canary);
    println!("Post-overflow saved_rbp:    0x{:016x}", frame.saved_rbp);
    println!("Post-overflow return_addr:  0x{:016x}", frame.return_addr);
    println!();

    // Check for corruption
    match frame.verify_canaries() {
        Ok(()) => println!("Canary check: PASSED (unexpected - overflow should have corrupted!)"),
        Err(e) => {
            println!("Canary check: FAILED - {}", e);
            println!("\nMemory corruption detected by canary!");
            println!("In a real exploit, the return address would now point to attacker code.");
        }
    }

    // Dump memory around overflow
    println!("\nMemory dump (end of buffer + overflow region):");
    let end_region = &frame.buffer[56..64];
    print!("  Buffer[56..64]: ");
    for b in end_region {
        print!("{:02x} ", b);
    }
    println!();

    print!("  End marker:     ");
    for b in &frame.end_marker {
        print!("{:02x} ", b);
    }
    println!();

    println!("\n=== Overflow Mode Complete ===");
    println!("\nTo debug with kdb:");
    println!("  1. Run this program: cargo run --example buffer_overflow_target overflow");
    println!("  2. Attach kdb to PID {}", std::process::id());
    println!("  3. Set watchpoint on post_canary address");
    println!("  4. Observe corruption pattern 0xCC in overflow region");
}

/// Run with canary detection demonstration
fn run_canary_mode() {
    println!("=== Buffer Overflow Target: CANARY DETECTION MODE ===");
    println!("PID: {}", std::process::id());
    println!();

    let mut frame = VulnerableStackFrame::new();
    println!("{}", frame.layout_info());

    // Demonstrate canary detection at various overflow sizes
    let test_sizes = [64, 72, 80, 88, 96];

    for size in test_sizes {
        // Reset frame
        frame = VulnerableStackFrame::new();

        let overflow_data: Vec<u8> = (0..size).map(|_| OVERFLOW_PATTERN).collect();

        unsafe {
            vulnerable_copy(frame.buffer.as_mut_ptr(), &overflow_data, 64);
        }

        let result = frame.verify_canaries();
        println!(
            "  Write {} bytes (overflow {}): {}",
            size,
            if size > 64 { size - 64 } else { 0 },
            if result.is_ok() { "SAFE" } else { "CORRUPTED" }
        );
    }

    println!("\n=== Canary Detection Mode Complete ===");
}

/// Run heap overflow mode
fn run_heap_mode() {
    println!("=== Buffer Overflow Target: HEAP MODE ===");
    println!("PID: {}", std::process::id());
    println!();

    let mut heap_buf = HeapBuffer::new(64);

    println!("Heap buffer allocated: {} bytes", heap_buf.alloc_size);
    println!("Buffer address: {:p}", heap_buf.buffer.as_ptr());
    println!();

    // Demonstrate heap overflow
    // Note: Vec's internal bounds checking makes this harder to demonstrate
    // In real code, this would use raw pointers

    println!("Heap overflow is more complex due to allocator metadata.");
    println!("In production, use tools like AddressSanitizer or kdb's heap tracking.");

    heap_buf.verify_canaries().expect("Canaries should be intact");
    println!("\nHeap canaries verified: OK");

    // Simulate corruption via unsafe pointer access
    println!("\nSimulating heap corruption via pointer arithmetic...");

    unsafe {
        // Write past the end of the buffer
        let end_ptr = heap_buf.buffer.as_mut_ptr().add(heap_buf.alloc_size);
        // This is UB but demonstrates what heap overflow looks like
        for i in 0..8 {
            std::ptr::write_volatile(end_ptr.add(i), OVERFLOW_PATTERN);
        }
    }

    println!("Wrote 8 bytes past buffer end (OVERFLOW_PATTERN = 0x{:02x})", OVERFLOW_PATTERN);
    println!("\n=== Heap Mode Complete ===");
}

/// Wait for debugger attachment
fn wait_for_debugger() {
    println!("\nWaiting for debugger attachment...");
    println!("PID: {}", std::process::id());
    println!("Press Enter to continue or attach debugger now.");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("safe");

    println!("Buffer Overflow Target - Memory Corruption Test");
    println!("================================================");
    println!("PID: {}", std::process::id());
    println!("Mode: {}", mode);
    println!();

    // Allow debugger attachment for all modes
    if env::var("KDB_WAIT").is_ok() {
        wait_for_debugger();
    }

    match mode {
        "safe" => run_safe_mode(),
        "overflow" => run_overflow_mode(),
        "canary" => run_canary_mode(),
        "heap" => run_heap_mode(),
        "wait" => {
            println!("Waiting mode: Process will sleep for debugger attachment.");
            wait_for_debugger();
            run_safe_mode();
        }
        _ => {
            eprintln!("Unknown mode: {}", mode);
            eprintln!("Valid modes: safe, overflow, canary, heap, wait");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_copy_within_bounds() {
        let mut frame = VulnerableStackFrame::new();
        let data = b"Hello";
        let copied = safe_copy(&mut frame.buffer, data);

        assert_eq!(copied, 5);
        assert!(frame.verify_canaries().is_ok());
    }

    #[test]
    fn test_canary_values() {
        let frame = VulnerableStackFrame::new();
        assert_eq!(frame.pre_canary, STACK_CANARY);
        assert_eq!(frame.post_canary, STACK_CANARY);
    }

    #[test]
    fn test_overflow_detection() {
        let mut frame = VulnerableStackFrame::new();

        // Overflow by 16 bytes (should corrupt end_marker and post_canary)
        unsafe {
            let overflow_data = [OVERFLOW_PATTERN; 80];
            vulnerable_copy(frame.buffer.as_mut_ptr(), &overflow_data, 64);
        }

        // Canary should be corrupted
        assert!(frame.verify_canaries().is_err());
    }

    #[test]
    fn test_frame_layout() {
        let frame = VulnerableStackFrame::new();
        let info = frame.layout_info();

        // Verify layout is contiguous
        assert!(info.buffer_start > info.pre_canary_addr);
        assert!(info.buffer_end > info.buffer_start);
        assert!(info.post_canary_addr > info.buffer_end);
    }
}
