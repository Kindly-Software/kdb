//! Field parsing for #[derive(CapsuleSerialize)]
//!
//! Parses struct fields and detects:
//! - Fixed-point types (Q8_8, Q16_16, Q32_32)
//! - Field attributes (#[capsule_serialize(skip)], #[capsule_serialize(hash_key)], #[capsule_serialize(skip_if = "...")])
//! - Invalid types (generates helpful error messages)

use crate::type_detector::{detect_fixed_point_type, type_name_for_error, FixedPointType};
use syn::{spanned::Spanned, DeriveInput, Error, Field, Fields, Type};

/// Parsed skip-if predicate (T0 compile-time data structure)
#[derive(Debug, Clone)]
pub enum SkipPredicate {
    /// Skip if `Option::is_none()`
    OptionIsNone,
    /// Skip if `Vec::is_empty()`
    VecIsEmpty,
    /// Skip if `String::is_empty()`
    StringIsEmpty,
    /// Skip if value == 0
    IsZero,
    /// Skip if value == false
    IsFalse,
    /// Custom predicate path (e.g., "my_module::is_special")
    CustomPath(String),
}

impl SkipPredicate {
    /// Parse a predicate string into a SkipPredicate enum
    pub fn parse(path: &str) -> Self {
        match path {
            "Option::is_none" => SkipPredicate::OptionIsNone,
            "Vec::is_empty" => SkipPredicate::VecIsEmpty,
            "String::is_empty" => SkipPredicate::StringIsEmpty,
            "is_zero" => SkipPredicate::IsZero,
            "is_false" => SkipPredicate::IsFalse,
            custom => SkipPredicate::CustomPath(custom.to_string()),
        }
    }

    /// Validate that predicate string is well-formed
    pub fn validate(predicate_str: &str) -> Result<(), String> {
        // Check for empty string
        if predicate_str.is_empty() {
            return Err("skip_if predicate cannot be empty".to_string());
        }

        // Built-in predicates always valid
        if matches!(
            predicate_str,
            "Option::is_none" | "Vec::is_empty" | "String::is_empty" | "is_zero" | "is_false"
        ) {
            return Ok(());
        }

        // Validate custom paths: should contain only valid identifiers and ::
        for segment in predicate_str.split("::") {
            if segment.is_empty() {
                return Err(format!(
                    "Invalid predicate path '{}': empty segment",
                    predicate_str
                ));
            }

            // Check first character is alphabetic or underscore
            if !segment
                .chars()
                .next()
                .map(|c| c.is_alphabetic() || c == '_')
                .unwrap_or(false)
            {
                return Err(format!(
                    "Invalid predicate path '{}': segment '{}' must start with letter or underscore",
                    predicate_str, segment
                ));
            }

            // Check remaining characters are alphanumeric or underscore
            if !segment
                .chars()
                .skip(1)
                .all(|c| c.is_alphanumeric() || c == '_')
            {
                return Err(format!(
                    "Invalid predicate path '{}': segment '{}' contains invalid characters",
                    predicate_str, segment
                ));
            }
        }

        Ok(())
    }
}

/// Parsed field information
#[derive(Debug, Clone)]
pub struct CapsuleField {
    /// Field name (ident)
    pub name: syn::Ident,
    /// Field type (original syn::Type)
    pub ty: syn::Type,
    /// Detected fixed-point type (if any)
    pub fp_type: Option<FixedPointType>,
    /// Skip field during serialization
    pub skip: bool,
    /// Include in hash but not serialization (audit keys)
    pub hash_key: bool,
    /// Mark field as previous hash for hash chain (Q34 Auditability)
    pub prev_hash: bool,
    /// Conditional skip predicate (e.g., "Option::is_none")
    pub skip_if: Option<SkipPredicate>,
}

/// Struct-level configuration options
#[derive(Debug, Clone, Default)]
pub struct CapsuleConfig {
    /// Auto-generate CRC32 verification methods
    pub auto_crc: bool,
}

