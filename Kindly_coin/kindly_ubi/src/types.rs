//! Core types for UBI distribution system

use serde::{Deserialize, Serialize};
use core::fmt;

/// Citizen identifier (0 to 4 billion citizens supported)
///
/// # ASSUM Framework
/// - `#ASSUME_CITIZEN_ID_UNIQUE`: Each citizen has unique ID from biometric verification
/// - `#VERIFY_CITIZEN_ID_UNIQUENESS`: Government biometric system enforces uniqueness
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CitizenId(u32);

impl CitizenId {
    /// Maximum citizen ID (4,294,967,295 = 4.29 billion)
    pub const MAX: u32 = u32::MAX;

    /// Create new citizen ID
    ///
    /// # Errors
    /// Returns error if ID exceeds MAX (which is impossible with u32)
    pub const fn new(id: u32) -> Self {
        CitizenId(id)
    }

    /// Get raw ID value
    pub const fn as_u32(&self) -> u32 {
        self.0
    }

    /// Convert to usize for array indexing
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for CitizenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Citizen#{}", self.0)
    }
}

/// Amount in smallest denomination (satoshi-like)
///
/// # ASSUM Framework
/// - `#ASSUME_AMOUNT_U64`: u64 sufficient for max supply (18.4 quintillion units)
/// - `#VERIFY_AMOUNT_NO_OVERFLOW`: All arithmetic uses checked operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Amount(u64);

impl Amount {
    /// Zero amount
    pub const ZERO: Amount = Amount(0);

    /// Maximum amount (u64::MAX)
    pub const MAX: Amount = Amount(u64::MAX);

    /// Create new amount
    pub const fn new(value: u64) -> Self {
        Amount(value)
    }

    /// Get raw value
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Checked addition
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_NO_OVERFLOW`: Checked arithmetic prevents overflow
    /// - `#VERIFY_OVERFLOW_HANDLING`: Returns None on overflow
    pub const fn checked_add(self, other: Amount) -> Option<Amount> {
        match self.0.checked_add(other.0) {
            Some(sum) => Some(Amount(sum)),
            None => None,
        }
    }

    /// Checked subtraction
    pub const fn checked_sub(self, other: Amount) -> Option<Amount> {
        match self.0.checked_sub(other.0) {
            Some(diff) => Some(Amount(diff)),
            None => None,
        }
    }

    /// Checked multiplication
    pub const fn checked_mul(self, multiplier: u64) -> Option<Amount> {
        match self.0.checked_mul(multiplier) {
            Some(product) => Some(Amount(product)),
            None => None,
        }
    }

    /// Checked division
    pub const fn checked_div(self, divisor: u64) -> Option<Amount> {
        match self.0.checked_div(divisor) {
            Some(quotient) => Some(Amount(quotient)),
            None => None,
        }
    }

    /// Calculate percentage (basis points)
    ///
    /// # Example
    /// ```
    /// # use kindly_ubi::Amount;
    /// let amount = Amount::new(100_000);
    /// let two_percent = amount.percentage(200); // 200 basis points = 2%
    /// assert_eq!(two_percent, Some(Amount::new(2_000)));
    /// ```
    pub const fn percentage(self, basis_points: u64) -> Option<Amount> {
        // basis_points / 10000 * amount
        match self.0.checked_mul(basis_points) {
            Some(product) => match product.checked_div(10_000) {
                Some(result) => Some(Amount(result)),
                None => None,
            },
            None => None,
        }
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display as decimal with 8 decimal places (like BTC)
        let whole = self.0 / 100_000_000;
        let frac = self.0 % 100_000_000;
        write!(f, "{}.{:08}", whole, frac)
    }
}

/// Block height for time-based distribution
///
/// # ASSUM Framework
/// - `#ASSUME_BLOCK_HEIGHT_MONOTONIC`: Block height always increases
/// - `#VERIFY_BLOCK_HEIGHT_CONSENSUS`: Consensus module validates block height
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlockHeight(u64);

impl BlockHeight {
    /// Genesis block height
    pub const GENESIS: BlockHeight = BlockHeight(0);

    /// Create new block height
    pub const fn new(height: u64) -> Self {
        BlockHeight(height)
    }

    /// Get raw height value
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Get next block height
    pub const fn next(self) -> Self {
        BlockHeight(self.0 + 1)
    }

    /// Check if this is genesis block
    pub const fn is_genesis(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for BlockHeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Block#{}", self.0)
    }
}

/// Distribution period (daily, weekly, monthly)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistributionPeriod {
    /// Daily distribution (every 24 hours)
    Daily,
    /// Weekly distribution (every 7 days)
    Weekly,
    /// Monthly distribution (every 30 days)
    Monthly,
}

impl DistributionPeriod {
    /// Get number of blocks per period (assuming 10 min blocks)
    pub const fn blocks_per_period(&self) -> u64 {
        match self {
            DistributionPeriod::Daily => 144,     // 24 hours * 6 blocks/hour
            DistributionPeriod::Weekly => 1_008,  // 7 days * 144 blocks/day
            DistributionPeriod::Monthly => 4_320, // 30 days * 144 blocks/day
        }
    }
}

/// UBI claim status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimStatus {
    /// Claim is pending verification
    Pending,
    /// Claim verified and processed
    Verified,
    /// Claim rejected (fraud detected)
    Rejected,
    /// Claim already processed (duplicate)
    Duplicate,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_citizen_id() {
        let citizen = CitizenId::new(12345);
        assert_eq!(citizen.as_u32(), 12345);
        assert_eq!(format!("{}", citizen), "Citizen#12345");
    }

    #[test]
    fn test_amount_arithmetic() {
        let a = Amount::new(100);
        let b = Amount::new(50);

        assert_eq!(a.checked_add(b), Some(Amount::new(150)));
        assert_eq!(a.checked_sub(b), Some(Amount::new(50)));
        assert_eq!(a.checked_mul(2), Some(Amount::new(200)));
        assert_eq!(a.checked_div(2), Some(Amount::new(50)));
    }

    #[test]
    fn test_amount_percentage() {
        let amount = Amount::new(100_000);

        // 2% (200 basis points)
        assert_eq!(amount.percentage(200), Some(Amount::new(2_000)));

        // 50% (5000 basis points)
        assert_eq!(amount.percentage(5_000), Some(Amount::new(50_000)));

        // 100% (10000 basis points)
        assert_eq!(amount.percentage(10_000), Some(Amount::new(100_000)));
    }

    #[test]
    fn test_amount_overflow() {
        let max = Amount::MAX;
        assert_eq!(max.checked_add(Amount::new(1)), None);
    }

    #[test]
    fn test_block_height() {
        let genesis = BlockHeight::GENESIS;
        assert!(genesis.is_genesis());
        assert_eq!(genesis.next(), BlockHeight::new(1));
    }

    #[test]
    fn test_distribution_period() {
        assert_eq!(DistributionPeriod::Daily.blocks_per_period(), 144);
        assert_eq!(DistributionPeriod::Weekly.blocks_per_period(), 1_008);
        assert_eq!(DistributionPeriod::Monthly.blocks_per_period(), 4_320);
    }
}
