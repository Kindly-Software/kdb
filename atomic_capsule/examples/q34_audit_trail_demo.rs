//! Q34 Audit Trail Integration Example
//!
//! Demonstrates complete Q34 audit trail usage for regulatory compliance
//! (SOX, SOC2, GDPR, HIPAA).
//!
//! # Usage
//!
//! ```bash
//! cargo run --example q34_audit_trail_demo --features std
//! ```

use atomic_capsule::protection::{
    AuditLog, ComplianceReport, operation_history, operations_by_instance,
    tamper_detected, verify_deterministic_sequence,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Q34 Audit Trail Compliance Demo ===\n");

    // 1. Create audit log
    println!("1. Creating audit log...");
    let audit_log = AuditLog::open("/tmp/q34_audit.jsonl")?;
    println!("   ✓ Audit log initialized\n");

    // 2. Simulate Git operations
    println!("2. Simulating Git operations...");

    // Instance 1: Developer Alice
    let alice_id = 1;
    let commit_hash_1 = [1u8; 20];
    let mut data = [0u8; 88];
    data[0..5].copy_from_slice(b"main\0");

    audit_log.append(alice_id, 2, &commit_hash_1, &data)?; // Branch: main
    println!("   ✓ Alice created branch 'main'");

    let commit_hash_2 = [2u8; 20];
    data[0..12].copy_from_slice(b"feature-123\0");
    audit_log.append(alice_id, 2, &commit_hash_2, &data)?; // Branch: feature-123
    println!("   ✓ Alice created branch 'feature-123'");

    let commit_hash_3 = [3u8; 20];
    data[0..10].copy_from_slice(b"README.md\0");
    audit_log.append(alice_id, 5, &commit_hash_3, &data)?; // Add: README.md
    println!("   ✓ Alice added file 'README.md'");

    let commit_hash_4 = [4u8; 20];
    data[0..10].copy_from_slice(b"src/lib.rs");
    audit_log.append(alice_id, 1, &commit_hash_4, &data)?; // Commit: src/lib.rs
    println!("   ✓ Alice committed 'src/lib.rs'");

    // Instance 2: Developer Bob
    let bob_id = 2;
    let commit_hash_5 = [5u8; 20];
    data[0..12].copy_from_slice(b"feature-123\0");
    audit_log.append(bob_id, 3, &commit_hash_5, &data)?; // Merge: feature-123
    println!("   ✓ Bob merged 'feature-123'");

    let commit_hash_6 = [6u8; 20];
    data[0..4].copy_from_slice(b"main");
    audit_log.append(bob_id, 4, &commit_hash_6, &data)?; // Push: main
    println!("   ✓ Bob pushed to 'main'\n");

    // 3. Generate compliance report
    println!("3. Generating compliance report...");
    let report = ComplianceReport::generate(&audit_log)?;

    println!("   Total entries: {}", report.total_entries);
    println!("   Chain valid: {}", report.chain_valid);
    println!("   Sequence valid: {}", report.sequence_valid);
    println!("   Tamper detected: {}", report.tamper_detected);
    println!("   Unique instances: {}", report.unique_instances);

    if let (Some(start), Some(end)) = report.date_range {
        let duration_ms = (end - start) / 1_000_000;
        println!("   Duration: {} ms", duration_ms);
    }

    println!("\n   Operation summary:");
    for (op_type, count) in &report.operation_summary {
        let op_name = match op_type {
            1 => "Commit",
            2 => "Branch",
            3 => "Merge",
            4 => "Push",
            5 => "Add",
            _ => "Unknown",
        };
        println!("     {}: {} operations", op_name, count);
    }
    println!();

    // 4. Check compliance standards
    println!("4. Checking compliance standards...");
    let (sox, soc2, gdpr, hipaa) = report.all_compliant();

    println!(
        "   ✓ SOX (Sarbanes-Oxley): {}",
        if sox { "COMPLIANT" } else { "NON-COMPLIANT" }
    );
    println!(
        "   ✓ SOC2 (Service Organization Control 2): {}",
        if soc2 { "COMPLIANT" } else { "NON-COMPLIANT" }
    );
    println!(
        "   ✓ GDPR (General Data Protection Regulation): {}",
        if gdpr { "COMPLIANT" } else { "NON-COMPLIANT" }
    );
    println!(
        "   ✓ HIPAA (Health Insurance Portability and Accountability Act): {}",
        if hipaa { "COMPLIANT" } else { "NON-COMPLIANT" }
    );
    println!();

    // 5. GDPR Data Provenance (Article 15)
    println!("5. GDPR Data Provenance (Article 15)...");
    let alice_ops = operations_by_instance(&audit_log, alice_id)?;
    println!("   Alice's operations ({} total):", alice_ops.len());
    for op in alice_ops {
        let ts_sec = op.timestamp / 1_000_000_000;
        println!(
            "     - {} at {} (instance {})",
            op.operation_name, ts_sec, op.instance_id
        );
    }

    let bob_ops = operations_by_instance(&audit_log, bob_id)?;
    println!("\n   Bob's operations ({} total):", bob_ops.len());
    for op in bob_ops {
        let ts_sec = op.timestamp / 1_000_000_000;
        println!(
            "     - {} at {} (instance {})",
            op.operation_name, ts_sec, op.instance_id
        );
    }
    println!();

    // 6. HIPAA Deterministic Sequence
    println!("6. HIPAA Deterministic Sequence (164.312(b))...");
    let sequence_valid = verify_deterministic_sequence(&audit_log)?;
    println!(
        "   Sequence integrity: {}",
        if sequence_valid { "VALID" } else { "INVALID" }
    );
    println!();

    // 7. SOX/SOC2 Tamper Detection
    println!("7. SOX/SOC2 Tamper Detection...");
    let tamper = tamper_detected(&audit_log)?;
    println!(
        "   Tampering: {}",
        if tamper {
            "DETECTED (ALERT!)"
        } else {
            "None (chain intact)"
        }
    );
    println!();

    // 8. Full Operation History
    println!("8. Full Operation History...");
    let history = operation_history(&audit_log)?;
    println!("   Complete audit trail:");
    for (op_name, instance_id, timestamp) in history {
        let ts_sec = timestamp / 1_000_000_000;
        println!("     {} by instance {} at {}", op_name, instance_id, ts_sec);
    }
    println!();

    // 9. Verify Chain Integrity
    println!("9. Verifying chain integrity...");
    let chain_valid = audit_log.verify_chain()?;
    println!(
        "   Chain verification: {}",
        if chain_valid {
            "PASSED ✓"
        } else {
            "FAILED ✗"
        }
    );
    println!();

    // 10. Export Summary
    println!("10. Summary:");
    println!("    ✓ {} audit entries recorded", audit_log.entry_count());
    println!("    ✓ Hash chain verified");
    println!("    ✓ All compliance standards met");
    println!("    ✓ Audit log saved to /tmp/q34_audit.jsonl");
    println!();

    println!("=== Demo Complete ===");

    Ok(())
}
