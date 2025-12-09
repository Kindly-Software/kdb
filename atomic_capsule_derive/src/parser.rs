//! # Capsule Attribute Parser
//!
//! Extracts and validates #[capsule(...)] attributes from derive input.

use syn::{DeriveInput, Error, Result};

/// Capsule attributes extracted from #[capsule(...)]
///
/// # ASSUM Framework
/// - `#ASSUME_ATTRS_VALID`: Attributes are syntactically valid
/// - `#VERIFY_ATTRS`: syn parsing validates or returns compile error
#[derive(Debug)]
pub struct CapsuleAttributes {
    /// Required: Cache line alignment (32/64/128/256 bytes)
    pub alignment: usize,
    /// Optional: Expected size in bytes
    pub size: Option<usize>,
    /// Optional: Capsule tier ("Atomic", "SIMD", "FixedPoint", etc.)
    pub tier: Option<String>,
    /// Optional: Enable auditable capsule with dual-hash generation (default: false)
    pub auditable: bool,
    /// Optional: Enable verified capsule with formal verification support (default: false)
    /// T0 Verified: TLA+/Spin model checking, Z3 theorem proving, KLEE symbolic execution
    pub verified: bool,
    /// Optional: Fast hash algorithm for development (default: "XxHash64")
    pub fast_hash: Option<String>,
    /// Optional: Crypto hash algorithm for audit trail (default: "Blake3")
    pub crypto_hash: Option<String>,
    /// Optional: Auto-generate padding field (generates compile error with suggested code)
    pub auto_pad: bool,
    /// Optional: Skip automatic Send + Sync trait generation (default: false)
    /// Use for GPU types with raw pointers (*mut T) that aren't thread-safe
    pub skip_send_sync: bool,
}

impl CapsuleAttributes {
    /// Extract capsule attributes from #[capsule(...)]
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ATTRIBUTE_PRESENT`: At least one #[capsule(...)] attribute exists
    /// - `#VERIFY_ATTRIBUTE`: Returns error if missing or invalid
    ///
    /// # Errors
    ///
    /// Returns compile error if:
    /// - Missing #[capsule(...)] attribute
    /// - Missing required `alignment` parameter
    /// - Invalid parameter values (non-integer, out of range)
    /// - Duplicate parameters
    pub fn from_derive_input(input: &DeriveInput) -> Result<Self> {
        // Find #[capsule(...)] attribute
        let capsule_attr = input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("capsule"))
            .ok_or_else(|| {
                Error::new_spanned(
                    input,
                    "Missing #[capsule(...)] attribute\n\
                     \n\
                     The ComputationalCapsule derive macro requires capsule configuration.\n\
                     \n\
                     Add this attribute to your struct:\n\
                     #[capsule(alignment = 64)]\n\
                     \n\
                     Common configurations:\n\
                     - alignment = 64:  Standard single cache line (most common)\n\
                     - alignment = 128: Dual cache line (DualAtomicU64 pattern)\n\
                     - alignment = 256: Multi-line capsules (large state)\n\
                     \n\
                     Optional parameters:\n\
                     - size = N:    Expected struct size in bytes\n\
                     - tier = \"T\":  Capsule tier (Atomic, SIMD, FixedPoint, etc.)\n\
                     - auto_pad = true: Generate padding suggestion\n\
                     \n\
                     Example:\n\
                     #[derive(ComputationalCapsule)]\n\
                     #[capsule(alignment = 64, size = 64)]\n\
                     #[repr(C, align(64))]\n\
                     struct MyCapsule { ... }\n\
                     \n\
                     Help: Add #[capsule(alignment = 64)] before your struct\n\
                     See: /home/samuel/Docs/The Computational Capsule.md\n\
                     See: /home/samuel/Primitives/atomic_capsule/CLAUDE.md (Examples)",
                )
            })?;

        let mut alignment = None;
        let mut size = None;
        let mut tier = None;
        let mut auditable = false;
        let mut verified = false;
        let mut fast_hash = None;
        let mut crypto_hash = None;
        let mut auto_pad = false;
        let mut skip_send_sync = false;

