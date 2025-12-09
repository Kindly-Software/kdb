//! MapBuilder - Safe RAII builder for MapDataCapsule
//!
//! # Design Philosophy
//!
//! Provides 100% safe API for constructing MapDataCapsule with owned, aligned buffers.
//! Users never touch `unsafe` - all allocation, alignment, and lifetime management
//! is handled internally with proper ASSUM tagging.
//!
//! # Example
//!
//! ```rust
//! use atomic_capsule::los::{MapBuilder, MapDataCapsule};
//!
//! // Safe builder pattern (no unsafe for users)
//! let map = MapBuilder::new(1024, 768)
//!     .with_cover(vec![0i32; 1024 * 768])
//!     .with_mud(vec![1i32; 1024 * 768])
//!     .with_cost(vec![5i32; 1024 * 768])
//!     .build()
//!     .expect("buffer size mismatch");
//!
//! // Safe access methods
//! let cover_value = map.cover()[100];
//! let capsule_ref = map.capsule();
//! ```
//!
//! # Chaos Compliance
//!
//! - ✅ 100% safe API for users
//! - ✅ Internal unsafe properly guarded with ASSUM tags
//! - ✅ RAII cleanup via Drop
//! - ✅ 32B SIMD alignment (AVX2 ready)
//! - ✅ Owned buffers (no lifetime issues)
//!
//! # ASSUM Tags (Internal Only)
//!
//! - #ASSUME_ALLOC_SUCCESS: std::alloc::alloc returns non-null (checked)
//! - #ASSUME_BUFFER_LIFETIME: Buffers live until Drop (RAII guaranteed)
//! - #ASSUME_ALIGNMENT: Allocated with 32B alignment (Layout enforced)

use crate::los::MapDataCapsule;
use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

/// Safe RAII builder for MapDataCapsule
///
/// Owns allocated buffers, handles alignment, provides safe API.
/// All unsafe operations are internal and properly tagged.
pub struct MapBuilder {
    width: u16,
    height: u16,
    cover: Option<Vec<i32>>,
    mud: Option<Vec<i32>>,
    cost: Option<Vec<i32>>,
}

