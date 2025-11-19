//! Error handling utilities for proc-macro error messages

use syn::{Error, Ident};

/// Generate helpful compile error for missing repr(C, align(N))
#[allow(dead_code)]
pub fn error_missing_repr(struct_name: &Ident) -> Error {
    Error::new(
        struct_name.span(),
        format!(
            "CapsuleSerialize requires #[repr(C, align(N))] for deterministic layout\n\
             \n\
             Add before struct definition:\n\
             #[repr(C, align(64))]  // or 128, 256\n\
             struct {} {{ ... }}\n\
             \n\
             Why: Fixed-point binary serialization needs predictable field ordering",
            struct_name
        ),
    )
}

/// Generate helpful compile error for invalid field type
#[allow(dead_code)]
pub fn error_invalid_field_type(field_name: &Ident, type_name: &str) -> Error {
    Error::new(
        field_name.span(),
        format!(
            "Field '{}' has unsupported type '{}'\n\
             \n\
             Supported fixed-point types:\n\
             - Q8_8: 1/256 precision (±128.00)\n\
             - Q16_16: 1/65536 precision (±32768.00)\n\
             - Q32_32: 1/4294967296 precision (highest)\n\
             \n\
             Options:\n\
             1. Change to fixed-point type:\n\
                {}: Q16_16,  // Example\n\
             \n\
             2. Exclude from serialization:\n\
                #[capsule_serialize(skip)]\n\
                {}: {},\n\
             \n\
             3. Include in hash only (audit trail):\n\
                #[capsule_serialize(hash_key)]\n\
                {}: {},",
            field_name, type_name, field_name, field_name, type_name, field_name, type_name
        ),
    )
}

/// Generate helpful compile error for conflicting attributes
#[allow(dead_code)]
pub fn error_conflicting_attributes(field_name: &Ident, attr1: &str, attr2: &str) -> Error {
    Error::new(
        field_name.span(),
        format!(
            "Field '{}' has conflicting attributes: {} and {}\n\
             \n\
             Choose one:\n\
             - #[capsule_serialize(skip)] - Exclude from serialization AND hash\n\
             - #[capsule_serialize(hash_key)] - Include in hash ONLY (not serialized)\n\
             \n\
             These are mutually exclusive.",
            field_name, attr1, attr2
        ),
    )
}

/// Generate helpful compile error for no serializable fields
#[allow(dead_code)]
pub fn error_no_serializable_fields(struct_name: &Ident) -> Error {
    Error::new(
        struct_name.span(),
        format!(
            "struct {} has no serializable fields\n\
             \n\
             All fields are marked #[capsule_serialize(skip)] or #[capsule_serialize(hash_key)].\n\
             At least one field must be serialized.\n\
             \n\
             Remove skip/hash_key from at least one fixed-point field.",
            struct_name
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_error_missing_repr() {
        let ident: Ident = parse_quote!(MyCapsule);
        let err = error_missing_repr(&ident);
        assert!(err.to_string().contains("repr(C, align(N))"));
    }

    #[test]
    fn test_error_invalid_field_type() {
        let ident: Ident = parse_quote!(price);
        let err = error_invalid_field_type(&ident, "f64");
        assert!(err.to_string().contains("unsupported type 'f64'"));
        assert!(err.to_string().contains("Q16_16"));
    }

    #[test]
    fn test_error_conflicting_attributes() {
        let ident: Ident = parse_quote!(amount);
        let err = error_conflicting_attributes(&ident, "skip", "hash_key");
        assert!(err.to_string().contains("conflicting attributes"));
    }
}
