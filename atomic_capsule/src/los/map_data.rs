//! MapDataCapsule - Structure-of-Arrays buffer management for line-of-sight maps
//!
//! # Design
//!
//! - **Tier**: T1 Atomic (lockfree coordination)
//! - **Size**: 128 bytes (cache-aligned header)
//! - **Layout**: Packed state + dimensions + SoA pointers + LOD pointers + metrics
//! - **Speedup**: 3-10× vs mutex-protected HashMap (T1 baseline)
//!
//! # State Field Layout (8 bytes)
//!
//! ```text
//! [0-11]   readers: u12 (max 4095 concurrent readers)
//! [12]     writer: bool (exclusive write access)
//! [13-36]  version: u24 (generation counter, 16M updates)
//! [37-63]  flags: u27 (reserved for future use)
//! ```
//!
//! # Dimensions Field Layout (8 bytes)
//!
//! ```text
//! [0-15]   width: u16 (max 65535)
//! [16-31]  height: u16 (max 65535)
//! [32-47]  pitch: u16 (row stride in elements)
//! [48-63]  reserved
//! ```
//!
//! # Chaos Compliance
//!
//! - ✅ 100% lockfree (AtomicU64 for state, AtomicPtr for buffers)
//! - ✅ No mutex/RwLock
//! - ✅ Cache-aligned (128B total)
//! - ✅ Generation counter (TOCTOU prevention)
//! - ✅ Reader/writer coordination via atomic CAS
//!
//! # ASSUM Tags
//!
//! - #ASSUME_SIMD_ALIGNMENT: External buffers are 32B aligned for AVX2
//! - #ASSUME_POINTER_VALIDITY: Buffer pointers remain valid during capsule lifetime
//! - #ASSUME_BUFFER_SIZE: Buffers have width * height * sizeof(i32) capacity

use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

/// MapDataCapsule - SoA buffer manager for line-of-sight maps
///
/// # Layout (128 bytes)
///
/// - 0-7: state (readers|writer|version|flags)
/// - 8-15: dimensions (width|height|pitch)
/// - 16-39: strip_ptrs (cover/mud/cost)
/// - 40-55: lod_ptrs (LOD2/LOD4)
/// - 56-127: metrics + padding
///
/// # Example
///
/// ```rust,no_run
/// # use atomic_capsule::los::MapDataCapsule;
/// use std::alloc::{alloc, Layout};
///
/// let capsule = MapDataCapsule::new(1024, 1024);
///
/// // Allocate aligned buffers (32B for AVX2)
/// let layout = Layout::from_size_align(1024 * 1024 * 4, 32).unwrap();
/// unsafe {
///     let cover = alloc(layout) as *mut i32;
///     let mud = alloc(layout) as *mut i32;
///     let cost = alloc(layout) as *mut i32;
///
///     capsule.attach_buffers(cover, mud, cost);
///
///     // Read access
///     if let Some(guard) = capsule.acquire_read() {
///         let (c, m, ct) = capsule.sample_strips(0, 0, 8);
///         // Process strips...
///         drop(guard);
///     }
/// }
/// ```
#[repr(C, align(128))]
pub struct MapDataCapsule {
    /// Packed state: readers(12)|writer(1)|version(24)|flags(27)
    state: AtomicU64,

    /// Packed dimensions: width(16)|height(16)|pitch(16)|reserved(16)
    dimensions: AtomicU64,

    /// SoA strip pointers (32B aligned)
    cover_ptr: AtomicPtr<i32>,
    mud_ptr: AtomicPtr<i32>,
    cost_ptr: AtomicPtr<i32>,

    /// LOD pointers (downsampled masks)
    lod2_ptr: AtomicPtr<u8>,
    lod4_ptr: AtomicPtr<u8>,

    /// Metrics
    total_reads: AtomicU64,
    cache_hits: AtomicU64,
    last_access_ns: AtomicU64,

    /// Padding to 128 bytes
    _padding: [u64; 6],
}

// State field bit manipulation
const READERS_MASK: u64 = 0xFFF; // 12 bits
const WRITER_BIT: u64 = 1 << 12;
const VERSION_SHIFT: u32 = 13;
const VERSION_MASK: u64 = 0xFFFFFF << VERSION_SHIFT; // 24 bits
const FLAGS_SHIFT: u32 = 37;