impl MapBuilder {
    /// Create new builder with dimensions
    ///
    /// # Arguments
    ///
    /// - `width`: Map width in cells
    /// - `height`: Map height in cells
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::los::MapBuilder;
    ///
    /// let builder = MapBuilder::new(1024, 768);
    /// ```
    #[inline]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cover: None,
            mud: None,
            cost: None,
        }
    }

    /// Attach cover buffer (takes ownership)
    ///
    /// # Arguments
    ///
    /// - `data`: Cover values (0-255 typical), length must equal width * height
    ///
    /// # Example
    ///
    /// ```rust
    /// # use atomic_capsule::los::MapBuilder;
    /// let builder = MapBuilder::new(100, 100)
    ///     .with_cover(vec![0i32; 100 * 100]);
    /// ```
    #[inline]
    pub fn with_cover(mut self, data: Vec<i32>) -> Self {
        self.cover = Some(data);
        self
    }

    /// Attach cover buffer from slice (copies data)
    ///
    /// # Arguments
    ///
    /// - `data`: Cover values slice, length must equal width * height
    ///
    /// # Example
    ///
    /// ```rust
    /// # use atomic_capsule::los::MapBuilder;
    /// let cover_data = &[0i32; 10000];
    /// let builder = MapBuilder::new(100, 100)
    ///     .with_cover_slice(cover_data);
    /// ```
    #[inline]
    pub fn with_cover_slice(mut self, data: &[i32]) -> Self {
        self.cover = Some(data.to_vec());
        self
    }

    /// Attach mud buffer (takes ownership)
    ///
    /// # Arguments
    ///
    /// - `data`: Mud/terrain cost values, length must equal width * height
    ///
    /// # Example
    ///
    /// ```rust
    /// # use atomic_capsule::los::MapBuilder;
    /// let builder = MapBuilder::new(100, 100)
    ///     .with_mud(vec![1i32; 100 * 100]);
    /// ```
    #[inline]
    pub fn with_mud(mut self, data: Vec<i32>) -> Self {
        self.mud = Some(data);
        self
    }

    /// Attach mud buffer from slice (copies data)
    ///
    /// # Arguments
    ///
    /// - `data`: Mud values slice, length must equal width * height
    #[inline]
    pub fn with_mud_slice(mut self, data: &[i32]) -> Self {
        self.mud = Some(data.to_vec());
        self
    }

    /// Attach cost buffer (takes ownership)
    ///
    /// # Arguments
    ///
    /// - `data`: Movement cost values, length must equal width * height
    ///
    /// # Example
    ///
    /// ```rust
    /// # use atomic_capsule::los::MapBuilder;
    /// let builder = MapBuilder::new(100, 100)
    ///     .with_cost(vec![5i32; 100 * 100]);
    /// ```
    #[inline]
    pub fn with_cost(mut self, data: Vec<i32>) -> Self {
        self.cost = Some(data);
        self
    }

    /// Attach cost buffer from slice (copies data)
    ///
    /// # Arguments
    ///
    /// - `data`: Cost values slice, length must equal width * height
    #[inline]
    pub fn with_cost_slice(mut self, data: &[i32]) -> Self {
        self.cost = Some(data.to_vec());
        self
    }

    /// Build MapData with owned buffers
    ///
    /// Allocates 32B-aligned buffers, copies data, attaches to capsule.
    /// All unsafe operations are internal and properly guarded.
    ///
    /// # Errors
    ///
    /// Returns `MapBuilderError` if:
    /// - Buffer size doesn't match width * height
    /// - Required buffers (cover/mud/cost) not provided
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::los::MapBuilder;
    ///
    /// let map = MapBuilder::new(100, 100)
    ///     .with_cover(vec![0i32; 10000])
    ///     .with_mud(vec![1i32; 10000])
    ///     .with_cost(vec![5i32; 10000])
    ///     .build()
    ///     .expect("valid buffers");
    /// ```
    pub fn build(self) -> Result<MapData, MapBuilderError> {
        let expected_len = (self.width as usize) * (self.height as usize);

        // Validate all buffers provided
        let cover = self.cover.ok_or(MapBuilderError::MissingBuffer("cover"))?;
        let mud = self.mud.ok_or(MapBuilderError::MissingBuffer("mud"))?;
        let cost = self.cost.ok_or(MapBuilderError::MissingBuffer("cost"))?;

        // Validate buffer sizes
        if cover.len() != expected_len {
            return Err(MapBuilderError::InvalidBufferSize {
                buffer: "cover",
                expected: expected_len,
                actual: cover.len(),
            });
        }
        if mud.len() != expected_len {
            return Err(MapBuilderError::InvalidBufferSize {
                buffer: "mud",
                expected: expected_len,
                actual: mud.len(),
            });
        }
        if cost.len() != expected_len {
            return Err(MapBuilderError::InvalidBufferSize {
                buffer: "cost",
                expected: expected_len,
                actual: cost.len(),
            });
        }

        // Allocate 32B-aligned buffers (AVX2 ready)
        let buffers = unsafe { AlignedBuffers::allocate(self.width, self.height)? };

        // Copy data to aligned buffers
        unsafe {
            std::ptr::copy_nonoverlapping(cover.as_ptr(), buffers.cover.as_ptr(), expected_len);
            std::ptr::copy_nonoverlapping(mud.as_ptr(), buffers.mud.as_ptr(), expected_len);
            std::ptr::copy_nonoverlapping(cost.as_ptr(), buffers.cost.as_ptr(), expected_len);
        }

        // Create capsule and attach buffers
        let capsule = MapDataCapsule::new(self.width, self.height);

        unsafe {
            capsule.attach_buffers(
                buffers.cover.as_ptr(),
                buffers.mud.as_ptr(),
                buffers.cost.as_ptr(),
            );
        }

        Ok(MapData { capsule, buffers })
    }
}

/// Owned MapData with RAII cleanup
///
/// Provides safe access to MapDataCapsule and owned buffers.
/// Automatically deallocates aligned buffers on drop.
pub struct MapData {
    capsule: MapDataCapsule,
    buffers: AlignedBuffers,
}

impl MapData {
    /// Get reference to underlying capsule
    ///
    /// # Example
    ///
    /// ```rust
    /// # use atomic_capsule::los::MapBuilder;
    /// # let map = MapBuilder::new(10, 10)
    /// #     .with_cover(vec![0i32; 100])
    /// #     .with_mud(vec![1i32; 100])
    /// #     .with_cost(vec![5i32; 100])
    /// #     .build()
    /// #     .unwrap();
    /// let capsule = map.capsule();
    /// let (width, height, pitch) = capsule.dimensions();
    /// ```
    #[inline]
    pub fn capsule(&self) -> &MapDataCapsule {
        &self.capsule
    }

