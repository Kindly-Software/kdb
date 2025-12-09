//! SOC2 Type II CC6.1 Export Functions
//!
//! Generates change control evidence reports with:
//! - Change ticket tracking
//! - Approval timestamps
//! - Observation period coverage
//! - Timestamp monotonicity validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::{ClapiError, ClapiResult};
use super::compliance_capsules::{ComplianceEntry, now_ns};
use super::ComplianceFramework;

/// SOC2 change control record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    /// Change ticket ID
    pub change_ticket: String,
    /// Change description
    pub description: String,
    /// Approver information
    pub approved_by: String,
    /// Approval timestamp (nanoseconds)
    pub approval_timestamp_ns: u64,
    /// Change execution timestamp (nanoseconds)
    pub timestamp_ns: u64,
    /// Entry hash
    pub hash: u64,
    /// Previous entry hash
    pub prev_hash: u64,
}

/// SOC2 Type II compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soc2Report {
    /// Report generation timestamp
    pub generated_at_ns: u64,
    /// Observation period start (nanoseconds)
    pub observation_start_ns: u64,
    /// Observation period end (nanoseconds)
    pub observation_end_ns: u64,
    /// Total change records
    pub total_records: usize,
    /// Change records
    pub change_records: Vec<ChangeRecord>,
    /// Hash chain integrity status
    pub chain_valid: bool,
    /// Timestamp monotonicity status
    pub timestamps_monotonic: bool,
    /// Report metadata
    pub metadata: HashMap<String, String>,
}

/// SOC2 exporter
pub struct Soc2Exporter;

impl Soc2Exporter {
    /// Export SOC2 Type II CC6.1 report
    ///
    /// # Arguments
    /// - `entries`: Compliance entries (filtered for SOC2 framework)
    /// - `observation_start_ns`: Observation period start (nanoseconds)
    /// - `observation_end_ns`: Observation period end (nanoseconds)
    ///
    /// # Performance
    /// - Latency: O(n) where n = number of entries
    /// - Memory: O(n) for record collection
    pub fn export(
        entries: &[ComplianceEntry],
        observation_start_ns: u64,
        observation_end_ns: u64,
    ) -> ClapiResult<Soc2Report> {
        let mut change_records = Vec::with_capacity(entries.len());
        let mut chain_valid = true;
        let mut timestamps_monotonic = true;
        let mut last_timestamp = 0u64;

        // Validate entries are SOC2 framework
        for entry in entries {
            if entry.framework != ComplianceFramework::Soc2TypeII {
                return Err(ClapiError::InvalidRequest {
                    reason: "Non-SOC2 entry found in SOC2 export".to_string(),
                });
            }
        }

        // Filter entries within observation period
        let filtered_entries: Vec<_> = entries
            .iter()
            .filter(|e| e.timestamp_ns >= observation_start_ns && e.timestamp_ns <= observation_end_ns)
            .collect();

        // Convert to change records
        for entry in &filtered_entries {
            let change_ticket = entry
                .metadata
                .iter()
                .find(|(k, _)| k == "change_ticket")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "UNKNOWN".to_string());

            let approved_by = entry
                .metadata
                .iter()
                .find(|(k, _)| k == "approved_by")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "SYSTEM".to_string());

            let approval_timestamp_ns = entry
                .metadata
                .iter()
                .find(|(k, _)| k == "approval_timestamp")
                .and_then(|(_, v)| v.parse::<u64>().ok())
                .unwrap_or(entry.timestamp_ns);

            // Check timestamp monotonicity
            if entry.timestamp_ns < last_timestamp {
                timestamps_monotonic = false;
            }
            last_timestamp = entry.timestamp_ns;

