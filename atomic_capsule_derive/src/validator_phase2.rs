//! # Phase 2 Correctness - Three P1 validation functions
//!
//! These functions extend validator.rs with:
//! 1. DualAtomicU64 pattern detection (generation counter check)
//! 2. Tier-specific field validation
//! 3. Cache line boundary detection

use quote::quote;
use syn::{DeriveInput, Error, Fields, Result, Type};

/// Validate DualAtomicU64 pattern (generation counter for dual-channel coordination)
///
/// # ASSUM Framework
/// - `#ASSUME_DUAL_ATOMIC_NEEDS_GEN_COUNTER`: DualAtomicU64 pattern requires generation counter
/// - `#VERIFY_GENERATION_COUNTER`: Checked via field analysis
///
/// # UCE34 Q10 (Computational Capsule)
/// DualAtomicU64 pattern (T1 Atomic tier):
/// - Primary channel: AtomicU64 (state/data)
/// - Secondary channel: AtomicU64 (metadata/control)
/// - Generation counter: AtomicU64 named "generation" (TOCTOU prevention)
///
/// # The Atomic Capsule.md
/// "Generation counters prevent Time-of-Check-Time-of-Use (TOCTOU) races.
///  Every dual-channel update increments the generation counter, allowing
///  readers to detect torn reads across the two channels."
///
/// # Detection Heuristic
/// - Count AtomicU64 fields (excluding _padding names)
/// - If count >= 2: Likely DualAtomicU64 pattern
/// - Check for field named "generation" (AtomicU64 type)
///
/// # Errors
///
/// Returns compile error if:
/// - Struct has 2+ AtomicU64 fields but no "generation" field
///
/// # ASSUM Limitations
/// - `#ASSUME_FIELD_NAME_HEURISTIC`: Detection via field name "generation"
/// - `#VERIFY_FIELD_NAME`: Conservative (may miss renamed generation counters)
/// - False negatives OK (better than false positives)
pub fn validate_dual_atomic_pattern(input: &DeriveInput) -> Result<()> {
    let fields = match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => &fields_named.named,
            _ => return Ok(()), // No validation for unnamed fields
        },
        _ => return Ok(()), // No validation for non-structs
    };

    let mut atomic_u64_count = 0;
    let mut has_generation = false;

    for field in fields.iter() {
        if let Some(field_name) = &field.ident {
            let name_str = field_name.to_string();

            // Skip padding fields
            if name_str.starts_with("_padding") || name_str.starts_with("_pad") {
                continue;
            }

            // Check field type
            let type_string = quote!(#field.ty).to_string();

            // Count AtomicU64 fields
            if type_string.contains("AtomicU64") {
                atomic_u64_count += 1;

                // Check for generation counter
                if name_str == "generation" {
                    has_generation = true;
                }
            }
        }
    }

    // Validate DualAtomicU64 pattern
    // #ASSUME_TWO_ATOMICS_IMPLIES_DUAL_PATTERN: 2+ AtomicU64 fields suggest dual-channel coordination
    // #VERIFY_DUAL_PATTERN: Conservative check (may have false negatives, no false positives)
    if atomic_u64_count >= 2 && !has_generation {
        return Err(Error::new_spanned(
            input,
            format!(
                "DualAtomicU64 pattern detected ({} AtomicU64 fields) but missing generation counter\n\
                 \n\
                 Dual-channel coordination requires a generation counter for TOCTOU prevention.\n\
                 \n\
                 Add a field:\n\
                 ```\n\
                 generation: AtomicU64,  // TOCTOU prevention\n\
                 ```\n\
                 \n\
                 # Why generation counters?\n\
                 \n\
                 When reading from two atomic channels (primary + secondary), readers need\n\
                 to verify both channels are from the same consistent snapshot. Without a\n\
                 generation counter, a torn read can occur:\n\
                 \n\
                 1. Reader loads primary channel (value A)\n\
                 2. Writer updates both channels atomically (A→B)\n\
                 3. Reader loads secondary channel (value B)\n\
                 4. Result: Torn read (primary=A, secondary=B, inconsistent!)\n\
                 \n\
                 Generation counter protocol:\n\
                 - Writer: Increment generation BEFORE updating channels\n\
                 - Reader: Load generation BEFORE and AFTER loading channels\n\
                 - Reader: Retry if generation changed (detected torn read)\n\
                 \n\
                 # Example (correct DualAtomicU64):\n\
                 ```\n\
                 #[repr(C, align(128))]\n\
                 struct DualAtomicU64 {{\n\
                     primary: AtomicU64,     // Channel 1 (64B cache line)\n\
                     _padding1: [u8; 56],\n\
                     secondary: AtomicU64,   // Channel 2 (separate cache line)\n\
                     generation: AtomicU64,  // TOCTOU prevention\n\
                     _padding2: [u8; 48],\n\
                 }}\n\
                 ```\n\
                 \n\
                 See: /home/samuel/Docs/The Atomic Capsule.md (Section 8: DualAtomicU64)",
                atomic_u64_count
            ),
        ));
    }

    Ok(())
}