    /// Get dimensions (width, height, pitch)
    #[inline]
    pub fn dimensions(&self) -> (u16, u16, u16) {
        self.capsule.dimensions()
    }

    /// Safe access to cover buffer
    ///
    /// # Example
    ///
    /// ```rust
    /// # use atomic_capsule::los::MapBuilder;
    /// # let map = MapBuilder::new(10, 10)
    /// #     .with_cover(vec![0i32; 100])
    /// #     .with_mud(vec![1i32; 100])
    /// #     .with_cost(vec![5i32; 100])
    /// #     .build()
    /// #     .unwrap();
    /// let cover = map.cover();
    /// assert_eq!(cover.len(), 100);
    /// ```
    #[inline]
    pub fn cover(&self) -> &[i32] {
        self.buffers.cover_slice()
    }

    /// Safe access to mud buffer
    #[inline]
    pub fn mud(&self) -> &[i32] {
        self.buffers.mud_slice()
    }

    /// Safe access to cost buffer
    #[inline]
    pub fn cost(&self) -> &[i32] {
        self.buffers.cost_slice()
    }

    /// Sample cover value at (x, y) - bounds checked
    ///
    /// # Example
    ///
    /// ```rust
    /// # use atomic_capsule::los::MapBuilder;
    /// # let map = MapBuilder::new(10, 10)
    /// #     .with_cover(vec![42i32; 100])
    /// #     .with_mud(vec![1i32; 100])
    /// #     .with_cost(vec![5i32; 100])
    /// #     .build()
    /// #     .unwrap();
    /// assert_eq!(map.sample_cover(5, 5), Some(42));
    /// assert_eq!(map.sample_cover(10, 10), None); // Out of bounds
    /// ```
    #[inline]
    pub fn sample_cover(&self, x: u16, y: u16) -> Option<i32> {
        self.capsule.sample_cover(x, y)
    }

    /// Get current version (generation counter)
    #[inline]
    pub fn version(&self) -> u32 {
        self.capsule.version()
    }

    /// Acquire read access guard
    ///
    /// # Example
    ///
    /// ```rust
    /// # use atomic_capsule::los::MapBuilder;
    /// # let map = MapBuilder::new(10, 10)
    /// #     .with_cover(vec![0i32; 100])
    /// #     .with_mud(vec![1i32; 100])
    /// #     .with_cost(vec![5i32; 100])
    /// #     .build()
    /// #     .unwrap();
    /// if let Some(guard) = map.acquire_read() {
    ///     // Perform read operations
    ///     drop(guard); // Explicit release
    /// }
    /// ```
    #[inline]
    pub fn acquire_read(&self) -> Option<crate::los::map_data::MapReadGuard<'_>> {
        self.capsule.acquire_read()
    }

    /// Acquire write access guard
    #[inline]
    pub fn acquire_write(&self) -> Option<crate::los::map_data::MapWriteGuard<'_>> {
        self.capsule.acquire_write()
    }
}

/// Internal: 32B-aligned buffers with RAII cleanup
///
/// All unsafe operations tagged with ASSUM.
struct AlignedBuffers {
    cover: NonNull<i32>,
    mud: NonNull<i32>,
    cost: NonNull<i32>,
    layout: Layout,
}

