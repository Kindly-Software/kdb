# Error Message Design Guide

**clippy-capsule-verify v0.2.0** - World-class diagnostic messages for Chaos compliance

## Philosophy

Our error messages follow the principle: **"Delight developers by making the fix obvious."**

Every error message should:
1. **Explain WHY** it's a problem (performance/correctness impact)
2. **Show WHAT'S wrong** (highlight the offending code)
3. **Provide HOW to fix** (with copy-paste ready code)
4. **Include METRICS** (honest performance data from B32 framework)
5. **Link to DOCS** (direct paths to Chaos patterns)

## Design Principles

### 1. Clarity Over Brevity

❌ **Bad** (terse, unhelpful):
```
error: Mutex forbidden in capsule
```

✅ **Good** (clear, actionable):
```
error: Mutex/RwLock causes 10-100× slowdown in computational capsule (field: `lock`)
  |
  | Replace Mutex with lockfree alternative:
  |
  | ❌ Before:
  |     lock: Mutex<HashMap<u64, u64>>  // FORBIDDEN - causes blocking
  |
  | ✅ After:
  |     lock: AtomicU64                    // Simple coordination (<5ns)
```

### 2. Visual Aids

Use ASCII art, diagrams, and formatting to enhance understanding:

```
Visual (64-byte cache line):
  ┌────┬────┬────┬────┬────┬────┬────┬────┐
  │ 8 instances of capsule `BadCapsule` │  ← HIGH contention!
  └────┴────┴────┴────┴────┴────┴────┴────┘
  All updating atomics → cache line bouncing
```

### 3. Honest Metrics (B32 Framework)

Never exaggerate. Use proven, reproducible numbers:

❌ **Bad** (exaggerated):
```
AtomicU64 is 1000× faster than Mutex!
```

✅ **Good** (honest, validated):
```
Mutex (lock/unlock): 30-100ns | AtomicU64 (CAS): <5ns (10-100× faster)
  └─ Validated via B32 framework (95% CI, 1000+ iterations)
```

### 4. Structured Formatting

Use section headers (`━━━`) to organize long messages:

```
━━━ Performance Impact ━━━

Mutex (lock/unlock): 30-100ns | AtomicU64 (CAS): <5ns (10× faster)

Why Mutex is slow:
  • Context switch overhead (~1-10μs)
  • Priority inversion in real-time systems
  • Non-deterministic latency (lock contention)

━━━ Lockfree Alternatives ━━━

1. AtomicU64/U32/U16/U8 (simple state):
   • Use case: Flags, counters, simple coordination
   • Latency: <5ns per operation
```

### 5. Ranked Solutions

Present multiple solutions, ranked by appropriateness:

```
Solution 1: DualAtomicU64 Pattern (RECOMMENDED)

Production-grade pattern with built-in versioning:
  primary: AtomicU64     secondary: AtomicU64
  ┌──────────┬──────┐   ┌──────────┬──────┐
  │ data(32) │gen(32)│   │ meta(32) │gen(32)│
  └──────────┴──────┘   └──────────┴──────┘

Solution 2: Standalone Generation Field (SIMPLE)

For simple capsules with single atomic field:
    generation: AtomicU64  // Increment on every state change
```

## Message Structure Template

Every enhanced error message follows this structure:

```rust
fn emit_diagnostic(cx: &LateContext, item: &Item, ...) {
    use crate::diagnostics::*;

    // 1. PRIMARY MESSAGE (lead with impact)
    let msg = format!(
        "IMPACT: problem description (field: `{}`)",
        field_name
    );

    cx.lint(LINT_NAME, |lint| {
        lint.primary_message(msg);
        lint.span(item.span);

        // 2. HELP (immediate actionable fix)
        lint.help("Concise fix instruction:");
        lint.note("");
        lint.note(&format_suggestion(before_code, after_code));

        // 3. EXPLANATION (why it matters)
        lint.note("");
        lint.note("━━━ Why This Matters ━━━");
        lint.note("");
        lint.note("Technical explanation...");

        // 4. PERFORMANCE METRICS (B32 validated)
        lint.note("");
        lint.note("━━━ Performance Impact ━━━");
        lint.note("");
        lint.note(&format_speedup(...));

        // 5. SOLUTIONS (ranked by appropriateness)
        lint.note("");
        lint.note("━━━ Solutions ━━━");
        lint.note("");
        lint.note("1. Best solution (most common)");
        lint.note("2. Alternative solution (specific use case)");

        // 6. FRAMEWORK COMPLIANCE
        lint.note("");
        lint.note("━━━ Framework Compliance ━━━");
        lint.note("");
        for line in format_framework_compliance(&[...]) {
            lint.note(&line);
        }

        // 7. DOCUMENTATION LINKS
        lint.note("");
        lint.note("━━━ Documentation ━━━");
        lint.note("");
        for (path, desc) in get_doc_references() {
            lint.note(&format!("• {} ({})", path, desc));
        }
    });
}
```

