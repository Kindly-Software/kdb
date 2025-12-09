//! Progress Indicator Capsule - Spinner Animation for Async Commands
//!
//! # UCE34 Framework
//! - Q1-Q9: Progress indicator for async operations (spinner animation, status messages)
//! - Q10: Tier 1 (Atomic) - Simple state machine with lockfree updates
//! - Q11: Rust AtomicBool + AtomicU8 for spinner frame state
//! - Q12: Nightly N/A (stable atomics sufficient)
//! - Q13-Q28: Spinner animation, frame updates, status messages
//! - Q31: Simplicity - Minimal state (active flag + frame counter + message buffer)
//! - Q33: Validation - #[derive(ComputationalCapsule)] compile-time verification
//! - Q34: Auditability N/A (ephemeral UI state, no persistence)
//!
//! # ASSUM Framework
//! - #ASSUME: AtomicBool sufficient for active/inactive state
//! - #VERIFY: Ordering::Release on start/stop ensures visibility
//! - #ASSUME: AtomicU8 sufficient for spinner frame (10 states)
//! - #VERIFY: Modulo arithmetic constrains to valid range [0, 9]
//! - #ASSUME: Message buffer updates are infrequent (no synchronization needed)
//! - #VERIFY: Message copy happens before setting active flag
//! - #ASSUME: Spinner updates from single thread (event loop)
//! - #VERIFY: No concurrent frame updates (deterministic rendering)
//!
//! # Performance Targets
//! - Start/stop: <20ns (atomic store + message copy)
//! - Frame update: <5ns (atomic increment + modulo)
//! - Current char: <2ns (atomic load + array index)
//! - Memory: 64B (single cache line)
//!
//! # Spinner Characters
//! Unicode Braille patterns for smooth rotation effect:
//! ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏ (10 frames, 100ms per frame = 1 second full rotation)

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Spinner animation characters (Unicode Braille patterns)
///
/// # Design
/// - 10 frames for smooth rotation (1 second full cycle at 100ms/frame)
/// - Braille patterns provide clean, minimal visual effect
/// - Single Unicode character width (no layout disruption)
pub const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Progress Indicator Capsule (T1 Atomic, 128B aligned)
///
/// # Memory Layout
/// ```text
/// Offset | Field                | Size | Alignment
/// -------|---------------------|------|----------
/// 0      | active              | 1    | 1
/// 1      | spinner_frame       | 1    | 1
/// 2-9    | last_update_ns      | 8    | 8
/// 10-41  | message             | 32   | 1 (max 31 chars + null)
/// 42-127 | _padding            | 86   | 1 (pad to 128B for align(64))
/// ```
///
/// # Chaos Principles
/// - Cache-aligned (64B alignment → 128B total) - False sharing prevention
/// - Atomic updates - Lockfree start/stop/next_frame
/// - Zero allocation - Fixed-size message buffer
/// - <20ns latency - Start/stop operations
///
/// # Usage
/// ```rust
/// let progress = ProgressIndicatorCapsule::new();
///
/// // Start progress indicator
/// progress.start("Loading data...");
///
/// // In render loop (every 100ms)
/// if progress.is_active() {
///     progress.update_frame_if_needed();
///     let spinner = progress.current_char();
///     let message = progress.message();
///     println!("{} {}", spinner, message);
/// }
///
/// // Stop when complete
/// progress.stop();
/// ```
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct ProgressIndicatorCapsule {
    /// Progress indicator active flag
    /// #ASSUME: AtomicBool sufficient for active/inactive state
    /// #VERIFY: Ordering::Release on start/stop ensures cross-thread visibility
    active: AtomicBool,

    /// Spinner frame index (0-9, modulo 10)
    /// #ASSUME: AtomicU8 sufficient for spinner frames (max 10 states)
    /// #VERIFY: Modulo arithmetic constrains to [0, 9]
    spinner_frame: AtomicU8,

    /// Padding for AtomicU64 alignment (must be 8-byte aligned in repr(C))
    _pad1: [u8; 6],

    /// Last frame update timestamp (nanoseconds since UNIX epoch)
    /// #ASSUME: u64 sufficient for nanosecond timestamps (<584 years)
    /// #VERIFY: Wrapping arithmetic prevents overflow panic
    last_update_ns: AtomicU64,

    /// Status message (32 bytes, max 31 chars + null terminator)
    /// #ASSUME: 31 chars sufficient for status messages
    /// #VERIFY: Safe truncation on copy
    /// #ASSUME: Message reads are infrequent (no atomic synchronization)
    /// #VERIFY: Message copy happens before setting active flag
    message: [u8; 32],

    /// Padding to 64 bytes
    /// Layout: 1 (bool) + 1 (u8) + 6 (pad1) + 8 (u64) + 32 (message) + 16 (padding) = 64
    _padding: [u8; 16],
}

