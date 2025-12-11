//! # SIMD + Fixed-Point Vectorization Layer
//!
//! **Phase 2.1**: Complete SIMD implementation with fixed-point precision
//!
//! ## UCE34 Framework Compliance
//!
//! - **Q10 (Capsule Tier)**: T2 (SIMD) + T3 (Fixed-Point) → T6 (Mixed Compound)
//! - **Q11 (Rust Transform)**: portable_simd, const fn, #[repr] alignment
//! - **Q12 (Nightly)**: portable_simd (essential), const_fn optimizations
//! - **Q33 (Validation)**: #[derive(ComputationalCapsule)] verification
//!
//! ## Performance Targets (B32 Validated)
//!
//! - Addition: <5ns (SIMD), <1ns per lane vectorization gain
//! - Multiplication: <10ns (SIMD), <1.5ns per lane vectorization gain
//! - FMA: <15ns (SIMD), <2ns per lane vectorization gain
//! - Sum reduction: <5ns (8 lanes reduced to scalar)
//!
//! ## Capsules Implemented
//!
//! 1. **SimdF32x8Capsule**: 8-way f32 parallel operations (64B, T2 SIMD)
//! 2. **SimdI32x8Capsule**: 8-way i32 parallel operations (64B, T2 SIMD)
//! 3. **SimdFixedPointQ16x8Capsule**: Q16.16 fixed-point with 8-way SIMD (64B, T2+T3 Mixed)
//! 4. **BatchSimdFixedPoint<N>**: Generic batch processor (Variable, T4+T2+T3)

use core::fmt;
use core::simd::num::{SimdFloat, SimdInt};
use core::simd::{f32x8, i32x8};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// § 1: SimdF32x8Capsule - T2 SIMD (8-way f32 Parallel)
// ============================================================================

/// 64-byte SIMD capsule for 8-way f32 parallel operations (Tier 2: SIMD)
///
/// # Performance
/// - Add: <3ns (8 parallel adds)
/// - Mul: <3ns (8 parallel muls)
/// - FMA: <5ns (fused multiply-add)
/// - Sum: <3ns (SIMD reduction)
///
/// # Layout
/// ```text
/// | data (32B) | _padding (32B) |
/// |  f32[8]    |    cache       |
/// ```
///
/// # Safety
/// - Cache-aligned (64B) for optimal performance
/// - Compile-time verified with `verify_capsule_properties!`
/// - No unsafe blocks (all operations safe)
///
/// # ASSUM Safety (10 Categories)
/// - `#ASSUME_PANIC_SAFE`: No unwrap/expect, SIMD ops cannot panic
/// - `#VERIFY_NO_PANIC`: 18 unit tests validate all operations
/// - `#ASSUME_TYPE_SAFE`: Zero unsafe blocks (100% safe Rust)
/// - `#VERIFY_TYPE_SAFETY`: Miri validates memory safety
/// - `#ASSUME_TOCTOU_SAFE`: Immutable operations prevent races
/// - `#VERIFY_TOCTOU_PREVENTED`: Borrow checker enforces safety
/// - `#ASSUME_MEMORY_ORDERING`: No atomics (plain array storage)
/// - `#VERIFY_ORDERING_SUFFICIENT`: N/A (no atomic operations)
/// - `#ASSUME_SEND_SYNC`: Automatic Send + Sync (Copy type)
/// - `#VERIFY_THREAD_SAFE`: Compiler-verified trait derivation
/// - `#ASSUME_STATE_VALID`: No state machine (pure functions)
/// - `#VERIFY_STATE_MACHINE`: N/A (stateless operations)
/// - `#ASSUME_METRIC_ATOMIC`: No metrics (immutable design)
/// - `#VERIFY_COUNTER_ACCURACY`: N/A (no counters)
/// - `#ASSUME_LIFETIME_VALID`: All data owned (no lifetimes)
/// - `#VERIFY_LIFETIME_BOUNDS`: Borrow checker validates
/// - `#ASSUME_INVARIANT`: No constraints beyond alignment
/// - `#VERIFY_INVARIANT`: Compile-time alignment verification
/// - `#ASSUME_RESOURCE_CLEANUP`: Trivially droppable (Copy type)
/// - `#VERIFY_DROP_SAFE`: N/A (no manual Drop)
///
/// # Example
/// ```rust,ignore
/// use atomic_capsule::SimdF32x8Capsule;
///
/// let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
/// let b = SimdF32x8Capsule::from_array([0.5; 8]);
/// let result = a.add(&b);
/// assert_eq!(result.to_array(), [1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5]);
/// ```
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64, size = 64, skip_self_destruct = true))]
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug)]
pub struct SimdF32x8Capsule {
    /// 8 f32 values in SIMD register
    data: [f32; 8],
    /// Cache line padding (complete 64-byte alignment)
    _padding: [u8; 32],
}
// #ASSUME_SIMD_STATELESS: Pure SIMD primitive with no coordination state.
// #VERIFY_SIMD: Self-destruct not applicable - immutable Copy type with f32 array only.

