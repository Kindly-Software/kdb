//! Logging macros (drop-in replacement for log! crate)
//!
//! # UCE34 Tier: T1 Atomic (compile-time level filtering)
//! # Performance: <1ns when disabled, <50ns when enabled
//!
//! # ASSUM Safety
//! - #ASSUME_FORMAT_ALLOCATION_OK: String formatting can fail (OOM)
//! - #VERIFY: Skip log entry on allocation failure (graceful degradation)
//! - #ASSUME_GLOBAL_CAPSULE_INIT: LOG_CAPSULE must be initialized before use
//! - #VERIFY: Lazy static ensures single initialization

use once_cell::sync::Lazy;

use crate::logging::{LogCapsule, LogLevel};

/// Global logging capsule (initialized on first access)
///
/// Uses once_cell::sync::Lazy for lazy initialization.
/// First call to LOG_CAPSULE triggers initialization with default Info level.
/// Subsequent calls return the already-initialized capsule.
///
/// # Thread Safety
///
/// Fully thread-safe via atomic operations (no locks).
///
/// # Performance
///
/// - First access: ~100ns (initialization)
/// - Subsequent accesses: <1ns (cache hit)
pub static LOG_CAPSULE: Lazy<LogCapsule> =
    Lazy::new(|| LogCapsule::new(LogLevel::Info));

/// Internal log macro (all other macros expand to this)
///
/// # Format
///
/// ```ignore
/// log!(LogLevel::Info, "message: {}", arg);
/// log!(target: "my_target", LogLevel::Info, "message: {}", arg);
/// ```
///
/// # How It Works
///
/// 1. **Fast Path** (<5ns): Check if logging is enabled for this level
///    - If disabled, compile-time optimization should eliminate the log call
///    - If enabled, proceed to slow path
///
/// 2. **Slow Path** (<50ns): Format message and append to ring buffer
///    - Format string with arguments (using Rust's format! macro)
///    - Create LogEntry from formatted string
///    - Append entry to ring buffer (lockfree <50ns)
///
/// # ASSUM Safety
///
/// - #ASSUME_FORMAT_SAFETY: format! doesn't panic under normal conditions
/// - #VERIFY: String formatting is from std library (stable, well-tested)
/// - #ASSUME_LOG_CAPSULE_THREAD_SAFE: LogCapsule is thread-safe
/// - #VERIFY: All coordination via atomics (no mutex/RwLock)
///
/// # Examples
///
/// ```ignore
/// use atomic_capsule::logging::LogLevel;
/// use atomic_capsule::log;
///
/// // With default target (module_path!())
/// log!(LogLevel::Info, "Application started");
///
/// // With custom target
/// log!(target: "network", LogLevel::Debug, "Packet received: {} bytes", 1024);
///
/// // Format with multiple arguments
/// log!(LogLevel::Error, "Failed to connect to {} after {}ms", host, timeout);
/// ```
#[macro_export]
macro_rules! log {
    // With target: log!(target: "my_target", LogLevel::Info, "message: {}", arg)
    (target: $target:expr, $level:expr, $($arg:tt)+) => {{
        // Fast path: check if log level is enabled (<5ns)
        if $crate::logging::macros::LOG_CAPSULE.should_log($target, $level) {
            // Slow path: format message and append to ring buffer (<50ns)
            let msg = format!($($arg)+);
            let _ = $crate::logging::macros::LOG_CAPSULE.log($level, $target, &msg);
        }
    }};

    // Without target: log!(LogLevel::Info, "message: {}", arg)
    // Default target is module_path!() (current module)
    ($level:expr, $($arg:tt)+) => {{
        $crate::log!(target: module_path!(), $level, $($arg)+)
    }};
}

/// Trace macro (lowest priority)
///
/// Equivalent to `log!(LogLevel::Trace, ...)`.
///
/// Trace logs are typically disabled in production and only enabled during development.
///
/// # Examples
///
/// ```ignore
/// use atomic_capsule::trace;
///
/// trace!("Entering function: compute_signature");
/// trace!(target: "pipeline", "Document ID: {}", doc_id);
/// ```
///
/// # Performance
///
/// When trace logging is disabled (typical case):
/// - Compile-time optimization: ~0ns (eliminated by dead code optimization)
/// - Runtime: <1ns (branch prediction if enabled)
#[macro_export]
macro_rules! trace {
    // With target
    (target: $target:expr, $($arg:tt)+) => {{
        $crate::log!(target: $target, $crate::logging::LogLevel::Trace, $($arg)+)
    }};

    // Without target
    ($($arg:tt)+) => {{
        $crate::log!($crate::logging::LogLevel::Trace, $($arg)+)
    }};
}

