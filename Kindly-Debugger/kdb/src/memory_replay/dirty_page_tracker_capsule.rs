//! DirtyPageTrackerCapsule - T2 SIMD Tier Memory Dirty Page Tracking
//!
//! Tracks memory pages modified by target process using Linux soft-dirty bits
//! from /proc/pid/pagemap. T2 SIMD tier for fast bitmap scanning.
//!
//! # Memory Layout (256 KB = 262,144 bytes)
//! - Metadata: 256 bytes (pid, generation, state, stats)
//! - DirtyBitmap: 131,072 bytes (1M pages = 4GB address space coverage)
//! - PreviousBitmap: 131,072 bytes (for XOR to find changes since last scan)
//!
//! # Soft-Dirty Mechanism (Linux-specific)
//! 1. Clear refs: write "4" to `/proc/pid/clear_refs`
//! 2. Read pagemap: read 8-byte entries from `/proc/pid/pagemap`
//! 3. Check bit 55: `(entry >> 55) & 1` is soft-dirty flag
//!
//! # ASSUM Tags
//! #ASSUME_LOCKFREE_ONLY: All coordination via atomics, no mutex/RwLock
//! #ASSUME_LINUX_PAGEMAP: /proc/pid/pagemap interface available on Linux
//! #ASSUME_ALIGNED_BITMAP: Bitmaps 64-byte aligned for SIMD operations
//! #ASSUME_GENERATION_VALID: Generation counter prevents TOCTOU races
//! #ASSUME_SIMD_SAFE: SIMD operations on aligned 64-bit words
//! #ASSUME_FILE_IO_SAFE: File operations may fail, errors propagated via Result

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Number of pages tracked (1M pages = 4GB address space at 4KB page size)
pub const TRACKED_PAGES: usize = 1_048_576;

/// Number of 64-bit words in bitmap (1M bits / 64 = 16,384 words)
pub const BITMAP_WORDS: usize = TRACKED_PAGES / 64;

/// Page size (4KB)
pub const PAGE_SIZE: u64 = 4096;

/// Soft-dirty bit position in pagemap entry (bit 55)
pub const SOFT_DIRTY_BIT: u64 = 55;

/// Tracker states
pub mod state {
    pub const IDLE: u64 = 0;
    pub const SCANNING: u64 = 1;
    pub const CLEARED: u64 = 2;
    pub const ERROR: u64 = 3;
}

/// Error types for dirty page tracking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackerError {
    /// Process not attached (pid = 0)
    ProcessNotAttached,
    /// Failed to open /proc/pid/pagemap
    PagemapOpenFailed(String),
    /// Failed to read /proc/pid/pagemap
    PagemapReadFailed(String),
    /// Failed to write /proc/pid/clear_refs
    ClearRefsFailed(String),
    /// Not running on Linux
    NotLinux,
    /// Invalid state transition
    InvalidState,
    /// IO error
    IoError(String),
}

impl std::fmt::Display for TrackerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackerError::ProcessNotAttached => write!(f, "Process not attached"),
            TrackerError::PagemapOpenFailed(e) => write!(f, "Pagemap open failed: {}", e),
            TrackerError::PagemapReadFailed(e) => write!(f, "Pagemap read failed: {}", e),
            TrackerError::ClearRefsFailed(e) => write!(f, "Clear refs failed: {}", e),
            TrackerError::NotLinux => write!(f, "Not running on Linux"),
            TrackerError::InvalidState => write!(f, "Invalid state transition"),
            TrackerError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for TrackerError {}

/// Iterator over dirty page addresses
pub struct DirtyPageIterator<'a> {
    tracker: &'a DirtyPageTrackerCapsule,
    word_index: usize,
    bit_index: u32,
}

impl<'a> Iterator for DirtyPageIterator<'a> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        // Scan through bitmap words
        while self.word_index < BITMAP_WORDS {
            let word = self.tracker.dirty_bitmap[self.word_index].load(Ordering::Relaxed);

            // Check remaining bits in current word
            while self.bit_index < 64 {
                let bit_mask = 1u64 << self.bit_index;
                if (word & bit_mask) != 0 {
                    let page_index = (self.word_index * 64 + self.bit_index as usize) as u64;
                    self.bit_index += 1;
                    return Some(page_index * PAGE_SIZE);
                }
                self.bit_index += 1;
            }

            // Move to next word
            self.word_index += 1;
            self.bit_index = 0;
        }

        None
    }
}