            change_records.push(ChangeRecord {
                change_ticket,
                description: entry.operation.clone(),
                approved_by,
                approval_timestamp_ns,
                timestamp_ns: entry.timestamp_ns,
                hash: entry.hash,
                prev_hash: entry.prev_hash,
            });
        }

        // Verify hash chain integrity
        for i in 1..change_records.len() {
            if change_records[i].prev_hash != change_records[i - 1].hash {
                chain_valid = false;
                break;
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("framework".to_string(), "SOC2-CC6.1".to_string());
        metadata.insert("compliance_standard".to_string(), "SOC2 Type II CC6.1".to_string());
        metadata.insert("observation_period_months".to_string(), "6".to_string());

        Ok(Soc2Report {
            generated_at_ns: now_ns(),
            observation_start_ns,
            observation_end_ns,
            total_records: change_records.len(),
            change_records,
            chain_valid,
            timestamps_monotonic,
            metadata,
        })
    }

    /// Calculate observation period coverage percentage
    pub fn coverage_percentage(report: &Soc2Report) -> f64 {
        if report.total_records == 0 {
            return 0.0;
        }

        let period_duration = report.observation_end_ns - report.observation_start_ns;
        if period_duration == 0 {
            return 0.0;
        }

        // Calculate coverage based on entry distribution
        let first_entry = report.change_records.first().map(|r| r.timestamp_ns).unwrap_or(0);
        let last_entry = report.change_records.last().map(|r| r.timestamp_ns).unwrap_or(0);

        if first_entry == 0 || last_entry == 0 {
            return 0.0;
        }

        let covered_duration = last_entry - first_entry;
        (covered_duration as f64 / period_duration as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_soc2_entry(
        change_ticket: &str,
        approved_by: &str,
        approval_ts: u64,
        execution_ts: u64,
        hash: u64,
        prev_hash: u64,
    ) -> ComplianceEntry {
        ComplianceEntry {
            framework: ComplianceFramework::Soc2TypeII,
            operation: format!("Change {}", change_ticket),
            timestamp_ns: execution_ts,
            hash,
            prev_hash,
            metadata: vec![
                ("change_ticket".to_string(), change_ticket.to_string()),
                ("approved_by".to_string(), approved_by.to_string()),
                ("approval_timestamp".to_string(), approval_ts.to_string()),
            ],
        }
    }

    #[test]
    fn test_soc2_export_basic() {
        let base_ts = now_ns();
        let observation_start = base_ts - (30 * 24 * 3600 * 1_000_000_000u64); // 30 days ago
        let observation_end = base_ts;

        let entries = vec![
            create_soc2_entry("CHG-001", "security@company.com", base_ts - 2000, base_ts - 1000, 0x1111, 0),
            create_soc2_entry("CHG-002", "ops@company.com", base_ts - 1000, base_ts - 500, 0x2222, 0x1111),
        ];

        let report = Soc2Exporter::export(&entries, observation_start, observation_end).unwrap();

        assert_eq!(report.total_records, 2);
        assert!(report.chain_valid);
        assert!(report.timestamps_monotonic);
    }

    #[test]
    fn test_soc2_observation_period_filter() {
        let base_ts = now_ns();
        let observation_start = base_ts;
        let observation_end = base_ts + 1_000_000_000; // 1 second window

        let entries = vec![
            create_soc2_entry("CHG-001", "alice@company.com", base_ts - 2000, base_ts - 1000, 0x1111, 0), // Before window
            create_soc2_entry("CHG-002", "bob@company.com", base_ts, base_ts + 500, 0x2222, 0x1111), // In window
            create_soc2_entry("CHG-003", "charlie@company.com", base_ts + 500, base_ts + 2_000_000_000, 0x3333, 0x2222), // After window
        ];

        let report = Soc2Exporter::export(&entries, observation_start, observation_end).unwrap();

        assert_eq!(report.total_records, 1); // Only entry 2 in window
    }

    #[test]
    fn test_soc2_timestamp_monotonicity() {
        let base_ts = now_ns();
        let observation_start = base_ts;
        let observation_end = base_ts + 10_000;

        let entries = vec![
            create_soc2_entry("CHG-001", "alice@company.com", base_ts, base_ts + 1000, 0x1111, 0),
            create_soc2_entry("CHG-002", "bob@company.com", base_ts, base_ts + 500, 0x2222, 0x1111), // Out of order
            create_soc2_entry("CHG-003", "charlie@company.com", base_ts, base_ts + 2000, 0x3333, 0x2222),
        ];

        let report = Soc2Exporter::export(&entries, observation_start, observation_end).unwrap();

        assert!(!report.timestamps_monotonic); // Should detect non-monotonic timestamps
    }
}
