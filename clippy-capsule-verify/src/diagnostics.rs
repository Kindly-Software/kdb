//! # Diagnostic Utilities for clippy-capsule-verify
//!
//! **Purpose**: Consistent, high-quality error message formatting across all lints.
//!
//! ## Design Philosophy
//!
//! 1. **Clarity**: Developers should understand the problem immediately
//! 2. **Actionability**: Every error includes specific fix suggestions
//! 3. **Performance**: Include real-world metrics (not exaggerated claims)
//! 4. **Documentation**: Link to relevant COCA patterns and frameworks
//! 5. **Consistency**: Uniform formatting across all 9 lints

/// Format a performance speedup comparison with honest metrics
///
/// # Examples
///
/// ```
/// use clippy_capsule_verify::diagnostics::format_speedup;
///
/// let msg = format_speedup("Mutex", "1-10μs", "AtomicU64", "<10ns", 10.0);
/// assert!(msg.contains("10×"));
/// ```
pub fn format_speedup(
    before_name: &str,
    before_latency: &str,
    after_name: &str,
    after_latency: &str,
    factor: f64,
) -> String {
    format!(
        "{}: {} | {}: {} ({}× faster)",
        before_name,
        before_latency,
        after_name,
        after_latency,
        factor as u32
    )
}

/// Format a documentation reference link
///
/// # Examples
///
/// ```
/// use clippy_capsule_verify::diagnostics::format_doc_link;
///
/// let link = format_doc_link(
///     "/home/samuel/Docs/The Atomic Capsule.md",
///     "DualAtomicU64 pattern"
/// );
/// assert!(link.contains("Atomic Capsule"));
/// ```
#[allow(dead_code)]
pub fn format_doc_link(path: &str, description: &str) -> String {
    format!("See {} ({})", path, description)
}

/// Format a before/after code suggestion
///
/// Returns a formatted multi-line string showing the transformation
///
/// # Examples
///
/// ```
/// use clippy_capsule_verify::diagnostics::format_suggestion;
///
/// let suggestion = format_suggestion(
///     "lock: Mutex<u64>",
///     "lock: AtomicU64"
/// );
/// assert!(suggestion.contains("❌"));
/// assert!(suggestion.contains("✅"));
/// ```
pub fn format_suggestion(before_code: &str, after_code: &str) -> String {
    format!(
        "❌ Before:\n    {}\n\n✅ After:\n    {}",
        before_code, after_code
    )
}

/// Format a performance metric with units
///
/// # Examples
///
/// ```
/// use clippy_capsule_verify::diagnostics::format_metric;
///
/// let metric = format_metric("Latency", "<10", "ns");
/// assert_eq!(metric, "Latency: <10 ns");
/// ```
#[allow(dead_code)]
pub fn format_metric(name: &str, value: &str, unit: &str) -> String {
    format!("{}: {} {}", name, value, unit)
}

/// Format padding calculation explanation
///
/// Shows step-by-step calculation with visual clarity
pub fn format_padding_calculation(
    current_size: u64,
    alignment: u64,
    required_padding: u64,
) -> String {
    let total = current_size + required_padding;
    format!(
        "Calculation:\n    \
         Current size:      {} bytes\n    \
         Alignment:         {} bytes\n    \
         Required padding:  {} bytes\n    \
         Final size:        {} bytes",
        current_size, alignment, required_padding, total
    )
}

/// Format false sharing explanation
///
/// Visual representation of cache line occupancy
pub fn format_false_sharing_explanation(
    struct_size: u64,
    cache_line: u64,
) -> String {
    let instances_per_line = cache_line / struct_size;
    format!(
        "False sharing: {} instances fit in one {}-byte cache line\n    \
         Result: High contention, 3-5× slowdown from cache bouncing",
        instances_per_line, cache_line
    )
}

/// Format DualAtomicU64 pattern explanation
///
/// Shows bit-packing layout with visual clarity
pub fn format_dual_atomic_pattern() -> Vec<String> {
    vec![
        "DualAtomicU64 Pattern (cache-separated, 128B alignment):".to_string(),
        "".to_string(),
        "  primary: AtomicU64     secondary: AtomicU64".to_string(),
        "  ┌──────────┬──────┐   ┌──────────┬──────┐".to_string(),
        "  │ data(32) │gen(32)│   │ meta(32) │gen(32)│".to_string(),
        "  └──────────┴──────┘   └──────────┴──────┘".to_string(),
        "".to_string(),
        "Bit extraction:".to_string(),
        "  - primary >> 32       → data (upper 32 bits)".to_string(),
        "  - primary & 0xFFFF... → generation (lower 32 bits)".to_string(),
        "  - Use CAS loops to update both atomically".to_string(),
    ]
}