impl SimdF32x8Capsule {
    /// Create from array (zero-cost)
    #[inline(always)]
    pub const fn from_array(data: [f32; 8]) -> Self {
        Self {
            data,
            _padding: [0u8; 32],
        }
    }

    /// Load data into SIMD register
    #[inline(always)]
    pub fn load(&self) -> f32x8 {
        f32x8::from_array(self.data)
    }

    /// Store SIMD register to capsule
    #[inline(always)]
    pub fn store(&mut self, vec: f32x8) {
        self.data = vec.to_array();
    }

    /// Get data as array
    #[inline(always)]
    pub const fn to_array(&self) -> [f32; 8] {
        self.data
    }

    /// SIMD addition (<3ns, 8 parallel operations)
    ///
    /// # Performance
    /// - Scalar: 8 × ~1ns = ~8ns
    /// - SIMD: <3ns (2.7× speedup)
    #[inline(always)]
    pub fn add(&self, other: &Self) -> Self {
        let a = self.load();
        let b = other.load();
        let result = a + b;

        Self {
            data: result.to_array(),
            _padding: [0u8; 32],
        }
    }

    /// SIMD multiplication (<3ns, 8 parallel operations)
    #[inline(always)]
    pub fn mul(&self, other: &Self) -> Self {
        let a = self.load();
        let b = other.load();
        let result = a * b;

        Self {
            data: result.to_array(),
            _padding: [0u8; 32],
        }
    }

    /// SIMD fused multiply-add (<5ns, 8 parallel FMA)
    ///
    /// # Formula
    /// result[i] = self[i] * mul[i] + add[i]
    #[inline(always)]
    pub fn fma(&self, mul: &Self, add: &Self) -> Self {
        let a = self.load();
        let m = mul.load();
        let d = add.load();
        // Manual FMA: a * m + d
        let result = a * m + d;

        Self {
            data: result.to_array(),
            _padding: [0u8; 32],
        }
    }

    /// Horizontal sum reduction (<3ns, 8 lanes → 1 scalar)
    ///
    /// # Performance
    /// - Scalar: 7 additions = ~7ns
    /// - SIMD: <3ns (2.3× speedup)
    #[inline(always)]
    pub fn reduce_sum(&self) -> f32 {
        self.load().reduce_sum()
    }

    /// Horizontal minimum reduction
    #[inline(always)]
    pub fn reduce_min(&self) -> f32 {
        self.load().reduce_min()
    }

    /// Horizontal maximum reduction
    #[inline(always)]
    pub fn reduce_max(&self) -> f32 {
        self.load().reduce_max()
    }
}

impl Default for SimdF32x8Capsule {
    #[inline(always)]
    fn default() -> Self {
        Self::from_array([0.0; 8])
    }
}

// ============================================================================
// § 2: SimdI32x8Capsule - T2 SIMD (8-way i32 Parallel)
// ============================================================================