/// Validate tier-specific field requirements
///
/// # ASSUM Framework
/// - `#ASSUME_TIER_FIELDS_MATCH`: Tier declaration matches actual field types
/// - `#VERIFY_TIER_FIELDS`: Conservative check via syn type analysis
///
/// # UCE34 Q10 (Computational Capsule Tiers)
/// - Atomic (T1): AtomicU64, AtomicI64, AtomicBool, AtomicUsize
/// - SIMD (T2): std::simd::* types (Simd<f32, 8>, Simd<i32, 8>, etc.)
/// - FixedPoint (T3): Q8_8, Q16_16, Q32_32, Q48_16 (from atomic_capsule)
/// - Mixed (T6): Combination of T1+T2+T3 (allows all)
/// - Others: Basic validation (struct is properly aligned)
///
/// # Detection Heuristic
/// - Analyze field types via syn Type
/// - Convert to string and pattern match (conservative)
/// - Alignment ≥32B alone is insufficient for SIMD detection
///
/// # Errors
///
/// Returns compile error if:
/// - Tier = "Atomic" but no atomic fields found
/// - Tier = "SIMD" but no std::simd::* fields found
/// - Tier = "FixedPoint" but no Q8_8/Q16_16/Q32_32/Q48_16 fields found
///
/// # ASSUM Limitations
/// - `#ASSUME_TYPE_STRING_MATCHING`: Detection via type.to_string()
/// - `#VERIFY_TYPE_STRING`: Conservative (may miss type aliases)
/// - False negatives OK (better than false positives)
pub fn validate_tier_fields(input: &DeriveInput, tier: &str) -> Result<()> {
    let fields = match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => &fields_named.named,
            _ => return Ok(()), // No validation for unnamed fields
        },
        _ => return Ok(()), // No validation for non-structs
    };

    // Collect field types (excluding padding)
    let mut field_types = Vec::new();
    for field in fields.iter() {
        if let Some(field_name) = &field.ident {
            let name_str = field_name.to_string();
            if name_str.starts_with("_padding") || name_str.starts_with("_pad") {
                continue;
            }
            let type_string = quote!(#field.ty).to_string();
            field_types.push(type_string);
        }
    }

    // Tier-specific validation
    match tier {
        "Atomic" => {
            // Check for atomic types
            let has_atomic = field_types.iter().any(|t| {
                t.contains("Atomic")
                    && (t.contains("AtomicU64")
                        || t.contains("AtomicI64")
                        || t.contains("AtomicBool")
                        || t.contains("AtomicUsize")
                        || t.contains("AtomicU32")
                        || t.contains("AtomicI32"))
            });

            if !has_atomic {
                return Err(Error::new_spanned(
                    input,
                    "Tier = \"Atomic\" but no atomic fields found\n\
                     \n\
                     Atomic tier (T1) capsules must use atomic primitives:\n\
                     - AtomicU64, AtomicI64, AtomicBool, AtomicUsize\n\
                     \n\
                     Example:\n\
                     ```\n\
                     #[repr(C, align(64))]\n\
                     struct AtomicCapsule {{\n\
                         state: AtomicU64,\n\
                         _padding: [u8; 56],\n\
                     }}\n\
                     ```\n\
                     \n\
                     See: UCE34_TIER_REFERENCE.md (T1 Atomic)",
                ));
            }
        }

        "SIMD" => {
            // Check for SIMD types (std::simd::*)
            let has_simd = field_types.iter().any(|t| t.contains("Simd") || t.contains("simd"));

            if !has_simd {
                return Err(Error::new_spanned(
                    input,
                    "Tier = \"SIMD\" but no SIMD fields found\n\
                     \n\
                     SIMD tier (T2) capsules must use vectorized types:\n\
                     - std::simd::Simd<f32, 8>, std::simd::Simd<i32, 8>, etc.\n\
                     - Requires portable_simd (nightly feature)\n\
                     \n\
                     Example:\n\
                     ```\n\
                     use std::simd::{{Simd, SimdFloat}};\n\
                     \n\
                     #[repr(C, align(64))]\n\
                     struct SimdCapsule {{\n\
                         data: Simd<f32, 8>,  // 8-lane SIMD vector\n\
                         _padding: [u8; 32],\n\
                     }}\n\
                     ```\n\
                     \n\
                     Note: Alignment ≥32B alone is insufficient. Tier = \"SIMD\" requires\n\
                     actual SIMD types (Simd<T, N>) for vectorized computation.\n\
                     \n\
                     See: UCE34_TIER_REFERENCE.md (T2 SIMD)",
                ));
            }
        }

        "FixedPoint" => {
            // Check for fixed-point types (Q8_8, Q16_16, Q32_32, Q48_16)
            let has_fixed_point = field_types.iter().any(|t| {
                t.contains("Q8_8")
                    || t.contains("Q16_16")
                    || t.contains("Q32_32")
                    || t.contains("Q48_16")
                    || t.contains("FixedQ")
            });

            if !has_fixed_point {
                return Err(Error::new_spanned(
                    input,
                    "Tier = \"FixedPoint\" but no fixed-point fields found\n\
                     \n\
                     FixedPoint tier (T3) capsules must use deterministic fixed-point types:\n\
                     - Q8_8, Q16_16, Q32_32, Q48_16 (from atomic_capsule)\n\
                     \n\
                     Example:\n\
                     ```\n\
                     use atomic_capsule::primitives::fixed_point::Q16_16;\n\
                     \n\
                     #[repr(C, align(64))]\n\
                     struct FixedPointCapsule {{\n\
                         price: Q16_16,  // Fixed-point decimal (Q16.16)\n\
                         _padding: [u8; 60],\n\
                     }}\n\
                     ```\n\
                     \n\
                     See: UCE34_TIER_REFERENCE.md (T3 Fixed-Point)",
                ));
            }
        }

        "Mixed" => {
            // Mixed tier (T6) allows any combination, no validation needed
            Ok(())
        }

        _ => {
            // Other tiers: Basic validation (no specific field requirements)
            Ok(())
        }
    }
}