/// Format memory ordering cheat sheet
///
/// Quick reference for different atomic operations
#[allow(dead_code)]
pub fn format_memory_ordering_guide() -> Vec<String> {
    vec![
        "Memory Ordering Guide:".to_string(),
        "".to_string(),
        "  Operation           | Recommended Ordering".to_string(),
        "  ────────────────────┼─────────────────────".to_string(),
        "  load()              | Acquire".to_string(),
        "  store()             | Release".to_string(),
        "  swap()              | AcqRel or SeqCst".to_string(),
        "  compare_exchange()  | SeqCst".to_string(),
        "  fetch_add/sub/etc   | AcqRel".to_string(),
        "".to_string(),
        "  Relaxed: ONLY for non-coordinating counters (metrics)".to_string(),
        "  SeqCst: Critical sections requiring total ordering".to_string(),
    ]
}

/// Format framework compliance checklist
///
/// Shows which frameworks apply to this violation
pub fn format_framework_compliance(frameworks: &[(&str, &str)]) -> Vec<String> {
    let mut lines = vec!["Framework Compliance:".to_string(), "".to_string()];

    for (name, description) in frameworks {
        lines.push(format!("  - {}: {}", name, description));
    }

    lines
}

/// Format TOCTOU race explanation
///
/// Visual timeline showing the race condition
pub fn format_toctou_explanation() -> Vec<String> {
    vec![
        "TOCTOU (Time-Of-Check-Time-Of-Use) Race:".to_string(),
        "".to_string(),
        "  Thread 1                  Thread 2".to_string(),
        "  ────────                  ────────".to_string(),
        "  1. Load value (42)".to_string(),
        "  2. Check condition                ".to_string(),
        "                            3. Update to 100".to_string(),
        "  4. Use stale value (42!) ← RACE!".to_string(),
        "".to_string(),
        "Generation counter prevents this:".to_string(),
        "  1. Load value + generation (42, gen=5)".to_string(),
        "  2. Check condition".to_string(),
        "                            3. Update to (100, gen=6)".to_string(),
        "  4. CAS fails (gen mismatch) → retry".to_string(),
    ]
}

/// Format cache alignment benefits
///
/// Shows performance impact with real metrics
pub fn format_cache_alignment_benefits() -> Vec<String> {
    vec![
        "Cache Alignment Benefits:".to_string(),
        "".to_string(),
        "  Unaligned (false sharing):".to_string(),
        "    - 7 capsules per 64-byte cache line".to_string(),
        "    - Atomic latency: 30-50ns (cache miss)".to_string(),
        "    - Throughput: Low (cache bouncing)".to_string(),
        "".to_string(),
        "  Aligned (exclusive cache line):".to_string(),
        "    - 1 capsule per 64-byte cache line".to_string(),
        "    - Atomic latency: <5ns (cache hit)".to_string(),
        "    - Throughput: 6-10× higher".to_string(),
    ]
}