impl AlignedBuffers {
    /// Allocate 32B-aligned buffers
    ///
    /// # Safety
    ///
    /// - #ASSUME_ALLOC_SUCCESS: std::alloc::alloc returns non-null pointer
    /// - #ASSUME_ALIGNMENT: Layout guarantees 32B alignment for AVX2
    unsafe fn allocate(width: u16, height: u16) -> Result<Self, MapBuilderError> {
        let size = (width as usize) * (height as usize) * std::mem::size_of::<i32>();

        // 32B alignment for AVX2 (256-bit vectors = 32 bytes)
        let layout = Layout::from_size_align(size, 32)
            .map_err(|_| MapBuilderError::AllocationFailed("invalid layout"))?;

        // #VERIFY_ALLOC_SUCCESS: Check non-null after allocation
        let cover_ptr = alloc(layout) as *mut i32;
        if cover_ptr.is_null() {
            return Err(MapBuilderError::AllocationFailed("cover buffer"));
        }

        let mud_ptr = alloc(layout) as *mut i32;
        if mud_ptr.is_null() {
            dealloc(cover_ptr as *mut u8, layout);
            return Err(MapBuilderError::AllocationFailed("mud buffer"));
        }

        let cost_ptr = alloc(layout) as *mut i32;
        if cost_ptr.is_null() {
            dealloc(cover_ptr as *mut u8, layout);
            dealloc(mud_ptr as *mut u8, layout);
            return Err(MapBuilderError::AllocationFailed("cost buffer"));
        }

        // #VERIFY_ALIGNMENT: Debug check alignment
        debug_assert_eq!(cover_ptr as usize % 32, 0, "cover buffer not 32B aligned");
        debug_assert_eq!(mud_ptr as usize % 32, 0, "mud buffer not 32B aligned");
        debug_assert_eq!(cost_ptr as usize % 32, 0, "cost buffer not 32B aligned");

        Ok(Self {
            cover: NonNull::new_unchecked(cover_ptr),
            mud: NonNull::new_unchecked(mud_ptr),
            cost: NonNull::new_unchecked(cost_ptr),
            layout,
        })
    }

    /// Safe slice access to cover buffer
    #[inline]
    fn cover_slice(&self) -> &[i32] {
        unsafe {
            let len = self.layout.size() / std::mem::size_of::<i32>();
            std::slice::from_raw_parts(self.cover.as_ptr(), len)
        }
    }

    /// Safe slice access to mud buffer
    #[inline]
    fn mud_slice(&self) -> &[i32] {
        unsafe {
            let len = self.layout.size() / std::mem::size_of::<i32>();
            std::slice::from_raw_parts(self.mud.as_ptr(), len)
        }
    }

    /// Safe slice access to cost buffer
    #[inline]
    fn cost_slice(&self) -> &[i32] {
        unsafe {
            let len = self.layout.size() / std::mem::size_of::<i32>();
            std::slice::from_raw_parts(self.cost.as_ptr(), len)
        }
    }
}

impl Drop for AlignedBuffers {
    /// RAII cleanup: deallocate all aligned buffers
    ///
    /// # Safety
    ///
    /// - #ASSUME_BUFFER_LIFETIME: Buffers were allocated with same Layout
    /// - #ASSUME_NO_DOUBLE_FREE: Drop called exactly once (Rust guarantees)
    fn drop(&mut self) {
        unsafe {
            dealloc(self.cover.as_ptr() as *mut u8, self.layout);
            dealloc(self.mud.as_ptr() as *mut u8, self.layout);
            dealloc(self.cost.as_ptr() as *mut u8, self.layout);
        }
    }
}

/// MapBuilder errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapBuilderError {
    /// Required buffer not provided
    MissingBuffer(&'static str),

    /// Buffer size doesn't match dimensions
    InvalidBufferSize {
        buffer: &'static str,
        expected: usize,
        actual: usize,
    },

    /// Memory allocation failed
    AllocationFailed(&'static str),
}

impl std::fmt::Display for MapBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingBuffer(name) => write!(f, "missing buffer: {}", name),
            Self::InvalidBufferSize {
                buffer,
                expected,
                actual,
            } => write!(
                f,
                "buffer '{}' size mismatch: expected {}, got {}",
                buffer, expected, actual
            ),
            Self::AllocationFailed(msg) => write!(f, "allocation failed: {}", msg),
        }
    }
}

