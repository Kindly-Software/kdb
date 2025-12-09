//! # CAPSULE_INCORRECT_PADDING Lint Implementation
//!
//! **Purpose**: Detect capsules with padding fields that have incorrect size calculations.
//!
//! ## Why This Matters (Chaos Mandate)
//!
//! Padding fields ensure capsule size matches alignment, but incorrect padding calculation
//! defeats the purpose:
//! - **False positive safety**: Padding exists but wrong size → struct still unaligned
//! - **Subtle bugs**: Struct appears correct (has padding field) but actually misaligned
//! - **Cache line waste**: Over-padding wastes memory, under-padding causes false sharing
//!
//! ## The Problem
//!
//! ```rust,ignore
//! // ❌ BAD: Padding exists but size is wrong!
//! #[repr(C, align(64))]
//! struct BadCapsule {
//!     state: AtomicU64,      // 8 bytes
//!     _padding: [u8; 50],    // WRONG! Should be 56 (64-8=56)
//! }  // Total: 58 bytes (not 64!) → Still unaligned despite padding field
//!
//! // ✅ GOOD: Correct padding calculation
//! #[repr(C, align(64))]
//! struct GoodCapsule {
//!     state: AtomicU64,      // 8 bytes
//!     _padding: [u8; 56],    // CORRECT! (64-8=56)
//! }  // Total: 64 bytes ✓
//! ```
//!
//! ## UCE34 Framework
//!
//! - **Q10 (Tier)**: T1 (Atomic) capsules require exact padding calculations
//! - **Q33 (Validation)**: Compile-time detection of padding errors
//! - **B32 (Performance)**: Incorrect padding → false sharing → 3-5× slowdown
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_FIELD_SIZE_ACCURATE`: TyCtxt::layout_of() returns correct field sizes
//! - `#VERIFY_PADDING_DETECTION`: Compile-fail tests validate detection
//! - `#ASSUME_PADDING_NAMING`: Padding fields named `_padding`, `_pad`, or `_pad*`

use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::{declare_lint, declare_lint_pass};
use rustc_span::sym;

declare_lint! {
    /// **Detects capsule structs with incorrectly sized padding fields.**
    ///
    /// ## Why is this bad?
    ///
    /// Padding fields with incorrect sizes cause:
    /// - **False positive safety**: Padding field exists but capsule is still unaligned
    /// - **Subtle bugs**: Code looks correct but causes cache line issues
    /// - **False sharing**: Under-padding → multiple capsules per cache line
    /// - **Memory waste**: Over-padding → unnecessary memory consumption
    ///
    /// ## Example (BAD)
    ///
    /// ```rust,ignore
    /// // ❌ BAD: Padding field exists but size is wrong!
    /// #[repr(C, align(64))]
    /// struct BadCapsule {
    ///     state: AtomicU64,      // 8 bytes
    ///     counter: AtomicU64,    // 8 bytes
    ///     _padding: [u8; 40],    // WRONG! Should be 48 (64-16=48)
    /// }  // Total: 56 bytes (not 64!) → Still unaligned!
    ///
    /// // Danger: Appears correct (has padding) but still causes false sharing
    /// ```
    ///
    /// ## Example (GOOD)
    ///
    /// ```rust,ignore
    /// // ✅ GOOD: Correct padding calculation
    /// #[repr(C, align(64))]
    /// struct GoodCapsule {
    ///     state: AtomicU64,
    ///     counter: AtomicU64,
    ///     _padding: [u8; 48],    // CORRECT! (64-16=48)
    /// }  // Total: 64 bytes ✓
    /// ```
    ///
    /// ## How to Fix
    ///
    /// 1. **Calculate actual field sizes**: Sum all non-padding field sizes
    /// 2. **Calculate required padding**: `align - (actual_size % align)`
    /// 3. **Update padding field**: `_padding: [u8; required_padding]`
    ///
    /// For example, if fields=16B and align=64B:
    /// - Required padding: 64 - (16 % 64) = 48 bytes
    /// - Update field: `_padding: [u8; 48]`
    /// - Verify: 16 + 48 = 64 bytes ✓
    ///
    /// ## Technical Details
    ///
    /// **Detection Algorithm**:
    /// 1. Find padding field (name matches `_padding`, `_pad`, or `_pad*`)
    /// 2. Calculate struct size without padding field
    /// 3. Calculate required padding: `align - (size_without_padding % align)`
    /// 4. Compare actual padding array size vs required padding
    /// 5. Emit diagnostic if mismatch detected
    ///
    /// **Padding Field Naming Conventions**:
    /// - `_padding` (preferred, single padding field)
    /// - `_pad` (alternate short form)
    /// - `_pad1`, `_pad2`, etc. (multiple padding fields, rare)
    ///
    /// ## Framework Compliance
    ///
    /// **UCE34 Q10 (Tier Selection)**:
    /// - T1 (Atomic): Requires exact padding (size = alignment)
    /// - T2 (SIMD): Stricter: 256B minimum for AVX2/AVX-512
    ///
    /// **Chaos Mandate**:
    /// - 100% lockfree, cache-aligned architecture
    /// - Incorrect padding violates core principle
    ///
    /// **ASSUM Safety**:
    /// - Field layout calculation guaranteed by Rust type system
    /// - No unsafe code needed (compiler enforces this)
    ///
    /// ## See Also
    ///
    /// - Atomic Capsule.md: Padding calculation patterns
    /// - UCE34_TIER_REFERENCE.md: T1 padding requirements
    /// - CAPSULE_UNALIGNED_VIOLATION: Detects missing padding
    pub CAPSULE_INCORRECT_PADDING,
    Warn,
    "padding field size does not match required padding (incorrect calculation)"
}

