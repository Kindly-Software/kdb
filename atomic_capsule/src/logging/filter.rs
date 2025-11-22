//! Target filtering for module-level log level control
//!
//! # UCE34 Tier: T0 Auditable (simple pattern matching)
//! # Performance: <10ns prefix match (typical case)

use crate::logging::LogLevel;
use std::collections::HashMap;

/// Target filter for module-level log level filtering
///
/// Supports both exact match and prefix matching for module paths.
/// For example, if filter "kindly_dedup" is set to Debug, then:
/// - "kindly_dedup::pipeline" → Debug
/// - "kindly_dedup::utils::hash" → Debug
/// - "other_module::util" → None (not matching)
///
/// # Examples
///
/// ```
/// use atomic_capsule::logging::{TargetFilter, LogLevel};
///
/// let mut filter = TargetFilter::new();
/// filter.add_target("kindly_dedup", LogLevel::Debug);
///
/// assert_eq!(filter.matches("kindly_dedup"), Some(LogLevel::Debug));
/// assert_eq!(filter.matches("kindly_dedup::pipeline"), Some(LogLevel::Debug));
/// assert_eq!(filter.matches("other_module"), None);
/// ```
pub struct TargetFilter {
    filters: HashMap<String, LogLevel>,
}

impl TargetFilter {
    /// Create a new empty target filter
    pub fn new() -> Self {
        Self {
            filters: HashMap::new(),
        }
    }

    /// Add target filter (module path → log level)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::logging::{TargetFilter, LogLevel};
    ///
    /// let mut filter = TargetFilter::new();
    /// filter.add_target("kindly_dedup", LogLevel::Debug);
    /// filter.add_target("atomic_capsule::logging", LogLevel::Trace);
    /// ```
    pub fn add_target(&mut self, target: &str, level: LogLevel) {
        self.filters.insert(target.to_string(), level);
    }

    /// Check if target matches any filter and return corresponding log level
    ///
    /// Uses both exact match and prefix matching:
    /// 1. First tries exact match: "kindly_dedup" == "kindly_dedup"
    /// 2. Then tries prefix match: "kindly_dedup" is prefix of "kindly_dedup::pipeline"
    ///
    /// Returns `Some(LogLevel)` if match found, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::logging::{TargetFilter, LogLevel};
    ///
    /// let mut filter = TargetFilter::new();
    /// filter.add_target("kindly_dedup", LogLevel::Debug);
    /// filter.add_target("atomic_capsule::logging", LogLevel::Trace);
    ///
    /// // Exact match
    /// assert_eq!(filter.matches("kindly_dedup"), Some(LogLevel::Debug));
    ///
    /// // Prefix match
    /// assert_eq!(filter.matches("kindly_dedup::pipeline"), Some(LogLevel::Debug));
    /// assert_eq!(filter.matches("kindly_dedup::utils::hash"), Some(LogLevel::Debug));
    ///
    /// // Exact match (nested)
    /// assert_eq!(filter.matches("atomic_capsule::logging"), Some(LogLevel::Trace));
    /// assert_eq!(filter.matches("atomic_capsule::logging::macros"), Some(LogLevel::Trace));
    ///
    /// // No match
    /// assert_eq!(filter.matches("other_module"), None);
    /// assert_eq!(filter.matches("kindly_deduplicate"), None); // Not a prefix match
    /// ```
    ///
    /// # ASSUM Safety
    /// - #ASSUME_HASHMAP_LOOKUP_SAFE: HashMap lookup is safe and deterministic
    /// - #VERIFY: HashMap only stores valid LogLevel values (added via add_target)
    pub fn matches(&self, target: &str) -> Option<LogLevel> {
        // Fast path: exact match first
        if let Some(&level) = self.filters.get(target) {
            return Some(level);
        }

        // Slow path: prefix match (e.g., "kindly_dedup" matches "kindly_dedup::pipeline")
        for (filter_target, &level) in &self.filters {
            // Check if filter_target is a prefix of target
            if target.starts_with(filter_target) {
                // Ensure it's a proper module path separation ("::" or end of string)
                let remaining = &target[filter_target.len()..];
                if remaining.is_empty() || remaining.starts_with("::") {
                    return Some(level);
                }
            }
        }

        None
    }

