//! SOX (Sarbanes-Oxley) Section 404 Export Functions
//!
//! Generates audit trail reports for SOX compliance with:
//! - GL (General Ledger) codes
//! - Approver information
//! - Fiscal year tracking
//! - Transaction integrity validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::{ClapiError, ClapiResult};
use super::compliance_capsules::{ComplianceEntry, now_ns};
use super::ComplianceFramework;

/// SOX General Ledger entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlEntry {
    /// GL account code (e.g., "4100" for SaaS expenses)
    pub gl_code: String,
    /// Transaction description
    pub description: String,
    /// Amount in cents
    pub amount_cents: i64,
    /// Approver email
    pub approver: String,
    /// Fiscal year
    pub fiscal_year: u16,
    /// Transaction timestamp (nanoseconds)
    pub timestamp_ns: u64,
    /// Entry hash (tamper detection)
    pub hash: u64,
    /// Previous entry hash (chain link)
    pub prev_hash: u64,
}

/// SOX 404 compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoxReport {
    /// Report generation timestamp
    pub generated_at_ns: u64,
    /// Fiscal year filter (if applicable)
    pub fiscal_year: Option<u16>,
    /// Total entries in report
    pub total_entries: usize,
    /// Total transaction amount (cents)
    pub total_amount_cents: i64,
    /// GL entries grouped by account code
    pub gl_entries: Vec<GlEntry>,
    /// Hash chain integrity status
    pub chain_valid: bool,
    /// Report metadata
    pub metadata: HashMap<String, String>,
}

/// SOX exporter
pub struct SoxExporter;

impl SoxExporter {
    /// Export SOX 404 report from compliance entries
    ///
    /// # Arguments
    /// - `entries`: Compliance entries (filtered for SOX framework)
    /// - `fiscal_year`: Optional fiscal year filter
    ///
    /// # Performance
    /// - Latency: O(n) where n = number of entries
    /// - Memory: O(n) for GL entry collection
    pub fn export(entries: &[ComplianceEntry], fiscal_year: Option<u16>) -> ClapiResult<SoxReport> {
        let mut gl_entries = Vec::with_capacity(entries.len());
        let mut total_amount_cents = 0i64;
        let mut chain_valid = true;

        // Validate entries are SOX framework
        for entry in entries {
            if entry.framework != ComplianceFramework::Sox404 {
                return Err(ClapiError::InvalidRequest {
                    reason: "Non-SOX entry found in SOX export".to_string(),
                });
            }
        }

        // Convert compliance entries to GL entries
        for entry in entries {
            let gl_code = entry
                .metadata
                .iter()
                .find(|(k, _)| k == "gl_code")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "UNKNOWN".to_string());

            let approver = entry
                .metadata
                .iter()
                .find(|(k, _)| k == "approver")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "SYSTEM".to_string());

            let fy = entry
                .metadata
                .iter()
                .find(|(k, _)| k == "fiscal_year")
                .and_then(|(_, v)| v.parse::<u16>().ok())
                .unwrap_or(2025);

            // Filter by fiscal year if specified
            if let Some(filter_fy) = fiscal_year {
                if fy != filter_fy {
                    continue;
                }
            }

            let amount_cents = entry
                .metadata
                .iter()
                .find(|(k, _)| k == "amount_cents")
                .and_then(|(_, v)| v.parse::<i64>().ok())
                .unwrap_or(0);

            total_amount_cents += amount_cents;

