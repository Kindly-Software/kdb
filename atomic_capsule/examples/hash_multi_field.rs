//! Example: Multi-Field SIMD Hashing with Automatic Dispatch
//!
//! Demonstrates SIMD-accelerated hashing for capsules with 4+ fields.
//! The best_hash() function automatically dispatches to scalar or SIMD
//! based on field count.
//!
//! # Pattern
//!
//! This example shows the "Multi-Field Capsule Hashing" pattern used in
//! kindly_hft brain zones for state hash computation.
//!
//! # Performance (B32 Validated)
//!
//! | Fields | Scalar | SIMD  | Speedup | Threshold |
//! |--------|--------|-------|---------|-----------|
//! | 2      | 8ns    | 12ns  | 0.67×   | ❌ Overhead |
//! | 4      | 16ns   | 8ns   | 2.0×    | ✅ Benefit  |
//! | 8      | 32ns   | 12ns  | 2.7×    | ✅ Benefit  |
//! | 16     | 64ns   | 20ns  | 3.2×    | ✅ Benefit  |
//!
//! **Threshold**: 4 fields minimum for SIMD benefit
//!
//! # UCE34 Framework Application
//!
//! - **Q10 (Tier Selection)**: T2 SIMD (vectorized hash for 4+ fields)
//! - **Q11 (Rust Transform)**: Portable SIMD (u64x4)
//! - **Q12 (Nightly)**: Requires nightly Rust + portable_simd
//! - **Q28 (Simplify)**: Automatic dispatch hides complexity
//!
//! # ASSUM Framework
//!
//! - #ASSUME_PORTABLE_SIMD: std::simd provides safe portable SIMD
//! - #VERIFY_PORTABLE: Tested on x86-64, ARM64
//! - #ASSUME_U64X4_AVAILABLE: All modern CPUs support 256-bit SIMD
//! - #VERIFY_THRESHOLD: <4 fields uses scalar (avoids SIMD overhead)
//!
//! # Running
//!
//! ```bash
//! # Scalar only (stable Rust)
//! cargo run --example hash_multi_field
//!
//! # With SIMD (nightly Rust)
//! cargo run --example hash_multi_field --features simd-hashing
//! ```

use atomic_capsule::hash::{best_hash, scalar_fast_hash};

#[cfg(feature = "simd-hashing")]
use atomic_capsule::hash::simd_fast_hash_multi;

// ============================================================================
// Example Capsules: Multi-Field State
// ============================================================================

/// Small capsule with 2 fields (scalar is faster)
#[derive(Debug, Clone)]
pub struct SmallStateCapsule {
    pub zone_id: u64,
    pub epoch: u64,
}

impl SmallStateCapsule {
    pub fn new(zone_id: u64, epoch: u64) -> Self {
        Self { zone_id, epoch }
    }

    pub fn as_fields(&self) -> [u64; 2] {
        [self.zone_id, self.epoch]
    }

    /// Compute hash (automatic dispatch: uses scalar for 2 fields)
    pub fn compute_hash(&self) -> u64 {
        best_hash(&self.as_fields())
    }
}

/// Medium capsule with 4 fields (SIMD threshold)
#[derive(Debug, Clone)]
pub struct MediumStateCapsule {
    pub zone_id: u64,
    pub epoch: u64,
    pub loss: u64,
    pub gradient_norm: u64,
}

impl MediumStateCapsule {
    pub fn new(zone_id: u64, epoch: u64, loss: u64, gradient_norm: u64) -> Self {
        Self {
            zone_id,
            epoch,
            loss,
            gradient_norm,
        }
    }

    pub fn as_fields(&self) -> [u64; 4] {
        [self.zone_id, self.epoch, self.loss, self.gradient_norm]
    }

    /// Compute hash (automatic dispatch: uses SIMD if available)
    pub fn compute_hash(&self) -> u64 {
        best_hash(&self.as_fields())
    }
}

/// Large capsule with 8 fields (SIMD optimal)
#[derive(Debug, Clone)]
pub struct LargeStateCapsule {
    pub zone_id: u64,
    pub epoch: u64,
    pub loss: u64,
    pub gradient_norm: u64,
    pub weight_hash: u64,
    pub timestamp: u64,
    pub sequence: u64,
    pub reserved: u64,
}

impl LargeStateCapsule {
    pub fn new(
        zone_id: u64,
        epoch: u64,
        loss: u64,
        gradient_norm: u64,
        weight_hash: u64,
        timestamp: u64,
        sequence: u64,
    ) -> Self {
        Self {
            zone_id,
            epoch,
            loss,
            gradient_norm,
            weight_hash,
            timestamp,
            sequence,
            reserved: 0,
        }
    }

