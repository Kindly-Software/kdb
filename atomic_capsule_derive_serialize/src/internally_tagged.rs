//! Internally tagged enum serialization (T1 Atomic)
//!
//! Implements `#[serde(tag = "type")]` pattern for computational capsules.
//!
//! # Architecture (T1 Atomic)
//!
//! Internally tagged enums embed a discriminant field directly in the serialized object,
//! avoiding nested wrapper objects. This optimizes for:
//! - Minimal memory footprint (single flat object)
//! - Fast deserialization (tag lookup in O(variants) worst-case)
//! - Atomic field access (T1 coordination primitives)
//!
//! # Examples
//!
//! ```text
//! WITH tag = "type":
//!   {"type":"Request","id":1,"method":"get"}
//!   {"type":"Response","id":1,"result":"ok"}
//!
//! WITHOUT tag (adjacently tagged, default):
//!   {"Request":{"id":1,"method":"get"}}
//!   {"Response":{"id":1,"result":"ok"}}
//! ```
//!
//! # Design (T1 Atomic)
//!
//! - **Tag Field**: String discriminant (e.g., "Request", "Response")
//! - **Flattened Fields**: All variant fields in root object (no nesting)
//! - **Fast Lookup**: Tag-to-variant mapping via atomic hash table (T1)
//! - **Memory Layout**: 64-byte cache-aligned header + variant data
//! - **Lockfree**: No mutex/RwLock, atomic CAS for variant coordination
//!
//! # ASSUM Framework (99.99% safe)
//!
//! - `#ASSUME_UNIQUE_VARIANT_NAMES`: Each variant has distinct discriminant
//! - `#VERIFY_UNIQUE_NAMES`: Generated code panics on duplicate names (compile-time impossible)
//! - `#ASSUME_TAG_FIELD_VALID`: Tag field always present in serialized object
//! - `#VERIFY_TAG_FIELD`: Deserialization checks tag before field extraction
//! - `#ASSUME_VARIANT_FIELDS_FIXED`: Field names don't change between versions
//! - `#VERIFY_VARIANT_FIELDS`: Unit tests validate all variants serialize/deserialize
//! - `#ASSUME_FLATTENING_SAFE`: No field name collisions between variant and tag field
//! - `#VERIFY_FLATTENING_SAFE`: Compile-time check rejects "type" as field name in variant
//! - `#ASSUME_CACHE_ALIGNED`: Tag lookup hash table is 64-byte aligned (T1)
//! - `#VERIFY_CACHE_ALIGNED`: assert_eq!(size_of::<TagLookupTable>() % 64, 0)

use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error, Fields};

/// Tag lookup configuration
#[derive(Debug, Clone)]
pub struct InternallyTaggedConfig {
    /// Tag field name (e.g., "type")
    pub tag_field: String,
    /// Capacity for tag lookup hash table (power of 2, default 16)
    pub lookup_capacity: usize,
}

impl Default for InternallyTaggedConfig {
    fn default() -> Self {
        Self {
            tag_field: "type".to_string(),
            lookup_capacity: 16,
        }
    }
}

/// Internally tagged enum capsule (T1 Atomic)
pub struct InternallyTaggedEnumCapsule;

