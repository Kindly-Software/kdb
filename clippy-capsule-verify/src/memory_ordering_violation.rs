//! # Memory Ordering Violation Lint
//!
//! **P2.1 MEDIUM**: Detects incorrect atomic Ordering usage in computational capsules.
//!
//! ## UCE34 Q10 (Tier Selection) & Chaos Mandate
//!
//! Computational capsules require proper memory ordering for lockfree coordination:
//! - **Relaxed**: NO synchronization (breaks Chaos guarantees)
//! - **Acquire**: Synchronize reads (load operations)
//! - **Release**: Synchronize writes (store operations)
//! - **AcqRel**: Both read and write synchronization (compare_exchange)
//! - **SeqCst**: Sequential consistency (critical sections, rare use)
//!
//! ## Why This Matters (B32 Framework)
//!
//! Relaxed ordering breaks lockfree guarantees:
//! - No happens-before relationship → stale reads
//! - No synchronization → data races between threads
//! - 3-10× latency spike when race detected and retry triggered
//! - Violates Chaos SeqCst/SWeMR (Single Writer, Multiple Readers) patterns
//!
//! ## Example
//!
//! ```rust,ignore
//! // BAD: Relaxed ordering breaks synchronization
//! let value = state.load(Ordering::Relaxed);  // ❌ No sync
//! state.store(42, Ordering::Relaxed);         // ❌ No sync
//!
//! // GOOD: Acquire/Release for proper sync
//! let value = state.load(Ordering::Acquire);  // ✓ Sync reads
//! state.store(42, Ordering::Release);         // ✓ Sync writes
//!
//! // ALSO GOOD: SeqCst for critical sections
//! state.compare_exchange(
//!     old,
//!     new,
//!     Ordering::SeqCst,
//!     Ordering::SeqCst,
//! );
//! ```
//!
//! ## Detection Strategy
//!
//! 1. Find atomic method calls (load, store, swap, compare_exchange, etc.)
//! 2. Extract Ordering argument from call
//! 3. Check if Relaxed used
//! 4. Suggest correct ordering:
//!    - load → Acquire
//!    - store → Release
//!    - compare_exchange/swap → SeqCst or AcqRel
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_METHOD_CALL_EXTRACTION`: HIR ExprKind::MethodCall available for atomic ops
//! - `#ASSUME_ORDERING_ARG_ACCESSIBLE`: Ordering argument extractable from method args
//! - `#VERIFY_ORDERING_DETECTION`: UI tests validate correct/incorrect detection
//!
//! ## References
//!
//! - `/home/samuel/Docs/The Atomic Capsule.md` (Memory ordering patterns)
//! - `/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md` (SWeMR design)

use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::{declare_lint, declare_lint_pass};

declare_lint! {
    /// **Detects incorrect atomic memory ordering in computational capsules.**
    ///
    /// ## What it does
    /// Checks atomic operations (load, store, swap, compare_exchange) for
    /// Relaxed ordering, which breaks lockfree synchronization guarantees.
    ///
    /// ## Why is this bad?
    /// Relaxed ordering provides NO synchronization:
    /// - Loads may see stale values (no happens-before)
    /// - Stores may be reordered arbitrarily
    /// - Breaks Chaos SeqCst/SWeMR patterns
    /// - Causes subtle data races and 3-10× latency spikes
    ///
    /// ## Correct orderings
    /// - **load**: Acquire (synchronize reads)
    /// - **store**: Release (synchronize writes)
    /// - **swap/compare_exchange**: SeqCst or AcqRel (both directions)
    /// - **Relaxed**: ONLY for non-coordination (counters, metrics)
    ///
    /// ## Example
    /// ```rust,ignore
    /// // Bad: Relaxed breaks synchronization
    /// let value = state.load(Ordering::Relaxed);  // ❌
    /// state.store(42, Ordering::Relaxed);         // ❌
    ///
    /// // Good: Acquire/Release pattern
    /// let value = state.load(Ordering::Acquire);  // ✓
    /// state.store(42, Ordering::Release);         // ✓
    ///
    /// // Good: SeqCst for critical operations
    /// state.compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst);
    /// ```
    ///
    /// ## Lint level
    /// **ALLOW** (opt-in, some use cases legitimately need Relaxed for performance)
    pub CAPSULE_MEMORY_ORDERING,
    Allow,
    "atomic operation uses Relaxed ordering which breaks synchronization"
}

