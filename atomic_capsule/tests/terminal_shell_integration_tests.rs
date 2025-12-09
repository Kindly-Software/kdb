//! T28 Q15-Q21: Integration Tests for TerminalShellCapsule
//!
//! ## Test Coverage
//!
//! - Q15: Shell process spawning (bash, sh, echo)
//! - Q16: PTY I/O (read/write/flush)
//! - Q17: Signal handling (interrupt, suspend, resume)
//! - Q18: Terminal resize
//! - Q19: Process lifecycle (spawn, wait, kill)
//! - Q20: Environment variables
//! - Q21: EOF handling
//!
//! ## Notes
//!
//! Tests marked with `#[ignore]` require actual shell processes and should be run manually:
//! ```bash
//! cargo test --test terminal_shell_integration_tests --features tui-terminal,terminal-unix -- --ignored
//! ```

#![cfg(all(unix, feature = "tui-terminal", feature = "terminal-unix"))]

use atomic_capsule::terminal::shell::{TerminalShellCapsule, ShellState, Signal};
use std::time::Duration;
use std::thread;

// ============================================================================
// Q15: SHELL PROCESS SPAWNING
// ============================================================================

#[test]
#[ignore] // Requires actual shell
fn q15_spawn_bash_simple() {
    let shell = TerminalShellCapsule::new();

    // Spawn bash
    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Verify running
    assert!(shell.is_running());
    assert_eq!(shell.state(), ShellState::Running);
    assert!(shell.generation() > 0);

    // Cleanup
    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q15_spawn_sh_simple() {
    let shell = TerminalShellCapsule::new();

    // Spawn sh
    shell.spawn("/bin/sh", 80, 24).expect("spawn failed");

    assert!(shell.is_running());
    assert_eq!(shell.state(), ShellState::Running);

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q15_spawn_echo_exits() {
    let shell = TerminalShellCapsule::new();

    // Spawn echo (exits immediately)
    shell.spawn("/bin/echo", 80, 24).expect("spawn failed");

    // Give time to exit
    thread::sleep(Duration::from_millis(50));

    // Wait for exit
    let code = shell.wait().expect("wait failed");
    assert_eq!(code, 0);
    assert_eq!(shell.state(), ShellState::Exited);
    assert_eq!(shell.exit_code(), Some(0));
}

#[test]
#[ignore] // Requires actual shell
fn q15_spawn_twice_fails() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Try to spawn again
    let result = shell.spawn("/bin/bash", 80, 24);
    assert!(result.is_err());

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q15_spawn_custom_size() {
    let shell = TerminalShellCapsule::new();

    // Spawn with 120x40 terminal
    shell.spawn("/bin/bash", 120, 40).expect("spawn failed");

    assert_eq!(shell.size(), (120, 40));

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

// ============================================================================
// Q16: PTY I/O
// ============================================================================

#[test]
#[ignore] // Requires actual shell
fn q16_write_read_echo() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Write command
    let cmd = b"echo hello\n";
    let written = shell.write(cmd).expect("write failed");
    assert_eq!(written, cmd.len());
    assert!(shell.bytes_written() >= cmd.len() as u64);

    shell.flush().expect("flush failed");

    // Give shell time to process
    thread::sleep(Duration::from_millis(200));

    // Read output
    let mut buf = [0u8; 1024];
    let n = shell.read(&mut buf).expect("read failed");

    assert!(n > 0, "Expected to read output");
    assert!(shell.bytes_read() > 0);

    // Output should contain "hello"
    let output = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(output.contains("hello"), "Output: {:?}", output);

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q16_multiple_writes() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Write multiple commands
    shell.write(b"echo one\n").expect("write failed");
    shell.write(b"echo two\n").expect("write failed");
    shell.write(b"echo three\n").expect("write failed");

    thread::sleep(Duration::from_millis(300));

    // Read output
    let mut buf = [0u8; 2048];
    let n = shell.read(&mut buf).expect("read failed");

    let output = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(output.contains("one"));
    assert!(output.contains("two"));
    assert!(output.contains("three"));

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q16_buffer_metrics_updated() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    let initial_written = shell.bytes_written();
    let initial_read = shell.bytes_read();

    shell.write(b"echo test\n").expect("write failed");
    thread::sleep(Duration::from_millis(200));

    let mut buf = [0u8; 1024];
    let _ = shell.read(&mut buf).expect("read failed");

    assert!(shell.bytes_written() > initial_written);
    assert!(shell.bytes_read() > initial_read);
    assert!(shell.last_activity_ns() > 0);

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q16_incremental_reads() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Write large command
    shell.write(b"seq 1 100\n").expect("write failed");

    thread::sleep(Duration::from_millis(200));

    // Read in small chunks
    let mut total = 0;
    let mut buf = [0u8; 64];

    for _ in 0..10 {
        if let Ok(n) = shell.read(&mut buf) {
            total += n;
            if n == 0 {
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(total > 0, "Expected to read output");

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

// ============================================================================
// Q17: SIGNAL HANDLING
// ============================================================================

#[test]
#[ignore] // Requires actual shell
fn q17_interrupt_signal() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Send interrupt (Ctrl+C)
    shell.interrupt().expect("interrupt failed");

    // Shell should still be running (signal sent to foreground job)
    assert!(shell.is_running());

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q17_suspend_resume() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Suspend
    shell.suspend().expect("suspend failed");
    assert_eq!(shell.state(), ShellState::Stopped);

    thread::sleep(Duration::from_millis(100));

    // Resume
    shell.resume().expect("resume failed");
    assert_eq!(shell.state(), ShellState::Running);

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q17_kill_terminates() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Kill
    shell.kill().expect("kill failed");

    // Wait should succeed
    let code = shell.wait().expect("wait failed");
    assert!(code != 0); // Killed process has non-zero exit
    assert_eq!(shell.state(), ShellState::Exited);
}

#[test]
#[ignore] // Requires actual shell
fn q17_signal_enum_values() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Test different signals
    shell.signal(Signal::Interrupt).expect("SIGINT failed");
    thread::sleep(Duration::from_millis(10));

    shell.signal(Signal::WindowChange).expect("SIGWINCH failed");
    thread::sleep(Duration::from_millis(10));

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

// ============================================================================
// Q18: TERMINAL RESIZE
// ============================================================================

#[test]
#[ignore] // Requires actual shell
fn q18_resize_terminal() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Initial size
    assert_eq!(shell.size(), (80, 24));

    // Resize to 120x40
    shell.resize(120, 40).expect("resize failed");
    assert_eq!(shell.size(), (120, 40));

    // Resize to 100x30
    shell.resize(100, 30).expect("resize failed");
    assert_eq!(shell.size(), (100, 30));

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q18_resize_sends_sigwinch() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Resize should send SIGWINCH to shell
    shell.resize(120, 40).expect("resize failed");

    // Shell should still be running
    assert!(shell.is_running());

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

// ============================================================================
// Q19: PROCESS LIFECYCLE
// ============================================================================

#[test]
#[ignore] // Requires actual shell
fn q19_spawn_wait_exit_code() {
    let shell = TerminalShellCapsule::new();

    // Spawn true (exits with 0)
    shell.spawn("/bin/true", 80, 24).expect("spawn failed");

    thread::sleep(Duration::from_millis(50));

    let code = shell.wait().expect("wait failed");
    assert_eq!(code, 0);
    assert_eq!(shell.exit_code(), Some(0));
}

#[test]
#[ignore] // Requires actual shell
fn q19_spawn_wait_nonzero_exit() {
    let shell = TerminalShellCapsule::new();

    // Spawn false (exits with 1)
    shell.spawn("/bin/false", 80, 24).expect("spawn failed");

    thread::sleep(Duration::from_millis(50));

    let code = shell.wait().expect("wait failed");
    assert_eq!(code, 1);
    assert_eq!(shell.exit_code(), Some(1));
}

#[test]
#[ignore] // Requires actual shell
fn q19_kill_wait_killed_exit() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    shell.kill().expect("kill failed");

    let code = shell.wait().expect("wait failed");
    // Killed processes have exit code 128 + SIGKILL (9) = 137
    assert!(code == 137 || code == 9 || code == -1);
}

#[test]
#[ignore] // Requires actual shell
fn q19_generation_increments_on_spawn() {
    let shell = TerminalShellCapsule::new();

    let initial_gen = shell.generation();
    assert_eq!(initial_gen, 0);

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    let new_gen = shell.generation();
    assert_eq!(new_gen, 1);

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

// ============================================================================
// Q20: ENVIRONMENT VARIABLES
// ============================================================================

#[test]
#[ignore] // Requires actual shell
fn q20_spawn_with_env() {
    let shell = TerminalShellCapsule::new();

    // Spawn with custom environment
    let env = [("TEST_VAR", "hello"), ("ANOTHER_VAR", "world")];
    shell.spawn_with_env("/bin/bash", &env, 80, 24).expect("spawn failed");

    // Write command to echo env var
    shell.write(b"echo $TEST_VAR\n").expect("write failed");

    thread::sleep(Duration::from_millis(200));

    let mut buf = [0u8; 1024];
    let n = shell.read(&mut buf).expect("read failed");

    let output = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(output.contains("hello"), "Output: {:?}", output);

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q20_spawn_with_multiple_env_vars() {
    let shell = TerminalShellCapsule::new();

    let env = [
        ("VAR1", "value1"),
        ("VAR2", "value2"),
        ("VAR3", "value3"),
    ];
    shell.spawn_with_env("/bin/bash", &env, 80, 24).expect("spawn failed");

    shell.write(b"echo $VAR1 $VAR2 $VAR3\n").expect("write failed");

    thread::sleep(Duration::from_millis(200));

    let mut buf = [0u8; 1024];
    let n = shell.read(&mut buf).expect("read failed");

    let output = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(output.contains("value1"));
    assert!(output.contains("value2"));
    assert!(output.contains("value3"));

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

// ============================================================================
// Q21: EOF HANDLING
// ============================================================================

#[test]
#[ignore] // Requires actual shell
fn q21_send_eof() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Send EOF (Ctrl+D)
    shell.send_eof().expect("send_eof failed");

    // Give shell time to exit
    thread::sleep(Duration::from_millis(200));

    // Shell should exit cleanly
    let code = shell.wait().expect("wait failed");
    assert_eq!(code, 0);
}

#[test]
#[ignore] // Requires actual shell
fn q21_eof_byte_value() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // EOF is Ctrl+D (0x04)
    shell.write(&[0x04]).expect("write failed");

    thread::sleep(Duration::from_millis(200));

    let code = shell.wait().expect("wait failed");
    assert_eq!(code, 0);
}

// ============================================================================
// STRESS TESTS
// ============================================================================

#[test]
#[ignore] // Requires actual shell
fn q21_rapid_write_read_cycles() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Rapid write/read cycles
    for i in 0..10 {
        let cmd = format!("echo test_{}\n", i);
        shell.write(cmd.as_bytes()).expect("write failed");
        thread::sleep(Duration::from_millis(50));

        let mut buf = [0u8; 256];
        let _ = shell.read(&mut buf);
    }

    assert!(shell.bytes_written() > 0);
    assert!(shell.bytes_read() > 0);

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}

#[test]
#[ignore] // Requires actual shell
fn q21_large_output_streaming() {
    let shell = TerminalShellCapsule::new();

    shell.spawn("/bin/bash", 80, 24).expect("spawn failed");

    // Generate large output
    shell.write(b"seq 1 1000\n").expect("write failed");

    thread::sleep(Duration::from_millis(500));

    // Read in chunks
    let mut total_read = 0;
    let mut buf = [0u8; 256];

    for _ in 0..50 {
        if let Ok(n) = shell.read(&mut buf) {
            total_read += n;
            if n == 0 {
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(total_read > 1000, "Expected large output, got {}", total_read);

    shell.kill().expect("kill failed");
    let _ = shell.wait();
}
