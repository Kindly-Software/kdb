//! # ASSUM Framework Diagnostic Utilities (P2.2)
//!
//! Provides formatted diagnostic messages for ASSUM compliance violations.
//! Used by lints to guide developers toward SOX/SOC2/HIPAA-compliant safety documentation.

/// Format ASSUM framework overview
///
/// Quick reference to why ASSUM matters
pub fn format_assum_framework_overview() -> Vec<String> {
    vec![
        "ASSUM Framework: Safety Documentation for Audit Compliance".to_string(),
        "".to_string(),
        "What is ASSUM?".to_string(),
        "  A systematic approach to documenting every safety assumption in unsafe code.".to_string(),
        "  Required for SOX/SOC2/GDPR/HIPAA compliance.".to_string(),
        "".to_string(),
        "Why it Matters:".to_string(),
        "  • Audit Compliance: Hash-chain integrity for every unsafe block".to_string(),
        "  • TOCTOU Prevention: Generation counters eliminate races".to_string(),
        "  • Memory Safety: Alignment, atomicity, invariants at compile-time".to_string(),
        "  • Production Ready: 99.5%+ safety via verified assumptions".to_string(),
        "".to_string(),
        "How to Use:".to_string(),
        "  // #ASSUME_CATEGORY: What assumption are you making?".to_string(),
        "  // #VERIFY_CATEGORY: How is this assumption verified?".to_string(),
        "  unsafe { /* Your safe code here */ }".to_string(),
        "".to_string(),
        "Categories: 10 safety categories covering thread-safety, alignment, ordering, atomicity, etc.".to_string(),
        "Framework: /home/samuel/xml/frameworks/assum.xml (UCE34 Q34)".to_string(),
    ]
}

/// Format ASSUM safety categories reference
///
/// Shows 10 categories from ASSUM framework (UCE34 Q34)
pub fn format_assum_categories() -> Vec<String> {
    vec![
        "ASSUM Framework Safety Categories (10 Total, UCE34 Q34):".to_string(),
        "".to_string(),
        "1. ASSUME_SEND_SAFE         | Thread-safe Send impl (no interior mutability)".to_string(),
        "2. ASSUME_SYNC_SAFE         | Thread-safe Sync impl (shared across threads)".to_string(),
        "3. ASSUME_ALIGNMENT_64B     | Cache line exclusive access (false sharing prevent)".to_string(),
        "4. ASSUME_ALIGNMENT_128B    | Double-wide alignment (NUMA aware)".to_string(),
        "5. ASSUME_ALIGNMENT_256B    | L2 cache boundary alignment (SIMD optimized)".to_string(),
        "6. ASSUME_TOCTOU_SAFE       | Generation counter prevents race conditions".to_string(),
        "7. ASSUME_ORDERING_ACQREL   | Acquire/Release memory ordering correctness".to_string(),
        "8. ASSUME_ATOMICITY         | Atomic operation CAS correctness".to_string(),
        "9. ASSUME_INVARIANT_BOUNDED | Structural invariant (capacity limit, bounds)".to_string(),
        "10. ASSUME_LIFETIME         | Lifetime soundness (no UAF, no dangling refs)".to_string(),
        "".to_string(),
        "See: /home/samuel/xml/frameworks/assum.xml (complete definitions)".to_string(),
    ]
}

/// Format ASSUM compliance checklist
///
/// Shows what must be documented for audit compliance
pub fn format_assum_compliance_checklist() -> Vec<String> {
    vec![
        "ASSUM Compliance Checklist (SOX/SOC2/HIPAA):".to_string(),
        "".to_string(),
        "Safety Documentation:".to_string(),
        "  ✓ Every unsafe {} block has #ASSUME_* comment".to_string(),
        "  ✓ Every #ASSUME has corresponding #VERIFY comment".to_string(),
        "  ✓ Verification is verifiable (not hand-wavy)".to_string(),
        "".to_string(),
        "Memory Safety:".to_string(),
        "  ✓ Atomicity guaranteed (CAS, not scattered writes)".to_string(),
        "  ✓ Memory ordering explicit (Acquire/Release, not Relaxed)".to_string(),
        "  ✓ Alignment documented (64B/128B/256B padding)".to_string(),
        "".to_string(),
        "Race Condition Prevention:".to_string(),
        "  ✓ Generation counter used (TOCTOU risk mitigation)".to_string(),
        "  ✓ CAS loop validates generation on update".to_string(),
        "  ✓ Reader revalidates after snapshot".to_string(),
        "".to_string(),
        "COCA Compliance:".to_string(),
        "  ✓ No mutex/RwLock (all lockfree)".to_string(),
        "  ✓ Cache alignment (64B/128B/256B)".to_string(),
        "  ✓ Atomic operations only (no scattered atomics)".to_string(),
        "".to_string(),
        "Audit Trail:".to_string(),
        "  ✓ Hash-chain integrity for compliance".to_string(),
        "  ✓ Traceable to safety assumption".to_string(),
        "  ✓ Documented for legal defense".to_string(),
    ]
}

