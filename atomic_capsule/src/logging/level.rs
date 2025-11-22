//! Log level enumeration
//!
//! # UCE34 Tier: T1 Atomic (AtomicU8 storage)
//! # Performance: <1ns level comparison

use core::fmt;

/// Log level enumeration (5 levels + Off)
///
/// Levels are ordered from most critical (Error) to least critical (Trace).
/// Repr(u8) ensures atomic load/store compatibility with AtomicU8.
///
/// # Examples
///
/// ```ignore
/// use atomic_capsule::logging::LogLevel;
///
/// assert!(LogLevel::Error > LogLevel::Warn);
/// assert!(LogLevel::Debug > LogLevel::Info);
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    /// Logging is completely disabled
    Off = 0,
    /// Only log errors (highest priority)
    Error = 1,
    /// Log warnings and errors
    Warn = 2,
    /// Log info, warnings, and errors (default)
    Info = 3,
    /// Log debug, info, warnings, and errors
    Debug = 4,
    /// Log everything (lowest priority)
    Trace = 5,
}

impl LogLevel {
    /// Parse log level from string (case-insensitive)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use atomic_capsule::logging::LogLevel;
    ///
    /// assert_eq!(LogLevel::from_str("info"), Some(LogLevel::Info));
    /// assert_eq!(LogLevel::from_str("DEBUG"), Some(LogLevel::Debug));
    /// assert_eq!(LogLevel::from_str("invalid"), None);
    /// ```
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" => Some(Self::Off),
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }

    /// Convert log level to string
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use atomic_capsule::logging::LogLevel;
    ///
    /// assert_eq!(LogLevel::Info.as_str(), "info");
    /// assert_eq!(LogLevel::Debug.as_str(), "debug");
    /// ```
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    /// Convert to u8 (for atomic storage)
    #[inline(always)]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Convert from u8 (for atomic load)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use atomic_capsule::logging::LogLevel;
    ///
    /// assert_eq!(LogLevel::from_u8(0), Some(LogLevel::Off));
    /// assert_eq!(LogLevel::from_u8(5), Some(LogLevel::Trace));
    /// assert_eq!(LogLevel::from_u8(6), None);
    /// ```
    #[inline(always)]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Off),
            1 => Some(Self::Error),
            2 => Some(Self::Warn),
            3 => Some(Self::Info),
            4 => Some(Self::Debug),
            5 => Some(Self::Trace),
            _ => None,
        }
    }
}

impl Default for LogLevel {
    /// Default log level is Info
    fn default() -> Self {
        Self::Info
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Trace);
        assert!(LogLevel::Off < LogLevel::Error);
    }

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("DEBUG"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str("invalid"), None);
        assert_eq!(LogLevel::from_str("off"), Some(LogLevel::Off));
        assert_eq!(LogLevel::from_str("trace"), Some(LogLevel::Trace));
    }

    #[test]
    fn test_log_level_to_u8() {
        assert_eq!(LogLevel::Off.to_u8(), 0);
        assert_eq!(LogLevel::Error.to_u8(), 1);
        assert_eq!(LogLevel::Warn.to_u8(), 2);
        assert_eq!(LogLevel::Info.to_u8(), 3);
        assert_eq!(LogLevel::Debug.to_u8(), 4);
        assert_eq!(LogLevel::Trace.to_u8(), 5);
    }

    #[test]
    fn test_log_level_from_u8() {
        assert_eq!(LogLevel::from_u8(0), Some(LogLevel::Off));
        assert_eq!(LogLevel::from_u8(1), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_u8(5), Some(LogLevel::Trace));
        assert_eq!(LogLevel::from_u8(6), None);
        assert_eq!(LogLevel::from_u8(255), None);
    }

    #[test]
    fn test_log_level_default() {
        assert_eq!(LogLevel::default(), LogLevel::Info);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Error.to_string(), "error");
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Trace.to_string(), "trace");
    }

    #[test]
    fn test_log_level_equality() {
        assert_eq!(LogLevel::Info, LogLevel::Info);
        assert_ne!(LogLevel::Info, LogLevel::Debug);
        assert!(LogLevel::Error != LogLevel::Warn);
    }
}