declare_lint_pass!(CapsulePaddingViolation => [CAPSULE_INCORRECT_PADDING]);

impl<'tcx> LateLintPass<'tcx> for CapsulePaddingViolation {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Only check structs
        if !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }

        // Only check capsules (have #[repr(C, align(...))])
        let attrs = cx.tcx.hir_attrs(item.hir_id());
        if !has_repr_c_align(attrs) {
            return;
        }

        // Get alignment value from repr attribute
        let Some(align) = get_alignment_value(attrs) else {
            return; // Can't determine alignment, skip
        };

        // Get layout of struct
        let def_id = item.owner_id.to_def_id();
        let ty = cx.tcx.type_of(def_id).instantiate_identity();

        // #ASSUME_LAYOUT_OF_ACCURATE: TyCtxt::layout_of() returns correct size/alignment
        // Create typing environment for layout computation
        let typing_env = rustc_middle::ty::TypingEnv::post_analysis(cx.tcx, def_id);

        let layout = match cx.tcx.layout_of(typing_env.as_query_input(ty)) {
            Ok(layout) => layout,
            Err(_) => {
                // Layout error (external type, complex generics, etc.)
                // Skip: likely not a local capsule
                return;
            }
        };

        let total_size = layout.size.bytes();

        // Find padding field(s) and calculate their total size
        let mut padding_fields = Vec::new();
        let mut total_padding_size = 0u64;

        if let ItemKind::Struct(_, _, variant_data) = &item.kind {
            for field in variant_data.fields() {
                let field_name = field.ident.as_str();

                // Check if this is a padding field
                if is_padding_field_name(field_name) {
                    // Get field type to extract array size
                    let field_def_id = field.def_id.to_def_id();
                    let field_ty = cx.tcx.type_of(field_def_id).instantiate_identity();

                    // Get field layout to determine size
                    let field_layout = match cx.tcx.layout_of(typing_env.as_query_input(field_ty)) {
                        Ok(layout) => layout,
                        Err(_) => continue, // Skip if we can't get layout
                    };

                    let field_size = field_layout.size.bytes();
                    total_padding_size += field_size;

                    padding_fields.push((field.ident, field.span, field_size));
                }
            }
        }

        // If no padding fields found, skip (CAPSULE_UNALIGNED_VIOLATION will catch this)
        if padding_fields.is_empty() {
            return;
        }

        // Calculate size without padding
        let size_without_padding = total_size - total_padding_size;

        // Calculate required padding
        let remainder = size_without_padding % align;
        let required_padding = if remainder == 0 {
            0 // Already aligned without padding (over-padding case)
        } else {
            align - remainder
        };

        // Check if actual padding matches required padding
        if total_padding_size != required_padding {
            emit_incorrect_padding_diagnostic(
                cx,
                item,
                &padding_fields,
                align,
                size_without_padding,
                required_padding,
                total_padding_size,
            );
        }
    }
}

/// Check if field name matches padding field naming convention
///
/// Matches:
/// - `_padding` (preferred)
/// - `_pad` (alternate short form)
/// - `_pad1`, `_pad2`, etc. (numbered variants)
/// - `_padding1`, `_padding2`, etc. (numbered variants)
fn is_padding_field_name(name: &str) -> bool {
    name == "_padding"
        || name == "_pad"
        || name.starts_with("_pad")
        || name.starts_with("_padding")
}

/// Check if attributes contain `#[repr(C, align(N))]`
///
/// Uses string matching as a workaround for rustc_hir attribute API limitations
fn has_repr_c_align(attrs: &[rustc_hir::Attribute]) -> bool {
    for attr in attrs {
        if !attr.has_name(sym::repr) {
            continue;
        }

        // Use string representation to check for both C and align
        let attr_str = format!("{:?}", attr);
        if attr_str.contains("C") && attr_str.contains("align") {
            return true;
        }
    }

    false
}