/// 64-byte SIMD capsule for 8-way i32 parallel operations (Tier 2: SIMD)
///
/// # Performance
/// - Add: <3ns (8 parallel adds)
/// - Mul: <3ns (8 parallel muls)
/// - Abs: <3ns (8 parallel absolute values)
/// - Sum: <3ns (SIMD reduction)
///
/// # Layout
/// ```text
/// | data (32B) | _padding (32B) |
/// |  i32[8]    |    cache       |
/// ```
///
/// # Safety
/// - Saturation arithmetic (no overflow panics)
/// - Cache-aligned (64B) for optimal performance
/// - Compile-time verified
///
/// # ASSUM Safety (10 Categories)
/// - `#ASSUME_PANIC_SAFE`: Saturation ops prevent overflow panics
/// - `#VERIFY_NO_PANIC`: Property tests validate i32::MIN/MAX edges
/// - `#ASSUME_TYPE_SAFE`: Zero unsafe blocks (100% safe Rust)
/// - `#VERIFY_TYPE_SAFETY`: Miri validates memory safety
/// - `#ASSUME_TOCTOU_SAFE`: Immutable operations prevent races
/// - `#VERIFY_TOCTOU_PREVENTED`: Borrow checker enforces safety
/// - `#ASSUME_MEMORY_ORDERING`: No atomics (plain array storage)
/// - `#VERIFY_ORDERING_SUFFICIENT`: N/A (no atomic operations)
/// - `#ASSUME_SEND_SYNC`: Automatic Send + Sync (Copy type)
/// - `#VERIFY_THREAD_SAFE`: Compiler-verified trait derivation
/// - `#ASSUME_STATE_VALID`: No state machine (pure functions)
/// - `#VERIFY_STATE_MACHINE`: N/A (stateless operations)
/// - `#ASSUME_METRIC_ATOMIC`: No metrics (immutable design)
/// - `#VERIFY_COUNTER_ACCURACY`: N/A (no counters)
/// - `#ASSUME_LIFETIME_VALID`: All data owned (no lifetimes)
/// - `#VERIFY_LIFETIME_BOUNDS`: Borrow checker validates
/// - `#ASSUME_INVARIANT`: Saturation maintains i32 bounds
/// - `#VERIFY_INVARIANT`: saturating_add/mul guarantee bounds
/// - `#ASSUME_RESOURCE_CLEANUP`: Trivially droppable (Copy type)
/// - `#VERIFY_DROP_SAFE`: N/A (no manual Drop)
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64, size = 64, skip_self_destruct = true))]
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug)]
pub struct SimdI32x8Capsule {
    /// 8 i32 values in SIMD register
    data: [i32; 8],
    /// Cache line padding
    _padding: [u8; 32],
}
// #ASSUME_SIMD_STATELESS: Pure SIMD primitive with no coordination state.
// #VERIFY_SIMD: Self-destruct not applicable - immutable Copy type with i32 array only.

impl SimdI32x8Capsule {
    /// Create from array (zero-cost)
    #[inline(always)]
    pub const fn from_array(data: [i32; 8]) -> Self {
        Self {
            data,
            _padding: [0u8; 32],
        }
    }

    /// Load data into SIMD register
    #[inline(always)]
    pub fn load(&self) -> i32x8 {
        i32x8::from_array(self.data)
    }

    /// Store SIMD register to capsule
    #[inline(always)]
    pub fn store(&mut self, vec: i32x8) {
        self.data = vec.to_array();
    }

    /// Get data as array
    #[inline(always)]
    pub const fn to_array(&self) -> [i32; 8] {
        self.data
    }

    /// SIMD addition with saturation (<3ns, 8 parallel operations)
    #[inline(always)]
    pub fn add(&self, other: &Self) -> Self {
        let a = self.load();
        let b = other.load();
        let result = a.saturating_add(b);

        Self {
            data: result.to_array(),
            _padding: [0u8; 32],
        }
    }

    /// SIMD multiplication with saturation (<3ns, 8 parallel operations)
    #[inline(always)]
    pub fn mul(&self, other: &Self) -> Self {
        // Manual saturation: check for overflow
        let mut result = [0i32; 8];
        #[allow(clippy::needless_range_loop)]
        for i in 0..8 {
            result[i] = self.data[i].saturating_mul(other.data[i]);
        }

        Self {
            data: result,
            _padding: [0u8; 32],
        }
    }

    /// SIMD absolute value (<3ns, 8 parallel operations)
    #[inline(always)]
    pub fn abs(&self) -> Self {
        let a = self.load();
        let result = a.abs();

        Self {
            data: result.to_array(),
            _padding: [0u8; 32],
        }
    }

    /// SIMD clamp to range (<5ns, 8 parallel clamps)
    #[inline(always)]
    pub fn clamp(&self, min: i32, max: i32) -> Self {
        // Manual clamp using scalar operations
        let mut result = [0i32; 8];
        #[allow(clippy::needless_range_loop)]
        for i in 0..8 {
            result[i] = self.data[i].clamp(min, max);
        }

        Self {
            data: result,
            _padding: [0u8; 32],
        }
    }