/// Parse field attributes (#[capsule_serialize(...)])
///
/// Supports:
/// - `#[capsule_serialize(skip)]`: Exclude from serialization + hash
/// - `#[capsule_serialize(hash_key)]`: Include in hash only (not serialized)
/// - `#[capsule_serialize(prev_hash)]`: Mark as previous hash for hash chain
/// - `#[capsule_serialize(skip_if = "...")]`: Conditional skip predicate
///
/// # ASSUM Framework
/// - `#ASSUME_ATTR_PARSE`: syn parses attributes correctly
/// - `#VERIFY_ATTR_PARSE`: Match arms handle all valid cases
fn parse_field_attributes(field: &Field) -> syn::Result<(bool, bool, bool, Option<SkipPredicate>)> {
    let mut skip = false;
    let mut hash_key = false;
    let mut prev_hash = false;
    let mut skip_if: Option<SkipPredicate> = None;

    for attr in &field.attrs {
        // Check if attribute is #[capsule_serialize(...)]
        if attr.path().is_ident("capsule_serialize") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    skip = true;
                    Ok(())
                } else if meta.path.is_ident("hash_key") {
                    hash_key = true;
                    Ok(())
                } else if meta.path.is_ident("prev_hash") {
                    prev_hash = true;
                    Ok(())
                } else if meta.path.is_ident("skip_if") {
                    // Parse skip_if = "..."
                    let value = meta.value()?;
                    let lit_str: syn::LitStr = value.parse()?;
                    let predicate_str = lit_str.value();

                    // Validate predicate
                    if let Err(err) = SkipPredicate::validate(&predicate_str) {
                        return Err(meta.error(format!(
                            "Invalid skip_if predicate: {}",
                            err
                        )));
                    }

                    skip_if = Some(SkipPredicate::parse(&predicate_str));
                    Ok(())
                } else {
                    Err(meta.error(format!(
                        "Unknown capsule_serialize attribute: {:?}\n\
                         Valid options: skip, hash_key, prev_hash, skip_if = \"...\"",
                        meta.path.get_ident()
                    )))
                }
            })?;
        }
    }

    // Validation: skip and hash_key are mutually exclusive
    if skip && hash_key {
        return Err(Error::new(
            field.span(),
            "Field cannot be both #[capsule_serialize(skip)] and #[capsule_serialize(hash_key)]",
        ));
    }

    // Validation: skip and skip_if are mutually exclusive
    if skip && skip_if.is_some() {
        return Err(Error::new(
            field.span(),
            "Field cannot be both #[capsule_serialize(skip)] and #[capsule_serialize(skip_if = \"...\")]",
        ));
    }

    // Validation: prev_hash must be u64 type
    if prev_hash {
        if let Type::Path(type_path) = &field.ty {
            let segment = type_path.path.segments.last();
            if segment.map(|s| s.ident != "u64").unwrap_or(true) {
                return Err(Error::new(
                    field.span(),
                    "Field marked #[capsule_serialize(prev_hash)] must be of type u64",
                ));
            }
        } else {
            return Err(Error::new(
                field.span(),
                "Field marked #[capsule_serialize(prev_hash)] must be of type u64",
            ));
        }
    }

    Ok((skip, hash_key, prev_hash, skip_if))
}

/// Parse struct-level attributes (#[capsule_serialize(...)])
///
/// Supports:
/// - `#[capsule_serialize(auto_crc = true)]`: Auto-generate CRC32 methods
///
/// # ASSUM Framework
/// - `#ASSUME_ATTR_PARSE`: syn parses attributes correctly
/// - `#VERIFY_ATTR_PARSE`: Match arms handle all valid cases
pub fn parse_capsule_config(input: &DeriveInput) -> syn::Result<CapsuleConfig> {
    let mut config = CapsuleConfig::default();

    for attr in &input.attrs {
        if attr.path().is_ident("capsule_serialize") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("auto_crc") {
                    // Parse auto_crc = true/false
                    let value = meta.value()?;
                    let lit: syn::LitBool = value.parse()?;
                    config.auto_crc = lit.value();
                    Ok(())
                } else {
                    Err(meta.error(format!(
                        "Unknown struct-level capsule_serialize attribute: {:?}\n\
                         Valid options: auto_crc",
                        meta.path.get_ident()
                    )))
                }
            })?;
        }
    }

    Ok(config)
}

