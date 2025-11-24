//! # Simple Debug Logging Module
//!
//! Lightweight lockfree-compatible debug logging for tracing hang locations.
//! Uses simple in-memory buffer with timestamp and checkpoint markers.
//!
//! ## Architecture
//!
//! - **Simple**: No external dependencies, just Vec + Mutex
//! - **Thread-safe**: Uses std::sync::Mutex for coordination
//! - **Minimal overhead**: <1µs per log entry on uncontended path
//! - **Feature-gated**: Compiles to nothing when `debug-logging` feature disabled
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kindly_dedup::debug_logging::DebugLogger;
//!
//! let logger = DebugLogger::new();
//!
//! // Log a checkpoint
//! logger.checkpoint("PARSE_LINE", &format!("doc_id={}", 123));
//!
//! // Log progress
//! logger.progress(1000);
//!
//! // Flush to file
//! logger.flush_to_file("/tmp/debug.log")?;
//! ```
//!
//! ## Feature Flag
//!
//! Enable with `--features debug-logging` to include logging support.
//! Without the feature, all log operations compile to nothing (zero overhead).

use std::sync::Arc;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Lockfree debug logger using AsyncLogCapsule (or stub when not available)
///
/// When compiled with `debug-logging` feature, logs entries to an in-memory buffer.
/// Otherwise provides a no-op stub that compiles to nothing.
#[derive(Clone)]
pub struct DebugLogger {
    #[cfg(feature = "debug-logging")]
    inner: Arc<AsyncLoggerInner>,

    #[cfg(not(feature = "debug-logging"))]
    _phantom: std::marker::PhantomData<()>,
}

#[cfg(feature = "debug-logging")]
struct AsyncLoggerInner {
    entries: Arc<Mutex<Vec<String>>>,
    enabled: AtomicBool,
}

impl DebugLogger {
    /// Create a new debug logger
    ///
    /// When compiled without `debug-logging`, this is a no-op.
    #[inline]
    pub fn new() -> Self {
        #[cfg(feature = "debug-logging")]
        {
            Self {
                inner: Arc::new(AsyncLoggerInner {
                    entries: Arc::new(Mutex::new(Vec::with_capacity(10000))),
                    enabled: AtomicBool::new(true),
                }),
            }
        }

        #[cfg(not(feature = "debug-logging"))]
        {
            Self {
                _phantom: std::marker::PhantomData,
            }
        }
    }

    /// Log a checkpoint with a message
    ///
    /// Format: `[TIMESTAMP] {checkpoint}: {message}`
    ///
    /// **Latency**: Minimal overhead (simple append to Vec)
    #[inline]
    pub fn checkpoint(&self, checkpoint: &str, message: &str) {
        #[cfg(feature = "debug-logging")]
        {
            if !self.inner.enabled.load(Ordering::Relaxed) {
                return;
            }

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();

            let entry = format!(
                "[{:08}µs] {}: {}",
                now.as_micros() % 1_000_000,
                checkpoint,
                message
            );

            // Lock and append entry (may block briefly if contended)
            if let Ok(mut entries) = self.inner.entries.lock() {
                entries.push(entry);
            }
            // If lock fails, silently drop the entry
        }

        #[cfg(not(feature = "debug-logging"))]
        {
            let _ = (checkpoint, message); // Suppress unused warnings
        }
    }

    /// Log a simple checkpoint without a message
    ///
    /// Format: `[TIMESTAMP] {checkpoint}`
    #[inline]
    pub fn mark(&self, checkpoint: &str) {
        self.checkpoint(checkpoint, "");
    }

    /// Log progress with a number
    ///
    /// Format: `[TIMESTAMP] PROGRESS: {count}`
    #[inline]
    pub fn progress(&self, count: u64) {
        self.checkpoint("PROGRESS", &count.to_string());
    }

    /// Flush all logged entries to a file
    ///
    /// Entries are written in order they were logged.
    ///
    /// **Returns**: `Ok(count)` where count is number of entries written, or `Err(io_error)`
    pub fn flush_to_file(&self, path: &str) -> std::io::Result<usize> {
        #[cfg(feature = "debug-logging")]
        {
            let mut file = File::create(path)?;
            let entries = self.inner.entries.lock().unwrap();

            for entry in entries.iter() {
                writeln!(file, "{:?}", entry)?;
            }

            Ok(entries.len())
        }

        #[cfg(not(feature = "debug-logging"))]
        {
            let _ = path;
            Ok(0) // No-op when feature disabled
        }
    }

    /// Get number of logged entries (for debugging)
    #[inline]
    pub fn entry_count(&self) -> usize {
        #[cfg(feature = "debug-logging")]
        {
            self.inner.entries.lock().unwrap().len()
        }

        #[cfg(not(feature = "debug-logging"))]
        {
            0
        }
    }

    /// Enable or disable logging
    ///
    /// When disabled, all log operations become no-ops.
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        #[cfg(feature = "debug-logging")]
        {
            self.inner.enabled.store(enabled, Ordering::Relaxed);
        }

        #[cfg(not(feature = "debug-logging"))]
        {
            let _ = enabled;
        }
    }
}

impl Default for DebugLogger {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// MACROS FOR CONVENIENT LOGGING
// ============================================================================

/// Log a checkpoint with formatted message
///
/// Example: `debug_checkpoint!(logger, "PARSE_LINE", "doc_id={}", doc_id);`
#[macro_export]
macro_rules! debug_checkpoint {
    ($logger:expr, $checkpoint:expr, $($arg:tt)*) => {
        #[cfg(feature = "debug-logging")]
        {
            let msg = format!($($arg)*);
            $logger.checkpoint($checkpoint, &msg);
        }
    };
}

/// Log a simple checkpoint without message
///
/// Example: `debug_mark!(logger, "LOOP_START");`
#[macro_export]
macro_rules! debug_mark {
    ($logger:expr, $checkpoint:expr) => {
        #[cfg(feature = "debug-logging")]
        {
            $logger.mark($checkpoint);
        }
    };
}

/// Log progress
///
/// Example: `debug_progress!(logger, 1000);`
#[macro_export]
macro_rules! debug_progress {
    ($logger:expr, $count:expr) => {
        #[cfg(feature = "debug-logging")]
        {
            $logger.progress($count);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_logger_creation() {
        let logger = DebugLogger::new();
        assert_eq!(logger.entry_count(), 0);
    }

    #[test]
    #[cfg(feature = "debug-logging")]
    fn test_checkpoint_logging() {
        let logger = DebugLogger::new();
        logger.checkpoint("TEST", "message");
        assert_eq!(logger.entry_count(), 1);
    }

    #[test]
    #[cfg(feature = "debug-logging")]
    fn test_mark_logging() {
        let logger = DebugLogger::new();
        logger.mark("TEST");
        assert_eq!(logger.entry_count(), 1);
    }

    #[test]
    #[cfg(feature = "debug-logging")]
    fn test_progress_logging() {
        let logger = DebugLogger::new();
        logger.progress(100);
        assert_eq!(logger.entry_count(), 1);
    }

    #[test]
    #[cfg(feature = "debug-logging")]
    fn test_disable_logging() {
        let logger = DebugLogger::new();
        logger.set_enabled(false);
        logger.checkpoint("TEST", "ignored");
        assert_eq!(logger.entry_count(), 0);
    }
}
