//! Export Format Implementations - JSON, CSV, Binary + 5 Additional Formats
//!
//! Provides serialization for compliance reports in multiple formats:
//! - JSON (primary): Full metadata, human-readable
//! - CSV (secondary): Spreadsheet-compatible, auditor-friendly
//! - Binary (optional): Compact, machine-readable
//! - SQL: INSERT statements for database import
//! - YAML: Human-readable configuration
//! - XML: Enterprise integration
//!
//! # Performance Targets (B32)
//! - JSON: <100μs per entry (serde_json)
//! - CSV: <50μs per entry (manual formatting)
//! - Binary: <20μs per entry (bincode)
//!
//! # New Format Modules
//! See `formats/` subdirectory for 8 specialized exporters with real implementations.

use crate::error::{ClapiError, ClapiResult};
use super::sox_exporter::SoxReport;
use super::soc2_exporter::Soc2Report;
use super::gdpr_exporter::GdprReport;

// Re-export new format modules
pub mod formats;

/// Export format enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// JSON format (default, full metadata)
    Json,
    /// CSV format (spreadsheet-compatible)
    Csv,
    /// Binary format (compact, bincode)
    Binary,
}

impl ExportFormat {
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

/// JSON exporter
pub struct JsonExporter;

impl JsonExporter {
    /// Export SOX report as JSON
    pub fn export_sox(report: &SoxReport) -> ClapiResult<String> {
        serde_json::to_string_pretty(report)
            .map_err(|e| ClapiError::JsonError(format!("JSON serialization failed: {}", e)))
    }

    /// Export SOC2 report as JSON
    pub fn export_soc2(report: &Soc2Report) -> ClapiResult<String> {
        serde_json::to_string_pretty(report)
            .map_err(|e| ClapiError::JsonError(format!("JSON serialization failed: {}", e)))
    }

    /// Export GDPR report as JSON
    pub fn export_gdpr(report: &GdprReport) -> ClapiResult<String> {
        serde_json::to_string_pretty(report)
            .map_err(|e| ClapiError::JsonError(format!("JSON serialization failed: {}", e)))
    }
}

/// CSV exporter
pub struct CsvExporter;

impl CsvExporter {
    /// Export SOX report as CSV
    pub fn export_sox(report: &SoxReport) -> ClapiResult<String> {
        let mut output = String::new();

        // Header
        output.push_str("GL Code,Description,Amount (Cents),Approver,Fiscal Year,Timestamp,Hash,Prev Hash\n");

        // Rows
        for entry in &report.gl_entries {
            output.push_str(&format!(
                "{},{},{},{},{},{},0x{:016x},0x{:016x}\n",
                Self::escape_csv(&entry.gl_code),
                Self::escape_csv(&entry.description),
                entry.amount_cents,
                Self::escape_csv(&entry.approver),
                entry.fiscal_year,
                entry.timestamp_ns,
                entry.hash,
                entry.prev_hash,
            ));
        }

        Ok(output)
    }

    /// Export SOC2 report as CSV
    pub fn export_soc2(report: &Soc2Report) -> ClapiResult<String> {
        let mut output = String::new();

        // Header
        output.push_str("Change Ticket,Description,Approved By,Approval Timestamp,Execution Timestamp,Hash,Prev Hash\n");

        // Rows
        for record in &report.change_records {
            output.push_str(&format!(
                "{},{},{},{},{},0x{:016x},0x{:016x}\n",
                Self::escape_csv(&record.change_ticket),
                Self::escape_csv(&record.description),
                Self::escape_csv(&record.approved_by),
                record.approval_timestamp_ns,
                record.timestamp_ns,
                record.hash,
                record.prev_hash,
            ));
        }

        Ok(output)
    }

    /// Export GDPR report as CSV (access logs)
    pub fn export_gdpr_access(report: &GdprReport) -> ClapiResult<String> {
        let mut output = String::new();

        // Header
        output.push_str("User ID,GDPR Article,Access Type,Accessor,Legal Basis,Purpose,Timestamp,Hash,Prev Hash\n");

        // Rows
        for log in &report.access_logs {
            output.push_str(&format!(
                "{},{},{},{},{},{},{},0x{:016x},0x{:016x}\n",
                Self::escape_csv(&log.user_id),
                Self::escape_csv(&log.gdpr_article),
                Self::escape_csv(&log.access_type),
                Self::escape_csv(&log.accessor),
                Self::escape_csv(log.legal_basis.as_deref().unwrap_or("N/A")),
                Self::escape_csv(log.purpose.as_deref().unwrap_or("N/A")),
                log.timestamp_ns,
                log.hash,
                log.prev_hash,
            ));
        }

        Ok(output)
    }

