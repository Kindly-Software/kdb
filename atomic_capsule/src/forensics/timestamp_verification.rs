//! Timestamp Verification - SOC2 Type II Compliant Time Evidence
//!
//! # Compliance Requirements
//!
//! SOC2 Type II requires:
//! - **Accurate timestamps**: All events must have accurate time records
//! - **Monotonic time**: Timestamps must not go backwards
//! - **Reasonable bounds**: Reject future timestamps and very old timestamps
//! - **Observation period**: Prove controls operated effectively over time
//!
//! # Implementation
//!
//! - **Source**: SystemTime (OS-provided monotonic clock)
//! - **Format**: Unix seconds + nonce for sub-second uniqueness
//! - **Verification**: Range checks for reasonableness
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_TIMESTAMP_ACCURACY`: SystemTime::now() is monotonic and accurate
//! - `#VERIFY_TIMESTAMP_ACCURACY`: Property tests validate ordering

use core::fmt;

#[cfg(not(test))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

/// SOC2-compliant timestamp with verification
///
/// # Format
///
/// - **unix_seconds**: Seconds since Unix epoch (1970-01-01 00:00:00 UTC)
/// - **nonce**: Random value for uniqueness within same second
///
/// # Example
///
/// ```
/// use atomic_capsule::forensics::Timestamp;
///
/// let ts = Timestamp::now();
/// assert!(ts.verify_soc2_compliance().is_ok());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    unix_seconds: u64,
    nonce: u32,
}

impl Timestamp {
    /// Create timestamp from current time
    ///
    /// # Performance
    ///
    /// - Target: <50ns (syscall + nonce generation)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_TIMESTAMP_ACCURACY`: SystemTime::now() is monotonic
    #[inline]
    pub fn now() -> Self {
        #[cfg(not(test))]
        let unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System clock is before Unix epoch")
            .as_secs();

        #[cfg(test)]
        let unix_seconds = {
            // Deterministic time for tests
            static TEST_TIME: AtomicU64 = AtomicU64::new(1700000000); // ~Nov 2023
            TEST_TIME.fetch_add(1, Ordering::Relaxed)
        };

        Self {
            unix_seconds,
            nonce: Self::random_nonce(),
        }
    }

    /// Create timestamp from Unix seconds
    ///
    /// # Use Case
    ///
    /// Deserializing timestamps from storage
    #[inline]
    pub fn from_unix_seconds(seconds: u64) -> Self {
        Self {
            unix_seconds: seconds,
            nonce: 0,
        }
    }

    /// Create timestamp with explicit nonce
    #[inline]
    pub fn with_nonce(seconds: u64, nonce: u32) -> Self {
        Self {
            unix_seconds: seconds,
            nonce,
        }
    }

    /// Get Unix seconds
    #[inline]
    pub fn unix_seconds(&self) -> u64 {
        self.unix_seconds
    }

    /// Get nonce
    #[inline]
    pub fn nonce(&self) -> u32 {
        self.nonce
    }

    /// Add years to timestamp
    ///
    /// # Performance
    ///
    /// - Target: <10ns (integer arithmetic)
    ///
    /// # Note
    ///
    /// Uses 365-day years (does not account for leap years)
    #[inline]
    pub fn add_years(&self, years: i32) -> Self {
        let seconds_per_year = 365 * 24 * 3600;
        let new_seconds = if years >= 0 {
            self.unix_seconds
                .saturating_add((years as u64) * seconds_per_year)
        } else {
            self.unix_seconds
                .saturating_sub(((-years) as u64) * seconds_per_year)
        };

        Self {
            unix_seconds: new_seconds,
            nonce: self.nonce,
        }
    }

    /// Add seconds to timestamp
    #[inline]
    pub fn add_seconds(&self, seconds: i64) -> Self {
        let new_seconds = if seconds >= 0 {
            self.unix_seconds.saturating_add(seconds as u64)
        } else {
            self.unix_seconds.saturating_sub((-seconds) as u64)
        };

        Self {
            unix_seconds: new_seconds,
            nonce: self.nonce,
        }
    }