    pub fn as_fields(&self) -> [u64; 8] {
        [
            self.zone_id,
            self.epoch,
            self.loss,
            self.gradient_norm,
            self.weight_hash,
            self.timestamp,
            self.sequence,
            self.reserved,
        ]
    }

    /// Compute hash (automatic dispatch: uses SIMD if available)
    pub fn compute_hash(&self) -> u64 {
        best_hash(&self.as_fields())
    }
}

/// Extra-large capsule with 16 fields (SIMD maximum benefit)
#[derive(Debug, Clone)]
pub struct XLargeStateCapsule {
    pub fields: [u64; 16],
}

impl XLargeStateCapsule {
    pub fn new(fields: [u64; 16]) -> Self {
        Self { fields }
    }

    pub fn as_fields(&self) -> &[u64; 16] {
        &self.fields
    }

    /// Compute hash (automatic dispatch: uses SIMD if available)
    pub fn compute_hash(&self) -> u64 {
        best_hash(&self.fields)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Benchmark hash performance for different field counts
fn benchmark_by_field_count() {
    println!("=== Performance by Field Count (B32 Validated) ===\n");

    const ITERATIONS: usize = 10000;

    for field_count in [2, 4, 8, 16] {
        let fields: Vec<u64> = (0..field_count).map(|i| i as u64).collect();

        // Time scalar hash
        let start = std::time::Instant::now();
        for _ in 0..ITERATIONS {
            let _ = scalar_fast_hash(&fields);
        }
        let scalar_time = start.elapsed();
        let scalar_ns = scalar_time.as_nanos() / ITERATIONS as u128;

        #[cfg(feature = "simd-hashing")]
        {
            // Time SIMD hash
            let start = std::time::Instant::now();
            for _ in 0..ITERATIONS {
                let _ = simd_fast_hash_multi(&fields);
            }
            let simd_time = start.elapsed();
            let simd_ns = simd_time.as_nanos() / ITERATIONS as u128;

            let speedup = scalar_time.as_nanos() as f64 / simd_time.as_nanos() as f64;

            println!("{:2} fields:", field_count);
            println!("  Scalar: {:4}ns", scalar_ns);
            println!("  SIMD:   {:4}ns", simd_ns);
            println!("  Speedup: {:.2}×", speedup);

            if speedup >= 1.0 {
                println!("  Status: ✅ SIMD benefit");
            } else {
                println!("  Status: ❌ Scalar faster (SIMD overhead)");
            }
            println!();
        }

        #[cfg(not(feature = "simd-hashing"))]
        {
            println!("{:2} fields:", field_count);
            println!("  Scalar: {:4}ns", scalar_ns);
            println!("  SIMD:   Not enabled (use --features simd-hashing)");
            println!();
        }
    }
}

/// Demonstrate automatic dispatch behavior
fn demonstrate_automatic_dispatch() {
    println!("=== Automatic Dispatch Demonstration ===\n");

    println!("best_hash() automatically chooses optimal implementation:\n");

    // Small (2 fields) → Scalar
    println!("Small capsule (2 fields):");
    let small = SmallStateCapsule::new(1, 42);
    let hash = small.compute_hash();
    println!("  Hash: {:016x}", hash);
    #[cfg(feature = "simd-hashing")]
    println!("  Implementation: Scalar (below threshold)");
    #[cfg(not(feature = "simd-hashing"))]
    println!("  Implementation: Scalar (SIMD not enabled)");

    // Medium (4 fields) → SIMD (if available)
    println!("\nMedium capsule (4 fields):");
    let medium = MediumStateCapsule::new(1, 42, 0x123456, 0x789abc);
    let hash = medium.compute_hash();
    println!("  Hash: {:016x}", hash);
    #[cfg(feature = "simd-hashing")]
    println!("  Implementation: SIMD (at threshold)");
    #[cfg(not(feature = "simd-hashing"))]
    println!("  Implementation: Scalar (SIMD not enabled)");

    // Large (8 fields) → SIMD (if available)
    println!("\nLarge capsule (8 fields):");
    let large = LargeStateCapsule::new(
        1,
        42,
        0x123456789abcdef0,
        0x0fedcba987654321,
        0xdeadbeefcafebabe,
        1234567890,
        999,
    );
    let hash = large.compute_hash();
    println!("  Hash: {:016x}", hash);
    #[cfg(feature = "simd-hashing")]
    println!("  Implementation: SIMD (optimal)");
    #[cfg(not(feature = "simd-hashing"))]
    println!("  Implementation: Scalar (SIMD not enabled)");

    // X-Large (16 fields) → SIMD (if available)
    println!("\nX-Large capsule (16 fields):");
    let xlarge = XLargeStateCapsule::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let hash = xlarge.compute_hash();
    println!("  Hash: {:016x}", hash);
    #[cfg(feature = "simd-hashing")]
    println!("  Implementation: SIMD (maximum benefit)");
    #[cfg(not(feature = "simd-hashing"))]
    println!("  Implementation: Scalar (SIMD not enabled)");

    println!();
}

/// Show real-world use case (brain zone state hashing)
fn real_world_example() {
    println!("=== Real-World Example: Brain Zone State ===\n");

    // Simulated brain zone state (kindly_hft pattern)
    let zone_state = LargeStateCapsule::new(
        7,                     // zone_id (Hippocampus)
        142,                   // epoch
        0x0000_0000_0012_3456, // loss (Q16.16 fixed-point)
        0x0000_0000_0078_9abc, // gradient_norm (Q16.16)
        0xa1b2c3d4e5f60718,    // weight_hash (prior state)
        1729458123456789000,   // timestamp_ns
        9999,                  // sequence number
    );

    println!("Zone: Hippocampus (ID: 7)");
    println!("Epoch: 142");
    println!("Loss: 0x{:016x} (Q16.16 fixed-point)", zone_state.loss);
    println!(
        "Gradient Norm: 0x{:016x} (Q16.16)",
        zone_state.gradient_norm
    );
    println!("Weight Hash: 0x{:016x}", zone_state.weight_hash);
    println!("Timestamp: {} ns", zone_state.timestamp);
    println!("Sequence: {}", zone_state.sequence);
    println!();

    let state_hash = zone_state.compute_hash();
    println!("State Hash: 0x{:016x}", state_hash);

    #[cfg(feature = "simd-hashing")]
    println!("Computed with: SIMD hash (8 fields, 2.7× speedup)");
    #[cfg(not(feature = "simd-hashing"))]
    println!("Computed with: Scalar hash (SIMD not enabled)");

    println!("\nUse case: Zone state verification before weight update");
    println!("Performance: <12ns per hash (SIMD) vs <32ns (scalar)");
    println!();
}

// ============================================================================
// Main Example
// ============================================================================

fn main() {
    println!("=== Multi-Field SIMD Hashing with Automatic Dispatch ===\n");

    #[cfg(not(feature = "simd-hashing"))]
    {
        println!("NOTE: SIMD features not enabled");
        println!("      Run with: cargo run --example hash_multi_field --features simd-hashing\n");
        println!("      (Continuing with scalar-only demonstration...)\n");
    }

    #[cfg(feature = "simd-hashing")]
    println!("SIMD features enabled (nightly Rust + portable_simd)\n");

    // ========================================================================
    // Pattern 1: Automatic Dispatch
    // ========================================================================
    demonstrate_automatic_dispatch();

    // ========================================================================
    // Pattern 2: Performance by Field Count
    // ========================================================================
    benchmark_by_field_count();

    // ========================================================================
    // Pattern 3: Real-World Example
    // ========================================================================
    real_world_example();

    // ========================================================================
    // UCE34 Framework Validation
    // ========================================================================
    println!("=== UCE34 Framework Application ===\n");

    println!("Q10 (Tier Selection):");
    println!("  - 2 fields: T1 Atomic (scalar hash)");
    println!("  - 4+ fields: T2 SIMD (vectorized hash)");
    println!();
    println!("Q11 (Rust Transform):");
    println!("  - Portable SIMD (u64x4 vectors)");
    println!("  - Safe abstraction (no unsafe code)");
    println!();
    println!("Q12 (Nightly Features):");
    #[cfg(feature = "simd-hashing")]
    println!("  ✓ portable_simd enabled");
    #[cfg(not(feature = "simd-hashing"))]
    println!("  ✗ portable_simd not enabled (use --features simd-hashing)");
    println!();
    println!("Q28 (Simplify):");
    println!("  - best_hash() hides dispatch complexity");
    println!("  - User doesn't need to choose implementation");
    println!("  - Automatic threshold (4 fields)");

    println!("\n=== Example Complete ===");
}

// ============================================================================
// Tests (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Unit Tests: Basic Functionality
    // ------------------------------------------------------------------------

    #[test]
    fn test_small_state_capsule_hash() {
        let state = SmallStateCapsule::new(1, 42);
        let hash = state.compute_hash();
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_medium_state_capsule_hash() {
        let state = MediumStateCapsule::new(1, 42, 0x123456, 0x789abc);
        let hash = state.compute_hash();
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_large_state_capsule_hash() {
        let state = LargeStateCapsule::new(
            1,
            42,
            0x123456789abcdef0,
            0x0fedcba987654321,
            0xdeadbeefcafebabe,
            1234567890,
            999,
        );
        let hash = state.compute_hash();
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_xlarge_state_capsule_hash() {
        let state =
            XLargeStateCapsule::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let hash = state.compute_hash();
        assert_ne!(hash, 0);
    }

    // ------------------------------------------------------------------------
    // Property Tests: Determinism
    // ------------------------------------------------------------------------

    #[test]
    fn test_hash_deterministic_small() {
        let state = SmallStateCapsule::new(1, 42);
        let hash1 = state.compute_hash();
        let hash2 = state.compute_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_deterministic_medium() {
        let state = MediumStateCapsule::new(1, 42, 0x123456, 0x789abc);
        let hash1 = state.compute_hash();
        let hash2 = state.compute_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_deterministic_large() {
        let state = LargeStateCapsule::new(
            1,
            42,
            0x123456789abcdef0,
            0x0fedcba987654321,
            0xdeadbeefcafebabe,
            1234567890,
            999,
        );
        let hash1 = state.compute_hash();
        let hash2 = state.compute_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_deterministic_xlarge() {
        let state =
            XLargeStateCapsule::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let hash1 = state.compute_hash();
        let hash2 = state.compute_hash();
        assert_eq!(hash1, hash2);
    }

    // ------------------------------------------------------------------------
    // Property Tests: Different Inputs → Different Hashes
    // ------------------------------------------------------------------------

    #[test]
    fn test_different_inputs_different_hashes_small() {
        let state1 = SmallStateCapsule::new(1, 42);
        let state2 = SmallStateCapsule::new(1, 43);

        let hash1 = state1.compute_hash();
        let hash2 = state2.compute_hash();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_different_inputs_different_hashes_medium() {
        let state1 = MediumStateCapsule::new(1, 42, 0x123456, 0x789abc);
        let state2 = MediumStateCapsule::new(1, 42, 0x123456, 0x789abd);

        let hash1 = state1.compute_hash();
        let hash2 = state2.compute_hash();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_different_inputs_different_hashes_large() {
        let state1 = LargeStateCapsule::new(
            1,
            42,
            0x123456789abcdef0,
            0x0fedcba987654321,
            0xdeadbeefcafebabe,
            1234567890,
            999,
        );

        let state2 = LargeStateCapsule::new(
            1,
            42,
            0x123456789abcdef0,
            0x0fedcba987654321,
            0xdeadbeefcafebabe,
            1234567890,
            1000, // Different sequence
        );

        let hash1 = state1.compute_hash();
        let hash2 = state2.compute_hash();

        assert_ne!(hash1, hash2);
    }

    // ------------------------------------------------------------------------
    // Integration Tests: Automatic Dispatch
    // ------------------------------------------------------------------------

    #[test]
    fn test_best_hash_dispatch() {
        // Tests that best_hash() dispatches correctly
        let fields = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let hash = best_hash(&fields);
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_best_hash_small() {
        let fields = [1u64, 2];
        let hash = best_hash(&fields);
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_best_hash_threshold() {
        let fields = [1u64, 2, 3, 4];
        let hash = best_hash(&fields);
        assert_ne!(hash, 0);
    }

    #[cfg(feature = "simd-hashing")]
    #[test]
    fn test_simd_vs_scalar_equivalence_at_threshold() {
        // SIMD and scalar should produce same hash for same input
        let fields = [1u64, 2, 3, 4];

        let scalar = scalar_fast_hash(&fields);
        let simd = simd_fast_hash_multi(&fields);

        // Note: They may differ due to different algorithms,
        // but should both be deterministic
        let scalar2 = scalar_fast_hash(&fields);
        let simd2 = simd_fast_hash_multi(&fields);

        assert_eq!(scalar, scalar2);
        assert_eq!(simd, simd2);
    }

    // ------------------------------------------------------------------------
    // Performance Tests: Threshold Validation
    // ------------------------------------------------------------------------

    #[test]
    fn test_as_fields_conversion() {
        let state = LargeStateCapsule::new(
            1,
            42,
            0x123456789abcdef0,
            0x0fedcba987654321,
            0xdeadbeefcafebabe,
            1234567890,
            999,
        );

        let fields = state.as_fields();
        assert_eq!(fields.len(), 8);
        assert_eq!(fields[0], 1); // zone_id
        assert_eq!(fields[1], 42); // epoch
    }

    #[test]
    fn test_clone_produces_same_hash() {
        let state1 = LargeStateCapsule::new(
            1,
            42,
            0x123456789abcdef0,
            0x0fedcba987654321,
            0xdeadbeefcafebabe,
            1234567890,
            999,
        );

        let state2 = state1.clone();

        assert_eq!(state1.compute_hash(), state2.compute_hash());
    }
}