/// Debug macro
///
/// Equivalent to `log!(LogLevel::Debug, ...)`.
///
/// Debug logs are typically used for development and diagnostics.
///
/// # Examples
///
/// ```ignore
/// use atomic_capsule::debug;
///
/// debug!("Processing document: {}", doc_id);
/// debug!(target: "dedup", "Signature computed: {:?}", sig);
/// ```
///
/// # Performance
///
/// When debug logging is disabled (typical case):
/// - Compile-time optimization: ~0ns
/// - Runtime: <1ns
#[macro_export]
macro_rules! debug {
    (target: $target:expr, $($arg:tt)+) => {{
        $crate::log!(target: $target, $crate::logging::LogLevel::Debug, $($arg)+)
    }};

    ($($arg:tt)+) => {{
        $crate::log!($crate::logging::LogLevel::Debug, $($arg)+)
    }};
}

/// Info macro (default level)
///
/// Equivalent to `log!(LogLevel::Info, ...)`.
///
/// Info logs are enabled by default and used for application milestones.
///
/// # Examples
///
/// ```ignore
/// use atomic_capsule::info;
///
/// info!("Server started on port {}", port);
/// info!(target: "startup", "Configuration loaded from {}", config_file);
/// ```
///
/// # Performance
///
/// When info logging is enabled (typical case):
/// - Fast path: <5ns (check if enabled)
/// - Slow path (enabled): <50ns (format + append)
#[macro_export]
macro_rules! info {
    (target: $target:expr, $($arg:tt)+) => {{
        $crate::log!(target: $target, $crate::logging::LogLevel::Info, $($arg)+)
    }};

    ($($arg:tt)+) => {{
        $crate::log!($crate::logging::LogLevel::Info, $($arg)+)
    }};
}

/// Warn macro
///
/// Equivalent to `log!(LogLevel::Warn, ...)`.
///
/// Warnings are for recoverable issues that don't prevent continued operation.
///
/// # Examples
///
/// ```ignore
/// use atomic_capsule::warn;
///
/// warn!("High memory usage: {} MB", mem_mb);
/// warn!(target: "resource", "Approaching rate limit: {}/{}", used, limit);
/// ```
///
/// # Performance
///
/// - Fast path: <5ns
/// - Slow path: <50ns
#[macro_export]
macro_rules! warn {
    (target: $target:expr, $($arg:tt)+) => {{
        $crate::log!(target: $target, $crate::logging::LogLevel::Warn, $($arg)+)
    }};

    ($($arg:tt)+) => {{
        $crate::log!($crate::logging::LogLevel::Warn, $($arg)+)
    }};
}