            gl_entries.push(GlEntry {
                gl_code,
                description: entry.operation.clone(),
                amount_cents,
                approver,
                fiscal_year: fy,
                timestamp_ns: entry.timestamp_ns,
                hash: entry.hash,
                prev_hash: entry.prev_hash,
            });
        }

        // Verify hash chain integrity
        for i in 1..gl_entries.len() {
            if gl_entries[i].prev_hash != gl_entries[i - 1].hash {
                chain_valid = false;
                break;
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("framework".to_string(), "SOX-404".to_string());
        metadata.insert("compliance_standard".to_string(), "Sarbanes-Oxley Section 404".to_string());

        Ok(SoxReport {
            generated_at_ns: now_ns(),
            fiscal_year,
            total_entries: gl_entries.len(),
            total_amount_cents,
            gl_entries,
            chain_valid,
            metadata,
        })
    }

    /// Generate summary statistics by GL code
    pub fn summarize_by_gl_code(report: &SoxReport) -> HashMap<String, (usize, i64)> {
        let mut summary = HashMap::new();

        for entry in &report.gl_entries {
            let stats = summary.entry(entry.gl_code.clone()).or_insert((0, 0));
            stats.0 += 1; // Count
            stats.1 += entry.amount_cents; // Total amount
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(gl_code: &str, amount_cents: i64, approver: &str, fiscal_year: u16, hash: u64, prev_hash: u64) -> ComplianceEntry {
        let metadata = vec![
            ("gl_code".to_string(), gl_code.to_string()),
            ("approver".to_string(), approver.to_string()),
            ("fiscal_year".to_string(), fiscal_year.to_string()),
            ("amount_cents".to_string(), amount_cents.to_string()),
        ];

        ComplianceEntry {
            framework: ComplianceFramework::Sox404,
            operation: format!("Transaction - GL {}", gl_code),
            timestamp_ns: now_ns(),
            hash,
            prev_hash,
            metadata,
        }
    }

    #[test]
    fn test_sox_export_basic() {
        let entries = vec![
            create_test_entry("4100", 250_00, "john@company.com", 2025, 0x1111, 0),
            create_test_entry("4200", 150_00, "jane@company.com", 2025, 0x2222, 0x1111),
        ];

        let report = SoxExporter::export(&entries, None).unwrap();

        assert_eq!(report.total_entries, 2);
        assert_eq!(report.total_amount_cents, 400_00);
        assert!(report.chain_valid);
        assert_eq!(report.fiscal_year, None);
    }

    #[test]
    fn test_sox_export_fiscal_year_filter() {
        let entries = vec![
            create_test_entry("4100", 100_00, "alice@company.com", 2024, 0x1111, 0),
            create_test_entry("4200", 200_00, "bob@company.com", 2025, 0x2222, 0x1111),
            create_test_entry("4300", 300_00, "charlie@company.com", 2025, 0x3333, 0x2222),
        ];

        let report = SoxExporter::export(&entries, Some(2025)).unwrap();

        assert_eq!(report.total_entries, 2); // Only 2025 entries
        assert_eq!(report.total_amount_cents, 500_00);
        assert_eq!(report.fiscal_year, Some(2025));
    }

    #[test]
    fn test_sox_summarize_by_gl_code() {
        let entries = vec![
            create_test_entry("4100", 100_00, "alice@company.com", 2025, 0x1111, 0),
            create_test_entry("4100", 200_00, "bob@company.com", 2025, 0x2222, 0x1111),
            create_test_entry("4200", 150_00, "charlie@company.com", 2025, 0x3333, 0x2222),
        ];

        let report = SoxExporter::export(&entries, None).unwrap();
        let summary = SoxExporter::summarize_by_gl_code(&report);

        assert_eq!(summary.len(), 2);
        assert_eq!(summary["4100"], (2, 300_00)); // 2 entries, $300
        assert_eq!(summary["4200"], (1, 150_00)); // 1 entry, $150
    }

    #[test]
    fn test_sox_chain_validation() {
        let entries = vec![
            create_test_entry("4100", 100_00, "alice@company.com", 2025, 0x1111, 0),
            create_test_entry("4200", 200_00, "bob@company.com", 2025, 0x2222, 0x1111),
            create_test_entry("4300", 300_00, "charlie@company.com", 2025, 0x3333, 0x9999), // Broken chain
        ];

        let report = SoxExporter::export(&entries, None).unwrap();

        assert!(!report.chain_valid); // Should detect broken chain
    }
}
