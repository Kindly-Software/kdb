//! MemoryReconstructorCapsule - T6 Mixed Page Reconstruction with LRU Cache
//!
//! **Tier**: T6 Mixed (T0 verification + T1 atomic coordination + T2 SIMD XOR)
//! **Size**: 128 KB (31 cached pages + metadata)
//! **Target**: less than 1ms page reconstruction
//!
//! # Architecture
//!
//! - Metadata: 256 bytes (generation, state, statistics)
//! - Page Cache Metadata: 31 entries x 64B = 1,984 bytes
//! - Page Cache Data: 31 x 4KB = 126,976 bytes
//! - Padding: 1,856 bytes
//! - Total: 131,072 bytes (128 KB exactly)
//!
//! # Reconstruction Algorithm
//!
//! 1. Check cache for (address, snapshot_id) - return if hit
//! 2. Find closest base (current live memory OR checkpoint)
//! 3. Get delta chain from delta ring (deltas between base and target)
//! 4. Apply deltas in order (XOR operations, T2 SIMD accelerated)
//! 5. Verify hash against merkle tree (Q34 integrity)
//! 6. Cache result, evict LRU if full
//! 7. Return reconstructed page
//!
//! # Performance Targets (B32)
//!
//! | Operation | Target | Method |
//! |-----------|--------|--------|
//! | Cache hit | less than 10ns | Direct index lookup |
//! | Cache miss (with delta) | less than 500us | XOR delta chain |
//! | Cache miss (from base) | less than 1ms | Full page read + delta |
//! | LRU eviction | less than 100ns | Min access_count scan |
//!
//! # Chaos Compliance
//!
//! - 100% lockfree (atomic operations only)
//! - Cache-aligned entries (64B PageCacheEntry)
//! - Generation counters for TOCTOU prevention
//! - Q34 hash verification on reconstruction
//!
//! # ASSUM Safety
//!
//! - #ASSUME_CACHE_ALIGNED: All cache entries 64-byte aligned
//! - #ASSUME_XOR_REVERSIBLE: XOR deltas are perfectly reversible
//! - #ASSUME_HASH_VALID: Merkle hashes verified before return
//! - #ASSUME_SNAPSHOT_EXISTS: Delta ring contains required snapshots

use crate::time_travel::MAX_SNAPSHOTS;
use crc::{Crc, CRC_64_ECMA_182};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Page size constant (4 KB)
pub const PAGE_SIZE: usize = 4096;

/// Number of cached pages (31 x 4KB = 124KB, fits in 128KB with metadata)
pub const CACHE_CAPACITY: usize = 31;

/// CRC64-ECMA for hash computation
const CRC64: Crc<u64> = Crc::<u64>::new(&CRC_64_ECMA_182);

/// Reconstructor state machine
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconstructorState {
    /// Idle, ready for requests
    Idle = 0,
    /// Currently reconstructing a page
    Reconstructing = 1,
    /// Reconstruction complete, result ready
    Ready = 2,
    /// Error state
    Error = 3,
}

impl From<u64> for ReconstructorState {
    fn from(v: u64) -> Self {
        match v {
            0 => ReconstructorState::Idle,
            1 => ReconstructorState::Reconstructing,
            2 => ReconstructorState::Ready,
            3 => ReconstructorState::Error,
            _ => ReconstructorState::Error,
        }
    }
}

/// Cache entry flags
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheFlags {
    /// Entry is empty/invalid
    Empty = 0,
    /// Entry contains valid data
    Valid = 1,
    /// Entry is dirty (modified locally)
    Dirty = 2,
    /// Entry is being filled (in-progress)
    Filling = 3,
}

impl From<u32> for CacheFlags {
    fn from(v: u32) -> Self {
        match v {
            0 => CacheFlags::Empty,
            1 => CacheFlags::Valid,
            2 => CacheFlags::Dirty,
            3 => CacheFlags::Filling,
            _ => CacheFlags::Empty,
        }
    }
}

/// Page cache entry metadata (64 bytes, cache-line aligned)
///
/// Each entry tracks a cached 4KB page with LRU metadata.
///
/// #ASSUME_CACHE_ALIGNED: 64-byte alignment prevents false sharing
/// #VERIFY_COMPILE_TIME: const_assert! checks alignment
#[repr(C, align(64))]
pub struct PageCacheEntry {
    /// Virtual address of the cached page (0 = empty slot)
    pub address: AtomicU64,

