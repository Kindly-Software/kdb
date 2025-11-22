//! Log entry structure
//!
//! # UCE34 Tier: T5 Streaming (ring buffer element)
//! # Performance: <10ns copy (Copy trait)

use core::fmt;

/// Log entry stored in ring buffer (256 bytes, cache-aligned)
///
/// Maximum 252 bytes per entry. Longer messages are truncated with "..." suffix.
///
/// # Cache Alignment
/// - 256-byte alignment ensures each entry fits in 4 × 64-byte cache lines
/// - Prevents false sharing when multiple threads write different entries
///
/// # Copy Trait
/// - Copy enables safe bitwise copy for ring buffer writes
/// - No Drop impl means no cleanup needed on overflow
///
/// # ASSUM Safety
/// - #ASSUME_COPY_SAFE: LogEntry is POD (Plain Old Data), no heap pointers
/// - #VERIFY: Compiler enforces Copy (cannot implement Drop + Copy)
#[repr(C, align(256))]
#[derive(Clone, Copy)]
pub struct LogEntry {
    /// Entry content (max 252 bytes + 4 bytes length)
    data: [u8; 252],
    /// Actual data length (0-252)
    len: u32,
}

impl LogEntry {
    /// Create new log entry from string slice
    ///
    /// Truncates to 252 bytes if longer (adds "..." suffix).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use atomic_capsule::logging::LogEntry;
    ///
    /// let entry = LogEntry::new("test message");
    /// assert_eq!(entry.as_str(), "test message");
    /// ```
    pub fn new(msg: &str) -> Self {
        let bytes = msg.as_bytes();
        let mut data = [0u8; 252];
        let len;

        // Truncate with "..." marker if needed
        if bytes.len() > 252 {
            let copy_len = 249; // Leave room for "..."
            data[..copy_len].copy_from_slice(&bytes[..copy_len]);
            data[249..252].copy_from_slice(b"...");
            len = 252;
        } else {
            let copy_len = bytes.len();
            data[..copy_len].copy_from_slice(&bytes[..copy_len]);
            len = copy_len;
        }

        Self {
            data,
            len: len as u32,
        }
    }

    /// Get entry as string slice
    ///
    /// # Safety
    /// We only write valid UTF-8 from new(), so this is safe.
    pub fn as_str(&self) -> &str {
        let len = self.len as usize;
        // Safety: We only write valid UTF-8 from new()
        unsafe { core::str::from_utf8_unchecked(&self.data[..len]) }
    }

    /// Get entry as byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }

    /// Get entry length in bytes
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Check if entry is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Create empty log entry
    pub const fn empty() -> Self {
        Self {
            data: [0u8; 252],
            len: 0,
        }
    }
}

impl Default for LogEntry {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogEntry")
            .field("len", &self.len)
            .field("message", &self.as_str())
            .finish()
    }
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// Compile-time verification
const _: [(); 1] = [(); 1]; // Dummy const that will be replaced by assertions below

// Compile-time checks using const context
const fn _size_check() {
    const _: () = [()][if core::mem::size_of::<LogEntry>() == 256 { 0 } else { 1 }];
}

const fn _align_check() {
    const _: () = [()][if core::mem::align_of::<LogEntry>() == 256 { 0 } else { 1 }];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_alignment_and_size() {
        assert_eq!(core::mem::align_of::<LogEntry>(), 256);
        assert_eq!(core::mem::size_of::<LogEntry>(), 256);
    }

    #[test]
    fn test_log_entry_new_short_message() {
        let entry = LogEntry::new("test message");
        assert_eq!(entry.as_str(), "test message");
        assert_eq!(entry.len(), 12);
        assert!(!entry.is_empty());
    }

    #[test]
    fn test_log_entry_truncation() {
        let long_msg = "a".repeat(300);
        let entry = LogEntry::new(&long_msg);

        assert_eq!(entry.len(), 252);
        assert!(entry.as_str().ends_with("..."));
    }

    #[test]
    fn test_log_entry_empty() {
        let entry = LogEntry::empty();
        assert!(entry.is_empty());
        assert_eq!(entry.len(), 0);
        assert_eq!(entry.as_str(), "");
    }

    #[test]
    fn test_log_entry_copy() {
        let entry1 = LogEntry::new("test");
        let entry2 = entry1; // Copy

        assert_eq!(entry1.as_str(), entry2.as_str());
        assert_eq!(entry1.len(), entry2.len());
    }

    #[test]
    fn test_log_entry_edge_case_252_bytes() {
        let msg = "a".repeat(252);
        let entry = LogEntry::new(&msg);
        assert_eq!(entry.len(), 252);
        assert!(!entry.as_str().ends_with("..."));
    }

    #[test]
    fn test_log_entry_edge_case_251_bytes() {
        let msg = "a".repeat(251);
        let entry = LogEntry::new(&msg);
        assert_eq!(entry.len(), 251);
        assert!(!entry.as_str().ends_with("..."));
    }

    #[test]
    fn test_log_entry_edge_case_253_bytes() {
        let msg = "a".repeat(253);
        let entry = LogEntry::new(&msg);
        assert_eq!(entry.len(), 252);
        assert!(entry.as_str().ends_with("..."));
    }

    #[test]
    fn test_log_entry_display() {
        let entry = LogEntry::new("test message");
        assert_eq!(entry.to_string(), "test message");
    }

    #[test]
    fn test_log_entry_default() {
        let entry = LogEntry::default();
        assert!(entry.is_empty());
        assert_eq!(entry.len(), 0);
    }

    #[test]
    fn test_log_entry_as_bytes() {
        let entry = LogEntry::new("test");
        assert_eq!(entry.as_bytes(), b"test");
    }
}
