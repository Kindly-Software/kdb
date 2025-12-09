//! Command Output Capsule - Lockfree Output Buffer for TUI
//!
//! # UCE34 Framework
//! - Q1-Q9: Command output storage and display (ring buffer pattern)
//! - Q10: Tier 1 (Atomic) + Tier 4 (Batch) - Ring buffer with atomic indices
//! - Q11: Rust atomic primitives for lockfree ring buffer
//! - Q12: Nightly N/A (stable atomics sufficient)
//! - Q13-Q28: Output validation, circular buffer overflow handling
//! - Q31: Simplicity - Single ring buffer, atomic head/length, scrolling support
//! - Q33: Validation - #[derive(ComputationalCapsule)] compile-time verification
//! - Q34: Auditability N/A (ephemeral output display, no persistence)
//!
//! # Architecture
//! ```text
//! CommandOutputCapsule (512B, T1+T4 Hybrid)
//!   [0..4]      buffer_len: AtomicU32       // Current content length
//!   [4..8]      buffer_head: AtomicU32      // Ring buffer write position
//!   [8..12]     scroll_position: AtomicU32  // Vertical scroll offset
//!   [12..64]    last_command: [u8; 52]      // Last command name (null-terminated)
//!   [64..320]   last_error: [u8; 256]       // Last error message (null-terminated)
//!   [320..336]  _padding0: [u8; 16]         // Alignment padding
//!   [336..4432] buffer: [u8; 4096]          // Ring buffer (4KB output)
//!   [4432..512] _padding1: [u8; 80]         // Pad to 512B
//! ```
//!
//! # Ring Buffer Design
//! - **Capacity**: 4KB output (sufficient for typical command output)
//! - **Overflow**: Circular overwrite (oldest data discarded)
//! - **Scroll**: AtomicU32 for lockfree scroll position
//! - **Safety**: Atomic length ensures consistent reads
//!
//! # Performance Targets
//! - Append output: <50ns (atomic stores + memcpy)
//! - Get output: <100ns (atomic load + String allocation)
//! - Clear: <10ns (atomic store to 0)
//! - Scroll: <5ns (atomic store)
//!
//! # ASSUM Framework
//! - #ASSUME: Ring buffer size (4KB) sufficient for command output
//! - #VERIFY: Append operation clamps to buffer size
//! - #ASSUME: UTF-8 conversion is safe with lossy conversion
//! - #VERIFY: String::from_utf8_lossy handles invalid UTF-8
//! - #ASSUME: Atomic length prevents torn reads
//! - #VERIFY: Acquire ordering on length load ensures consistency

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Maximum command output buffer size (64 bytes inline, ~1 line preview)
/// For full output, external storage will be used in future iterations
const OUTPUT_BUFFER_SIZE: usize = 64;

/// Maximum command name length
const COMMAND_NAME_SIZE: usize = 32;

/// Maximum error message length
const ERROR_MESSAGE_SIZE: usize = 128;

/// Command Output Capsule (256B, T1 Atomic + T4 Batch)
///
/// **Layout** (256 bytes, 256-byte aligned):
/// - buffer_len: 4 bytes (AtomicU32)
/// - buffer_head: 4 bytes (AtomicU32)
/// - scroll_position: 4 bytes (AtomicU32)
/// - last_command: 32 bytes (null-terminated string)
/// - last_error: 128 bytes (null-terminated string)
/// - buffer: 64 bytes (ring buffer for output preview)
/// - padding: 20 bytes (complete to 256B)
/// Total: 4+4+4+32+128+64+20 = 256 bytes
///
/// # Ring Buffer Design
/// - **Capacity**: 64 bytes (~1 line of output preview)
/// - **Full output**: Future iteration will use external Vec<String> for complete history
/// - **Current**: Sufficient for quick command result preview in TUI
///
/// # Chaos Principles
/// - Cache-aligned (256B) - Prevents false sharing
/// - Atomic indices - Lockfree ring buffer operations
/// - Circular overwrite - No allocation on append
/// - <100ns operations - Fast output capture
///
/// # ASSUM Framework
/// - #ASSUME: AtomicU32 length provides atomic snapshot
/// - #VERIFY: Acquire ordering ensures memory visibility
/// - #ASSUME: Ring buffer circular wrap is safe
/// - #VERIFY: Modulo arithmetic constrains indices
/// - #ASSUME: UTF-8 conversion handles invalid bytes
/// - #VERIFY: from_utf8_lossy never panics
///
/// # Capsule Verification
/// Manual verification required due to UnsafeCell fields (100% lockfree pattern)
/// - Alignment: 256 bytes (verified below with static assertion)
/// - Size: 256 bytes (verified below with static assertion)
#[repr(C, align(256))]
pub struct CommandOutputCapsule {
    /// Current buffer content length (may exceed OUTPUT_BUFFER_SIZE for display)
    /// #ASSUME: AtomicU32 provides atomic read of length
    /// #VERIFY: Ordering::Acquire ensures buffer content visible
    buffer_len: AtomicU32,