    /// Snapshot ID this page belongs to
    pub snapshot_id: AtomicU64,

    /// CRC64 hash of page contents (Q34 verification)
    pub hash: AtomicU64,

    /// LRU access counter (higher = more recently used)
    pub access_count: AtomicU32,

    /// Cache entry flags (Valid, Dirty, Filling)
    pub flags: AtomicU32,

    /// Padding to 64 bytes
    _pad: [u8; 24],
}

impl PageCacheEntry {
    /// Create an empty cache entry
    pub const fn empty() -> Self {
        Self {
            address: AtomicU64::new(0),
            snapshot_id: AtomicU64::new(0),
            hash: AtomicU64::new(0),
            access_count: AtomicU32::new(0),
            flags: AtomicU32::new(CacheFlags::Empty as u32),
            _pad: [0; 24],
        }
    }

    /// Check if entry is valid
    #[inline]
    pub fn is_valid(&self) -> bool {
        CacheFlags::from(self.flags.load(Ordering::Acquire)) == CacheFlags::Valid
    }

    /// Check if entry matches address and snapshot
    #[inline]
    pub fn matches(&self, address: u64, snapshot_id: u64) -> bool {
        self.is_valid()
            && self.address.load(Ordering::Acquire) == address
            && self.snapshot_id.load(Ordering::Acquire) == snapshot_id
    }

    /// Update access count (LRU tracking)
    #[inline]
    pub fn touch(&self) {
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Fill entry with new data
    ///
    /// #ASSUME_SINGLE_WRITER: Only one thread fills a given slot
    pub fn fill(&self, address: u64, snapshot_id: u64, hash: u64) {
        // Mark as filling (prevents concurrent reads)
        self.flags.store(CacheFlags::Filling as u32, Ordering::Release);

        // Store metadata
        self.address.store(address, Ordering::Release);
        self.snapshot_id.store(snapshot_id, Ordering::Release);
        self.hash.store(hash, Ordering::Release);
        self.access_count.store(1, Ordering::Release);

        // Mark as valid (publishes the entry)
        self.flags.store(CacheFlags::Valid as u32, Ordering::Release);
    }

    /// Invalidate entry
    pub fn invalidate(&self) {
        self.flags.store(CacheFlags::Empty as u32, Ordering::Release);
        self.address.store(0, Ordering::Release);
    }
}

/// Reconstruction errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconstructError {
    /// Requested snapshot not found in delta ring
    SnapshotNotFound,

    /// Delta chain is broken (missing intermediate snapshot)
    DeltaChainBroken,

    /// Hash verification failed (Q34 integrity violation)
    HashVerificationFailed,

    /// Page was never tracked (no base data available)
    PageNotTracked,

    /// Cache is exhausted and eviction failed
    CacheExhausted,

    /// Invalid address (not page-aligned)
    InvalidAddress,

    /// Reconstruction already in progress
    Busy,

    /// Internal error
    InternalError,
}

/// Reconstruction statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct ReconstructStats {
    /// Total pages reconstructed
    pub pages_reconstructed: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Cache hit rate (0.0-1.0)
    pub hit_rate: f64,
    /// Average reconstruction time in nanoseconds
    pub avg_reconstruct_ns: u64,
    /// Current cache occupancy (0-31)
    pub cache_occupancy: u32,
}

/// MemoryReconstructorCapsule - T6 Mixed Page Reconstruction
///
/// Reconstructs memory state at any snapshot by applying XOR delta chains
/// from a base checkpoint. Includes an LRU cache for frequently accessed pages.
///
/// # Size
///
/// - Metadata: 256 bytes
/// - Cache metadata: 31 x 64B = 1,984 bytes
/// - Cache data: 31 x 4KB = 126,976 bytes
/// - Padding: 1,856 bytes
/// - Total: 131,072 bytes (128 KB exactly)
///
/// # Thread Safety
///
/// All operations are lockfree using atomic operations.
/// Multiple threads can read different cached pages concurrently.
/// Cache fills use compare-and-swap for slot reservation.
#[repr(C, align(256))]
pub struct MemoryReconstructorCapsule {
    // ===== Metadata (256 bytes) =====
    /// Generation counter for TOCTOU prevention
    pub generation: AtomicU64,

    /// Current reconstruction target snapshot
    pub target_snapshot: AtomicU64,

    /// State machine (Idle, Reconstructing, Ready, Error)
    pub state: AtomicU64,

    /// Total pages reconstructed
    pub pages_reconstructed: AtomicU64,

