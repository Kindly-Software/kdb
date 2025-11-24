//! # Size Constraint Validation for Computational Capsules
//!
//! **Phase 3 Enhancement**: Enforce tier-specific size constraints at compile-time.
//!
//! ## UCE34 Q10-Q12: Tier-Specific Size Limits
//!
//! Different capsule tiers have different size constraints for optimal performance:
//!
//! - **T1 (Atomic)**: <= 256B (fits in 4× cache lines, allows padding for alignment)
//! - **Hot Path**: <= 128B (fits in 2× cache lines, critical for <100ns operations)
//! - **T2 (SIMD)**: <= 512B (allows vectorized operations)
//! - **General**: No hard limit (but warn on >1KB as potential design smell)
//!
//! ## Why Size Matters (B32 Framework)
//!
//! - **Cache efficiency**: Larger structs → more cache misses → higher latency
//! - **Memory bandwidth**: Atomic loads/stores compete for cache lines
//! - **False sharing**: Adjacent capsules in same cache line → contention
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_TYCTXT_SIZE_ACCURATE`: TyCtxt layout_of() returns correct size_of::<T>()
//! - `#VERIFY_SIZE_ACCURATE`: Compile-fail tests validate size constraint enforcement
//! - `#ASSUME_TIER_DETECTION_ACCURATE`: Tier inference from attributes is correct
//!
//! ## Example
//!
//! ```rust,ignore
//! // Good: T1 capsule within 256B limit
//! #[derive(CapsuleSerialize, ComputationalCapsule)]
//! #[capsule(alignment = 64, size = 128, tier = "Atomic")]
//! #[repr(C, align(64))]
//! struct PaymentCapsule {  // 128B
//!     amount: AtomicU64,
//!     fee: AtomicU64,
//!     _padding: [u8; 112],
//! }
//!
//! // Bad: T1 capsule exceeds 256B limit
//! #[derive(CapsuleSerialize, ComputationalCapsule)]
//! #[capsule(alignment = 64, size = 512, tier = "Atomic")]  // Lint warning!
//! #[repr(C, align(64))]
//! struct OversizedCapsule {  // 512B
//!     data: [AtomicU64; 64],
//! }
//! ```

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use rustc_span::Symbol;

/// Capsule tier for size constraint enforcement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsuleTier {
    /// Tier 1: Atomic coordination (<100ns, <= 256B)
    Atomic,
    /// Hot path operations (<100ns, <= 128B)
    HotPath,
    /// Tier 2: SIMD vectorization (<= 512B)
    Simd,
    /// General purpose (no hard limit, warn >1KB)
    General,
}

impl CapsuleTier {
    /// Get maximum size in bytes for this tier
    ///
    /// # UCE34 Q10 (Tier Selection)
    ///
    /// Size limits based on cache line efficiency and latency targets:
    /// - Atomic (T1): 256B = 4× cache lines (allows padding for 64B/128B alignment)
    /// - HotPath: 128B = 2× cache lines (critical <100ns operations)
    /// - SIMD (T2): 512B = 8× cache lines (vectorized operations)
    /// - General: 1024B = 16× cache lines (warning threshold, not hard limit)
    pub fn max_size_bytes(self) -> u64 {
        match self {
            CapsuleTier::Atomic => 256,
            CapsuleTier::HotPath => 128,
            CapsuleTier::Simd => 512,
            CapsuleTier::General => 1024, // Warning threshold
        }
    }

    /// Get tier from attribute value
    ///
    /// # Example Attributes
    ///
    /// ```rust,ignore
    /// #[capsule(tier = "Atomic")]
    /// #[capsule(tier = "HotPath")]
    /// #[capsule(tier = "SIMD")]
    /// ```
    pub fn from_attribute(tier_str: &str) -> Option<Self> {
        match tier_str {
            "Atomic" | "T1" => Some(CapsuleTier::Atomic),
            "HotPath" | "Hot" => Some(CapsuleTier::HotPath),
            "SIMD" | "T2" => Some(CapsuleTier::Simd),
            "General" | "Default" => Some(CapsuleTier::General),
            _ => None,
        }
    }
}

/// Size constraint violation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeConstraintViolation {
    /// Struct exceeds tier-specific size limit
    ExceedsLimit {
        tier: CapsuleTier,
        actual_size: u64,
        max_size: u64,
    },
    /// Could not determine struct size (layout error)
    LayoutError,
}