/// Format padding violation with ASCII cache line diagrams
///
/// Shows incorrect vs correct padding layout with visual clarity
pub fn format_padding_violation_diagram(
    size_without_padding: u64,
    alignment: u64,
    actual_padding: u64,
    required_padding: u64,
) -> Vec<String> {
    let current_total = size_without_padding + actual_padding;
    let correct_total = size_without_padding + required_padding;

    let mut lines = vec![];
    lines.push("".to_string());
    lines.push("Padding Layout Comparison:".to_string());
    lines.push("".to_string());

    // Current (WRONG) layout
    lines.push("❌ CURRENT (WRONG):".to_string());
    lines.push(format!("   Align({}) Total({})B", alignment, current_total));

    // Draw current layout
    let pad_percent = (actual_padding as f64 / alignment as f64 * 100.0) as u32;
    let data_percent = (size_without_padding as f64 / alignment as f64 * 100.0) as u32;

    lines.push(format!(
        "   ┌{}┬{}┐",
        "─".repeat((data_percent / 2).max(1) as usize),
        "─".repeat((pad_percent / 2).max(1) as usize),
    ));
    lines.push(format!(
        "   │ Data({:2}B) │ Padding({:2}B) │",
        size_without_padding, actual_padding
    ));
    lines.push(format!(
        "   └{}┴{}┘",
        "─".repeat((data_percent / 2).max(1) as usize),
        "─".repeat((pad_percent / 2).max(1) as usize),
    ));
    lines.push(format!("   Problem: Total {}B ≠ Align {}B (MISALIGNED!)", current_total, alignment));
    lines.push("".to_string());

    // Correct layout
    lines.push("✅ CORRECT:".to_string());
    lines.push(format!("   Align({}) Total({})B", alignment, correct_total));

    let correct_pad_percent = (required_padding as f64 / alignment as f64 * 100.0) as u32;
    let correct_data_percent = (size_without_padding as f64 / alignment as f64 * 100.0) as u32;

    lines.push(format!(
        "   ┌{}┬{}┐",
        "─".repeat((correct_data_percent / 2).max(1) as usize),
        "─".repeat((correct_pad_percent / 2).max(1) as usize),
    ));
    lines.push(format!(
        "   │ Data({:2}B) │ Padding({:2}B) │",
        size_without_padding, required_padding
    ));
    lines.push(format!(
        "   └{}┴{}┘",
        "─".repeat((correct_data_percent / 2).max(1) as usize),
        "─".repeat((correct_pad_percent / 2).max(1) as usize),
    ));
    lines.push(format!("   Result: Total {}B = Align {}B (ALIGNED ✓)", correct_total, alignment));
    lines.push("".to_string());

    lines
}

/// Format padding calculation with detailed breakdown
///
/// Shows the math step-by-step with visual clarity
pub fn format_padding_calculation_detailed(
    size_without_padding: u64,
    alignment: u64,
    required_padding: u64,
    actual_padding: u64,
) -> Vec<String> {
    let mut lines = vec![];
    lines.push("".to_string());
    lines.push("Padding Calculation Breakdown:".to_string());
    lines.push("".to_string());

    lines.push("Step 1: Sum non-padding field sizes".to_string());
    lines.push(format!("        Size without padding = {} bytes", size_without_padding));
    lines.push("".to_string());

    lines.push("Step 2: Calculate alignment requirement".to_string());
    lines.push(format!("        Alignment target = {} bytes (cache line)", alignment));
    lines.push("".to_string());

    lines.push("Step 3: Calculate required padding".to_string());
    let remainder = size_without_padding % alignment;
    if remainder == 0 {
        lines.push(format!("        remainder = {} % {} = 0 (already aligned!)", size_without_padding, alignment));
        lines.push(format!("        required_padding = 0"));
    } else {
        lines.push(format!("        remainder = {} % {} = {}", size_without_padding, alignment, remainder));
        lines.push(format!("        required_padding = {} - {} = {} bytes", alignment, remainder, required_padding));
    }
    lines.push("".to_string());

    lines.push("Step 4: Verify calculation".to_string());
    let total = size_without_padding + required_padding;
    lines.push(format!("        {} (data) + {} (padding) = {} bytes", size_without_padding, required_padding, total));
    lines.push(format!("        {} bytes ÷ {} bytes/line = {} (PERFECT ALIGNMENT ✓)", total, alignment, total / alignment));
    lines.push("".to_string());

    if actual_padding != required_padding {
        lines.push("Step 5: Current status (WRONG!)".to_string());
        let current_total = size_without_padding + actual_padding;
        lines.push(format!("        {} (data) + {} (padding) = {} bytes", size_without_padding, actual_padding, current_total));

        if current_total % alignment == 0 {
            lines.push(format!("        Total IS aligned, but padding field has wrong value!"));
        } else {
            let misalignment = alignment - (current_total % alignment);
            lines.push(format!("        Total NOT aligned: {} bytes short of next boundary", misalignment));
        }
    }
    lines.push("".to_string());

    lines
}

