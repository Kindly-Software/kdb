//! AuditLogBridge - Async/Blocking Bridge for AsyncLogCapsule
//!
//! Enables T5 Streaming tier AsyncLogCapsule (blocking I/O) in async tokio runtime
//! using spawn_blocking + MPSC channel (Phase 5.6 WebSocket pattern reused)

use crate::error::{ClapiError, ClapiResult};
use atomic_capsule::collections::{AsyncLogCapsule, LogEntry};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Async/blocking bridge for audit logging
pub struct AuditLogBridge {
    /// Core lockfree capsule (T5 Streaming tier)
    #[allow(dead_code)]  // Used by worker thread via Arc clone
    log: Arc<AsyncLogCapsule>,

    /// Channel for sending events from async context
    sender_tx: mpsc::Sender<String>,

    /// Worker thread handle
    worker_handle: Option<JoinHandle<()>>,

    /// Error counter (lockfree)
    error_counter: Arc<AtomicU64>,

    /// Shutdown signal
    shutdown: Arc<AtomicBool>,
}

impl AuditLogBridge {
    /// Create new async/blocking bridge
    pub fn new() -> Self {
        let log = Arc::new(AsyncLogCapsule::new());
        let error_counter = Arc::new(AtomicU64::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));

        let (sender_tx, mut receiver_rx) = mpsc::channel::<String>(1024);

        // Log, error_counter, shutdown clones for worker
        let log_worker = Arc::clone(&log);
        let error_counter_worker = Arc::clone(&error_counter);
        let shutdown_worker = Arc::clone(&shutdown);

        // Spawn worker thread
        let worker_handle = tokio::spawn(async move {
            let mut batch = Vec::new();
            let mut last_flush = std::time::Instant::now();
            const BATCH_SIZE: usize = 100;
            const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

            loop {
                // Collect batch with timeout
                let timeout = FLUSH_INTERVAL - last_flush.elapsed().min(FLUSH_INTERVAL);
                match tokio::time::timeout(timeout, receiver_rx.recv()).await {
                    Ok(Some(msg)) => {
                        batch.push(msg);
                        if batch.len() >= BATCH_SIZE {
                            Self::flush_batch(&log_worker, &mut batch, &error_counter_worker);
                            last_flush = std::time::Instant::now();
                        }
                    }
                    Ok(None) => {
                        // Channel closed
                        if !batch.is_empty() {
                            Self::flush_batch(&log_worker, &mut batch, &error_counter_worker);
                        }
                        break;
                    }
                    Err(_) => {
                        // Timeout - flush batch
                        if !batch.is_empty() {
                            Self::flush_batch(&log_worker, &mut batch, &error_counter_worker);
                            last_flush = std::time::Instant::now();
                        }
                    }
                }

                // Check shutdown
                if shutdown_worker.load(Ordering::Relaxed) && receiver_rx.is_empty() {
                    break;
                }
            }
        });

        Self {
            log,
            sender_tx,
            worker_handle: Some(worker_handle),
            error_counter,
            shutdown,
        }
    }

    /// Append audit event (async, channel send + T5 append)
    pub async fn append(&self, msg: impl AsRef<str>) -> ClapiResult<()> {
        self.sender_tx.send(msg.as_ref().to_string()).await.map_err(|_| {
            self.error_counter.fetch_add(1, Ordering::Relaxed);
            ClapiError::IoError("Audit log worker shutdown".to_string())
        })
    }

    /// Log request event
    pub async fn log_request(
        &self,
        user_id: u64,
        amount: i64,
        prev_hash: u64,
    ) -> ClapiResult<()> {
        let msg = format!(
            "ResponseReceived user={} amount={} prev_hash=0x{:x}",
            user_id, amount, prev_hash
        );
        self.append(msg).await
    }

    /// Log error event
    pub async fn log_error(&self, user_id: u64, prev_hash: u64) -> ClapiResult<()> {
        let msg = format!("ErrorOccurred user={} prev_hash=0x{:x}", user_id, prev_hash);
        self.append(msg).await
    }

    /// Get error count (lockfree)
    pub fn error_count(&self) -> u64 {
        self.error_counter.load(Ordering::Relaxed)
    }

    /// Flush all pending events (sync - AsyncLogCapsule append is already lockfree)
    fn flush_batch(
        log: &Arc<AsyncLogCapsule>,
        batch: &mut Vec<String>,
        errors: &Arc<AtomicU64>,
    ) {
        for msg in batch.drain(..) {
            let entry = LogEntry::new(&msg);
            if log.append(entry).is_err() {
                errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Shutdown bridge gracefully
    pub async fn shutdown(&mut self) -> ClapiResult<()> {
        self.shutdown.store(true, Ordering::Release);
        self.sender_tx.closed().await;

        if let Some(handle) = self.worker_handle.take() {
            handle.await.map_err(|e| ClapiError::IoError(format!("Worker error: {}", e)))?;
        }

        Ok(())
    }
}

impl Default for AuditLogBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AuditLogBridge {
    fn drop(&mut self) {
        // Signal shutdown
        self.shutdown.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bridge_creation() {
        let bridge = AuditLogBridge::new();
        assert_eq!(bridge.error_count(), 0);
    }

    #[tokio::test]
    async fn test_append_event() {
        let bridge = AuditLogBridge::new();
        assert!(bridge.append("test event").await.is_ok());
    }

    #[tokio::test]
    async fn test_log_request() {
        let bridge = AuditLogBridge::new();
        assert!(bridge.log_request(42, 12345, 0x1234).await.is_ok());
    }

    #[tokio::test]
    async fn test_log_error() {
        let bridge = AuditLogBridge::new();
        assert!(bridge.log_error(42, 0x5678).await.is_ok());
    }
}
