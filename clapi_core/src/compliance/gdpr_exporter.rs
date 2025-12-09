//! GDPR Article 30 Export Functions
//!
//! Generates data processing activity records with:
//! - Data access logging (Article 15)
//! - Right to be forgotten tracking (Article 17)
//! - Processing purpose documentation (Article 30)
//! - Legal basis tracking

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::{ClapiError, ClapiResult};
use super::compliance_capsules::{ComplianceEntry, now_ns};
use super::ComplianceFramework;

/// GDPR data access log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLog {
    /// User ID (data subject)
    pub user_id: String,
    /// GDPR article (e.g., "15" for right of access)
    pub gdpr_article: String,
    /// Access type (read, modify, delete)
    pub access_type: String,
    /// Accessor (system/service that accessed data)
    pub accessor: String,
    /// Legal basis (consent, legitimate_interest, etc.)
    pub legal_basis: Option<String>,
    /// Processing purpose
    pub purpose: Option<String>,
    /// Access timestamp (nanoseconds)
    pub timestamp_ns: u64,
    /// Entry hash
    pub hash: u64,
    /// Previous entry hash
    pub prev_hash: u64,
}

/// GDPR Right to be Forgotten (RTBF) request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtbfRequest {
    /// User ID (data subject)
    pub user_id: String,
    /// Request ID
    pub request_id: String,
    /// Request timestamp (nanoseconds)
    pub request_timestamp_ns: u64,
    /// Completion timestamp (if processed)
    pub completion_timestamp_ns: Option<u64>,
    /// Status (pending, completed, rejected)
    pub status: String,
    /// Entry hash
    pub hash: u64,
    /// Previous entry hash
    pub prev_hash: u64,
}

/// GDPR Article 30 compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdprReport {
    /// Report generation timestamp
    pub generated_at_ns: u64,
    /// User ID filter (if specified)
    pub user_id_filter: Option<String>,
    /// Total access logs
    pub total_access_logs: usize,
    /// Total RTBF requests
    pub total_rtbf_requests: usize,
    /// Access logs
    pub access_logs: Vec<AccessLog>,
    /// RTBF requests
    pub rtbf_requests: Vec<RtbfRequest>,
    /// Hash chain integrity status
    pub chain_valid: bool,
    /// Report metadata
    pub metadata: HashMap<String, String>,
}

/// GDPR exporter
pub struct GdprExporter;

impl GdprExporter {
    /// Export GDPR Article 30 report
    ///
    /// # Arguments
    /// - `entries`: Compliance entries (filtered for GDPR framework)
    /// - `user_id_filter`: Optional user ID filter
    ///
    /// # Performance
    /// - Latency: O(n) where n = number of entries
    /// - Memory: O(n) for log collection
    pub fn export(entries: &[ComplianceEntry], user_id_filter: Option<&str>) -> ClapiResult<GdprReport> {
        let mut access_logs = Vec::new();
        let mut rtbf_requests = Vec::new();
        let mut chain_valid = true;

        // Validate entries are GDPR framework
        for entry in entries {
            if entry.framework != ComplianceFramework::GdprArticle30 {
                return Err(ClapiError::InvalidRequest {
                    reason: "Non-GDPR entry found in GDPR export".to_string(),
                });
            }
        }

        // Process entries
        for entry in entries {
            let user_id = entry
                .metadata
                .iter()
                .find(|(k, _)| k == "user_id")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "UNKNOWN".to_string());

            // Filter by user ID if specified
            if let Some(filter) = user_id_filter {
                if user_id != filter {
                    continue;
                }
            }

            let gdpr_article = entry
                .metadata
                .iter()
                .find(|(k, _)| k == "gdpr_article")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "UNKNOWN".to_string());

            // Check if this is an RTBF request (Article 17)
            if gdpr_article == "17" {
                let request_id = entry
                    .metadata
                    .iter()
                    .find(|(k, _)| k == "request_id")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| format!("rtbf_{}", entry.hash));