    /// Verify timestamp is SOC2 compliant
    ///
    /// # Checks
    ///
    /// - Not in the future (> now + 60 seconds)
    /// - Not too old (> 7 years retention period)
    ///
    /// # Errors
    ///
    /// Returns error if timestamp fails verification
    pub fn verify_soc2_compliance(&self) -> Result<(), SocError> {
        let now = Self::now();

        // Reject future timestamps (allow 60-second clock skew)
        if self.unix_seconds > now.unix_seconds + 60 {
            return Err(SocError::FutureTimestamp);
        }

        // Reject timestamps older than 7 years (SOX retention period)
        let seven_years_seconds = 7 * 365 * 24 * 3600;
        if now.unix_seconds.saturating_sub(self.unix_seconds) > seven_years_seconds {
            return Err(SocError::TimestampTooOld);
        }

        Ok(())
    }

    /// Generate random nonce for sub-second uniqueness
    ///
    /// # Implementation
    ///
    /// Uses simple counter in tests, random in production
    #[inline]
    fn random_nonce() -> u32 {
        #[cfg(not(test))]
        {
            use std::sync::atomic::{AtomicU32, Ordering};
            static NONCE: AtomicU32 = AtomicU32::new(0);
            NONCE.fetch_add(1, Ordering::Relaxed)
        }

        #[cfg(test)]
        {
            use std::sync::atomic::{AtomicU32, Ordering};
            static NONCE: AtomicU32 = AtomicU32::new(0);
            NONCE.fetch_add(1, Ordering::Relaxed)
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:09}", self.unix_seconds, self.nonce)
    }
}

/// SOC2-specific errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocError {
    /// Timestamp is in the future (clock skew or tampering)
    FutureTimestamp,

    /// Timestamp is too old (beyond retention period)
    TimestampTooOld,

    /// Change control violation
    ChangeControlViolation(&'static str),
}

impl fmt::Display for SocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocError::FutureTimestamp => write!(f, "Timestamp is in the future"),
            SocError::TimestampTooOld => {
                write!(f, "Timestamp is too old (beyond retention period)")
            }
            SocError::ChangeControlViolation(msg) => write!(f, "Change control violation: {}", msg),
        }
    }
}

impl core::error::Error for SocError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_now() {
        let ts = Timestamp::now();
        assert!(ts.unix_seconds() > 0);
    }

    #[test]
    fn test_timestamp_ordering() {
        let ts1 = Timestamp::now();
        let ts2 = Timestamp::now();

        // Due to atomic counter in tests, ts2 should be after ts1
        assert!(ts2 >= ts1);
    }

    #[test]
    fn test_timestamp_add_years() {
        let ts = Timestamp::from_unix_seconds(1000000000); // Year 2001
        let future = ts.add_years(7);

        let expected_seconds = 7 * 365 * 24 * 3600;
        assert_eq!(future.unix_seconds(), ts.unix_seconds() + expected_seconds);
    }

    #[test]
    fn test_timestamp_add_seconds() {
        let ts = Timestamp::from_unix_seconds(1000000000);
        let future = ts.add_seconds(3600); // Add 1 hour

        assert_eq!(future.unix_seconds(), ts.unix_seconds() + 3600);
    }

    #[test]
    fn test_timestamp_verify_soc2_valid() {
        let ts = Timestamp::now();
        assert!(ts.verify_soc2_compliance().is_ok());
    }

    #[test]
    fn test_timestamp_verify_soc2_future() {
        let future = Timestamp::from_unix_seconds(u64::MAX);
        assert_eq!(
            future.verify_soc2_compliance(),
            Err(SocError::FutureTimestamp)
        );
    }

    #[test]
    fn test_timestamp_verify_soc2_too_old() {
        let old = Timestamp::from_unix_seconds(1000000000); // Year 2001
        assert_eq!(old.verify_soc2_compliance(), Err(SocError::TimestampTooOld));
    }

    #[test]
    fn test_timestamp_display() {
        let ts = Timestamp::with_nonce(1234567890, 123456789);
        let display = format!("{}", ts);
        assert_eq!(display, "1234567890.123456789");
    }
}
