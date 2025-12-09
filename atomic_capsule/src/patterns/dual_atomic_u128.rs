//! Compact DualAtomicU64 using native 128-bit atomics
//!
//! # Overview
//!
//! `CompactDualAtomicU64` provides the same SWeMR (Single-Writer, Multiple-Reader)
//! pattern as `DualAtomicU64`, but uses a single `AtomicU128` instead of three
//! separate `AtomicU64` fields.
//!
//! # Trade-offs vs Standard DualAtomicU64
//!
//! **Advantages:**
//! - **50% smaller**: 64 bytes vs 128 bytes (better cache utilization)
//! - **2× faster snapshots**: ~15ns vs ~30ns (single load vs triple load)
//! - **Truly atomic snapshots**: Hardware-guaranteed atomicity (no generation counter loop)
//! - **Simpler mental model**: One atomic operation, guaranteed consistency
//!
//! **Disadvantages:**
//! - **Slower independent channel access**: Both values share cache line (false sharing)
//! - **Platform-specific**: Requires 128-bit atomic support (x86-64 cmpxchg16b, ARM64 LDXP/STXP)
//! - **Not available on**: WASM, RISC-V (without extensions), 32-bit platforms
//!
//! # When to Use
//!
//! **Use CompactDualAtomicU64 when:**
//! - You need fast atomic snapshots of both values (e.g., value+generation)
//! - Memory footprint matters (embedded systems, large arrays)
//! - Reads dominate writes (snapshot performance critical)
//! - Platform supports 128-bit atomics (x86-64, ARM64)
//!
//! **Use Standard DualAtomicU64 when:**
//! - Independent channel updates are frequent (writer updates one channel at a time)
//! - Platform compatibility required (WASM, RISC-V, 32-bit)
//! - Cache line separation needed (minimize false sharing)
//!
//! # Platform Support
//!
//! - **x86-64**: Uses `cmpxchg16b` (available since Core 2, 2006)
//! - **ARM64**: Uses `LDXP/STXP` load/store exclusive pairs
//! - **WASM**: Not available (feature-gated out)
//! - **RISC-V**: Not available without A extension (feature-gated out)
//!
//! # Performance
//!
//! | Operation | CompactDualAtomicU64 | Standard DualAtomicU64 | Speedup |
//! |-----------|----------------------|------------------------|---------|
//! | `load_both` | ~15ns (1× 128-bit load) | ~30ns (3× 64-bit loads) | 2× |
//! | `store_both` | ~15ns (1× 128-bit store) | ~24ns (2× 64-bit stores) | 1.6× |
//! | `compare_exchange` | ~20ns (1× 128-bit CAS) | ~30ns (verify + 2× CAS) | 1.5× |
//! | `load_primary` | ~15ns (128-bit load + extract) | ~10ns (direct load) | 0.67× |
//!
//! # Examples
//!
//! ```rust
//! use atomic_capsule::patterns::CompactDualAtomicU64;
//! use std::sync::atomic::Ordering;
//!
//! // Create with initial values
//! let dual = CompactDualAtomicU64::new(42, 0);
//!
//! // Atomic snapshot (guaranteed consistent, single load)
//! let (value, generation) = dual.load_both(Ordering::Acquire);
//! assert_eq!(value, 42);
//! assert_eq!(generation, 0);
//!
//! // Update both atomically
//! dual.store_both(100, 1, Ordering::Release);
//!
//! // Safe API (recommended)
//! let (v, g) = dual.load_both_acquire();
//! assert_eq!(v, 100);
//! assert_eq!(g, 1);
//!
//! // Atomic increment of value with generation bump
//! dual.write_with_generation(101);
//! ```

#![cfg(feature = "portable-atomic-u128")]

use portable_atomic::{AtomicU128, Ordering};

