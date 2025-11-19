//! Fixed-point type detection
//!
//! Detects Q8_8, Q16_16, Q32_32 types from field types.
//! Supports:
//! - Direct types: Q8_8, Q16_16, Q32_32
//! - Generic containers: Option<Q16_16>, Vec<Q16_16>
//! - Type aliases and paths

use syn::{GenericArgument, PathArguments, Type};

/// Fixed-point type variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedPointType {
    /// Q8.8 format (8 integer bits, 8 fractional bits, 1/256 precision)
    Q8_8,
    /// Q16.16 format (16 integer bits, 16 fractional bits, 1/65536 precision)
    Q16_16,
    /// Q32.32 format (32 integer bits, 32 fractional bits, highest precision)
    Q32_32,
}

impl FixedPointType {
    /// Scale factor for converting to/from floating point
    #[allow(dead_code)]
    pub fn scale_factor(&self) -> i64 {
        match self {
            FixedPointType::Q8_8 => 256,
            FixedPointType::Q16_16 => 65536,
            FixedPointType::Q32_32 => 4294967296,
        }
    }

    /// Bit shift for fast multiplication/division
    #[allow(dead_code)]
    pub fn bit_shift(&self) -> u8 {
        match self {
            FixedPointType::Q8_8 => 8,
            FixedPointType::Q16_16 => 16,
            FixedPointType::Q32_32 => 32,
        }
    }

    /// Type name as string
    pub fn type_name(&self) -> &'static str {
        match self {
            FixedPointType::Q8_8 => "Q8_8",
            FixedPointType::Q16_16 => "Q16_16",
            FixedPointType::Q32_32 => "Q32_32",
        }
    }
}

/// Detects fixed-point type from syn::Type
///
/// # ASSUM Framework
/// - `#ASSUME_TYPE_PATHS`: syn parses type paths correctly
/// - `#VERIFY_TYPE_PATHS`: Pattern matching validates structure
///
/// # Examples
///
/// ```rust,ignore
/// detect_fixed_point_type(&parse_quote!(Q16_16)) => Some(FixedPointType::Q16_16)
/// detect_fixed_point_type(&parse_quote!(Option<Q16_16>)) => Some(FixedPointType::Q16_16)
/// detect_fixed_point_type(&parse_quote!(f64)) => None
/// ```
pub fn detect_fixed_point_type(ty: &Type) -> Option<FixedPointType> {
    match ty {
        Type::Path(type_path) => {
            // Extract last segment of path (e.g., "Q16_16" from "atomic_capsule::Q16_16")
            let segment = type_path.path.segments.last()?;
            let ident = &segment.ident;

            // Direct match: Q8_8, Q16_16, Q32_32
            if ident == "Q8_8" {
                return Some(FixedPointType::Q8_8);
            } else if ident == "Q16_16" {
                return Some(FixedPointType::Q16_16);
            } else if ident == "Q32_32" {
                return Some(FixedPointType::Q32_32);
            }

            // Container types: Option<Q16_16>, Vec<Q16_16>, etc.
            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                for arg in &args.args {
                    if let GenericArgument::Type(inner_ty) = arg {
                        // Recursive detection for nested types
                        if let Some(fp_type) = detect_fixed_point_type(inner_ty) {
                            return Some(fp_type);
                        }
                    }
                }
            }

            None
        }
        // Not a fixed-point type
        _ => None,
    }
}

/// Checks if type is a supported fixed-point type
#[allow(dead_code)]
pub fn is_fixed_point_type(ty: &Type) -> bool {
    detect_fixed_point_type(ty).is_some()
}

/// Generates type name for error messages
pub fn type_name_for_error(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => {
            // Extract full path for clarity
            type_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
        }
        Type::Reference(type_ref) => {
            format!("&{}", type_name_for_error(&type_ref.elem))
        }
        Type::Array(type_array) => {
            format!("[{}; ...]", type_name_for_error(&type_array.elem))
        }
        _ => quote::quote!(#ty).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_detect_q8_8() {
        let ty: Type = parse_quote!(Q8_8);
        assert_eq!(detect_fixed_point_type(&ty), Some(FixedPointType::Q8_8));
    }

    #[test]
    fn test_detect_q16_16() {
        let ty: Type = parse_quote!(Q16_16);
        assert_eq!(detect_fixed_point_type(&ty), Some(FixedPointType::Q16_16));
    }

    #[test]
    fn test_detect_q32_32() {
        let ty: Type = parse_quote!(Q32_32);
        assert_eq!(detect_fixed_point_type(&ty), Some(FixedPointType::Q32_32));
    }

    #[test]
    fn test_detect_option_q16_16() {
        let ty: Type = parse_quote!(Option<Q16_16>);
        assert_eq!(detect_fixed_point_type(&ty), Some(FixedPointType::Q16_16));
    }

    #[test]
    fn test_detect_vec_q16_16() {
        let ty: Type = parse_quote!(Vec<Q16_16>);
        assert_eq!(detect_fixed_point_type(&ty), Some(FixedPointType::Q16_16));
    }

    #[test]
    fn test_detect_non_fixed_point() {
        let ty: Type = parse_quote!(f64);
        assert_eq!(detect_fixed_point_type(&ty), None);
    }

    #[test]
    fn test_scale_factors() {
        assert_eq!(FixedPointType::Q8_8.scale_factor(), 256);
        assert_eq!(FixedPointType::Q16_16.scale_factor(), 65536);
        assert_eq!(FixedPointType::Q32_32.scale_factor(), 4294967296);
    }
}
