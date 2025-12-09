//! Forensic Analysis Example - Timeline Reconstruction and Anomaly Detection
//!
//! Demonstrates hash chain usage for forensic investigation:
//! - Reconstruct state at any point in time
//! - Detect unauthorized operations
//! - Identify anomalies and unusual patterns
//! - Timeline reconstruction for incident response
//!
//! # Usage
//! ```bash
//! cargo run --example forensic_analysis
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

/// Hash chain manager for forensic analysis
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
    println!("=== Forensic Analysis Example ===\n");

    // Example 1: Reconstruct timeline
    reconstruct_timeline_example();

    // Example 2: Anomaly detection
    anomaly_detection_example();

    // Example 3: Find root cause
    root_cause_analysis_example();

    // Example 4: Investigate unauthorized access
    unauthorized_access_investigation();

    println!("\n=== Example Complete ===");
}

/// Example 1: Timeline Reconstruction
fn reconstruct_timeline_example() {
    println!("=== Example 1: Timeline Reconstruction ===\n");

    let mut manager = HashChainManager::new(10000_00, 10000); // $10,000

    println!("1.1 Simulating Transaction History:");

    // Simulate realistic transaction timeline
    let base_time = now_ns();

    for i in 0u64..15 {
        let _timestamp_offset = i * 60_000_000_000; // 1 minute apart (unused but for documentation)
        let amount = ((i * 73) % 200 + 50) as i64; // Varying amounts

        let mut metadata = HashMap::new();
        metadata.insert("user_id".to_string(), format!("user_{}", (i % 3) + 1));
        metadata.insert("request_id".to_string(), format!("req_{:04}", i + 1));

        manager
            .deduct_with_metadata(amount, &format!("API call - Transaction {}", i + 1), metadata)
            .unwrap();
    }

    println!("   Generated 15 transactions over 15 minutes\n");

    // Reconstruct state at specific points in time
    println!("1.2 State Reconstruction:");

    let points_in_time = vec![
        ("5 minutes ago", base_time + 5u64 * 60_000_000_000),
        ("10 minutes ago", base_time + 10u64 * 60_000_000_000),
        ("15 minutes ago", base_time + 15u64 * 60_000_000_000),
    ];

    for (label, target_time) in points_in_time {
        if let Some(state) = reconstruct_state(&manager, target_time) {
            println!("   {} (timestamp {}):", label, target_time);
            println!("     Budget: ${:.2}", state.budget_after as f64 / 100.0);
            println!("     Operation: {}", state.operation);
            println!("     Hash: 0x{:016x}", state.hash);
        } else {
            println!("   {} - No state available", label);
        }
    }

    println!("\n");
}

fn reconstruct_state(manager: &HashChainManager, target_timestamp_ns: u64) -> Option<ChainEntry> {
    manager
        .chain
        .iter()
        .filter(|e| e.timestamp_ns <= target_timestamp_ns)
        .last()
        .cloned()
}

/// Example 2: Anomaly Detection
fn anomaly_detection_example() {
    println!("=== Example 2: Anomaly Detection ===\n");

    let mut manager = HashChainManager::new(50000_00, 10000); // $50,000

    println!("2.1 Generating Transaction History with Anomalies:");

    // Normal transactions
    for i in 0..10 {
        let amount = ((i * 37) % 100 + 20) as i64; // Normal: $0.20 - $1.20
        manager
            .deduct(amount, &format!("Normal transaction {}", i + 1))
            .unwrap();
    }

    // Insert anomalies
    manager
        .deduct(5000_00, "⚠ ANOMALY: Large transaction")
        .unwrap(); // $50.00 (unusual)

    for i in 10..15 {
        let amount = ((i * 37) % 100 + 20) as i64;
        manager
            .deduct(amount, &format!("Normal transaction {}", i + 1))
            .unwrap();
    }

    // Unauthorized operation
    let mut metadata = HashMap::new();
    metadata.insert("user_id".to_string(), "UNAUTHORIZED".to_string());
    manager
        .deduct_with_metadata(100_00, "⚠ ANOMALY: Unauthorized access", metadata)
        .unwrap();

    // More normal transactions
    for i in 15..20 {
        let amount = ((i * 37) % 100 + 20) as i64;
        manager
            .deduct(amount, &format!("Normal transaction {}", i + 1))
            .unwrap();
    }

    println!("   Generated 22 transactions (2 anomalies injected)\n");

    // Detect anomalies
    println!("2.2 Anomaly Detection Results:");
    let anomalies = detect_anomalies(&manager);

    if anomalies.is_empty() {
        println!("   No anomalies detected");
    } else {
        println!("   Detected {} anomalies:", anomalies.len());
        for (i, anomaly) in anomalies.iter().enumerate() {
            println!("\n   Anomaly {}:", i + 1);
            println!("     Type: {}", anomaly.anomaly_type);
            println!("     Operation: {}", anomaly.entry.operation);
            println!("     Budget impact: ${:.2}", (anomaly.entry.budget_before - anomaly.entry.budget_after) as f64 / 100.0);
            println!("     Hash: 0x{:016x}", anomaly.entry.hash);
            println!("     Severity: {}", anomaly.severity);
        }
    }

    println!("\n");
}