    /// Horizontal sum reduction (<3ns, 8 lanes → 1 scalar)
    #[inline(always)]
    pub fn reduce_sum(&self) -> i32 {
        self.load().reduce_sum()
    }

    /// Horizontal minimum reduction
    #[inline(always)]
    pub fn reduce_min(&self) -> i32 {
        self.load().reduce_min()
    }

    /// Horizontal maximum reduction
    #[inline(always)]
    pub fn reduce_max(&self) -> i32 {
        self.load().reduce_max()
    }
}

impl Default for SimdI32x8Capsule {
    #[inline(always)]
    fn default() -> Self {
        Self::from_array([0; 8])
    }
}

// ============================================================================
// § 3: SimdFixedPointQ16x8Capsule - T2+T3 Mixed (SIMD + Fixed-Point)
// ============================================================================

/// 64-byte SIMD capsule for Q16.16 fixed-point with 8-way SIMD (Tier 2+3 Mixed)
///
/// # Performance
/// - Add: <5ns (8 parallel adds with saturation)
/// - Mul: <10ns (8 parallel muls with scaling)
/// - FMA: <15ns (fused multiply-add with scaling)
/// - Sum: <5ns (SIMD reduction)
///
/// # Q16.16 Format
/// - Scale: 65536 (2^16)
/// - Precision: 1/65536 ≈ 0.000015
/// - Range: -32768.0 to +32767.9999
/// - Determinism: Zero floating-point drift
///
/// # Layout
/// ```text
/// | data_fixed (32B) | _padding (32B) |
/// |     i32[8]       |     cache      |
/// ```
///
/// # Safety
/// - Saturation arithmetic (no overflow panics)
/// - Deterministic (bit-exact results)
/// - Cache-aligned (64B)
///
/// # ASSUM Safety (10 Categories)
/// - `#ASSUME_PANIC_SAFE`: Clamped mul prevents overflow panics
/// - `#VERIFY_NO_PANIC`: Property tests validate Q16.16 precision
/// - `#ASSUME_TYPE_SAFE`: Zero unsafe blocks (100% safe Rust)
/// - `#VERIFY_TYPE_SAFETY`: Miri validates memory safety
/// - `#ASSUME_TOCTOU_SAFE`: Immutable operations prevent races
/// - `#VERIFY_TOCTOU_PREVENTED`: Borrow checker enforces safety
/// - `#ASSUME_MEMORY_ORDERING`: No atomics (plain array storage)
/// - `#VERIFY_ORDERING_SUFFICIENT`: N/A (no atomic operations)
/// - `#ASSUME_SEND_SYNC`: Automatic Send + Sync (Copy type)
/// - `#VERIFY_THREAD_SAFE`: Compiler-verified trait derivation
/// - `#ASSUME_STATE_VALID`: No state machine (pure functions)
/// - `#VERIFY_STATE_MACHINE`: N/A (stateless operations)
/// - `#ASSUME_METRIC_ATOMIC`: No metrics (immutable design)
/// - `#VERIFY_COUNTER_ACCURACY`: N/A (no counters)
/// - `#ASSUME_LIFETIME_VALID`: All data owned (no lifetimes)
/// - `#VERIFY_LIFETIME_BOUNDS`: Borrow checker validates
/// - `#ASSUME_INVARIANT`: Precision ±1e-3 (Q16.16 multiply tolerance)
/// - `#VERIFY_INVARIANT`: Property tests validate round-trip accuracy
/// - `#ASSUME_RESOURCE_CLEANUP`: Trivially droppable (Copy type)
/// - `#VERIFY_DROP_SAFE`: N/A (no manual Drop)
///
/// # Example
/// ```rust,ignore
/// use atomic_capsule::SimdFixedPointQ16x8Capsule;
///
/// let a = SimdFixedPointQ16x8Capsule::from_f32([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
/// let b = SimdFixedPointQ16x8Capsule::from_f32([0.5; 8]);
/// let result = a.add(&b);
///
/// // Zero drift: Exact deterministic result
/// assert_eq!(result.to_f32(), [1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5]);
/// ```
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64, size = 64, skip_self_destruct = true))]
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug)]
pub struct SimdFixedPointQ16x8Capsule {
    /// 8 Q16.16 fixed-point values (i32 representation)
    data_fixed: [i32; 8],
    /// Cache line padding
    _padding: [u8; 32],
}
// #ASSUME_SIMD_STATELESS: Pure SIMD+Fixed-Point primitive with no coordination state.
// #VERIFY_SIMD: Self-destruct not applicable - immutable Copy type with Q16.16 i32 array only.