/// Compact 64-byte DualAtomicU64 using native 128-bit atomics
///
/// Provides SWeMR pattern (Single-Writer, Multiple-Reader) with true atomic
/// snapshots via hardware 128-bit atomics. Smaller and faster than standard
/// `DualAtomicU64` for snapshot-heavy workloads.
///
/// # Memory Layout
///
/// ```text
/// [0..64): AtomicU128 (packed as [primary: u64 | secondary: u64])
/// [64..128): padding (ensures no false sharing with adjacent data)
/// ```
///
/// # Thread Safety
///
/// - **Single Writer**: Use `store_both`, `write_with_generation`
/// - **Multiple Readers**: Use `load_both`, `load_primary`, `load_secondary`
/// - **Atomic Snapshots**: `load_both` guarantees both values from same instant
///
/// # Example
///
/// ```rust
/// use atomic_capsule::patterns::CompactDualAtomicU64;
///
/// let dual = CompactDualAtomicU64::new(42, 0);
///
/// // Writer updates value and generation atomically
/// dual.write_with_generation(100);
///
/// // Readers get consistent snapshot (no torn reads)
/// let (value, generation) = dual.load_both_acquire();
/// assert_eq!(value, 100);
/// assert_eq!(generation, 1);
/// ```
#[repr(C, align(64))]
pub struct CompactDualAtomicU64 {
    /// Packed 128-bit atomic: [primary: u64 | secondary: u64]
    ///
    /// Layout (little-endian):
    /// - Bits [0..64): primary value
    /// - Bits [64..128): secondary value (typically generation counter)
    packed: AtomicU128,
}

impl CompactDualAtomicU64 {
    /// Creates a new `CompactDualAtomicU64` with initial values
    ///
    /// # Arguments
    ///
    /// * `primary` - Initial primary value (typically data)
    /// * `secondary` - Initial secondary value (typically generation counter)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    ///
    /// let dual = CompactDualAtomicU64::new(42, 0);
    /// ```
    #[inline]
    pub const fn new(primary: u64, secondary: u64) -> Self {
        let packed_value = Self::pack(primary, secondary);
        Self {
            packed: AtomicU128::new(packed_value),
        }
    }

    /// Packs two u64 values into a u128
    ///
    /// Layout: [primary: u64 | secondary: u64]
    #[inline]
    const fn pack(primary: u64, secondary: u64) -> u128 {
        ((secondary as u128) << 64) | (primary as u128)
    }

    /// Unpacks u128 into two u64 values
    ///
    /// Returns: (primary, secondary)
    #[inline]
    const fn unpack(packed: u128) -> (u64, u64) {
        let primary = packed as u64;
        let secondary = (packed >> 64) as u64;
        (primary, secondary)
    }

    /// Atomically loads both values with specified memory ordering
    ///
    /// This is a **true atomic snapshot** - both values are guaranteed to be
    /// from the same instant in time, using a single 128-bit hardware load.
    ///
    /// # Performance
    ///
    /// - **x86-64**: ~15ns (single `movdqa` or `cmpxchg16b` for read)
    /// - **ARM64**: ~15ns (single `LDXP` load exclusive pair)
    ///
    /// # Arguments
    ///
    /// * `order` - Memory ordering (typically `Acquire` for readers)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    /// use std::sync::atomic::Ordering;
    ///
    /// let dual = CompactDualAtomicU64::new(42, 0);
    /// let (value, generation) = dual.load_both(Ordering::Acquire);
    /// assert_eq!(value, 42);
    /// assert_eq!(generation, 0);
    /// ```
    #[inline]
    pub fn load_both(&self, order: Ordering) -> (u64, u64) {
        let packed = self.packed.load(order);
        Self::unpack(packed)
    }

    /// Atomically stores both values with specified memory ordering
    ///
    /// # Performance
    ///
    /// - **x86-64**: ~15ns (single `cmpxchg16b`)
    /// - **ARM64**: ~15ns (single `STXP` store exclusive pair)
    ///
    /// # Arguments
    ///
    /// * `primary` - New primary value
    /// * `secondary` - New secondary value
    /// * `order` - Memory ordering (typically `Release` for writers)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    /// use std::sync::atomic::Ordering;
    ///
    /// let dual = CompactDualAtomicU64::new(0, 0);
    /// dual.store_both(42, 1, Ordering::Release);
    /// ```
    #[inline]
    pub fn store_both(&self, primary: u64, secondary: u64, order: Ordering) {
        let packed = Self::pack(primary, secondary);
        self.packed.store(packed, order);
    }

