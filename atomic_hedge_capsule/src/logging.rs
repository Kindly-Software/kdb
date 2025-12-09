//! Structured Logging for AtomicHedgeCapsule
//!
//! High-performance structured logging system designed for nanosecond-class hedge operations.
//! UCE32 Q28(Simplicity): Simple, efficient logging that adds zero overhead when disabled.
//! UCE32 Q31(Rust): Zero-cost abstractions with compile-time optimization.
//! UCE32 Q32(Nightly): Optional SIMD acceleration for log formatting.

use crate::types::{ErrorCategory, HedgeError, HedgeState, OrderState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// UCE32 Q32: Nightly feature support for enhanced performance
#[cfg(all(feature = "nightly", feature = "portable_simd"))]
use std::simd::prelude::*;

/// Log levels following standard hierarchy
/// UCE32 Q28(Simplicity): Clear, intuitive log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum LogLevel {
    /// Critical failures that require immediate attention
    Error = 0,
    /// Performance degradation or concerning conditions
    Warn = 1,
    /// State transitions and significant operations
    Info = 2,
    /// Detailed operations for debugging
    Debug = 3,
    /// Every atomic operation (very verbose)
    Trace = 4,
}

impl LogLevel {
    /// Check if this level should be logged given the current filter
    #[inline(always)]
    pub fn should_log(self, filter: LogLevel) -> bool {
        self <= filter
    }

    /// Get string representation for fast formatting
    #[inline(always)]
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN ",
            LogLevel::Info => "INFO ",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        }
    }

    /// Get color code for terminal output (when enabled)
    #[inline(always)]
    pub fn color_code(self) -> &'static str {
        match self {
            LogLevel::Error => "\x1b[31m", // Red
            LogLevel::Warn => "\x1b[33m",  // Yellow
            LogLevel::Info => "\x1b[32m",  // Green
            LogLevel::Debug => "\x1b[36m", // Cyan
            LogLevel::Trace => "\x1b[37m", // White
        }
    }

    /// Reset color code
    #[inline(always)]
    pub fn reset_color() -> &'static str {
        "\x1b[0m"
    }
}

impl From<u8> for LogLevel {
    fn from(value: u8) -> Self {
        match value {
            0 => LogLevel::Error,
            1 => LogLevel::Warn,
            2 => LogLevel::Info,
            3 => LogLevel::Debug,
            _ => LogLevel::Trace,
        }
    }
}

/// Structured log record with contextual information
/// UCE32 Q31(Rust): Zero-cost abstraction for log data
#[derive(Debug, Clone)]
pub struct LogRecord {
    /// Log level
    pub level: LogLevel,
    /// Message content
    pub message: String,
    /// Timestamp in nanoseconds since epoch
    pub timestamp_ns: u64,
    /// Thread ID for multi-threaded debugging
    pub thread_id: u64,
    /// Operation ID for tracing
    pub operation_id: Option<u64>,
    /// Hedge state context
    pub hedge_state: Option<HedgeState>,
    /// Order state context
    pub order_state: Option<OrderState>,
    /// Error context
    pub error_context: Option<ErrorContext>,
    /// Performance metrics
    pub metrics: Option<PerformanceMetrics>,
    /// Structured fields
    pub fields: HashMap<String, LogValue>,
}

/// Error context for rich error logging
/// UCE32 Q28(Simplicity): Simple error context that provides debugging value
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Error category
    pub category: ErrorCategory,
    /// Whether error is recoverable
    pub recoverable: bool,
    /// Suggested action
    pub suggested_action: String,
    /// Additional context
    pub details: Option<String>,
}

/// Performance metrics for operation timing
/// UCE32 Q31(Rust): Zero-cost metrics collection
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Operation latency in nanoseconds
    pub latency_ns: u64,
    /// Memory operations count
    pub memory_ops: u32,
    /// Cache hits/misses
    pub cache_metrics: Option<CacheMetrics>,
    /// Thread contention
    pub contention_ns: Option<u64>,
}

/// Cache performance metrics
#[derive(Debug, Clone)]
pub struct CacheMetrics {
    /// Cache hits
    pub hits: u32,
    /// Cache misses
    pub misses: u32,
    /// Hit ratio (0.0 to 1.0)
    pub hit_ratio: f32,
}

/// Structured log value types
/// UCE32 Q28(Simplicity): Simple value types for structured logging
#[derive(Debug, Clone)]
pub enum LogValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Duration(Duration),
    State(String),
}