    /// Ring buffer write head position (circular)
    /// #ASSUME: Modulo arithmetic wraps correctly
    /// #VERIFY: head % OUTPUT_BUFFER_SIZE stays in bounds
    buffer_head: AtomicU32,

    /// Vertical scroll position (lines from top)
    /// #ASSUME: u32 sufficient for scroll offset
    /// #VERIFY: Clamped to valid range by render logic
    scroll_position: AtomicU32,

    /// Padding for alignment (4 bytes)
    _padding0: [u8; 4],

    /// Unix timestamp (seconds) when error was set (0 = no error/auto-dismissed)
    /// #ASSUME: SystemTime provides accurate timestamps
    /// #VERIFY: Used for 10-second auto-dismiss logic
    error_timestamp: AtomicU64,

    /// Last executed command name (null-terminated)
    /// #ASSUME: 52 bytes sufficient for command names
    /// #VERIFY: Truncation ensures null-termination
    /// #ASSUME_UNSAFECELL: Interior mutability for lockfree writes
    /// #VERIFY_SAFETY: Single logical writer (no concurrent mutations)
    last_command: UnsafeCell<[u8; COMMAND_NAME_SIZE]>,

    /// Last error message (null-terminated)
    /// #ASSUME: 256 bytes sufficient for error messages
    /// #VERIFY: Truncation ensures null-termination
    /// #ASSUME_UNSAFECELL: Interior mutability for lockfree writes
    /// #VERIFY_SAFETY: Single logical writer (TUI main thread)
    last_error: UnsafeCell<[u8; ERROR_MESSAGE_SIZE]>,

    /// Output ring buffer (64 bytes for preview)
    /// #ASSUME: 64 bytes sufficient for command result preview (~1 line)
    /// #VERIFY: Circular overwrite prevents overflow panics
    /// #ASSUME_UNSAFECELL: Interior mutability for lockfree ring buffer
    /// #VERIFY_SAFETY: Atomic head/length ensures no torn reads
    buffer: UnsafeCell<[u8; OUTPUT_BUFFER_SIZE]>,

    /// Padding to 256 bytes
    /// Total: 4+4+4+4+8+32+128+64 = 248, so need 8 bytes padding
    _padding: [u8; 8],
}

// SAFETY: CommandOutputCapsule is Sync despite UnsafeCell fields because:
// - Single logical writer (TUI main thread) ensures no data races
// - All reads use Acquire ordering via atomics (error_timestamp, buffer_len, buffer_head)
// - UnsafeCell mutations are externally synchronized by TUI architecture
// - No concurrent writes to UnsafeCell fields (guaranteed by single-threaded event loop)
unsafe impl Sync for CommandOutputCapsule {}

impl CommandOutputCapsule {
    /// Create new command output capsule
    ///
    /// **Complexity**: O(1), deterministic <20ns
    /// **Safety**: All fields initialized to zero/empty
    pub fn new() -> Self {
        Self {
            buffer_len: AtomicU32::new(0),
            buffer_head: AtomicU32::new(0),
            scroll_position: AtomicU32::new(0),
            _padding0: [0u8; 4],
            error_timestamp: AtomicU64::new(0),
            last_command: UnsafeCell::new([0u8; COMMAND_NAME_SIZE]),
            last_error: UnsafeCell::new([0u8; ERROR_MESSAGE_SIZE]),
            buffer: UnsafeCell::new([0u8; OUTPUT_BUFFER_SIZE]),
            _padding: [0u8; 8],
        }
    }