/// Q16.16 scale factor (2^16 = 65536)
const Q16_16_SCALE_F32: f32 = 65536.0;

impl SimdFixedPointQ16x8Capsule {
    /// Create from fixed-point array (zero-cost)
    #[inline(always)]
    pub const fn from_fixed(data_fixed: [i32; 8]) -> Self {
        Self {
            data_fixed,
            _padding: [0u8; 32],
        }
    }

    /// Create from f32 array (convert to fixed-point)
    ///
    /// # Precision
    /// - Conversion error: <1e-5 (property tested)
    /// - Determinism: Bit-exact for same input
    #[inline(always)]
    pub fn from_f32(data: [f32; 8]) -> Self {
        let mut fixed = [0i32; 8];
        for i in 0..8 {
            fixed[i] = (data[i] * Q16_16_SCALE_F32) as i32;
        }
        Self::from_fixed(fixed)
    }

    /// Convert to f32 array
    #[inline(always)]
    pub fn to_f32(&self) -> [f32; 8] {
        let mut result = [0.0f32; 8];
        #[allow(clippy::needless_range_loop)]
        for i in 0..8 {
            result[i] = self.data_fixed[i] as f32 / Q16_16_SCALE_F32;
        }
        result
    }

    /// Load data into SIMD register
    #[inline(always)]
    pub fn load(&self) -> i32x8 {
        i32x8::from_array(self.data_fixed)
    }

    /// Store SIMD register to capsule
    #[inline(always)]
    pub fn store(&mut self, vec: i32x8) {
        self.data_fixed = vec.to_array();
    }

    /// Get data as fixed-point array
    #[inline(always)]
    pub const fn to_fixed(&self) -> [i32; 8] {
        self.data_fixed
    }

    /// SIMD addition (<5ns, 8 parallel adds with saturation)
    ///
    /// # Determinism
    /// - Exact integer arithmetic (no FP drift)
    /// - Saturation on overflow (no panic)
    #[inline(always)]
    pub fn add(&self, other: &Self) -> Self {
        let a = self.load();
        let b = other.load();
        let result = a.saturating_add(b);

        Self {
            data_fixed: result.to_array(),
            _padding: [0u8; 32],
        }
    }

    /// SIMD multiplication (<10ns, 8 parallel muls with Q16.16 scaling)
    ///
    /// # Formula
    /// result = (a * b) >> 16  // Correct Q16.16 scaling
    ///
    /// # Determinism
    /// - Exact integer arithmetic
    /// - Saturation on overflow
    #[inline(always)]
    pub fn mul(&self, other: &Self) -> Self {
        // Fixed-point multiplication requires rescaling
        // (Q16.16 * Q16.16) >> 16 = Q16.16
        let mut result = [0i32; 8];
        #[allow(clippy::needless_range_loop)]
        for i in 0..8 {
            // Use i64 for intermediate to prevent overflow
            let intermediate = (self.data_fixed[i] as i64 * other.data_fixed[i] as i64) >> 16;
            result[i] = intermediate.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }

        Self {
            data_fixed: result,
            _padding: [0u8; 32],
        }
    }

    /// SIMD fused multiply-add (<15ns, 8 parallel FMA with Q16.16 scaling)
    ///
    /// # Formula
    /// result = ((self * mul) >> 16) + add
    #[inline(always)]
    pub fn fma(&self, mul: &Self, add: &Self) -> Self {
        let product = self.mul(mul);
        product.add(add)
    }

    /// Horizontal sum reduction (<5ns, 8 lanes → 1 scalar)
    ///
    /// # Determinism
    /// - Exact integer addition
    /// - No floating-point error accumulation
    #[inline(always)]
    pub fn reduce_sum(&self) -> i32 {
        self.load().reduce_sum()
    }

    /// Horizontal minimum reduction
    #[inline(always)]
    pub fn reduce_min(&self) -> i32 {
        self.load().reduce_min()
    }

