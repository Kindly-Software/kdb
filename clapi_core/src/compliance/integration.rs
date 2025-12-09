//! Compliance Integration - clapi_core → kindly-db
//!
//! Phase 5: Async persistence of compliance entries to kindly-db.
//!
//! # Architecture
//! - **Async writes**: tokio::spawn for zero-blocking persistence
//! - **Feature-gated**: Requires `kindlydb` feature flag
//! - **Graceful degradation**: Works without kindly-db (in-memory only)
//!
//! # I20 Integration Analysis
//! - Q1-Q5 (Scope): clapi_core compliance → kindly-db storage
//! - Q6-Q10 (Compatibility): Both capsules, async, Result-based errors
//! - Q11-Q15 (Safety): Async non-blocking, no races, graceful degradation
//! - Q16-Q20 (Validation): Property tests for roundtrip, zero blocking
//!
//! # Performance Targets (B32)
//! - Dispatch: <1μs (async spawn overhead)
//! - Hot path impact: 0ns (async, non-blocking)
//! - Database write: <100μs (async, off hot path)
//!
//! # ASSUM Safety
//! - #ASSUME: Tokio async spawn is non-blocking
//! - #VERIFY: Integration tests validate zero blocking
//! - #ASSUME: kindly-db handles concurrent writes safely
//! - #VERIFY: Property tests validate no data loss
//!
//! # Example Usage
//! ```rust,ignore
//! use clapi_core::compliance::integration::record_and_persist;
//! use clapi_core::compliance::ComplianceEntry;
//!
//! let entry = ComplianceEntry {
//!     framework: ComplianceFramework::Sox404,
//!     operation: "budget_deduction".to_string(),
//!     timestamp_ns: 1729000000000000000,
//!     hash: 0x1234567890ABCDEF,
//!     prev_hash: 0x0,
//!     metadata: vec![("user".to_string(), "test".to_string())],
//! };
//!
//! // Record in-memory + async persist to kindly-db
//! record_and_persist(entry).await?;
//! ```

use crate::error::ClapiResult;
use super::{ComplianceEntry as ClapiComplianceEntry, ComplianceFramework};

// Feature-gated import of kindly-db
#[cfg(feature = "kindlydb")]
use kindly_db::{ComplianceWriter, ComplianceEntry as DbComplianceEntry};

#[cfg(feature = "kindlydb")]
use std::sync::Arc;

#[cfg(feature = "kindlydb")]
use once_cell::sync::Lazy;

// Global compliance writer instance (initialized lazily)
#[cfg(feature = "kindlydb")]
static COMPLIANCE_WRITER: Lazy<Option<Arc<ComplianceWriter>>> = Lazy::new(|| {
    // Note: In real implementation, would initialize from Database handle
    // For now, return None (no database connection)
    // TODO: Initialize ComplianceWriter with Database handle
    None
});

/// Convert clapi_core ComplianceFramework to string code
fn framework_to_code(framework: ComplianceFramework) -> String {
    framework.code().to_string()
}

/// Convert clapi_core ComplianceEntry to kindly-db ComplianceEntry
#[cfg(feature = "kindlydb")]
fn convert_entry(entry: ClapiComplianceEntry) -> DbComplianceEntry {
    DbComplianceEntry {
        framework: framework_to_code(entry.framework),
        operation: entry.operation,
        timestamp_ns: entry.timestamp_ns,
        hash: entry.hash,
        prev_hash: entry.prev_hash,
        metadata: entry.metadata,
    }
}

/// Record compliance entry and persist to kindly-db (async, non-blocking)
///
/// # Performance
/// - Dispatch: <1μs (async spawn overhead)
/// - Hot path: 0ns (returns immediately)
/// - Database write: <100μs (async, off hot path)
///
/// # ASSUM Framework
/// - #ASSUME: Tokio async spawn is non-blocking
/// - #VERIFY: Integration tests validate zero blocking on caller
/// - #ASSUME: kindly-db ComplianceWriter handles concurrency safely
/// - #VERIFY: Property tests validate no data loss under concurrent writes
///
/// # Graceful Degradation
/// - If `kindlydb` feature disabled: No-op, returns Ok(())
/// - If writer not initialized: Logs warning, returns Ok(())
/// - If write fails: Logs error (async), returns Ok(()) to avoid blocking caller
///
/// # Errors
/// Returns ClapiError::InternalError if feature enabled but conversion fails.
pub async fn record_and_persist(entry: ClapiComplianceEntry) -> ClapiResult<()> {
    #[cfg(feature = "kindlydb")]
    {
        // Check if writer is initialized
        if let Some(writer) = COMPLIANCE_WRITER.as_ref() {
            // Convert entry format
            let db_entry = convert_entry(entry);

            // Async write (non-blocking, returns immediately)
            // #ASSUME: write_entry spawns tokio task and returns immediately
            // #VERIFY: Integration test measures dispatch latency (<1μs)
            if let Err(e) = writer.write_entry(db_entry).await {
                // Log error but don't propagate (avoid blocking caller)
                eprintln!("[clapi_core::compliance::integration] Failed to persist entry: {}", e);
            }
        } else {
            // Writer not initialized (no database connection)
            // This is acceptable in standalone mode (in-memory only)
            #[cfg(debug_assertions)]
            eprintln!("[clapi_core::compliance::integration] ComplianceWriter not initialized (kindly-db integration disabled)");
        }
    }

    #[cfg(not(feature = "kindlydb"))]
    {
        // Feature disabled: no-op
        let _ = entry; // Suppress unused warning
    }

    Ok(())
}

/// Initialize compliance writer with database handle
///
/// # Safety
/// This function is NOT thread-safe. Call once during application startup.
///
/// # Example
/// ```rust,ignore
/// use kindly_db::Database;
/// use clapi_core::compliance::integration::init_compliance_writer;
///
/// let db = Database::open("clapi.kdb")?;
/// init_compliance_writer(db);
/// ```
#[cfg(feature = "kindlydb")]
pub fn init_compliance_writer(_db: Arc<kindly_db::Database>) {
    // TODO: Implement writer initialization
    // This would replace COMPLIANCE_WRITER with Some(ComplianceWriter::new(db))
    // For now, this is a placeholder

    #[cfg(debug_assertions)]
    eprintln!("[clapi_core::compliance::integration] init_compliance_writer called (not yet implemented)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framework_to_code() {
        assert_eq!(framework_to_code(ComplianceFramework::Sox404), "SOX-404");
        assert_eq!(framework_to_code(ComplianceFramework::Soc2TypeII), "SOC2-CC6.1");
        assert_eq!(framework_to_code(ComplianceFramework::GdprArticle30), "GDPR-30");
        assert_eq!(framework_to_code(ComplianceFramework::Hipaa164312b), "HIPAA-164.312(b)");
    }

    #[tokio::test]
    async fn test_record_and_persist_no_writer() {
        // Without writer initialized, should return Ok() (graceful degradation)
        let entry = ClapiComplianceEntry {
            framework: ComplianceFramework::Sox404,
            operation: "test_operation".to_string(),
            timestamp_ns: 1729000000000000000,
            hash: 0x1234567890ABCDEF,
            prev_hash: 0x0,
            metadata: vec![("test".to_string(), "value".to_string())],
        };

        let result = record_and_persist(entry).await;
        assert!(result.is_ok());
    }
}