    /// Export GDPR report as CSV (RTBF requests)
    pub fn export_gdpr_rtbf(report: &GdprReport) -> ClapiResult<String> {
        let mut output = String::new();

        // Header
        output.push_str("User ID,Request ID,Request Timestamp,Completion Timestamp,Status,Hash,Prev Hash\n");

        // Rows
        for req in &report.rtbf_requests {
            output.push_str(&format!(
                "{},{},{},{},{},0x{:016x},0x{:016x}\n",
                Self::escape_csv(&req.user_id),
                Self::escape_csv(&req.request_id),
                req.request_timestamp_ns,
                req.completion_timestamp_ns.map(|ts| ts.to_string()).unwrap_or_else(|| "N/A".to_string()),
                Self::escape_csv(&req.status),
                req.hash,
                req.prev_hash,
            ));
        }

        Ok(output)
    }

    /// Escape CSV field (quote if contains comma, quote, or newline)
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

/// Binary exporter (stub - would use bincode in production)
pub struct BinaryExporter;

impl BinaryExporter {
    /// Export SOX report as binary (stub)
    pub fn export_sox(_report: &SoxReport) -> ClapiResult<Vec<u8>> {
        // In production, use bincode or similar
        Err(ClapiError::InvalidRequest { reason: "Binary export not yet implemented".to_string() })
    }

    /// Export SOC2 report as binary (stub)
    pub fn export_soc2(_report: &Soc2Report) -> ClapiResult<Vec<u8>> {
        Err(ClapiError::InvalidRequest { reason: "Binary export not yet implemented".to_string() })
    }

    /// Export GDPR report as binary (stub)
    pub fn export_gdpr(_report: &GdprReport) -> ClapiResult<Vec<u8>> {
        Err(ClapiError::InvalidRequest { reason: "Binary export not yet implemented".to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use super::super::sox_exporter::GlEntry;

    #[test]
    fn test_export_format_metadata() {
        assert_eq!(ExportFormat::Json.mime_type(), "application/json");
        assert_eq!(ExportFormat::Csv.mime_type(), "text/csv");
        assert_eq!(ExportFormat::Binary.mime_type(), "application/octet-stream");

        assert_eq!(ExportFormat::Json.extension(), "json");
        assert_eq!(ExportFormat::Csv.extension(), "csv");
        assert_eq!(ExportFormat::Binary.extension(), "bin");
    }

    #[test]
    fn test_csv_escape() {
        assert_eq!(CsvExporter::escape_csv("simple"), "simple");
        assert_eq!(CsvExporter::escape_csv("with,comma"), "\"with,comma\"");
        assert_eq!(CsvExporter::escape_csv("with\"quote"), "\"with\"\"quote\"");
        assert_eq!(CsvExporter::escape_csv("with\nnewline"), "\"with\nnewline\"");
    }

    #[test]
    fn test_json_sox_export() {
        let report = SoxReport {
            generated_at_ns: 1000000,
            fiscal_year: Some(2025),
            total_entries: 1,
            total_amount_cents: 100_00,
            gl_entries: vec![GlEntry {
                gl_code: "4100".to_string(),
                description: "Test transaction".to_string(),
                amount_cents: 100_00,
                approver: "test@company.com".to_string(),
                fiscal_year: 2025,
                timestamp_ns: 1000000,
                hash: 0x1234,
                prev_hash: 0,
            }],
            chain_valid: true,
            metadata: HashMap::new(),
        };

        let json = JsonExporter::export_sox(&report).unwrap();
        assert!(json.contains("\"fiscal_year\": 2025"));
        assert!(json.contains("\"gl_code\": \"4100\""));
    }

    #[test]
    fn test_csv_sox_export() {
        let report = SoxReport {
            generated_at_ns: 1000000,
            fiscal_year: Some(2025),
            total_entries: 1,
            total_amount_cents: 100_00,
            gl_entries: vec![GlEntry {
                gl_code: "4100".to_string(),
                description: "Test transaction".to_string(),
                amount_cents: 100_00,
                approver: "test@company.com".to_string(),
                fiscal_year: 2025,
                timestamp_ns: 1000000,
                hash: 0x1234,
                prev_hash: 0,
            }],
            chain_valid: true,
            metadata: HashMap::new(),
        };

        let csv = CsvExporter::export_sox(&report).unwrap();
        assert!(csv.contains("GL Code,Description,Amount (Cents)"));
        assert!(csv.contains("4100,Test transaction,10000"));
    }
}