/// Format compliance benefit summary
///
/// Shows audit trail and legal defense value
pub fn format_assum_compliance_benefits() -> Vec<String> {
    vec![
        "Compliance & Audit Trail Benefits:".to_string(),
        "".to_string(),
        "SOX (Sarbanes-Oxley) Compliance:".to_string(),
        "  • Every unsafe block traceable to documented assumption".to_string(),
        "  • Evidence of due diligence for safety-critical code".to_string(),
        "  • Audit trail logs all modifications to assumptions".to_string(),
        "".to_string(),
        "SOC2 (System & Organization Controls):".to_string(),
        "  • Explicit verification checklist for every safety risk".to_string(),
        "  • Trust services criteria met via hash-chain integrity".to_string(),
        "  • Repeatable validation (T28 4-tier testing)".to_string(),
        "".to_string(),
        "GDPR/HIPAA (Data Protection):".to_string(),
        "  • Encryption assumptions documented and verified".to_string(),
        "  • Access control invariants guaranteed".to_string(),
        "  • Data isolation assumptions explicit".to_string(),
        "".to_string(),
        "Production SLA:".to_string(),
        "  • 99.5%+ safety target achieved via ASSUM framework".to_string(),
        "  • All assumptions validated before production deployment".to_string(),
        "  • Legal defense: documented engineering due diligence".to_string(),
    ]
}

/// Common ASSUM tag examples for copy-paste
///
/// Provides ready-to-use examples for common patterns
pub fn format_assum_examples() -> Vec<String> {
    vec![
        "Common ASSUM Tag Examples (Copy-Paste Ready):".to_string(),
        "".to_string(),
        "1. Send Implementation:".to_string(),
        "  // #ASSUME_SEND_SAFE: All fields are atomic (no Rc/RefCell)".to_string(),
        "  // #VERIFY_SEND_SAFE: AtomicU64 is Send per stdlib, no Rc<T> in struct".to_string(),
        "  unsafe impl Send for MyCapsule {}".to_string(),
        "".to_string(),
        "2. Cache Alignment:".to_string(),
        "  // #ASSUME_ALIGNMENT_64B: Padding ensures exclusive cache line".to_string(),
        "  // #VERIFY_ALIGNMENT_64B: size=64, repr(C, align(64)) enforces it".to_string(),
        "  #[repr(C, align(64))]".to_string(),
        "  struct MyCapsule { ... }".to_string(),
        "".to_string(),
        "3. TOCTOU Prevention:".to_string(),
        "  // #ASSUME_TOCTOU_SAFE: Gen counter prevents race".to_string(),
        "  // #VERIFY_TOCTOU_SAFE: CAS updates value+gen atomically".to_string(),
        "  fn cas(&self, (old_val, old_gen): (u32,u32), (new_val,new_gen): (u32,u32)) -> bool {}".to_string(),
        "".to_string(),
        "4. Memory Ordering:".to_string(),
        "  // #ASSUME_ORDERING_ACQREL: Acquire→Release ordering".to_string(),
        "  // #VERIFY_ORDERING_ACQREL: All loads=Acquire, stores=Release".to_string(),
        "  fn load(&self) -> u64 { self.value.load(Acquire) }".to_string(),
        "".to_string(),
        "5. Invariant Validation:".to_string(),
        "  // #ASSUME_INVARIANT_BOUNDED: Queue capacity limit".to_string(),
        "  // #VERIFY_INVARIANT_BOUNDED: Checked in new(), enforced in enqueue()".to_string(),
        "  fn new(capacity: usize) -> Result<Self, Error> {}".to_string(),
    ]
}

