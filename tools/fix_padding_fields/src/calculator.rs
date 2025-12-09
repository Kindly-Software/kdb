//! Padding calculation utilities for computational capsules.

use crate::parser::CapsuleInfo;
use anyhow::Result;

/// Calculates required padding for a computational capsule.
///
/// # ASSUME_PADDING_CALCULATION
/// Padding calculation follows standard alignment rules: (align - (size % align)) % align
///
/// # VERIFY
/// Tests confirm padding calculation for all alignment values
pub struct PaddingCalculator {
    capsule: CapsuleInfo,
    total_data_size: usize,
    required_padding: usize,
}

impl PaddingCalculator {
    /// Create a new padding calculator for a capsule.
    ///
    /// # Arguments
    ///
    /// * `capsule` - The capsule information
    ///
    /// # Returns
    ///
    /// A new `PaddingCalculator` instance
    pub fn new(capsule: &CapsuleInfo) -> Result<Self> {
        let total_data_size = capsule.fields.iter().map(|f| f.size_bytes).sum();
        let required_padding = calculate_padding(total_data_size, capsule.alignment);

        Ok(Self {
            capsule: capsule.clone(),
            total_data_size,
            required_padding,
        })
    }

    /// Get total data size (all fields except padding).
    #[inline]
    pub fn total_data_size(&self) -> usize {
        self.total_data_size
    }

    /// Get required padding size to match alignment.
    #[inline]
    pub fn required_padding(&self) -> usize {
        self.required_padding
    }

    /// Check if padding needs fixing (current != required).
    ///
    /// Returns true if:
    /// - No padding fields exist
    /// - Multiple padding fields need consolidation
    /// - Single padding field has wrong size
    pub fn needs_fixing(&self) -> bool {
        // No padding at all
        if self.capsule.padding_fields.is_empty() {
            return true;
        }

        // Multiple padding fields always need consolidation
        if self.capsule.needs_consolidation() {
            return true;
        }

        // Single padding field: check if size matches
        self.capsule.total_padding_size != self.required_padding
    }

    /// Check if this capsule needs consolidation of multiple padding fields.
    #[inline]
    pub fn needs_consolidation(&self) -> bool {
        self.capsule.needs_consolidation()
    }
}

/// Calculate required padding to align structure size to alignment.
///
/// Formula: padding = (alignment - (data_size % alignment)) % alignment
///
/// Examples:
/// - data_size=24, alignment=64 → padding=40
/// - data_size=64, alignment=64 → padding=0
/// - data_size=65, alignment=64 → padding=63
fn calculate_padding(data_size: usize, alignment: usize) -> usize {
    (alignment - (data_size % alignment)) % alignment
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{FieldInfo, PaddingFieldInfo};

    #[test]
    fn test_calculate_padding() {
        // 64-byte alignment
        assert_eq!(calculate_padding(0, 64), 0); // Empty struct
        assert_eq!(calculate_padding(8, 64), 56); // One u64
        assert_eq!(calculate_padding(24, 64), 40); // Three u64s
        assert_eq!(calculate_padding(64, 64), 0); // Exact match
        assert_eq!(calculate_padding(65, 64), 63); // One byte over

        // 128-byte alignment
        assert_eq!(calculate_padding(8, 128), 120);
        assert_eq!(calculate_padding(128, 128), 0);

        // 32-byte alignment
        assert_eq!(calculate_padding(16, 32), 16);
    }

    #[test]
    fn test_needs_fixing_no_padding() {
        let capsule = CapsuleInfo {
            name: "Test".to_string(),
            alignment: 64,
            total_size: 64,
            padding_fields: vec![],
            total_padding_size: 0,
            fields: vec![FieldInfo {
                name: "state".to_string(),
                ty: "AtomicU64".to_string(),
                size_bytes: 8,
            }],
        };

        let calc = PaddingCalculator::new(&capsule).unwrap();
        assert!(calc.needs_fixing());
        assert_eq!(calc.required_padding(), 56);
    }

    #[test]
    fn test_needs_fixing_correct_padding() {
        let capsule = CapsuleInfo {
            name: "Test".to_string(),
            alignment: 64,
            total_size: 64,
            padding_fields: vec![PaddingFieldInfo {
                name: "_padding".to_string(),
                size_bytes: 56,
            }],
            total_padding_size: 56,
            fields: vec![FieldInfo {
                name: "state".to_string(),
                ty: "AtomicU64".to_string(),
                size_bytes: 8,
            }],
        };

        let calc = PaddingCalculator::new(&capsule).unwrap();
        assert!(!calc.needs_fixing());
        assert!(!calc.needs_consolidation());
    }

    #[test]
    fn test_needs_consolidation_multiple_padding() {
        let capsule = CapsuleInfo {
            name: "Test".to_string(),
            alignment: 128,
            total_size: 128,
            padding_fields: vec![
                PaddingFieldInfo {
                    name: "_padding1".to_string(),
                    size_bytes: 8,
                },
                PaddingFieldInfo {
                    name: "_padding2".to_string(),
                    size_bytes: 48,
                },
            ],
            total_padding_size: 56,
            fields: vec![
                FieldInfo {
                    name: "state".to_string(),
                    ty: "AtomicU64".to_string(),
                    size_bytes: 8,
                },
                FieldInfo {
                    name: "counter".to_string(),
                    ty: "AtomicU64".to_string(),
                    size_bytes: 8,
                },
            ],
        };

        let calc = PaddingCalculator::new(&capsule).unwrap();
        assert!(calc.needs_fixing());
        assert!(calc.needs_consolidation());
        assert_eq!(calc.total_data_size(), 16);
        assert_eq!(calc.required_padding(), 112);
    }
}