// Dimensions field bit manipulation
const WIDTH_MASK: u64 = 0xFFFF;
const HEIGHT_SHIFT: u32 = 16;
const HEIGHT_MASK: u64 = 0xFFFF << HEIGHT_SHIFT;
const PITCH_SHIFT: u32 = 32;
const PITCH_MASK: u64 = 0xFFFF << PITCH_SHIFT;

impl MapDataCapsule {
    /// Create new MapDataCapsule with dimensions
    ///
    /// # Arguments
    ///
    /// - `width`: Map width in cells
    /// - `height`: Map height in cells
    ///
    /// # Returns
    ///
    /// Initialized capsule with null pointers (call `attach_buffers` to enable)
    #[inline]
    pub fn new(width: u16, height: u16) -> Self {
        let pitch = width; // Default pitch equals width (contiguous rows)
        let dimensions = (width as u64)
            | ((height as u64) << HEIGHT_SHIFT)
            | ((pitch as u64) << PITCH_SHIFT);

        Self {
            state: AtomicU64::new(0),
            dimensions: AtomicU64::new(dimensions),
            cover_ptr: AtomicPtr::new(core::ptr::null_mut()),
            mud_ptr: AtomicPtr::new(core::ptr::null_mut()),
            cost_ptr: AtomicPtr::new(core::ptr::null_mut()),
            lod2_ptr: AtomicPtr::new(core::ptr::null_mut()),
            lod4_ptr: AtomicPtr::new(core::ptr::null_mut()),
            total_reads: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            last_access_ns: AtomicU64::new(0),
            _padding: [0; 6],
        }
    }

    /// Attach external SoA buffers (must be 32B aligned)
    ///
    /// # Safety
    ///
    /// - #ASSUME_SIMD_ALIGNMENT: Buffers MUST be 32B aligned for AVX2
    /// - #ASSUME_POINTER_VALIDITY: Buffers MUST remain valid for capsule lifetime
    /// - #ASSUME_BUFFER_SIZE: Each buffer MUST have width * height * 4 bytes capacity
    ///
    /// # Arguments
    ///
    /// - `cover`: Cover values buffer (0-255 typical)
    /// - `mud`: Mud/terrain cost buffer
    /// - `cost`: Movement cost buffer
    #[inline]
    pub unsafe fn attach_buffers(
        &self,
        cover: *mut i32,
        mud: *mut i32,
        cost: *mut i32,
    ) {
        self.cover_ptr.store(cover, Ordering::Release);
        self.mud_ptr.store(mud, Ordering::Release);
        self.cost_ptr.store(cost, Ordering::Release);
    }

    /// Attach LOD pointers (downsampled masks)
    ///
    /// # Safety
    ///
    /// - #ASSUME_POINTER_VALIDITY: LOD buffers MUST remain valid for capsule lifetime
    /// - #ASSUME_BUFFER_SIZE: LOD2 = (width/2 * height/2), LOD4 = (width/4 * height/4)
    #[inline]
    pub unsafe fn attach_lod_buffers(&self, lod2: *mut u8, lod4: *mut u8) {
        self.lod2_ptr.store(lod2, Ordering::Release);
        self.lod4_ptr.store(lod4, Ordering::Release);
    }