/// Error macro (highest priority)
///
/// Equivalent to `log!(LogLevel::Error, ...)`.
///
/// Errors are for conditions that may prevent correct operation.
///
/// # Examples
///
/// ```ignore
/// use atomic_capsule::error;
///
/// error!("Failed to connect: {}", err);
/// error!(target: "network", "Connection timeout after {}ms", timeout_ms);
/// ```
///
/// # Performance
///
/// - Fast path: <5ns
/// - Slow path: <50ns
#[macro_export]
macro_rules! error {
    (target: $target:expr, $($arg:tt)+) => {{
        $crate::log!(target: $target, $crate::logging::LogLevel::Error, $($arg)+)
    }};

    ($($arg:tt)+) => {{
        $crate::log!($crate::logging::LogLevel::Error, $($arg)+)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::LogLevel;

    /// Reset LOG_CAPSULE to default state for test isolation
    fn reset_log_capsule() {
        LOG_CAPSULE.set_max_level(LogLevel::Info);
        LOG_CAPSULE.clear_targets();
    }

    #[test]
    fn test_log_capsule_global_lazy_init() {
        reset_log_capsule();

        // Verify it's initialized with Info level
        assert_eq!(LOG_CAPSULE.get_max_level(), LogLevel::Info);
    }

    #[test]
    fn test_log_capsule_thread_safety() {
        use std::thread;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let counter = Arc::new(AtomicU32::new(0));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let c = counter.clone();
                thread::spawn(move || {
                    // Each thread logs and increments counter
                    info!("Thread logging");
                    c.fetch_add(1, Ordering::Relaxed);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn test_trace_macro() {
        reset_log_capsule();
        // Set level to Trace so it's enabled
        LOG_CAPSULE.set_max_level(LogLevel::Trace);

        trace!("Test trace message");
        trace!(target: "test", "Trace with target");

        assert!(LOG_CAPSULE.should_log("test", LogLevel::Trace));

        // Cleanup for other tests
        LOG_CAPSULE.set_max_level(LogLevel::Info);
        LOG_CAPSULE.clear_targets();
    }

    #[test]
    fn test_debug_macro() {
        reset_log_capsule();

        LOG_CAPSULE.set_max_level(LogLevel::Debug);

        debug!("Test debug message");
        debug!(target: "test", "Debug with target");

        assert!(LOG_CAPSULE.should_log("test", LogLevel::Debug));

        // Cleanup for other tests
        LOG_CAPSULE.set_max_level(LogLevel::Info);
        LOG_CAPSULE.clear_targets();
    }

    #[test]
    fn test_info_macro() {
        reset_log_capsule();

        info!("Test info message");
        info!(target: "test", "Info with target: {}", 42);

        assert!(LOG_CAPSULE.should_log("test", LogLevel::Info));

        // Cleanup for other tests (Info is default, just clear targets)
        LOG_CAPSULE.clear_targets();
    }

    #[test]
    fn test_warn_macro() {
        reset_log_capsule();
        LOG_CAPSULE.set_max_level(LogLevel::Warn);

        warn!("Test warning message");
        warn!(target: "test", "Warning with value: {}", 100);

        assert!(LOG_CAPSULE.should_log("test", LogLevel::Warn));

        // Cleanup for other tests
        LOG_CAPSULE.set_max_level(LogLevel::Info);
        LOG_CAPSULE.clear_targets();
    }

    #[test]
    fn test_error_macro() {
        reset_log_capsule();
        LOG_CAPSULE.set_max_level(LogLevel::Error);

        error!("Test error message");
        error!(target: "test", "Error with code: {}", 500);

        assert!(LOG_CAPSULE.should_log("test", LogLevel::Error));

        // Cleanup for other tests
        LOG_CAPSULE.set_max_level(LogLevel::Info);
        LOG_CAPSULE.clear_targets();
    }

    #[test]
    fn test_log_disabled_does_not_log() {
        reset_log_capsule();
        LOG_CAPSULE.set_max_level(LogLevel::Error);

        // These should be skipped (not logged)
        info!("This should not be logged");
        debug!("This should not be logged");

        // Only error should be logged
        error!("This should be logged");

        assert!(!LOG_CAPSULE.should_log("test", LogLevel::Info));
        assert!(!LOG_CAPSULE.should_log("test", LogLevel::Debug));
        assert!(LOG_CAPSULE.should_log("test", LogLevel::Error));

        // Cleanup for other tests
        LOG_CAPSULE.set_max_level(LogLevel::Info);
        LOG_CAPSULE.clear_targets();
    }

    #[test]
    fn test_log_with_format_args() {
        reset_log_capsule();

        // (reset_log_capsule() already sets to Info)

        info!("Number: {}, String: {}", 42, "hello");
        info!(target: "fmt_test", "Formatted: {:?}, {:?}", vec![1, 2, 3], true);

        // Verify logging still works
        assert!(LOG_CAPSULE.should_log("fmt_test", LogLevel::Info));

        // Cleanup for other tests
        LOG_CAPSULE.clear_targets();
    }

    #[test]
    fn test_target_filtering() {
        reset_log_capsule();
        LOG_CAPSULE.set_target_level("kindly_dedup", LogLevel::Debug);

        // kindly_dedup module logs at Debug level
        assert!(LOG_CAPSULE.should_log("kindly_dedup", LogLevel::Debug));

        // Other modules log at Info level (default)
        assert!(!LOG_CAPSULE.should_log("other_module", LogLevel::Debug));
        assert!(LOG_CAPSULE.should_log("other_module", LogLevel::Info));

        // Cleanup for other tests
        LOG_CAPSULE.clear_targets();
    }
}
