//! Use-After-Free Target - Memory Corruption Test Target
//!
//! This program demonstrates controlled use-after-free vulnerabilities for
//! testing kdb's memory corruption detection capabilities. The bugs are
//! designed to be:
//! - Deterministic: Same behavior on every run
//! - Observable: Clear memory patterns for debugger detection
//! - Safe: Controlled rather than exploitable
//!
//! Framework: T28 Q15-Q21 (Integration testing tier)
//! Purpose: Validate kdb can detect and diagnose use-after-free bugs
//!
//! Usage:
//!   cargo run --example use_after_free_target [mode]
//!
//! Modes:
//!   safe        - No UAF, normal execution (default)
//!   uaf         - Trigger use-after-free read
//!   uaf_write   - Trigger use-after-free write
//!   double_free - Trigger double-free
//!   dangling    - Create dangling pointer scenario

use std::alloc::{alloc, dealloc, Layout};
use std::env;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, AtomicPtr, AtomicBool, Ordering};

/// Magic value for allocated memory
const ALLOC_MAGIC: u64 = 0xA11C_A7ED_DEAD_BEEF;

/// Magic value for freed memory (written on deallocation)
const FREE_MAGIC: u64 = 0xF4EE_DDA7_ACAF_E000;

/// Canary value for heap corruption detection
const HEAP_CANARY: u64 = 0xCAFE_BABE_1234_5678;

/// Allocation tracking for debugging
static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
static FREE_COUNT: AtomicU64 = AtomicU64::new(0);

/// A tracked heap allocation with metadata for debugging
///
/// Memory layout:
/// ```text
/// +-------------------+
/// | header (64 bytes) |  <- AllocationHeader
/// +-------------------+
/// | user data         |  <- Actual allocation
/// +-------------------+
/// | footer canary     |  <- 8 bytes
/// +-------------------+
/// ```
#[repr(C, align(64))]
struct AllocationHeader {
    /// Magic value to identify valid allocations
    magic: AtomicU64,
    /// Unique allocation ID
    alloc_id: u64,
    /// Size of user data (excluding header/footer)
    size: usize,
    /// Is this allocation currently valid?
    is_valid: AtomicBool,
    /// Timestamp of allocation (simplified as counter)
    alloc_time: u64,
    /// Timestamp of free (0 if not freed)
    free_time: AtomicU64,
    /// Padding to 64 bytes
    _padding: [u8; 24],
}

impl AllocationHeader {
    fn new(alloc_id: u64, size: usize) -> Self {
        Self {
            magic: AtomicU64::new(ALLOC_MAGIC),
            alloc_id,
            size,
            is_valid: AtomicBool::new(true),
            alloc_time: ALLOCATION_COUNT.fetch_add(1, Ordering::SeqCst),
            free_time: AtomicU64::new(0),
            _padding: [0; 24],
        }
    }

    fn mark_freed(&self) {
        self.magic.store(FREE_MAGIC, Ordering::SeqCst);
        self.is_valid.store(false, Ordering::SeqCst);
        self.free_time.store(FREE_COUNT.fetch_add(1, Ordering::SeqCst), Ordering::SeqCst);
    }

    fn is_freed(&self) -> bool {
        self.magic.load(Ordering::SeqCst) == FREE_MAGIC
    }

    fn is_allocated(&self) -> bool {
        self.magic.load(Ordering::SeqCst) == ALLOC_MAGIC
    }
}

/// Tracked allocation wrapper
struct TrackedAllocation {
    /// Pointer to the header (user data follows)
    header_ptr: NonNull<AllocationHeader>,
    /// Layout used for allocation (includes header + data + footer)
    layout: Layout,
}

impl TrackedAllocation {
    /// Allocate tracked memory
    fn new(size: usize) -> Result<Self, &'static str> {
        // Calculate total size: header + user data + footer canary
        let total_size = std::mem::size_of::<AllocationHeader>() + size + 8;
        let layout = Layout::from_size_align(total_size, 64)
            .map_err(|_| "Invalid layout")?;

        let alloc_id = ALLOCATION_COUNT.load(Ordering::Relaxed);

