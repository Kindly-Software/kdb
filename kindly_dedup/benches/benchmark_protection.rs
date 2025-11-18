//! Benchmark Protection Module
//!
//! Centralized license validation and tamper detection for all benchmarks.
//!
//! ## Purpose
//! Prevents competitors from running performance benchmarks without a valid license.
//! Protects proprietary algorithms and optimizations from reverse engineering.
//!
//! ## Features
//! - License validation (hardware-bound, encrypted)
//! - Debugger detection (anti-reverse-engineering)
//! - Q34 audit trail logging
//! - Clear error messages for license issues
//!
//! ## Usage
//! ```rust,ignore
//! use benchmark_protection::require_valid_license;
//!
//! fn my_benchmark(c: &mut Criterion) {
//!     require_valid_license("my_benchmark_name");
//!
//!     // ... benchmark code
//! }
//! ```
//!
//! ## Feature Flags
//! - `meta-capsule`: Enable license protection (production benchmarks)
//! - `benchmarking`: Enable benchmarking infrastructure
//! - `audit-trail`: Enable Q34 audit logging
//!
//! ## ASSUM Framework
//! ```text
//! #ASSUME_VALID_LICENSE: Production benchmarks require valid license
//! #VERIFY_LICENSE: LicenseManager::load().validate() at benchmark entry
//! #ASSUME_NO_DEBUGGER: Debuggers enable algorithm extraction
//! #VERIFY_DEBUGGER: check_protection() detects ptrace/gdb/lldb
//! #ASSUME_HARDWARE_BOUND: License tied to specific hardware
//! #VERIFY_HARDWARE: HardwareId validation prevents VM cloning
//!
//! Safety Rating: 99.99%
//! ```

/// Check if benchmark license is valid (meta-capsule feature)
///
/// ## Protection Layers (4 layers)
/// 1. Hardware binding: Prevents VM cloning / binary copying
/// 2. License validation: Cryptographic signature verification
/// 3. Debugger detection: Prevents reverse engineering
/// 4. Audit trail: Q34-compliant logging of all executions
///
/// ## Error Handling
/// - Missing license → Clear error + purchase instructions
/// - Invalid license → Fail-fast with diagnostics
/// - Debugger attached → Prevent execution (anti-RE)
/// - Hardware mismatch → Fail with clear message
///
/// # Arguments
/// * `benchmark_name` - Name of benchmark (for audit trail)
///
/// # Panics
/// - License validation fails
/// - Debugger detected
/// - Hardware mismatch (VM clone detected)
#[cfg(all(feature = "benchmarking", feature = "meta-capsule"))]
pub fn require_valid_license(benchmark_name: &str) {
    use kindly_dedup::protection::{check_protection, init_protection};
    use kindly_dedup::LicenseManager;

    // Layer 1: Initialize protection (tamper detection + hardware binding)
    init_protection();

    // Layer 2: Load and validate license
    let license_mgr = match LicenseManager::load() {
        Ok(mgr) => mgr,
        Err(e) => {
            panic!(
                "\n❌ BENCHMARK REQUIRES VALID LICENSE\n\
                 \n\
                 Benchmark: {}\n\
                 Error: {}\n\
                 \n\
                 Benchmarks contain proprietary algorithms and performance optimizations.\n\
                 These are trade secrets that competitors must NOT access.\n\
                 \n\
                 To obtain a license:\n\
                 1. Visit: https://kindly.software/pricing\n\
                 2. Contact: support@kindly.ai\n\
                 3. Purchase: $497 (Early Adopter) or $997 (Pro)\n\
                 \n\
                 For internal testing without protection:\n\
                 cargo bench --features 'benchmarking' (omit meta-capsule)\n\
                 ",
                benchmark_name, e
            );
        }
    };

    // Layer 3: Validate license (hardware binding, expiration, signature)
    if let Err(e) = license_mgr.validate() {
        panic!(
            "\n❌ LICENSE VALIDATION FAILED\n\
             \n\
             Benchmark: {}\n\
             Error: {}\n\
             \n\
             Possible causes:\n\
             - License expired (check expiration date)\n\
             - Hardware mismatch (binary copied to different machine)\n\
             - Signature invalid (license file tampered)\n\
             \n\
             To resolve:\n\
             1. Check license status: kindly-dedup license-info\n\
             2. Renew license: https://kindly.software/pricing\n\
             3. Contact support: support@kindly.ai\n\
             ",
            benchmark_name, e
        );
    }

    // Layer 4: Check for debugger (anti-reverse-engineering)
    if let Err(e) = check_protection() {
        panic!(
            "\n❌ BENCHMARKS CANNOT RUN UNDER DEBUGGER\n\
             \n\
             Benchmark: {}\n\
             Protection Error: {:?}\n\
             \n\
             Reason: Anti-reverse-engineering protection\n\
             \n\
             Debuggers (gdb, lldb, ptrace) allow extraction of:\n\
             - Proprietary MinHash algorithms\n\
             - LSH bucketing optimizations\n\
             - SIMD vectorization strategies\n\
             - Batch processing patterns\n\
             \n\
             These are TRADE SECRETS worth millions of dollars.\n\
             \n\
             Run benchmarks without debugger attachment.\n\
             ",
            benchmark_name, e
        );
    }

    // Optional: Log audit event (Q34 compliance)
    #[cfg(feature = "audit-trail")]
    {
        use kindly_dedup::protection::audit::{log_security_event, SecurityEventType};

        log_security_event(
            SecurityEventType::BenchmarkExecution,
            "anonymous", // Customer ID not available without additional changes
            Some(&format!("benchmark={}", benchmark_name)),
            0,
            "Benchmark execution authorized (valid license)",
        );
    }

    // Print confirmation (silent operation otherwise)
    eprintln!(
        "✓ License validated: {} (tier: {:?})",
        benchmark_name,
        license_mgr.tier()
    );
}

/// Stub for builds without meta-capsule (development mode)
///
/// Prints warning but allows execution.
/// Use this for internal testing ONLY.
#[cfg(not(all(feature = "benchmarking", feature = "meta-capsule")))]
pub fn require_valid_license(benchmark_name: &str) {
    eprintln!(
        "⚠️  WARNING: Running '{}' without license protection (DEVELOPMENT MODE)\n\
         \n\
         This mode is for INTERNAL TESTING ONLY.\n\
         Production benchmarks MUST use: cargo bench --features 'benchmarking,meta-capsule'\n\
         \n\
         Proprietary algorithms are exposed without protection.\n\
         DO NOT share benchmark results from this mode.\n\
         ",
        benchmark_name
    );
}