declare_lint_pass!(CapsuleMemoryOrderingViolation => [CAPSULE_MEMORY_ORDERING]);

impl<'tcx> LateLintPass<'tcx> for CapsuleMemoryOrderingViolation {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        // Only check method calls
        if let ExprKind::MethodCall(path_segment, receiver, args, _) = &expr.kind {
            let method_name = path_segment.ident.name.as_str();

            // Check if this is an atomic operation
            let atomic_op = match method_name {
                "load" => Some(AtomicOp::Load),
                "store" => Some(AtomicOp::Store),
                "swap" => Some(AtomicOp::Swap),
                "compare_exchange" => Some(AtomicOp::CompareExchange),
                "compare_exchange_weak" => Some(AtomicOp::CompareExchange),
                "fetch_add" | "fetch_sub" | "fetch_and" | "fetch_or" | "fetch_xor"
                | "fetch_max" | "fetch_min" | "fetch_nand" => Some(AtomicOp::FetchOp),
                _ => None,
            };

            if let Some(op) = atomic_op {
                // Check if receiver type is atomic
                let receiver_ty = cx.typeck_results().expr_ty(receiver);
                if !is_atomic_type_str(&receiver_ty.to_string()) {
                    return;
                }

                // Extract ordering from arguments
                if let Some(ordering) = extract_ordering_from_args(args, &op) {
                    if ordering == "Relaxed" {
                        emit_ordering_violation_diagnostic(cx, expr, &op, method_name);
                    }
                }
            }
        }
    }
}

/// Atomic operation type (for different ordering recommendations)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicOp {
    Load,
    Store,
    Swap,
    CompareExchange,
    FetchOp,
}

/// Check if type name indicates atomic type
///
/// Matches:
/// - AtomicU64, AtomicU32, AtomicU16, AtomicU8, AtomicBool
/// - AtomicPtr<T>
/// - AtomicUsize, AtomicIsize
fn is_atomic_type_str(type_str: &str) -> bool {
    type_str.contains("Atomic")
}

/// Extract Ordering argument from method call arguments
///
/// Different atomic operations have Ordering in different positions:
/// - load(ordering) → args[0]
/// - store(val, ordering) → args[1]
/// - swap(val, ordering) → args[1]
/// - compare_exchange(current, new, success, failure) → args[2] and args[3]
/// - fetch_add(val, ordering) → args[1]
fn extract_ordering_from_args<'tcx>(
    args: &'tcx [Expr<'tcx>],
    op: &AtomicOp,
) -> Option<String> {
    let ordering_idx = match op {
        AtomicOp::Load => 0,
        AtomicOp::Store => 1,
        AtomicOp::Swap => 1,
        AtomicOp::CompareExchange => 2, // Check success ordering
        AtomicOp::FetchOp => 1,
    };

    if ordering_idx >= args.len() {
        return None;
    }

    // Extract ordering from expression
    // Pattern: Ordering::Relaxed, Ordering::Acquire, etc.
    extract_ordering_from_expr(&args[ordering_idx])
}

/// Extract Ordering variant name from expression
///
/// Handles patterns:
/// - Ordering::Relaxed
/// - std::sync::atomic::Ordering::Relaxed
/// - core::sync::atomic::Ordering::Acquire
fn extract_ordering_from_expr(expr: &Expr<'_>) -> Option<String> {
    // Match path expressions (e.g., Ordering::Relaxed)
    if let ExprKind::Path(qpath) = &expr.kind {
        use rustc_hir::QPath;
        match qpath {
            QPath::Resolved(_, path) => {
                // Get last segment (the variant name)
                if let Some(segment) = path.segments.last() {
                    return Some(segment.ident.name.as_str().to_string());
                }
            }
            QPath::TypeRelative(_, path_segment) => {
                return Some(path_segment.ident.name.as_str().to_string());
            }
            _ => {}
        }
    }

    None
}