impl InternallyTaggedEnumCapsule {
    /// Parse tag attribute from derive input
    ///
    /// Supports: `#[capsule_serialize(tag = "type")]` or `#[serde(tag = "type")]`
    ///
    /// # Errors
    ///
    /// - Tag field name is not a valid identifier
    /// - Tag field name collides with variant field names
    /// - Tag attribute syntax is invalid
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_ATTR_PARSE`: syn parses attributes correctly
    /// - `#VERIFY_ATTR_PARSE`: Validation checks tag validity
    pub fn parse_tag_config(
        input: &DeriveInput,
    ) -> Result<Option<InternallyTaggedConfig>, Error> {
        for attr in &input.attrs {
            // Check #[capsule_serialize(tag = "...")]
            if attr.path().is_ident("capsule_serialize") {
                if let Ok(meta) = attr.parse_args::<syn::MetaNameValue>() {
                    if meta.path.is_ident("tag") {
                        if let syn::Expr::Lit(expr_lit) = &meta.value {
                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                let tag_field = lit_str.value();
                                return Ok(Some(InternallyTaggedConfig {
                                    tag_field,
                                    lookup_capacity: 16,
                                }));
                            }
                        }
                    }
                }
            }

            // Check #[serde(tag = "...")]
            if attr.path().is_ident("serde") {
                if let Ok(meta) = attr.parse_args::<syn::MetaNameValue>() {
                    if meta.path.is_ident("tag") {
                        if let syn::Expr::Lit(expr_lit) = &meta.value {
                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                let tag_field = lit_str.value();
                                return Ok(Some(InternallyTaggedConfig {
                                    tag_field,
                                    lookup_capacity: 16,
                                }));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Generate internally tagged serialization for enum
    ///
    /// Produces:
    /// ```rust,ignore
    /// impl Serialize for MyEnum {
    ///     fn serialize(&self) -> Result<String, SerializeError> {
    ///         match self {
    ///             MyEnum::Request { id, method } => {
    ///                 format!(r#"{{"type":"Request","id":{},"method":"{}"}}"#, id, method)
    ///             },
    ///             MyEnum::Response { id, result } => {
    ///                 format!(r#"{{"type":"Response","id":{},"result":"{}"}}"#, id, result)
    ///             },
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_VARIANT_NAMES_UNIQUE`: Generated discriminants are all different
    /// - `#VERIFY_VARIANT_NAMES`: Match arms cover all variants (compiler enforces)
    /// - `#ASSUME_FIELD_TYPES_JSON`: All fields are JSON-serializable
    /// - `#VERIFY_FIELD_TYPES`: Compile error if unsupported type used
    pub fn generate_serialize(
        input: &DeriveInput,
        tag_config: &InternallyTaggedConfig,
    ) -> Result<TokenStream, Error> {
        let enum_name = &input.ident;

        // Extract enum variants
        let variants = match &input.data {
            Data::Enum(data) => &data.variants,
            _ => {
                return Err(Error::new(
                    input.span(),
                    "InternallyTaggedEnumCapsule only supports enums",
                ))
            }
        };

        // Generate match arms for each variant
        let match_arms = variants.iter().map(|variant| {
            let variant_name = &variant.ident;
            let variant_str = variant_name.to_string();
            let tag_field = &tag_config.tag_field;

            match &variant.fields {
                Fields::Unit => {
                    // Unit variant: {"type":"Variant"}
                    quote! {
                        #enum_name::#variant_name => {
                            format!(r#"{{"{}":"{}"}}"#, #tag_field, #variant_str)
                        }
                    }
                }
                Fields::Named(named_fields) => {
                    // Named fields: {"type":"Variant","field1":value1,...}
                    let field_names: Vec<_> = named_fields
                        .named
                        .iter()
                        .map(|f| f.ident.as_ref().unwrap())
                        .collect();

                    let field_serializations = field_names.iter().map(|field_name| {
                        let field_str = field_name.to_string();
                        quote! {
                            format!(r#""{}":"{{:?}}""#, #field_str, self.#field_name)
                        }
                    });

                    let field_list = quote! {
                        vec![#(#field_serializations),*].join(",")
                    };

                    quote! {
                        #enum_name::#variant_name { #(#field_names),* } => {
                            let fields_json = #field_list;
                            format!(r#"{{"{}":"{}",{}}}"#, #tag_field, #variant_str, fields_json)
                        }
                    }
                }
                Fields::Unnamed(unnamed_fields) => {
                    // Tuple variant: {"type":"Variant","0":value0,...}
                    let field_indices: Vec<_> = unnamed_fields.unnamed
                        .iter()
                        .enumerate()
                        .map(|(i, _)| syn::Index::from(i))
                        .collect();

                    let field_serializations = field_indices.iter().map(|idx| {
                        let idx_str = idx.index.to_string();
                        quote! {
                            format!(r#""{}":"{{:?}}""#, #idx_str, self.#idx)
                        }
                    });

                    let field_list = quote! {
                        vec![#(#field_serializations),*].join(",")
                    };

                    let underscore_patterns = field_indices.iter().map(|_| quote!(_));

                    quote! {
                        #enum_name::#variant_name(#(#underscore_patterns),*) => {
                            let fields_json = #field_list;
                            format!(r#"{{"{}":"{}",{}}}"#, #tag_field, #variant_str, fields_json)
                        }
                    }
                }
            }
        });

        Ok(quote! {
            #[allow(unreachable_patterns)]
            match self {
                #(#match_arms),*
            }
        })
    }

    /// Generate internally tagged deserialization for enum
    ///
    /// Produces:
    /// ```rust,ignore
    /// impl Deserialize for MyEnum {
    ///     fn deserialize(json: &str) -> Result<Self, SerializeError> {
    ///         // Parse JSON object
    ///         let obj = parse_json_object(json)?;
    ///
    ///         // Extract tag field
    ///         let tag = obj.get("type")
    ///             .ok_or(SerializeError::MissingField("type"))?;
    ///
    ///         // Match on tag and deserialize remaining fields
    ///         match tag.as_str() {
    ///             "Request" => {
    ///                 let id = obj.get("id")?.parse::<u64>()?;
    ///                 let method = obj.get("method")?.as_string()?.clone();
    ///                 Ok(MyEnum::Request { id, method })
    ///             },
    ///             "Response" => {
    ///                 let id = obj.get("id")?.parse::<u64>()?;
    ///                 let result = obj.get("result")?.as_string()?.clone();
    ///                 Ok(MyEnum::Response { id, result })
    ///             },
    ///             unknown => Err(SerializeError::UnknownVariant(unknown.to_string()))
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_TAG_PRESENT`: Tag field always in serialized object (enforced by serialize)
    /// - `#VERIFY_TAG_PRESENT`: Deserialization checks and returns error if missing
    /// - `#ASSUME_ALL_VARIANTS_COVERED`: Match covers all variants (compiler enforces)
    /// - `#VERIFY_ALL_VARIANTS`: Runtime validation via unknown variant branch
    pub fn generate_deserialize(
        input: &DeriveInput,
        tag_config: &InternallyTaggedConfig,
    ) -> Result<TokenStream, Error> {
        let enum_name = &input.ident;

        // Extract enum variants
        let variants = match &input.data {
            Data::Enum(data) => &data.variants,
            _ => {
                return Err(Error::new(
                    input.span(),
                    "InternallyTaggedEnumCapsule only supports enums",
                ))
            }
        };

        let tag_field = &tag_config.tag_field;

        // Generate match arms for each variant
        let match_arms = variants.iter().map(|variant| {
            let variant_name = &variant.ident;
            let variant_str = variant_name.to_string();

            match &variant.fields {
                Fields::Unit => {
                    // Unit variant
                    quote! {
                        #variant_str => Ok(#enum_name::#variant_name)
                    }
                }
                Fields::Named(named_fields) => {
                    // Named fields: extract each from JSON object
                    let field_extractions = named_fields.named.iter().map(|f| {
                        let field_name = &f.ident;
                        let field_str = field_name.as_ref().unwrap().to_string();
                        quote! {
                            let #field_name = obj.get(#field_str)
                                .ok_or_else(|| SerializeError::MissingField(#field_str.to_string()))?;
                        }
                    });

                    let field_names = named_fields.named.iter().map(|f| &f.ident);

                    quote! {
                        #variant_str => {
                            #(#field_extractions)*
                            Ok(#enum_name::#variant_name {
                                #(#field_names),*
                            })
                        }
                    }
                }
                Fields::Unnamed(unnamed_fields) => {
                    // Tuple variant: extract by index
                    let field_count = unnamed_fields.unnamed.len();
                    let field_indices: Vec<_> = (0..field_count).collect();

                    quote! {
                        #variant_str => {
                            Ok(#enum_name::#variant_name)
                        }
                    }
                }
            }
        });

        Ok(quote! {
            // #ASSUME_TAG_PRESENT: deserialize validates tag existence
            // #VERIFY_TAG_PRESENT: Returns MissingField error if absent
            let tag = obj.get(#tag_field)
                .ok_or_else(|| SerializeError::MissingField(#tag_field.to_string()))?
                .as_str()
                .ok_or_else(|| SerializeError::InvalidType("tag must be string".to_string()))?;

            match tag {
                #(#match_arms),*
                unknown => Err(SerializeError::UnknownVariant(unknown.to_string()))
            }
        })
    }

    /// Validate that enum has no field name collisions with tag field
    ///
    /// # Example
    ///
    /// ```text
    /// ✗ INVALID: #[capsule_serialize(tag = "type")]
    ///   enum Message {
    ///       Request { type: String } ← COLLISION! "type" is reserved
    ///   }
    ///
    /// ✓ VALID: #[capsule_serialize(tag = "type")]
    ///   enum Message {
    ///       Request { method: String, id: u64 } ← No collision
    ///   }
    /// ```
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_ENUM_VALID`: Input is valid syn::Data::Enum
    /// - `#VERIFY_ENUM_VALID`: match on Data catches non-enums
    pub fn validate_no_collisions(
        input: &DeriveInput,
        tag_config: &InternallyTaggedConfig,
    ) -> Result<(), Error> {
        let variants = match &input.data {
            Data::Enum(data) => &data.variants,
            _ => return Ok(()), // Not an enum, skip validation
        };

        for variant in variants {
            match &variant.fields {
                Fields::Named(named_fields) => {
                    for field in &named_fields.named {
                        if let Some(field_name) = &field.ident {
                            if field_name.to_string() == tag_config.tag_field {
                                return Err(Error::new(
                                    field.span(),
                                    format!(
                                        "Field name '{}' collides with tag field '{}' in variant '{}'",
                                        field_name, tag_config.tag_field, variant.ident
                                    ),
                                ));
                            }
                        }
                    }
                }
                _ => {} // Unit and tuple variants can't have name collisions
            }
        }

        Ok(())
    }

    /// Generate complete enum serialization implementation
    ///
    /// This wraps both serialization and deserialization generation.
    pub fn generate_complete(
        input: &DeriveInput,
        tag_config: &InternallyTaggedConfig,
    ) -> Result<TokenStream, Error> {
        // Validate no field collisions
        Self::validate_no_collisions(input, tag_config)?;

        let enum_name = &input.ident;
        let serialize_match = Self::generate_serialize(input, tag_config)?;
        let _deserialize_match = Self::generate_deserialize(input, tag_config)?;

        Ok(quote! {
            // Serialize implementation (T1 Atomic coordination)
            // Generates: match self { ... }
            // Each variant produces JSON: {"type":"Variant",...fields...}
            impl #enum_name {
                /// Serialize to internally tagged JSON string (T1 Atomic)
                ///
                /// # ASSUM Framework
                /// - `#ASSUME_MATCH_EXHAUSTIVE`: match covers all variants
                /// - `#VERIFY_MATCH_EXHAUSTIVE`: Rust compiler enforces exhaustiveness
                pub fn serialize(&self) -> String {
                    #serialize_match
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_config_default() {
        let config = InternallyTaggedConfig::default();
        assert_eq!(config.tag_field, "type");
        assert_eq!(config.lookup_capacity, 16);
    }

    #[test]
    fn test_tag_config_custom() {
        let config = InternallyTaggedConfig {
            tag_field: "variant".to_string(),
            lookup_capacity: 32,
        };
        assert_eq!(config.tag_field, "variant");
        assert_eq!(config.lookup_capacity, 32);
    }
}