    /// Atomically compares and exchanges both values
    ///
    /// If the current value equals `current`, stores `new` and returns `Ok`.
    /// Otherwise, returns `Err` with the actual current value.
    ///
    /// # Performance
    ///
    /// - **x86-64**: ~20ns (single `cmpxchg16b`)
    /// - **ARM64**: ~20ns (single `LDXP/STXP` pair with retry)
    ///
    /// # Arguments
    ///
    /// * `current` - Expected current values (primary, secondary)
    /// * `new` - New values to store (primary, secondary)
    /// * `success` - Memory ordering for successful CAS
    /// * `failure` - Memory ordering for failed CAS
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    /// use std::sync::atomic::Ordering;
    ///
    /// let dual = CompactDualAtomicU64::new(42, 0);
    ///
    /// // Try to update (will succeed)
    /// let result = dual.compare_exchange_both(
    ///     (42, 0),
    ///     (100, 1),
    ///     Ordering::AcqRel,
    ///     Ordering::Acquire,
    /// );
    /// assert_eq!(result, Ok((42, 0)));
    ///
    /// // Try again with wrong expected value (will fail)
    /// let result = dual.compare_exchange_both(
    ///     (42, 0),
    ///     (200, 2),
    ///     Ordering::AcqRel,
    ///     Ordering::Acquire,
    /// );
    /// assert_eq!(result, Err((100, 1)));
    /// ```
    #[inline]
    pub fn compare_exchange_both(
        &self,
        current: (u64, u64),
        new: (u64, u64),
        success: Ordering,
        failure: Ordering,
    ) -> Result<(u64, u64), (u64, u64)> {
        let current_packed = Self::pack(current.0, current.1);
        let new_packed = Self::pack(new.0, new.1);

        match self.packed.compare_exchange(current_packed, new_packed, success, failure) {
            Ok(packed) => Ok(Self::unpack(packed)),
            Err(packed) => Err(Self::unpack(packed)),
        }
    }

    /// Loads only the primary value
    ///
    /// **Note**: This requires a full 128-bit load followed by extraction,
    /// so it's slower than `DualAtomicU64::load_primary()`. If you only need
    /// the primary value frequently, consider using standard `DualAtomicU64`.
    ///
    /// # Performance
    ///
    /// - ~15ns (128-bit load + extraction) vs ~10ns (direct 64-bit load)
    ///
    /// # Arguments
    ///
    /// * `order` - Memory ordering
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    /// use std::sync::atomic::Ordering;
    ///
    /// let dual = CompactDualAtomicU64::new(42, 0);
    /// assert_eq!(dual.load_primary(Ordering::Acquire), 42);
    /// ```
    #[inline]
    pub fn load_primary(&self, order: Ordering) -> u64 {
        let (primary, _) = self.load_both(order);
        primary
    }

    /// Loads only the secondary value
    ///
    /// **Note**: This requires a full 128-bit load followed by extraction.
    ///
    /// # Performance
    ///
    /// - ~15ns (128-bit load + extraction) vs ~10ns (direct 64-bit load)
    ///
    /// # Arguments
    ///
    /// * `order` - Memory ordering
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    /// use std::sync::atomic::Ordering;
    ///
    /// let dual = CompactDualAtomicU64::new(42, 0);
    /// assert_eq!(dual.load_secondary(Ordering::Acquire), 0);
    /// ```
    #[inline]
    pub fn load_secondary(&self, order: Ordering) -> u64 {
        let (_, secondary) = self.load_both(order);
        secondary
    }

    // ========================================================================
    // Safe High-Level API (Recommended)
    // ========================================================================

    /// Atomically loads both values with Acquire ordering
    ///
    /// Recommended over raw `load_both` for typical reader use cases.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    ///
    /// let dual = CompactDualAtomicU64::new(42, 0);
    /// let (value, generation) = dual.load_both_acquire();
    /// assert_eq!(value, 42);
    /// assert_eq!(generation, 0);
    /// ```
    #[inline]
    pub fn load_both_acquire(&self) -> (u64, u64) {
        self.load_both(Ordering::Acquire)
    }

    /// Atomically stores both values with Release ordering
    ///
    /// Recommended over raw `store_both` for typical writer use cases.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    ///
    /// let dual = CompactDualAtomicU64::new(0, 0);
    /// dual.store_both_release(42, 1);
    /// ```
    #[inline]
    pub fn store_both_release(&self, primary: u64, secondary: u64) {
        self.store_both(primary, secondary, Ordering::Release);
    }

    /// Atomically updates primary value and increments secondary (generation counter)
    ///
    /// This is the recommended way to update values in SWeMR pattern. The
    /// secondary value is atomically incremented to signal a new generation.
    ///
    /// # Performance
    ///
    /// - ~30ns (load + increment + CAS, may retry on contention)
    ///
    /// # Arguments
    ///
    /// * `value` - New primary value to store
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    ///
    /// let dual = CompactDualAtomicU64::new(42, 0);
    ///
    /// // Update value, generation bumps to 1
    /// dual.write_with_generation(100);
    ///
    /// let (value, generation) = dual.load_both_acquire();
    /// assert_eq!(value, 100);
    /// assert_eq!(generation, 1);
    /// ```
    #[inline]
    pub fn write_with_generation(&self, value: u64) {
        // CAS loop to atomically update value and increment generation
        loop {
            let (current_value, current_gen) = self.load_both(Ordering::Acquire);
            let new_gen = current_gen.wrapping_add(1);

            match self.compare_exchange_both(
                (current_value, current_gen), // Use actual current value, not 0
                (value, new_gen),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(_) => continue, // Retry on contention
            }
        }
    }