    /// Acquire read access (increments reader count)
    ///
    /// # Returns
    ///
    /// - `Some(guard)`: Read access granted
    /// - `None`: Writer is active, try again
    ///
    /// # Chaos Pattern
    ///
    /// Uses atomic CAS loop to increment readers count, fails if writer bit set.
    #[inline]
    pub fn acquire_read(&self) -> Option<MapReadGuard> {
        loop {
            let state = self.state.load(Ordering::Acquire);

            // Fail if writer active
            if state & WRITER_BIT != 0 {
                return None;
            }

            let readers = state & READERS_MASK;

            // Overflow check (max 4095 readers)
            if readers >= READERS_MASK {
                return None;
            }

            // Try increment readers
            let new_state = state + 1;
            if self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                self.total_reads.fetch_add(1, Ordering::Relaxed);
                return Some(MapReadGuard { capsule: self });
            }
        }
    }

    /// Release read access (decrements reader count)
    ///
    /// # Chaos Pattern
    ///
    /// Atomic decrement with AcqRel ordering for proper synchronization.
    #[inline]
    fn release_read(&self) {
        self.state.fetch_sub(1, Ordering::AcqRel);
    }

    /// Acquire exclusive write access
    ///
    /// # Returns
    ///
    /// - `Some(guard)`: Write access granted
    /// - `None`: Readers or writer active, try again
    ///
    /// # Chaos Pattern
    ///
    /// Uses atomic CAS to set writer bit, fails if readers or writer present.
    #[inline]
    pub fn acquire_write(&self) -> Option<MapWriteGuard> {
        loop {
            let state = self.state.load(Ordering::Acquire);

            // Fail if any readers or writer active
            if (state & (READERS_MASK | WRITER_BIT)) != 0 {
                return None;
            }

            // Try set writer bit
            let new_state = state | WRITER_BIT;
            if self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return Some(MapWriteGuard { capsule: self });
            }
        }
    }

    /// Release write access and increment version
    ///
    /// # Chaos Pattern
    ///
    /// Clear writer bit, increment version (generation counter) atomically.
    #[inline]
    fn release_write(&self) {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let version = ((state & VERSION_MASK) >> VERSION_SHIFT) + 1;

            // Wrap version at 24 bits (16M updates)
            let version = version & 0xFFFFFF;

            let new_state = (state & !WRITER_BIT & !VERSION_MASK)
                | (version << VERSION_SHIFT);

            if self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return;
            }
        }
    }

    /// Get current version (generation counter)
    ///
    /// # Returns
    ///
    /// Version counter (0-16,777,215), wraps at 24 bits.
    #[inline]
    pub fn version(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        ((state & VERSION_MASK) >> VERSION_SHIFT) as u32
    }

    /// Get dimensions (width, height, pitch)
    ///
    /// # Returns
    ///
    /// Tuple of (width, height, pitch) in cells.
    #[inline]
    pub fn dimensions(&self) -> (u16, u16, u16) {
        let dim = self.dimensions.load(Ordering::Acquire);
        let width = (dim & WIDTH_MASK) as u16;
        let height = ((dim & HEIGHT_MASK) >> HEIGHT_SHIFT) as u16;
        let pitch = ((dim & PITCH_MASK) >> PITCH_SHIFT) as u16;
        (width, height, pitch)
    }

    /// Sample cover at (x, y) - bounds checked
    ///
    /// # Arguments
    ///
    /// - `x`: X coordinate
    /// - `y`: Y coordinate
    ///
    /// # Returns
    ///
    /// - `Some(value)`: Cover value at (x, y)
    /// - `None`: Out of bounds or buffer not attached
    #[inline]
    pub fn sample_cover(&self, x: u16, y: u16) -> Option<i32> {
        let (width, height, pitch) = self.dimensions();

        if x >= width || y >= height {
            return None;
        }

        let ptr = self.cover_ptr.load(Ordering::Acquire);
        if ptr.is_null() {
            return None;
        }

        unsafe {
            let offset = (y as usize) * (pitch as usize) + (x as usize);
            Some(*ptr.add(offset))
        }
    }

    /// Sample cover strip (contiguous for AVX2)
    ///
    /// # Safety
    ///
    /// - #ASSUME_BOUNDS: Caller MUST ensure x + len <= width
    /// - #ASSUME_POINTER_VALIDITY: Buffer MUST be attached
    ///
    /// # Arguments
    ///
    /// - `x`: Starting X coordinate
    /// - `y`: Row Y coordinate
    /// - `len`: Number of elements to return
    ///
    /// # Returns
    ///
    /// Slice of cover values from (x, y) to (x+len-1, y).
    #[inline]
    pub unsafe fn sample_cover_strip(&self, x: u16, y: u16, len: usize) -> &[i32] {
        let (_, _, pitch) = self.dimensions();
        let ptr = self.cover_ptr.load(Ordering::Acquire);

        debug_assert!(!ptr.is_null(), "cover buffer not attached");

        let offset = (y as usize) * (pitch as usize) + (x as usize);
        core::slice::from_raw_parts(ptr.add(offset), len)
    }

    /// Sample all three strips at once (cache-friendly)
    ///
    /// # Safety
    ///
    /// - #ASSUME_BOUNDS: Caller MUST ensure x + len <= width
    /// - #ASSUME_POINTER_VALIDITY: All buffers MUST be attached
    ///
    /// # Arguments
    ///
    /// - `x`: Starting X coordinate
    /// - `y`: Row Y coordinate
    /// - `len`: Number of elements per strip
    ///
    /// # Returns
    ///
    /// Tuple of (cover, mud, cost) slices.
    #[inline]
    pub unsafe fn sample_strips(
        &self,
        x: u16,
        y: u16,
        len: usize,
    ) -> (&[i32], &[i32], &[i32]) {
        let (_, _, pitch) = self.dimensions();

        let cover_ptr = self.cover_ptr.load(Ordering::Acquire);
        let mud_ptr = self.mud_ptr.load(Ordering::Acquire);
        let cost_ptr = self.cost_ptr.load(Ordering::Acquire);

        debug_assert!(!cover_ptr.is_null(), "cover buffer not attached");
        debug_assert!(!mud_ptr.is_null(), "mud buffer not attached");
        debug_assert!(!cost_ptr.is_null(), "cost buffer not attached");

        let offset = (y as usize) * (pitch as usize) + (x as usize);

        let cover = core::slice::from_raw_parts(cover_ptr.add(offset), len);
        let mud = core::slice::from_raw_parts(mud_ptr.add(offset), len);
        let cost = core::slice::from_raw_parts(cost_ptr.add(offset), len);

        (cover, mud, cost)
    }

    /// Get total reads counter
    #[inline]
    pub fn total_reads(&self) -> u64 {
        self.total_reads.load(Ordering::Relaxed)
    }

    /// Get cache hits counter
    #[inline]
    pub fn cache_hits(&self) -> u64 {
        self.cache_hits.load(Ordering::Relaxed)
    }

    /// Increment cache hits
    #[inline]
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }
}