/// Validate size constraints for a capsule struct
///
/// # Arguments
///
/// - `tcx`: Type context for layout calculations
/// - `def_id`: Definition ID of the struct
/// - `tier`: Capsule tier (from #[capsule(tier = "...")] attribute)
///
/// # Returns
///
/// - `Ok(actual_size)`: Size is within constraints
/// - `Err(SizeConstraintViolation)`: Size exceeds tier limit
///
/// # ASSUM Framework
///
/// - `#ASSUME_LAYOUT_OF_VALID`: TyCtxt::layout_of() returns correct size
/// - `#VERIFY_LAYOUT_OF`: Compile-fail tests check oversized capsules are caught
///
/// # Performance (B32 Framework)
///
/// - TyCtxt::layout_of() is O(1) cached lookup (~50μs)
/// - Total overhead: <100μs per struct (acceptable for compile-time lint)
pub fn validate_size_constraints<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    tier: CapsuleTier,
) -> Result<u64, SizeConstraintViolation> {
    // Get type from DefId
    // #ASSUME_TYPE_OF_VALID: def_id refers to valid struct definition
    let ty = tcx.type_of(def_id).instantiate_identity();

    // Get layout (includes size calculation)
    // #ASSUME_LAYOUT_OF_ACCURATE: TyCtxt computes correct layout

    // Create typing environment for layout computation
    let typing_env = rustc_middle::ty::TypingEnv::post_analysis(tcx, def_id);
    let layout = tcx
        .layout_of(typing_env.as_query_input(ty))
        .map_err(|_| SizeConstraintViolation::LayoutError)?;

    let actual_size = layout.size.bytes();
    let max_size = tier.max_size_bytes();

    if actual_size > max_size {
        return Err(SizeConstraintViolation::ExceedsLimit {
            tier,
            actual_size,
            max_size,
        });
    }

    Ok(actual_size)
}

/// Extract tier from #[capsule(tier = "...")] attribute
///
/// # Heuristics
///
/// If tier not explicitly specified, infer from:
/// 1. Alignment: 64B/128B → likely Atomic (T1)
/// 2. Derives: CapsuleSerialize → likely Atomic (audit trails)
/// 3. Default: General (no strict limit)
///
/// Uses string matching as a workaround for rustc_hir attribute API limitations
pub fn infer_tier_from_attributes(attrs: &[rustc_hir::Attribute]) -> CapsuleTier {
    use rustc_span::sym;

    // Check for explicit #[capsule(tier = "...")] attribute
    for attr in attrs {
        if attr.has_name(Symbol::intern("capsule")) {
            let attr_str = format!("{:?}", attr);

            // Look for tier = "TierName" pattern
            if let Some(tier_start) = attr_str.find("tier") {
                let rest = &attr_str[tier_start..];
                // Try to extract tier name from common patterns
                for tier_name in &["Atomic", "T1", "HotPath", "Hot", "SIMD", "T2", "General", "Default"] {
                    if rest.contains(tier_name) {
                        if let Some(tier) = CapsuleTier::from_attribute(tier_name) {
                            return tier;
                        }
                    }
                }
            }
        }
    }

    // Heuristic: Check alignment (64B/128B → likely Atomic)
    for attr in attrs {
        if !attr.has_name(sym::repr) {
            continue;
        }

        let attr_str = format!("{:?}", attr);

        // Look for align(64) or align(128) pattern
        if let Some(align_start) = attr_str.find("align") {
            let rest = &attr_str[align_start..];
            if let Some(open_paren) = rest.find('(') {
                if let Some(close_paren) = rest.find(')') {
                    let num_str = &rest[open_paren + 1..close_paren];
                    if let Ok(align) = num_str.trim().parse::<u64>() {
                        if align == 64 || align == 128 {
                            return CapsuleTier::Atomic;
                        }
                    }
                }
            }
        }
    }

    // Default: General tier (no strict limit)
    CapsuleTier::General
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_max_sizes() {
        assert_eq!(CapsuleTier::Atomic.max_size_bytes(), 256);
        assert_eq!(CapsuleTier::HotPath.max_size_bytes(), 128);
        assert_eq!(CapsuleTier::Simd.max_size_bytes(), 512);
        assert_eq!(CapsuleTier::General.max_size_bytes(), 1024);
    }

    #[test]
    fn test_tier_from_attribute() {
        assert_eq!(CapsuleTier::from_attribute("Atomic"), Some(CapsuleTier::Atomic));
        assert_eq!(CapsuleTier::from_attribute("T1"), Some(CapsuleTier::Atomic));
        assert_eq!(CapsuleTier::from_attribute("HotPath"), Some(CapsuleTier::HotPath));
        assert_eq!(CapsuleTier::from_attribute("SIMD"), Some(CapsuleTier::Simd));
        assert_eq!(CapsuleTier::from_attribute("Invalid"), None);
    }
}