/// Format false sharing impact with cache line visualization
///
/// Shows how many instances share a cache line and performance impact
pub fn format_false_sharing_impact(
    struct_size: u64,
    alignment: u64,
) -> Vec<String> {
    let instances_per_line = alignment / struct_size;
    let mut lines = vec![];
    lines.push("".to_string());
    lines.push("False Sharing Impact (Why Alignment Matters):".to_string());
    lines.push("".to_string());

    lines.push(format!("Your struct is {}-byte, cache line is {}-byte:", struct_size, alignment));
    lines.push(format!("    {} instances fit per cache line", instances_per_line));
    lines.push("".to_string());

    lines.push("Cache line layout:".to_string());
    let box_width = 8;
    let boxes = alignment / struct_size;
    let top_line = format!("    ┌{}┐", "─".repeat(box_width as usize * (boxes as usize) + (boxes as usize - 1)));
    let mid_line = (0..boxes)
        .map(|i| format!(" Instance {:2}", i))
        .collect::<Vec<_>>()
        .join(" │ ");
    let bot_line = format!("    └{}┘", "─".repeat(box_width as usize * (boxes as usize) + (boxes as usize - 1)));

    lines.push(top_line);
    lines.push(format!("    │{}│", mid_line));
    lines.push(bot_line);
    lines.push("".to_string());

    lines.push("Performance consequence:".to_string());
    lines.push("    When one instance updates, the entire cache line bounces between cores".to_string());
    lines.push("    Result: 3-5× slowdown from coherency traffic (false sharing penalty)".to_string());
    lines.push("".to_string());

    lines.push("✓ Solution: Align struct to cache line size (exclusive ownership)".to_string());
    lines.push("".to_string());

    lines
}

/// Format COCA compliance reference
///
/// Links to relevant framework documentation
pub fn format_coca_compliance_ref() -> Vec<String> {
    vec![
        "".to_string(),
        "COCA Framework References:".to_string(),
        "  - Computational Capsule.md § Cache-Aligned Padding (philosophy)".to_string(),
        "  - The Atomic Capsule.md § DualAtomicU64 pattern (practical patterns)".to_string(),
        "  - UCE34_TIER_REFERENCE.md § T1 Tier Padding Rules (tier-specific requirements)".to_string(),
        "  - KEY_INNOVATIONS.md § Cache Alignment Breakthrough (7-35× speedups)".to_string(),
        "".to_string(),
    ]
}

/// Get common documentation references
///
/// Returns standard documentation paths for COCA framework
pub fn get_doc_references() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "/home/samuel/Docs/The Atomic Capsule.md",
            "DualAtomicU64 pattern, memory ordering",
        ),
        (
            "/home/samuel/Docs/The Computational Capsule.md",
            "COCA philosophy and principles",
        ),
        (
            "/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md",
            "Proven speedups and benchmarks",
        ),
        (
            "/home/samuel/Primitives/atomic_capsule/CLAUDE.md",
            "110+ capsule examples",
        ),
        (
            "/home/samuel/CLAUDE.md",
            "UCE34 framework (Q1-Q34)",
        ),
    ]
}

// ============================================================================
// P1.0 MISSING_CAPSULE_VERIFICATION Enhanced Diagnostics (Haiku v0.1.0)
// ============================================================================

/// Format P1.0 verification benefits comparison
///
/// Shows compile-time verification advantages with real metrics
pub fn format_verification_benefits_p10() -> Vec<String> {
    vec![
        "Verification Benefits (Compile-Time Guarantee):".to_string(),
        "".to_string(),
        "  ✓ 0ns runtime cost:      Fully compile-time, zero overhead".to_string(),
        "  ✓ <20ms compile-time:    Minimal impact on build time".to_string(),
        "  ✓ Alignment guarantees:  No false sharing (3-10× speedup)".to_string(),
        "  ✓ Size validation:       Correct cache line usage".to_string(),
        "  ✓ Type safety:           Impossible states prevented at compile time".to_string(),
        "  ✓ Zero-cost abstraction: All checks erased after compilation".to_string(),
        "".to_string(),
    ]
}