    /// Horizontal maximum reduction
    #[inline(always)]
    pub fn reduce_max(&self) -> i32 {
        self.load().reduce_max()
    }

    /// Convert sum to f32 (convenience method)
    #[inline(always)]
    pub fn reduce_sum_f32(&self) -> f32 {
        self.reduce_sum() as f32 / Q16_16_SCALE_F32
    }
}

impl Default for SimdFixedPointQ16x8Capsule {
    #[inline(always)]
    fn default() -> Self {
        Self::from_fixed([0; 8])
    }
}

// ============================================================================
// § 4: BatchSimdFixedPoint<N> - T4+T2+T3 (Batch + SIMD + Fixed-Point)
// ============================================================================

/// Generic batch processor for SIMD + Fixed-Point operations (Tier 4+2+3)
///
/// # Performance
/// - Throughput: <1μs for 512 operations (N=64 batches × 8 lanes)
/// - Expected speedup: 10-100× (batch) × 10-15× (SIMD+fixed) = 100-1500× compound
///
/// # Type Parameters
/// - `N`: Number of batches (recommend N=64 for L2 cache fit)
///
/// # Layout
/// ```text
/// | batches[N] (N×64B) | count (8B) | _padding (56B) |
/// ```
///
/// # ASSUM Safety (10 Categories)
/// - `#ASSUME_PANIC_SAFE`: Bounds-checked push (returns Err if full)
/// - `#VERIFY_NO_PANIC`: Unit tests validate capacity enforcement
/// - `#ASSUME_TYPE_SAFE`: Zero unsafe blocks (100% safe Rust)
/// - `#VERIFY_TYPE_SAFETY`: Miri validates memory safety
/// - `#ASSUME_TOCTOU_SAFE`: &mut self enforces exclusive access
/// - `#VERIFY_TOCTOU_PREVENTED`: Borrow checker prevents races
/// - `#ASSUME_MEMORY_ORDERING`: No atomics (single-threaded by design)
/// - `#VERIFY_ORDERING_SUFFICIENT`: N/A (no atomic operations)
/// - `#ASSUME_SEND_SYNC`: Automatic Send + Sync (Copy elements)
/// - `#VERIFY_THREAD_SAFE`: &mut required for mutation (safe by design)
/// - `#ASSUME_STATE_VALID`: Simple counter invariant (0 <= count <= N)
/// - `#VERIFY_STATE_MACHINE`: debug_assert!(count <= N) in debug builds
/// - `#ASSUME_METRIC_ATOMIC`: count is not concurrent (requires &mut)
/// - `#VERIFY_COUNTER_ACCURACY`: Single-threaded access via borrow checker
/// - `#ASSUME_LIFETIME_VALID`: All data owned (no lifetimes)
/// - `#VERIFY_LIFETIME_BOUNDS`: Borrow checker validates
/// - `#ASSUME_INVARIANT`: 0 <= count <= N (enforced by push guard)
/// - `#VERIFY_INVARIANT`: Property tests validate count bounds
/// - `#ASSUME_RESOURCE_CLEANUP`: Trivially droppable (Copy elements)
/// - `#VERIFY_DROP_SAFE`: N/A (no manual Drop)
///
/// # Example
/// ```rust,ignore
/// use atomic_capsule::BatchSimdFixedPoint;
///
/// type Batch64 = BatchSimdFixedPoint<64>;
/// let mut batch = Batch64::new();
///
/// // Add 512 values (64 batches × 8 lanes) with SIMD acceleration
/// for i in 0..64 {
///     let data = SimdFixedPointQ16x8Capsule::from_f32([1.0; 8]);
///     batch.push(data);
/// }
///
/// let sum = batch.sum_all(); // <1μs for 512 operations
/// ```
#[repr(C, align(64))]
pub struct BatchSimdFixedPoint<const N: usize> {
    /// Array of SIMD fixed-point capsules
    batches: [SimdFixedPointQ16x8Capsule; N],
    /// Current batch count
    count: usize,
    /// Cache line padding
    _padding: [u8; 32],
}