/// Parse all struct fields and detect fixed-point types
///
/// # ASSUM Framework
/// - `#ASSUME_STRUCT_FIELDS`: DeriveInput is a struct with named fields
/// - `#VERIFY_STRUCT_FIELDS`: Error returned if not a struct
///
/// # Returns
/// - Ok(Vec<CapsuleField>) if all fields are valid
/// - Err with helpful error message if invalid field found
pub fn parse_capsule_fields(input: &DeriveInput) -> syn::Result<Vec<CapsuleField>> {
    // Extract named fields from struct
    let fields = match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => &fields_named.named,
            Fields::Unnamed(_) => {
                return Err(Error::new(
                    input.span(),
                    "CapsuleSerialize requires named fields (not tuple struct)",
                ));
            }
            Fields::Unit => {
                return Err(Error::new(
                    input.span(),
                    "CapsuleSerialize requires struct with fields (not unit struct)",
                ));
            }
        },
        _ => unreachable!("validate_capsule_struct already checked this"),
    };

    let mut capsule_fields = Vec::new();

    for field in fields {
        let name = field
            .ident
            .as_ref()
            .ok_or_else(|| Error::new(field.span(), "Field must have a name"))?
            .clone();

        let ty = field.ty.clone();

        // Parse field attributes
        let (skip, hash_key, prev_hash, skip_if) = parse_field_attributes(field)?;

        // Detect fixed-point type
        let fp_type = detect_fixed_point_type(&ty);

        // Validate: Non-skipped fields must be fixed-point types (unless prev_hash or skip_if)
        if !skip && !hash_key && !prev_hash && skip_if.is_none() && fp_type.is_none() {
            let type_name = type_name_for_error(&ty);
            return Err(Error::new(
                field.span(),
                format!(
                    "Field '{}' has unsupported type '{}'\n\
                     \n\
                     Fixed-point serialization requires one of:\n\
                     - Q8_8 (1/256 precision)\n\
                     - Q16_16 (1/65536 precision)\n\
                     - Q32_32 (highest precision)\n\
                     \n\
                     Options:\n\
                     1. Change type to Q8_8, Q16_16, or Q32_32\n\
                     2. Mark field with #[capsule_serialize(skip)] to exclude from serialization\n\
                     3. Mark field with #[capsule_serialize(hash_key)] to include in hash only\n\
                     4. Mark field with #[capsule_serialize(skip_if = \"...\")] for conditional skip",
                    name, type_name
                ),
            ));
        }

        capsule_fields.push(CapsuleField {
            name,
            ty,
            fp_type,
            skip,
            hash_key,
            prev_hash,
            skip_if,
        });
    }

    // Validation: At least one serializable field
    let serializable_count = capsule_fields
        .iter()
        .filter(|f| !f.skip && !f.hash_key)
        .count();

    if serializable_count == 0 {
        return Err(Error::new(
            input.span(),
            "CapsuleSerialize requires at least one serializable field\n\
             All fields are marked #[capsule_serialize(skip)] or #[capsule_serialize(hash_key)]",
        ));
    }

    Ok(capsule_fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_parse_valid_fields() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(128))]
            struct MyCapsule {
                amount: Q16_16,
                fee: Q16_16,
            }
        };
        let fields = parse_capsule_fields(&input).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "amount");
        assert_eq!(fields[0].fp_type, Some(FixedPointType::Q16_16));
        assert!(!fields[0].skip);
    }

    #[test]
    fn test_parse_field_with_skip() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(128))]
            struct MyCapsule {
                amount: Q16_16,
                #[capsule_serialize(skip)]
                internal_id: u64,
            }
        };
        let fields = parse_capsule_fields(&input).unwrap();
        assert_eq!(fields.len(), 2);
        assert!(!fields[0].skip);
        assert!(fields[1].skip);
    }

    #[test]
    fn test_parse_field_with_hash_key() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(128))]
            struct MyCapsule {
                amount: Q16_16,
                #[capsule_serialize(hash_key)]
                audit_key: u64,
            }
        };
        let fields = parse_capsule_fields(&input).unwrap();
        assert_eq!(fields.len(), 2);
        assert!(!fields[0].hash_key);
        assert!(fields[1].hash_key);
    }

    #[test]
    fn test_parse_invalid_type() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(128))]
            struct MyCapsule {
                price: f64,
            }
        };
        let err = parse_capsule_fields(&input).unwrap_err();
        assert!(err.to_string().contains("unsupported type 'f64'"));
        assert!(err.to_string().contains("Q8_8, Q16_16, or Q32_32"));
    }

    #[test]
    fn test_parse_all_fields_skipped() {
        let input: DeriveInput = parse_quote! {
            #[repr(C, align(128))]
            struct MyCapsule {
                #[capsule_serialize(skip)]
                internal_id: u64,
            }
        };
        let err = parse_capsule_fields(&input).unwrap_err();
        assert!(err.to_string().contains("at least one serializable field"));
    }

    #[test]
    fn test_parse_tuple_struct_rejected() {
        let input: DeriveInput = parse_quote! {
            #[repr(C)]
            struct MyCapsule(Q16_16);
        };
        let err = parse_capsule_fields(&input).unwrap_err();
        assert!(err.to_string().contains("named fields"));
    }
}