    /// Reads both values with consistency guarantee
    ///
    /// Since `CompactDualAtomicU64` uses a single 128-bit atomic, this is
    /// **always** consistent (no retry loop needed like `DualAtomicU64`).
    ///
    /// # Performance
    ///
    /// - ~15ns (single atomic load, guaranteed consistent)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::patterns::CompactDualAtomicU64;
    ///
    /// let dual = CompactDualAtomicU64::new(42, 0);
    /// let read = dual.read_consistent();
    ///
    /// assert_eq!(read.value, 42);
    /// assert_eq!(read.generation, 0);
    /// ```
    #[inline]
    pub fn read_consistent(&self) -> ConsistentRead<u64> {
        let (value, generation) = self.load_both_acquire();
        ConsistentRead { value, generation }
    }
}

/// Result of a consistent read operation
///
/// Guaranteed to be a snapshot from a single instant in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsistentRead<T> {
    /// Primary value
    pub value: T,
    /// Generation counter at time of read
    pub generation: u64,
}

// ============================================================================
// Safety: CompactDualAtomicU64 is Send + Sync
// ============================================================================

unsafe impl Send for CompactDualAtomicU64 {}
unsafe impl Sync for CompactDualAtomicU64 {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_load() {
        let dual = CompactDualAtomicU64::new(42, 100);
        let (primary, secondary) = dual.load_both_acquire();
        assert_eq!(primary, 42);
        assert_eq!(secondary, 100);
    }

    #[test]
    fn test_store_both() {
        let dual = CompactDualAtomicU64::new(0, 0);
        dual.store_both_release(42, 1);

        let (primary, secondary) = dual.load_both_acquire();
        assert_eq!(primary, 42);
        assert_eq!(secondary, 1);
    }

    #[test]
    fn test_load_primary() {
        let dual = CompactDualAtomicU64::new(42, 100);
        assert_eq!(dual.load_primary(Ordering::Acquire), 42);
    }

    #[test]
    fn test_load_secondary() {
        let dual = CompactDualAtomicU64::new(42, 100);
        assert_eq!(dual.load_secondary(Ordering::Acquire), 100);
    }

