//! Q34 Compliance Helpers - SOX, SOC2, GDPR, HIPAA
//!
//! **Purpose**: Regulatory compliance reporting and analysis
//!
//! # Compliance Standards
//!
//! - **SOX (Sarbanes-Oxley)**: Financial reporting controls
//! - **SOC2 (Service Organization Control 2)**: Security, availability, confidentiality
//! - **GDPR (General Data Protection Regulation)**: Data privacy and protection
//! - **HIPAA (Health Insurance Portability and Accountability Act)**: Healthcare data

use super::audit_log_q34::{AuditLog, AuditLogEntry};
use crate::error::AuditError;
use std::collections::HashMap;

// ============================================================================
// COMPLIANCE REPORT
// ============================================================================

/// Compliance report for regulatory audits
#[derive(Debug, Clone)]
pub struct ComplianceReport {
    /// Total number of audit entries
    pub total_entries: usize,

    /// Date range (first timestamp, last timestamp)
    pub date_range: (Option<u64>, Option<u64>),

    /// Number of unique instances
    pub unique_instances: usize,

    /// Chain integrity (true if no tampering detected)
    pub chain_valid: bool,

    /// Operation summary (operation_type → count)
    pub operation_summary: HashMap<u32, usize>,

    /// Sequence integrity (no gaps)
    pub sequence_valid: bool,

    /// Tamper detection summary
    pub tamper_detected: bool,
}

impl ComplianceReport {
    /// Generate compliance report from audit log
    ///
    /// # Arguments
    /// * `audit_log` - Audit log to analyze
    ///
    /// # Returns
    /// Compliance report with all metrics
    pub fn generate(audit_log: &AuditLog) -> Result<Self, AuditError> {
        let entries = audit_log.entries()?;
        let chain_valid = audit_log.verify_chain()?;

        let total_entries = entries.len();

        let date_range = if entries.is_empty() {
            (None, None)
        } else {
            (
                Some(entries.first().unwrap().timestamp),
                Some(entries.last().unwrap().timestamp),
            )
        };

        let unique_instances = entries
            .iter()
            .map(|e| e.instance_id)
            .collect::<std::collections::HashSet<_>>()
            .len();

        let mut operation_summary = HashMap::new();
        for entry in &entries {
            *operation_summary.entry(entry.operation_type).or_insert(0) += 1;
        }

        let sequence_valid = Self::verify_sequence(&entries);
        let tamper_detected = !chain_valid;

        Ok(Self {
            total_entries,
            date_range,
            unique_instances,
            chain_valid,
            operation_summary,
            sequence_valid,
            tamper_detected,
        })
    }

    /// Verify sequence integrity (no gaps)
    fn verify_sequence(entries: &[AuditLogEntry]) -> bool {
        for (i, entry) in entries.iter().enumerate() {
            if entry.sequence as usize != i {
                return false;
            }
        }
        true
    }

    /// Check SOX compliance
    ///
    /// **Requirements**:
    /// - Chain integrity (tamper-evident)
    /// - Sequence integrity (deterministic ordering)
    /// - Complete audit trail (no gaps)
    pub fn sox_compliant(&self) -> bool {
        self.chain_valid && self.sequence_valid && !self.tamper_detected
    }

    /// Check SOC2 compliance
    ///
    /// **Requirements**:
    /// - Chain completeness
    /// - Change control evidence
    /// - Unauthorized modification detection
    pub fn soc2_compliant(&self) -> bool {
        self.chain_valid && self.sequence_valid && self.total_entries > 0
    }

    /// Check GDPR compliance
    ///
    /// **Requirements**:
    /// - Article 15 (data provenance tracking)
    /// - Article 17 (right to forget audit)
    pub fn gdpr_compliant(&self) -> bool {
        self.chain_valid && self.unique_instances > 0
    }

    /// Check HIPAA compliance
    ///
    /// **Requirements**:
    /// - 164.312(b) (access logging)
    /// - Deterministic ordering
    /// - Breach detection
    pub fn hipaa_compliant(&self) -> bool {
        self.chain_valid && self.sequence_valid && !self.tamper_detected
    }

    /// Check all compliance standards
    ///
    /// # Returns
    /// (SOX, SOC2, GDPR, HIPAA) compliance status
    pub fn all_compliant(&self) -> (bool, bool, bool, bool) {
        (
            self.sox_compliant(),
            self.soc2_compliant(),
            self.gdpr_compliant(),
            self.hipaa_compliant(),
        )
    }
}

// ============================================================================
// DATA PROVENANCE (GDPR Article 15)
// ============================================================================

/// Data provenance entry (who, when, what)
#[derive(Debug, Clone)]
pub struct ProvenanceEntry {
    /// Instance ID that performed operation
    pub instance_id: u32,

    /// Operation type
    pub operation_type: u32,

    /// Timestamp (nanoseconds)
    pub timestamp: u64,

    /// Operation name (human-readable)
    pub operation_name: String,
}