                let status = entry
                    .metadata
                    .iter()
                    .find(|(k, _)| k == "status")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| "pending".to_string());

                let completion_ts = entry
                    .metadata
                    .iter()
                    .find(|(k, _)| k == "completion_timestamp")
                    .and_then(|(_, v)| v.parse::<u64>().ok());

                rtbf_requests.push(RtbfRequest {
                    user_id: user_id.clone(),
                    request_id,
                    request_timestamp_ns: entry.timestamp_ns,
                    completion_timestamp_ns: completion_ts,
                    status,
                    hash: entry.hash,
                    prev_hash: entry.prev_hash,
                });
            } else {
                // Regular access log
                let access_type = entry
                    .metadata
                    .iter()
                    .find(|(k, _)| k == "access_type")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| "read".to_string());

                let accessor = entry
                    .metadata
                    .iter()
                    .find(|(k, _)| k == "accessor")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| "SYSTEM".to_string());

                let legal_basis = entry
                    .metadata
                    .iter()
                    .find(|(k, _)| k == "legal_basis")
                    .map(|(_, v)| v.clone());

                let purpose = entry
                    .metadata
                    .iter()
                    .find(|(k, _)| k == "purpose")
                    .map(|(_, v)| v.clone());

                access_logs.push(AccessLog {
                    user_id: user_id.clone(),
                    gdpr_article,
                    access_type,
                    accessor,
                    legal_basis,
                    purpose,
                    timestamp_ns: entry.timestamp_ns,
                    hash: entry.hash,
                    prev_hash: entry.prev_hash,
                });
            }
        }

        // Verify hash chain integrity (combined access logs + RTBF requests)
        let mut all_hashes: Vec<(u64, u64)> = access_logs
            .iter()
            .map(|log| (log.hash, log.prev_hash))
            .chain(rtbf_requests.iter().map(|req| (req.hash, req.prev_hash)))
            .collect();
        all_hashes.sort_by_key(|(hash, _)| *hash);

        for i in 1..all_hashes.len() {
            if all_hashes[i].1 != all_hashes[i - 1].0 {
                chain_valid = false;
                break;
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("framework".to_string(), "GDPR-30".to_string());
        metadata.insert("compliance_standard".to_string(), "GDPR Article 30".to_string());
        metadata.insert("data_controller".to_string(), "Organization Name".to_string());

        Ok(GdprReport {
            generated_at_ns: now_ns(),
            user_id_filter: user_id_filter.map(|s| s.to_string()),
            total_access_logs: access_logs.len(),
            total_rtbf_requests: rtbf_requests.len(),
            access_logs,
            rtbf_requests,
            chain_valid,
            metadata,
        })
    }

    /// Summarize access by user
    pub fn summarize_by_user(report: &GdprReport) -> HashMap<String, usize> {
        let mut summary = HashMap::new();

        for log in &report.access_logs {
            *summary.entry(log.user_id.clone()).or_insert(0) += 1;
        }

        summary
    }

    /// Summarize RTBF requests by status
    pub fn summarize_rtbf_by_status(report: &GdprReport) -> HashMap<String, usize> {
        let mut summary = HashMap::new();

        for req in &report.rtbf_requests {
            *summary.entry(req.status.clone()).or_insert(0) += 1;
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_gdpr_access_entry(
        user_id: &str,
        article: &str,
        access_type: &str,
        accessor: &str,
        hash: u64,
        prev_hash: u64,
    ) -> ComplianceEntry {
        ComplianceEntry {
            framework: ComplianceFramework::GdprArticle30,
            operation: format!("GDPR: User {} {}", user_id, access_type),
            timestamp_ns: now_ns(),
            hash,
            prev_hash,
            metadata: vec![
                ("user_id".to_string(), user_id.to_string()),
                ("gdpr_article".to_string(), article.to_string()),
                ("access_type".to_string(), access_type.to_string()),
                ("accessor".to_string(), accessor.to_string()),
            ],
        }
    }

    fn create_gdpr_rtbf_entry(user_id: &str, request_id: &str, status: &str, hash: u64, prev_hash: u64) -> ComplianceEntry {
        ComplianceEntry {
            framework: ComplianceFramework::GdprArticle30,
            operation: format!("GDPR: RTBF request {}", request_id),
            timestamp_ns: now_ns(),
            hash,
            prev_hash,
            metadata: vec![
                ("user_id".to_string(), user_id.to_string()),
                ("gdpr_article".to_string(), "17".to_string()),
                ("request_id".to_string(), request_id.to_string()),
                ("status".to_string(), status.to_string()),
            ],
        }
    }

    #[test]
    fn test_gdpr_export_basic() {
        let entries = vec![
            create_gdpr_access_entry("user_123", "15", "read", "api_service", 0x1111, 0),
            create_gdpr_access_entry("user_123", "15", "modify", "web_app", 0x2222, 0x1111),
        ];

        let report = GdprExporter::export(&entries, None).unwrap();

        assert_eq!(report.total_access_logs, 2);
        assert_eq!(report.total_rtbf_requests, 0);
        assert_eq!(report.user_id_filter, None);
    }

    #[test]
    fn test_gdpr_export_user_filter() {
        let entries = vec![
            create_gdpr_access_entry("user_123", "15", "read", "api_service", 0x1111, 0),
            create_gdpr_access_entry("user_456", "15", "read", "web_app", 0x2222, 0x1111),
            create_gdpr_access_entry("user_123", "15", "modify", "api_service", 0x3333, 0x2222),
        ];

        let report = GdprExporter::export(&entries, Some("user_123")).unwrap();

        assert_eq!(report.total_access_logs, 2); // Only user_123 entries
        assert_eq!(report.user_id_filter, Some("user_123".to_string()));
    }

    #[test]
    fn test_gdpr_rtbf_requests() {
        let entries = vec![
            create_gdpr_access_entry("user_123", "15", "read", "api_service", 0x1111, 0),
            create_gdpr_rtbf_entry("user_123", "rtbf_2025_001", "pending", 0x2222, 0x1111),
            create_gdpr_rtbf_entry("user_456", "rtbf_2025_002", "completed", 0x3333, 0x2222),
        ];

        let report = GdprExporter::export(&entries, None).unwrap();

        assert_eq!(report.total_access_logs, 1);
        assert_eq!(report.total_rtbf_requests, 2);

        let summary = GdprExporter::summarize_rtbf_by_status(&report);
        assert_eq!(summary["pending"], 1);
        assert_eq!(summary["completed"], 1);
    }

    #[test]
    fn test_gdpr_summarize_by_user() {
        let entries = vec![
            create_gdpr_access_entry("user_123", "15", "read", "api_service", 0x1111, 0),
            create_gdpr_access_entry("user_123", "15", "modify", "web_app", 0x2222, 0x1111),
            create_gdpr_access_entry("user_456", "15", "read", "api_service", 0x3333, 0x2222),
        ];

        let report = GdprExporter::export(&entries, None).unwrap();
        let summary = GdprExporter::summarize_by_user(&report);

        assert_eq!(summary["user_123"], 2);
        assert_eq!(summary["user_456"], 1);
    }
}
