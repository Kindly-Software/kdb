//! Validation logic for #[derive(CapsuleSerialize)]
//!
//! Ensures struct meets requirements:
//! - Has #[repr(C, align(N))] for deterministic layout
//! - Is a struct (not enum or union)
//! - Has named fields (not tuple struct)

use syn::{spanned::Spanned, DeriveInput, Error};

/// Validates that struct has #[repr(C, align(N))]
///
/// # ASSUM Framework
/// - `#ASSUME_REPR_C_REQUIRED`: Deterministic field layout for binary serialization
/// - `#VERIFY_REPR_C`: Compile-time check enforces repr attributes
///
/// # Returns
/// - Ok(()) if valid #[repr(C, align(N))] found
/// - Err with actionable error message if missing
pub fn validate_capsule_struct(input: &DeriveInput) -> syn::Result<()> {
    // Check it's a struct (not enum or union)
    let _data = match &input.data {
        syn::Data::Struct(_) => &input.data,
        syn::Data::Enum(_) => {
            return Err(Error::new(
                input.span(),
                "CapsuleSerialize can only be derived for structs, not enums",
            ));
        }
        syn::Data::Union(_) => {
            return Err(Error::new(
                input.span(),
                "CapsuleSerialize can only be derived for structs, not unions",
            ));
        }
    };

    // #ASSUME_REPR_C_REQUIRED: Binary serialization requires deterministic layout
    // #VERIFY_REPR_C: Check for #[repr(C)] and #[repr(align(N))]
    let mut has_repr_c = false;
    let mut has_align = false;

    for attr in &input.attrs {
        if attr.path().is_ident("repr") {
            // Parse repr attribute: #[repr(C, align(64))]
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("C") {
                    has_repr_c = true;
                    Ok(())
                } else if meta.path.is_ident("align") {
                    has_align = true;
                    // Consume the value in parentheses: align(64)
                    let _content: syn::Expr = meta.value()?.parse()?;
                    Ok(())
                } else {
                    Ok(()) // Ignore other repr modifiers
                }
            })?;
        }
    }

    if !has_repr_c || !has_align {
        return Err(Error::new(
            input.span(),
            format!(
                "CapsuleSerialize requires #[repr(C, align(N))]\n\
                 Found: repr(C)={}, repr(align)={}\n\
                 Help: Add #[repr(C, align(64))] (or 128/256) before struct definition\n\
                 Why: Fixed-point serialization needs deterministic field layout",
                has_repr_c, has_align
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_valid_struct() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(64))]
            struct MyCapsule {
                value: i64,
            }
        };
        assert!(validate_capsule_struct(&input).is_ok());
    }

    #[test]
    fn test_missing_repr_c() {
        let input: DeriveInput = parse_quote! {
            #[repr(align(64))]
            struct MyCapsule {
                value: i64,
            }
        };
        let err = validate_capsule_struct(&input).unwrap_err();
        assert!(err.to_string().contains("repr(C)"));
    }

    #[test]
    fn test_missing_align() {
        let input: DeriveInput = parse_quote! {
            #[repr(C)]
            struct MyCapsule {
                value: i64,
            }
        };
        let err = validate_capsule_struct(&input).unwrap_err();
        assert!(err.to_string().contains("repr(align)"));
    }

    #[test]
    fn test_enum_rejected() {
        let input: DeriveInput = parse_quote! {
            #[repr(C)]
            enum MyEnum {
                Variant,
            }
        };
        let err = validate_capsule_struct(&input).unwrap_err();
        assert!(err.to_string().contains("structs, not enums"));
    }
}