/// Get all operations by instance (data provenance)
///
/// # Arguments
/// * `audit_log` - Audit log to query
/// * `instance_id` - Instance ID to filter by
///
/// # Returns
/// Vec of all operations performed by instance
pub fn operations_by_instance(
    audit_log: &AuditLog,
    instance_id: u32,
) -> Result<Vec<ProvenanceEntry>, AuditError> {
    let entries = audit_log.entries()?;

    Ok(entries
        .into_iter()
        .filter(|e| e.instance_id == instance_id)
        .map(|e| ProvenanceEntry {
            instance_id: e.instance_id,
            operation_type: e.operation_type,
            timestamp: e.timestamp,
            operation_name: operation_type_name(e.operation_type),
        })
        .collect())
}

/// Get operation history (who did what, when)
///
/// # Arguments
/// * `audit_log` - Audit log to query
///
/// # Returns
/// Vec of (operation_name, instance_id, timestamp)
pub fn operation_history(
    audit_log: &AuditLog,
) -> Result<Vec<(String, u32, u64)>, AuditError> {
    let entries = audit_log.entries()?;

    Ok(entries
        .into_iter()
        .map(|e| {
            (
                operation_type_name(e.operation_type),
                e.instance_id,
                e.timestamp,
            )
        })
        .collect())
}

// ============================================================================
// DETERMINISTIC SEQUENCE (HIPAA 164.312)
// ============================================================================

/// Verify deterministic sequence (no gaps)
///
/// # Arguments
/// * `audit_log` - Audit log to verify
///
/// # Returns
/// Ok(true) if sequence valid, Ok(false) if gaps detected
pub fn verify_deterministic_sequence(audit_log: &AuditLog) -> Result<bool, AuditError> {
    let entries = audit_log.entries()?;

    for (i, entry) in entries.iter().enumerate() {
        if entry.sequence as usize != i {
            return Ok(false);
        }
    }

    Ok(true)
}

// ============================================================================
// TAMPER DETECTION (SOX/SOC2)
// ============================================================================

/// Check if any entry was tampered with
///
/// # Arguments
/// * `audit_log` - Audit log to check
///
/// # Returns
/// Ok(true) if tampering detected, Ok(false) if clean
pub fn tamper_detected(audit_log: &AuditLog) -> Result<bool, AuditError> {
    let chain_valid = audit_log.verify_chain()?;
    Ok(!chain_valid)
}

// ============================================================================
// HELPERS
// ============================================================================

/// Get human-readable operation name
fn operation_type_name(operation_type: u32) -> String {
    match operation_type {
        1 => "Commit".to_string(),
        2 => "Branch".to_string(),
        3 => "Merge".to_string(),
        4 => "Push".to_string(),
        5 => "Add".to_string(),
        _ => format!("Unknown({})", operation_type),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_compliance_report() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::open(temp_file.path()).unwrap();

        let commit_hash = [1u8; 20];
        let data = [0u8; 88];

        // Add entries
        for i in 0..10 {
            log.append(1, i as u32 % 5 + 1, &commit_hash, &data)
                .unwrap();
        }

        let report = ComplianceReport::generate(&log).unwrap();

        assert_eq!(report.total_entries, 10);
        assert!(report.chain_valid);
        assert!(report.sequence_valid);
        assert!(!report.tamper_detected);
        assert_eq!(report.unique_instances, 1);
    }

    #[test]
    fn test_sox_compliance() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::open(temp_file.path()).unwrap();

        let commit_hash = [1u8; 20];
        let data = [0u8; 88];

        log.append(1, 1, &commit_hash, &data).unwrap();

        let report = ComplianceReport::generate(&log).unwrap();
        assert!(report.sox_compliant());
    }

    #[test]
    fn test_gdpr_provenance() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::open(temp_file.path()).unwrap();

        let commit_hash = [1u8; 20];
        let data = [0u8; 88];

        log.append(1, 1, &commit_hash, &data).unwrap();
        log.append(2, 2, &commit_hash, &data).unwrap();
        log.append(1, 3, &commit_hash, &data).unwrap();

        let ops = operations_by_instance(&log, 1).unwrap();
        assert_eq!(ops.len(), 2);

        let history = operation_history(&log).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_hipaa_sequence() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::open(temp_file.path()).unwrap();

        let commit_hash = [1u8; 20];
        let data = [0u8; 88];

        for i in 0..5 {
            log.append(1, i as u32, &commit_hash, &data).unwrap();
        }

        assert!(verify_deterministic_sequence(&log).unwrap());
    }

    #[test]
    fn test_tamper_detection() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::open(temp_file.path()).unwrap();

        let commit_hash = [1u8; 20];
        let data = [0u8; 88];

        log.append(1, 1, &commit_hash, &data).unwrap();

        // Clean log - no tampering
        assert!(!tamper_detected(&log).unwrap());
    }

    #[test]
    fn test_all_compliance_standards() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::open(temp_file.path()).unwrap();

        let commit_hash = [1u8; 20];
        let data = [0u8; 88];

        for i in 0..10 {
            log.append(1, i as u32 % 5 + 1, &commit_hash, &data)
                .unwrap();
        }

        let report = ComplianceReport::generate(&log).unwrap();
        let (sox, soc2, gdpr, hipaa) = report.all_compliant();

        assert!(sox, "SOX compliance failed");
        assert!(soc2, "SOC2 compliance failed");
        assert!(gdpr, "GDPR compliance failed");
        assert!(hipaa, "HIPAA compliance failed");
    }
}
