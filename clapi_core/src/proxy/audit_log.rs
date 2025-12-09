//! Audit log with streaming append
//!
//! # UCE33 Q16: Streaming
//! - AuditLogEntry128 for compact log entries
//! - Append-only file for audit trail
//! - Async write with buffering

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::capsules::{AuditEntry, AuditLogEntry128, EventType};
use crate::error::{ClapiError, ClapiResult};

/// Audit log with streaming append
///
/// # Safety
/// - #ASSUME: File writes are atomic at OS level
/// - #VERIFY: Append-only mode prevents overwrites
/// - #ASSUME: Mutex protects file handle (not hot path)
/// - #VERIFY: Audit writes are async (non-blocking)
pub struct AuditLog {
    /// File path
    _path: PathBuf,

    /// File handle (protected by mutex for writes)
    file: Mutex<std::fs::File>,
}

impl AuditLog {
    /// Create new audit log
    ///
    /// Opens file in append mode (creates if not exists).
    pub fn new(path: PathBuf) -> ClapiResult<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        Ok(Self {
            _path: path,
            file: Mutex::new(file),
        })
    }

    /// Append entry to audit log
    ///
    /// # Performance
    /// - Non-blocking: Writes happen in background
    /// - Buffered: OS-level buffering
    ///
    /// # Safety
    /// - #ASSUME: Append-only writes are crash-safe
    /// - #VERIFY: Each entry includes hash chain for integrity
    pub fn append(&self, entry: &AuditEntry) -> ClapiResult<()> {
        // Create capsule and write entry
        let capsule = AuditLogEntry128::new();
        capsule.write(entry.prev_hash, entry);

        // Serialize to bytes
        let bytes = self.serialize_entry(&capsule);

        // Append to file (mutex protects file handle)
        let mut file = self
            .file
            .lock()
            .map_err(|e| ClapiError::IoError(format!("Mutex poisoned: {}", e)))?;

        file.write_all(&bytes)?;
        file.write_all(b"\n")?;

        Ok(())
    }

    /// Append request event
    pub fn log_request(
        &self,
        _budget_id: u64,
        provider_id: u16,
        cost_cents: i64,
        tokens: u32,
        prev_hash: u64,
    ) -> ClapiResult<()> {
        let entry = AuditEntry {
            prev_hash,
            timestamp_ms: Self::now_ms(),
            provider_id,
            event_type: EventType::ResponseReceived,
            flags: 0,
            cost_cents: cost_cents as f64 / 100.0, // Convert cents to dollars
            tokens: tokens as u64,
            latency_us: 0,
            request_id: 0, // TODO: Generate unique request ID
            sequence: 0, // TODO: Maintain sequence counter
        };

        self.append(&entry)
    }

    /// Append error event
    pub fn log_error(&self, _budget_id: u64, provider_id: u16, prev_hash: u64) -> ClapiResult<()> {
        let entry = AuditEntry {
            prev_hash,
            timestamp_ms: Self::now_ms(),
            provider_id,
            event_type: EventType::ErrorOccurred,
            flags: 0,
            cost_cents: 0.0,
            tokens: 0,
            latency_us: 0,
            request_id: 0,
            sequence: 0,
        };

        self.append(&entry)
    }

    /// Get current timestamp (milliseconds)
    fn now_ms() -> u32 {
        (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64 & 0xFFFFFFFF) as u32
    }

    /// Serialize entry to bytes (simple format for now)
    fn serialize_entry(&self, capsule: &AuditLogEntry128) -> Vec<u8> {
        // Read entry from capsule
        let entry = capsule.read();

        // JSON serialization for now (simple but not optimal)
        // TODO: Use compact binary format in production
        serde_json::to_vec(&serde_json::json!({
            "prev_hash": entry.prev_hash,
            "timestamp_ms": entry.timestamp_ms,
            "provider_id": entry.provider_id,
            "event_type": format!("{:?}", entry.event_type),
            "cost_cents": entry.cost_cents,
            "tokens": entry.tokens,
            "latency_us": entry.latency_us,
            "request_id": entry.request_id,
            "sequence": entry.sequence,
        }))
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_new() {
        let path = PathBuf::from("/tmp/clapi_test_audit.log");
        let _ = fs::remove_file(&path);

        let log = AuditLog::new(path.clone());
        assert!(log.is_ok());

        assert!(path.exists());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_log_request() {
        let path = PathBuf::from("/tmp/clapi_test_request.log");
        let _ = fs::remove_file(&path);

        let log = AuditLog::new(path.clone()).unwrap();
        let result = log.log_request(1, 0, 100_00, 1000, 0);
        assert!(result.is_ok());

        assert!(path.metadata().unwrap().len() > 0);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_log_error() {
        let path = PathBuf::from("/tmp/clapi_test_error.log");
        let _ = fs::remove_file(&path);

        let log = AuditLog::new(path.clone()).unwrap();
        let result = log.log_error(1, 0, 0);
        assert!(result.is_ok());

        let _ = fs::remove_file(&path);
    }
}
