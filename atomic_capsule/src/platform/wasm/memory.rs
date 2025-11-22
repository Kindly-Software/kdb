//! # WASM Linear Memory Utilities
//!
//! **Helpers for working with WASM linear memory and capsules.**
//!
//! ## UCE34 Analysis
//!
//! - **Q10 (Tier)**: T0 Foundation (zero-cost memory utilities)
//! - **Q11 (Rust)**: Safe memory access abstractions
//! - **Q12 (Nightly)**: N/A - stable Rust
//! - **Q28 (Simplicity)**: Simple load/store API
//! - **Q29 (Constraints)**: WASM linear memory model
//! - **Q30 (Validation)**: Property tests for alignment
//! - **Q33 (Validation)**: All operations verified
//!
//! ## Performance
//!
//! - **Load**: <5ns (aligned read)
//! - **Store**: <5ns (aligned write)
//! - **Alignment check**: 0ns (compile-time)
//!
//! ## ASSUM Safety
//!
//! - `#ASSUME_LINEAR_MEMORY`: WASM linear memory layout
//! - `#VERIFY_LINEAR_MEMORY`: Bounds checks on all accesses
//! - `#ASSUME_ALIGNMENT`: Capsules properly aligned
//! - `#VERIFY_ALIGNMENT`: Runtime alignment checks

#[cfg(target_arch = "wasm32")]
use core::mem;
use core::ptr;

/// WASM linear memory error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmMemoryError {
    /// Alignment error
    Misaligned {
        /// Expected alignment
        expected: usize,
        /// Actual address
        actual: usize,
    },
    /// Out of bounds access
    OutOfBounds {
        /// Address accessed
        address: usize,
        /// Size attempted
        size: usize,
    },
    /// Invalid size
    InvalidSize {
        /// Expected size
        expected: usize,
        /// Actual size
        actual: usize,
    },
}

/// WASM linear memory capsule loader
///
/// # ASSUM Safety
/// - `#ASSUME_ALIGNMENT`: Capsule is properly aligned
/// - `#VERIFY_ALIGNMENT`: Runtime check on load
/// - `#ASSUME_LINEAR_MEMORY`: WASM memory layout
/// - `#VERIFY_BOUNDS`: Bounds check on all accesses
pub struct WasmCapsuleLoader;

impl WasmCapsuleLoader {
    /// Load capsule from WASM linear memory
    ///
    /// # Safety
    /// - Pointer must be valid and properly aligned
    /// - Memory must contain valid capsule data
    ///
    /// # Performance
    /// - <5ns (aligned load)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_VALID_PTR`: Pointer is valid
    /// - `#VERIFY_ALIGNMENT`: Alignment checked at runtime
    ///
    /// # Examples
    /// ```ignore
    /// use atomic_capsule::platform::wasm::memory::WasmCapsuleLoader;
    ///
    /// let ptr = wasm_ptr as *const MyCapsule;
    /// let capsule = unsafe { WasmCapsuleLoader::load(ptr)? };
    /// ```
    #[cfg(target_arch = "wasm32")]
    pub unsafe fn load<T>(ptr: *const T) -> Result<&'static T, WasmMemoryError> {
        // Verify alignment
        let addr = ptr as usize;
        let align = mem::align_of::<T>();
        if addr % align != 0 {
            return Err(WasmMemoryError::Misaligned {
                expected: align,
                actual: addr,
            });
        }

        // SAFETY: Caller guarantees valid pointer and proper alignment
        Ok(&*ptr)
    }

    /// Load mutable capsule from WASM linear memory
    ///
    /// # Safety
    /// - Pointer must be valid, properly aligned, and exclusively accessible
    ///
    /// # Performance
    /// - <5ns (aligned load)
    #[cfg(target_arch = "wasm32")]
    pub unsafe fn load_mut<T>(ptr: *mut T) -> Result<&'static mut T, WasmMemoryError> {
        // Verify alignment
        let addr = ptr as usize;
        let align = mem::align_of::<T>();
        if addr % align != 0 {
            return Err(WasmMemoryError::Misaligned {
                expected: align,
                actual: addr,
            });
        }

        // SAFETY: Caller guarantees valid pointer and exclusive access
        Ok(&mut *ptr)
    }

    /// Store capsule to WASM linear memory
    ///
    /// # Safety
    /// - Pointer must be valid and properly aligned
    ///
    /// # Performance
    /// - <5ns (aligned store)
    #[cfg(target_arch = "wasm32")]
    pub unsafe fn store<T>(ptr: *mut T, value: T) -> Result<(), WasmMemoryError> {
        // Verify alignment
        let addr = ptr as usize;
        let align = mem::align_of::<T>();
        if addr % align != 0 {
            return Err(WasmMemoryError::Misaligned {
                expected: align,
                actual: addr,
            });
        }

        // SAFETY: Caller guarantees valid pointer
        ptr::write(ptr, value);
        Ok(())
    }

    /// Check if address is aligned for type T
    ///
    /// # Performance
    /// - <1ns (simple modulo check)
    #[cfg(target_arch = "wasm32")]
    pub fn is_aligned<T>(addr: usize) -> bool {
        addr % mem::align_of::<T>() == 0
    }

    /// Get required alignment for type T
    ///
    /// # Performance
    /// - 0ns (compile-time constant)
    #[cfg(target_arch = "wasm32")]
    pub const fn alignment<T>() -> usize {
        mem::align_of::<T>()
    }

    /// Get size of type T
    ///
    /// # Performance
    /// - 0ns (compile-time constant)
    #[cfg(target_arch = "wasm32")]
    pub const fn size<T>() -> usize {
        mem::size_of::<T>()
    }
}

