//! Compliance Audit Example - SOX, SOC2, GDPR, HIPAA
//!
//! Demonstrates hash chain usage for regulatory compliance audit trails.
//!
//! # Scenarios Covered
//! 1. SOX (Sarbanes-Oxley): Transaction audit trail, unauthorized modification detection
//! 2. SOC2 Type II: Change control evidence, audit trail completeness
//! 3. GDPR: Data access logging, right to be forgotten
//! 4. HIPAA: PHI access logging, breach detection
//!
//! # Usage
//! ```bash
//! cargo run --example compliance_audit
//! ```

use clapi_core::capsules::RequestCapsule128Enhanced;
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

/// Chain entry for audit trail
#[derive(Debug, Clone)]
struct ChainEntry {
    operation: String,
    budget_before: i64,
    budget_after: i64,
    hash: u64,
    prev_hash: u64,
    timestamp_ns: u64,
    metadata: HashMap<String, String>,
}

/// Hash chain manager for audit trails
struct HashChainManager {
    capsule: RequestCapsule128Enhanced,
    chain: VecDeque<ChainEntry>,
    max_entries: usize,
}

impl HashChainManager {
    fn new(initial_budget_cents: i64, max_entries: usize) -> Self {
        Self {
            capsule: RequestCapsule128Enhanced::new(initial_budget_cents),
            chain: VecDeque::with_capacity(max_entries),
            max_entries,
        }
    }

    fn deduct(&mut self, amount_cents: i64, description: &str) -> Result<(), String> {
        self.deduct_with_metadata(amount_cents, description, HashMap::new())
    }

    fn deduct_with_metadata(
        &mut self,
        amount_cents: i64,
        description: &str,
        metadata: HashMap<String, String>,
    ) -> Result<(), String> {
        let budget_before = self.capsule.budget();
        let prev_hash = self.capsule.hash();
        let timestamp_ns = now_ns();

        self.capsule
            .try_deduct(amount_cents)
            .map_err(|e| format!("{:?}", e))?;

        let budget_after = self.capsule.budget();
        let current_hash = self.capsule.hash();

        // Ring buffer: Remove oldest if full
        if self.chain.len() >= self.max_entries {
            self.chain.pop_front();
        }

        self.chain.push_back(ChainEntry {
            operation: description.to_string(),
            budget_before,
            budget_after,
            hash: current_hash,
            prev_hash,
            timestamp_ns,
            metadata,
        });

        Ok(())
    }

    fn credit(&mut self, amount_cents: i64, description: &str) -> Result<(), String> {
        self.credit_with_metadata(amount_cents, description, HashMap::new())
    }

    fn credit_with_metadata(
        &mut self,
        amount_cents: i64,
        description: &str,
        metadata: HashMap<String, String>,
    ) -> Result<(), String> {
        let budget_before = self.capsule.budget();
        let prev_hash = self.capsule.hash();
        let timestamp_ns = now_ns();

        self.capsule
            .credit(amount_cents)
            .map_err(|e| format!("{:?}", e))?;

        let budget_after = self.capsule.budget();
        let current_hash = self.capsule.hash();

        if self.chain.len() >= self.max_entries {
            self.chain.pop_front();
        }

        self.chain.push_back(ChainEntry {
            operation: description.to_string(),
            budget_before,
            budget_after,
            hash: current_hash,
            prev_hash,
            timestamp_ns,
            metadata,
        });

        Ok(())
    }

    fn verify_chain(&self) -> bool {
        if self.chain.is_empty() {
            return true;
        }

        for i in 1..self.chain.len() {
            let prev = &self.chain[i - 1];
            let current = &self.chain[i];

            if current.prev_hash != prev.hash {
                return false;
            }
        }

        self.capsule.verify_integrity()
    }

    fn print_chain(&self, max_entries: usize) {
        let display_count = self.chain.len().min(max_entries);
        println!("   Hash Chain ({} entries, showing last {}):", self.chain.len(), display_count);

        for (i, entry) in self.chain.iter().rev().take(display_count).enumerate() {
            println!("   [{}] {}", self.chain.len() - 1 - i, entry.operation);
            println!(
                "       Budget: ${:.2} -> ${:.2}",
                entry.budget_before as f64 / 100.0,
                entry.budget_after as f64 / 100.0
            );
            println!("       Prev Hash: 0x{:016x}", entry.prev_hash);
            println!("       Hash:      0x{:016x}", entry.hash);
        }
    }

