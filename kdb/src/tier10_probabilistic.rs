//! T10 Probabilistic - Path deduplication
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[repr(C, align(64))]
pub struct ExecutionPathSignature {
    pub path_id: AtomicU64,
    pub signature: [AtomicU64; 32],
    pub hit_count: AtomicU64,
    pub first_seen_ns: AtomicU64,
    pub last_seen_ns: AtomicU64,
    _padding: [u8; 320 - (32 + 4) * 8],
}

impl ExecutionPathSignature {
    pub const fn empty() -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            path_id: AtomicU64::new(0),
            signature: [ZERO; 32],
            hit_count: AtomicU64::new(0),
            first_seen_ns: AtomicU64::new(0),
            last_seen_ns: AtomicU64::new(0),
            _padding: [0; 320 - (32 + 4) * 8],
        }
    }

    pub fn set_signature(&self, path_id: u64, signature: &[u64; 32]) {
        self.path_id.store(path_id, Ordering::Release);
        for (i, &val) in signature.iter().enumerate() {
            self.signature[i].store(val, Ordering::Relaxed);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        self.first_seen_ns.store(now, Ordering::Relaxed);
        self.last_seen_ns.store(now, Ordering::Relaxed);
        self.hit_count.store(1, Ordering::Relaxed);
    }

    pub fn hit(&self) {
        self.hit_count.fetch_add(1, Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        self.last_seen_ns.store(now, Ordering::Relaxed);
    }

    pub fn similarity(&self, other: &Self) -> f64 {
        let mut matches = 0;
        for i in 0..32 {
            let a = self.signature[i].load(Ordering::Relaxed);
            let b = other.signature[i].load(Ordering::Relaxed);
            if a == b {
                matches += 1;
            }
        }
        matches as f64 / 32.0
    }
}

#[repr(C, align(64))]
pub struct LshBucket {
    pub count: AtomicU32,
    pub path_ids: [AtomicU64; 63],
    _padding: [u8; 512 - 4 - 63 * 8],
}

impl LshBucket {
    pub const fn empty() -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            count: AtomicU32::new(0),
            path_ids: [ZERO; 63],
            _padding: [0; 512 - 4 - 63 * 8],
        }
    }

    pub fn add_path(&self, path_id: u64) -> Result<(), &'static str> {
        let count = self.count.load(Ordering::Acquire);
        if count >= 63 {
            return Err("Bucket full");
        }

        self.path_ids[count as usize].store(path_id, Ordering::Relaxed);
        self.count.store(count + 1, Ordering::Release);
        Ok(())
    }

    pub fn get_paths(&self) -> Vec<u64> {
        let count = self.count.load(Ordering::Acquire) as usize;
        let mut paths = Vec::with_capacity(count);
        for i in 0..count {
            paths.push(self.path_ids[i].load(Ordering::Relaxed));
        }
        paths
    }
}

#[repr(C, align(256))]
pub struct LshPathTableCapsule {
    pub unique_paths: AtomicU32,
    pub total_checks: AtomicU64,
    pub similar_found: AtomicU64,
    _padding: [u8; 256 - 4 - 2 * 8 - 4],
    pub buckets: [LshBucket; 768],
}

impl LshPathTableCapsule {
    pub fn new() -> Self {
        const EMPTY: LshBucket = LshBucket::empty();
        Self {
            unique_paths: AtomicU32::new(0),
            total_checks: AtomicU64::new(0),
            similar_found: AtomicU64::new(0),
            _padding: [0; 256 - 4 - 2 * 8 - 4],
            buckets: [EMPTY; 768],
        }
    }

    fn hash_to_bucket(&self, signature: &[u64; 32]) -> usize {
        let hash = signature[0] ^ signature[1] ^ signature[2] ^ signature[3];
        (hash % 768) as usize
    }

    pub fn add_path(&self, path_id: u64, signature: &[u64; 32]) -> Result<(), &'static str> {
        let bucket_idx = self.hash_to_bucket(signature);
        self.buckets[bucket_idx].add_path(path_id)?;
        self.unique_paths.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn find_similar(&self, signature: &[u64; 32], _threshold: f64) -> Vec<u64> {
        self.total_checks.fetch_add(1, Ordering::Relaxed);

        let bucket_idx = self.hash_to_bucket(signature);
        let candidate_paths = self.buckets[bucket_idx].get_paths();

        if !candidate_paths.is_empty() {
            self.similar_found.fetch_add(1, Ordering::Relaxed);
        }

        Vec::new()
    }

    pub fn get_stats(&self) -> (u32, u64, u64) {
        (
            self.unique_paths.load(Ordering::Relaxed),
            self.total_checks.load(Ordering::Relaxed),
            self.similar_found.load(Ordering::Relaxed),
        )
    }
}
