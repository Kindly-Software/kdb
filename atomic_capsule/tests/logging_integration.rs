#[cfg(feature = "logging")]
mod logging_integration_tests {
    use atomic_capsule::logging::{LogLevel, LogEntry, EnvLoggerCapsule};

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Trace);
    }

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new("Test message");
        assert!(!entry.is_empty());
        assert_eq!(entry.as_str(), "Test message");
    }

    #[test]
    fn test_env_logger_parse_global_level() {
        let filters = EnvLoggerCapsule::parse_rust_log("debug").unwrap();
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].0, "");
        assert_eq!(filters[0].1, LogLevel::Debug);
    }

    #[test]
    fn test_env_logger_parse_target_level() {
        let filters = EnvLoggerCapsule::parse_rust_log("kindly_dedup=trace").unwrap();
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].0, "kindly_dedup");
        assert_eq!(filters[0].1, LogLevel::Trace);
    }

    #[test]
    fn test_env_logger_parse_multiple_targets() {
        let filters = EnvLoggerCapsule::parse_rust_log("debug,kindly_dedup=trace,other=info").unwrap();
        assert_eq!(filters.len(), 3);
        assert_eq!(filters[0].0, "");
        assert_eq!(filters[0].1, LogLevel::Debug);
        assert_eq!(filters[1].0, "kindly_dedup");
        assert_eq!(filters[1].1, LogLevel::Trace);
        assert_eq!(filters[2].0, "other");
        assert_eq!(filters[2].1, LogLevel::Info);
    }

    #[test]
    fn test_env_logger_init() {
        // Should not panic or error
        let result = EnvLoggerCapsule::init();
        assert!(result.is_ok());
    }

    #[test]
    fn test_api_exports() {
        // Verify all public APIs are accessible
        use atomic_capsule::logging::{LogLevel, LogEntry, LogCapsule, LogOutputCapsule};
        use atomic_capsule::logging::{EnvLoggerCapsule, LogError};
        use atomic_capsule::logging::TargetFilter;

        // If this compiles, the APIs are exported
        std::mem::drop((LogLevel::Info, LogEntry::empty(), LogError::RingFull { capacity: 100 }));
        let _ = LogCapsule::new(LogLevel::Info);
        let _ = LogOutputCapsule::new(LogLevel::Info);
        let _ = TargetFilter::new();
        let _ = EnvLoggerCapsule::init();
    }
}
