//! Audit Trail Export Formats - JSON, CSV, Binary
//!
//! Provides serialization of ComplianceAuditCapsule events for compliance reporting.
//!
//! # Supported Formats
//! - **JSON**: Full metadata, human-readable, auditor-friendly
//! - **CSV**: Spreadsheet-compatible, Excel/LibreOffice
//! - **Binary**: Compact, machine-readable (future)
//!
//! # Performance Targets (B32)
//! - JSON: <100μs per entry (serde_json)
//! - CSV: <50μs per entry (manual formatting)
//! - Export iteration: O(1) memory (streaming)

use crate::error::{ClapiError, ClapiResult};
use super::audit_capsule::*;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Export format enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditExportFormat {
    /// JSON format (default, full metadata)
    Json,
    /// CSV format (spreadsheet-compatible)
    Csv,
    /// Binary format (compact, bincode) - future
    Binary,
}

impl AuditExportFormat {
    /// Get MIME type for format
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Csv => "text/csv",
            Self::Binary => "application/octet-stream",
        }
    }

    /// Get file extension for format
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Binary => "bin",
        }
    }
}

/// Audit trail export report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrailReport {
    /// Export metadata
    pub generated_at_ns: u64,
    pub total_events: usize,
    pub chain_valid: bool,
    pub generation: u64,

    /// Events in chronological order
    pub events: Vec<AuditEventExport>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Serializable audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventExport {
    pub timestamp_ns: u64,
    pub timestamp_iso: String,
    pub user_id: u64,
    pub event_type: String,
    pub status: String,
    pub amount_cents: i64,
    pub prev_hash: String,
    pub curr_hash: String,
}

impl From<&AuditEvent> for AuditEventExport {
    fn from(event: &AuditEvent) -> Self {
        // Convert timestamp to ISO 8601
        let timestamp_iso = timestamp_to_iso(event.timestamp_ns);

        // Convert event type
        let event_type = AuditEventType::from_u8(event.event_type)
            .map(|t| t.as_str())
            .unwrap_or("Unknown")
            .to_string();

        // Convert status
        let status = match AuditEventStatus::from_u8(event.status) {
            Some(AuditEventStatus::Success) => "Success",
            Some(AuditEventStatus::Failure) => "Failure",
            Some(AuditEventStatus::Pending) => "Pending",
            None => "Unknown",
        }.to_string();

        Self {
            timestamp_ns: event.timestamp_ns,
            timestamp_iso,
            user_id: event.user_id,
            event_type,
            status,
            amount_cents: event.amount_cents,
            prev_hash: format!("0x{:016x}", event.prev_hash),
            curr_hash: format!("0x{:016x}", event.curr_hash),
        }
    }
}

/// JSON exporter
pub struct JsonExporter;

impl JsonExporter {
    /// Export audit trail as JSON
    ///
    /// # Performance
    /// - Target: <100μs per entry
    /// - Format: Pretty-printed JSON
    pub fn export(capsule: &ComplianceAuditCapsule) -> ClapiResult<String> {
        let events = capsule.get_events();
        let chain_valid = capsule.verify_integrity();

        let report = AuditTrailReport {
            generated_at_ns: now_ns(),
            total_events: events.len(),
            chain_valid,
            generation: capsule.generation(),
            events: events.iter().map(|e| AuditEventExport::from(e)).collect(),
            metadata: HashMap::new(),
        };

        serde_json::to_string_pretty(&report)
            .map_err(|e| ClapiError::JsonError(format!("JSON serialization failed: {}", e)))
    }

    /// Export with custom metadata
    pub fn export_with_metadata(
        capsule: &ComplianceAuditCapsule,
        metadata: HashMap<String, String>,
    ) -> ClapiResult<String> {
        let events = capsule.get_events();
        let chain_valid = capsule.verify_integrity();

        let report = AuditTrailReport {
            generated_at_ns: now_ns(),
            total_events: events.len(),
            chain_valid,
            generation: capsule.generation(),
            events: events.iter().map(|e| AuditEventExport::from(e)).collect(),
            metadata,
        };

        serde_json::to_string_pretty(&report)
            .map_err(|e| ClapiError::JsonError(format!("JSON serialization failed: {}", e)))
    }
}

/// CSV exporter
pub struct CsvExporter;