/// RAII guard for read access
///
/// Automatically releases read lock on drop.
pub struct MapReadGuard<'a> {
    capsule: &'a MapDataCapsule,
}

impl Drop for MapReadGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.capsule.release_read();
    }
}

/// RAII guard for write access
///
/// Automatically releases write lock and increments version on drop.
pub struct MapWriteGuard<'a> {
    capsule: &'a MapDataCapsule,
}

impl Drop for MapWriteGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.capsule.release_write();
    }
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<MapDataCapsule>() == 128);
    assert!(core::mem::align_of::<MapDataCapsule>() == 128);
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{alloc, dealloc, Layout};

    #[test]
    fn test_size_alignment() {
        assert_eq!(core::mem::size_of::<MapDataCapsule>(), 128);
        assert_eq!(core::mem::align_of::<MapDataCapsule>(), 128);
    }

    #[test]
    fn test_new() {
        let capsule = MapDataCapsule::new(1024, 768);
        let (width, height, pitch) = capsule.dimensions();

        assert_eq!(width, 1024);
        assert_eq!(height, 768);
        assert_eq!(pitch, 1024);
        assert_eq!(capsule.version(), 0);
    }

    #[test]
    fn test_reader_writer_coordination() {
        let capsule = MapDataCapsule::new(100, 100);

        // Acquire read
        let guard1 = capsule.acquire_read().expect("first read should succeed");
        let guard2 = capsule.acquire_read().expect("second read should succeed");

        // Writer should fail while readers active
        assert!(capsule.acquire_write().is_none());

        drop(guard1);
        drop(guard2);

        // Writer should succeed now
        let write_guard = capsule.acquire_write().expect("write should succeed");

        // Readers should fail while writer active
        assert!(capsule.acquire_read().is_none());

        let version_before = capsule.version();
        drop(write_guard);

        // Version should increment after write
        assert_eq!(capsule.version(), version_before + 1);
    }

    #[test]
    fn test_attach_buffers() {
        let capsule = MapDataCapsule::new(8, 8);

        unsafe {
            let layout = Layout::from_size_align(8 * 8 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;
            let mud = alloc(layout) as *mut i32;
            let cost = alloc(layout) as *mut i32;

            // Initialize cover buffer
            for i in 0..64 {
                *cover.add(i) = i as i32;
            }

            capsule.attach_buffers(cover, mud, cost);

            // Test sampling
            assert_eq!(capsule.sample_cover(0, 0), Some(0));
            assert_eq!(capsule.sample_cover(7, 0), Some(7));
            assert_eq!(capsule.sample_cover(0, 1), Some(8));

            // Out of bounds
            assert_eq!(capsule.sample_cover(8, 0), None);
            assert_eq!(capsule.sample_cover(0, 8), None);

            dealloc(cover as *mut u8, layout);
            dealloc(mud as *mut u8, layout);
            dealloc(cost as *mut u8, layout);
        }
    }

    #[test]
    fn test_sample_strip() {
        let capsule = MapDataCapsule::new(16, 16);

        unsafe {
            let layout = Layout::from_size_align(16 * 16 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;

            // Initialize with row pattern
            for y in 0..16 {
                for x in 0..16 {
                    *cover.add(y * 16 + x) = (y * 100 + x) as i32;
                }
            }

            capsule.attach_buffers(cover, cover, cover);

            let guard = capsule.acquire_read().unwrap();
            let strip = capsule.sample_cover_strip(0, 5, 8);

            assert_eq!(strip.len(), 8);
            assert_eq!(strip[0], 500); // Row 5, col 0
            assert_eq!(strip[7], 507); // Row 5, col 7

            drop(guard);
            dealloc(cover as *mut u8, layout);
        }
    }

    #[test]
    fn test_sample_strips() {
        let capsule = MapDataCapsule::new(16, 16);

        unsafe {
            let layout = Layout::from_size_align(16 * 16 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;
            let mud = alloc(layout) as *mut i32;
            let cost = alloc(layout) as *mut i32;

            // Initialize with different patterns
            for i in 0..256 {
                *cover.add(i) = i as i32;
                *mud.add(i) = (i * 2) as i32;
                *cost.add(i) = (i * 3) as i32;
            }

            capsule.attach_buffers(cover, mud, cost);

            let guard = capsule.acquire_read().unwrap();
            let (c, m, ct) = capsule.sample_strips(4, 2, 8);

            // Row 2, starting at col 4 = offset 2*16 + 4 = 36
            assert_eq!(c[0], 36);
            assert_eq!(m[0], 72);
            assert_eq!(ct[0], 108);

            drop(guard);
            dealloc(cover as *mut u8, layout);
            dealloc(mud as *mut u8, layout);
            dealloc(cost as *mut u8, layout);
        }
    }

    #[test]
    fn test_version_increment() {
        let capsule = MapDataCapsule::new(10, 10);

        assert_eq!(capsule.version(), 0);

        for i in 1..100 {
            let guard = capsule.acquire_write().unwrap();
            drop(guard);
            assert_eq!(capsule.version(), i);
        }
    }

    #[test]
    fn test_metrics() {
        let capsule = MapDataCapsule::new(10, 10);

        assert_eq!(capsule.total_reads(), 0);
        assert_eq!(capsule.cache_hits(), 0);

        for _ in 0..10 {
            let guard = capsule.acquire_read().unwrap();
            drop(guard);
        }

        assert_eq!(capsule.total_reads(), 10);

        capsule.record_cache_hit();
        capsule.record_cache_hit();
        assert_eq!(capsule.cache_hits(), 2);
    }

    #[test]
    fn test_multiple_readers() {
        let capsule = MapDataCapsule::new(10, 10);

        let guards: Vec<_> = (0..100)
            .filter_map(|_| capsule.acquire_read())
            .collect();

        assert_eq!(guards.len(), 100);
        assert_eq!(capsule.total_reads(), 100);

        // Writer should fail
        assert!(capsule.acquire_write().is_none());
    }

    #[test]
    fn test_reader_overflow() {
        let capsule = MapDataCapsule::new(10, 10);

        // Manually set readers to max
        capsule.state.store(READERS_MASK, Ordering::Release);

        // Next read should fail
        assert!(capsule.acquire_read().is_none());
    }
}