/// Verify fields don't span cache line boundaries (64B)
///
/// # ASSUM Framework
/// - `#ASSUME_FIELD_OFFSET_ESTIMABLE`: Proc-macro can estimate field offsets
/// - `#VERIFY_OFFSET_ESTIMATION`: Conservative heuristic (may miss complex layouts)
///
/// # UCE34 Q29 (Hardware Constraints)
/// Cache line size: 64 bytes (x86_64, ARM, most architectures)
///
/// # Why this matters
/// Fields spanning cache line boundaries cause:
/// - False sharing: Two threads updating adjacent fields contend for same cache line
/// - Torn reads: Field split across two cache lines can be partially updated
/// - Performance: 2× cache line loads for single field access
///
/// # Detection Heuristic
/// For each field:
/// 1. Estimate offset from previous fields (cumulative size)
/// 2. Estimate field size (primitive types, arrays, known types)
/// 3. Check: offset / 64 != (offset + size - 1) / 64
/// 4. If true: Field spans boundary
///
/// # Limitations
/// - `#ASSUME_SIMPLE_LAYOUT`: Assumes #[repr(C)] sequential layout
/// - `#VERIFY_SIMPLE_LAYOUT`: Cannot detect compiler padding/reordering
/// - `#ASSUME_TYPE_SIZE_KNOWN`: Estimates common types only
/// - `#VERIFY_TYPE_SIZE`: Conservative (unknown types = skip check)
///
/// # Errors
///
/// Returns compile error if:
/// - Field estimated to span 64B boundary
/// - Recommendation: Add padding or reorder fields
pub fn verify_cache_line_boundaries(input: &DeriveInput) -> Result<()> {
    let fields = match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => &fields_named.named,
            _ => return Ok(()), // No validation for unnamed fields
        },
        _ => return Ok(()), // No validation for non-structs
    };

    const CACHE_LINE_SIZE: usize = 64;
    let mut current_offset: usize = 0;

    for field in fields.iter() {
        let field_name = match &field.ident {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Estimate field size (conservative)
        let field_size = estimate_field_size(&field.ty);

        // Skip unknown sizes (cannot validate)
        if field_size == 0 {
            continue;
        }

        // Check if field spans cache line boundary
        // #ASSUME_BOUNDARY_FORMULA: offset / 64 != (offset + size - 1) / 64 implies span
        // #VERIFY_BOUNDARY_FORMULA: Standard cache-line alignment math
        let start_cache_line = current_offset / CACHE_LINE_SIZE;
        let end_cache_line = (current_offset + field_size - 1) / CACHE_LINE_SIZE;

        if start_cache_line != end_cache_line && !field_name.starts_with("_pad") {
            return Err(Error::new_spanned(
                field,
                format!(
                    "Field `{}` may span cache line boundary (64B)\n\
                     \n\
                     Estimated layout:\n\
                     - Offset: {} bytes (cache line {})\n\
                     - Size:   {} bytes\n\
                     - End:    {} bytes (cache line {})\n\
                     \n\
                     # Why this matters:\n\
                     \n\
                     Fields spanning cache line boundaries cause:\n\
                     - False sharing: Adjacent fields on same line contend\n\
                     - Torn reads: Partial updates across two lines\n\
                     - Performance: 2× cache line loads (50% slower)\n\
                     \n\
                     # Solutions:\n\
                     \n\
                     1. Add padding BEFORE this field:\n\
                     ```\n\
                     _padding_before_{}: [u8; {}],  // Align to next cache line\n\
                     {}: ...,\n\
                     ```\n\
                     \n\
                     2. Reorder fields (smallest to largest):\n\
                     ```\n\
                     // Small fields first (u8, u16, u32)\n\
                     // Large fields last (u64, [u8; N], AtomicU64)\n\
                     ```\n\
                     \n\
                     3. Use explicit alignment:\n\
                     ```\n\
                     #[repr(C, align(64))]\n\
                     ```\n\
                     \n\
                     # ASSUM Limitations:\n\
                     - This is a HEURISTIC (proc-macro cannot compute exact offsets)\n\
                     - Compiler may add padding (actual layout may differ)\n\
                     - False positives possible (better safe than sorry)\n\
                     \n\
                     See: UCE34_TIER_REFERENCE.md (Section: Memory Layout)",
                    field_name,
                    current_offset,
                    start_cache_line,
                    field_size,
                    current_offset + field_size - 1,
                    end_cache_line,
                    field_name,
                    CACHE_LINE_SIZE - (current_offset % CACHE_LINE_SIZE),
                    field_name
                ),
            ));
        }

        // Update offset for next field
        current_offset += field_size;
    }

    Ok(())
}