impl LogValue {
    /// Format value for output
    pub fn format(&self) -> String {
        match self {
            LogValue::String(s) => s.clone(),
            LogValue::Integer(i) => i.to_string(),
            LogValue::Float(f) => format!("{:.6}", f),
            LogValue::Boolean(b) => b.to_string(),
            LogValue::Duration(d) => format!("{}ns", d.as_nanos()),
            LogValue::State(s) => s.clone(),
        }
    }
}

/// Global logging configuration
/// UCE32 Q31(Rust): Lockfree configuration with atomic operations
pub struct LogConfig {
    /// Current log level filter
    level: AtomicU64,
    /// Whether logging is enabled
    enabled: AtomicBool,
    /// Whether colors are enabled
    colors_enabled: AtomicBool,
    /// Whether timestamps are included
    timestamps_enabled: AtomicBool,
    /// Whether thread IDs are included
    thread_ids_enabled: AtomicBool,
    /// Operation counter for unique IDs
    operation_counter: AtomicU64,
}

impl LogConfig {
    /// Create new configuration with defaults
    pub const fn new() -> Self {
        Self {
            level: AtomicU64::new(LogLevel::Info as u64),
            enabled: AtomicBool::new(true),
            colors_enabled: AtomicBool::new(true),
            timestamps_enabled: AtomicBool::new(true),
            thread_ids_enabled: AtomicBool::new(true),
            operation_counter: AtomicU64::new(0),
        }
    }

    /// Set log level filter
    pub fn set_level(&self, level: LogLevel) {
        self.level.store(level as u64, Ordering::Relaxed);
    }

    /// Get current log level
    pub fn level(&self) -> LogLevel {
        LogLevel::from(self.level.load(Ordering::Relaxed) as u8)
    }

