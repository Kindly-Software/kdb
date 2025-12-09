//! Shared utilities for field detection and classification.
//!
//! This module provides pure functions for identifying and classifying fields
//! in computational capsules, following Q28 simplicity principles.

use syn::{Field, Fields, ItemStruct};

/// Check if a field name represents a padding field.
///
/// # ASSUME_PADDING_NAMING_CONVENTION
/// Padding fields follow conventions: _padding, _pad, _padding1, _padding2, etc.
///
/// # VERIFY
/// Tests validate all padding naming patterns
///
/// # Arguments
///
/// * `field_name` - The field name to check
///
/// # Returns
///
/// `true` if the field name represents padding, `false` otherwise
#[inline]
pub fn is_padding_field(field_name: &str) -> bool {
    field_name.starts_with("_pad")
}

/// Check if a field is excluded from data size calculations.
///
/// Excluded fields include padding fields and other metadata fields
/// that should not count toward the data size.
///
/// # Arguments
///
/// * `field_name` - The field name to check
///
/// # Returns
///
/// `true` if the field should be excluded, `false` otherwise
#[inline]
pub fn is_excluded_field(field_name: &str) -> bool {
    is_padding_field(field_name)
}

/// Extract all named fields from a struct.
///
/// # Arguments
///
/// * `item_struct` - The struct to extract fields from
///
/// # Returns
///
/// A vector of references to named fields, or empty vector if not named fields
pub fn extract_named_fields(item_struct: &ItemStruct) -> Vec<&Field> {
    match &item_struct.fields {
        Fields::Named(fields_named) => fields_named.named.iter().collect(),
        _ => Vec::new(),
    }
}

/// Find all padding fields in a struct.
///
/// # Arguments
///
/// * `fields` - Vector of field references to search
///
/// # Returns
///
/// Vector of field references that are padding fields
pub fn find_padding_fields<'a>(fields: &'a [&'a Field]) -> Vec<&'a Field> {
    fields
        .iter()
        .filter(|field| {
            field
                .ident
                .as_ref()
                .map(|ident| is_padding_field(&ident.to_string()))
                .unwrap_or(false)
        })
        .copied()
        .collect()
}

/// Find all non-padding (user) fields in a struct.
///
/// # Arguments
///
/// * `fields` - Vector of field references to search
///
/// # Returns
///
/// Vector of field references that are user fields (not padding)
pub fn find_user_fields<'a>(fields: &'a [&'a Field]) -> Vec<&'a Field> {
    fields
        .iter()
        .filter(|field| {
            field
                .ident
                .as_ref()
                .map(|ident| !is_padding_field(&ident.to_string()))
                .unwrap_or(true)
        })
        .copied()
        .collect()
}

/// Estimate size of a Rust type from its string representation.
///
/// # ASSUME_TYPE_STRING_FORMAT
/// Type strings follow standard Rust syntax
///
/// # VERIFY
/// Tests confirm size calculation accuracy
///
/// # Arguments
///
/// * `ty_str` - String representation of the type
///
/// # Returns
///
/// Estimated size in bytes
pub fn estimate_type_size(ty_str: &str) -> usize {
    let ty_clean = ty_str.replace(" ", "");

    // Atomic types (check DualAtomic FIRST, as it contains "Atomic")
    if ty_clean.contains("DualAtomicU64") {
        return 16;
    }
    if ty_clean.contains("AtomicU64") || ty_clean.contains("AtomicI64") {
        return 8;
    }
    if ty_clean.contains("AtomicU32") || ty_clean.contains("AtomicI32") {
        return 4;
    }
    if ty_clean.contains("AtomicU16") || ty_clean.contains("AtomicI16") {
        return 2;
    }
    if ty_clean.contains("AtomicU8") || ty_clean.contains("AtomicI8") || ty_clean.contains("AtomicBool") {
        return 1;
    }

    // Primitive types
    match ty_clean.as_str() {
        "u8" | "i8" | "bool" => 1,
        "u16" | "i16" => 2,
        "u32" | "i32" | "f32" => 4,
        "u64" | "i64" | "f64" => 8,
        "u128" | "i128" => 16,
        "usize" | "isize" => 8, // Assume 64-bit
        _ => {
            // Array types [T; N]
            if ty_clean.starts_with('[') && ty_clean.contains(';') {
                if let Some(semicolon_idx) = ty_clean.find(';') {
                    if let Some(bracket_idx) = ty_clean.rfind(']') {
                        let count_str = &ty_clean[semicolon_idx + 1..bracket_idx];
                        if let Ok(count) = count_str.trim().parse::<usize>() {
                            // Get element type size
                            let elem_type = &ty_clean[1..semicolon_idx];
                            let elem_size = estimate_type_size(elem_type);
                            return elem_size * count;
                        }
                    }
                }
            }
            // Default unknown types to 8 bytes
            8
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_padding_field() {
        assert!(is_padding_field("_padding"));
        assert!(is_padding_field("_padding1"));
        assert!(is_padding_field("_padding2"));
        assert!(is_padding_field("_pad"));
        assert!(is_padding_field("_pad1"));
        assert!(!is_padding_field("state"));
        assert!(!is_padding_field("data"));
    }

    #[test]
    fn test_is_excluded_field() {
        assert!(is_excluded_field("_padding"));
        assert!(is_excluded_field("_padding1"));
        assert!(!is_excluded_field("state"));
    }

    #[test]
    fn test_estimate_type_size_atomics() {
        assert_eq!(estimate_type_size("AtomicU64"), 8);
        assert_eq!(estimate_type_size("AtomicU32"), 4);
        assert_eq!(estimate_type_size("AtomicU16"), 2);
        assert_eq!(estimate_type_size("AtomicU8"), 1);
        assert_eq!(estimate_type_size("AtomicBool"), 1);
        assert_eq!(estimate_type_size("DualAtomicU64"), 16);
    }

    #[test]
    fn test_estimate_type_size_primitives() {
        assert_eq!(estimate_type_size("u64"), 8);
        assert_eq!(estimate_type_size("u32"), 4);
        assert_eq!(estimate_type_size("u16"), 2);
        assert_eq!(estimate_type_size("u8"), 1);
        assert_eq!(estimate_type_size("bool"), 1);
        assert_eq!(estimate_type_size("f32"), 4);
        assert_eq!(estimate_type_size("f64"), 8);
        assert_eq!(estimate_type_size("usize"), 8);
    }

    #[test]
    fn test_estimate_type_size_arrays() {
        assert_eq!(estimate_type_size("[u8; 64]"), 64);
        assert_eq!(estimate_type_size("[u32; 8]"), 32); // 4 * 8
        assert_eq!(estimate_type_size("[u64; 10]"), 80); // 8 * 10
    }

    #[test]
    fn test_estimate_type_size_with_whitespace() {
        assert_eq!(estimate_type_size("[ u8 ; 64 ]"), 64);
        assert_eq!(estimate_type_size("Atomic U64"), 8); // Should still work
    }
}