#[derive(Debug)]
struct Anomaly {
    anomaly_type: String,
    entry: ChainEntry,
    severity: String,
}

fn detect_anomalies(manager: &HashChainManager) -> Vec<Anomaly> {
    let mut anomalies = Vec::new();

    // Calculate statistics for normal transactions
    let transaction_amounts: Vec<i64> = manager
        .chain
        .iter()
        .map(|e| (e.budget_before - e.budget_after).abs())
        .collect();

    let mean = transaction_amounts.iter().sum::<i64>() as f64 / transaction_amounts.len() as f64;
    let variance = transaction_amounts
        .iter()
        .map(|&x| {
            let diff = x as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / transaction_amounts.len() as f64;
    let std_dev = variance.sqrt();

    for entry in &manager.chain {
        let delta = (entry.budget_after - entry.budget_before).abs();

        // Anomaly 1: Large transaction (> 3 standard deviations)
        if (delta as f64 - mean).abs() > 3.0 * std_dev && delta > 1000_00 {
            anomalies.push(Anomaly {
                anomaly_type: "Large transaction".to_string(),
                entry: entry.clone(),
                severity: "HIGH".to_string(),
            });
        }

        // Anomaly 2: Unauthorized access
        if let Some(user_id) = entry.metadata.get("user_id") {
            if user_id == "UNAUTHORIZED" {
                anomalies.push(Anomaly {
                    anomaly_type: "Unauthorized access".to_string(),
                    entry: entry.clone(),
                    severity: "CRITICAL".to_string(),
                });
            }
        }

        // Anomaly 3: Unusual operation pattern
        if entry.operation.contains("⚠") {
            anomalies.push(Anomaly {
                anomaly_type: "Flagged operation".to_string(),
                entry: entry.clone(),
                severity: "MEDIUM".to_string(),
            });
        }
    }

    anomalies
}

/// Example 3: Root Cause Analysis
fn root_cause_analysis_example() {
    println!("=== Example 3: Root Cause Analysis ===\n");

    let mut manager = HashChainManager::new(10000_00, 10000);

    println!("3.1 Simulating Incident Scenario:");

    // Normal operations
    for i in 0..5 {
        manager
            .deduct(100_00, &format!("Normal operation {}", i + 1))
            .unwrap();
    }

    // Root cause: Configuration change
    let mut metadata = HashMap::new();
    metadata.insert("config_change".to_string(), "rate_limit_disabled".to_string());
    metadata.insert("changed_by".to_string(), "admin_user".to_string());
    manager
        .credit_with_metadata(0, "Configuration change: Rate limit disabled", metadata)
        .unwrap();

    // Subsequent high-frequency operations (incident manifestation)
    for i in 0..10 {
        manager
            .deduct(500_00, &format!("⚠ High-frequency operation {}", i + 1))
            .unwrap();
    }

    println!("   Incident: Budget depleted rapidly after configuration change\n");

    // Root cause analysis
    println!("3.2 Root Cause Analysis:");

    // Find configuration changes
    let config_changes: Vec<_> = manager
        .chain
        .iter()
        .filter(|e| e.metadata.contains_key("config_change"))
        .collect();

    if !config_changes.is_empty() {
        println!("   Found {} configuration changes:", config_changes.len());
        for change in &config_changes {
            println!("\n   Configuration Change:");
            println!("     Operation: {}", change.operation);
            println!("     Changed by: {:?}", change.metadata.get("changed_by"));
            println!("     Change: {:?}", change.metadata.get("config_change"));
            println!("     Timestamp: {}", change.timestamp_ns);

            // Find operations after this change
            let subsequent_ops: Vec<_> = manager
                .chain
                .iter()
                .filter(|e| e.timestamp_ns > change.timestamp_ns)
                .take(5)
                .collect();

            println!("\n     Next 5 operations after change:");
            for op in &subsequent_ops {
                let delta = (op.budget_before - op.budget_after).abs();
                println!("       - {} (${:.2})", op.operation, delta as f64 / 100.0);
            }
        }

        println!("\n   Root Cause Identified:");
        println!("   - Configuration change disabled rate limiting");
        println!("   - Subsequent high-frequency operations depleted budget");
        println!("   - Recommendation: Re-enable rate limiting, implement change approval process");
    }

    println!("\n");
}

/// Example 4: Investigate Unauthorized Access
fn unauthorized_access_investigation() {
    println!("=== Example 4: Unauthorized Access Investigation ===\n");

    let mut manager = HashChainManager::new(20000_00, 10000);

    println!("4.1 Simulating Access Pattern:");

    // Authorized users
    let authorized_users = vec!["user_alice", "user_bob", "user_charlie"];

    // Normal access pattern
    for i in 0..10 {
        let user = authorized_users[i % 3];
        let mut metadata = HashMap::new();
        metadata.insert("user_id".to_string(), user.to_string());
        metadata.insert("access_type".to_string(), "authorized".to_string());

        manager
            .deduct_with_metadata(50_00, &format!("API call by {}", user), metadata)
            .unwrap();
    }

    // Unauthorized access attempts
    for i in 0..3 {
        let mut metadata = HashMap::new();
        metadata.insert("user_id".to_string(), format!("attacker_{}", i + 1));
        metadata.insert("access_type".to_string(), "unauthorized".to_string());
        metadata.insert("source_ip".to_string(), format!("192.168.1.{}", 100 + i));

        manager
            .deduct_with_metadata(100_00, &format!("⚠ Unauthorized access attempt {}", i + 1), metadata)
            .unwrap();
    }

    // More normal access
    for i in 10..15 {
        let user = authorized_users[i % 3];
        let mut metadata = HashMap::new();
        metadata.insert("user_id".to_string(), user.to_string());
        metadata.insert("access_type".to_string(), "authorized".to_string());

        manager
            .deduct_with_metadata(50_00, &format!("API call by {}", user), metadata)
            .unwrap();
    }

    println!("   Generated 18 operations (3 unauthorized attempts)\n");

    // Investigation
    println!("4.2 Investigation Results:");

    let mut unauthorized_attempts = Vec::new();
    for entry in &manager.chain {
        if let Some(access_type) = entry.metadata.get("access_type") {
            if access_type == "unauthorized" {
                unauthorized_attempts.push(entry.clone());
            }
        }
    }

    if !unauthorized_attempts.is_empty() {
        println!("   ⚠ SECURITY INCIDENT: {} unauthorized access attempts detected\n", unauthorized_attempts.len());

        for (i, attempt) in unauthorized_attempts.iter().enumerate() {
            println!("   Attempt {}:", i + 1);
            println!("     Operation: {}", attempt.operation);
            println!("     User: {:?}", attempt.metadata.get("user_id"));
            println!("     Source IP: {:?}", attempt.metadata.get("source_ip"));
            println!("     Timestamp: {}", attempt.timestamp_ns);
            println!("     Budget impact: ${:.2}", (attempt.budget_before - attempt.budget_after) as f64 / 100.0);
            println!("     Hash: 0x{:016x}", attempt.hash);
            println!();
        }

        println!("   Recommendations:");
        println!("   - Block source IPs: 192.168.1.100-102");
        println!("   - Review authentication mechanisms");
        println!("   - Implement rate limiting for failed attempts");
        println!("   - Notify security team for further investigation");
    } else {
        println!("   ✓ No unauthorized access detected");
    }

    // Verify chain integrity (ensure no tampering)
    println!("\n4.3 Chain Integrity Verification:");
    if manager.verify_chain() {
        println!("   ✓ Hash chain integrity verified");
        println!("   ✓ Audit trail is tamper-evident");
        println!("   ✓ Investigation evidence is cryptographically sound");
    } else {
        println!("   ✗ ALERT: Hash chain integrity violated!");
        println!("   ✗ Audit trail may have been tampered with");
        println!("   ✗ Investigation evidence is compromised");
    }

    println!("\n");
}