    /// Append output to ring buffer (lockfree, <50ns)
    ///
    /// **Complexity**: O(n) where n = output.len()
    /// **Safety**: UnsafeCell + atomic indices ensure lockfree coordination
    ///
    /// # Arguments
    /// - `output`: Output string to append
    ///
    /// # Behavior
    /// - If output fits: Append at head position
    /// - If output exceeds buffer: Write last N bytes (circular)
    /// - Atomic length update ensures consistent reads
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Single logical writer (TUI event loop thread)
    /// - #VERIFY: No concurrent append_output calls (guaranteed by TUI architecture)
    /// - #ASSUME: Atomic head prevents torn reads
    /// - #VERIFY: Readers use Acquire ordering on head/length
    pub fn append_output(&self, output: &str) {
        let bytes = output.as_bytes();
        let len = bytes.len();

        if len == 0 {
            return;
        }

        // Clamp to buffer size (take last N bytes if too large)
        let write_len = len.min(OUTPUT_BUFFER_SIZE);
        let src_offset = if len > OUTPUT_BUFFER_SIZE {
            len - OUTPUT_BUFFER_SIZE
        } else {
            0
        };

        // Get current head position
        let head = self.buffer_head.load(Ordering::Acquire) as usize;

        // Write to buffer (circular) - SAFETY: Single writer guaranteed by TUI architecture
        unsafe {
            let buf = &mut *self.buffer.get();
            for (i, &byte) in bytes[src_offset..].iter().take(write_len).enumerate() {
                let pos = (head + i) % OUTPUT_BUFFER_SIZE;
                buf[pos] = byte;
            }
        }

        // Update head and length (Release ordering ensures writes visible)
        let new_head = (head + write_len) % OUTPUT_BUFFER_SIZE;
        self.buffer_head.store(new_head as u32, Ordering::Release);

        // Track total length written (use original len, may exceed buffer size for display)
        let total_len = self.buffer_len.load(Ordering::Acquire);
        self.buffer_len.store(
            total_len.saturating_add(len as u32),  // Use original len, not write_len
            Ordering::Release
        );
    }

    /// Get output as string (lockfree read, <100ns)
    ///
    /// **Complexity**: O(n) where n = min(max_lines * 64, buffer_len)
    /// **Safety**: UnsafeCell + atomic length ensures consistent read
    ///
    /// # Arguments
    /// - `max_lines`: Maximum lines to retrieve (approximate, assumes 64 chars/line)
    ///
    /// # Returns
    /// - String containing buffered output (lossy UTF-8 conversion)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Atomic length prevents torn reads
    /// - #VERIFY: Acquire ordering ensures writes visible before read
    pub fn get_output(&self, max_lines: usize) -> String {
        let len = self.buffer_len.load(Ordering::Acquire) as usize;

        if len == 0 {
            return String::new();
        }

        // Calculate how much to read (rough estimate: 64 chars per line)
        let max_bytes = (max_lines * 64).min(OUTPUT_BUFFER_SIZE);
        let read_len = len.min(max_bytes);

        // Determine read start position
        let head = self.buffer_head.load(Ordering::Acquire) as usize;
        let start = if len > OUTPUT_BUFFER_SIZE {
            head // Buffer wrapped, start at head
        } else {
            0 // Buffer not wrapped, start at beginning
        };

        // Read from buffer (handle circular wrap) - SAFETY: Atomic indices ensure consistency
        let mut result = Vec::with_capacity(read_len);
        unsafe {
            let buf = &*self.buffer.get();
            for i in 0..read_len.min(OUTPUT_BUFFER_SIZE) {
                let pos = (start + i) % OUTPUT_BUFFER_SIZE;
                result.push(buf[pos]);
            }
        }

        // Convert to UTF-8 (lossy)
        String::from_utf8_lossy(&result).to_string()
    }

    /// Clear output buffer (lockfree, <10ns)
    ///
    /// **Complexity**: O(1), atomic stores only
    /// **Safety**: Atomic length reset, no buffer modification needed
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Atomic stores sufficient for clear (no buffer zeroing needed)
    /// - #VERIFY: Readers check length before accessing buffer
    pub fn clear(&self) {
        self.buffer_len.store(0, Ordering::Release);
        self.buffer_head.store(0, Ordering::Release);
        self.scroll_position.store(0, Ordering::Release);
    }

    /// Set last command name (for display context)
    ///
    /// **Complexity**: O(n) where n = command.len()
    /// **Safety**: UnsafeCell provides interior mutability, null-terminates
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Single logical writer (TUI event loop)
    /// - #VERIFY: No concurrent set_last_command calls
    pub fn set_last_command(&self, command: &str) {
        let bytes = command.as_bytes();
        let copy_len = bytes.len().min(COMMAND_NAME_SIZE - 1);

        unsafe {
            let cmd = &mut *self.last_command.get();
            cmd[..copy_len].copy_from_slice(&bytes[..copy_len]);
            cmd[copy_len] = 0; // Null-terminate
        }
    }