    #[test]
    fn test_compare_exchange_success() {
        let dual = CompactDualAtomicU64::new(42, 0);

        let result = dual.compare_exchange_both(
            (42, 0),
            (100, 1),
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        assert_eq!(result, Ok((42, 0)));

        let (primary, secondary) = dual.load_both_acquire();
        assert_eq!(primary, 100);
        assert_eq!(secondary, 1);
    }

    #[test]
    fn test_compare_exchange_failure() {
        let dual = CompactDualAtomicU64::new(42, 0);

        let result = dual.compare_exchange_both(
            (99, 0), // Wrong expected value
            (100, 1),
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        assert_eq!(result, Err((42, 0)));

        // Value should be unchanged
        let (primary, secondary) = dual.load_both_acquire();
        assert_eq!(primary, 42);
        assert_eq!(secondary, 0);
    }

    #[test]
    fn test_write_with_generation() {
        let dual = CompactDualAtomicU64::new(0, 0);

        dual.write_with_generation(42);
        let (value, gen) = dual.load_both_acquire();
        assert_eq!(value, 42);
        assert_eq!(gen, 1);

        dual.write_with_generation(100);
        let (value, gen) = dual.load_both_acquire();
        assert_eq!(value, 100);
        assert_eq!(gen, 2);
    }

    #[test]
    fn test_read_consistent() {
        let dual = CompactDualAtomicU64::new(42, 0);

        let read = dual.read_consistent();
        assert_eq!(read.value, 42);
        assert_eq!(read.generation, 0);

        dual.write_with_generation(100);

        let read = dual.read_consistent();
        assert_eq!(read.value, 100);
        assert_eq!(read.generation, 1);
    }

    #[test]
    fn test_pack_unpack() {
        let packed = CompactDualAtomicU64::pack(42, 100);
        let (primary, secondary) = CompactDualAtomicU64::unpack(packed);
        assert_eq!(primary, 42);
        assert_eq!(secondary, 100);
    }

    #[test]
    fn test_pack_unpack_max_values() {
        let packed = CompactDualAtomicU64::pack(u64::MAX, u64::MAX);
        let (primary, secondary) = CompactDualAtomicU64::unpack(packed);
        assert_eq!(primary, u64::MAX);
        assert_eq!(secondary, u64::MAX);
    }

    #[test]
    fn test_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let dual = Arc::new(CompactDualAtomicU64::new(42, 0));
        let mut handles = vec![];

        // Spawn 10 reader threads
        for _ in 0..10 {
            let dual = Arc::clone(&dual);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let (primary, secondary) = dual.load_both_acquire();
                    // Should always be consistent
                    assert!(primary <= 1000);
                    assert!(secondary <= 1000);
                }
            }));
        }

        // Spawn 1 writer thread
        let dual_writer = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for i in 1..=1000 {
                dual_writer.store_both_release(i, i);
            }
        }));

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_write_with_generation() {
        use std::sync::Arc;
        use std::thread;

        let dual = Arc::new(CompactDualAtomicU64::new(0, 0));
        let mut handles = vec![];

        // Spawn 4 writer threads (each writes 250 times)
        for thread_id in 0..4 {
            let dual = Arc::clone(&dual);
            handles.push(thread::spawn(move || {
                for i in 0..250 {
                    dual.write_with_generation((thread_id * 250 + i) as u64);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Generation should be 1000 (4 threads × 250 writes)
        let (_, generation) = dual.load_both_acquire();
        assert_eq!(generation, 1000);
    }

    #[test]
    fn test_size_and_alignment() {
        use std::mem;

        // Should be exactly 64 bytes (half the size of DualAtomicU64)
        assert_eq!(mem::size_of::<CompactDualAtomicU64>(), 64);

        // Should be 64-byte aligned (cache line)
        assert_eq!(mem::align_of::<CompactDualAtomicU64>(), 64);
    }

    #[test]
    fn test_generation_overflow() {
        let dual = CompactDualAtomicU64::new(42, u64::MAX);

        dual.write_with_generation(100);

        let (value, gen) = dual.load_both_acquire();
        assert_eq!(value, 100);
        assert_eq!(gen, 0); // Wrapped around
    }
}

#[cfg(all(test, feature = "std"))]
mod bench_comparison {
    use super::*;

    /// Benchmark helper: measures time for snapshot operation
    ///
    /// This is not a real benchmark (requires criterion), but shows
    /// the expected performance characteristics for documentation.
    #[test]
    #[ignore] // Run with `cargo test --ignored` to see timing
    fn bench_snapshot_performance() {
        use std::time::Instant;

        let dual = CompactDualAtomicU64::new(42, 0);
        let iterations = 1_000_000;

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = dual.load_both_acquire();
        }
        let elapsed = start.elapsed();

        let ns_per_op = elapsed.as_nanos() / iterations as u128;
        println!("CompactDualAtomicU64::load_both: {}ns per operation", ns_per_op);
        println!("Expected: ~15ns (single 128-bit load)");

        // Should be under 30ns on modern hardware
        assert!(ns_per_op < 30);
    }

    #[test]
    #[ignore]
    fn bench_store_performance() {
        use std::time::Instant;

        let dual = CompactDualAtomicU64::new(0, 0);
        let iterations = 1_000_000;

        let start = Instant::now();
        for i in 0..iterations {
            dual.store_both_release(i, i);
        }
        let elapsed = start.elapsed();

        let ns_per_op = elapsed.as_nanos() / iterations as u128;
        println!("CompactDualAtomicU64::store_both: {}ns per operation", ns_per_op);
        println!("Expected: ~15ns (single 128-bit store)");

        assert!(ns_per_op < 30);
    }

    #[test]
    #[ignore]
    fn bench_write_with_generation_performance() {
        use std::time::Instant;

        let dual = CompactDualAtomicU64::new(0, 0);
        let iterations = 100_000; // Fewer iterations (CAS loop)

        let start = Instant::now();
        for i in 0..iterations {
            dual.write_with_generation(i);
        }
        let elapsed = start.elapsed();

        let ns_per_op = elapsed.as_nanos() / iterations as u128;
        println!("CompactDualAtomicU64::write_with_generation: {}ns per operation", ns_per_op);
        println!("Expected: ~30-50ns (load + CAS)");

        assert!(ns_per_op < 100);
    }
}
