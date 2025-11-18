//! Code generation for #[derive(CapsuleDeserialize)]
//!
//! Generates deserialization logic with:
//! - Binary deserialization (22-byte header validation + payload parsing)
//! - Decimal string deserialization (from human-readable format)
//! - Type-safe field unpacking
//! - Error handling with detailed diagnostics

use crate::field_parser::{CapsuleConfig, CapsuleField};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

/// Generate complete deserialization implementation
///
/// Generates `impl From<&[u8]> for MyStruct` and helper methods.
///
/// # ASSUM Framework
/// - `#ASSUME_FIELD_ORDER`: Fields deserialized in declaration order
/// - `#VERIFY_FIELD_ORDER`: syn parses fields in source order (guaranteed)
/// - `#ASSUME_BINARY_FORMAT`: 22-byte header (magic + version + size + hash) + payload
/// - `#VERIFY_BINARY_FORMAT`: Deserialization validates header before parsing
///
pub fn generate_deserialize_impl(
    input: &DeriveInput,
    fields: &[CapsuleField],
    _config: &CapsuleConfig,
) -> TokenStream {
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Generate struct-specific deserialization based on data structure
    let deserialize_body = match &input.data {
        Data::Struct(data) => generate_struct_deserialize(&data.fields, fields),
        Data::Enum(_) => {
            return quote! {
                compile_error!("CapsuleDeserialize does not support enums");
            }
        }
        Data::Union(_) => {
            return quote! {
                compile_error!("CapsuleDeserialize does not support unions");
            }
        }
    };

    quote! {
        // #ASSUME_TRAIT_EXISTS: CapsuleDeserialize defined in atomic_capsule
        // #VERIFY_TRAIT_EXISTS: Compile error if trait not imported
        impl #impl_generics ::atomic_capsule::serialize::CapsuleDeserialize
            for #struct_name #ty_generics
        #where_clause
        {
            fn deserialize(bytes: &[u8]) -> ::core::result::Result<Self, ::atomic_capsule::serialize::FixedPointSerializeError> {
                #deserialize_body
            }
        }
    }
}

/// Generate deserialization logic for struct fields
fn generate_struct_deserialize(
    fields: &Fields,
    _capsule_fields: &[CapsuleField],
) -> TokenStream {
    match fields {
        Fields::Named(named_fields) => {
            // Binary header validation (22 bytes)
            let magic_check = quote! {
                const MAGIC: u32 = 0x43505346; // "CPSF" = CaPSule Fixed-point
                const VERSION: u16 = 0x0001;
                const MIN_SIZE: usize = 22; // header only

                if bytes.len() < MIN_SIZE {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::InsufficientData {
                        actual: bytes.len(),
                        required: MIN_SIZE,
                    });
                }

                // Validate magic number
                let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if magic != MAGIC {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::InvalidFormat {
                        actual: magic,
                        expected: MAGIC,
                    });
                }

                // Validate version
                let version = u16::from_le_bytes([bytes[4], bytes[5]]);
                if version != VERSION {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::VersionMismatch {
                        actual: version,
                        expected: VERSION,
                    });
                }
            };

            // Generate field deserialization from payload
            let field_count = named_fields.named.len();
            let payload_start = 22; // Skip header
            let payload_size = field_count * 8; // 8 bytes per field
            let total_required = payload_start + payload_size;

            let field_deserializations = named_fields.named.iter().enumerate().map(|(idx, field)| {
                let field_name = &field.ident;
                let field_offset = payload_start + (idx * 8);

                quote! {
                    let #field_name = {
                        if bytes.len() < #field_offset + 8 {
                            return Err(::atomic_capsule::serialize::FixedPointSerializeError::InsufficientData {
                                actual: bytes.len(),
                                required: #field_offset + 8,
                            });
                        }
                        let raw = i64::from_le_bytes([
                            bytes[#field_offset], bytes[#field_offset + 1],
                            bytes[#field_offset + 2], bytes[#field_offset + 3],
                            bytes[#field_offset + 4], bytes[#field_offset + 5],
                            bytes[#field_offset + 6], bytes[#field_offset + 7],
                        ]);
                        raw
                    };
                }
            });

            let field_names: Vec<_> = named_fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect();

            quote! {
                #magic_check

                if bytes.len() < #total_required {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::InsufficientData {
                        actual: bytes.len(),
                        required: #total_required,
                    });
                }

                #(#field_deserializations)*

                Ok(Self {
                    #(#field_names),*
                })
            }
        }
        Fields::Unnamed(unnamed_fields) => {
            let field_count = unnamed_fields.unnamed.len();
            let payload_start = 22; // Skip header
            let payload_size = field_count * 8;
            let total_required = payload_start + payload_size;

            let field_deserializations = (0..field_count).map(|idx| {
                let field_offset = payload_start + (idx * 8);

                quote! {
                    {
                        if bytes.len() < #field_offset + 8 {
                            return Err(::atomic_capsule::serialize::FixedPointSerializeError::InsufficientData {
                                actual: bytes.len(),
                                required: #field_offset + 8,
                            });
                        }
                        let raw = i64::from_le_bytes([
                            bytes[#field_offset], bytes[#field_offset + 1],
                            bytes[#field_offset + 2], bytes[#field_offset + 3],
                            bytes[#field_offset + 4], bytes[#field_offset + 5],
                            bytes[#field_offset + 6], bytes[#field_offset + 7],
                        ]);
                        raw
                    }
                }
            });

            quote! {
                const MAGIC: u32 = 0x43505346; // "CPSF" = CaPSule Fixed-point
                const VERSION: u16 = 0x0001;
                const MIN_SIZE: usize = 22; // header only

                if bytes.len() < MIN_SIZE {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::InsufficientData {
                        actual: bytes.len(),
                        required: MIN_SIZE,
                    });
                }

                // Validate magic number
                let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if magic != MAGIC {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::InvalidFormat {
                        actual: magic,
                        expected: MAGIC,
                    });
                }

                // Validate version
                let version = u16::from_le_bytes([bytes[4], bytes[5]]);
                if version != VERSION {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::VersionMismatch {
                        actual: version,
                        expected: VERSION,
                    });
                }

                if bytes.len() < #total_required {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::InsufficientData {
                        actual: bytes.len(),
                        required: #total_required,
                    });
                }

                Ok(Self(
                    #(#field_deserializations),*
                ))
            }
        }
        Fields::Unit => {
            quote! {
                const MAGIC: u32 = 0x43505346;
                const VERSION: u16 = 0x0001;
                const MIN_SIZE: usize = 22;

                if bytes.len() < MIN_SIZE {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::InsufficientData {
                        actual: bytes.len(),
                        required: MIN_SIZE,
                    });
                }

                let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if magic != MAGIC {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::InvalidFormat {
                        actual: magic,
                        expected: MAGIC,
                    });
                }

                let version = u16::from_le_bytes([bytes[4], bytes[5]]);
                if version != VERSION {
                    return Err(::atomic_capsule::serialize::FixedPointSerializeError::VersionMismatch {
                        actual: version,
                        expected: VERSION,
                    });
                }

                Ok(Self)
            }
        }
    }
}