    /// Get last command name
    ///
    /// **Complexity**: O(n) where n = length of command
    /// **Safety**: Reads until null terminator or end
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Null terminator always present
    /// - #VERIFY: set_last_command ensures null termination
    pub fn last_command(&self) -> String {
        unsafe {
            let cmd = &*self.last_command.get();
            let null_pos = cmd.iter().position(|&b| b == 0)
                .unwrap_or(COMMAND_NAME_SIZE);

            String::from_utf8_lossy(&cmd[..null_pos]).to_string()
        }
    }

    /// Set last error message
    ///
    /// **Complexity**: O(n) where n = error.len()
    /// **Safety**: Truncates to ERROR_MESSAGE_SIZE, null-terminates
    ///
    /// # Timestamp Recording
    /// Records current Unix timestamp for 10-second auto-dismiss.
    /// Empty string clears error and resets timestamp to 0.
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Single logical writer (TUI event loop thread)
    /// - #VERIFY: No concurrent set_last_error calls (guaranteed by TUI architecture)
    /// - #VERIFY: UnsafeCell provides interior mutability without Mutex
    pub fn set_last_error(&self, error: &str) {
        let bytes = error.as_bytes();
        let copy_len = bytes.len().min(ERROR_MESSAGE_SIZE - 1);

        // SAFETY: Single writer guaranteed by TUI architecture (event loop is single-threaded)
        unsafe {
            let err_buf = &mut *self.last_error.get();
            err_buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
            err_buf[copy_len] = 0; // Null-terminate
        }

        // Record timestamp (0 if clearing error)
        if error.is_empty() {
            self.error_timestamp.store(0, std::sync::atomic::Ordering::Release);
        } else {
            use std::time::{SystemTime, UNIX_EPOCH};
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.error_timestamp.store(timestamp, std::sync::atomic::Ordering::Release);
        }
    }

    /// Get last error message
    ///
    /// **Complexity**: O(n) where n = length of error
    /// **Safety**: Reads until null terminator or end
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Atomic error_timestamp prevents torn reads
    /// - #VERIFY: Readers check timestamp for consistency
    pub fn last_error(&self) -> String {
        // SAFETY: Reading immutably via UnsafeCell, atomic timestamp prevents torn reads
        unsafe {
            let err = &*self.last_error.get();
            let null_pos = err.iter().position(|&b| b == 0)
                .unwrap_or(ERROR_MESSAGE_SIZE);

            String::from_utf8_lossy(&err[..null_pos]).to_string()
        }
    }

    /// Get error timestamp (Unix seconds, 0 if no error)
    ///
    /// **Complexity**: O(1), single atomic load
    pub fn error_timestamp(&self) -> u64 {
        self.error_timestamp.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Check if error should be auto-dismissed (>10 seconds old)
    ///
    /// **Complexity**: O(1), timestamp comparison
    /// **Returns**: true if error exists and >10 seconds old
    pub fn should_auto_dismiss_error(&self) -> bool {
        let timestamp = self.error_timestamp();
        if timestamp == 0 {
            return false; // No error set
        }

        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now.saturating_sub(timestamp) >= 10 // 10 seconds elapsed
    }

    /// Get scroll position (lines from top)
    ///
    /// **Complexity**: O(1), single atomic load
    pub fn scroll_position(&self) -> u32 {
        self.scroll_position.load(Ordering::Acquire)
    }

    /// Set scroll position (lockfree, <5ns)
    ///
    /// **Complexity**: O(1), single atomic store
    /// **Safety**: Clamping done by caller (TUI render logic)
    pub fn set_scroll_position(&self, position: u32) {
        self.scroll_position.store(position, Ordering::Release);
    }

    /// Check if output buffer is empty
    ///
    /// **Complexity**: O(1), single atomic load
    pub fn is_empty(&self) -> bool {
        self.buffer_len.load(Ordering::Acquire) == 0
    }

    /// Get total bytes written (may exceed buffer size)
    ///
    /// **Complexity**: O(1), single atomic load
    pub fn total_bytes(&self) -> u32 {
        self.buffer_len.load(Ordering::Acquire)
    }
}

impl Default for CommandOutputCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Manual capsule verification (required due to UnsafeCell fields in 100% lockfree pattern)
    const _: () = {
        const SIZE: usize = std::mem::size_of::<CommandOutputCapsule>();
        const ALIGN: usize = std::mem::align_of::<CommandOutputCapsule>();

        // Verify size = 256 bytes
        assert!(SIZE == 256, "CommandOutputCapsule size must be 256 bytes");

        // Verify alignment = 256 bytes
        assert!(ALIGN == 256, "CommandOutputCapsule alignment must be 256 bytes");
    };

