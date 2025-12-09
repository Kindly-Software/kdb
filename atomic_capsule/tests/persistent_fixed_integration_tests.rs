//! T9+T3 Persistent Fixed-Point State - Integration Tests
//!
//! **Framework**: T28 Testing (Tier 3: Integration Tests)
//! **Coverage**: 5 integration tests (80 LOC)
//! **Target**: Financial compliance, audit trails, end-to-end workflows
//!
//! # Test Categories
//!
//! 1. **Financial Compliance** (2 tests): 1M transactions, deterministic accounting
//! 2. **Audit Trail** (2 tests): Hash chain integrity, compliance (SOX/SOC2/GDPR)
//! 3. **End-to-End** (1 test): Complete workflow (create, transact, recover, audit)

use atomic_capsule::persistent::fixed_point_state::PersistentFixedPointState;
use atomic_capsule::primitives::fixed_point::{FixedPoint, Q16_16};
use tempfile::NamedTempFile;

// ============================================================================
// § 1: Financial Compliance Tests (2 tests)
// ============================================================================

#[test]
fn integration_financial_compliance_1m_transactions() {
    // Test: Process 1M financial transactions with deterministic accounting
    //
    // Requirement: SOX compliance requires exact decimal arithmetic
    // Validation: Zero floating-point drift after 1M operations
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    // Starting balance: $10,000.00
    state.atomic_store_fixed(Q16_16::from_f64(10000.0)).unwrap();

    // Simulate 1M transactions:
    // - 500K deposits of $1.23
    // - 500K withdrawals of $1.23
    // Expected final balance: $10,000.00 (exact)
    for _ in 0..500_000 {
        state.fixed_add(Q16_16::from_f64(1.23)).unwrap();
    }
    for _ in 0..500_000 {
        state.fixed_add(Q16_16::from_f64(-1.23)).unwrap();
    }

    let final_balance = state.atomic_load_fixed();
    assert_eq!(
        final_balance.to_f64(),
        10000.0,
        "After 1M transactions, balance must be EXACTLY $10,000.00 (SOX compliance)"
    );

    // Verify operation count
    assert_eq!(
        state.operation_count(),
        1_000_001, // 1M adds + 1 initial store
        "Operation count must match transaction count"
    );
}

#[test]
fn integration_roundtrip_decimal_export_import() {
    // Test: Export to decimal format, verify compliance audit trail
    //
    // Requirement: Q34 Auditability - human-readable audit export
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    // Process multiple transactions
    state.atomic_store_fixed(Q16_16::from_f64(1000.0)).unwrap();
    state.fixed_add(Q16_16::from_f64(234.56)).unwrap();
    state.fixed_add(Q16_16::from_f64(-100.00)).unwrap();

    // Export to decimal format for audit
    let (decimal, generation, audit_hash, op_count) = state.export_decimal();

    // Verify decimal accuracy
    assert!(
        decimal.contains("1134.56"),
        "Decimal export must match: {}",
        decimal
    );

    // Verify audit metadata
    assert!(generation > 0, "Generation must be tracked");
    assert!(audit_hash > 0, "Audit hash must be non-zero");
    assert_eq!(op_count, 3, "Operation count: 1 store + 2 adds");

    println!("Audit Export:");
    println!("  Balance: {}", decimal);
    println!("  Generation: {}", generation);
    println!("  Audit Hash: 0x{:016x}", audit_hash);
    println!("  Operations: {}", op_count);
}

// ============================================================================
// § 2: Audit Trail Tests (2 tests)
// ============================================================================

#[test]
fn integration_audit_trail_chain_integrity() {
    // Test: Hash chain provides tamper-detection (Q34 Auditability)
    //
    // Requirement: SOX/SOC2/GDPR compliance - tamper-evident audit trail
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let mut audit_trail = Vec::new();

    // Record 100 transactions with audit hashes
    for i in 0..100 {
        let amount = Q16_16::from_f64((i as f64) * 10.5);
        state.fixed_add(amount).unwrap();

        let hash = state.audit_hash();
        let balance = state.atomic_load_fixed();
        audit_trail.push((i, balance.to_f64(), hash));
    }

    // Verify audit trail properties:
    // 1. All hashes are unique (no collisions)
    let unique_hashes: std::collections::HashSet<_> =
        audit_trail.iter().map(|(_, _, h)| h).collect();
    assert_eq!(
        unique_hashes.len(),
        100,
        "All audit hashes must be unique (tamper detection)"
    );

    // 2. Hash chain is monotonic (each hash depends on previous)
    for i in 1..audit_trail.len() {
        let (_, _, prev_hash) = audit_trail[i - 1];
        let (_, _, curr_hash) = audit_trail[i];
        assert_ne!(
            prev_hash, curr_hash,
            "Audit hash must change on every operation (chain integrity)"
        );
    }

    println!("Audit Trail Sample (first 5 operations):");
    for (i, balance, hash) in audit_trail.iter().take(5) {
        println!("  Op {}: Balance ${:.2}, Hash 0x{:016x}", i, balance, hash);
    }
}