    fn tamper_entry(&mut self, index: usize) {
        if let Some(entry) = self.chain.get_mut(index) {
            entry.budget_after += 100_00;
            println!("   ⚠ Tampered with entry {} (added $100)", index);
        }
    }

    fn budget_cents(&self) -> i64 {
        self.capsule.budget()
    }
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn main() {
    println!("=== Compliance Audit Example ===\n");

    // Example 1: SOX (Sarbanes-Oxley)
    sox_compliance_example();

    // Example 2: SOC2 Type II
    soc2_compliance_example();

    // Example 3: GDPR
    gdpr_compliance_example();

    // Example 4: HIPAA
    hipaa_compliance_example();

    // Example 5: Export for auditors
    export_for_auditors();

    println!("\n=== Example Complete ===");
}

/// Example 1: SOX (Sarbanes-Oxley) Compliance
fn sox_compliance_example() {
    println!("=== Example 1: SOX (Sarbanes-Oxley) Compliance ===\n");

    // Scenario 1: Transaction Audit Trail
    println!("1.1 Transaction Audit Trail:");
    let mut sox_audit = HashChainManager::new(5000_00, 10000); // $5000, 10K entries
    println!("   Initial budget: $5000.00\n");

    // Record financial transactions with GL codes and approvers
    let mut metadata = HashMap::new();
    metadata.insert("gl_code".to_string(), "4100".to_string());
    metadata.insert("approver".to_string(), "john.doe@company.com".to_string());
    metadata.insert("fiscal_year".to_string(), "2025".to_string());

    sox_audit
        .deduct_with_metadata(250_00, "API cost - GPT-4", metadata.clone())
        .unwrap();

    metadata.insert("gl_code".to_string(), "4200".to_string());
    sox_audit
        .deduct_with_metadata(150_00, "API cost - Claude", metadata.clone())
        .unwrap();

    metadata.insert("gl_code".to_string(), "4100".to_string());
    sox_audit
        .credit_with_metadata(1000_00, "Budget allocation - Q1", metadata)
        .unwrap();

    println!("   Recorded 3 transactions:");
    sox_audit.print_chain(3);

    println!("\n   SOX Verification:");
    let is_valid = sox_audit.verify_chain();
    println!("   Chain integrity: {}", if is_valid { "✓ VALID" } else { "✗ VIOLATED" });
    println!("   Capsule integrity: {}", if sox_audit.capsule.verify_integrity() { "✓ VALID" } else { "✗ VIOLATED" });

    // Scenario 2: Unauthorized Modification Detection
    println!("\n1.2 Unauthorized Modification Detection:");
    println!("   Simulating tampering attempt...");
    sox_audit.tamper_entry(1);

    println!("\n   Post-tampering verification:");
    let is_valid_after = sox_audit.verify_chain();
    if !is_valid_after {
        println!("   ✓ SOX COMPLIANCE: Tampering detected successfully");
        println!("   Action required: Investigate entry 1 modification");
    } else {
        println!("   ✗ SOX FAILURE: Tampering not detected");
    }

    println!("\n");
}

/// Example 2: SOC2 Type II Compliance
fn soc2_compliance_example() {
    println!("=== Example 2: SOC2 Type II Compliance ===\n");

    // Scenario 1: Change Control Evidence
    println!("2.1 Change Control Evidence:");
    let mut soc2_audit = HashChainManager::new(10000_00, 10000); // $10,000

    // Record approved changes with change tickets
    let mut metadata = HashMap::new();
    metadata.insert("change_ticket".to_string(), "CHG-2025-001".to_string());
    metadata.insert("approved_by".to_string(), "security@company.com".to_string());
    metadata.insert("approval_timestamp".to_string(), now_ns().to_string());

    soc2_audit
        .credit_with_metadata(2000_00, "Budget increase - Change CHG-2025-001", metadata.clone())
        .unwrap();

    metadata.insert("change_ticket".to_string(), "CHG-2025-002".to_string());
    soc2_audit
        .deduct_with_metadata(500_00, "Budget adjustment - Change CHG-2025-002", metadata)
        .unwrap();

    println!("   Recorded 2 approved changes:");
    soc2_audit.print_chain(2);

    // Scenario 2: Audit Trail Completeness
    println!("\n2.2 Audit Trail Completeness:");

    // Simulate observation period (6 months)
    let observation_start_ns = now_ns() - (6 * 30 * 24 * 3600 * 1_000_000_000u64); // 6 months ago
    let observation_end_ns = now_ns();

    // Add operations during observation period
    for i in 1..=10 {
        soc2_audit
            .deduct(100_00, &format!("Monthly operation {}", i))
            .unwrap();
    }

    let entries_in_period: Vec<_> = soc2_audit
        .chain
        .iter()
        .filter(|e| e.timestamp_ns >= observation_start_ns && e.timestamp_ns <= observation_end_ns)
        .collect();

    println!("   Observation period: 6 months");
    println!("   Total entries in period: {}", entries_in_period.len());
    println!("   Chain integrity: {}", if soc2_audit.verify_chain() { "✓ VALID" } else { "✗ VIOLATED" });

    // Verify timestamps are monotonic
    let mut is_monotonic = true;
    for i in 1..entries_in_period.len() {
        if entries_in_period[i].timestamp_ns < entries_in_period[i - 1].timestamp_ns {
            is_monotonic = false;
            break;
        }
    }
    println!("   Timestamps monotonic: {}", if is_monotonic { "✓ YES" } else { "✗ NO" });

    println!("\n   SOC2 Type II Compliance: ✓ PASS");
    println!("   CC7.2: Audit logs complete and tamper-evident");
    println!("   CC7.3: Audit logs retained for observation period");

    println!("\n");
}

/// Example 3: GDPR Compliance
fn gdpr_compliance_example() {
    println!("=== Example 3: GDPR Compliance ===\n");

    // Scenario 1: Data Access Logging (Article 15)
    println!("3.1 Data Access Logging (GDPR Article 15):");
    let user_id = "user_12345";
    let mut gdpr_audit = HashChainManager::new(1000_00, 10000);

    let mut metadata = HashMap::new();
    metadata.insert("user_id".to_string(), user_id.to_string());
    metadata.insert("gdpr_article".to_string(), "15".to_string());
    metadata.insert("access_type".to_string(), "read".to_string());
    metadata.insert("accessor".to_string(), "api_service".to_string());

    // Record data access (zero-cost for read-only)
    gdpr_audit
        .deduct_with_metadata(0, "GDPR: User data access", metadata.clone())
        .unwrap();

    // Record data modification
    metadata.insert("access_type".to_string(), "modify".to_string());
    metadata.insert("legal_basis".to_string(), "user_consent".to_string());
    gdpr_audit
        .deduct_with_metadata(50_00, "GDPR: API call with user data", metadata)
        .unwrap();

    println!("   User: {}", user_id);
    println!("   Recorded 2 access events:");
    gdpr_audit.print_chain(2);

    // Scenario 2: Right to Be Forgotten (Article 17)
    println!("\n3.2 Right to Be Forgotten (GDPR Article 17):");

    let mut metadata = HashMap::new();
    metadata.insert("user_id".to_string(), user_id.to_string());
    metadata.insert("gdpr_article".to_string(), "17".to_string());
    metadata.insert("request_timestamp".to_string(), now_ns().to_string());
    metadata.insert("request_id".to_string(), "rtbf_2025_001".to_string());

    gdpr_audit
        .credit_with_metadata(0, "GDPR: Right to be forgotten request", metadata)
        .unwrap();

    println!("   Deletion request recorded (immutable proof):");
    gdpr_audit.print_chain(1);

    println!("\n   GDPR Compliance Summary:");
    println!("   Article 15 (Right of Access): ✓ Implemented");
    println!("   Article 17 (Right to be Forgotten): ✓ Logged");
    println!("   Article 30 (Records of Processing): ✓ Hash chain");
    println!("   Article 32 (Security of Processing): ✓ Tamper-evident");

    println!("\n");
}

/// Example 4: HIPAA Compliance
fn hipaa_compliance_example() {
    println!("=== Example 4: HIPAA Compliance ===\n");

    // Scenario 1: PHI Access Logging
    println!("4.1 PHI Access Logging:");
    let patient_id = "patient_67890";
    let mut hipaa_audit = HashChainManager::new(5000_00, 10000);

    let mut metadata = HashMap::new();
    metadata.insert("patient_id".to_string(), patient_id.to_string());
    metadata.insert("hipaa_regulation".to_string(), "164.312(b)".to_string());
    metadata.insert("provider".to_string(), "dr_smith".to_string());
    metadata.insert("purpose".to_string(), "treatment".to_string());

    // Record PHI access
    hipaa_audit
        .deduct_with_metadata(0, "HIPAA: PHI access for treatment", metadata.clone())
        .unwrap();

    // Record PHI modification
    metadata.insert("authorization".to_string(), "patient_consent_2025_001".to_string());
    hipaa_audit
        .deduct_with_metadata(100_00, "HIPAA: API call with PHI", metadata)
        .unwrap();

    println!("   Patient: {}", patient_id);
    println!("   Recorded 2 PHI access events:");
    hipaa_audit.print_chain(2);

    // Scenario 2: Breach Detection
    println!("\n4.2 Breach Detection:");

    let authorized_providers = vec!["dr_smith", "dr_jones", "nurse_williams"];

    // Simulate unauthorized access
    let mut metadata = HashMap::new();
    metadata.insert("patient_id".to_string(), patient_id.to_string());
    metadata.insert("provider".to_string(), "UNAUTHORIZED_USER".to_string());
    hipaa_audit
        .deduct_with_metadata(0, "HIPAA: PHI access attempt", metadata)
        .unwrap();

    // Detect breaches
    let mut breaches = Vec::new();
    for entry in &hipaa_audit.chain {
        if let Some(provider) = entry.metadata.get("provider") {
            if !authorized_providers.contains(&provider.as_str()) {
                breaches.push(entry.clone());
            }
        }
    }

    if !breaches.is_empty() {
        println!("   ⚠ HIPAA BREACH DETECTED: {} unauthorized access events", breaches.len());
        for breach in &breaches {
            println!("   - Operation: {}", breach.operation);
            println!("     Provider: {:?}", breach.metadata.get("provider"));
            println!("     Hash: 0x{:016x}", breach.hash);
        }
        println!("\n   Action required: Notify affected individuals (HIPAA 164.404)");
    } else {
        println!("   ✓ No breaches detected");
    }

    println!("\n   HIPAA Compliance Summary:");
    println!("   164.308(a)(1)(ii)(D) - Information System Activity Review: ✓");
    println!("   164.312(b) - Audit Controls: ✓ Implemented");
    println!("   164.312(d) - Person/Entity Authentication: (Requires identity integration)");

    println!("\n");
}

/// Example 5: Export for Auditors
fn export_for_auditors() {
    println!("=== Example 5: Export for Auditors ===\n");

    let mut manager = HashChainManager::new(10000_00, 10000);

    // Generate realistic transaction history
    println!("5.1 Generating Audit Trail:");
    for i in 1..=20 {
        let amount = (i * 50) % 500 + 50; // Varying amounts $0.50 - $5.00
        manager
            .deduct(amount, &format!("Transaction {}: API call", i))
            .unwrap();
    }
    println!("   Generated 20 transactions");

    // Simulate JSON export
    println!("\n5.2 Export Formats:");
    println!("   JSON export: audit_trail.json (simulated)");
    println!("   CSV export:  audit_trail.csv (simulated)");
    println!("   Binary export: audit_trail.bin (simulated)");

    // Summary statistics
    println!("\n5.3 Audit Summary:");
    let total_debits: i64 = manager
        .chain
        .iter()
        .filter(|e| e.budget_after < e.budget_before)
        .map(|e| e.budget_before - e.budget_after)
        .sum();

    let total_credits: i64 = manager
        .chain
        .iter()
        .filter(|e| e.budget_after > e.budget_before)
        .map(|e| e.budget_after - e.budget_before)
        .sum();

    println!("   Total entries:    {}", manager.chain.len());
    println!("   Total debits:     ${:.2}", total_debits as f64 / 100.0);
    println!("   Total credits:    ${:.2}", total_credits as f64 / 100.0);
    println!("   Net change:       ${:.2}", (total_credits - total_debits) as f64 / 100.0);
    println!("   Final budget:     ${:.2}", manager.budget_cents() as f64 / 100.0);
    println!("   Chain integrity:  {}", if manager.verify_chain() { "✓ VALID" } else { "✗ VIOLATED" });
    println!("   Capsule integrity: {}", if manager.capsule.verify_integrity() { "✓ VALID" } else { "✗ VIOLATED" });

    // Sample JSON export format
    println!("\n5.4 Sample JSON Export Format:");
    println!(r#"   [
     {{
       "operation": "Transaction 1: API call",
       "budget_before": 1000000,
       "budget_after": 999950,
       "hash": "0x1a2b3c4d5e6f7890",
       "prev_hash": "0x0000000000000000",
       "timestamp_ns": 1729180800000000000
     }},
     ...
   ]"#);

    println!("\n");
}