impl std::error::Error for MapBuilderError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let map = MapBuilder::new(10, 10)
            .with_cover(vec![0i32; 100])
            .with_mud(vec![1i32; 100])
            .with_cost(vec![5i32; 100])
            .build()
            .expect("valid buffers");

        let (width, height, pitch) = map.dimensions();
        assert_eq!(width, 10);
        assert_eq!(height, 10);
        assert_eq!(pitch, 10);
    }

    #[test]
    fn test_builder_slice_input() {
        let cover_data = vec![42i32; 100];
        let mud_data = vec![7i32; 100];
        let cost_data = vec![3i32; 100];

        let map = MapBuilder::new(10, 10)
            .with_cover_slice(&cover_data)
            .with_mud_slice(&mud_data)
            .with_cost_slice(&cost_data)
            .build()
            .expect("valid slices");

        assert_eq!(map.cover()[0], 42);
        assert_eq!(map.mud()[0], 7);
        assert_eq!(map.cost()[0], 3);
    }

    #[test]
    fn test_safe_access() {
        let map = MapBuilder::new(8, 8)
            .with_cover((0..64).collect())
            .with_mud(vec![1i32; 64])
            .with_cost(vec![2i32; 64])
            .build()
            .unwrap();

        // Safe slice access
        let cover = map.cover();
        assert_eq!(cover.len(), 64);
        assert_eq!(cover[0], 0);
        assert_eq!(cover[63], 63);

        // Safe sample access
        assert_eq!(map.sample_cover(0, 0), Some(0));
        assert_eq!(map.sample_cover(7, 7), Some(63));
        assert_eq!(map.sample_cover(8, 0), None); // Out of bounds
    }

    #[test]
    fn test_error_missing_buffer() {
        let result = MapBuilder::new(10, 10)
            .with_cover(vec![0i32; 100])
            .with_mud(vec![1i32; 100])
            // Missing cost buffer
            .build();

        assert_eq!(result.err(), Some(MapBuilderError::MissingBuffer("cost")));
    }

    #[test]
    fn test_error_invalid_size() {
        let result = MapBuilder::new(10, 10)
            .with_cover(vec![0i32; 50]) // Wrong size
            .with_mud(vec![1i32; 100])
            .with_cost(vec![5i32; 100])
            .build();

        match result {
            Err(MapBuilderError::InvalidBufferSize {
                buffer,
                expected,
                actual,
            }) => {
                assert_eq!(buffer, "cover");
                assert_eq!(expected, 100);
                assert_eq!(actual, 50);
            }
            _ => panic!("expected InvalidBufferSize error"),
        }
    }

    #[test]
    fn test_raii_cleanup() {
        // Should not leak memory (verified by Miri/Valgrind)
        {
            let map = MapBuilder::new(1024, 1024)
                .with_cover(vec![0i32; 1024 * 1024])
                .with_mud(vec![1i32; 1024 * 1024])
                .with_cost(vec![5i32; 1024 * 1024])
                .build()
                .unwrap();

            let _ = map.cover()[0];
        } // Drop cleans up all allocations
    }

    #[test]
    fn test_alignment() {
        let map = MapBuilder::new(16, 16)
            .with_cover(vec![0i32; 256])
            .with_mud(vec![1i32; 256])
            .with_cost(vec![2i32; 256])
            .build()
            .unwrap();

        // Verify 32B alignment (AVX2 ready)
        let cover_ptr = map.cover().as_ptr() as usize;
        let mud_ptr = map.mud().as_ptr() as usize;
        let cost_ptr = map.cost().as_ptr() as usize;

        assert_eq!(cover_ptr % 32, 0, "cover not 32B aligned");
        assert_eq!(mud_ptr % 32, 0, "mud not 32B aligned");
        assert_eq!(cost_ptr % 32, 0, "cost not 32B aligned");
    }

    #[test]
    fn test_reader_writer_coordination() {
        let map = MapBuilder::new(10, 10)
            .with_cover(vec![0i32; 100])
            .with_mud(vec![1i32; 100])
            .with_cost(vec![5i32; 100])
            .build()
            .unwrap();

        // Multiple readers OK
        let guard1 = map.acquire_read().expect("first read");
        let guard2 = map.acquire_read().expect("second read");

        // Writer blocked
        assert!(map.acquire_write().is_none());

        drop(guard1);
        drop(guard2);

        // Writer succeeds
        let write_guard = map.acquire_write().expect("write");

        // Readers blocked
        assert!(map.acquire_read().is_none());

        let version_before = map.version();
        drop(write_guard);

        // Version incremented
        assert_eq!(map.version(), version_before + 1);
    }

    #[test]
    fn test_large_map() {
        // Test with realistic map size
        let width = 2048;
        let height = 2048;
        let size = (width * height) as usize;

        let map = MapBuilder::new(width as u16, height as u16)
            .with_cover(vec![0i32; size])
            .with_mud(vec![1i32; size])
            .with_cost(vec![5i32; size])
            .build()
            .expect("large map allocation");

        assert_eq!(map.cover().len(), size);
        assert_eq!(map.dimensions(), (width as u16, height as u16, width as u16));
    }
}