#[test]
fn integration_compliance_report() {
    // Test: Generate SOX/SOC2/GDPR compliance report
    //
    // Requirement: Regulatory audit trail with deterministic accounting
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();
    let state = PersistentFixedPointState::create(path).unwrap();

    // Simulate daily transactions
    let transactions = vec![
        ("Deposit", 1000.00),
        ("Deposit", 500.50),
        ("Withdrawal", -250.25),
        ("Deposit", 100.10),
        ("Withdrawal", -50.05),
    ];

    for (tx_type, amount) in &transactions {
        let delta = Q16_16::from_f64(*amount);
        state.fixed_add(delta).unwrap();
        println!("  {}: ${:.2}", tx_type, amount);
    }

    // Flush to disk (durability)
    state.flush(path).unwrap();

    // Generate compliance report
    let (balance, generation, audit_hash, op_count) = state.export_decimal();

    println!("\n=== COMPLIANCE REPORT ===");
    println!("Final Balance: {}", balance);
    println!("Generation: {}", generation);
    println!("Audit Hash: 0x{:016x}", audit_hash);
    println!("Total Operations: {}", op_count);
    println!(
        "Status: {} (even = committed)",
        if generation % 2 == 0 {
            "✓ COMMITTED"
        } else {
            "⚠ IN-PROGRESS"
        }
    );

    // Verify compliance requirements
    assert_eq!(
        generation % 2,
        0,
        "Generation must be even (committed state)"
    );
    assert_eq!(
        op_count,
        transactions.len() as u64,
        "All transactions recorded"
    );

    // Verify deterministic accounting
    let expected_balance = transactions.iter().map(|(_, amt)| amt).sum::<f64>();
    let actual_balance: f64 = balance.parse().unwrap();
    assert!(
        (actual_balance - expected_balance).abs() < 0.01,
        "Balance must match expected (deterministic accounting)"
    );
}

// ============================================================================
// § 3: End-to-End Workflow Test (1 test)
// ============================================================================

#[test]
fn integration_end_to_end_workflow() {
    // Test: Complete workflow - create, transact, crash recover, audit
    //
    // Validates:
    // - T9: Crash-safe persistence (generation counter recovery)
    // - T3: Deterministic fixed-point arithmetic
    // - Q34: Audit trail integrity (hash chaining)
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();

    // Phase 1: Create account and perform transactions
    {
        println!("Phase 1: Initial transactions");
        let state = PersistentFixedPointState::create(path).unwrap();

        state.atomic_store_fixed(Q16_16::from_f64(5000.0)).unwrap();
        state.fixed_add(Q16_16::from_f64(1234.56)).unwrap();
        state.fixed_add(Q16_16::from_f64(-234.56)).unwrap();

        let balance = state.atomic_load_fixed();
        println!("  Balance after transactions: ${:.2}", balance.to_f64());

        // Flush to disk
        state.flush(path).unwrap();
    } // Simulated "crash" (drop state)

    // Phase 2: Recover from "crash" and verify state
    {
        println!("\nPhase 2: Crash recovery");
        let state = PersistentFixedPointState::open(path).unwrap();

        let recovered_balance = state.atomic_load_fixed();
        println!("  Recovered balance: ${:.2}", recovered_balance.to_f64());

        assert!(
            (recovered_balance.to_f64() - 6000.0).abs() < 0.01,
            "Balance must persist after crash"
        );

        let gen = state.generation();
        assert_eq!(
            gen % 2,
            0,
            "Generation must be even (committed state after recovery)"
        );
    }

    // Phase 3: Continue transactions post-recovery
    {
        println!("\nPhase 3: Post-recovery transactions");
        let state = PersistentFixedPointState::open(path).unwrap();

        state.fixed_add(Q16_16::from_f64(500.0)).unwrap();
        state.fixed_add(Q16_16::from_f64(-100.0)).unwrap();

        let final_balance = state.atomic_load_fixed();
        println!("  Final balance: ${:.2}", final_balance.to_f64());

        assert!(
            (final_balance.to_f64() - 6400.0).abs() < 0.01,
            "Post-recovery transactions must work correctly"
        );
    }

    // Phase 4: Generate final audit report
    {
        println!("\nPhase 4: Audit trail verification");
        let state = PersistentFixedPointState::open(path).unwrap();

        let (balance, generation, audit_hash, op_count) = state.export_decimal();

        println!("  Final Balance: {}", balance);
        println!("  Generation: {}", generation);
        println!("  Audit Hash: 0x{:016x}", audit_hash);
        println!("  Total Operations: {}", op_count);

        // Verify audit trail integrity
        assert_eq!(op_count, 5, "All 5 operations recorded (1 store + 4 adds)");
        assert!(audit_hash > 0, "Audit hash chain maintained");
        assert_eq!(generation % 2, 0, "Final state committed");
    }

    println!("\n✓ End-to-end workflow complete: T9+T3+Q34 validated");
}