## Diagnostic Utilities API

### Core Functions

#### `format_speedup(before_name, before_latency, after_name, after_latency, factor)`

Formats a performance comparison with honest metrics.

```rust
let msg = format_speedup(
    "Mutex (lock/unlock)",
    "30-100ns",
    "AtomicU64 (CAS)",
    "<5ns",
    10.0
);
// Output: "Mutex (lock/unlock): 30-100ns | AtomicU64 (CAS): <5ns (10× faster)"
```

#### `format_suggestion(before_code, after_code)`

Shows before/after code transformation with visual markers.

```rust
let suggestion = format_suggestion(
    "lock: Mutex<u64>",
    "lock: AtomicU64"
);
// Output:
// ❌ Before:
//     lock: Mutex<u64>
//
// ✅ After:
//     lock: AtomicU64
```

#### `format_padding_calculation(current_size, alignment, required_padding)`

Shows step-by-step padding math.

```rust
let calc = format_padding_calculation(8, 64, 56);
// Output:
// Calculation:
//     Current size:      8 bytes
//     Alignment:         64 bytes
//     Required padding:  56 bytes
//     Final size:        64 bytes
```

### Visual Diagrams

#### `format_dual_atomic_pattern()`

Returns ASCII art showing DualAtomicU64 bit layout.

#### `format_toctou_explanation()`

Returns visual timeline showing TOCTOU race condition.

#### `format_cache_alignment_benefits()`

Returns before/after comparison of cache performance.

#### `format_memory_ordering_guide()`

Returns quick reference table for atomic ordering.

### Framework Helpers

#### `format_framework_compliance(frameworks: &[(&str, &str)])`

Formats framework compliance checklist.

```rust
for line in format_framework_compliance(&[
    ("Chaos", "100% lockfree mandate"),
    ("UCE34 Q33", "Atomic capsule verification"),
    ("B32", "10-100× proven speedups"),
]) {
    lint.note(&line);
}
// Output:
// Framework Compliance:
//
//   - Chaos: 100% lockfree mandate
//   - UCE34 Q33: Atomic capsule verification
//   - B32: 10-100× proven speedups
```

#### `get_doc_references()`

Returns standard documentation paths.

```rust
for (path, desc) in get_doc_references() {
    lint.note(&format!("• {} ({})", path, desc));
}
// Output:
// • /home/samuel/Docs/The Atomic Capsule.md (DualAtomicU64 pattern)
// • /home/samuel/Primitives/Docs/KEY_INNOVATIONS.md (Proven speedups)
// • /home/samuel/CLAUDE.md (UCE34 framework)
```

## Message Length Guidelines

- **Primary message**: ≤100 characters (visible at a glance)
- **Total message**: ≤500 lines (readable in terminal)
- **Section length**: ≤30 lines (fits on screen)

If a message exceeds 500 lines, consider:
1. Moving detailed explanations to documentation
2. Splitting into multiple related lints
3. Making sections collapsible (future enhancement)

## Testing Error Messages

### Manual Testing

```bash
# Test on intentionally broken code
cargo clippy --all-features -- -D clippy::capsule_mutex_violation

# Verify output includes:
# ✓ Primary message is clear
# ✓ Help section has actionable fix
# ✓ Performance metrics included
# ✓ Documentation links present
# ✓ Visual aids render correctly
```

### Automated Testing

Create test cases in `tests/error_message_quality.rs`:

```rust
#[test]
fn test_mutex_violation_message_quality() {
    let output = compile_fail_test("mutex_in_capsule.rs");

    // Check message contains required elements
    assert!(output.contains("10-100× slowdown"));
    assert!(output.contains("❌ Before:"));
    assert!(output.contains("✅ After:"));
    assert!(output.contains("━━━ Performance Impact ━━━"));
    assert!(output.contains("/home/samuel/Docs/The Atomic Capsule.md"));
}
```

## Common Patterns

### Pattern 1: Performance Degradation

Use when violation causes measurable slowdown:

```
error: [IMPACT] causes N× slowdown: [specific problem]
  |
  | [Before/After code example]
  |
  | ━━━ Performance Impact ━━━
  | [Honest metrics with speedup factor]
  |
  | ━━━ Why This Is Slow ━━━
  | [Technical explanation]
  |
  | ━━━ Faster Alternative ━━━
  | [Recommended solution]
```

### Pattern 2: Correctness Issue

Use when violation risks data races or UB:

```
error: [RACE CONDITION] risk: [specific problem]
  |
  | ━━━ Race Scenario ━━━
  | [Visual timeline showing the race]
  |
  | ━━━ Solution ━━━
  | [Prevention mechanism with code example]
  |
  | ━━━ When to Suppress ━━━
  | [Acceptable exceptions with safety requirements]
```

### Pattern 3: Alignment/Memory Layout

Use for cache alignment or padding issues:

```
error: [CACHE LINE ISSUE]: [specific problem]
  |
  | ━━━ Visual Diagnosis ━━━
  | [ASCII diagram showing the problem]
  |
  | ━━━ Calculation ━━━
  | [Step-by-step math]
  |
  | ━━━ Exact Fix ━━━
  | [Copy-paste ready code]
```

## Anti-Patterns to Avoid

### ❌ Avoid: Technical Jargon Without Explanation

Bad:
```
error: SWeMR violation detected
```

Good:
```
error: Single-Writer-Multiple-Reader (SWeMR) pattern violation
  |
  | SWeMR ensures:
  |   • One writer at a time (exclusive access)
  |   • Many readers simultaneously (shared access)
  |   • No reader/writer conflicts
```

### ❌ Avoid: Vague Suggestions

Bad:
```
help: use a better type
```

Good:
```
help: Replace with lockfree alternative:

❌ Before:
    lock: Mutex<u64>

✅ After:
    lock: AtomicU64
```

### ❌ Avoid: Exaggerated Claims

Bad:
```
AtomicU64 is 1000000× faster!
```

Good:
```
Mutex: 30-100ns | AtomicU64: <5ns (10-100× faster, B32 validated)
```

### ❌ Avoid: Missing Documentation Links

Bad:
```
see documentation for details
```

Good:
```
━━━ Documentation ━━━

• /home/samuel/Docs/The Atomic Capsule.md (DualAtomicU64 pattern)
• /home/samuel/Primitives/Docs/KEY_INNOVATIONS.md (Proven speedups)
```

## Accessibility

### Terminal Compatibility

- Use box-drawing characters (━ ┌ └ ├ │) - supported in all modern terminals
- Provide plain-text fallback in parentheses: `━━━ Section ━━━` vs `--- Section ---`
- Test in: xterm, iTerm2, Windows Terminal, tmux

### Screen Reader Friendly

- Use descriptive section headers
- Avoid ASCII art that doesn't translate to speech
- Provide textual descriptions alongside diagrams

### Color Blindness

- Don't rely solely on color for meaning
- Use symbols (✅ ❌ •) in addition to formatting
- Test with `--color=never` flag

## Version History

- **v2.0 (2025-11-23)**: Enhanced diagnostics with visual aids, honest metrics, framework compliance
- **v1.0 (2025-11-10)**: Initial diagnostic messages (basic functional)

## Contributing

When adding new lints or enhancing messages:

1. Follow the **Message Structure Template**
2. Use **diagnostic utilities** from `src/diagnostics.rs`
3. Include **honest B32 metrics** (validated, reproducible)
4. Add **visual aids** (ASCII art, timelines, diagrams)
5. Link to **documentation** (specific sections, not just paths)
6. Test **message quality** (readability, actionability)
7. Update **BEFORE_AFTER_EXAMPLES.md** with your improvement

## Examples

See `BEFORE_AFTER_EXAMPLES.md` for comprehensive before/after comparisons of all 9 enhanced lints.