impl CsvExporter {
    /// Export audit trail as CSV
    ///
    /// # Performance
    /// - Target: <50μs per entry
    /// - Format: RFC 4180 compliant CSV
    ///
    /// # Security
    /// - CSV formula injection prevention (prefix dangerous chars with ')
    pub fn export(capsule: &ComplianceAuditCapsule) -> ClapiResult<String> {
        let events = capsule.get_events();
        let mut output = String::new();

        // Header
        output.push_str("Timestamp (ns),Timestamp (ISO),User ID,Event Type,Status,Amount (cents),Prev Hash,Curr Hash\n");

        // Rows
        for event in &events {
            let export_event = AuditEventExport::from(event);
            output.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                export_event.timestamp_ns,
                Self::escape_csv(&export_event.timestamp_iso),
                export_event.user_id,
                Self::escape_csv(&export_event.event_type),
                Self::escape_csv(&export_event.status),
                export_event.amount_cents,
                Self::escape_csv(&export_event.prev_hash),
                Self::escape_csv(&export_event.curr_hash),
            ));
        }

        Ok(output)
    }

    /// Escape CSV field (RFC 4180 + formula injection prevention)
    ///
    /// # Security (OWASP A03:2021 – Injection Prevention)
    /// Prevents CSV formula injection by prefixing dangerous characters with single quote.
    fn escape_csv(field: &str) -> String {
        // SECURITY: Prevent CSV formula injection (CVE-level vulnerability)
        // Excel, LibreOffice, Google Sheets interpret =, +, -, @, \t, \r as formulas
        let sanitized = if field.starts_with('=') || field.starts_with('+')
                        || field.starts_with('-') || field.starts_with('@')
                        || field.starts_with('\t') || field.starts_with('\r') {
            format!("'{}", field)  // Prefix with ' to force literal interpretation
        } else {
            field.to_string()
        };

        if sanitized.contains(',') || sanitized.contains('"') || sanitized.contains('\n') {
            format!("\"{}\"", sanitized.replace('"', "\"\""))
        } else {
            sanitized
        }
    }
}

/// User-specific audit export
pub struct UserAuditExporter;

impl UserAuditExporter {
    /// Export audit trail for specific user
    pub fn export_json(capsule: &ComplianceAuditCapsule, user_id: u64) -> ClapiResult<String> {
        let all_events = capsule.get_events();
        let user_events: Vec<_> = all_events.iter()
            .filter(|e| e.user_id == user_id)
            .collect();

        let report = AuditTrailReport {
            generated_at_ns: now_ns(),
            total_events: user_events.len(),
            chain_valid: capsule.verify_integrity(),
            generation: capsule.generation(),
            events: user_events.iter().map(|e| AuditEventExport::from(*e)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("user_id".to_string(), user_id.to_string());
                map
            },
        };

        serde_json::to_string_pretty(&report)
            .map_err(|e| ClapiError::JsonError(format!("JSON serialization failed: {}", e)))
    }

    /// Export user audit trail as CSV
    pub fn export_csv(capsule: &ComplianceAuditCapsule, user_id: u64) -> ClapiResult<String> {
        let all_events = capsule.get_events();
        let user_events: Vec<_> = all_events.iter()
            .filter(|e| e.user_id == user_id)
            .collect();

        let mut output = String::new();

        // Header
        output.push_str("Timestamp (ns),Timestamp (ISO),Event Type,Status,Amount (cents),Prev Hash,Curr Hash\n");

        // Rows
        for event in &user_events {
            let export_event = AuditEventExport::from(*event);
            output.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                export_event.timestamp_ns,
                CsvExporter::escape_csv(&export_event.timestamp_iso),
                CsvExporter::escape_csv(&export_event.event_type),
                CsvExporter::escape_csv(&export_event.status),
                export_event.amount_cents,
                CsvExporter::escape_csv(&export_event.prev_hash),
                CsvExporter::escape_csv(&export_event.curr_hash),
            ));
        }

        Ok(output)
    }
}

/// Compliance framework-specific export
pub struct ComplianceExporter;

impl ComplianceExporter {
    /// Export SOX 404 compliant audit trail
    pub fn export_sox404(capsule: &ComplianceAuditCapsule) -> ClapiResult<String> {
        let mut metadata = HashMap::new();
        metadata.insert("compliance_framework".to_string(), "SOX 404".to_string());
        metadata.insert("requirement".to_string(), "User access control and authorization tracking".to_string());

        JsonExporter::export_with_metadata(capsule, metadata)
    }

    /// Export SOC2 Type II compliant audit trail
    pub fn export_soc2(capsule: &ComplianceAuditCapsule) -> ClapiResult<String> {
        let mut metadata = HashMap::new();
        metadata.insert("compliance_framework".to_string(), "SOC2 Type II".to_string());
        metadata.insert("requirement".to_string(), "Audit trail availability and data protection".to_string());

        JsonExporter::export_with_metadata(capsule, metadata)
    }

    /// Export GDPR Article 30 compliant audit trail
    pub fn export_gdpr_article30(capsule: &ComplianceAuditCapsule) -> ClapiResult<String> {
        let mut metadata = HashMap::new();
        metadata.insert("compliance_framework".to_string(), "GDPR Article 30".to_string());
        metadata.insert("requirement".to_string(), "Processing activity records and data subject rights".to_string());

        JsonExporter::export_with_metadata(capsule, metadata)
    }
}