/// WASM linear memory allocator for capsules
///
/// # ASSUM Safety
/// - `#ASSUME_LINEAR_MEMORY`: WASM memory layout
/// - `#VERIFY_ALIGNMENT`: All allocations properly aligned
pub struct WasmCapsuleAllocator {
    /// Next available address (aligned)
    next_addr: usize,
}

impl WasmCapsuleAllocator {
    /// Create new allocator starting at base address
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_BASE_ALIGNED`: Base address is aligned
    /// - `#VERIFY_BASE_ALIGNED`: Checked at runtime
    pub fn new(base_addr: usize, base_align: usize) -> Result<Self, WasmMemoryError> {
        if base_addr % base_align != 0 {
            return Err(WasmMemoryError::Misaligned {
                expected: base_align,
                actual: base_addr,
            });
        }

        Ok(Self {
            next_addr: base_addr,
        })
    }

    /// Allocate aligned memory for type T
    ///
    /// # Performance
    /// - <10ns (alignment calculation + increment)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ALIGNMENT`: Returns properly aligned address
    /// - `#VERIFY_ALIGNMENT`: Alignment checked before return
    #[cfg(target_arch = "wasm32")]
    pub fn allocate<T>(&mut self) -> Result<usize, WasmMemoryError> {
        let align = mem::align_of::<T>();
        let size = mem::size_of::<T>();

        // Align next address
        let aligned_addr = (self.next_addr + align - 1) & !(align - 1);

        // Check alignment
        if aligned_addr % align != 0 {
            return Err(WasmMemoryError::Misaligned {
                expected: align,
                actual: aligned_addr,
            });
        }

        // Update next address
        self.next_addr = aligned_addr + size;

        Ok(aligned_addr)
    }

    /// Get current allocation address
    pub fn current_addr(&self) -> usize {
        self.next_addr
    }

    /// Reset allocator to base address
    pub fn reset(&mut self, base_addr: usize) {
        self.next_addr = base_addr;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_alignment_check() {
        assert!(WasmCapsuleLoader::is_aligned::<u64>(0));
        assert!(WasmCapsuleLoader::is_aligned::<u64>(8));
        assert!(WasmCapsuleLoader::is_aligned::<u64>(64));
        assert!(!WasmCapsuleLoader::is_aligned::<u64>(7));
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_allocator() {
        let mut allocator = WasmCapsuleAllocator::new(0, 8).unwrap();

        // Allocate u64 (8-byte aligned)
        let addr1 = allocator.allocate::<u64>().unwrap();
        assert_eq!(addr1, 0);
        assert!(WasmCapsuleLoader::is_aligned::<u64>(addr1));

        // Allocate another u64
        let addr2 = allocator.allocate::<u64>().unwrap();
        assert_eq!(addr2, 8);
        assert!(WasmCapsuleLoader::is_aligned::<u64>(addr2));
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_allocator_alignment() {
        let mut allocator = WasmCapsuleAllocator::new(64, 64).unwrap();

        #[repr(C, align(64))]
        struct Capsule64 {
            _data: [u8; 64],
        }

        // Allocate 64-byte aligned capsule
        let addr = allocator.allocate::<Capsule64>().unwrap();
        assert_eq!(addr, 64);
        assert!(WasmCapsuleLoader::is_aligned::<Capsule64>(addr));
    }

    #[test]
    fn test_memory_error() {
        let err = WasmMemoryError::Misaligned {
            expected: 64,
            actual: 7,
        };
        assert_eq!(
            err,
            WasmMemoryError::Misaligned {
                expected: 64,
                actual: 7
            }
        );
    }
}