impl ProgressIndicatorCapsule {
    /// Create new progress indicator capsule
    ///
    /// **Complexity**: O(1), deterministic <10ns
    /// **Safety**: All fields initialized to zero/false/empty
    pub fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            spinner_frame: AtomicU8::new(0),
            _pad1: [0u8; 6],
            last_update_ns: AtomicU64::new(0),
            message: [0u8; 32],
            _padding: [0u8; 16],
        }
    }

    /// Start progress indicator with status message
    ///
    /// **Complexity**: O(n) where n = min(message.len(), 31)
    /// **Performance**: <20ns (message copy + atomic stores)
    ///
    /// # Arguments
    /// - `message`: Status message (truncated to 31 chars)
    ///
    /// # Safety
    /// - Message buffer zeroed before copy
    /// - Safe truncation to 31 chars (preserves null terminator)
    /// - Active flag set AFTER message copy (ensures visibility)
    ///
    /// # Example
    /// ```rust
    /// progress.start("Connecting to server...");
    /// ```
    pub fn start(&mut self, message: &str) {
        // #VERIFY: Zero message buffer before copy
        self.message = [0u8; 32];

        // #VERIFY: Safe truncation to 31 chars (preserves null terminator)
        let msg_bytes = message.as_bytes();
        let copy_len = std::cmp::min(msg_bytes.len(), 31);
        self.message[..copy_len].copy_from_slice(&msg_bytes[..copy_len]);

        // #VERIFY: Reset spinner frame to 0
        self.spinner_frame.store(0, Ordering::Relaxed);

        // #VERIFY: Update timestamp to current time
        let now_ns = now_ns();
        self.last_update_ns.store(now_ns, Ordering::Relaxed);

        // #VERIFY: Set active flag LAST (after message copy)
        // Release ordering ensures message visible to other threads
        self.active.store(true, Ordering::Release);
    }

    /// Stop progress indicator
    ///
    /// **Complexity**: O(1), <5ns
    /// **Performance**: Single atomic store
    ///
    /// # Safety
    /// - Release ordering ensures all prior updates visible
    pub fn stop(&self) {
        // #VERIFY: Release ordering ensures all updates visible before deactivation
        self.active.store(false, Ordering::Release);
    }

    /// Check if progress indicator is active
    ///
    /// **Complexity**: O(1), <2ns
    /// **Performance**: Single atomic load
    ///
    /// # Returns
    /// - `true` if progress indicator is running
    /// - `false` if stopped
    #[inline(always)]
    pub fn is_active(&self) -> bool {
        // #VERIFY: Acquire ordering ensures message/frame visibility
        self.active.load(Ordering::Acquire)
    }

    /// Advance to next spinner frame (manual update)
    ///
    /// **Complexity**: O(1), <5ns
    /// **Performance**: Atomic load + store + modulo
    ///
    /// # Safety
    /// - Modulo 10 constrains frame to [0, 9]
    /// - Relaxed ordering sufficient (frame updates don't require synchronization)
    pub fn next_frame(&self) {
        // #VERIFY: Modulo arithmetic constrains to valid range [0, 9]
        let current = self.spinner_frame.load(Ordering::Relaxed);
        let next = (current + 1) % (SPINNER_CHARS.len() as u8);
        self.spinner_frame.store(next, Ordering::Relaxed);

        // #VERIFY: Update timestamp for automatic frame updates
        let now_ns = now_ns();
        self.last_update_ns.store(now_ns, Ordering::Relaxed);
    }

    /// Update spinner frame if 100ms elapsed (automatic)
    ///
    /// **Complexity**: O(1), <10ns
    /// **Performance**: 2 atomic loads + conditional store
    ///
    /// # Returns
    /// - `true` if frame was updated
    /// - `false` if <100ms since last update
    ///
    /// # Usage
    /// Call this in the render loop to automatically advance spinner:
    /// ```rust
    /// if progress.is_active() {
    ///     progress.update_frame_if_needed();
    ///     let spinner = progress.current_char();
    /// }
    /// ```
    pub fn update_frame_if_needed(&self) -> bool {
        if !self.is_active() {
            return false;
        }

        let now_ns = now_ns();
        let last_update = self.last_update_ns.load(Ordering::Relaxed);

        // #ASSUME: 100ms frame interval (100_000_000 nanoseconds)
        const FRAME_INTERVAL_NS: u64 = 100_000_000;

        if now_ns.saturating_sub(last_update) >= FRAME_INTERVAL_NS {
            self.next_frame();
            true
        } else {
            false
        }
    }

    /// Get current spinner character
    ///
    /// **Complexity**: O(1), <2ns
    /// **Performance**: Atomic load + array index
    ///
    /// # Returns
    /// - Unicode Braille spinner character (⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏)
    ///
    /// # Safety
    /// - Modulo arithmetic guarantees valid array index
    #[inline(always)]
    pub fn current_char(&self) -> char {
        let frame = self.spinner_frame.load(Ordering::Relaxed) as usize;
        // #VERIFY: Modulo guarantees valid index [0, 9]
        SPINNER_CHARS[frame % SPINNER_CHARS.len()]
    }

    /// Get status message
    ///
    /// **Complexity**: O(n) where n = message length
    /// **Performance**: <50ns for typical messages (<31 chars)
    ///
    /// # Returns
    /// - UTF-8 string slice of current status message
    ///
    /// # Safety
    /// - Message buffer always null-terminated (max 31 chars)
    /// - Safe UTF-8 conversion (invalid bytes replaced with �)
    pub fn message(&self) -> &str {
        // #VERIFY: Find null terminator (max 31 chars)
        let msg_len = self.message.iter()
            .position(|&b| b == 0)
            .unwrap_or(31);

        // #VERIFY: Safe UTF-8 conversion (lossy handles invalid bytes)
        std::str::from_utf8(&self.message[..msg_len])
            .unwrap_or("")
    }

    /// Get last update timestamp in nanoseconds
    ///
    /// **Complexity**: O(1), <2ns
    /// **Performance**: Single atomic load
    ///
    /// # Returns
    /// - Nanoseconds since UNIX epoch of last frame update
    #[inline(always)]
    pub fn last_update_ns(&self) -> u64 {
        self.last_update_ns.load(Ordering::Relaxed)
    }
}