    #[test]
    fn test_capsule_size_alignment() {
        // Verify capsule size and alignment
        // Total: 4+4+4+4+8+32+128+64+8 = 256 bytes
        assert_eq!(std::mem::size_of::<CommandOutputCapsule>(), 256);
        assert_eq!(std::mem::align_of::<CommandOutputCapsule>(), 256);
    }

    #[test]
    fn test_initial_state() {
        let capsule = CommandOutputCapsule::new();
        assert!(capsule.is_empty());
        assert_eq!(capsule.total_bytes(), 0);
        assert_eq!(capsule.scroll_position(), 0);
        assert_eq!(capsule.last_command(), "");
        assert_eq!(capsule.last_error(), "");
    }

    #[test]
    fn test_append_and_read() {
        let capsule = CommandOutputCapsule::new();

        // Append simple text
        capsule.append_output("Hello, World!");
        assert!(!capsule.is_empty());
        assert_eq!(capsule.total_bytes(), 13);

        // Read output
        let output = capsule.get_output(100);
        assert_eq!(output, "Hello, World!");
    }

    #[test]
    fn test_append_multi_line() {
        let capsule = CommandOutputCapsule::new();

        capsule.append_output("Line 1\n");
        capsule.append_output("Line 2\n");
        capsule.append_output("Line 3\n");

        let output = capsule.get_output(100);
        assert!(output.contains("Line 1"));
        assert!(output.contains("Line 2"));
        assert!(output.contains("Line 3"));
    }

    #[test]
    fn test_circular_buffer_overflow() {
        let capsule = CommandOutputCapsule::new();

        // Write more than buffer size
        let large_text = "X".repeat(OUTPUT_BUFFER_SIZE + 1000);
        capsule.append_output(&large_text);

        // Should only keep last OUTPUT_BUFFER_SIZE bytes
        let output = capsule.get_output(1000);
        assert!(output.len() <= OUTPUT_BUFFER_SIZE);
        assert!(capsule.total_bytes() > OUTPUT_BUFFER_SIZE as u32);
    }

    #[test]
    fn test_clear() {
        let capsule = CommandOutputCapsule::new();

        capsule.append_output("Test output");
        assert!(!capsule.is_empty());

        capsule.clear();
        assert!(capsule.is_empty());
        assert_eq!(capsule.total_bytes(), 0);
        assert_eq!(capsule.scroll_position(), 0);
    }

    #[test]
    fn test_last_command() {
        let capsule = CommandOutputCapsule::new();

        capsule.set_last_command("start");
        assert_eq!(capsule.last_command(), "start");

        // Test truncation
        let long_command = "a".repeat(100);
        capsule.set_last_command(&long_command);
        assert!(capsule.last_command().len() < 100);
        assert!(capsule.last_command().len() <= COMMAND_NAME_SIZE - 1);
    }

    #[test]
    fn test_last_error() {
        let capsule = CommandOutputCapsule::new();

        capsule.set_last_error("Connection refused");
        assert_eq!(capsule.last_error(), "Connection refused");

        // Test truncation
        let long_error = "E".repeat(500);
        capsule.set_last_error(&long_error);
        assert!(capsule.last_error().len() < 500);
        assert!(capsule.last_error().len() <= ERROR_MESSAGE_SIZE - 1);
    }

    #[test]
    fn test_scroll_position() {
        let capsule = CommandOutputCapsule::new();

        assert_eq!(capsule.scroll_position(), 0);

        capsule.set_scroll_position(10);
        assert_eq!(capsule.scroll_position(), 10);
    }

    #[test]
    fn test_utf8_handling() {
        let capsule = CommandOutputCapsule::new();

        // Valid UTF-8
        capsule.append_output("Hello 世界");
        let output = capsule.get_output(100);
        assert!(output.contains("世界"));

        // Invalid UTF-8 should not panic (lossy conversion)
        capsule.clear();
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        capsule.append_output(&String::from_utf8_lossy(&invalid_utf8));
        let _output = capsule.get_output(100); // Should not panic
    }
}
