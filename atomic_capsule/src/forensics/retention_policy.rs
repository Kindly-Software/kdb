//! Retention Policy - SOX 7-Year Audit Trail Retention
//!
//! # Compliance Requirements
//!
//! SOX (Sarbanes-Oxley Act) Section 802 requires:
//! - **7-year retention**: All audit trails must be retained for minimum 7 years
//! - **No deletion**: Cannot delete records within retention window
//! - **Tamper-proof**: Retention policy cannot be modified retroactively
//!
//! # Implementation
//!
//! - **Storage**: Retention dates stored immutably in capsule snapshots
//! - **Enforcement**: Garbage collection checks retention before deletion
//! - **Verification**: Audit trail can prove retention compliance
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_TIMESTAMP_ACCURACY`: SystemTime provides accurate timestamps
//! - `#VERIFY_TIMESTAMP_ACCURACY`: Property tests validate timestamp math

use super::timestamp_verification::Timestamp;

/// SOX-compliant retention policy
///
/// # Default
///
/// 7 years (SOX requirement)
///
/// # Example
///
/// ```
/// use atomic_capsule::forensics::RetentionPolicy;
///
/// let policy = RetentionPolicy::sox_compliant();
/// assert_eq!(policy.retention_years(), 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Retention period in years
    retention_years: u32,

    /// Creation timestamp (for calculating expiry)
    created_at: Timestamp,
}

impl RetentionPolicy {
    /// Create SOX-compliant retention policy (7 years)
    ///
    /// # Performance
    ///
    /// - Target: <50ns (timestamp + field assignment)
    #[inline]
    pub fn sox_compliant() -> Self {
        Self::new(7)
    }

    /// Create custom retention policy
    ///
    /// # Arguments
    ///
    /// - `years`: Retention period in years (must be >= 1)
    ///
    /// # Panics
    ///
    /// Panics if `years` is 0
    #[inline]
    pub fn new(years: u32) -> Self {
        assert!(years > 0, "Retention period must be at least 1 year");

        Self {
            retention_years: years,
            created_at: Timestamp::now(),
        }
    }

    /// Create retention policy with explicit creation timestamp
    ///
    /// # Use Case
    ///
    /// Deserializing retention policy from storage
    #[inline]
    pub fn with_timestamp(years: u32, created_at: Timestamp) -> Self {
        assert!(years > 0, "Retention period must be at least 1 year");

        Self {
            retention_years: years,
            created_at,
        }
    }

    /// Get retention period in years
    #[inline]
    pub fn retention_years(&self) -> u32 {
        self.retention_years
    }

    /// Get creation timestamp
    #[inline]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Check if retention policy is still active (not expired)
    ///
    /// # Performance
    ///
    /// - Target: <50ns (timestamp arithmetic + comparison)
    ///
    /// # Returns
    ///
    /// - `true` if retention period has not expired
    /// - `false` if retention period has expired
    #[inline]
    pub fn should_retain(&self) -> bool {
        let now = Timestamp::now();
        let expiry = self.created_at.add_years(self.retention_years as i32);
        now < expiry
    }

    /// Check if retention policy has expired (past retention deadline)
    ///
    /// # Returns
    ///
    /// - `true` if retention period has expired (safe to delete)
    /// - `false` if still within retention period (must keep)
    #[inline]
    pub fn is_expired(&self) -> bool {
        !self.should_retain()
    }

    /// Get expiry timestamp (when retention period ends)
    ///
    /// # Performance
    ///
    /// - Target: <50ns (timestamp arithmetic)
    #[inline]
    pub fn expiry_timestamp(&self) -> Timestamp {
        self.created_at.add_years(self.retention_years as i32)
    }

    /// Get remaining retention time in seconds
    ///
    /// # Returns
    ///
    /// - Positive value: seconds remaining in retention period
    /// - Zero or negative: retention period expired
    #[inline]
    pub fn remaining_seconds(&self) -> i64 {
        let now = Timestamp::now();
        let expiry = self.expiry_timestamp();
        expiry.unix_seconds() as i64 - now.unix_seconds() as i64
    }
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self::sox_compliant()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_policy_sox_compliant() {
        let policy = RetentionPolicy::sox_compliant();
        assert_eq!(policy.retention_years(), 7);
    }

    #[test]
    fn test_retention_policy_custom() {
        let policy = RetentionPolicy::new(5);
        assert_eq!(policy.retention_years(), 5);
    }

    #[test]
    #[should_panic(expected = "Retention period must be at least 1 year")]
    fn test_retention_policy_zero_years_panics() {
        let _ = RetentionPolicy::new(0);
    }

    #[test]
    fn test_retention_policy_should_retain() {
        let policy = RetentionPolicy::new(7);
        // Freshly created policy should always be retained
        assert!(policy.should_retain());
        assert!(!policy.is_expired());
    }

    #[test]
    fn test_retention_policy_expiry() {
        let policy = RetentionPolicy::new(7);
        let expiry = policy.expiry_timestamp();

        // Expiry should be ~7 years in the future
        let now = Timestamp::now();
        let seven_years_seconds = 7 * 365 * 24 * 3600;
        let diff = (expiry.unix_seconds() as i64 - now.unix_seconds() as i64).abs();

        // Allow 1-day tolerance for leap years
        assert!(diff >= seven_years_seconds - 86400);
        assert!(diff <= seven_years_seconds + 86400);
    }

    #[test]
    fn test_retention_policy_remaining_seconds() {
        let policy = RetentionPolicy::new(7);
        let remaining = policy.remaining_seconds();

        // Should be roughly 7 years in seconds
        let seven_years_seconds = 7 * 365 * 24 * 3600;
        assert!(remaining >= seven_years_seconds - 86400); // Allow 1-day tolerance
        assert!(remaining <= seven_years_seconds + 86400);
    }

    #[test]
    fn test_retention_policy_with_timestamp() {
        let past = Timestamp::from_unix_seconds(1000000000); // Year 2001
        let policy = RetentionPolicy::with_timestamp(7, past);

        // Policy from 2001 should be expired by now
        assert!(policy.is_expired());
        assert!(!policy.should_retain());
    }
}
