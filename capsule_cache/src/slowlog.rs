//! Slowlog (bounded, lockfree) for recording slow commands.
//! Uses `RingBufferCapsule` (T5 Streaming) with a fixed capacity (16,384 entries).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use atomic_capsule::collections::{
    queue::{MPMC, QueueCapsule},
    RingBufferCapsule, RingBufferEntry, SyncLogEntry,
};

/// Opcode of the slow command.
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum SlowOp {
    Get = 1,
    Set = 2,
    Del = 3,
    Incr = 4,
    Expire = 5,
    Ttl = 6,
    Mget = 7,
    Mset = 8,
    Other = 255,
}

impl SlowOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            SlowOp::Get => "GET",
            SlowOp::Set => "SET",
            SlowOp::Del => "DEL",
            SlowOp::Incr => "INCR",
            SlowOp::Expire => "EXPIRE",
            SlowOp::Ttl => "TTL",
            SlowOp::Mget => "MGET",
            SlowOp::Mset => "MSET",
            SlowOp::Other => "OTHER",
        }
    }
}

/// Slowlog entry (Copy to satisfy RingBufferEntry).
#[derive(Copy, Clone)]
pub struct SlowLogEntry {
    /// Monotonic sequence number (used for RESET filtering)
    pub seq: u64,
    /// Unix timestamp (ns)
    pub ts_ns: u64,
    /// Command duration (ns)
    pub duration_ns: u64,
    /// Hash of the primary key (to avoid large payloads)
    pub key_hash: u64,
    /// Opcode
    pub op: SlowOp,
    /// Whether the command succeeded (no -ERR)
    pub ok: bool,
}

impl RingBufferEntry for SlowLogEntry {
    fn empty() -> Self {
        SlowLogEntry {
            seq: 0,
            ts_ns: 0,
            duration_ns: 0,
            key_hash: 0,
            op: SlowOp::Other,
            ok: true,
        }
    }

    fn is_empty(&self) -> bool {
        self.seq == 0
    }
}

/// Bounded slowlog; lockfree ring buffer underneath.
pub struct SlowLog {
    ring: RingBufferCapsule<SlowLogEntry>,
    threshold_ns: u64,
    seq: AtomicU64,
    reset_at: AtomicU64,
    writer: Option<SlowLogWriter>,
    export_path: Option<String>,
}

impl SlowLog {
    /// Create a slowlog with a duration threshold (nanoseconds).
    pub fn new(threshold_ns: u64) -> Self {
        Self::with_export(threshold_ns, None).unwrap_or_else(|_| Self {
            ring: RingBufferCapsule::new(),
            threshold_ns,
            seq: AtomicU64::new(0),
            reset_at: AtomicU64::new(0),
            writer: None,
            export_path: None,
        })
    }

    /// Create a slowlog with optional file export.
    pub fn with_export(threshold_ns: u64, export_path: Option<&str>) -> std::io::Result<Self> {
        let writer = if let Some(path) = export_path {
            Some(SlowLogWriter::new(path)?)
        } else {
            None
        };

        Ok(Self {
            ring: RingBufferCapsule::new(),
            threshold_ns,
            seq: AtomicU64::new(0),
            reset_at: AtomicU64::new(0),
            writer,
            export_path: export_path.map(|s| s.to_string()),
        })
    }

    /// Record a slow entry if it crosses the threshold.
    pub fn maybe_record(&self, op: SlowOp, key_hash: u64, duration_ns: u64, ok: bool) {
        if duration_ns < self.threshold_ns {
            return;
        }

        let seq = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
        let ts_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let entry = SlowLogEntry {
            seq,
            ts_ns,
            duration_ns,
            key_hash,
            op,
            ok,
        };

        let _ = self.ring.record(entry);

        if let Some(writer) = &self.writer {
            let msg = format!(
                "seq={}\tts_ns={}\top={}\tduration_ns={}\tkey_hash={:#x}\tok={}",
                entry.seq,
                entry.ts_ns,
                entry.op.as_str(),
                entry.duration_ns,
                entry.key_hash,
                entry.ok as u8
            );
            let _ = writer.append(SyncLogEntry::new(&msg));
        }
    }

    /// Get the most recent N entries (newest first).
    pub fn recent(&self, count: usize) -> Vec<SlowLogEntry> {
        let cutoff = self.reset_at.load(Ordering::Acquire);
        let capacity = self.ring.capacity();
        let snapshot = self.ring.get_recent(capacity);
        let mut out = Vec::with_capacity(count.min(capacity));
        for entry in snapshot {
            if entry.seq > cutoff {
                out.push(entry);
            }
            if out.len() >= count {
                break;
            }
        }
        out
    }

    /// Total recorded entries (monotonic).
    pub fn total(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    /// Slowlog threshold (ns).
    pub fn threshold_ns(&self) -> u64 {
        self.threshold_ns
    }

    /// Reset the visible window (entries before current seq are hidden).
    pub fn reset(&self) {
        let current = self.seq.load(Ordering::Acquire);
        self.reset_at.store(current, Ordering::Release);
    }

    /// Count entries after the last reset.
    pub fn len_since_reset(&self) -> u64 {
        let seq = self.seq.load(Ordering::Acquire);
        let cutoff = self.reset_at.load(Ordering::Acquire);
        seq.saturating_sub(cutoff)
    }

    /// Export path if configured.
    pub fn export_path(&self) -> Option<&str> {
        self.export_path.as_deref()
    }
}

/// Background writer for slowlog export (no mutex, bounded queue).
struct SlowLogWriter {
    queue: Arc<QueueCapsule<SyncLogEntry, MPMC>>,
    shutdown: Arc<AtomicBool>,
}

impl SlowLogWriter {
    fn new(path: &str) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let writer = BufWriter::new(file);

        let queue: Arc<QueueCapsule<SyncLogEntry, MPMC>> =
            Arc::new(QueueCapsule::<SyncLogEntry, MPMC>::new(4096).expect("queue"));
        let shutdown = Arc::new(AtomicBool::new(false));

        let queue_clone = Arc::clone(&queue);
        let shutdown_clone = Arc::clone(&shutdown);

        // Detach thread; controlled via shutdown flag.
        let mut writer = writer;
        thread::spawn(move || {
            while !shutdown_clone.load(Ordering::Acquire) {
                let mut batch: Vec<SyncLogEntry> = Vec::with_capacity(128);
                while let Some(entry) = queue_clone.pop() {
                    batch.push(entry);
                    if batch.len() >= 128 {
                        break;
                    }
                }

                for entry in batch {
                    let _ = writeln!(writer, "{}", entry.as_str());
                }
                let _ = writer.flush();
                thread::sleep(std::time::Duration::from_millis(100));
            }

            // Final drain
            while let Some(entry) = queue_clone.pop() {
                let _ = writeln!(writer, "{}", entry.as_str());
            }
            let _ = writer.flush();
        });

        Ok(Self { queue, shutdown })
    }

    fn append(&self, entry: SyncLogEntry) -> Result<(), String> {
        self.queue
            .push(entry)
            .map_err(|e| format!("slowlog export queue full: {:?}", e))
    }
}

impl Drop for SlowLogWriter {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
    }
}

/// Hash helper for keys (stable DefaultHasher).
pub fn hash_key<K: Hash>(key: &K) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}