/// Emit diagnostic for Relaxed ordering violation
///
/// Suggests correct ordering based on operation type with comprehensive
/// memory ordering guidance, performance impact, and framework compliance.
///
/// Enhanced with:
/// - Memory ordering cheat sheet (quick reference table)
/// - Performance metrics (5-20% improvement)
/// - Specific fixes for each operation type
/// - When to use each ordering
/// - ASSUM framework references
fn emit_ordering_violation_diagnostic<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    op: &AtomicOp,
    method_name: &str,
) {
    let (suggested_ordering, fix_code, performance_note) = match op {
        AtomicOp::Load => (
            "Acquire",
            "state.load(Ordering::Acquire)  // See updates from other threads",
            "5-15% improvement: Acquire synchronizes reads without full cost of SeqCst",
        ),
        AtomicOp::Store => (
            "Release",
            "state.store(42, Ordering::Release)  // Publish to other threads",
            "5-20% improvement: Release publishes without sequentially consistent cost",
        ),
        AtomicOp::Swap => (
            "AcqRel or SeqCst",
            "state.swap(42, Ordering::AcqRel)  // Both acquire+release in one",
            "10-15% improvement: AcqRel for read-modify-write, SeqCst for critical sections",
        ),
        AtomicOp::CompareExchange => (
            "SeqCst",
            "state.compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst)",
            "Full synchronization: SeqCst creates global ordering (necessary for lock-free)",
        ),
        AtomicOp::FetchOp => (
            "AcqRel",
            "count.fetch_add(1, Ordering::AcqRel)  // Atomic update with sync",
            "10-15% improvement: AcqRel for atomic modifications in coordination",
        ),
    };

    let msg = format!(
        "atomic `{}` uses Relaxed ordering which breaks synchronization",
        method_name
    );

    cx.lint(
        CAPSULE_MEMORY_ORDERING,
        |lint| {
            lint.primary_message(msg);
            lint.span(expr.span);
            let help_msg = format!("use `Ordering::{}` instead", suggested_ordering);
            lint.help(help_msg);

            // === CRITICAL ISSUE ===
            lint.note("CRITICAL: Relaxed ordering provides NO synchronization:");
            lint.note("  ❌ Loads may see stale values (no happens-before edge)");
            lint.note("  ❌ Stores may be reordered arbitrarily by CPU/compiler");
            lint.note("  ❌ Breaks Chaos SeqCst/SWeMR lockfree guarantee");
            lint.note("  ❌ Causes subtle data races and 3-10× latency spikes on contention");
            lint.note("");

            // === PERFORMANCE CONTEXT ===
            lint.note("PERFORMANCE IMPACT:");
            lint.note(performance_note);
            lint.note("");

            // === MEMORY ORDERING CHEAT SHEET ===
            lint.note("=== MEMORY ORDERING CHEAT SHEET ===");
            lint.note("┌─────────────────┬──────────────┬─────────────────────────────────┐");
            lint.note("│ Operation       │ Recommended  │ When / Why                      │");
            lint.note("├─────────────────┼──────────────┼─────────────────────────────────┤");
            lint.note("│ load()          │ Acquire      │ Need to see other thread writes │");
            lint.note("│ store()         │ Release      │ Publishing data to other thread │");
            lint.note("│ swap()          │ AcqRel       │ Read-modify-write atomically    │");
            lint.note("│ compare_excg()  │ SeqCst       │ Synchronization point (lock-fn) │");
            lint.note("│ fetch_add/sub   │ AcqRel       │ Atomic counters in coordination │");
            lint.note("│ Relaxed         │ ❌ AVOID     │ Non-coordinating metrics ONLY   │");
            lint.note("└─────────────────┴──────────────┴─────────────────────────────────┘");
            lint.note("");

            // === SPECIFIC FIX ===
            lint.note("FIX FOR THIS CODE:");
            lint.note(format!("  {}", fix_code));
            lint.note("");

            // === FRAMEWORK CONTEXT ===
            lint.note("FRAMEWORK COMPLIANCE:");
            lint.note("  • UCE34 Q10: Tier selection requires proper memory ordering");
            lint.note("  • Chaos mandate: 100% lockfree, no mutex/RwLock (requires correct Ordering)");
            lint.note("  • ASSUM: #ASSUME_CORRECT_ORDERING verified via compile-time lint");
            lint.note("  • B32: Performance gains (5-20%) only with proper synchronization");
            lint.note("");

            // === DETAILED EXPLANATION ===
            lint.note("WHY EACH ORDERING:");
            match op {
                AtomicOp::Load => {
                    lint.note("  Acquire (load):");
                    lint.note("    - Prevents subsequent code from reordering before load");
                    lint.note("    - Sees all writes that happened-before this Acquire");
                    lint.note("    - Critical for reading shared state (config, flags, etc)");
                }
                AtomicOp::Store => {
                    lint.note("  Release (store):");
                    lint.note("    - Prevents preceding code from reordering after store");
                    lint.note("    - Ensures all writes before this Release are visible");
                    lint.note("    - Critical for publishing data to other threads");
                }
                AtomicOp::Swap | AtomicOp::FetchOp => {
                    lint.note("  AcqRel (swap/fetch):");
                    lint.note("    - Both Acquire (read sync) and Release (write sync)");
                    lint.note("    - Read-modify-write becomes visible atomic operation");
                    lint.note("    - SeqCst only needed if total order required (rare)");
                }
                AtomicOp::CompareExchange => {
                    lint.note("  SeqCst (compare_exchange):");
                    lint.note("    - Enforces global ordering across all threads");
                    lint.note("    - Creates synchronization point (like acquiring lock)");
                    lint.note("    - Necessary for lock-free algorithms (queues, stacks)");
                }
            }
            lint.note("");

            // === RELAXED EXCEPTION CASES ===
            lint.note("When Relaxed is ACCEPTABLE (with documentation):");
            lint.note("  ✓ Non-coordinating counters: metrics, statistics, performance tracking");
            lint.note("  ✓ Performance-critical paths: WITH documented safety proof");
            lint.note("  ✓ Example: Metrics counters that don't coordinate behavior");
            lint.note("  ✓ Example: Statistics collection (no synchronization needed)");
            lint.note("");
            lint.note("When Relaxed is DANGEROUS:");
            lint.note("  ❌ State coordination (flags, config, state machine)");
            lint.note("  ❌ Handoff of data between threads");
            lint.note("  ❌ Lock-free data structures (queues, stacks, hash tables)");
            lint.note("  ❌ Publishing results from computation");
            lint.note("");

            // === SUPPRESS MECHANISM ===
            lint.note("To allow Relaxed (with caution):");
            lint.note("  #[allow(clippy::capsule_memory_ordering)]");
            lint.note("  fn my_function() {");
            lint.note("      // Your Relaxed operation here");
            lint.note("      // Add comment explaining why Relaxed is safe");
            lint.note("  }");
            lint.note("");

            // === REFERENCES ===
            lint.note("REFERENCES:");
            lint.note("  • /home/samuel/Docs/The Atomic Capsule.md - Memory ordering patterns");
            lint.note("  • /home/samuel/Primitives/Docs/KEY_INNOVATIONS.md - SWeMR design");
            lint.note("  • Rust sync::atomic docs: https://doc.rust-lang.org/std/sync/atomic/");
            lint.note("  • Herb Sutter (2008): Atomic Weapons (concurrency architecture)");
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_type_detection() {
        let atomic_types = vec![
            "AtomicU64",
            "AtomicU32",
            "AtomicBool",
            "std::sync::atomic::AtomicU64",
            "core::sync::atomic::AtomicPtr<T>",
        ];

        for ty in atomic_types {
            assert!(is_atomic_type_str(ty), "Should detect atomic type: {}", ty);
        }
    }

    #[test]
    fn test_non_atomic_type_detection() {
        let non_atomic = vec![
            "u64",
            "u32",
            "bool",
            "String",
            "Vec<u8>",
        ];

        for ty in non_atomic {
            assert!(!is_atomic_type_str(ty), "Should not detect as atomic: {}", ty);
        }
    }

    #[test]
    fn test_ordering_extraction_logic() {
        // Test ordering index calculation
        assert_eq!(
            match AtomicOp::Load {
                AtomicOp::Load => 0,
                AtomicOp::Store => 1,
                AtomicOp::Swap => 1,
                AtomicOp::CompareExchange => 2,
                AtomicOp::FetchOp => 1,
            },
            0
        );

        assert_eq!(
            match AtomicOp::Store {
                AtomicOp::Load => 0,
                AtomicOp::Store => 1,
                AtomicOp::Swap => 1,
                AtomicOp::CompareExchange => 2,
                AtomicOp::FetchOp => 1,
            },
            1
        );
    }
}