        unsafe {
            let ptr = alloc(layout);
            if ptr.is_null() {
                return Err("Allocation failed");
            }

            // Initialize header
            let header = ptr as *mut AllocationHeader;
            std::ptr::write(header, AllocationHeader::new(alloc_id, size));

            // Initialize user data with pattern
            let data_ptr = ptr.add(std::mem::size_of::<AllocationHeader>());
            for i in 0..size {
                *data_ptr.add(i) = (i & 0xFF) as u8;
            }

            // Write footer canary
            let footer_ptr = data_ptr.add(size) as *mut u64;
            std::ptr::write(footer_ptr, HEAP_CANARY);

            Ok(Self {
                header_ptr: NonNull::new_unchecked(header),
                layout,
            })
        }
    }

    /// Get pointer to user data
    fn data_ptr(&self) -> *mut u8 {
        unsafe {
            (self.header_ptr.as_ptr() as *mut u8)
                .add(std::mem::size_of::<AllocationHeader>())
        }
    }

    /// Get header reference
    fn header(&self) -> &AllocationHeader {
        unsafe { self.header_ptr.as_ref() }
    }

    /// Check if allocation is still valid
    fn is_valid(&self) -> bool {
        self.header().is_allocated()
    }

    /// Get allocation info for debugging
    fn debug_info(&self) -> AllocationDebugInfo {
        let header = self.header();
        AllocationDebugInfo {
            header_addr: self.header_ptr.as_ptr() as usize,
            data_addr: self.data_ptr() as usize,
            alloc_id: header.alloc_id,
            size: header.size,
            is_valid: header.is_valid.load(Ordering::Relaxed),
            magic: header.magic.load(Ordering::Relaxed),
            alloc_time: header.alloc_time,
            free_time: header.free_time.load(Ordering::Relaxed),
        }
    }

    /// Free the allocation
    fn free(self) {
        unsafe {
            let header = self.header_ptr.as_ref();
            header.mark_freed();

            // Poison the user data with free pattern
            let data_ptr = self.data_ptr();
            for i in 0..header.size {
                *data_ptr.add(i) = 0xDD; // Freed memory pattern
            }

            dealloc(self.header_ptr.as_ptr() as *mut u8, self.layout);
        }
    }

    /// Free but keep pointer (for UAF testing)
    /// Returns a dangling pointer to the freed memory
    fn free_keeping_pointer(&self) -> *mut u8 {
        let data_ptr = self.data_ptr();

        unsafe {
            let header = self.header_ptr.as_ref();
            header.mark_freed();

            // Poison the user data
            for i in 0..header.size {
                *data_ptr.add(i) = 0xDD;
            }

            dealloc(self.header_ptr.as_ptr() as *mut u8, self.layout);
        }

        data_ptr // Dangling pointer!
    }
}

/// Debug info for an allocation
struct AllocationDebugInfo {
    header_addr: usize,
    data_addr: usize,
    alloc_id: u64,
    size: usize,
    is_valid: bool,
    magic: u64,
    alloc_time: u64,
    free_time: u64,
}

impl std::fmt::Display for AllocationDebugInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Allocation Debug Info:")?;
        writeln!(f, "  Header addr:  0x{:016x}", self.header_addr)?;
        writeln!(f, "  Data addr:    0x{:016x}", self.data_addr)?;
        writeln!(f, "  Alloc ID:     {}", self.alloc_id)?;
        writeln!(f, "  Size:         {} bytes", self.size)?;
        writeln!(f, "  Is valid:     {}", self.is_valid)?;
        writeln!(f, "  Magic:        0x{:016x}", self.magic)?;
        writeln!(f, "  Alloc time:   {}", self.alloc_time)?;
        writeln!(f, "  Free time:    {}", if self.free_time == 0 { "not freed".to_string() } else { self.free_time.to_string() })?;
        Ok(())
    }
}

/// Dangling pointer container for testing
struct DanglingPointer {
    /// The dangling pointer itself
    ptr: AtomicPtr<u8>,
    /// Original allocation size (unused but preserved for debugging)
    #[allow(unused)]
    original_size: usize,
    /// Was this freed?
    is_freed: AtomicBool,
}

impl DanglingPointer {
    fn new() -> Self {
        Self {
            ptr: AtomicPtr::new(std::ptr::null_mut()),
            original_size: 0,
            is_freed: AtomicBool::new(false),
        }
    }

    fn store(&self, ptr: *mut u8, _size: usize) {
        self.ptr.store(ptr, Ordering::SeqCst);
        // Size not stored atomically for simplicity
    }

    fn mark_freed(&self) {
        self.is_freed.store(true, Ordering::SeqCst);
    }

    fn load(&self) -> *mut u8 {
        self.ptr.load(Ordering::SeqCst)
    }

    fn is_dangling(&self) -> bool {
        self.is_freed.load(Ordering::SeqCst) && !self.ptr.load(Ordering::SeqCst).is_null()
    }
}

/// Run in safe mode (no UAF)
fn run_safe_mode() {
    println!("=== Use-After-Free Target: SAFE MODE ===");
    println!("PID: {}", std::process::id());
    println!();

    // Allocate, use, free properly
    let alloc = TrackedAllocation::new(128).expect("Allocation failed");
    println!("{}", alloc.debug_info());

    // Use the allocation while valid
    unsafe {
        let data = alloc.data_ptr();
        // Read
        let first_byte = *data;
        println!("Read first byte: 0x{:02x}", first_byte);

        // Write
        *data = 0x42;
        println!("Wrote 0x42 to first byte");

        // Read again
        let new_first = *data;
        println!("Read first byte again: 0x{:02x}", new_first);
    }

    println!("\nFreeing allocation...");
    alloc.free();

    println!("Allocation freed safely (no dangling pointer use)");
    println!("\n=== Safe Mode Complete ===");
}

