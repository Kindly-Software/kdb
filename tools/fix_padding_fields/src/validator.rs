//! Validation utilities for padding correctness.

#[allow(dead_code)]
/// Validates that struct size equals alignment.
///
/// # Arguments
///
/// * `size` - Actual struct size
/// * `alignment` - Required alignment
///
/// # Returns
///
/// `true` if size == alignment, `false` otherwise
pub fn validate_size_equals_alignment(size: usize, alignment: usize) -> bool {
    size == alignment
}

#[allow(dead_code)]
/// Validates padding size for alignment.
///
/// # Arguments
///
/// * `data_size` - Size of all data fields
/// * `padding_size` - Size of padding field
/// * `alignment` - Required alignment
///
/// # Returns
///
/// `true` if data_size + padding_size == alignment, `false` otherwise
pub fn validate_padding_size(data_size: usize, padding_size: usize, alignment: usize) -> bool {
    data_size + padding_size == alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_size_equals_alignment() {
        assert!(validate_size_equals_alignment(64, 64));
        assert!(validate_size_equals_alignment(128, 128));
        assert!(!validate_size_equals_alignment(64, 128));
        assert!(!validate_size_equals_alignment(63, 64));
    }

    #[test]
    fn test_validate_padding_size() {
        assert!(validate_padding_size(8, 56, 64));
        assert!(validate_padding_size(24, 40, 64));
        assert!(validate_padding_size(0, 128, 128));
        assert!(!validate_padding_size(8, 55, 64));
        assert!(!validate_padding_size(8, 57, 64));
    }
}