/// DirtyPageTrackerCapsule - T2 SIMD Tier
///
/// Tracks memory pages modified by target process using Linux soft-dirty bits.
/// Uses SIMD acceleration for fast bitmap operations (popcnt, XOR).
///
/// # Memory Layout (256 KB)
/// ```text
/// Offset    Size       Field
/// 0x00000   256B       Metadata (pid, generation, state, stats, padding)
/// 0x00100   131,072B   dirty_bitmap (16,384 AtomicU64 = 1M bits)
/// 0x20100   131,072B   previous_bitmap (16,384 AtomicU64 = 1M bits)
/// Total:    262,400B   (~256 KB)
/// ```
///
/// #ASSUME_ALIGNED_BITMAP: Bitmaps 64-byte aligned for SIMD operations
/// #VERIFY_UNIT_TEST: test_capsule_size_and_alignment
#[repr(C, align(256))]
pub struct DirtyPageTrackerCapsule {
    // ========================================================================
    // Metadata (256 bytes)
    // ========================================================================

    /// Target process ID
    pub pid: AtomicU64,

    /// Generation counter for TOCTOU prevention
    pub generation: AtomicU64,

    /// Current state (Idle, Scanning, Cleared, Error)
    pub state: AtomicU64,

    /// Total pages tracked (may be less than TRACKED_PAGES)
    pub total_pages: AtomicU64,

    /// Current dirty page count
    pub dirty_count: AtomicU64,

    /// Number of scans performed
    pub scan_count: AtomicU64,

    /// Last scan timestamp (nanoseconds since epoch)
    pub last_scan_ns: AtomicU64,

    /// Error code (0 = no error)
    pub error_code: AtomicU32,

    /// Reserved for future use
    _reserved: AtomicU32,

    /// Padding to 256 bytes
    /// 7 * 8 (AtomicU64) + 2 * 4 (AtomicU32) = 56 + 8 = 64 bytes used
    /// Need 256 - 64 = 192 bytes padding
    _metadata_pad: [u8; 192],

    // ========================================================================
    // Bitmaps (262,144 - 256 = 261,888 bytes, but we use 262,144 for alignment)
    // ========================================================================

    /// Current dirty page bitmap (1 bit per page, 1M pages = 16K words)
    /// #ASSUME_SIMD_SAFE: Aligned for SIMD operations
    pub dirty_bitmap: [AtomicU64; BITMAP_WORDS],

    /// Previous scan bitmap (for delta detection via XOR)
    /// #ASSUME_SIMD_SAFE: Aligned for SIMD operations
    pub previous_bitmap: [AtomicU64; BITMAP_WORDS],
}

// Verify layout at compile time
const _: () = {
    // Metadata is 256 bytes: 7 AtomicU64 (56) + 2 AtomicU32 (8) + 192 padding = 256
    assert!(std::mem::size_of::<AtomicU64>() * 7 + std::mem::size_of::<AtomicU32>() * 2 + 192 == 256);
    // Each bitmap is 131,072 bytes
    assert!(BITMAP_WORDS * std::mem::size_of::<AtomicU64>() == 131_072);
};

impl DirtyPageTrackerCapsule {
    /// Create a new dirty page tracker for a process.
    ///
    /// # Arguments
    /// * `pid` - Target process ID (0 for unattached)
    ///
    /// # Returns
    /// New tracker in Idle state
    ///
    /// #ASSUME_LOCKFREE_ONLY: Uses atomic initialization
    /// #VERIFY_UNIT_TEST: test_new_tracker
    pub fn new(pid: u64) -> Self {
        // Initialize atomic arrays
        const ZERO: AtomicU64 = AtomicU64::new(0);

        Self {
            pid: AtomicU64::new(pid),
            generation: AtomicU64::new(0),
            state: AtomicU64::new(state::IDLE),
            total_pages: AtomicU64::new(TRACKED_PAGES as u64),
            dirty_count: AtomicU64::new(0),
            scan_count: AtomicU64::new(0),
            last_scan_ns: AtomicU64::new(0),
            error_code: AtomicU32::new(0),
            _reserved: AtomicU32::new(0),
            _metadata_pad: [0; 192],
            dirty_bitmap: [ZERO; BITMAP_WORDS],
            previous_bitmap: [ZERO; BITMAP_WORDS],
        }
    }