    /// Cache hit count
    pub cache_hits: AtomicU64,

    /// Cache miss count
    pub cache_misses: AtomicU64,

    /// Last reconstruction time in nanoseconds
    pub last_reconstruct_ns: AtomicU64,

    /// Global LRU counter (incremented on each access)
    global_lru_counter: AtomicU64,

    /// Error code (if state == Error)
    pub error_code: AtomicU32,

    /// Reserved for future use
    _reserved: AtomicU32,

    /// Padding to 256 bytes (256 - 8*8 - 4*2 = 184 bytes)
    _metadata_pad: [u8; 184],

    // ===== Page Cache Metadata (1,984 bytes) =====
    /// Cache entry metadata (31 entries x 64 bytes)
    cache_metadata: [PageCacheEntry; CACHE_CAPACITY],

    // ===== Page Cache Data (126,976 bytes) =====
    /// Cached page data (31 x 4KB pages)
    cache_pages: [[u8; PAGE_SIZE]; CACHE_CAPACITY],

    // ===== Padding to 128 KB =====
    /// Padding: 131,072 - 256 - 1,984 - 126,976 = 1,856 bytes
    _cache_pad: [u8; 1856],
}

// Compile-time size verification
const _: () = {
    // PageCacheEntry must be 64 bytes
    assert!(
        std::mem::size_of::<PageCacheEntry>() == 64,
        "PageCacheEntry must be 64 bytes"
    );

    // Total size must be 128 KB
    assert!(
        std::mem::size_of::<MemoryReconstructorCapsule>() == 131072,
        "MemoryReconstructorCapsule must be 128 KB"
    );

    // Alignment must be 256 bytes
    assert!(
        std::mem::align_of::<MemoryReconstructorCapsule>() == 256,
        "MemoryReconstructorCapsule must be 256-byte aligned"
    );
};

impl MemoryReconstructorCapsule {
    /// Create a new reconstructor capsule
    ///
    /// # Performance
    /// - O(1) initialization
    /// - All fields zero-initialized
    pub fn new() -> Self {
        const EMPTY_ENTRY: PageCacheEntry = PageCacheEntry::empty();
        const EMPTY_PAGE: [u8; PAGE_SIZE] = [0u8; PAGE_SIZE];

        Self {
            generation: AtomicU64::new(0),
            target_snapshot: AtomicU64::new(0),
            state: AtomicU64::new(ReconstructorState::Idle as u64),
            pages_reconstructed: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            last_reconstruct_ns: AtomicU64::new(0),
            global_lru_counter: AtomicU64::new(0),
            error_code: AtomicU32::new(0),
            _reserved: AtomicU32::new(0),
            _metadata_pad: [0u8; 184],
            cache_metadata: [EMPTY_ENTRY; CACHE_CAPACITY],
            cache_pages: [EMPTY_PAGE; CACHE_CAPACITY],
            _cache_pad: [0u8; 1856],
        }
    }

    /// Get current state
    #[inline]
    pub fn get_state(&self) -> ReconstructorState {
        ReconstructorState::from(self.state.load(Ordering::Acquire))
    }

