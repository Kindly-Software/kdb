//! T28 Unit Tests for CNLSRuleCapsule (Tier 1 Atomic + Tier 3 Fixed-Point + Q34)
//!
//! **Source**: Migrated from planck-universe/src/physics/cnls_rule.rs (lines 448-589)
//! **Total**: 15 tests (unit tests for CNLSRuleCapsule)
//!
//! **Framework Compliance**:
//! - T28 Q1-Q7 (Unit): Initialization, atomic operations, Q34 hash chain
//!
//! **Test Coverage**:
//! - Initialization: new, default, load_params
//! - Atomic tracking: energy, phase coherence, generation counter
//! - Q34 hash chain: current_hash, prev_hash, update_hash_chain
//! - Capsule properties: Alignment, size verification

#![cfg(feature = "cnls")]

use atomic_capsule::patterns::cnls::CNLSRuleCapsule;

// ========================================
// T28 Q1-Q7: Unit Tests (15 tests)
// ========================================

#[test]
fn test_cnls_rule_capsule_initialization() {
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);
    let (hbar, g, dt, dx) = rule.load_params();
    assert!((hbar - 1.0).abs() < 1e-10);
    assert!((g - 1.0).abs() < 1e-10);
    assert!((dt - 0.01).abs() < 1e-10);
    assert!((dx - 1.0).abs() < 1e-10);
}

#[test]
fn test_cnls_rule_default() {
    let rule = CNLSRuleCapsule::default();
    let (hbar, g, dt, dx) = rule.load_params();
    assert!((hbar - 1.0).abs() < 1e-10);
    assert!((g - 1.0).abs() < 1e-10);
    assert!((dt - 0.01).abs() < 1e-10);
    assert!((dx - 1.0).abs() < 1e-10);
}

#[test]
fn test_cnls_rule_individual_getters() {
    let rule = CNLSRuleCapsule::new(2.0, 3.0, 0.02, 0.5);

    assert!((rule.hbar_over_2m() - 2.0).abs() < 1e-10);
    assert!((rule.coupling_g() - 3.0).abs() < 1e-10);
    assert!((rule.dt() - 0.02).abs() < 1e-10);
    assert!((rule.dx() - 0.5).abs() < 1e-10);
}

#[test]
fn test_cnls_energy_tracking() {
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    assert_eq!(rule.total_energy(), 0.0);

    rule.update_energy(123.456);
    assert!((rule.total_energy() - 123.456).abs() < 1e-6);
}

#[test]
fn test_cnls_phase_coherence_tracking() {
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    assert_eq!(rule.phase_coherence(), 0.0);

    rule.update_phase_coherence(0.85);
    assert!((rule.phase_coherence() - 0.85).abs() < 1e-6);
}

#[test]
fn test_cnls_generation_counter() {
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    assert_eq!(rule.generation(), 0);

    rule.next_generation();
    assert_eq!(rule.generation(), 1);

    rule.next_generation();
    assert_eq!(rule.generation(), 2);
}

#[test]
fn test_cnls_hash_chain() {
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    assert_eq!(rule.current_hash(), 0);
    assert_eq!(rule.prev_hash(), 0);

    rule.update_hash_chain(12345);
    assert_eq!(rule.current_hash(), 12345);
    assert_eq!(rule.prev_hash(), 0);

    rule.update_hash_chain(67890);
    assert_eq!(rule.current_hash(), 67890);
    assert_eq!(rule.prev_hash(), 12345);
}

#[test]
fn test_cnls_rule_capsule_alignment() {
    assert_eq!(std::mem::align_of::<CNLSRuleCapsule>(), 128);
    assert_eq!(std::mem::size_of::<CNLSRuleCapsule>(), 128);
}

#[test]
fn test_cnls_energy_update_multiple_times() {
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    rule.update_energy(100.0);
    rule.update_energy(200.0);
    rule.update_energy(150.0);

    assert!((rule.total_energy() - 150.0).abs() < 1e-6);
}

#[test]
fn test_cnls_phase_coherence_bounds() {
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    // Test boundary values [0, 1]
    rule.update_phase_coherence(0.0);
    assert!((rule.phase_coherence() - 0.0).abs() < 1e-6);

    rule.update_phase_coherence(1.0);
    assert!((rule.phase_coherence() - 1.0).abs() < 1e-6);

    rule.update_phase_coherence(0.5);
    assert!((rule.phase_coherence() - 0.5).abs() < 1e-6);
}

#[test]
fn test_cnls_generation_multiple_increments() {
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    for i in 1..=100 {
        rule.next_generation();
        assert_eq!(rule.generation(), i);
    }
}

#[test]
fn test_cnls_hash_chain_multiple_updates() {
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    let hashes = [111, 222, 333, 444, 555];

    for (i, &hash) in hashes.iter().enumerate() {
        rule.update_hash_chain(hash);
        assert_eq!(rule.current_hash(), hash);

        if i > 0 {
            assert_eq!(rule.prev_hash(), hashes[i - 1]);
        }
    }
}

#[test]
#[should_panic(expected = "Timestep dt must be positive")]
fn test_cnls_rule_invalid_dt_zero() {
    CNLSRuleCapsule::new(1.0, 1.0, 0.0, 1.0);
}

#[test]
#[should_panic(expected = "Timestep dt must be positive")]
fn test_cnls_rule_invalid_dt_negative() {
    CNLSRuleCapsule::new(1.0, 1.0, -0.01, 1.0);
}

#[test]
#[should_panic(expected = "Spatial step dx must be positive")]
fn test_cnls_rule_invalid_dx_zero() {
    CNLSRuleCapsule::new(1.0, 1.0, 0.01, 0.0);
}