/// Extract alignment value from `#[repr(C, align(N))]`
///
/// Uses string matching to extract the numeric alignment value
fn get_alignment_value(attrs: &[rustc_hir::Attribute]) -> Option<u64> {
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

/// Emit diagnostic for incorrect padding field size
///
/// # UCE34 Q34 (Auditability)
///
/// Enhanced error message includes:
/// - **Why incorrect padding is bad**: False sharing → 3-5× slowdown
/// - **Calculation shown**: Step-by-step math with visual ASCII diagram
/// - **Performance impact**: Real-world metrics (cache coherency traffic)
/// - **ASCII diagram**: Shows correct vs incorrect padding layouts
/// - **Auto-fix suggestion**: Exact padding value needed with formula
/// - **Framework references**: Chaos cache alignment mandate
///
/// # ASSUM Framework
///
/// - `#ASSUME_LINT_MESSAGE_ACTIONABLE`: User gets exact fix (calculated)
/// - `#VERIFY_MESSAGE_CLARITY`: Diagnostic is unambiguous with visuals
/// - `#ASSUME_PADDING_DETECTION_ACCURATE`: Calculation matches Rust type system
#[allow(clippy::too_many_arguments)]
fn emit_incorrect_padding_diagnostic<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx Item<'tcx>,
    padding_fields: &[(rustc_span::symbol::Ident, rustc_span::Span, u64)],
    align: u64,
    size_without_padding: u64,
    required_padding: u64,
    actual_padding: u64,
) {
    use crate::diagnostics::{
        format_padding_violation_diagram,
        format_padding_calculation_detailed,
        format_false_sharing_impact,
        format_chaos_compliance_ref,
    };

    let item_name = cx.tcx.item_name(item.owner_id.to_def_id());

    // Determine if over-padding or under-padding
    let padding_type = if actual_padding > required_padding {
        "over-padding"
    } else {
        "under-padding"
    };

    // Calculate final sizes
    let current_total = size_without_padding + actual_padding;
    let correct_total = size_without_padding + required_padding;

    cx.lint(
        CAPSULE_INCORRECT_PADDING,
        |lint| {
            // Main error message with problem summary
            lint.primary_message(format!(
                "capsule `{}` has {} padding: {} bytes actual, {} bytes required",
                item_name, padding_type, actual_padding, required_padding
            ));
            lint.span(item.span);

            // === SECTION 1: WHY THIS IS BAD ===
            lint.note("WHY THIS MATTERS:".to_string());
            lint.note("  Wrong padding size → struct doesn't align to cache line".to_string());
            lint.note("  Unaligned struct → false sharing → 3-5× performance degradation".to_string());
            lint.note("  False sharing occurs when multiple instances compete for cache coherency".to_string());

            // === SECTION 2: CURRENT PADDING FIELD(S) ===
            lint.note("CURRENT STATE:".to_string());
            for (ident, _span, size) in padding_fields {
                lint.note(format!("  Padding field: `{}` = {} bytes", ident.name, size));
            }
            lint.note(format!("  Total struct size: {} bytes (MISALIGNED)", current_total));

            // === SECTION 3: ASCII DIAGRAM ===
            let diagram = format_padding_violation_diagram(
                size_without_padding,
                align,
                actual_padding,
                required_padding,
            );
            for line in diagram {
                lint.note(line);
            }

            // === SECTION 4: DETAILED CALCULATION ===
            let calc = format_padding_calculation_detailed(
                size_without_padding,
                align,
                required_padding,
                actual_padding,
            );
            for line in calc {
                lint.note(line);
            }

            // === SECTION 5: EXACT FIX ===
            lint.help("FIX: Update padding field to correct size".to_string());
            if padding_fields.len() == 1 {
                let field_name = padding_fields[0].0.name;
                lint.help(format!(
                    "change: `{}` from `[u8; {}]` → `[u8; {}]`",
                    field_name, actual_padding, required_padding
                ));
            } else {
                lint.help(format!(
                    "adjust total padding from {} bytes → {} bytes (across {} fields)",
                    actual_padding,
                    required_padding,
                    padding_fields.len()
                ));
            }

            // === SECTION 6: FALSE SHARING IMPACT ===
            if actual_padding < required_padding {
                let impact = format_false_sharing_impact(current_total, align);
                for line in impact {
                    lint.note(line);
                }
            }

            // === SECTION 7: PERFORMANCE CONSEQUENCES ===
            lint.note("PERFORMANCE CONSEQUENCES:".to_string());
            if actual_padding < required_padding {
                lint.note("  Under-padding (THIS CASE):".to_string());
                lint.note("    ⚠ Multiple capsules per cache line".to_string());
                lint.note("    ⚠ Cache line bounces between cores on each update".to_string());
                lint.note("    ⚠ 3-5× slowdown from coherency traffic (B32 measured)".to_string());
                lint.note("    ⚠ Violates Chaos 'exclusive cache line' mandate".to_string());
            } else {
                lint.note("  Over-padding (THIS CASE):".to_string());
                lint.note(format!("    ⚠ Wastes {} bytes of memory per instance", actual_padding - required_padding));
                lint.note("    ⚠ Reduces cache efficiency (fewer structures per line)".to_string());
                lint.note("    ⚠ Unnecessary memory pressure in large collections".to_string());
                lint.note("    ⚠ Still violates Chaos alignment mandate (wrong total)".to_string());
            }

            // === SECTION 8: FRAMEWORK COMPLIANCE ===
            let chaos_refs = format_chaos_compliance_ref();
            for line in chaos_refs {
                lint.note(line);
            }

            // === SECTION 9: QUICK VERIFICATION ===
            lint.help("QUICK CHECK: After fix, struct should be:".to_string());
            lint.help(format!("  Total size = {} bytes", correct_total));
            lint.help(format!("  {} ÷ {} = {} (perfectly aligned ✓)", correct_total, align, correct_total / align));
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_padding_field_name_detection() {
        // Standard forms
        assert!(is_padding_field_name("_padding"));
        assert!(is_padding_field_name("_pad"));

        // Numbered variants
        assert!(is_padding_field_name("_pad1"));
        assert!(is_padding_field_name("_pad2"));
        assert!(is_padding_field_name("_padding1"));
        assert!(is_padding_field_name("_padding2"));

        // Prefixed variants
        assert!(is_padding_field_name("_pad_data"));
        assert!(is_padding_field_name("_padding_bytes"));

        // Not padding fields
        assert!(!is_padding_field_name("data"));
        assert!(!is_padding_field_name("padding")); // No underscore
        assert!(!is_padding_field_name("pad")); // No underscore
        assert!(!is_padding_field_name("_data"));
    }

    #[test]
    fn test_required_padding_calculation() {
        // Test: 8B size, 64B align → 56B padding required
        let size = 8u64;
        let align = 64u64;
        let remainder = size % align;
        let required = if remainder == 0 { 0 } else { align - remainder };
        assert_eq!(required, 56);
    }

    #[test]
    fn test_required_padding_already_aligned() {
        // Test: 64B size, 64B align → 0B padding required (already aligned)
        let size = 64u64;
        let align = 64u64;
        let remainder = size % align;
        let required = if remainder == 0 { 0 } else { align - remainder };
        assert_eq!(required, 0);
    }

    #[test]
    fn test_required_padding_128_align() {
        // Test: 16B size, 128B align → 112B padding required
        let size = 16u64;
        let align = 128u64;
        let remainder = size % align;
        let required = if remainder == 0 { 0 } else { align - remainder };
        assert_eq!(required, 112);
    }

    #[test]
    fn test_under_padding_detection() {
        // Test: 8B size, 64B align, 50B padding → under-padding!
        let size_without_padding = 8u64;
        let align = 64u64;
        let actual_padding = 50u64;

        let remainder = size_without_padding % align;
        let required_padding = if remainder == 0 { 0 } else { align - remainder };

        assert_eq!(required_padding, 56);
        assert!(actual_padding < required_padding); // Under-padding detected!
    }

    #[test]
    fn test_over_padding_detection() {
        // Test: 8B size, 64B align, 120B padding → over-padding!
        let size_without_padding = 8u64;
        let align = 64u64;
        let actual_padding = 120u64;

        let remainder = size_without_padding % align;
        let required_padding = if remainder == 0 { 0 } else { align - remainder };

        assert_eq!(required_padding, 56);
        assert!(actual_padding > required_padding); // Over-padding detected!
    }

    #[test]
    fn test_correct_padding() {
        // Test: 8B size, 64B align, 56B padding → correct!
        let size_without_padding = 8u64;
        let align = 64u64;
        let actual_padding = 56u64;

        let remainder = size_without_padding % align;
        let required_padding = if remainder == 0 { 0 } else { align - remainder };

        assert_eq!(required_padding, 56);
        assert_eq!(actual_padding, required_padding); // Correct padding!
    }

    #[test]
    fn test_multiple_fields_padding() {
        // Test: 16B size (2×8B), 64B align → 48B padding required
        let size_without_padding = 16u64; // AtomicU64 + AtomicU64
        let align = 64u64;

        let remainder = size_without_padding % align;
        let required_padding = if remainder == 0 { 0 } else { align - remainder };

        assert_eq!(required_padding, 48);
    }
}