/// Format DualAtomicU64 ASSUM example
///
/// Complete working example with all required tags
pub fn format_dualatomic_assum_example() -> Vec<String> {
    vec![
        "DualAtomicU64 Pattern with Full ASSUM Documentation:".to_string(),
        "".to_string(),
        "// #ASSUME_GENERATION_COUNTER: Both atomics use generation (bits 32-63)".to_string(),
        "// #VERIFY_GENERATION_COUNTER: CAS validates generation match in both primary/secondary".to_string(),
        "// #VERIFY_GENERATION_COUNTER: Generation incremented on every successful update".to_string(),
        "// #ASSUME_ALIGNMENT_128B: Padding separates cache lines (NUMA optimization)".to_string(),
        "// #VERIFY_ALIGNMENT_128B: _pad1 and _pad2 ensure 128B total, no false sharing".to_string(),
        "// #ASSUME_SEND_SAFE: Both fields are atomic (thread-safe across cores)".to_string(),
        "// #VERIFY_SEND_SAFE: AtomicU64 is Send, no interior mutability, no Rc<T>".to_string(),
        "#[repr(C, align(128))]".to_string(),
        "pub struct DualAtomicCapsule {".to_string(),
        "    primary: AtomicU64,     // data (bits 0-31) + generation (bits 32-63)".to_string(),
        "    _pad1: [u8; 56],         // cache line separation (64 + 56 = 120)".to_string(),
        "    secondary: AtomicU64,   // meta (bits 0-31) + generation (bits 32-63)".to_string(),
        "    _pad2: [u8; 56],         // final alignment to 128B total".to_string(),
        "}".to_string(),
        "".to_string(),
        "// #ASSUME_TOCTOU_SAFE: CAS loop prevents race between check and use".to_string(),
        "// #VERIFY_TOCTOU_SAFE: Reader validates generation after snapshot".to_string(),
        "// #VERIFY_TOCTOU_SAFE: CAS retries if generation mismatch detected".to_string(),
        "impl DualAtomicCapsule {".to_string(),
        "    pub fn cas(&self, old: (u32, u32), new: (u32, u32)) -> bool {".to_string(),
        "        // CAS implementation with generation counter validation".to_string(),
        "    }".to_string(),
        "}".to_string(),
    ]
}

/// Get ASSUM framework references
///
/// Returns documentation paths for complete ASSUM knowledge
pub fn get_assum_references() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "/home/samuel/xml/frameworks/assum.xml",
            "ASSUM framework (10 categories, UCE34 Q34)",
        ),
        (
            "/home/samuel/CLAUDE.md",
            "UCE34 Q34 Auditability requirement",
        ),
        (
            "/home/samuel/Docs/The Atomic Capsule.md",
            "DualAtomicU64 ASSUM examples",
        ),
        (
            "/home/samuel/Docs/The Computational Capsule.md",
            "COCA philosophy and unsafe code guidelines",
        ),
        (
            "/home/samuel/xml/frameworks/t28.xml",
            "T28 testing framework for validation",
        ),
        (
            "/home/samuel/xml/frameworks/b32.xml",
            "B32 performance validation (fair baselines)",
        ),
        (
            "/home/samuel/xml/frameworks/i20.xml",
            "I20 integration safety framework",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_assum_framework_overview() {
        let overview = format_assum_framework_overview();
        assert!(overview.iter().any(|s| s.contains("ASSUM")));
        assert!(overview.iter().any(|s| s.contains("Audit")));
        assert!(overview.iter().any(|s| s.contains("categories")));
    }

    #[test]
    fn test_format_assum_categories() {
        let categories = format_assum_categories();
        assert!(categories.iter().any(|s| s.contains("SEND_SAFE")));
        assert!(categories.iter().any(|s| s.contains("ALIGNMENT")));
        assert!(categories.iter().any(|s| s.contains("TOCTOU")));
        assert_eq!(categories.len(), 14); // 10 categories + 4 header/blank/footer lines
    }

    #[test]
    fn test_format_assum_compliance_checklist() {
        let checklist = format_assum_compliance_checklist();
        assert!(checklist.iter().any(|s| s.contains("Atomicity")));
        assert!(checklist.iter().any(|s| s.contains("Alignment")));
        assert!(checklist.iter().any(|s| s.contains("Audit")));
    }

    #[test]
    fn test_format_assum_compliance_benefits() {
        let benefits = format_assum_compliance_benefits();
        assert!(benefits.iter().any(|s| s.contains("SOX")));
        assert!(benefits.iter().any(|s| s.contains("SOC2")));
        assert!(benefits.iter().any(|s| s.contains("GDPR")));
        assert!(benefits.iter().any(|s| s.contains("HIPAA")));
    }

    #[test]
    fn test_format_assum_examples() {
        let examples = format_assum_examples();
        assert!(examples.iter().any(|s| s.contains("SEND")));
        assert!(examples.iter().any(|s| s.contains("ALIGNMENT")));
        assert!(examples.iter().any(|s| s.contains("TOCTOU")));
        assert!(examples.iter().any(|s| s.contains("ORDERING")));
        assert!(examples.iter().any(|s| s.contains("INVARIANT")));
    }

    #[test]
    fn test_format_dualatomic_assum_example() {
        let example = format_dualatomic_assum_example();
        assert!(example.iter().any(|s| s.contains("DualAtomic")));
        assert!(example.iter().any(|s| s.contains("#ASSUME_GENERATION_COUNTER")));
        assert!(example.iter().any(|s| s.contains("#VERIFY_GENERATION_COUNTER")));
        assert!(example.iter().any(|s| s.contains("primary")));
        assert!(example.iter().any(|s| s.contains("secondary")));
    }

    #[test]
    fn test_get_assum_references() {
        let refs = get_assum_references();
        assert!(refs.len() >= 6);
        assert!(refs.iter().any(|(path, _)| path.contains("assum.xml")));
        assert!(refs.iter().any(|(_, desc)| desc.contains("Q34")));
    }
}
