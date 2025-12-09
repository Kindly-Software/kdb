//! # Repr Attribute Validation
//!
//! Validates that #[repr(C, align(N))] matches #[capsule(alignment = N)].

use syn::{DeriveInput, Error, Result};

/// Extract alignment from #[repr(C, align(N))] attribute
///
/// # ASSUM Framework
/// - `#ASSUME_REPR_PRESENT`: Struct has #[repr(...)] attribute
/// - `#VERIFY_REPR`: Returns None if missing or invalid
///
/// # Returns
/// - `Some(alignment)` if #[repr(C, align(N))] found
/// - `None` if no repr attribute or no alignment specified
pub fn extract_repr_alignment(input: &DeriveInput) -> Option<usize> {
    for attr in &input.attrs {
        if attr.path().is_ident("repr") {
            // Try parsing repr content (e.g., "C, align(64)")
            if let Ok(list) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            ) {
                for meta in list {
                    // Check for align(N) meta
                    if let syn::Meta::List(meta_list) = meta {
                        if meta_list.path.is_ident("align") {
                            // Parse align(64) -> 64
                            if let Ok(lit) = meta_list.parse_args::<syn::LitInt>() {
                                if let Ok(alignment) = lit.base10_parse::<usize>() {
                                    return Some(alignment);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Validate that #[repr(C, align(N))] matches #[capsule(alignment = N)]
///
/// # ASSUM Framework
/// - `#ASSUME_REPR_MATCHES_CAPSULE`: User sets both attributes correctly
/// - `#VERIFY_REPR_MATCHES`: Explicit check with clear error message
///
/// # Errors
///
/// Returns compile error if:
/// - Missing #[repr(C, align(N))] attribute
/// - #[repr(...)] alignment doesn't match #[capsule(alignment = ...)]
pub fn validate_repr_alignment(input: &DeriveInput, expected_alignment: usize) -> Result<()> {
    let repr_alignment = extract_repr_alignment(input);

    match repr_alignment {
        None => {
            // Missing #[repr(C, align(N))]
            Err(Error::new_spanned(
                input,
                format!(
                    "Missing or invalid #[repr(C, align(N))] attribute\n\
                     Expected: #[repr(C, align({expected_alignment}))]\n\
                     Help: Add #[repr(C, align({expected_alignment}))] to match capsule alignment\n\
                     \n\
                     Example:\n\
                     #[derive(ComputationalCapsule)]\n\
                     #[capsule(alignment = {expected_alignment})]\n\
                     #[repr(C, align({expected_alignment}))]  // ← Add this!\n\
                     struct MyCapsule {{ ... }}"
                ),
            ))
        }
        Some(actual_alignment) if actual_alignment != expected_alignment => {
            // Mismatched alignment
            Err(Error::new_spanned(
                input,
                format!(
                    "Alignment mismatch between #[repr(...)] and #[capsule(...)]\n\
                     \n\
                     #[capsule(alignment = {expected_alignment})] specifies {expected_alignment} bytes\n\
                     #[repr(C, align({actual_alignment}))] specifies {actual_alignment} bytes\n\
                     \n\
                     These MUST match. Choose one:\n\
                     \n\
                     Option 1: Update repr to match capsule\n\
                     #[repr(C, align({expected_alignment}))]  // Change {actual_alignment} → {expected_alignment}\n\
                     \n\
                     Option 2: Update capsule to match repr\n\
                     #[capsule(alignment = {actual_alignment})]  // Change {expected_alignment} → {actual_alignment}\n\
                     \n\
                     Help: Use alignment = 64 for standard capsules"
                ),
            ))
        }
        Some(_) => {
            // Alignment matches - OK
            Ok(())
        }
    }
}

/// Check if struct has #[repr(C)] or #[repr(C, ...)] attribute
///
/// # ASSUM Framework
/// - `#ASSUME_REPR_C`: Capsules should use #[repr(C)] for deterministic layout
/// - `#VERIFY_REPR_C`: Explicit check
///
/// # Returns
/// - `true` if #[repr(C, ...)] found
/// - `false` otherwise
pub fn has_repr_c(input: &DeriveInput) -> bool {
    for attr in &input.attrs {
        if attr.path().is_ident("repr") {
            if let Ok(list) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            ) {
                for meta in list {
                    if let syn::Meta::Path(path) = meta {
                        if path.is_ident("C") {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Validate that struct has #[repr(C, ...)] for deterministic layout
///
/// # UCE33 Q11 (Rust Transform)
/// Capsules MUST use #[repr(C)] for predictable field layout (cache-aware design).
///
/// # Errors
///
/// Returns compile error if missing #[repr(C)]
pub fn validate_repr_c(input: &DeriveInput) -> Result<()> {
    if !has_repr_c(input) {
        return Err(Error::new_spanned(
            input,
            "Capsules must use #[repr(C)] for deterministic field layout\n\
             \n\
             Computational capsules require predictable memory layout for cache optimization.\n\
             \n\
             Help: Add #[repr(C, align(N))] to your struct:\n\
             \n\
             #[derive(ComputationalCapsule)]\n\
             #[capsule(alignment = 64)]\n\
             #[repr(C, align(64))]  // ← Add this!\n\
             struct MyCapsule { ... }\n\
             \n\
             UCE33 Q11: Rust's #[repr(C)] ensures zero-cost predictable layout",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_extract_repr_alignment_64() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(64))]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        assert_eq!(extract_repr_alignment(&input), Some(64));
    }

    #[test]
    fn test_extract_repr_alignment_128() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(128))]
            struct TestCapsule {
                data: [u8; 128],
            }
        };

        assert_eq!(extract_repr_alignment(&input), Some(128));
    }

    #[test]
    fn test_extract_repr_alignment_missing() {
        let input: DeriveInput = parse_quote! {
            #[repr(C)]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        assert_eq!(extract_repr_alignment(&input), None);
    }

    #[test]
    fn test_validate_repr_alignment_match() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(64))]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        assert!(validate_repr_alignment(&input, 64).is_ok());
    }

    #[test]
    fn test_validate_repr_alignment_mismatch() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(32))]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        let result = validate_repr_alignment(&input, 64);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("mismatch"));
        assert!(err_msg.contains("32"));
        assert!(err_msg.contains("64"));
    }

    #[test]
    fn test_validate_repr_alignment_missing() {
        let input: DeriveInput = parse_quote! {
            #[repr(C)]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        let result = validate_repr_alignment(&input, 64);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing"));
    }

    #[test]
    fn test_has_repr_c_true() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(64))]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        assert!(has_repr_c(&input));
    }

    #[test]
    fn test_has_repr_c_false() {
        let input: DeriveInput = parse_quote! {
            #[repr(align(64))]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        assert!(!has_repr_c(&input));
    }

    #[test]
    fn test_validate_repr_c_ok() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(64))]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        assert!(validate_repr_c(&input).is_ok());
    }

    #[test]
    fn test_validate_repr_c_missing() {
        let input: DeriveInput = parse_quote! {
            #[repr(align(64))]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        let result = validate_repr_c(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("repr(C)"));
    }
}