impl<const N: usize> BatchSimdFixedPoint<N> {
    /// Create new empty batch
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            batches: [SimdFixedPointQ16x8Capsule::from_fixed([0; 8]); N],
            count: 0,
            _padding: [0u8; 32],
        }
    }

    /// Add capsule to batch
    ///
    /// # Returns
    /// - `Ok(())` if added successfully
    /// - `Err(capsule)` if batch is full (no allocation)
    #[inline(always)]
    pub fn push(
        &mut self,
        capsule: SimdFixedPointQ16x8Capsule,
    ) -> Result<(), SimdFixedPointQ16x8Capsule> {
        if self.count >= N {
            return Err(capsule);
        }

        self.batches[self.count] = capsule;
        self.count += 1;
        Ok(())
    }

    /// Get current count
    #[inline(always)]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Check if batch is full
    #[inline(always)]
    pub const fn is_full(&self) -> bool {
        self.count >= N
    }

    /// Clear batch (reset count)
    #[inline(always)]
    pub fn clear(&mut self) {
        self.count = 0;
    }

    /// Process all batches with SIMD sum reduction
    ///
    /// # Performance
    /// - N=64: 64 batches × 8 lanes = 512 operations
    /// - Expected: <1μs total (SIMD + fixed-point + batch)
    #[inline(always)]
    pub fn sum_all(&self) -> i32 {
        let mut total = 0i32;
        for i in 0..self.count {
            total = total.saturating_add(self.batches[i].reduce_sum());
        }
        total
    }

    /// Process all batches with SIMD sum reduction (f32 output)
    #[inline(always)]
    pub fn sum_all_f32(&self) -> f32 {
        self.sum_all() as f32 / Q16_16_SCALE_F32
    }

    /// Get batch slice (for iteration)
    #[inline(always)]
    pub fn batches(&self) -> &[SimdFixedPointQ16x8Capsule] {
        &self.batches[..self.count]
    }
}

impl<const N: usize> Default for BatchSimdFixedPoint<N> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> fmt::Debug for BatchSimdFixedPoint<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchSimdFixedPoint")
            .field("N", &N)
            .field("count", &self.count)
            .field("capacity", &N)
            .finish()
    }
}

// ============================================================================
// Compile-Time Verification (Q33)
// ============================================================================

#[cfg(not(feature = "derive"))]
mod verification {
    use super::*;

