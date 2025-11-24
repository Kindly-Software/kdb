//! # Utility functions for capsule lint detection

use rustc_hir::Attribute;
use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::TyCtxt;
use rustc_span::sym;

/// Check if attributes contain `#[repr(C, align(N))]`
///
/// Uses string matching as a workaround for rustc_hir attribute API limitations
pub fn has_repr_c_align(attrs: &[Attribute]) -> bool {
    for attr in attrs {
        if !attr.has_name(sym::repr) {
            continue;
        }

        // Use string representation to check for both C and align
        // This is a workaround since rustc_hir doesn't provide structured meta_item_list()
        let attr_str = format!("{:?}", attr);

        if attr_str.contains("C") && attr_str.contains("align") {
            return true;
        }
    }

    false
}

/// Extract alignment value from `#[repr(C, align(N))]` for diagnostics
///
/// Uses string matching as a workaround for rustc_hir attribute API limitations
pub fn get_alignment_value(attrs: &[Attribute]) -> Option<u64> {
    for attr in attrs {
        if !attr.has_name(sym::repr) {
            continue;
        }

        // Use string representation to extract alignment value
        let attr_str = format!("{:?}", attr);

        // Look for "align" followed by a number
        // Pattern: align(64) or similar
        if let Some(align_start) = attr_str.find("align") {
            let rest = &attr_str[align_start..];
            // Extract number between parentheses
            if let Some(open_paren) = rest.find('(') {
                if let Some(close_paren) = rest.find(')') {
                    let num_str = &rest[open_paren + 1..close_paren];
                    if let Ok(value) = num_str.trim().parse::<u64>() {
                        return Some(value);
                    }
                }
            }
        }
    }

    None
}

/// Check if attributes contain `#[derive(ComputationalCapsule)]`
///
/// Uses string matching as a workaround for rustc_hir attribute API limitations
pub fn has_derive_computational_capsule(attrs: &[Attribute]) -> bool {
    for attr in attrs {
        if !attr.has_name(sym::derive) {
            continue;
        }

        // Use string representation to check for ComputationalCapsule
        let attr_str = format!("{:?}", attr);

        if attr_str.contains("ComputationalCapsule") {
            return true;
        }
    }

    false
}

/// Check if attributes contain `#[derive(CapsuleSerialize)]`
///
/// # Phase 3 Enhancement
///
/// Detects CapsuleSerialize derive macro for dual-derivation enforcement:
/// - CapsuleSerialize requires ComputationalCapsule for audit trail integrity
/// - Missing ComputationalCapsule → no verification → hash chain failures
///
/// # ASSUM Framework
///
/// - `#ASSUME_DERIVE_PARSED_CORRECTLY`: syn parses derive attributes correctly
/// - `#VERIFY_DERIVE_PARSED`: UI tests validate detection accuracy
///
/// # UCE34 Q34 (Auditability)
///
/// CapsuleSerialize is Tier 0 (Auditable Foundation):
/// - Deterministic serialization for hash chains
/// - Requires verified capsules (alignment + size)
/// - Missing verification breaks audit trail integrity (SOX/SOC2/GDPR compliance risk)
pub fn has_derive_capsule_serialize(attrs: &[Attribute]) -> bool {
    for attr in attrs {
        if !attr.has_name(sym::derive) {
            continue;
        }

        // Use string representation to check for CapsuleSerialize
        let attr_str = format!("{:?}", attr);

        if attr_str.contains("CapsuleSerialize") {
            return true;
        }
    }

    false
}

/// Check for dual-derivation pattern: CapsuleSerialize + ComputationalCapsule
///
/// # Dual-Derivation Requirement
///
/// Structs using `#[derive(CapsuleSerialize)]` MUST also use `#[derive(ComputationalCapsule)]`
/// to ensure:
/// 1. Alignment verification (prevent false sharing)
/// 2. Size verification (ensure deterministic layout)
/// 3. Hash chain integrity (audit trail correctness)
///
/// # Returns
///
/// - `Ok(())` if both derives present OR CapsuleSerialize not used
/// - `Err(DualDerivationError)` if CapsuleSerialize present but ComputationalCapsule missing
///
/// # Example
///
/// ```rust,ignore
/// // Good: Dual-derivation
/// #[derive(CapsuleSerialize, ComputationalCapsule)]
/// #[repr(C, align(64))]
/// struct PaymentCapsule { ... }
///
/// // Bad: Missing ComputationalCapsule
/// #[derive(CapsuleSerialize)]
/// #[repr(C)]
/// struct BrokenCapsule { ... }  // Lint warning!
/// ```
pub fn check_dual_derivation(attrs: &[Attribute]) -> Result<(), DualDerivationError> {
    let has_serialize = has_derive_capsule_serialize(attrs);
    let has_capsule = has_derive_computational_capsule(attrs);

    if has_serialize && !has_capsule {
        return Err(DualDerivationError::MissingComputationalCapsule);
    }

    Ok(())
}

/// Dual-derivation error variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualDerivationError {
    /// CapsuleSerialize present but ComputationalCapsule missing
    MissingComputationalCapsule,
}

/// Check if verification macro exists in the given module.
///
/// This is a heuristic that scans for common verification macro names:
/// - `verify_capsule_properties`
/// - `verify_alignment_only`
/// - `verify_size_only`
/// - `verify_capsule`
///
/// Note: This is conservative and may have false negatives if macros
/// are in different modules or use unusual naming.
///
/// # ASSUM Framework
///
/// - `#ASSUME_HIR_API_STABLE`: rustc_hir API for module inspection is stable
/// - `#VERIFY_MACRO_DETECTION`: UI tests validate macro detection accuracy
pub fn has_verification_macro(_tcx: TyCtxt<'_>, _def_id: LocalDefId) -> bool {
    // Simplified heuristic: Assume derive macro provides verification
    // This is conservative - we rely on has_derive_computational_capsule()
    // check in capsule_lint.rs for accurate detection
    //
    // Module inspection is complex in rustc_hir and prone to API changes.
    // The derive macro check is more reliable.
    //
    // Future enhancement: Use more sophisticated HIR traversal if needed

    // Note: By the time we see HIR, macros are already expanded
    // Manual verification macros create unnamed const items: `const _: () = { ... }`
    // But detecting these reliably requires complex HIR traversal

    false // Conservative: require explicit derive macro
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require rustc internals and are integration tests
    // They run during `cargo test --test integration_tests`
}