    /// Get number of filters
    pub fn len(&self) -> usize {
        self.filters.len()
    }

    /// Check if filter is empty
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    /// Clear all filters
    pub fn clear(&mut self) {
        self.filters.clear();
    }

    /// Get filter for specific target (exact match only)
    pub fn get(&self, target: &str) -> Option<LogLevel> {
        self.filters.get(target).copied()
    }

    /// Iterate over all filters
    pub fn iter(&self) -> impl Iterator<Item = (&String, &LogLevel)> {
        self.filters.iter()
    }
}

impl Default for TargetFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_filter_empty() {
        let filter = TargetFilter::new();
        assert!(filter.is_empty());
        assert_eq!(filter.len(), 0);
        assert_eq!(filter.matches("anything"), None);
    }

    #[test]
    fn test_target_filter_exact_match() {
        let mut filter = TargetFilter::new();
        filter.add_target("kindly_dedup", LogLevel::Debug);

        assert_eq!(filter.matches("kindly_dedup"), Some(LogLevel::Debug));
        assert_eq!(filter.len(), 1);
    }

    #[test]
    fn test_target_filter_prefix_match() {
        let mut filter = TargetFilter::new();
        filter.add_target("kindly_dedup", LogLevel::Debug);

        // Prefix matches should work with "::" separator
        assert_eq!(
            filter.matches("kindly_dedup::pipeline"),
            Some(LogLevel::Debug)
        );
        assert_eq!(
            filter.matches("kindly_dedup::utils::hash"),
            Some(LogLevel::Debug)
        );
    }

    #[test]
    fn test_target_filter_no_false_prefix_match() {
        let mut filter = TargetFilter::new();
        filter.add_target("kindly", LogLevel::Debug);

        // Should NOT match "kindly_dedup" (not a module path)
        assert_eq!(filter.matches("kindly_dedup"), None);

        // Should match "kindly::dedup" (proper module path)
        assert_eq!(filter.matches("kindly::dedup"), Some(LogLevel::Debug));
    }

    #[test]
    fn test_target_filter_nested_exact_match() {
        let mut filter = TargetFilter::new();
        filter.add_target("atomic_capsule::logging", LogLevel::Trace);

        // Exact match
        assert_eq!(
            filter.matches("atomic_capsule::logging"),
            Some(LogLevel::Trace)
        );

        // Prefix match
        assert_eq!(
            filter.matches("atomic_capsule::logging::macros"),
            Some(LogLevel::Trace)
        );

        // No match
        assert_eq!(filter.matches("atomic_capsule"), None);
        assert_eq!(filter.matches("atomic_capsule::other"), None);
    }

    #[test]
    fn test_target_filter_multiple_filters() {
        let mut filter = TargetFilter::new();
        filter.add_target("kindly_dedup", LogLevel::Debug);
        filter.add_target("atomic_capsule", LogLevel::Info);
        filter.add_target("other", LogLevel::Warn);

        assert_eq!(filter.len(), 3);
        assert_eq!(filter.matches("kindly_dedup"), Some(LogLevel::Debug));
        assert_eq!(filter.matches("atomic_capsule"), Some(LogLevel::Info));
        assert_eq!(filter.matches("other"), Some(LogLevel::Warn));
        assert_eq!(filter.matches("unknown"), None);
    }

    #[test]
    fn test_target_filter_get_exact() {
        let mut filter = TargetFilter::new();
        filter.add_target("kindly_dedup", LogLevel::Debug);

        // get() should only do exact match
        assert_eq!(filter.get("kindly_dedup"), Some(LogLevel::Debug));
        assert_eq!(filter.get("kindly_dedup::pipeline"), None); // Not exact
    }

    #[test]
    fn test_target_filter_clear() {
        let mut filter = TargetFilter::new();
        filter.add_target("kindly_dedup", LogLevel::Debug);
        assert!(!filter.is_empty());

        filter.clear();
        assert!(filter.is_empty());
        assert_eq!(filter.matches("kindly_dedup"), None);
    }
}