// Helper: Convert timestamp to ISO 8601
fn timestamp_to_iso(timestamp_ns: u64) -> String {
    use std::time::Duration;

    let duration = Duration::from_nanos(timestamp_ns);

    // Convert to ISO 8601 format
    // For simplicity, using a basic implementation
    // Production code should use chrono crate
    let secs = duration.as_secs();
    let nanos = duration.subsec_nanos();

    // Basic ISO 8601: YYYY-MM-DDTHH:MM:SS.sssZ
    format!("{}.{:09}Z", secs, nanos)
}

// Helper: Get current timestamp
#[inline]
fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_export_empty() {
        let capsule = ComplianceAuditCapsule::new();
        let json = JsonExporter::export(&capsule).unwrap();
        assert!(json.contains("\"total_events\": 0"));
        assert!(json.contains("\"chain_valid\": true"));
    }

    #[test]
    fn test_json_export_with_events() {
        let mut capsule = ComplianceAuditCapsule::new();
        capsule.log_login(100, true);
        capsule.log_payment(100, 5000, AuditEventStatus::Success);
        capsule.log_export(100, true);

        let json = JsonExporter::export(&capsule).unwrap();
        assert!(json.contains("\"total_events\": 3"));
        assert!(json.contains("\"user_id\": 100"));
        assert!(json.contains("\"event_type\": \"Login\""));
        assert!(json.contains("\"event_type\": \"Payment\""));
        assert!(json.contains("\"event_type\": \"Export\""));
    }

    #[test]
    fn test_csv_export_empty() {
        let capsule = ComplianceAuditCapsule::new();
        let csv = CsvExporter::export(&capsule).unwrap();
        assert!(csv.contains("Timestamp (ns),Timestamp (ISO),User ID"));
        // Should only have header
        assert_eq!(csv.lines().count(), 1);
    }

    #[test]
    fn test_csv_export_with_events() {
        let mut capsule = ComplianceAuditCapsule::new();
        capsule.log_login(100, true);
        capsule.log_payment(100, 5000, AuditEventStatus::Success);

        let csv = CsvExporter::export(&capsule).unwrap();
        assert!(csv.contains("User ID"));
        assert!(csv.contains("100,Login,Success"));
        assert!(csv.contains("100,Payment,Success,5000"));
    }

    #[test]
    fn test_csv_escape_formulas() {
        assert_eq!(CsvExporter::escape_csv("=SUM(A1:A10)"), "'=SUM(A1:A10)");
        assert_eq!(CsvExporter::escape_csv("+1234"), "'+1234");
        assert_eq!(CsvExporter::escape_csv("-1234"), "'-1234");
        assert_eq!(CsvExporter::escape_csv("@user"), "'@user");
    }

    #[test]
    fn test_csv_escape_quotes() {
        assert_eq!(CsvExporter::escape_csv("simple"), "simple");
        assert_eq!(CsvExporter::escape_csv("with,comma"), "\"with,comma\"");
        assert_eq!(CsvExporter::escape_csv("with\"quote"), "\"with\"\"quote\"");
    }

    #[test]
    fn test_user_audit_export() {
        let mut capsule = ComplianceAuditCapsule::new();

        // User 100 events
        capsule.log_login(100, true);
        capsule.log_payment(100, 5000, AuditEventStatus::Success);

        // User 200 events
        capsule.log_login(200, true);

        let json = UserAuditExporter::export_json(&capsule, 100).unwrap();
        assert!(json.contains("\"total_events\": 2"));
        assert!(json.contains("\"user_id\": \"100\""));
    }

    #[test]
    fn test_compliance_export_sox404() {
        let mut capsule = ComplianceAuditCapsule::new();
        capsule.log_login(100, true);
        capsule.log_permission_change(100, true);

        let json = ComplianceExporter::export_sox404(&capsule).unwrap();
        assert!(json.contains("SOX 404"));
        assert!(json.contains("User access control"));
    }

    #[test]
    fn test_compliance_export_soc2() {
        let mut capsule = ComplianceAuditCapsule::new();
        capsule.log_access(100, true);
        capsule.log_export(100, true);

        let json = ComplianceExporter::export_soc2(&capsule).unwrap();
        assert!(json.contains("SOC2 Type II"));
        assert!(json.contains("Audit trail availability"));
    }

    #[test]
    fn test_compliance_export_gdpr() {
        let mut capsule = ComplianceAuditCapsule::new();
        capsule.log_access(100, true);
        capsule.log_export(100, true);

        let json = ComplianceExporter::export_gdpr_article30(&capsule).unwrap();
        assert!(json.contains("GDPR Article 30"));
        assert!(json.contains("Processing activity records"));
    }

    #[test]
    fn test_export_format_metadata() {
        assert_eq!(AuditExportFormat::Json.mime_type(), "application/json");
        assert_eq!(AuditExportFormat::Csv.mime_type(), "text/csv");
        assert_eq!(AuditExportFormat::Binary.mime_type(), "application/octet-stream");

        assert_eq!(AuditExportFormat::Json.extension(), "json");
        assert_eq!(AuditExportFormat::Csv.extension(), "csv");
        assert_eq!(AuditExportFormat::Binary.extension(), "bin");
    }
}
