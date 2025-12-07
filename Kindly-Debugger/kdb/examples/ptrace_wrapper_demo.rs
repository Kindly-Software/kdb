//! PtraceWrapperCapsule Demo
//!
//! Demonstrates T1 Atomic ptrace syscall wrapper operations.
//!
//! # Platform
//! Linux only (requires ptrace syscalls)
//!
//! # Permissions
//! Requires CAP_SYS_PTRACE capability or root privileges
//!
//! # Usage
//! ```bash
//! # Compile
//! cargo build --example ptrace_wrapper_demo --features derive
//!
//! # Run as root (required for ptrace)
//! sudo ./target/debug/examples/ptrace_wrapper_demo <pid>
//! ```

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Error: This example requires Linux (ptrace syscalls)");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use kdb::ptrace::{ProcessState, PtraceWrapperCapsule};
    use std::env;

    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <pid>", args[0]);
        eprintln!("\nExample: sudo {} 1234", args[0]);
        eprintln!("\nNote: Requires CAP_SYS_PTRACE capability or root privileges");
        std::process::exit(1);
    }

    let pid: i32 = args[1].parse().map_err(|_| {
        eprintln!("Error: Invalid PID '{}'", args[1]);
        std::process::exit(1);
    })?;

    println!("=== PtraceWrapperCapsule Demo ===");
    println!("Target PID: {}", pid);
    println!();

    // Create wrapper capsule
    let wrapper = PtraceWrapperCapsule::new();
    println!("✓ Created PtraceWrapperCapsule (256 bytes, cache-aligned)");
    println!("  Initial state: {:?}", wrapper.get_state());
    println!(
        "  Size: {} bytes",
        std::mem::size_of::<PtraceWrapperCapsule>()
    );
    println!(
        "  Alignment: {} bytes",
        std::mem::align_of::<PtraceWrapperCapsule>()
    );
    println!();

    // Attach to process
    println!("Attaching to process {}...", pid);
    match wrapper.attach(pid) {
        Ok(()) => {
            println!("✓ Successfully attached");
            println!("  State: {:?}", wrapper.get_state());
            println!("  PID: {}", wrapper.get_pid());
            println!("  Generation: {}", wrapper.get_generation());
        }
        Err(e) => {
            eprintln!("✗ Failed to attach: {}", e);
            eprintln!("\nCommon causes:");
            eprintln!("  - Process doesn't exist");
            eprintln!("  - Insufficient permissions (need CAP_SYS_PTRACE or root)");
            eprintln!("  - Process is already being traced");
            return Err(e.into());
        }
    }
    println!();

    // Get registers (only works on x86_64)
    #[cfg(target_arch = "x86_64")]
    {
        println!("Reading CPU registers...");
        match wrapper.getregs() {
            Ok(regs) => {
                println!("✓ Successfully read registers");
                println!("  RIP (instruction pointer): 0x{:016x}", regs.rip);
                println!("  RSP (stack pointer):       0x{:016x}", regs.rsp);
                println!("  RBP (frame pointer):       0x{:016x}", regs.rbp);
                println!("  RAX:                       0x{:016x}", regs.rax);
            }
            Err(e) => {
                eprintln!("✗ Failed to read registers: {}", e);
            }
        }
        println!();
    }

    // Read memory (try to read from stack pointer)
    #[cfg(target_arch = "x86_64")]
    {
        println!("Reading memory from stack...");
        match wrapper.getregs() {
            Ok(regs) => {
                let stack_addr = regs.rsp;
                match wrapper.peek_data(stack_addr) {
                    Ok(value) => {
                        println!("✓ Successfully read memory");
                        println!("  Address: 0x{:016x}", stack_addr);
                        println!("  Value:   0x{:016x}", value);
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to read memory: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("✗ Could not get stack pointer: {}", e);
            }
        }
        println!();
    }

    // Check process state
    println!("Process state:");
    println!("  State: {:?}", wrapper.get_state());
    println!("  Is stopped: {}", wrapper.is_stopped());
    println!("  Operation count: {}", wrapper.get_operation_count());
    println!("  Error count: {}", wrapper.get_error_count());
    println!("  Generation: {}", wrapper.get_generation());
    println!();

    // Continue process
    println!("Continuing process...");
    match wrapper.cont() {
        Ok(()) => {
            println!("✓ Process continued");
            println!("  State: {:?}", wrapper.get_state());
        }
        Err(e) => {
            eprintln!("✗ Failed to continue: {}", e);
        }
    }
    println!();

    // Wait for process to stop (this will block indefinitely unless process hits breakpoint)
    println!("Waiting for process to stop (press Ctrl+C to interrupt)...");
    println!("Note: Process will only stop if it receives a signal or hits a breakpoint");
    println!("      In a real debugger, you would set breakpoints first");
    println!();

    // Detach from process
    println!("Detaching from process...");
    match wrapper.detach() {
        Ok(()) => {
            println!("✓ Successfully detached");
            println!("  State: {:?}", wrapper.get_state());
            println!("  Final operation count: {}", wrapper.get_operation_count());
        }
        Err(e) => {
            eprintln!("✗ Failed to detach: {}", e);
            return Err(e.into());
        }
    }
    println!();

    println!("=== Demo Complete ===");
    println!("Metrics:");
    println!("  Total operations: {}", wrapper.get_operation_count());
    println!("  Total errors: {}", wrapper.get_error_count());
    println!("  Final generation: {}", wrapper.get_generation());

    Ok(())
}