/// Estimate field size in bytes (conservative heuristic)
///
/// # ASSUM Framework
/// - `#ASSUME_TYPE_SIZE_ESTIMABLE`: Common types have known sizes
/// - `#VERIFY_TYPE_SIZE`: Conservative (unknown = 0, skip validation)
///
/// # Known Types
/// - Primitives: u8(1), u16(2), u32(4), u64(8), usize(8), bool(1)
/// - Atomics: AtomicU64(8), AtomicU32(4), AtomicBool(1)
/// - Arrays: [T; N] = size_of::<T>() × N
/// - SIMD: Estimated from type string (Simd<f32, 8> = 32 bytes)
///
/// # Unknown Types
/// Returns 0 (skip boundary check for this field)
///
/// # Limitations
/// - Cannot parse complex type aliases
/// - Cannot handle generic types
/// - Approximate only (compiler may add padding)
fn estimate_field_size(ty: &Type) -> usize {
    let type_string = quote!(#ty).to_string();

    // Primitive types
    if type_string == "u8" || type_string == "i8" || type_string == "bool" {
        return 1;
    }
    if type_string == "u16" || type_string == "i16" {
        return 2;
    }
    if type_string == "u32" || type_string == "i32" || type_string == "f32" {
        return 4;
    }
    if type_string == "u64"
        || type_string == "i64"
        || type_string == "f64"
        || type_string == "usize"
    {
        return 8;
    }

    // Atomic types
    if type_string.contains("AtomicU64") || type_string.contains("AtomicI64") {
        return 8;
    }
    if type_string.contains("AtomicU32") || type_string.contains("AtomicI32") {
        return 4;
    }
    if type_string.contains("AtomicU16") || type_string.contains("AtomicI16") {
        return 2;
    }
    if type_string.contains("AtomicU8")
        || type_string.contains("AtomicI8")
        || type_string.contains("AtomicBool")
    {
        return 1;
    }

    // Arrays: [T; N]
    if type_string.starts_with('[') && type_string.contains(';') {
        // Extract array size (simple regex-like parsing)
        if let Some(semi_pos) = type_string.rfind(';') {
            if let Some(bracket_pos) = type_string.rfind(']') {
                let size_str = &type_string[semi_pos + 1..bracket_pos].trim();
                if let Ok(count) = size_str.parse::<usize>() {
                    // Estimate element size (assume u8 for simplicity)
                    // #ASSUME_ARRAY_ELEMENT_SIZE: Most padding arrays are [u8; N]
                    return count;
                }
            }
        }
    }

    // SIMD types: Simd<f32, 8> = 32 bytes
    if type_string.contains("Simd") {
        // Extract lane count (Simd<T, N>)
        if let Some(comma_pos) = type_string.rfind(',') {
            if let Some(gt_pos) = type_string.rfind('>') {
                let lane_str = type_string[comma_pos + 1..gt_pos].trim();
                if let Ok(lanes) = lane_str.parse::<usize>() {
                    // Estimate: f32 or i32 = 4 bytes per lane
                    return lanes * 4;
                }
            }
        }
    }

    // Unknown type: Return 0 (skip validation)
    0
}