        // Parse nested meta using parse_nested_meta (syn 2.0 API)
        capsule_attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("alignment") {
                if alignment.is_some() {
                    return Err(meta.error("Duplicate `alignment` parameter"));
                }
                let value: syn::LitInt = meta.value()?.parse()?;
                alignment = Some(value.base10_parse::<usize>()?);
                Ok(())
            } else if meta.path.is_ident("size") {
                if size.is_some() {
                    return Err(meta.error("Duplicate `size` parameter"));
                }
                let value: syn::LitInt = meta.value()?.parse()?;
                size = Some(value.base10_parse::<usize>()?);
                Ok(())
            } else if meta.path.is_ident("tier") {
                if tier.is_some() {
                    return Err(meta.error("Duplicate `tier` parameter"));
                }
                let value: syn::LitStr = meta.value()?.parse()?;
                tier = Some(value.value());
                Ok(())
            } else if meta.path.is_ident("auditable") {
                if auditable {
                    return Err(meta.error("Duplicate `auditable` parameter"));
                }
                let value: syn::LitBool = meta.value()?.parse()?;
                auditable = value.value();
                Ok(())
            } else if meta.path.is_ident("verified") {
                if verified {
                    return Err(meta.error("Duplicate `verified` parameter"));
                }
                let value: syn::LitBool = meta.value()?.parse()?;
                verified = value.value();
                Ok(())
            } else if meta.path.is_ident("fast_hash") {
                if fast_hash.is_some() {
                    return Err(meta.error("Duplicate `fast_hash` parameter"));
                }
                let value: syn::LitStr = meta.value()?.parse()?;
                fast_hash = Some(value.value());
                Ok(())
            } else if meta.path.is_ident("crypto_hash") {
                if crypto_hash.is_some() {
                    return Err(meta.error("Duplicate `crypto_hash` parameter"));
                }
                let value: syn::LitStr = meta.value()?.parse()?;
                crypto_hash = Some(value.value());
                Ok(())
            } else if meta.path.is_ident("auto_pad") {
                if auto_pad {
                    return Err(meta.error("Duplicate `auto_pad` parameter"));
                }
                let value: syn::LitBool = meta.value()?.parse()?;
                auto_pad = value.value();
                Ok(())
            } else if meta.path.is_ident("skip_send_sync") {
                if skip_send_sync {
                    return Err(meta.error("Duplicate `skip_send_sync` parameter"));
                }
                let value: syn::LitBool = meta.value()?.parse()?;
                skip_send_sync = value.value();
                Ok(())
            } else {
                Err(meta.error(format!(
                    "Unknown parameter `{}`. Valid: alignment, size, tier, auditable, verified, fast_hash, crypto_hash, auto_pad, skip_send_sync",
                    meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default()
                )))
            }
        })?;

        // alignment is required
        let alignment = alignment.ok_or_else(|| {
            Error::new_spanned(
                capsule_attr,
                "Missing required `alignment` parameter\n\
                 \n\
                 All computational capsules must specify cache line alignment.\n\
                 \n\
                 Valid alignments:\n\
                 - 32 bytes:  Sub-cache-line (tight packing in arrays)\n\
                 - 64 bytes:  Single cache line (prevents false sharing) [MOST COMMON]\n\
                 - 128 bytes: Dual cache line (DualAtomicU64 pattern)\n\
                 - 256 bytes: Multi-line capsules (large complex state)\n\
                 - 512 bytes: Cache slots (maximum false sharing prevention)\n\
                 \n\
                 Add alignment to your #[capsule(...)] attribute:\n\
                 #[capsule(alignment = 64)]\n\
                 \n\
                 Example:\n\
                 #[derive(ComputationalCapsule)]\n\
                 #[capsule(alignment = 64, size = 64)]\n\
                 #[repr(C, align(64))]\n\
                 struct MyCapsule {\n\
                     state: AtomicU64,\n\
                     _padding: [u8; 56],\n\
                 }\n\
                 \n\
                 Help: Use alignment = 64 for most capsules\n\
                 See: /home/samuel/Docs/The Computational Capsule.md (Section: Alignment)",
            )
        })?;

        Ok(CapsuleAttributes {
            alignment,
            size,
            tier,
            auditable,
            verified,
            fast_hash,
            crypto_hash,
            auto_pad,
            skip_send_sync,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_parse_alignment_only() {
        let input: DeriveInput = parse_quote! {
            #[capsule(alignment = 64)]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        let attrs = CapsuleAttributes::from_derive_input(&input).unwrap();
        assert_eq!(attrs.alignment, 64);
        assert_eq!(attrs.size, None);
        assert_eq!(attrs.tier, None);
    }

    #[test]
    fn test_parse_all_attributes() {
        let input: DeriveInput = parse_quote! {
            #[capsule(alignment = 128, size = 512, tier = "Atomic")]
            struct TestCapsule {
                data: [u8; 512],
            }
        };

        let attrs = CapsuleAttributes::from_derive_input(&input).unwrap();
        assert_eq!(attrs.alignment, 128);
        assert_eq!(attrs.size, Some(512));
        assert_eq!(attrs.tier, Some("Atomic".to_string()));
    }

    #[test]
    fn test_missing_capsule_attr() {
        let input: DeriveInput = parse_quote! {
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        let result = CapsuleAttributes::from_derive_input(&input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing #[capsule"));
    }

    #[test]
    fn test_missing_alignment() {
        let input: DeriveInput = parse_quote! {
            #[capsule(size = 64)]
            struct TestCapsule {
                data: [u8; 64],
            }
        };

        let result = CapsuleAttributes::from_derive_input(&input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing required `alignment`"));
    }
}