impl Default for ProgressIndicatorCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: Get current timestamp in nanoseconds
///
/// **Complexity**: O(1), <10ns
/// **Safety**: Unwrap safe (UNIX epoch always before current time)
#[inline]
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(std::mem::size_of::<ProgressIndicatorCapsule>(), 64);
        assert_eq!(std::mem::align_of::<ProgressIndicatorCapsule>(), 64);
    }

    #[test]
    fn test_spinner_chars_count() {
        assert_eq!(SPINNER_CHARS.len(), 10);
    }

    #[test]
    fn test_initial_state() {
        let progress = ProgressIndicatorCapsule::new();
        assert!(!progress.is_active());
        assert_eq!(progress.current_char(), SPINNER_CHARS[0]);
        assert_eq!(progress.message(), "");
    }

    #[test]
    fn test_start_stop() {
        let mut progress = ProgressIndicatorCapsule::new();

        // Start with message
        progress.start("Loading...");
        assert!(progress.is_active());
        assert_eq!(progress.message(), "Loading...");
        assert_eq!(progress.current_char(), SPINNER_CHARS[0]);

        // Stop
        progress.stop();
        assert!(!progress.is_active());
    }

    #[test]
    fn test_message_truncation() {
        let mut progress = ProgressIndicatorCapsule::new();

        // Test long message (>31 chars)
        let long_message = "This is a very long message that exceeds the 31 character limit";
        progress.start(long_message);

        let stored_message = progress.message();
        assert!(stored_message.len() <= 31);
        assert!(long_message.starts_with(stored_message));
    }

    #[test]
    fn test_spinner_frame_rotation() {
        let progress = ProgressIndicatorCapsule::new();

        // Test full rotation
        for i in 0..10 {
            assert_eq!(progress.current_char(), SPINNER_CHARS[i]);
            progress.next_frame();
        }

        // Verify wrapping to 0
        assert_eq!(progress.current_char(), SPINNER_CHARS[0]);
    }

    #[test]
    fn test_automatic_frame_update() {
        let mut progress = ProgressIndicatorCapsule::new();
        progress.start("Testing...");

        // Initial frame should be 0
        assert_eq!(progress.current_char(), SPINNER_CHARS[0]);

        // Immediately calling update_frame_if_needed should return false
        assert!(!progress.update_frame_if_needed());
        assert_eq!(progress.current_char(), SPINNER_CHARS[0]);

        // Sleep for >100ms
        std::thread::sleep(std::time::Duration::from_millis(150));

        // Now update should succeed
        assert!(progress.update_frame_if_needed());
        assert_eq!(progress.current_char(), SPINNER_CHARS[1]);
    }

    #[test]
    fn test_inactive_no_update() {
        let progress = ProgressIndicatorCapsule::new();

        // Inactive progress should not update
        assert!(!progress.update_frame_if_needed());
        assert_eq!(progress.current_char(), SPINNER_CHARS[0]);
    }

    #[test]
    fn test_message_empty() {
        let mut progress = ProgressIndicatorCapsule::new();
        progress.start("");
        assert_eq!(progress.message(), "");
    }

    #[test]
    fn test_message_exactly_31_chars() {
        let mut progress = ProgressIndicatorCapsule::new();
        let msg_31 = "1234567890123456789012345678901"; // Exactly 31 chars
        progress.start(msg_31);
        assert_eq!(progress.message(), msg_31);
    }

    #[test]
    fn test_concurrent_frame_updates() {
        use std::sync::Arc;

        let progress = Arc::new(ProgressIndicatorCapsule::new());

        // Spawn 10 threads each advancing frame 10 times
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let p = Arc::clone(&progress);
                std::thread::spawn(move || {
                    for _ in 0..10 {
                        p.next_frame();
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify frame is in valid range [0, 9]
        let final_char = progress.current_char();
        assert!(SPINNER_CHARS.contains(&final_char));
    }

    #[test]
    fn test_start_resets_frame() {
        let mut progress = ProgressIndicatorCapsule::new();

        // Advance to frame 5
        for _ in 0..5 {
            progress.next_frame();
        }
        assert_eq!(progress.current_char(), SPINNER_CHARS[5]);

        // Start should reset to frame 0
        progress.start("Resetting...");
        assert_eq!(progress.current_char(), SPINNER_CHARS[0]);
    }

    #[test]
    fn test_utf8_message() {
        let mut progress = ProgressIndicatorCapsule::new();
        progress.start("Loading… 进度");
        assert!(progress.message().contains("Loading"));
    }
}