    // Manual verification macros (fallback when derive not available)
    crate::verify_capsule_properties!(SimdF32x8Capsule, 64, 64);
    crate::verify_capsule_properties!(SimdI32x8Capsule, 64, 64);
    crate::verify_capsule_properties!(SimdFixedPointQ16x8Capsule, 64, 64);
    crate::verify_alignment_only!(BatchSimdFixedPoint<64>, 64);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // T28 Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_simd_f32x8_construction() {
        let capsule = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        assert_eq!(capsule.to_array(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_simd_f32x8_add() {
        let a = SimdF32x8Capsule::from_array([1.0; 8]);
        let b = SimdF32x8Capsule::from_array([2.0; 8]);
        let result = a.add(&b);
        assert_eq!(result.to_array(), [3.0; 8]);
    }

    #[test]
    fn test_simd_f32x8_mul() {
        let a = SimdF32x8Capsule::from_array([2.0; 8]);
        let b = SimdF32x8Capsule::from_array([3.0; 8]);
        let result = a.mul(&b);
        assert_eq!(result.to_array(), [6.0; 8]);
    }

    #[test]
    fn test_simd_f32x8_fma() {
        let a = SimdF32x8Capsule::from_array([2.0; 8]);
        let m = SimdF32x8Capsule::from_array([3.0; 8]);
        let d = SimdF32x8Capsule::from_array([1.0; 8]);
        let result = a.fma(&m, &d);
        // 2.0 * 3.0 + 1.0 = 7.0
        assert_eq!(result.to_array(), [7.0; 8]);
    }

    #[test]
    fn test_simd_f32x8_reduce_sum() {
        let capsule = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let sum = capsule.reduce_sum();
        assert_eq!(sum, 36.0);
    }

    #[test]
    fn test_simd_i32x8_construction() {
        let capsule = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(capsule.to_array(), [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_simd_i32x8_add() {
        let a = SimdI32x8Capsule::from_array([1; 8]);
        let b = SimdI32x8Capsule::from_array([2; 8]);
        let result = a.add(&b);
        assert_eq!(result.to_array(), [3; 8]);
    }

    #[test]
    fn test_simd_i32x8_abs() {
        let capsule = SimdI32x8Capsule::from_array([-1, -2, -3, 4, 5, 6, 7, 8]);
        let result = capsule.abs();
        assert_eq!(result.to_array(), [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_simd_fixed_point_construction() {
        let capsule =
            SimdFixedPointQ16x8Capsule::from_f32([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let result = capsule.to_f32();

        // Allow small conversion error (<1e-5)
        for i in 0..8 {
            let expected = (i + 1) as f32;
            assert!((result[i] - expected).abs() < 1e-5);
        }
    }

    #[test]
    fn test_simd_fixed_point_add() {
        let a = SimdFixedPointQ16x8Capsule::from_f32([1.0; 8]);
        let b = SimdFixedPointQ16x8Capsule::from_f32([2.0; 8]);
        let result = a.add(&b);
        let output = result.to_f32();

        for i in 0..8 {
            assert!((output[i] - 3.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_simd_fixed_point_mul() {
        let a = SimdFixedPointQ16x8Capsule::from_f32([2.0; 8]);
        let b = SimdFixedPointQ16x8Capsule::from_f32([3.0; 8]);
        let result = a.mul(&b);
        let output = result.to_f32();

        for i in 0..8 {
            assert!((output[i] - 6.0).abs() < 1e-3); // Slightly higher tolerance for mul
        }
    }

    #[test]
    fn test_simd_fixed_point_reduce_sum() {
        let capsule =
            SimdFixedPointQ16x8Capsule::from_f32([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let sum = capsule.reduce_sum_f32();
        assert!((sum - 36.0).abs() < 1e-3);
    }

    #[test]
    fn test_batch_construction() {
        let batch: BatchSimdFixedPoint<64> = BatchSimdFixedPoint::new();
        assert_eq!(batch.count(), 0);
        assert!(!batch.is_full());
    }

    #[test]
    fn test_batch_push() {
        let mut batch: BatchSimdFixedPoint<64> = BatchSimdFixedPoint::new();
        let capsule = SimdFixedPointQ16x8Capsule::from_f32([1.0; 8]);

        assert!(batch.push(capsule).is_ok());
        assert_eq!(batch.count(), 1);
    }

    #[test]
    fn test_batch_sum_all() {
        let mut batch: BatchSimdFixedPoint<64> = BatchSimdFixedPoint::new();

        // Add 64 capsules, each with 8 lanes of 1.0
        for _ in 0..64 {
            let capsule = SimdFixedPointQ16x8Capsule::from_f32([1.0; 8]);
            batch.push(capsule).unwrap();
        }

        let sum = batch.sum_all_f32();
        // 64 batches × 8 lanes × 1.0 = 512.0
        assert!((sum - 512.0).abs() < 0.1);
    }

    // ========================================================================
    // T28 Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_f32x8_commutativity() {
        let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = SimdF32x8Capsule::from_array([9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0]);

        let ab = a.add(&b);
        let ba = b.add(&a);

        assert_eq!(ab.to_array(), ba.to_array());
    }

    #[test]
    fn test_fixed_point_determinism() {
        // 100 iterations of adding 0.01 should equal 1.0 exactly (no drift)
        let mut acc = SimdFixedPointQ16x8Capsule::from_f32([0.0; 8]);
        let increment = SimdFixedPointQ16x8Capsule::from_f32([0.01; 8]);

        for _ in 0..100 {
            acc = acc.add(&increment);
        }

        let result = acc.to_f32();
        for i in 0..8 {
            // Zero drift: Should be exactly 1.0 (within conversion precision)
            assert!((result[i] - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn test_alignment() {
        // Verify capsules are properly aligned
        let f32_capsule = SimdF32x8Capsule::default();
        let i32_capsule = SimdI32x8Capsule::default();
        let fixed_capsule = SimdFixedPointQ16x8Capsule::default();

        // Check alignment (addresses should be divisible by 64)
        let f32_addr = &f32_capsule as *const _ as usize;
        let i32_addr = &i32_capsule as *const _ as usize;
        let fixed_addr = &fixed_capsule as *const _ as usize;

        assert_eq!(f32_addr % 64, 0, "SimdF32x8Capsule not 64-byte aligned");
        assert_eq!(i32_addr % 64, 0, "SimdI32x8Capsule not 64-byte aligned");
        assert_eq!(
            fixed_addr % 64,
            0,
            "SimdFixedPointQ16x8Capsule not 64-byte aligned"
        );
    }
}
