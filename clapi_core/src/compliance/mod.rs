//! Compliance Module - SOX, SOC2, GDPR, HIPAA Export Infrastructure
//!
//! Phase 4: Production-grade compliance audit trail exports for regulatory requirements.
//!
//! # Supported Regulations
//! - **SOX (Sarbanes-Oxley) Section 404**: Transaction audit trails, GL codes, approvals
//! - **SOC2 Type II CC6.1**: Change control evidence, completeness validation
//! - **GDPR Article 30**: Data processing logs, access tracking, right to be forgotten
//! - **HIPAA 164.312(b)**: PHI access logging, breach detection (future)
//!
//! # Architecture
//! - **ComplianceCapsule256** (T1+T5 Mixed): Atomic state + streaming export
//! - **Hash chain integration**: Leverages existing RequestCapsule128Enhanced patterns
//! - **Export formats**: JSON (primary), CSV (secondary), binary (optional)
//! - **Zero-copy streaming**: O(1) memory for large exports
//!
//! # UCE34 Framework Analysis
//! - Q10: Tier 1 (Atomic) for state + Tier 5 (Streaming) for exports = Tier 6 (Mixed)
//! - Q11: AtomicU64 counters + streaming iterator for O(1) memory
//! - Q12: No nightly features required (stable Rust)
//! - Q33: Compile-time verification via #[derive(ComputationalCapsule)]
//!
//! # Performance Targets (B32)
//! - Export metadata preparation: <1μs
//! - JSON serialization: <100μs per entry (serde_json)
//! - CSV serialization: <50μs per entry (manual formatting)
//! - Binary serialization: <20μs per entry (bincode)
//! - Streaming iteration: O(1) memory, <10ns per entry overhead
//!
//! # ASSUM Safety
//! - All atomic operations documented with #ASSUME / #VERIFY tags
//! - Zero unsafe blocks (100% safe Rust)
//! - Lockfree coordination via compare-exchange loops
//!
//! # T28 Testing Coverage
//! - Unit tests (Q1-Q7): Capsule invariants, export format correctness
//! - Property tests (Q8-Q14): Concurrent export safety, hash chain integrity
//! - Integration tests (Q15-Q21): End-to-end SOX/SOC2/GDPR exports
//! - Stress tests (Q22-Q28): 100K+ entry exports, memory constraints

pub mod compliance_capsules;
pub mod sox_exporter;
pub mod soc2_exporter;
pub mod gdpr_exporter;
pub mod export_formats;
pub mod export_capsule;
pub mod integration;
pub mod audit_capsule;
pub mod audit_export;

pub use compliance_capsules::{ComplianceCapsule256, ComplianceMetrics, ComplianceEntry};
pub use sox_exporter::{SoxExporter, SoxReport, GlEntry};
pub use soc2_exporter::{Soc2Exporter, Soc2Report, ChangeRecord};
pub use gdpr_exporter::{GdprExporter, GdprReport, AccessLog, RtbfRequest};
pub use export_formats::{ExportFormat, JsonExporter, CsvExporter, BinaryExporter};
pub use export_capsule::DataExportCapsule;
pub use audit_capsule::{ComplianceAuditCapsule, AuditEvent, AuditEventType, AuditEventStatus};
pub use audit_export::{AuditExportFormat, AuditTrailReport, JsonExporter as AuditJsonExporter, CsvExporter as AuditCsvExporter};

// Phase 5: kindly-db integration (feature-gated)
#[cfg(feature = "kindlydb")]
pub use integration::{record_and_persist, init_compliance_writer};

/// Module version (Phase 4 compliance infrastructure)
pub const COMPLIANCE_MODULE_VERSION: &str = "0.4.0";

/// Maximum entries per export batch (streaming chunk size)
pub const MAX_EXPORT_BATCH_SIZE: usize = 10_000;

/// Supported compliance frameworks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplianceFramework {
    /// SOX (Sarbanes-Oxley) Section 404
    Sox404,
    /// SOC2 Type II CC6.1
    Soc2TypeII,
    /// GDPR Article 30
    GdprArticle30,
    /// HIPAA 164.312(b) (future)
    Hipaa164312b,
}

impl ComplianceFramework {
    /// Get human-readable framework name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Sox404 => "SOX (Sarbanes-Oxley) Section 404",
            Self::Soc2TypeII => "SOC2 Type II CC6.1",
            Self::GdprArticle30 => "GDPR Article 30",
            Self::Hipaa164312b => "HIPAA 164.312(b)",
        }
    }

    /// Get regulatory reference code
    pub fn code(&self) -> &'static str {
        match self {
            Self::Sox404 => "SOX-404",
            Self::Soc2TypeII => "SOC2-CC6.1",
            Self::GdprArticle30 => "GDPR-30",
            Self::Hipaa164312b => "HIPAA-164.312(b)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framework_metadata() {
        let sox = ComplianceFramework::Sox404;
        assert_eq!(sox.name(), "SOX (Sarbanes-Oxley) Section 404");
        assert_eq!(sox.code(), "SOX-404");

        let soc2 = ComplianceFramework::Soc2TypeII;
        assert_eq!(soc2.name(), "SOC2 Type II CC6.1");
        assert_eq!(soc2.code(), "SOC2-CC6.1");

        let gdpr = ComplianceFramework::GdprArticle30;
        assert_eq!(gdpr.name(), "GDPR Article 30");
        assert_eq!(gdpr.code(), "GDPR-30");
    }
}