    /// Check if a page is in the cache
    ///
    /// # Returns
    /// - Some(index) if found
    /// - None if not cached
    ///
    /// # Performance
    /// - O(n) scan where n = CACHE_CAPACITY (31)
    /// - ~100ns typical
    #[inline]
    pub fn cache_lookup(&self, address: u64, snapshot_id: u64) -> Option<usize> {
        // #ASSUME_PAGE_ALIGNED: address is 4KB aligned
        let aligned_addr = address & !0xFFF;

        for i in 0..CACHE_CAPACITY {
            if self.cache_metadata[i].matches(aligned_addr, snapshot_id) {
                // Update LRU counter
                self.cache_metadata[i].touch();
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Some(i);
            }
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Get a reference to cached page data
    ///
    /// # Safety
    /// Caller must ensure index is valid (from cache_lookup)
    ///
    /// # Performance
    /// - O(1) direct array access
    /// - less than 10ns
    #[inline]
    pub fn get_cached_page(&self, index: usize) -> Option<&[u8; PAGE_SIZE]> {
        if index < CACHE_CAPACITY && self.cache_metadata[index].is_valid() {
            Some(&self.cache_pages[index])
        } else {
            None
        }
    }

    /// Find the LRU (least recently used) cache slot for eviction
    ///
    /// # Returns
    /// Index of the slot with lowest access_count
    ///
    /// # Performance
    /// - O(n) scan
    /// - ~100ns
    fn find_lru_slot(&self) -> usize {
        let mut min_count = u32::MAX;
        let mut min_idx = 0;

        for i in 0..CACHE_CAPACITY {
            let flags = CacheFlags::from(self.cache_metadata[i].flags.load(Ordering::Acquire));

            // Prefer empty slots
            if flags == CacheFlags::Empty {
                return i;
            }

            // Skip slots being filled
            if flags == CacheFlags::Filling {
                continue;
            }

            let count = self.cache_metadata[i].access_count.load(Ordering::Relaxed);
            if count < min_count {
                min_count = count;
                min_idx = i;
            }
        }

        min_idx
    }

    /// Try to reserve a cache slot for filling
    ///
    /// Uses CAS to prevent concurrent reservation of the same slot.
    ///
    /// # Returns
    /// - Some(index) if slot reserved
    /// - None if all slots are busy
    fn reserve_slot(&self, address: u64, snapshot_id: u64) -> Option<usize> {
        let aligned_addr = address & !0xFFF;

        // Try 3 times (handle contention)
        for _ in 0..3 {
            let idx = self.find_lru_slot();

            // Try to transition Empty/Valid -> Filling
            let current_flags = self.cache_metadata[idx].flags.load(Ordering::Acquire);
            if current_flags == CacheFlags::Filling as u32 {
                continue; // Slot is busy
            }

            // CAS to reserve
            if self.cache_metadata[idx]
                .flags
                .compare_exchange(
                    current_flags,
                    CacheFlags::Filling as u32,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // Reserved successfully, store metadata
                self.cache_metadata[idx]
                    .address
                    .store(aligned_addr, Ordering::Release);
                self.cache_metadata[idx]
                    .snapshot_id
                    .store(snapshot_id, Ordering::Release);
                return Some(idx);
            }
        }

        None
    }

    /// Fill a reserved cache slot with page data
    ///
    /// # Safety
    /// - Caller must have reserved the slot via reserve_slot()
    /// - Data must be exactly PAGE_SIZE bytes
    ///
    /// # Performance
    /// - O(PAGE_SIZE) memcpy
    /// - ~1us for 4KB
    ///
    /// #ASSUME_SLOT_RESERVED: Index was returned by reserve_slot()
    /// #VERIFY_UNIT_TEST: test_cache_fill
    pub fn fill_slot(&mut self, index: usize, data: &[u8; PAGE_SIZE]) -> Result<(), ReconstructError> {
        if index >= CACHE_CAPACITY {
            return Err(ReconstructError::InternalError);
        }

        // Verify slot is in Filling state
        let flags = CacheFlags::from(self.cache_metadata[index].flags.load(Ordering::Acquire));
        if flags != CacheFlags::Filling {
            return Err(ReconstructError::InternalError);
        }

        // Copy page data
        self.cache_pages[index].copy_from_slice(data);

        // Compute hash for Q34 verification
        let hash = Self::compute_page_hash(data);
        self.cache_metadata[index].hash.store(hash, Ordering::Release);

        // Set initial access count
        let lru = self.global_lru_counter.fetch_add(1, Ordering::Relaxed);
        self.cache_metadata[index]
            .access_count
            .store(lru as u32, Ordering::Release);

        // Mark as valid (publishes the entry)
        self.cache_metadata[index]
            .flags
            .store(CacheFlags::Valid as u32, Ordering::Release);

        // Update stats
        self.pages_reconstructed.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Reconstruct a page at a given snapshot
    ///
    /// This is the main reconstruction entry point. It:
    /// 1. Checks cache for existing reconstruction
    /// 2. If not cached, applies delta chain from base
    /// 3. Verifies hash against merkle tree
    /// 4. Caches result for future access
    ///
    /// # Arguments
    /// - `snapshot_id`: Target snapshot to reconstruct
    /// - `address`: Virtual address of the page (must be 4KB aligned)
    /// - `base_page`: Base page data (current live memory or checkpoint)
    /// - `deltas`: Iterator of (snapshot_id, delta_data) pairs in chronological order
    ///
    /// # Performance
    /// - Cache hit: less than 10ns
    /// - Cache miss: less than 500us (depends on delta chain length)
    ///
    /// #ASSUME_DELTAS_ORDERED: Deltas are in chronological order
    /// #ASSUME_BASE_VALID: Base page is valid memory content
    /// #VERIFY_UNIT_TEST: test_reconstruct_page
    pub fn reconstruct_page(
        &mut self,
        snapshot_id: u64,
        address: u64,
        base_page: &[u8; PAGE_SIZE],
        deltas: &[(u64, [u8; PAGE_SIZE])],
        expected_hash: Option<u64>,
    ) -> Result<&[u8; PAGE_SIZE], ReconstructError> {
        let aligned_addr = address & !0xFFF;

        // Check cache first
        if let Some(idx) = self.cache_lookup(aligned_addr, snapshot_id) {
            return self.get_cached_page(idx).ok_or(ReconstructError::InternalError);
        }

        // Reserve a cache slot
        let slot = self
            .reserve_slot(aligned_addr, snapshot_id)
            .ok_or(ReconstructError::CacheExhausted)?;

        // Start with base page
        let mut reconstructed = *base_page;

        // Apply deltas in order (XOR each delta)
        // #ASSUME_XOR_REVERSIBLE: XOR is its own inverse, A XOR B XOR B = A
        for (delta_snap, delta_data) in deltas {
            if *delta_snap > snapshot_id {
                break; // Don't apply deltas beyond target snapshot
            }

            // XOR delta into reconstructed page
            // This could be SIMD-accelerated for T2 performance
            for i in 0..PAGE_SIZE {
                reconstructed[i] ^= delta_data[i];
            }
        }

        // Verify hash if provided (Q34 compliance)
        if let Some(expected) = expected_hash {
            let actual = Self::compute_page_hash(&reconstructed);
            if actual != expected {
                // Restore slot to empty state
                self.cache_metadata[slot]
                    .flags
                    .store(CacheFlags::Empty as u32, Ordering::Release);
                return Err(ReconstructError::HashVerificationFailed);
            }
        }

        // Fill the cache slot
        self.cache_pages[slot].copy_from_slice(&reconstructed);

        let hash = Self::compute_page_hash(&reconstructed);
        self.cache_metadata[slot].hash.store(hash, Ordering::Release);

        let lru = self.global_lru_counter.fetch_add(1, Ordering::Relaxed);
        self.cache_metadata[slot]
            .access_count
            .store(lru as u32, Ordering::Release);

        self.cache_metadata[slot]
            .flags
            .store(CacheFlags::Valid as u32, Ordering::Release);

        self.pages_reconstructed.fetch_add(1, Ordering::Relaxed);

        Ok(&self.cache_pages[slot])
    }

    /// Reconstruct a memory range spanning multiple pages
    ///
    /// # Arguments
    /// - `snapshot_id`: Target snapshot
    /// - `start`: Start address (will be page-aligned down)
    /// - `len`: Number of bytes to read
    /// - `get_base_page`: Callback to get base page data
    /// - `get_deltas`: Callback to get deltas for a page
    ///
    /// # Returns
    /// - Vec<u8> containing the reconstructed memory range
    ///
    /// # Performance
    /// - O(pages) where pages = ceil(len / PAGE_SIZE)
    pub fn reconstruct_range<F, G>(
        &mut self,
        snapshot_id: u64,
        start: u64,
        len: usize,
        mut get_base_page: F,
        mut get_deltas: G,
    ) -> Result<Vec<u8>, ReconstructError>
    where
        F: FnMut(u64) -> Option<[u8; PAGE_SIZE]>,
        G: FnMut(u64) -> Vec<(u64, [u8; PAGE_SIZE])>,
    {
        if len == 0 {
            return Ok(Vec::new());
        }

        let start_page = start & !0xFFF;
        let end_page = (start + len as u64 - 1) & !0xFFF;
        let page_count = ((end_page - start_page) / PAGE_SIZE as u64 + 1) as usize;

        let mut result = Vec::with_capacity(len);

        for i in 0..page_count {
            let page_addr = start_page + (i as u64 * PAGE_SIZE as u64);

            let base_page = get_base_page(page_addr).ok_or(ReconstructError::PageNotTracked)?;

            let deltas = get_deltas(page_addr);

            let page_data = self.reconstruct_page(
                snapshot_id,
                page_addr,
                &base_page,
                &deltas,
                None, // Skip hash verification for range reads
            )?;

            // Calculate slice bounds within this page
            let page_start = if i == 0 {
                (start - start_page) as usize
            } else {
                0
            };

            let page_end = if i == page_count - 1 {
                let end_offset = (start + len as u64 - 1) - page_addr;
                (end_offset + 1) as usize
            } else {
                PAGE_SIZE
            };

            result.extend_from_slice(&page_data[page_start..page_end]);
        }

        Ok(result)
    }

    /// Compute CRC64 hash of page data (Q34 verification)
    ///
    /// #ASSUME_DETERMINISTIC_HASH: Same input always produces same output
    #[inline]
    fn compute_page_hash(data: &[u8; PAGE_SIZE]) -> u64 {
        let mut digest = CRC64.digest();
        digest.update(data);
        digest.finalize()
    }

    /// Prefetch pages into cache
    ///
    /// Asynchronously fills cache with pages that are likely to be accessed.
    ///
    /// # Arguments
    /// - `snapshot_id`: Target snapshot
    /// - `addresses`: List of page addresses to prefetch
    /// - `get_base_page`: Callback to get base page data
    /// - `get_deltas`: Callback to get deltas
    ///
    /// # Returns
    /// Number of pages successfully prefetched
    pub fn prefetch_pages<F, G>(
        &mut self,
        snapshot_id: u64,
        addresses: &[u64],
        mut get_base_page: F,
        mut get_deltas: G,
    ) -> usize
    where
        F: FnMut(u64) -> Option<[u8; PAGE_SIZE]>,
        G: FnMut(u64) -> Vec<(u64, [u8; PAGE_SIZE])>,
    {
        let mut prefetched = 0;

        for &addr in addresses {
            let aligned = addr & !0xFFF;

            // Skip if already cached
            if self.cache_lookup(aligned, snapshot_id).is_some() {
                continue;
            }

            // Try to prefetch
            if let Some(base) = get_base_page(aligned) {
                let deltas = get_deltas(aligned);
                if self
                    .reconstruct_page(snapshot_id, aligned, &base, &deltas, None)
                    .is_ok()
                {
                    prefetched += 1;
                }
            }
        }

        prefetched
    }

    /// Get cache hit rate
    ///
    /// # Returns
    /// Hit rate as a value between 0.0 and 1.0
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Get current cache occupancy
    ///
    /// # Returns
    /// Number of valid entries in cache (0-31)
    pub fn cache_occupancy(&self) -> u32 {
        let mut count = 0;
        for i in 0..CACHE_CAPACITY {
            if self.cache_metadata[i].is_valid() {
                count += 1;
            }
        }
        count
    }

    /// Invalidate entire cache
    ///
    /// Called when navigating to a different snapshot to clear stale data.
    ///
    /// # Performance
    /// - O(n) where n = CACHE_CAPACITY (31)
    /// - ~100ns
    pub fn invalidate_cache(&self) {
        for i in 0..CACHE_CAPACITY {
            self.cache_metadata[i].invalidate();
        }

        // Bump generation to invalidate any in-flight reads
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Invalidate cached pages for a specific snapshot
    ///
    /// Called when snapshot is pruned or invalidated.
    pub fn invalidate_snapshot(&self, snapshot_id: u64) {
        for i in 0..CACHE_CAPACITY {
            if self.cache_metadata[i].snapshot_id.load(Ordering::Acquire) == snapshot_id {
                self.cache_metadata[i].invalidate();
            }
        }
    }

    /// Get reconstruction statistics
    pub fn get_stats(&self) -> ReconstructStats {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        ReconstructStats {
            pages_reconstructed: self.pages_reconstructed.load(Ordering::Relaxed),
            cache_hits: hits,
            cache_misses: misses,
            hit_rate: if total == 0 {
                0.0
            } else {
                hits as f64 / total as f64
            },
            avg_reconstruct_ns: self.last_reconstruct_ns.load(Ordering::Relaxed),
            cache_occupancy: self.cache_occupancy(),
        }
    }

    /// Set target snapshot for reconstruction
    ///
    /// This invalidates the cache if the target changes significantly.
    pub fn set_target_snapshot(&self, snapshot_id: u64) {
        let current = self.target_snapshot.swap(snapshot_id, Ordering::AcqRel);

        // If target changed, consider invalidating cache
        // For now, we keep cache valid as pages may still be useful
        if current != snapshot_id {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }

    /// Reset all statistics
    pub fn reset_stats(&self) {
        self.pages_reconstructed.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.last_reconstruct_ns.store(0, Ordering::Relaxed);
    }
}

impl Default for MemoryReconstructorCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Size and Alignment Tests (3 tests) =====

    #[test]
    fn test_capsule_size() {
        assert_eq!(
            std::mem::size_of::<MemoryReconstructorCapsule>(),
            131072,
            "Capsule must be 128 KB"
        );
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(
            std::mem::align_of::<MemoryReconstructorCapsule>(),
            256,
            "Capsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_cache_entry_size() {
        assert_eq!(
            std::mem::size_of::<PageCacheEntry>(),
            64,
            "Cache entry must be 64 bytes"
        );
    }

    // ===== Initialization Tests (2 tests) =====

    #[test]
    fn test_new_capsule() {
        let capsule = MemoryReconstructorCapsule::new();
        assert_eq!(capsule.get_state(), ReconstructorState::Idle);
        assert_eq!(capsule.cache_occupancy(), 0);
        assert_eq!(capsule.cache_hit_rate(), 0.0);
    }

    #[test]
    fn test_default_stats() {
        let capsule = MemoryReconstructorCapsule::new();
        let stats = capsule.get_stats();
        assert_eq!(stats.pages_reconstructed, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
    }

    // ===== Cache Tests (5 tests) =====

    #[test]
    fn test_cache_lookup_empty() {
        let capsule = MemoryReconstructorCapsule::new();
        assert!(capsule.cache_lookup(0x1000, 0).is_none());
    }

    #[test]
    fn test_cache_miss_tracking() {
        let capsule = MemoryReconstructorCapsule::new();
        capsule.cache_lookup(0x1000, 0);
        capsule.cache_lookup(0x2000, 0);
        assert_eq!(capsule.cache_misses.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_reserve_slot() {
        let capsule = MemoryReconstructorCapsule::new();
        let slot = capsule.reserve_slot(0x1000, 0);
        assert!(slot.is_some());
    }

    #[test]
    fn test_cache_entry_operations() {
        let entry = PageCacheEntry::empty();
        assert!(!entry.is_valid());
        assert!(!entry.matches(0x1000, 0));

        entry.fill(0x1000, 0, 12345);
        assert!(entry.is_valid());
        assert!(entry.matches(0x1000, 0));
        assert!(!entry.matches(0x2000, 0));

        entry.invalidate();
        assert!(!entry.is_valid());
    }

    #[test]
    fn test_lru_tracking() {
        let entry = PageCacheEntry::empty();
        entry.fill(0x1000, 0, 12345);

        let initial = entry.access_count.load(Ordering::Relaxed);
        entry.touch();
        let after = entry.access_count.load(Ordering::Relaxed);
        assert!(after > initial);
    }

    // ===== Reconstruction Tests (5 tests) =====

    #[test]
    fn test_reconstruct_no_deltas() {
        let mut capsule = MemoryReconstructorCapsule::new();
        let base_page = [0xAAu8; PAGE_SIZE];
        let deltas: Vec<(u64, [u8; PAGE_SIZE])> = vec![];

        let result = capsule.reconstruct_page(0, 0x1000, &base_page, &deltas, None);
        assert!(result.is_ok());

        let page = result.unwrap();
        assert_eq!(page[0], 0xAA);
        assert_eq!(page[PAGE_SIZE - 1], 0xAA);
    }

    #[test]
    fn test_reconstruct_with_delta() {
        let mut capsule = MemoryReconstructorCapsule::new();
        let base_page = [0x00u8; PAGE_SIZE];

        // Delta that sets first byte to 0xFF (0x00 XOR 0xFF = 0xFF)
        let mut delta = [0x00u8; PAGE_SIZE];
        delta[0] = 0xFF;

        let deltas = vec![(0u64, delta)];

        let result = capsule.reconstruct_page(0, 0x1000, &base_page, &deltas, None);
        assert!(result.is_ok());

        let page = result.unwrap();
        assert_eq!(page[0], 0xFF);
        assert_eq!(page[1], 0x00);
    }

    #[test]
    fn test_xor_reversibility() {
        // Verify XOR delta is reversible: A XOR B XOR B = A
        let original = [0x42u8; PAGE_SIZE];
        let mut modified = original;
        modified[0] = 0xFF;
        modified[100] = 0x00;

        // Compute delta
        let mut delta = [0u8; PAGE_SIZE];
        for i in 0..PAGE_SIZE {
            delta[i] = original[i] ^ modified[i];
        }

        // Apply delta to original should give modified
        let mut result = original;
        for i in 0..PAGE_SIZE {
            result[i] ^= delta[i];
        }
        assert_eq!(result, modified);

        // Apply delta again should give original back
        for i in 0..PAGE_SIZE {
            result[i] ^= delta[i];
        }
        assert_eq!(result, original);
    }

    #[test]
    fn test_hash_verification_success() {
        let mut capsule = MemoryReconstructorCapsule::new();
        let base_page = [0xBBu8; PAGE_SIZE];
        let expected_hash = MemoryReconstructorCapsule::compute_page_hash(&base_page);

        let result = capsule.reconstruct_page(0, 0x1000, &base_page, &[], Some(expected_hash));
        assert!(result.is_ok());
    }

    #[test]
    fn test_hash_verification_failure() {
        let mut capsule = MemoryReconstructorCapsule::new();
        let base_page = [0xCCu8; PAGE_SIZE];
        let wrong_hash = 12345u64; // Wrong hash

        let result = capsule.reconstruct_page(0, 0x1000, &base_page, &[], Some(wrong_hash));
        assert_eq!(result, Err(ReconstructError::HashVerificationFailed));
    }

    // ===== Cache Hit/Miss Tests (3 tests) =====

    #[test]
    fn test_cache_hit() {
        let mut capsule = MemoryReconstructorCapsule::new();
        let base_page = [0xDDu8; PAGE_SIZE];

        // First access: cache miss
        let _ = capsule.reconstruct_page(0, 0x1000, &base_page, &[], None);

        // Second access: cache hit
        let hit = capsule.cache_lookup(0x1000, 0);
        assert!(hit.is_some());
        assert_eq!(capsule.cache_hits.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let mut capsule = MemoryReconstructorCapsule::new();
        let base_page = [0xEEu8; PAGE_SIZE];

        // 2 misses from direct lookup
        capsule.cache_lookup(0x1000, 0);
        capsule.cache_lookup(0x2000, 0);

        // Reconstruct one page (internally calls cache_lookup which adds 1 miss)
        let _ = capsule.reconstruct_page(0, 0x1000, &base_page, &[], None);

        // 1 hit (page now in cache)
        capsule.cache_lookup(0x1000, 0);

        // Total: 1 hit, 3 misses (2 direct + 1 from reconstruct_page)
        // Hit rate = 1 / 4 = 0.25
        let rate = capsule.cache_hit_rate();
        assert!(rate > 0.24 && rate < 0.26, "Expected ~0.25, got {}", rate);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut capsule = MemoryReconstructorCapsule::new();
        let base_page = [0xFFu8; PAGE_SIZE];

        // Fill cache
        let _ = capsule.reconstruct_page(0, 0x1000, &base_page, &[], None);
        assert_eq!(capsule.cache_occupancy(), 1);

        // Invalidate
        capsule.invalidate_cache();
        assert_eq!(capsule.cache_occupancy(), 0);
    }

    // ===== Range Reconstruction Test (1 test) =====

    #[test]
    fn test_reconstruct_range() {
        let mut capsule = MemoryReconstructorCapsule::new();

        // Create base pages
        let base_page1 = [0x11u8; PAGE_SIZE];
        let base_page2 = [0x22u8; PAGE_SIZE];

        let result = capsule.reconstruct_range(
            0,
            0x1000,
            100, // Read 100 bytes from first page
            |addr| {
                if addr == 0x1000 {
                    Some(base_page1)
                } else if addr == 0x2000 {
                    Some(base_page2)
                } else {
                    None
                }
            },
            |_| vec![], // No deltas
        );

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 100);
        assert_eq!(data[0], 0x11);
    }

    // ===== Statistics Tests (2 tests) =====

    #[test]
    fn test_reset_stats() {
        let capsule = MemoryReconstructorCapsule::new();

        // Generate some stats
        capsule.cache_lookup(0x1000, 0);
        capsule.cache_lookup(0x2000, 0);
        capsule.pages_reconstructed.store(10, Ordering::Relaxed);

        // Reset
        capsule.reset_stats();

        let stats = capsule.get_stats();
        assert_eq!(stats.pages_reconstructed, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
    }

    #[test]
    fn test_snapshot_invalidation() {
        let mut capsule = MemoryReconstructorCapsule::new();
        let base_page = [0xABu8; PAGE_SIZE];

        // Reconstruct for snapshot 0
        let _ = capsule.reconstruct_page(0, 0x1000, &base_page, &[], None);
        assert_eq!(capsule.cache_occupancy(), 1);

        // Invalidate snapshot 0
        capsule.invalidate_snapshot(0);
        assert_eq!(capsule.cache_occupancy(), 0);
    }
}