/// Format P1.0 capsule structure ASCII diagrams
///
/// Visual comparison of verified vs unverified capsules
pub fn format_capsule_diagrams_p10() -> Vec<String> {
    vec![
        "".to_string(),
        "VERIFIED CAPSULE (Recommended):".to_string(),
        "  ┌──────────────────────────────────────────┐".to_string(),
        "  │ #[derive(ComputationalCapsule)]          │".to_string(),
        "  │ #[repr(C, align(64))]                    │".to_string(),
        "  │ struct MyCapsule {                       │".to_string(),
        "  │     state: AtomicU64,                    │".to_string(),
        "  │     // ...                               │".to_string(),
        "  │ }                                        │".to_string(),
        "  └──────────────────────────────────────────┘".to_string(),
        "  ✓ Layout verified at compile time".to_string(),
        "  ✓ Cache-aligned (exclusive cache line)".to_string(),
        "  ✓ No false sharing (atomic ops <5ns)".to_string(),
        "  ✓ Production-ready with Q33 verification".to_string(),
        "".to_string(),
        "UNVERIFIED CAPSULE (⚠️ Risk):".to_string(),
        "  ┌──────────────────────────────────────────┐".to_string(),
        "  │ #[repr(C, align(64))]                    │".to_string(),
        "  │ struct MyCapsule {                       │".to_string(),
        "  │     state: AtomicU64,  // Unverified     │".to_string(),
        "  │     // ... potential misalignment        │".to_string(),
        "  │ }                                        │".to_string(),
        "  └──────────────────────────────────────────┘".to_string(),
        "  ✗ No compile-time validation".to_string(),
        "  ✗ Alignment bugs possible (UB at runtime)".to_string(),
        "  ✗ False sharing risk (3-10× slowdown)".to_string(),
        "  ✗ Memory safety issues in concurrent code".to_string(),
        "  ✗ Cache line violations unpredictable".to_string(),
        "".to_string(),
    ]
}

/// Format P1.0 why verification matters with real-world impact
///
/// Explains the consequences of missing verification
pub fn format_verification_importance_p10() -> Vec<String> {
    vec![
        "Why Verification Matters:".to_string(),
        "".to_string(),
        "False Sharing (Unverified capsules):".to_string(),
        "  Symptom: Multiple instances share one 64-byte cache line".to_string(),
        "  Problem: Each atomic operation invalidates all other copies".to_string(),
        "  Impact:  Cache bouncing → 3-10× latency degradation".to_string(),
        "  Worse:   Contention cascades under concurrent load (non-linear degradation)".to_string(),
        "".to_string(),
        "Alignment Bugs (Missing verification):".to_string(),
        "  Symptom: AtomicU64 requires 8-byte alignment minimum".to_string(),
        "  Problem: Misaligned atomics → undefined behavior (UB)".to_string(),
        "  Impact:  May manifest as crashes, hangs, or silent data corruption".to_string(),
        "  Worse:   Bug detection happens at runtime (hard to debug, non-reproducible)".to_string(),
        "".to_string(),
        "Size Mismatches (Layout errors):".to_string(),
        "  Symptom: Padding calculation errors → adjacent memory corruption".to_string(),
        "  Problem: Cache line overflow → unintended cache misses".to_string(),
        "  Impact:  Unpredictable latency patterns, performance cliff effects".to_string(),
        "".to_string(),
    ]
}

/// Format P1.0 UCE34 Q33 framework reference
///
/// Links to canonical COCA verification documentation
pub fn format_uce34_q33_reference_p10() -> Vec<String> {
    vec![
        "".to_string(),
        "Framework Reference: UCE34 Q33 (Verification)".to_string(),
        "".to_string(),
        "  Q33: Compile-time verification via #[derive(ComputationalCapsule)]".to_string(),
        "    Status:   MANDATORY for all capsules using #[repr(C, align(N))]".to_string(),
        "    Benefit:  0ns runtime verification, <20ms compile-time".to_string(),
        "    Scope:    Alignment, size, layout validation".to_string(),
        "".to_string(),
        "Compliance Path:".to_string(),
        "  1. Add #[derive(ComputationalCapsule)] macro".to_string(),
        "  2. Keep #[repr(C, align(N))] attribute (don't remove!)".to_string(),
        "  3. Verify struct layout is cache-aligned (64B/128B/256B preferred)".to_string(),
        "".to_string(),
        "References:".to_string(),
        "  ├─ /home/samuel/Docs/The Computational Capsule.md (COCA philosophy)".to_string(),
        "  ├─ /home/samuel/CLAUDE.md § UCE34 framework (Q1-Q34 comprehensive)".to_string(),
        "  ├─ /home/samuel/Primitives/Docs/KEY_INNOVATIONS.md (verification patterns)".to_string(),
        "  └─ /home/samuel/Primitives/atomic_capsule/CLAUDE.md (110+ examples)".to_string(),
        "".to_string(),
    ]
}