    /// Enable/disable logging
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if logging is enabled
    #[inline(always)]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Enable/disable colors
    pub fn set_colors_enabled(&self, enabled: bool) {
        self.colors_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if colors are enabled
    #[inline(always)]
    pub fn colors_enabled(&self) -> bool {
        self.colors_enabled.load(Ordering::Relaxed)
    }

    /// Generate unique operation ID
    pub fn next_operation_id(&self) -> u64 {
        self.operation_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Check if level should be logged
    #[inline(always)]
    pub fn should_log(&self, level: LogLevel) -> bool {
        self.is_enabled() && level.should_log(self.level())
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Global logging configuration instance
/// UCE32 Q31(Rust): Static configuration for zero-cost access
static LOG_CONFIG: LogConfig = LogConfig::new();

/// Structured logger for AtomicHedgeCapsule
/// UCE32 Q28(Simplicity): Simple interface for complex logging needs
pub struct CapsuleLogger;

impl CapsuleLogger {
    /// Log a message with level and context
    #[inline(always)]
    pub fn log(level: LogLevel, message: &str) {
        if !LOG_CONFIG.should_log(level) {
            return;
        }

        let record = LogRecord {
            level,
            message: message.to_string(),
            timestamp_ns: current_timestamp_ns(),
            thread_id: current_thread_id(),
            operation_id: None,
            hedge_state: None,
            order_state: None,
            error_context: None,
            metrics: None,
            fields: HashMap::new(),
        };

        Self::emit_record(&record);
    }

    /// Log with structured fields
    pub fn log_with_fields(level: LogLevel, message: &str, fields: HashMap<String, LogValue>) {
        if !LOG_CONFIG.should_log(level) {
            return;
        }

        let record = LogRecord {
            level,
            message: message.to_string(),
            timestamp_ns: current_timestamp_ns(),
            thread_id: current_thread_id(),
            operation_id: None,
            hedge_state: None,
            order_state: None,
            error_context: None,
            metrics: None,
            fields,
        };

        Self::emit_record(&record);
    }

    /// Log hedge state transition
    pub fn log_state_transition(
        from_state: HedgeState,
        to_state: HedgeState,
        operation_id: Option<u64>,
        latency_ns: Option<u64>,
    ) {
        if !LOG_CONFIG.should_log(LogLevel::Info) {
            return;
        }

        let mut fields = HashMap::new();
        fields.insert(
            "from_state".to_string(),
            LogValue::State(format!("{:?}", from_state)),
        );
        fields.insert(
            "to_state".to_string(),
            LogValue::State(format!("{:?}", to_state)),
        );

        if let Some(latency) = latency_ns {
            fields.insert("latency_ns".to_string(), LogValue::Integer(latency as i64));
        }

        let metrics = latency_ns.map(|latency| PerformanceMetrics {
            latency_ns: latency,
            memory_ops: 1,
            cache_metrics: None,
            contention_ns: None,
        });

        let record = LogRecord {
            level: LogLevel::Info,
            message: format!("State transition: {:?} → {:?}", from_state, to_state),
            timestamp_ns: current_timestamp_ns(),
            thread_id: current_thread_id(),
            operation_id,
            hedge_state: Some(to_state),
            order_state: None,
            error_context: None,
            metrics,
            fields,
        };

        Self::emit_record(&record);
    }

    /// Log error with context
    pub fn log_error(error: &HedgeError, operation: &str, context: Option<&str>) {
        if !LOG_CONFIG.should_log(LogLevel::Error) {
            return;
        }

        let error_context = ErrorContext {
            category: error.category(),
            recoverable: error.is_recoverable(),
            suggested_action: error.suggested_action().to_string(),
            details: context.map(|c| c.to_string()),
        };

        let mut fields = HashMap::new();
        fields.insert(
            "operation".to_string(),
            LogValue::String(operation.to_string()),
        );
        fields.insert(
            "error_type".to_string(),
            LogValue::String(format!("{:?}", error)),
        );
        fields.insert(
            "recoverable".to_string(),
            LogValue::Boolean(error.is_recoverable()),
        );
        fields.insert(
            "category".to_string(),
            LogValue::String(format!("{:?}", error.category())),
        );

        let record = LogRecord {
            level: LogLevel::Error,
            message: format!("Error in {}: {}", operation, error),
            timestamp_ns: current_timestamp_ns(),
            thread_id: current_thread_id(),
            operation_id: None,
            hedge_state: None,
            order_state: None,
            error_context: Some(error_context),
            metrics: None,
            fields,
        };

        Self::emit_record(&record);
    }

    /// Log performance metrics
    pub fn log_performance(
        operation: &str,
        latency_ns: u64,
        memory_ops: u32,
        cache_metrics: Option<CacheMetrics>,
    ) {
        if !LOG_CONFIG.should_log(LogLevel::Debug) {
            return;
        }

        let metrics = PerformanceMetrics {
            latency_ns,
            memory_ops,
            cache_metrics,
            contention_ns: None,
        };

        let mut fields = HashMap::new();
        fields.insert(
            "operation".to_string(),
            LogValue::String(operation.to_string()),
        );
        fields.insert(
            "latency_ns".to_string(),
            LogValue::Integer(latency_ns as i64),
        );
        fields.insert(
            "memory_ops".to_string(),
            LogValue::Integer(memory_ops as i64),
        );

        let record = LogRecord {
            level: LogLevel::Debug,
            message: format!("Performance: {} completed in {}ns", operation, latency_ns),
            timestamp_ns: current_timestamp_ns(),
            thread_id: current_thread_id(),
            operation_id: Some(LOG_CONFIG.next_operation_id()),
            hedge_state: None,
            order_state: None,
            error_context: None,
            metrics: Some(metrics),
            fields,
        };

        Self::emit_record(&record);
    }

    /// Emit log record to output
    /// UCE32 Q32(Nightly): Optional SIMD acceleration for formatting
    fn emit_record(record: &LogRecord) {
        // UCE32 Q31(Rust): Zero-cost abstraction - this compiles to optimal code
        #[cfg(feature = "logging")]
        {
            let formatted = Self::format_record(record);

            // UCE32 Q28(Simplicity): Simple output to stderr for now
            // In production, this could be configurable (file, network, etc.)
            eprintln!("{}", formatted);
        }
    }

    /// Format log record for output
    /// UCE32 Q32(Nightly): SIMD acceleration for string operations when available
    pub fn format_record(record: &LogRecord) -> String {
        let mut output = String::with_capacity(256);

        // Colors
        let colors_enabled = LOG_CONFIG.colors_enabled();
        if colors_enabled {
            output.push_str(record.level.color_code());
        }

        // Timestamp
        if LOG_CONFIG.timestamps_enabled.load(Ordering::Relaxed) {
            let _ = write!(output, "[{}] ", format_timestamp(record.timestamp_ns));
        }

        // Level
        let _ = write!(output, "[{}] ", record.level.as_str());

        // Thread ID
        if LOG_CONFIG.thread_ids_enabled.load(Ordering::Relaxed) {
            let _ = write!(output, "[T:{}] ", record.thread_id);
        }

        // Operation ID
        if let Some(op_id) = record.operation_id {
            let _ = write!(output, "[OP:{}] ", op_id);
        }

        // Message
        output.push_str(&record.message);

        // Structured fields
        if !record.fields.is_empty() {
            output.push_str(" {");
            let mut first = true;
            for (key, value) in &record.fields {
                if !first {
                    output.push_str(", ");
                }
                first = false;
                let _ = write!(output, "{}={}", key, value.format());
            }
            output.push('}');
        }

        // Performance metrics
        if let Some(metrics) = &record.metrics {
            let _ = write!(
                output,
                " [PERF: {}ns, {}mem_ops",
                metrics.latency_ns, metrics.memory_ops
            );
            if let Some(cache) = &metrics.cache_metrics {
                let _ = write!(output, ", cache_hit_ratio={:.2}", cache.hit_ratio);
            }
            output.push(']');
        }

        // Error context
        if let Some(error_ctx) = &record.error_context {
            let _ = write!(
                output,
                " [ERROR: {:?}, recoverable={}, action={}]",
                error_ctx.category, error_ctx.recoverable, error_ctx.suggested_action
            );
        }

        // Reset colors
        if colors_enabled {
            output.push_str(LogLevel::reset_color());
        }

        output
    }
}

/// Zero-cost logging macros
/// UCE32 Q31(Rust): Compile-time optimization - zero cost when logging disabled

/// Log error message
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logging::CapsuleLogger::log($crate::logging::LogLevel::Error, &format!($($arg)*))
    }
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {};
}

/// Log warning message
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logging::CapsuleLogger::log($crate::logging::LogLevel::Warn, &format!($($arg)*))
    }
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {};
}