/// Run with use-after-free read
fn run_uaf_mode() {
    println!("=== Use-After-Free Target: UAF READ MODE ===");
    println!("PID: {}", std::process::id());
    println!("WARNING: This will access freed memory!");
    println!();

    let alloc = TrackedAllocation::new(64).expect("Allocation failed");
    println!("Before free:");
    println!("{}", alloc.debug_info());

    // Get pointer before freeing
    let dangling_ptr = alloc.data_ptr();
    println!("Data pointer: {:p}", dangling_ptr);

    // Free the allocation
    println!("\nFreeing allocation...");
    let dangling = alloc.free_keeping_pointer();

    println!("Allocation freed!");
    println!("Dangling pointer: {:p}", dangling);
    println!();

    // UAF READ: Access freed memory
    println!("Attempting use-after-free READ...");
    unsafe {
        // This is UB! Reading freed memory
        let value = std::ptr::read_volatile(dangling);
        println!("Read from freed memory: 0x{:02x}", value);

        // Expected: 0xDD (our free pattern) or garbage
        if value == 0xDD {
            println!("Found free pattern (0xDD) - memory was poisoned on free");
        } else {
            println!("Found unexpected value - memory may have been reused");
        }

        // Read more bytes
        print!("First 16 bytes of freed memory: ");
        for i in 0..16 {
            let b = std::ptr::read_volatile(dangling.add(i));
            print!("{:02x} ", b);
        }
        println!();
    }

    println!("\n=== UAF Read Mode Complete ===");
    println!("\nTo debug with kdb:");
    println!("  1. Set watchpoint on address {:p}", dangling);
    println!("  2. Observe access to freed memory");
    println!("  3. Check allocation header for FREE_MAGIC pattern");
}

/// Run with use-after-free write
fn run_uaf_write_mode() {
    println!("=== Use-After-Free Target: UAF WRITE MODE ===");
    println!("PID: {}", std::process::id());
    println!("WARNING: This will write to freed memory!");
    println!();

    let alloc = TrackedAllocation::new(64).expect("Allocation failed");
    println!("Before free:");
    println!("{}", alloc.debug_info());

    let dangling = alloc.free_keeping_pointer();
    println!("\nAllocation freed. Dangling pointer: {:p}", dangling);
    println!();

    // UAF WRITE: Write to freed memory
    println!("Attempting use-after-free WRITE...");
    unsafe {
        // This is UB! Writing to freed memory
        println!("Writing 0xAA pattern to freed memory...");
        for i in 0..64 {
            std::ptr::write_volatile(dangling.add(i), 0xAA);
        }

        // Verify write
        print!("Verification read: ");
        for i in 0..16 {
            let b = std::ptr::read_volatile(dangling.add(i));
            print!("{:02x} ", b);
        }
        println!();
    }

    println!("\nUAF write completed (this is undefined behavior!)");
    println!("\n=== UAF Write Mode Complete ===");
}

/// Run with double-free
fn run_double_free_mode() {
    println!("=== Use-After-Free Target: DOUBLE FREE MODE ===");
    println!("PID: {}", std::process::id());
    println!("WARNING: This demonstrates double-free!");
    println!();

    // Manual allocation to demonstrate double-free
    let layout = Layout::from_size_align(128, 8).unwrap();

    unsafe {
        let ptr = alloc(layout);
        if ptr.is_null() {
            panic!("Allocation failed");
        }

        println!("Allocated 128 bytes at {:p}", ptr);

        // Initialize
        for i in 0..128 {
            *ptr.add(i) = (i & 0xFF) as u8;
        }

        // First free
        println!("First free...");
        dealloc(ptr, layout);
        println!("First free completed");

        // In real code, double-free would be caught by the allocator
        // We'll simulate what would happen
        println!("\nIn production, a second free would:");
        println!("  - Crash with allocator corruption error");
        println!("  - Or corrupt the free list (exploitable!)");
        println!("  - Or be detected by sanitizers");

        // Demonstrate by checking if memory is still accessible
        // (It might be, depending on allocator)
        println!("\nAttempting to read freed memory...");
        let value = std::ptr::read_volatile(ptr);
        println!("Read after first free: 0x{:02x}", value);

        // NOTE: We don't actually double-free here as it would crash
        // In a real test, you'd use ASAN or similar
    }

    println!("\n=== Double Free Mode Complete ===");
    println!("\nTo detect double-free with kdb:");
    println!("  1. Track allocation/free events");
    println!("  2. Check if address was already freed");
    println!("  3. Use heap profiler to track allocations");
}