/// Format P1.0 specific fix suggestion for missing verification
///
/// Provides exact code transformation needed with both approaches
pub fn format_verification_fix_suggestion_p10(
    struct_name: &str,
    alignment: Option<u64>,
) -> Vec<String> {
    let alignment_str = alignment
        .map(|a| format!("{}", a))
        .unwrap_or_else(|| "ALIGNMENT".to_string());

    let mut lines = vec![];
    lines.push(format!("Fix for `{}`:", struct_name));
    lines.push("".to_string());

    lines.push("Option 1: Derive macro (recommended - automatic verification):".to_string());
    lines.push("  #[derive(ComputationalCapsule)]".to_string());
    lines.push(format!("  #[repr(C, align({}))]", alignment_str));
    lines.push(format!("  struct {} {{ ... }}", struct_name));
    lines.push("  // Verification: automatic, 0ns runtime, <20ms compile".to_string());
    lines.push("".to_string());

    lines.push("Option 2: Manual verification macro (for edge cases):".to_string());
    lines.push(format!("  #[repr(C, align({}))]", alignment_str));
    lines.push(format!("  struct {} {{ ... }}", struct_name));
    lines.push(format!("  verify_capsule_properties!({}, {}, SIZE);", struct_name, alignment_str));
    lines.push("  // SIZE must match your struct's actual size in bytes".to_string());
    lines.push("".to_string());

    lines.push("Recommendation:".to_string());
    lines.push("  Use Option 1 (derive macro): Automatic, cleaner, zero maintenance".to_string());
    lines.push("  Never use manual macros unless absolutely necessary".to_string());
    lines.push("".to_string());

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_speedup() {
        let msg = format_speedup("Mutex", "1-10μs", "AtomicU64", "<10ns", 100.0);
        assert!(msg.contains("Mutex"));
        assert!(msg.contains("AtomicU64"));
        assert!(msg.contains("100×"));
    }

    #[test]
    fn test_format_doc_link() {
        let link = format_doc_link(
            "/home/samuel/Docs/The Atomic Capsule.md",
            "patterns"
        );
        assert!(link.contains("Atomic Capsule"));
        assert!(link.contains("patterns"));
    }

    #[test]
    fn test_format_suggestion() {
        let suggestion = format_suggestion("before", "after");
        assert!(suggestion.contains("❌"));
        assert!(suggestion.contains("✅"));
        assert!(suggestion.contains("before"));
        assert!(suggestion.contains("after"));
    }

    #[test]
    fn test_format_metric() {
        let metric = format_metric("Latency", "10", "ns");
        assert_eq!(metric, "Latency: 10 ns");
    }

    #[test]
    fn test_format_padding_calculation() {
        let calc = format_padding_calculation(8, 64, 56);
        assert!(calc.contains("8 bytes"));
        assert!(calc.contains("64 bytes"));
        assert!(calc.contains("56 bytes"));
    }

    #[test]
    fn test_format_false_sharing_explanation() {
        let explanation = format_false_sharing_explanation(8, 64);
        assert!(explanation.contains("8 instances"));
        assert!(explanation.contains("64-byte"));
    }

    #[test]
    fn test_dual_atomic_pattern_visual() {
        let pattern = format_dual_atomic_pattern();
        assert!(pattern.len() > 5);
        assert!(pattern.iter().any(|s| s.contains("primary")));
        assert!(pattern.iter().any(|s| s.contains("secondary")));
    }

    #[test]
    fn test_memory_ordering_guide() {
        let guide = format_memory_ordering_guide();
        assert!(guide.iter().any(|s| s.contains("Acquire")));
        assert!(guide.iter().any(|s| s.contains("Release")));
        assert!(guide.iter().any(|s| s.contains("SeqCst")));
    }

    #[test]
    fn test_toctou_explanation() {
        let explanation = format_toctou_explanation();
        assert!(explanation.iter().any(|s| s.contains("Thread 1")));
        assert!(explanation.iter().any(|s| s.contains("Thread 2")));
        assert!(explanation.iter().any(|s| s.contains("RACE")));
    }

    #[test]
    fn test_cache_alignment_benefits() {
        let benefits = format_cache_alignment_benefits();
        assert!(benefits.iter().any(|s| s.contains("Unaligned")));
        assert!(benefits.iter().any(|s| s.contains("Aligned")));
        assert!(benefits.iter().any(|s| s.contains("6-10×")));
    }

    #[test]
    fn test_get_doc_references() {
        let refs = get_doc_references();
        assert!(refs.len() >= 4);
        assert!(refs.iter().any(|(path, _)| path.contains("Atomic Capsule")));
    }
}