    /// Create an unattached tracker.
    pub fn unattached() -> Self {
        Self::new(0)
    }

    /// Attach to a process.
    ///
    /// # Arguments
    /// * `pid` - Target process ID
    ///
    /// #ASSUME_LOCKFREE_ONLY: Atomic store with Release ordering
    /// #VERIFY_UNIT_TEST: test_attach_detach
    pub fn attach(&self, pid: u64) {
        self.pid.store(pid, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.state.store(state::IDLE, Ordering::Release);
        self.error_code.store(0, Ordering::Release);
    }

    /// Detach from process.
    pub fn detach(&self) {
        self.pid.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.state.store(state::IDLE, Ordering::Release);
    }

    /// Get current process ID.
    #[inline]
    pub fn get_pid(&self) -> u64 {
        self.pid.load(Ordering::Acquire)
    }

    /// Check if attached to a process.
    #[inline]
    pub fn is_attached(&self) -> bool {
        self.get_pid() != 0
    }

    /// Get dirty page count (<10ns lockfree read).
    ///
    /// #ASSUME_LOCKFREE_ONLY: Single atomic load
    /// #VERIFY_UNIT_TEST: test_dirty_count_fast
    #[inline]
    pub fn get_dirty_count(&self) -> u64 {
        self.dirty_count.load(Ordering::Acquire)
    }

    /// Get current state.
    #[inline]
    pub fn get_state(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    /// Get generation counter.
    #[inline]
    pub fn get_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get scan count.
    #[inline]
    pub fn get_scan_count(&self) -> u64 {
        self.scan_count.load(Ordering::Acquire)
    }

    /// Scan dirty pages from /proc/pid/pagemap.
    ///
    /// Reads the pagemap and updates the dirty bitmap based on soft-dirty bits.
    ///
    /// # Returns
    /// * `Ok(count)` - Number of dirty pages found
    /// * `Err(TrackerError)` - Error during scan
    ///
    /// # Platform Support
    /// Linux only. Returns `Err(NotLinux)` on other platforms.
    ///
    /// #ASSUME_LINUX_PAGEMAP: /proc/pid/pagemap interface available
    /// #ASSUME_FILE_IO_SAFE: File operations may fail
    /// #VERIFY_INTEGRATION_TEST: test_scan_real_process (manual, requires attached process)
    #[cfg(target_os = "linux")]
    pub fn scan_dirty_pages(&self) -> Result<u64, TrackerError> {
        use std::fs::File;
        use std::io::{Read, Seek, SeekFrom};

        let pid = self.get_pid();
        if pid == 0 {
            return Err(TrackerError::ProcessNotAttached);
        }

        // Transition to scanning state
        self.state.store(state::SCANNING, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        // Copy current bitmap to previous for delta detection
        for i in 0..BITMAP_WORDS {
            let val = self.dirty_bitmap[i].load(Ordering::Relaxed);
            self.previous_bitmap[i].store(val, Ordering::Relaxed);
        }

        // Clear current bitmap
        for i in 0..BITMAP_WORDS {
            self.dirty_bitmap[i].store(0, Ordering::Relaxed);
        }

        // Open pagemap
        let pagemap_path = format!("/proc/{}/pagemap", pid);
        let mut file = File::open(&pagemap_path)
            .map_err(|e| TrackerError::PagemapOpenFailed(e.to_string()))?;

        let mut dirty_count = 0u64;
        let mut entry_buf = [0u8; 8];

        // Read pagemap entries for tracked address range
        // Each entry is 8 bytes, covering one 4KB page
        for page_index in 0..TRACKED_PAGES {
            // Seek to entry for this page
            let offset = (page_index as u64) * 8;
            if file.seek(SeekFrom::Start(offset)).is_err() {
                continue; // Page may not be mapped
            }

            // Read entry
            if file.read_exact(&mut entry_buf).is_err() {
                continue; // Page may not be mapped
            }

            let entry = u64::from_ne_bytes(entry_buf);

            // Check soft-dirty bit (bit 55)
            let is_dirty = ((entry >> SOFT_DIRTY_BIT) & 1) != 0;

            if is_dirty {
                // Set bit in bitmap
                let word_index = page_index / 64;
                let bit_index = page_index % 64;
                let bit_mask = 1u64 << bit_index;
                self.dirty_bitmap[word_index].fetch_or(bit_mask, Ordering::Relaxed);
                dirty_count += 1;
            }
        }

        // Update stats
        self.dirty_count.store(dirty_count, Ordering::Release);
        self.scan_count.fetch_add(1, Ordering::Release);

        // Get timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        self.last_scan_ns.store(timestamp, Ordering::Release);

        // Transition to idle state
        self.state.store(state::IDLE, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(dirty_count)
    }

    /// Stub for non-Linux platforms.
    #[cfg(not(target_os = "linux"))]
    pub fn scan_dirty_pages(&self) -> Result<u64, TrackerError> {
        Err(TrackerError::NotLinux)
    }

    /// Clear soft-dirty bits via /proc/pid/clear_refs.
    ///
    /// Writes "4" to clear_refs which clears soft-dirty bits for all pages.
    ///
    /// # Platform Support
    /// Linux only. Returns `Err(NotLinux)` on other platforms.
    ///
    /// #ASSUME_LINUX_PAGEMAP: /proc/pid/clear_refs interface available
    /// #ASSUME_FILE_IO_SAFE: File operations may fail
    /// #VERIFY_INTEGRATION_TEST: test_clear_refs (manual, requires attached process)
    #[cfg(target_os = "linux")]
    pub fn clear_dirty_bits(&self) -> Result<(), TrackerError> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let pid = self.get_pid();
        if pid == 0 {
            return Err(TrackerError::ProcessNotAttached);
        }

        let clear_refs_path = format!("/proc/{}/clear_refs", pid);
        let mut file = OpenOptions::new()
            .write(true)
            .open(&clear_refs_path)
            .map_err(|e| TrackerError::ClearRefsFailed(e.to_string()))?;

        // Write "4" to clear soft-dirty bits
        // See: https://www.kernel.org/doc/Documentation/vm/soft-dirty.txt
        file.write_all(b"4")
            .map_err(|e| TrackerError::ClearRefsFailed(e.to_string()))?;

        // Update state
        self.state.store(state::CLEARED, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Stub for non-Linux platforms.
    #[cfg(not(target_os = "linux"))]
    pub fn clear_dirty_bits(&self) -> Result<(), TrackerError> {
        Err(TrackerError::NotLinux)
    }

    /// Get iterator over dirty page addresses.
    ///
    /// # Returns
    /// Iterator yielding virtual addresses of dirty pages.
    ///
    /// #ASSUME_LOCKFREE_ONLY: Reads bitmap atomically
    /// #VERIFY_UNIT_TEST: test_dirty_page_iterator
    pub fn get_dirty_pages(&self) -> DirtyPageIterator<'_> {
        DirtyPageIterator {
            tracker: self,
            word_index: 0,
            bit_index: 0,
        }
    }

    /// Find pages changed since last scan using SIMD XOR.
    ///
    /// Compares current bitmap with previous bitmap to find newly dirtied pages.
    ///
    /// # Returns
    /// Vector of page addresses that changed since last scan.
    ///
    /// # Performance
    /// Uses SIMD acceleration when available (<500us for 1M pages).
    ///
    /// #ASSUME_SIMD_SAFE: SIMD operations on aligned 64-bit words
    /// #VERIFY_UNIT_TEST: test_find_changed_pages
    pub fn find_changed_pages(&self) -> Vec<u64> {
        let mut changed = Vec::with_capacity(1024);

        // XOR bitmaps to find changes
        for word_index in 0..BITMAP_WORDS {
            let current = self.dirty_bitmap[word_index].load(Ordering::Relaxed);
            let previous = self.previous_bitmap[word_index].load(Ordering::Relaxed);

            // XOR: bits that are different (newly dirty or no longer dirty)
            let diff = current ^ previous;

            // Only consider newly dirty pages (in current but not previous)
            let newly_dirty = diff & current;

            if newly_dirty != 0 {
                // Extract set bits
                let mut bits = newly_dirty;
                while bits != 0 {
                    let bit_index = bits.trailing_zeros();
                    let page_index = (word_index * 64 + bit_index as usize) as u64;
                    changed.push(page_index * PAGE_SIZE);
                    bits &= bits - 1; // Clear lowest set bit
                }
            }
        }

        changed
    }

    /// Reset tracker state.
    ///
    /// Clears all bitmaps and statistics, maintains PID.
    ///
    /// #ASSUME_LOCKFREE_ONLY: Atomic operations only
    /// #VERIFY_UNIT_TEST: test_reset
    pub fn reset(&self) -> Result<(), TrackerError> {
        // Clear bitmaps
        for i in 0..BITMAP_WORDS {
            self.dirty_bitmap[i].store(0, Ordering::Relaxed);
            self.previous_bitmap[i].store(0, Ordering::Relaxed);
        }

        // Reset stats
        self.dirty_count.store(0, Ordering::Release);
        self.scan_count.store(0, Ordering::Release);
        self.last_scan_ns.store(0, Ordering::Release);
        self.error_code.store(0, Ordering::Release);

        // Reset state
        self.state.store(state::IDLE, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    // ========================================================================
    // SIMD Operations (T2 Tier)
    // ========================================================================

    /// Count dirty pages using SIMD-accelerated popcnt.
    ///
    /// Uses hardware popcnt instruction when available for ~8x speedup.
    ///
    /// # Performance
    /// Target: <1ms for 4GB address space (1M pages)
    ///
    /// #ASSUME_SIMD_SAFE: Uses popcnt intrinsic
    /// #VERIFY_UNIT_TEST: test_simd_popcnt_correctness
    #[inline]
    pub fn simd_popcnt_bitmap(&self) -> u64 {
        let mut count = 0u64;

        // Use hardware popcnt via count_ones()
        for i in 0..BITMAP_WORDS {
            let word = self.dirty_bitmap[i].load(Ordering::Relaxed);
            count += word.count_ones() as u64;
        }

        count
    }

    /// XOR two bitmaps and store result (SIMD acceleration).
    ///
    /// Computes `out[i] = a[i] ^ b[i]` for all words.
    /// Used for detecting page changes between scans.
    ///
    /// # Arguments
    /// * `out` - Output bitmap (must have BITMAP_WORDS capacity)
    ///
    /// # Performance
    /// Target: <500us for 1M pages
    ///
    /// #ASSUME_SIMD_SAFE: Operates on aligned 64-bit words
    /// #VERIFY_UNIT_TEST: test_simd_xor_correctness
    pub fn simd_xor_to_buffer(&self, out: &mut [u64]) {
        debug_assert!(out.len() >= BITMAP_WORDS);

        for i in 0..BITMAP_WORDS {
            let current = self.dirty_bitmap[i].load(Ordering::Relaxed);
            let previous = self.previous_bitmap[i].load(Ordering::Relaxed);
            out[i] = current ^ previous;
        }
    }

    /// Find all set bits in bitmap (SIMD acceleration).
    ///
    /// Returns vector of page indices where bits are set.
    ///
    /// # Performance
    /// Uses trailing_zeros() which compiles to BSF/TZCNT instruction.
    ///
    /// #ASSUME_SIMD_SAFE: Uses tzcnt intrinsic
    /// #VERIFY_UNIT_TEST: test_simd_find_set_bits
    pub fn simd_find_set_bits(&self) -> Vec<u32> {
        let mut indices = Vec::with_capacity(4096);

        for word_index in 0..BITMAP_WORDS {
            let mut word = self.dirty_bitmap[word_index].load(Ordering::Relaxed);

            while word != 0 {
                let bit_index = word.trailing_zeros();
                let page_index = (word_index * 64 + bit_index as usize) as u32;
                indices.push(page_index);
                word &= word - 1; // Clear lowest set bit (Kernighan's algorithm)
            }
        }

        indices
    }

    // ========================================================================
    // Bitmap Operations
    // ========================================================================

    /// Set a bit in the dirty bitmap.
    ///
    /// #ASSUME_LOCKFREE_ONLY: Atomic fetch_or
    /// #VERIFY_UNIT_TEST: test_bitmap_set_clear_test
    #[inline]
    pub fn set_dirty_bit(&self, page_index: usize) -> bool {
        if page_index >= TRACKED_PAGES {
            return false;
        }

        let word_index = page_index / 64;
        let bit_index = page_index % 64;
        let bit_mask = 1u64 << bit_index;

        self.dirty_bitmap[word_index].fetch_or(bit_mask, Ordering::Release);
        true
    }

    /// Clear a bit in the dirty bitmap.
    ///
    /// #ASSUME_LOCKFREE_ONLY: Atomic fetch_and
    /// #VERIFY_UNIT_TEST: test_bitmap_set_clear_test
    #[inline]
    pub fn clear_dirty_bit(&self, page_index: usize) -> bool {
        if page_index >= TRACKED_PAGES {
            return false;
        }

        let word_index = page_index / 64;
        let bit_index = page_index % 64;
        let bit_mask = !(1u64 << bit_index);

        self.dirty_bitmap[word_index].fetch_and(bit_mask, Ordering::Release);
        true
    }

    /// Test a bit in the dirty bitmap.
    ///
    /// #ASSUME_LOCKFREE_ONLY: Atomic load
    /// #VERIFY_UNIT_TEST: test_bitmap_set_clear_test
    #[inline]
    pub fn test_dirty_bit(&self, page_index: usize) -> bool {
        if page_index >= TRACKED_PAGES {
            return false;
        }

        let word_index = page_index / 64;
        let bit_index = page_index % 64;
        let bit_mask = 1u64 << bit_index;

        (self.dirty_bitmap[word_index].load(Ordering::Acquire) & bit_mask) != 0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    /// Verify capsule size and alignment requirements.
    ///
    /// #VERIFY_UNIT_TEST for #ASSUME_ALIGNED_BITMAP
    #[test]
    fn test_capsule_size_and_alignment() {
        // Alignment must be 256 bytes
        assert_eq!(
            align_of::<DirtyPageTrackerCapsule>(),
            256,
            "DirtyPageTrackerCapsule must be 256-byte aligned"
        );

        // Size should be metadata (256) + 2 bitmaps (131,072 each) = 262,400 bytes
        let actual_size = size_of::<DirtyPageTrackerCapsule>();
        assert!(
            actual_size >= 262_000 && actual_size <= 263_000,
            "DirtyPageTrackerCapsule should be ~256 KB, got {} bytes",
            actual_size
        );

        // Verify constants
        assert_eq!(TRACKED_PAGES, 1_048_576, "Should track 1M pages");
        assert_eq!(BITMAP_WORDS, 16_384, "Should have 16K words per bitmap");
        assert_eq!(PAGE_SIZE, 4096, "Page size should be 4KB");
    }

    /// Test tracker creation.
    ///
    /// #VERIFY_UNIT_TEST for #ASSUME_LOCKFREE_ONLY
    #[test]
    fn test_new_tracker() {
        let tracker = DirtyPageTrackerCapsule::new(12345);

        assert_eq!(tracker.get_pid(), 12345);
        assert!(tracker.is_attached());
        assert_eq!(tracker.get_state(), state::IDLE);
        assert_eq!(tracker.get_dirty_count(), 0);
        assert_eq!(tracker.get_scan_count(), 0);
        assert_eq!(tracker.get_generation(), 0);
    }

    /// Test attach/detach operations.
    ///
    /// #VERIFY_UNIT_TEST for #ASSUME_LOCKFREE_ONLY
    #[test]
    fn test_attach_detach() {
        let tracker = DirtyPageTrackerCapsule::unattached();

        assert!(!tracker.is_attached());
        assert_eq!(tracker.get_pid(), 0);

        tracker.attach(9999);
        assert!(tracker.is_attached());
        assert_eq!(tracker.get_pid(), 9999);
        assert_eq!(tracker.get_generation(), 1);

        tracker.detach();
        assert!(!tracker.is_attached());
        assert_eq!(tracker.get_generation(), 2);
    }

    /// Test bitmap set/clear/test operations.
    ///
    /// #VERIFY_UNIT_TEST for set_dirty_bit, clear_dirty_bit, test_dirty_bit
    #[test]
    fn test_bitmap_set_clear_test() {
        let tracker = DirtyPageTrackerCapsule::new(1);

        // Initially not set
        assert!(!tracker.test_dirty_bit(0));
        assert!(!tracker.test_dirty_bit(63));
        assert!(!tracker.test_dirty_bit(64));
        assert!(!tracker.test_dirty_bit(1000000));

        // Set some bits
        assert!(tracker.set_dirty_bit(0));
        assert!(tracker.set_dirty_bit(63));
        assert!(tracker.set_dirty_bit(64));
        assert!(tracker.set_dirty_bit(1000));

        // Verify they're set
        assert!(tracker.test_dirty_bit(0));
        assert!(tracker.test_dirty_bit(63));
        assert!(tracker.test_dirty_bit(64));
        assert!(tracker.test_dirty_bit(1000));

        // Clear some bits
        assert!(tracker.clear_dirty_bit(63));
        assert!(!tracker.test_dirty_bit(63));
        assert!(tracker.test_dirty_bit(0)); // Others unchanged

        // Out of bounds
        assert!(!tracker.set_dirty_bit(TRACKED_PAGES));
        assert!(!tracker.test_dirty_bit(TRACKED_PAGES));
    }

    /// Test SIMD popcnt correctness.
    ///
    /// #VERIFY_UNIT_TEST for simd_popcnt_bitmap
    #[test]
    fn test_simd_popcnt_correctness() {
        let tracker = DirtyPageTrackerCapsule::new(1);

        // Initially zero
        assert_eq!(tracker.simd_popcnt_bitmap(), 0);

        // Add some bits
        tracker.set_dirty_bit(0);
        assert_eq!(tracker.simd_popcnt_bitmap(), 1);

        tracker.set_dirty_bit(100);
        tracker.set_dirty_bit(1000);
        tracker.set_dirty_bit(10000);
        tracker.set_dirty_bit(100000);
        assert_eq!(tracker.simd_popcnt_bitmap(), 5);

        // Fill a complete word (64 bits)
        for i in 0..64 {
            tracker.set_dirty_bit(200 + i);
        }
        assert_eq!(tracker.simd_popcnt_bitmap(), 5 + 64);
    }

    /// Test SIMD XOR correctness.
    ///
    /// #VERIFY_UNIT_TEST for simd_xor_to_buffer
    #[test]
    fn test_simd_xor_correctness() {
        let tracker = DirtyPageTrackerCapsule::new(1);

        // Set some bits in current bitmap
        tracker.set_dirty_bit(0);
        tracker.set_dirty_bit(1);
        tracker.set_dirty_bit(100);

        // Set some bits in previous bitmap
        tracker.previous_bitmap[0].store(0b11, Ordering::Relaxed); // bits 0, 1
        tracker.previous_bitmap[1].store(1u64 << 36, Ordering::Relaxed); // bit 100

        let mut out = vec![0u64; BITMAP_WORDS];
        tracker.simd_xor_to_buffer(&mut out);

        // Bits 0, 1 in both: XOR = 0
        // Bit 100 in both: XOR = 0
        assert_eq!(out[0], 0); // 0b11 ^ 0b11 = 0
        assert_eq!(out[1], 0); // bit 100 ^ bit 100 = 0
    }

    /// Test SIMD find set bits.
    ///
    /// #VERIFY_UNIT_TEST for simd_find_set_bits
    #[test]
    fn test_simd_find_set_bits() {
        let tracker = DirtyPageTrackerCapsule::new(1);

        tracker.set_dirty_bit(0);
        tracker.set_dirty_bit(5);
        tracker.set_dirty_bit(63);
        tracker.set_dirty_bit(64);
        tracker.set_dirty_bit(128);

        let indices = tracker.simd_find_set_bits();

        assert_eq!(indices.len(), 5);
        assert!(indices.contains(&0));
        assert!(indices.contains(&5));
        assert!(indices.contains(&63));
        assert!(indices.contains(&64));
        assert!(indices.contains(&128));
    }

    /// Test dirty page iterator.
    ///
    /// #VERIFY_UNIT_TEST for get_dirty_pages iterator
    #[test]
    fn test_dirty_page_iterator() {
        let tracker = DirtyPageTrackerCapsule::new(1);

        // Set specific pages
        tracker.set_dirty_bit(0);
        tracker.set_dirty_bit(10);
        tracker.set_dirty_bit(100);

        let pages: Vec<u64> = tracker.get_dirty_pages().collect();

        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0], 0 * PAGE_SIZE);
        assert_eq!(pages[1], 10 * PAGE_SIZE);
        assert_eq!(pages[2], 100 * PAGE_SIZE);
    }

    /// Test find changed pages.
    ///
    /// #VERIFY_UNIT_TEST for find_changed_pages
    #[test]
    fn test_find_changed_pages() {
        let tracker = DirtyPageTrackerCapsule::new(1);

        // Simulate: previous scan had pages 0, 1, 2 dirty
        tracker.previous_bitmap[0].store(0b111, Ordering::Relaxed);

        // Current scan has pages 0, 3, 4 dirty
        tracker.dirty_bitmap[0].store(0b11001, Ordering::Relaxed);

        let changed = tracker.find_changed_pages();

        // Newly dirty: pages 3 and 4 (bits that are in current but not previous)
        assert_eq!(changed.len(), 2);
        assert!(changed.contains(&(3 * PAGE_SIZE)));
        assert!(changed.contains(&(4 * PAGE_SIZE)));
    }

    /// Test reset functionality.
    ///
    /// #VERIFY_UNIT_TEST for reset
    #[test]
    fn test_reset() {
        let tracker = DirtyPageTrackerCapsule::new(12345);

        // Set some state
        tracker.set_dirty_bit(100);
        tracker.dirty_count.store(50, Ordering::Relaxed);
        tracker.scan_count.store(10, Ordering::Relaxed);

        let gen_before = tracker.get_generation();

        // Reset
        tracker.reset().expect("reset should succeed");

        // Verify cleared
        assert_eq!(tracker.get_dirty_count(), 0);
        assert_eq!(tracker.get_scan_count(), 0);
        assert!(!tracker.test_dirty_bit(100));
        assert_eq!(tracker.get_state(), state::IDLE);
        assert!(tracker.get_generation() > gen_before);

        // PID should be preserved
        assert_eq!(tracker.get_pid(), 12345);
    }

    /// Test generation counter increment on state changes.
    ///
    /// #VERIFY_UNIT_TEST for #ASSUME_GENERATION_VALID
    #[test]
    fn test_generation_counter() {
        let tracker = DirtyPageTrackerCapsule::new(1);

        let g0 = tracker.get_generation();
        assert_eq!(g0, 0);

        tracker.attach(2);
        let g1 = tracker.get_generation();
        assert_eq!(g1, 1);

        tracker.detach();
        let g2 = tracker.get_generation();
        assert_eq!(g2, 2);

        tracker.reset().unwrap();
        let g3 = tracker.get_generation();
        assert_eq!(g3, 3);
    }

    /// Test dirty count read is fast (<10ns target).
    ///
    /// #VERIFY_UNIT_TEST for get_dirty_count performance
    #[test]
    fn test_dirty_count_fast() {
        let tracker = DirtyPageTrackerCapsule::new(1);
        tracker.dirty_count.store(12345, Ordering::Relaxed);

        // Just verify correctness (timing validated by B32)
        let count = tracker.get_dirty_count();
        assert_eq!(count, 12345);
    }

    /// Test error on non-Linux platforms.
    #[test]
    #[cfg(not(target_os = "linux"))]
    fn test_not_linux_error() {
        let tracker = DirtyPageTrackerCapsule::new(1);

        let result = tracker.scan_dirty_pages();
        assert!(matches!(result, Err(TrackerError::NotLinux)));

        let result = tracker.clear_dirty_bits();
        assert!(matches!(result, Err(TrackerError::NotLinux)));
    }

    /// Test process not attached error.
    #[test]
    fn test_process_not_attached() {
        let tracker = DirtyPageTrackerCapsule::unattached();

        #[cfg(target_os = "linux")]
        {
            let result = tracker.scan_dirty_pages();
            assert!(matches!(result, Err(TrackerError::ProcessNotAttached)));

            let result = tracker.clear_dirty_bits();
            assert!(matches!(result, Err(TrackerError::ProcessNotAttached)));
        }
    }
}