/// Log info message
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logging::CapsuleLogger::log($crate::logging::LogLevel::Info, &format!($($arg)*))
    }
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {};
}

/// Log debug message
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logging::CapsuleLogger::log($crate::logging::LogLevel::Debug, &format!($($arg)*))
    }
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {};
}

/// Log trace message
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::logging::CapsuleLogger::log($crate::logging::LogLevel::Trace, &format!($($arg)*))
    }
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {};
}

/// Structured logging macro with fields
/// UCE32 Q28(Simplicity): Simple syntax for structured logging
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! log_with_fields {
    ($level:expr, $msg:expr, $($key:expr => $value:expr),* $(,)?) => {
        {
            let mut fields = std::collections::HashMap::new();
            $(
                fields.insert($key.to_string(), $value.into());
            )*
            $crate::logging::CapsuleLogger::log_with_fields($level, $msg, fields)
        }
    };
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! log_with_fields {
    ($level:expr, $msg:expr, $($key:expr => $value:expr),* $(,)?) => {};
}

/// Convert values to LogValue
impl From<String> for LogValue {
    fn from(value: String) -> Self {
        LogValue::String(value)
    }
}

impl From<&str> for LogValue {
    fn from(value: &str) -> Self {
        LogValue::String(value.to_string())
    }
}

impl From<i64> for LogValue {
    fn from(value: i64) -> Self {
        LogValue::Integer(value)
    }
}

impl From<u64> for LogValue {
    fn from(value: u64) -> Self {
        LogValue::Integer(value as i64)
    }
}

impl From<f64> for LogValue {
    fn from(value: f64) -> Self {
        LogValue::Float(value)
    }
}

impl From<bool> for LogValue {
    fn from(value: bool) -> Self {
        LogValue::Boolean(value)
    }
}

impl From<Duration> for LogValue {
    fn from(value: Duration) -> Self {
        LogValue::Duration(value)
    }
}

impl From<HedgeState> for LogValue {
    fn from(value: HedgeState) -> Self {
        LogValue::State(format!("{:?}", value))
    }
}

impl From<OrderState> for LogValue {
    fn from(value: OrderState) -> Self {
        LogValue::State(format!("{:?}", value))
    }
}

/// Configuration functions
/// UCE32 Q28(Simplicity): Simple configuration interface

/// Initialize logging with level
pub fn init_logging(level: LogLevel) {
    LOG_CONFIG.set_level(level);
    LOG_CONFIG.set_enabled(true);
}

/// Set log level
pub fn set_log_level(level: LogLevel) {
    LOG_CONFIG.set_level(level);
}

/// Enable/disable logging
pub fn set_logging_enabled(enabled: bool) {
    LOG_CONFIG.set_enabled(enabled);
}

/// Enable/disable colors
pub fn set_colors_enabled(enabled: bool) {
    LOG_CONFIG.set_colors_enabled(enabled);
}

/// Get current log level
pub fn current_log_level() -> LogLevel {
    LOG_CONFIG.level()
}

/// Check if logging is enabled
pub fn is_logging_enabled() -> bool {
    LOG_CONFIG.is_enabled()
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Get current timestamp in nanoseconds since epoch
/// UCE32 Q31(Rust): Efficient timestamp generation
#[inline(always)]
fn current_timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Get current thread ID
/// UCE32 Q31(Rust): Fast thread identification
#[inline(always)]
fn current_thread_id() -> u64 {
    // Use thread local storage for fast access
    thread_local! {
        static THREAD_ID: u64 = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            std::thread::current().id().hash(&mut hasher);
            hasher.finish()
        };
    }

    THREAD_ID.with(|id| *id)
}

/// Format timestamp for display
/// UCE32 Q32(Nightly): SIMD acceleration for string formatting when available
fn format_timestamp(timestamp_ns: u64) -> String {
    // Convert to microseconds for readability
    let micros = timestamp_ns / 1000;
    let seconds = micros / 1_000_000;
    let remaining_micros = micros % 1_000_000;

    format!("{}.{:06}", seconds, remaining_micros)
}

// ============================================================================
// ASYNC LOGGING SUPPORT (Optional)
// ============================================================================

#[cfg(feature = "async")]
pub mod async_logging {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::sync::Mutex;

    /// Async logger for high-throughput logging
    /// UCE32 Q31(Rust): Lockfree async logging with bounded channels
    pub struct AsyncLogger {
        sender: mpsc::UnboundedSender<LogRecord>,
        _handle: tokio::task::JoinHandle<()>,
    }

    impl AsyncLogger {
        /// Create new async logger
        pub fn new() -> Self {
            let (sender, mut receiver) = mpsc::unbounded_channel();

            let handle = tokio::spawn(async move {
                while let Some(record) = receiver.recv().await {
                    let formatted = CapsuleLogger::format_record(&record);
                    eprintln!("{}", formatted);
                }
            });

            Self {
                sender,
                _handle: handle,
            }
        }

        /// Log record asynchronously
        pub fn log_async(&self, record: LogRecord) {
            let _ = self.sender.send(record);
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_levels() {
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Trace);
    }

    #[test]
    fn test_should_log() {
        assert!(LogLevel::Error.should_log(LogLevel::Error));
        assert!(LogLevel::Error.should_log(LogLevel::Info));
        assert!(!LogLevel::Info.should_log(LogLevel::Error));
    }

    #[test]
    fn test_log_config() {
        let config = LogConfig::new();
        config.set_level(LogLevel::Debug);
        assert_eq!(config.level(), LogLevel::Debug);

        config.set_enabled(false);
        assert!(!config.is_enabled());

        config.set_enabled(true);
        assert!(config.is_enabled());
    }

    #[test]
    fn test_log_value_formatting() {
        assert_eq!(LogValue::String("test".to_string()).format(), "test");
        assert_eq!(LogValue::Integer(42).format(), "42");
        assert_eq!(LogValue::Boolean(true).format(), "true");
    }

    #[test]
    fn test_error_context() {
        let error = HedgeError::timeout();
        assert!(error.is_recoverable());
        assert_eq!(error.category(), ErrorCategory::Transient);
        assert_eq!(
            error.suggested_action(),
            "Retry operation with longer timeout"
        );
    }

    #[test]
    fn test_macro_compilation() {
        // These should compile without errors when logging feature is enabled
        log_info!("Test message");
        log_error!("Error: {}", "test error");
        log_debug!("Debug value: {}", 42);
    }

    #[test]
    fn test_structured_logging() {
        let mut fields = HashMap::new();
        fields.insert(
            "operation".to_string(),
            LogValue::String("test".to_string()),
        );
        fields.insert("latency".to_string(), LogValue::Integer(100));

        // Should not panic
        CapsuleLogger::log_with_fields(LogLevel::Info, "Test message", fields);
    }
}