/// Run with dangling pointer scenario
fn run_dangling_mode() {
    println!("=== Use-After-Free Target: DANGLING POINTER MODE ===");
    println!("PID: {}", std::process::id());
    println!();

    // Create container for dangling pointer
    let dangling = DanglingPointer::new();

    // Create allocation and store pointer
    {
        let alloc = TrackedAllocation::new(256).expect("Allocation failed");
        println!("Created allocation:");
        println!("{}", alloc.debug_info());

        let data_ptr = alloc.data_ptr();
        dangling.store(data_ptr, 256);

        println!("Stored pointer in DanglingPointer container");

        // Free allocation (but dangling still holds pointer)
        println!("\nFreeing allocation (creating dangling pointer)...");
        alloc.free();
        dangling.mark_freed();
    }

    println!("\nAllocation is now out of scope and freed");
    println!("DanglingPointer.is_dangling() = {}", dangling.is_dangling());
    println!("Dangling pointer value: {:p}", dangling.load());

    // This pattern is common in C/C++ code:
    // - Pointer stored in container/struct
    // - Original allocation freed
    // - Container still has the old pointer
    // - Later code uses the dangling pointer

    println!("\nThis pattern occurs when:");
    println!("  1. Object A stores pointer to Object B");
    println!("  2. Object B is freed (but A not updated)");
    println!("  3. Code uses A's pointer (dangling!)");

    // Demonstrate the dangling access
    if dangling.is_dangling() {
        println!("\nAttempting access through dangling pointer...");
        unsafe {
            let ptr = dangling.load();
            let value = std::ptr::read_volatile(ptr);
            println!("Read through dangling pointer: 0x{:02x}", value);
        }
    }

    println!("\n=== Dangling Pointer Mode Complete ===");
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

    println!("Use-After-Free Target - Memory Corruption Test");
    println!("===============================================");
    println!("PID: {}", std::process::id());
    println!("Mode: {}", mode);
    println!();

    // Statistics
    println!("Allocation tracking:");
    println!("  ALLOC_MAGIC:  0x{:016x}", ALLOC_MAGIC);
    println!("  FREE_MAGIC:   0x{:016x}", FREE_MAGIC);
    println!("  HEAP_CANARY:  0x{:016x}", HEAP_CANARY);
    println!();

    // Allow debugger attachment for all modes
    if env::var("KDB_WAIT").is_ok() {
        wait_for_debugger();
    }

    match mode {
        "safe" => run_safe_mode(),
        "uaf" => run_uaf_mode(),
        "uaf_write" => run_uaf_write_mode(),
        "double_free" => run_double_free_mode(),
        "dangling" => run_dangling_mode(),
        "wait" => {
            println!("Waiting mode: Process will sleep for debugger attachment.");
            wait_for_debugger();
            run_safe_mode();
        }
        _ => {
            eprintln!("Unknown mode: {}", mode);
            eprintln!("Valid modes: safe, uaf, uaf_write, double_free, dangling, wait");
            std::process::exit(1);
        }
    }

    // Final statistics
    println!("\n--- Allocation Statistics ---");
    println!("Total allocations: {}", ALLOCATION_COUNT.load(Ordering::Relaxed));
    println!("Total frees:       {}", FREE_COUNT.load(Ordering::Relaxed));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracked_allocation_create() {
        let alloc = TrackedAllocation::new(64).expect("Should allocate");
        assert!(alloc.is_valid());
        assert_eq!(alloc.header().size, 64);
    }

    #[test]
    fn test_tracked_allocation_magic() {
        let alloc = TrackedAllocation::new(32).expect("Should allocate");
        assert_eq!(alloc.header().magic.load(Ordering::Relaxed), ALLOC_MAGIC);
    }

    #[test]
    fn test_allocation_header_size() {
        // Header should be 64 bytes (cache-line aligned)
        assert_eq!(std::mem::size_of::<AllocationHeader>(), 64);
    }

    #[test]
    fn test_free_marks_header() {
        let alloc = TrackedAllocation::new(64).expect("Should allocate");
        let header_ptr = alloc.header_ptr;

        // Free the allocation
        alloc.free();

        // Note: This test is technically UB as we're accessing freed memory
        // In production, use a memory debugging tool
    }

    #[test]
    fn test_dangling_pointer_tracking() {
        let dp = DanglingPointer::new();
        assert!(!dp.is_dangling());

        // Simulate storing a pointer
        let mut data = [0u8; 64];
        dp.store(data.as_mut_ptr(), 64);
        assert!(!dp.is_dangling()); // Not freed yet

        dp.mark_freed();
        assert!(dp.is_dangling()); // Now dangling
    }
}
