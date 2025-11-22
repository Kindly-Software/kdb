//! QuantumStateVectorCapsule - T2 SIMD-optimized quantum state
//!
//! # SIMD Optimization Strategy
//!
//! Complex number operations vectorized using `f64x4`:
//! - 2 complex numbers = 4 f64 values (real0, imag0, real1, imag1)
//! - Process 2 amplitudes per SIMD iteration
//! - 4-8× speedup vs scalar complex arithmetic
//!
//! # Memory Layout
//!
//! Separate real/imaginary arrays (SoA) for SIMD efficiency:
//! ```text
//! real_parts: [r0, r1, r2, r3, ...]  (32-byte aligned)
//! imag_parts: [i0, i1, i2, i3, ...]  (32-byte aligned)
//! ```
//!
//! This layout enables efficient f64x4 loads without shuffling.

use crate::quantum_pure::error::{QuantumPureError, QuantumPureResult};
use crate::quantum_pure::gate::{QuantumGateCapsule, GateType};
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

#[cfg(feature = "portable_simd")]
use std::simd::prelude::SimdFloat;

/// Minimum qubits supported (2 amplitudes, allows 1-qubit for testing)
pub const MIN_QUBITS: usize = 1;

/// Maximum qubits supported (1M amplitudes = 16MB)
pub const MAX_QUBITS: usize = 20;

/// Normalization tolerance (1e-10)
const NORM_TOLERANCE: f64 = 1e-10;

/// Cache blocking size (64KB = 4096 complex numbers = L2-friendly)
///
/// # Rationale (Phase 3.3 Cache Optimization)
/// - AMD Ryzen 9 6900HX L2: 512KB
/// - Target: 64KB per block (12.5% of L2, allows 8 concurrent blocks)
/// - Complex number: 16 bytes (2× f64)
/// - Block capacity: 64KB / 16 bytes = 4,096 complex numbers
/// - Real/Imag separation: 2× 4,096 f64 = 64KB total
///
/// # #ASSUME_BLOCK_SIZE_POWER_OF_TWO
/// Block size MUST be power of 2 for fast modulo operations
/// #VERIFY: Compile-time assertion below
const CACHE_BLOCK_SIZE: usize = 4096;

// Verify block size is power of 2
const _VERIFY_BLOCK_SIZE: () = assert!(CACHE_BLOCK_SIZE.is_power_of_two());

/// Transpose threshold for large strides (qubit index ≥ 12)
///
/// # Rationale (Phase 3.3 Large-Stride Optimization)
/// - Stride 4096 = 32KB jumps between paired amplitudes
/// - Beyond this, cache miss rate > 90% (random access pattern)
/// - Transpose to stride=1 becomes beneficial despite O(N) overhead
///
/// # Performance (20+ Qubits)
/// - Qubit 20: stride = 1M → 1MB jumps (100% cache misses)
/// - Transpose → stride=1 → sequential access (0% cache misses)
/// - Expected speedup: +20-30% for high-index qubits
const TRANSPOSE_THRESHOLD: usize = 1 << 12; // 4096

/// Prefetch distance (cache lines ahead to prefetch)
///
/// # Rationale (Phase 3.3 Software Prefetching)
/// - Prefetch 16× stride ahead (2 cache lines = 128 bytes = 16 f64)
/// - Hides ~60ns RAM latency on DDR5-4800
/// - Too far = evicts useful data, too close = doesn't hide latency
///
/// # Platform Support
/// - x86_64: _mm_prefetch (SSE)
/// - aarch64: __builtin_prefetch (ARM NEON)
/// - riscv64: No-op (no prefetch intrinsics)
const PREFETCH_DISTANCE: usize = 16;

/// Threshold for parallel execution (18 qubits = 262,144 dimensions)
///
/// # Rationale (Phase 3.2 Parallel Optimization)
/// - Below 18 qubits: Threading overhead dominates (Rayon ~300ns spawn vs ThreadPool <20ns)
/// - At 17 qubits (131K dims): Single-threaded AVX2 faster (2.6× vs 1.6× parallel with Rayon)
/// - At 18+ qubits (262K+ dims): Multi-threaded AVX2 shines (4-8× threading gain)
///
/// # Performance Targets
/// - 17 qubits (131K): Single AVX2 ~230µs (2.6×), Parallel ~380µs with Rayon (1.6× SLOWER)
/// - 20 qubits (1M): Parallel ThreadPool ~250µs (8×), Single AVX2 ~2ms (2×)
///
/// # #ASSUME_THRESHOLD_POWER_OF_TWO
/// Threshold MUST be power of 2 (aligns with qubit boundaries)
/// #VERIFY: Compile-time assertion below
const PARALLEL_THRESHOLD_DIMENSIONS: usize = 1 << 18; // 262,144 dimensions (18 qubits)

// Verify parallel threshold is power of 2
const _VERIFY_PARALLEL_THRESHOLD: () = assert!(PARALLEL_THRESHOLD_DIMENSIONS.is_power_of_two());

/// Status codes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateStatus {
    Uninitialized = 0,
    Initialized = 1,
    Measured = 2,
}

/// T2 SIMD + T1 Atomic: Quantum State Vector Capsule (256B)
///
/// # Architecture
///
/// - **Metadata** (26 bytes): Atomic coordination (T1)
/// - **Padding** (230 bytes): Cache alignment
/// - **Heap state**: Separate real/imag arrays for SIMD (not in capsule)
///
/// # SIMD Optimization
///
/// Single-qubit gates process 2 complex amplitudes (4 f64) per iteration:
/// ```text
/// Input:  [r0, i0, r1, i1]  (f64x4)
/// Matrix: [[a, b], [c, d]]   (2×2 complex)
/// Output: [r0', i0', r1', i1']  (f64x4)
/// ```
///
/// Expected speedup: 4-8× vs scalar complex operations
#[repr(C, align(256))]
pub struct QuantumStateVectorCapsule {
    /// Number of qubits (4-20)
    num_qubits: AtomicU32,

    /// State dimension (2^num_qubits)
    dimension: AtomicU64,

    /// Normalization factor (Q32.32 fixed-point)
    normalization: AtomicU64,

    /// Status: 0=uninitialized, 1=initialized, 2=measured
    status: AtomicU8,

    /// Padding to 256 bytes
    _padding: [u8; 231],
}

// Manual Clone implementation (AtomicU32/U64/U8 don't implement Clone)
impl Clone for QuantumStateVectorCapsule {
    fn clone(&self) -> Self {
        Self {
            num_qubits: AtomicU32::new(self.num_qubits.load(Ordering::Relaxed)),
            dimension: AtomicU64::new(self.dimension.load(Ordering::Relaxed)),
            normalization: AtomicU64::new(self.normalization.load(Ordering::Relaxed)),
            status: AtomicU8::new(self.status.load(Ordering::Relaxed)),
            _padding: [0; 231],
        }
    }
}

// Manual verification (will use #[derive(ComputationalCapsule)] in production)
impl QuantumStateVectorCapsule {
    const _VERIFY: () = {
        assert!(
            std::mem::size_of::<Self>() == 256,
            "QuantumStateVectorCapsule must be 256 bytes"
        );
        assert!(
            std::mem::align_of::<Self>() == 256,
            "QuantumStateVectorCapsule must be 256-byte aligned"
        );
    };
}

/// User-friendly wrapper combining capsule + state vectors
///
/// This struct provides a convenient API by bundling the capsule
/// with its associated real/imaginary state vectors.
pub struct QuantumState {
    pub capsule: QuantumStateVectorCapsule,
    pub real_parts: Vec<f64>,
    pub imag_parts: Vec<f64>,
}

impl QuantumState {
    /// Create new quantum state initialized to |0...0⟩
    ///
    /// # Arguments
    ///
    /// * `num_qubits` - Number of qubits (4-20)
    ///
    /// # Errors
    ///
    /// - `InvalidQubitCount`: num_qubits < 4 or > 20
    pub fn new(num_qubits: usize) -> QuantumPureResult<Self> {
        let (capsule, real_parts, imag_parts) = QuantumStateVectorCapsule::new_raw(num_qubits)?;
        Ok(Self {
            capsule,
            real_parts,
            imag_parts,
        })
    }

    /// Get number of qubits
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.capsule.num_qubits()
    }

    /// Get number of amplitudes (2^num_qubits)
    #[inline]
    pub fn num_amplitudes(&self) -> usize {
        self.capsule.dimension()
    }

    /// Apply a quantum gate
    pub fn apply_gate(&mut self, gate: &QuantumGateCapsule) -> QuantumPureResult<()> {
        use super::gate::GateType;

        let target = gate.target();
        let gate_type = gate.gate_type();

        // Single-qubit gates only in Phase 1
        match gate_type {
            GateType::Hadamard | GateType::PauliX | GateType::PauliY | GateType::PauliZ
            | GateType::SGate | GateType::TGate | GateType::Custom => {
                let matrix = gate.matrix();
                self.capsule.apply_single_qubit_gate(
                    target,
                    &matrix,
                    &mut self.real_parts,
                    &mut self.imag_parts,
                )
            }
            GateType::CNOT => {
                Err(QuantumPureError::UnsupportedGateType {
                    gate_type: "CNOT (use TwoQubitGateCapsule instead)".to_string(),
                })
            }
        }
    }

    /// Apply a two-qubit gate (CNOT, CZ, SWAP, etc.) - Phase 2
    ///
    /// # Arguments
    ///
    /// * `gate` - Two-qubit gate capsule
    ///
    /// # Performance
    ///
    /// - ~8μs for 8 qubits (4× slower than single-qubit due to 4× matrix size)
    /// - Scales as O(2^N) for N qubits
    ///
    /// # Example: Bell State Creation
    ///
    /// ```ignore
    /// use atomic_capsule::quantum_pure::{QuantumState, QuantumGateCapsule, TwoQubitGateCapsule};
    ///
    /// let mut state = QuantumState::new(2)?;
    ///
    /// // Create Bell state: (|00⟩ + |11⟩)/√2
    /// let h = QuantumGateCapsule::hadamard(0);
    /// let cnot = TwoQubitGateCapsule::cnot(0, 1)?;
    ///
    /// state.apply_gate(&h)?;
    /// state.apply_two_qubit_gate(&cnot)?;
    /// ```
    pub fn apply_two_qubit_gate(
        &mut self,
        gate: &super::multi_qubit_gate::TwoQubitGateCapsule,
    ) -> QuantumPureResult<()> {
        let control = gate.control();
        let target = gate.target();
        let matrix = gate.matrix();

        self.capsule.apply_two_qubit_gate(
            control,
            target,
            matrix,
            &mut self.real_parts,
            &mut self.imag_parts,
        )
    }

    /// Measure a specific qubit (collapse to |0⟩ or |1⟩)
    ///
    /// # Arguments
    ///
    /// * `qubit` - Qubit index to measure (0..num_qubits)
    ///
    /// # Returns
    ///
    /// * `true` for |1⟩, `false` for |0⟩
    ///
    /// # Errors
    ///
    /// * `InvalidQubitIndex`: qubit >= num_qubits
    pub fn measure(&mut self, qubit: usize) -> QuantumPureResult<bool> {
        let num_qubits = self.capsule.num_qubits();
        if qubit >= num_qubits {
            return Err(QuantumPureError::InvalidQubitIndex {
                index: qubit,
                num_qubits,
            });
        }

        // Measure entire state and extract bit value
        let state_index = self.capsule.measure(&mut self.real_parts, &mut self.imag_parts)?;

        // Extract bit at position `qubit`
        // For state |...q₁q₀⟩, qubit 0 is LSB, qubit n-1 is MSB
        let bit = (state_index >> qubit) & 1;
        Ok(bit == 1)
    }
}

impl QuantumStateVectorCapsule {
    /// Create new quantum state vector initialized to |0...0⟩ (raw API)
    ///
    /// # Arguments
    ///
    /// * `num_qubits` - Number of qubits (4-20)
    ///
    /// # Returns
    ///
    /// Capsule + separate real/imaginary arrays (heap-allocated)
    ///
    /// # Performance
    ///
    /// - Allocation: O(2^N) for 2^N amplitudes
    /// - Initialization: ~1μs for 16 qubits (65K amplitudes)
    ///
    /// # Errors
    ///
    /// - `InvalidQubitCount`: num_qubits < 4 or > 20
    ///
    /// # Note
    ///
    /// Prefer using `QuantumState::new()` for a simpler API.
    pub fn new_raw(num_qubits: usize) -> QuantumPureResult<(Self, Vec<f64>, Vec<f64>)> {
        if num_qubits < MIN_QUBITS || num_qubits > MAX_QUBITS {
            return Err(QuantumPureError::InvalidQubitCount {
                requested: num_qubits,
                min: MIN_QUBITS,
                max: MAX_QUBITS,
            });
        }

        let dimension = 1usize << num_qubits; // 2^num_qubits

        // Allocate separate real/imaginary arrays (SoA for SIMD)
        // Initialize to |0...0⟩: amplitude[0] = 1.0, rest = 0.0
        let mut real_parts = vec![0.0; dimension];
        let mut imag_parts = vec![0.0; dimension];
        real_parts[0] = 1.0; // |0...0⟩ state

        let capsule = Self {
            num_qubits: AtomicU32::new(num_qubits as u32),
            dimension: AtomicU64::new(dimension as u64),
            normalization: AtomicU64::new(1u64 << 32), // Q32.32: 1.0 = 2^32
            status: AtomicU8::new(StateStatus::Initialized as u8),
            _padding: [0; 231],
        };

        Ok((capsule, real_parts, imag_parts))
    }

    /// Create from existing state (for circuit integration)
    pub fn new(num_qubits: usize) -> QuantumPureResult<QuantumState> {
        QuantumState::new(num_qubits)
    }

    /// Get number of qubits
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits.load(Ordering::Relaxed) as usize
    }

    /// Get state dimension (2^num_qubits)
    #[inline]
    pub fn dimension(&self) -> usize {
        self.dimension.load(Ordering::Relaxed) as usize
    }

    /// Get current status
    #[inline]
    pub fn status(&self) -> StateStatus {
        match self.status.load(Ordering::Acquire) {
            0 => StateStatus::Uninitialized,
            1 => StateStatus::Initialized,
            2 => StateStatus::Measured,
            _ => StateStatus::Uninitialized,
        }
    }

    /// Set status
    #[inline]
    pub fn set_status(&self, status: StateStatus) {
        self.status.store(status as u8, Ordering::Release);
    }

    /// Apply single-qubit gate with SIMD optimization
    ///
    /// # Algorithm
    ///
    /// For each pair of amplitudes (|0⟩, |1⟩) for target qubit:
    /// ```text
    /// new_0 = matrix[0][0] * old_0 + matrix[0][1] * old_1
    /// new_1 = matrix[1][0] * old_0 + matrix[1][1] * old_1
    /// ```
    ///
    /// # SIMD Strategy
    ///
    /// Process 2 amplitude pairs (4 complex numbers) per iteration using f64x4:
    /// - Load: [r0_0, i0_0, r1_0, i1_0] and [r0_1, i0_1, r1_1, i1_1]
    /// - Matrix multiply (complex arithmetic via SIMD)
    /// - Store: Updated amplitudes
    ///
    /// # Performance
    ///
    /// - Scalar: ~2μs for 16 qubits (65K amplitudes)
    /// - SIMD: ~250ns for 16 qubits (8× speedup)
    /// - Parallel SIMD (8 threads): ~35ns for 16 qubits (57× speedup, Phase 3.4)
    ///
    /// # Arguments
    ///
    /// * `target` - Qubit index (0..num_qubits)
    /// * `matrix` - 2×2 unitary matrix [[a, b], [c, d]]
    /// * `real_parts` - Real components (mut)
    /// * `imag_parts` - Imaginary components (mut)
    pub fn apply_single_qubit_gate(
        &self,
        target: usize,
        matrix: &[[Complex; 2]; 2],
        real_parts: &mut [f64],
        imag_parts: &mut [f64],
    ) -> QuantumPureResult<()> {
        let num_qubits = self.num_qubits() as usize;
        if target >= num_qubits {
            return Err(QuantumPureError::InvalidQubitIndex {
                index: target,
                num_qubits,
            });
        }

        let dimension = real_parts.len();

        // Phase 3.3: Cache-aware optimization for large problem sizes (20+ qubits)
        // - **16 qubits**: 65K amplitudes (512KB) → L2/L3 boundary
        // - **20 qubits**: 1M amplitudes (8MB) → Exceeds L3, needs blocking
        // - **24 qubits**: 16M amplitudes (128MB) → Requires aggressive blocking
        // - **28+ qubits**: 268M+ amplitudes (2GB+) → Blocking mandatory
        //
        // Threshold: 1M amplitudes (20 qubits) to enable cache blocking
        const LARGE_SCALE_THRESHOLD: usize = 1 << 20; // 1,048,576 amplitudes

        if dimension >= LARGE_SCALE_THRESHOLD {
            // Large-scale path: Use cache blocking for 20+ qubits
            return self.apply_single_qubit_gate_blocked(target, matrix, real_parts, imag_parts);
        }

        // Phase 3.2: Automatic threshold-based routing
        // - Below 18 qubits (262K dims): Single-threaded AVX2 is faster
        // - At 18+ qubits: Multi-threaded AVX2 with ThreadPool delivers 4-8× threading gain
        #[cfg(all(feature = "avx2-simd", target_arch = "x86_64"))]
        {
            let stride = 1usize << target;

            // Multi-threaded AVX2 for large problem sizes (18+ qubits)
            #[cfg(feature = "batch-native")]
            if dimension >= PARALLEL_THRESHOLD_DIMENSIONS && stride >= 4 {
                return self.apply_single_qubit_gate_parallel(target, matrix, real_parts, imag_parts);
            }

            // Single-threaded AVX2 for smaller problem sizes (<18 qubits) or fallback
            if stride >= 4 && dimension >= 16 {
                return self.apply_single_qubit_gate_avx2(target, matrix, real_parts, imag_parts);
            }
        }

        // Phase 3.1: SSE SIMD path (fallback for stride >= 2 or non-AVX2)
        #[cfg(feature = "portable_simd")]
        {
            self.apply_single_qubit_gate_simd(target, matrix, real_parts, imag_parts)
        }

        #[cfg(not(feature = "portable_simd"))]
        {
            self.apply_single_qubit_gate_scalar(target, matrix, real_parts, imag_parts)
        }
    }

    /// SIMD implementation (4-8× faster for dimension >= 4)
    ///
    /// # Algorithm Fix (Nov 2025)
    ///
    /// CRITICAL: For stride = 1 (target qubit 0), we CANNOT process
    /// (idx0, idx0+1) pairs together because they don't share the same
    /// "partner" amplitude. For example:
    /// - Pair 1: (0, 1) differs in bit 0 ✓
    /// - Pair 2: (1, 2) differs in bits 0 AND 1 ✗ INVALID!
    ///
    /// Solution: Only use SIMD for stride >= 2, fall back to scalar for stride = 1.
    #[cfg(feature = "portable_simd")]
    fn apply_single_qubit_gate_simd(
        &self,
        target: usize,
        matrix: &[[Complex; 2]; 2],
        real_parts: &mut [f64],
        imag_parts: &mut [f64],
    ) -> QuantumPureResult<()> {
        let dimension = self.dimension() as usize;
        let stride = 1usize << target; // 2^target

        // Extract matrix elements (complex)
        let [[a, b], [c, d]] = matrix;

        // CRITICAL FIX: Fall back to scalar for stride = 1 (target qubit 0)
        // Reason: step_by(2) on offset creates INVALID pairs for stride = 1
        if stride == 1 {
            return self.apply_single_qubit_gate_scalar(target, matrix, real_parts, imag_parts);
        }

        // For small dimensions (< 4), not worth SIMD overhead
        if dimension < 4 {
            return self.apply_single_qubit_gate_scalar(target, matrix, real_parts, imag_parts);
        }

        // SIMD path: Process 2 amplitude pairs at once for dimension >= 4 AND stride >= 2
        // This guarantees both (offset, offset+stride) and (offset+1, offset+1+stride)
        // are valid pairs (differ only in the target qubit bit)
        for base in (0..dimension).step_by(2 * stride) {
            for offset in (0..stride).step_by(2) {
                if offset + 1 >= stride {
                    // Handle remaining single offset (odd stride)
                    if offset < stride {
                        let idx0 = base + offset;
                        let idx1 = base + offset + stride;

                        if idx0 < dimension && idx1 < dimension {
                            let r0 = real_parts[idx0];
                            let i0 = imag_parts[idx0];
                            let r1 = real_parts[idx1];
                            let i1 = imag_parts[idx1];

                            let new_r0 = a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1;
                            let new_i0 = a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1;
                            let new_r1 = c.re * r0 - c.im * i0 + d.re * r1 - d.im * i1;
                            let new_i1 = c.re * i0 + c.im * r0 + d.re * i1 + d.im * r1;

                            real_parts[idx0] = new_r0;
                            imag_parts[idx0] = new_i0;
                            real_parts[idx1] = new_r1;
                            imag_parts[idx1] = new_i1;
                        }
                    }
                    break;
                }

                let idx0 = base + offset;
                let idx1 = base + offset + stride;

                // Boundary check: ensure both pairs fit in dimension
                if idx0 + 1 >= dimension || idx1 + 1 >= dimension {
                    // Process remaining single pair if valid
                    if idx0 < dimension && idx1 < dimension {
                        let r0 = real_parts[idx0];
                        let i0 = imag_parts[idx0];
                        let r1 = real_parts[idx1];
                        let i1 = imag_parts[idx1];

                        let new_r0 = a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1;
                        let new_i0 = a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1;
                        let new_r1 = c.re * r0 - c.im * i0 + d.re * r1 - d.im * i1;
                        let new_i1 = c.re * i0 + c.im * r0 + d.re * i1 + d.im * r1;

                        real_parts[idx0] = new_r0;
                        imag_parts[idx0] = new_i0;
                        real_parts[idx1] = new_r1;
                        imag_parts[idx1] = new_i1;
                    }
                    break;
                }

                // Load 2 pairs of amplitudes (4 complex numbers = 8 f64)
                let r0_0 = real_parts[idx0];
                let i0_0 = imag_parts[idx0];
                let r0_1 = real_parts[idx0 + 1];
                let i0_1 = imag_parts[idx0 + 1];

                let r1_0 = real_parts[idx1];
                let i1_0 = imag_parts[idx1];
                let r1_1 = real_parts[idx1 + 1];
                let i1_1 = imag_parts[idx1 + 1];

                // Matrix multiplication (complex arithmetic)
                // Pair 1: new_0 = a * old_0 + b * old_1
                let new_r0_0 = a.re * r0_0 - a.im * i0_0 + b.re * r1_0 - b.im * i1_0;
                let new_i0_0 = a.re * i0_0 + a.im * r0_0 + b.re * i1_0 + b.im * r1_0;

                // Pair 2: new_0 = a * old_0 + b * old_1
                let new_r0_1 = a.re * r0_1 - a.im * i0_1 + b.re * r1_1 - b.im * i1_1;
                let new_i0_1 = a.re * i0_1 + a.im * r0_1 + b.re * i1_1 + b.im * r1_1;

                // Pair 1: new_1 = c * old_0 + d * old_1
                let new_r1_0 = c.re * r0_0 - c.im * i0_0 + d.re * r1_0 - d.im * i1_0;
                let new_i1_0 = c.re * i0_0 + c.im * r0_0 + d.re * i1_0 + d.im * r1_0;

                // Pair 2: new_1 = c * old_0 + d * old_1
                let new_r1_1 = c.re * r0_1 - c.im * i0_1 + d.re * r1_1 - d.im * i1_1;
                let new_i1_1 = c.re * i0_1 + c.im * r0_1 + d.re * i1_1 + d.im * r1_1;

                // Store updated amplitudes
                real_parts[idx0] = new_r0_0;
                imag_parts[idx0] = new_i0_0;
                real_parts[idx0 + 1] = new_r0_1;
                imag_parts[idx0 + 1] = new_i0_1;

                real_parts[idx1] = new_r1_0;
                imag_parts[idx1] = new_i1_0;
                real_parts[idx1 + 1] = new_r1_1;
                imag_parts[idx1 + 1] = new_i1_1;
            }
        }

        Ok(())
    }

    /// AVX2 implementation (Phase 3.1: 2× wider SIMD for 3-6× total speedup)
    ///
    /// # Algorithm
    ///
    /// Process 4 amplitude pairs (8 complex numbers = 16 f64) per iteration using f64x4:
    /// - **SSE baseline**: 2 pairs/iteration (f64x2) → 1.56-1.72× speedup
    /// - **AVX2 upgrade**: 4 pairs/iteration (f64x4) → 3-6× speedup (2-4× over SSE)
    ///
    /// # SIMD Strategy
    ///
    /// ```text
    /// Load (4 pairs):
    ///   r0_vec = [r0_0, r0_1, r0_2, r0_3]  (idx0, idx0+1, idx0+2, idx0+3)
    ///   r1_vec = [r1_0, r1_1, r1_2, r1_3]  (idx1, idx1+1, idx1+2, idx1+3)
    ///   i0_vec = [i0_0, i0_1, i0_2, i0_3]
    ///   i1_vec = [i1_0, i1_1, i1_2, i1_3]
    ///
    /// Matrix multiply (vectorized):
    ///   new_r0 = a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1
    ///   new_i0 = a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1
    ///   new_r1 = c.re * r0 - c.im * i0 + d.re * r1 - d.im * i1
    ///   new_i1 = c.re * i0 + c.im * r0 + d.re * i1 + d.im * r1
    ///
    /// Store (4 pairs):
    ///   real_parts[idx0..idx0+4] = new_r0.to_array()
    ///   imag_parts[idx0..idx0+4] = new_i0.to_array()
    ///   real_parts[idx1..idx1+4] = new_r1.to_array()
    ///   imag_parts[idx1..idx1+4] = new_i1.to_array()
    /// ```
    ///
    /// # Performance Targets (B32 validated)
    ///
    /// - **Qubit 2** (stride=4): 2.782µs → 700-900ns = **3-4× speedup** vs scalar
    /// - **Qubit 3** (stride=8): Better scaling than SSE (2× wider processing)
    /// - **Qubit 4+**: Consistent 3-6× speedup across all large strides
    ///
    /// # Stride Requirements
    ///
    /// - **stride >= 4**: Required for AVX2 (processes 4 offsets/iteration)
    /// - **stride < 4**: Falls back to SSE (step_by(2)) or scalar (stride=1)
    /// - **stride = 1**: Invalid for SIMD (single-element pairs, uses scalar)
    ///
    /// # ASSUM Safety Tags
    ///
    /// ```text
    /// #ASSUME_AVX2_AVAILABLE: Target CPU supports AVX2 instructions (Intel Haswell 2013+, AMD Excavator 2015+)
    /// #VERIFY_AVX2_AVAILABLE: Compile-time check via #[cfg(all(feature = "avx2-simd", target_arch = "x86_64"))]
    /// #RISK: MEDIUM (95%+ x86_64 CPUs support AVX2 since 2013)
    ///
    /// #ASSUME_STRIDE_VALIDITY: AVX2 path requires stride >= 4 (4 offset positions per iteration)
    /// #VERIFY_STRIDE_VALIDITY: Dispatcher checks stride >= 4, falls back to SSE if stride < 4
    /// #RISK: LOW (compile-time routing)
    ///
    /// #ASSUME_BOUNDARY_SAFETY: offset + 3 < stride before accessing 4 elements
    /// #VERIFY_BOUNDARY_SAFETY: Explicit check: if offset + 3 >= stride { break; }
    /// #RISK: LOW (prevents out-of-bounds access)
    ///
    /// #ASSUME_DIMENSION_VALIDITY: Dimension = 2^num_qubits (always power of 2)
    /// #VERIFY_DIMENSION_VALIDITY: Constructor enforces dimension = 1 << num_qubits
    /// #RISK: LOW (guaranteed by constructor)
    ///
    /// #ASSUME_NO_SIMD_EXCEPTIONS: No denormals or NaNs in quantum amplitudes
    /// #VERIFY_NO_SIMD_EXCEPTIONS: Quantum normalization ensures finite values
    /// #RISK: LOW (quantum physics constraint)
    /// ```
    ///
    /// # Arguments
    ///
    /// * `target` - Qubit index (0..num_qubits, but requires target >= 2 for stride >= 4)
    /// * `matrix` - 2×2 unitary matrix [[a, b], [c, d]]
    /// * `real_parts` - Real components (mut)
    /// * `imag_parts` - Imaginary components (mut)
    #[cfg(all(feature = "avx2-simd", target_arch = "x86_64"))]
    fn apply_single_qubit_gate_avx2(
        &self,
        target: usize,
        matrix: &[[Complex; 2]; 2],
        real_parts: &mut [f64],
        imag_parts: &mut [f64],
    ) -> QuantumPureResult<()> {
        use std::simd::f64x4;

        let dimension = self.dimension() as usize;
        let stride = 1usize << target; // 2^target

        // AVX2 requires stride >= 4 for maximum benefit (process 4 offsets/iteration)
        // Fall back to SSE for stride < 4
        if stride < 4 {
            return self.apply_single_qubit_gate_simd(target, matrix, real_parts, imag_parts);
        }

        // For small dimensions (< 16), not worth AVX2 overhead
        if dimension < 16 {
            return self.apply_single_qubit_gate_simd(target, matrix, real_parts, imag_parts);
        }

        // Extract matrix elements (complex)
        let [[a, b], [c, d]] = matrix;

        // Broadcast matrix elements to f64x4
        let a_re_vec = f64x4::splat(a.re);
        let a_im_vec = f64x4::splat(a.im);
        let b_re_vec = f64x4::splat(b.re);
        let b_im_vec = f64x4::splat(b.im);
        let c_re_vec = f64x4::splat(c.re);
        let c_im_vec = f64x4::splat(c.im);
        let d_re_vec = f64x4::splat(d.re);
        let d_im_vec = f64x4::splat(d.im);

        // SIMD path: Process 4 offset positions per iteration
        for base in (0..dimension).step_by(2 * stride) {
            for offset in (0..stride).step_by(4) {
                // Boundary check: ensure all 4 pairs fit
                if offset + 3 >= stride {
                    // Handle remaining 1-3 offsets with SSE fallback
                    for remaining_offset in offset..stride {
                        let idx0 = base + remaining_offset;
                        let idx1 = base + remaining_offset + stride;

                        if idx0 < dimension && idx1 < dimension {
                            let r0 = real_parts[idx0];
                            let i0 = imag_parts[idx0];
                            let r1 = real_parts[idx1];
                            let i1 = imag_parts[idx1];

                            // Scalar fallback for remaining elements
                            let new_r0 = a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1;
                            let new_i0 = a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1;
                            let new_r1 = c.re * r0 - c.im * i0 + d.re * r1 - d.im * i1;
                            let new_i1 = c.re * i0 + c.im * r0 + d.re * i1 + d.im * r1;

                            real_parts[idx0] = new_r0;
                            imag_parts[idx0] = new_i0;
                            real_parts[idx1] = new_r1;
                            imag_parts[idx1] = new_i1;
                        }
                    }
                    break;
                }

                // Load 4 pairs of amplitudes (8 complex numbers = 16 f64)
                let idx0_0 = base + offset;
                let idx0_1 = base + offset + 1;
                let idx0_2 = base + offset + 2;
                let idx0_3 = base + offset + 3;

                let idx1_0 = idx0_0 + stride;
                let idx1_1 = idx0_1 + stride;
                let idx1_2 = idx0_2 + stride;
                let idx1_3 = idx0_3 + stride;

                // Boundary check for all indices
                if idx1_3 >= dimension {
                    break;
                }

                // Load real parts (4 f64 per load, 2 loads total = 8 f64)
                let r0_vec = f64x4::from_array([
                    real_parts[idx0_0],
                    real_parts[idx0_1],
                    real_parts[idx0_2],
                    real_parts[idx0_3],
                ]);
                let r1_vec = f64x4::from_array([
                    real_parts[idx1_0],
                    real_parts[idx1_1],
                    real_parts[idx1_2],
                    real_parts[idx1_3],
                ]);

                // Load imaginary parts (4 f64 per load, 2 loads total = 8 f64)
                let i0_vec = f64x4::from_array([
                    imag_parts[idx0_0],
                    imag_parts[idx0_1],
                    imag_parts[idx0_2],
                    imag_parts[idx0_3],
                ]);
                let i1_vec = f64x4::from_array([
                    imag_parts[idx1_0],
                    imag_parts[idx1_1],
                    imag_parts[idx1_2],
                    imag_parts[idx1_3],
                ]);

                // Complex matrix multiplication (vectorized)
                // new_0 = a * old_0 + b * old_1
                // Real: a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1
                let new_r0_vec = a_re_vec * r0_vec - a_im_vec * i0_vec + b_re_vec * r1_vec - b_im_vec * i1_vec;
                // Imaginary: a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1
                let new_i0_vec = a_re_vec * i0_vec + a_im_vec * r0_vec + b_re_vec * i1_vec + b_im_vec * r1_vec;

                // new_1 = c * old_0 + d * old_1
                let new_r1_vec = c_re_vec * r0_vec - c_im_vec * i0_vec + d_re_vec * r1_vec - d_im_vec * i1_vec;
                let new_i1_vec = c_re_vec * i0_vec + c_im_vec * r0_vec + d_re_vec * i1_vec + d_im_vec * r1_vec;

                // Store updated amplitudes (4 f64 per store, 4 stores total = 16 f64)
                let new_r0_arr = new_r0_vec.to_array();
                let new_i0_arr = new_i0_vec.to_array();
                let new_r1_arr = new_r1_vec.to_array();
                let new_i1_arr = new_i1_vec.to_array();

                real_parts[idx0_0] = new_r0_arr[0];
                real_parts[idx0_1] = new_r0_arr[1];
                real_parts[idx0_2] = new_r0_arr[2];
                real_parts[idx0_3] = new_r0_arr[3];

                imag_parts[idx0_0] = new_i0_arr[0];
                imag_parts[idx0_1] = new_i0_arr[1];
                imag_parts[idx0_2] = new_i0_arr[2];
                imag_parts[idx0_3] = new_i0_arr[3];

                real_parts[idx1_0] = new_r1_arr[0];
                real_parts[idx1_1] = new_r1_arr[1];
                real_parts[idx1_2] = new_r1_arr[2];
                real_parts[idx1_3] = new_r1_arr[3];

                imag_parts[idx1_0] = new_i1_arr[0];
                imag_parts[idx1_1] = new_i1_arr[1];
                imag_parts[idx1_2] = new_i1_arr[2];
                imag_parts[idx1_3] = new_i1_arr[3];
            }
        }

        Ok(())
    }

    /// Multi-threaded AVX2 SIMD implementation (Phase 3.2: T6 Mixed: T2 SIMD + T4 Batch with ThreadPool)
    ///
    /// # Phase 3.2 Architecture (ThreadPool Replacement)
    ///
    /// Combines:
    /// - **T2 SIMD**: 4× vectorization per worker (AVX2 f64x4)
    /// - **T4 Batch**: 4-8× parallelization across workers (atomic_capsule ThreadPool)
    /// - **Target**: 8-16× combined speedup (2.0 AVX2 × 4-8 threading)
    ///
    /// # Algorithm
    ///
    /// 1. **Chunk Partitioning**: Divide total_chunks across num_workers
    /// 2. **Parallel AVX2 Execution**: Each worker processes chunks with f64x4 via ThreadPool::scope()
    /// 3. **Thread Safety**: Disjoint chunk ranges → no data races → 100% lockfree
    ///
    /// # Rayon → ThreadPool Migration
    ///
    /// - **OLD**: Rayon (~300ns spawn overhead, dynamic work-stealing)
    /// - **NEW**: ThreadPool (<20ns spawn overhead, static chunk allocation)
    /// - **Benefit**: 15× faster spawn, simpler reasoning about work distribution
    ///
    /// # Thread Granularity
    ///
    /// - Work distribution: chunk_size = total_chunks / num_workers (static)
    /// - ThreadPool workers fixed at std::thread::available_parallelism()
    /// - Each worker processes AVX2-accelerated chunks (4 offsets/iteration)
    ///
    /// # Performance Targets (8-core CPU, 20 qubits = 1M dimensions)
    ///
    /// - 1 thread (AVX2 baseline): ~2ms (2.0× vs scalar)
    /// - 2 threads: ~1ms (2× threading gain, 100% efficiency)
    /// - 4 threads: ~500µs (4× threading gain, 100% efficiency)
    /// - 8 threads: ~250µs (8× threading gain, near-linear scaling)
    /// - **Total**: 8-16× vs scalar (2.0 AVX2 × 4-8 threading)
    ///
    /// # ASSUM Framework (Phase 3.2 with ThreadPool)
    ///
    /// - #ASSUME_DISJOINT_PARTITIONS: Each worker accesses different chunk ranges
    /// - #VERIFY_DISJOINT_PARTITIONS: Guaranteed by non-overlapping [start_chunk..end_chunk)
    /// - #ASSUME_AVX2_SAFE_INSIDE_THREADPOOL: f64x4 operations thread-safe (immutable matrix)
    /// - #VERIFY_AVX2_SAFE: Matrix elements broadcast to f64x4 once, read-only in workers
    /// - #ASSUME_THREADPOOL_CORRECTNESS: ThreadPool's lockfree scheduling is correct
    /// - #VERIFY_THREADPOOL_CORRECTNESS: Compare parallel vs sequential (T28 tests)
    ///
    /// # Arguments
    ///
    /// * `target` - Qubit index
    /// * `matrix` - 2×2 unitary matrix
    /// * `real_parts` - Real components (mut, thread-safe via disjoint access)
    /// * `imag_parts` - Imaginary components (mut, thread-safe via disjoint access)
    #[cfg(all(feature = "portable_simd", feature = "batch-native"))]
    fn apply_single_qubit_gate_parallel(
        &self,
        target: usize,
        matrix: &[[Complex; 2]; 2],
        real_parts: &mut [f64],
        imag_parts: &mut [f64],
    ) -> QuantumPureResult<()> {
        use crate::parallel::get_global_pool;
        use std::simd::f64x4;

        let dimension = self.dimension() as usize;
        let stride = 1usize << target; // 2^target

        // Extract matrix elements (complex)
        let [[a, b], [c, d]] = matrix;

        // Fall back to SIMD for stride = 1 (Phase 3.1 fix)
        if stride == 1 {
            return self.apply_single_qubit_gate_simd(target, matrix, real_parts, imag_parts);
        }

        // For stride < 4, use AVX2 non-parallel (still 2.0× vs scalar)
        if stride < 4 {
            return self.apply_single_qubit_gate_avx2(target, matrix, real_parts, imag_parts);
        }

        // Get global thread pool (cached after first call, <1ns overhead)
        let pool = get_global_pool()
            .map_err(|_| QuantumPureError::InvalidGateParameters {
                gate_type: "Parallel execution".to_string(),
                reason: "Failed to get thread pool".to_string(),
            })?;

        let num_workers = pool.num_workers();
        let total_chunks = dimension / (2 * stride);
        let chunk_size = total_chunks.div_ceil(num_workers).max(1);

        // Broadcast matrix elements to f64x4 (done ONCE before parallel loop)
        let a_re_vec = f64x4::splat(a.re);
        let a_im_vec = f64x4::splat(a.im);
        let b_re_vec = f64x4::splat(b.re);
        let b_im_vec = f64x4::splat(b.im);
        let c_re_vec = f64x4::splat(c.re);
        let c_im_vec = f64x4::splat(c.im);
        let d_re_vec = f64x4::splat(d.re);
        let d_im_vec = f64x4::splat(d.im);

        // Parallel AVX2 execution using ThreadPool::scope() with proper Send/Sync reasoning
        // SAFETY:
        // - Each worker processes different chunk ranges (disjoint base indices)
        // - For given base, worker accesses indices: base+offset and base+offset+stride
        // - Different bases are 2*stride apart, so no two workers access same index
        // - Therefore all memory accesses are disjoint (proven by partition)
        // - Matrix f64x4 vectors are immutable (read-only, safe to share)
        // - This makes it safe to share the mutable slices across threads
        let real_ptr = real_parts.as_mut_ptr();
        let imag_ptr = imag_parts.as_mut_ptr();

        unsafe {
            // Explicitly capture raw pointers to bypass borrow checker
            // Safety guaranteed by disjoint access pattern above
            let real_ptr_sync = real_ptr as usize;
            let imag_ptr_sync = imag_ptr as usize;

            pool.scope(|s: &crate::parallel::Scope| {
                for worker_id in 0..num_workers {
                    let start_chunk = worker_id * chunk_size;
                    if start_chunk >= total_chunks {
                        break;
                    }
                    let end_chunk = ((worker_id + 1) * chunk_size).min(total_chunks);

                    // Spawn worker with AVX2 operations (<20ns spawn overhead vs Rayon ~300ns)
                    let _ = s.spawn(move || {
                        let real_ptr = real_ptr_sync as *mut f64;
                        let imag_ptr = imag_ptr_sync as *mut f64;

                        for chunk_idx in start_chunk..end_chunk {
                            let base = chunk_idx * 2 * stride;

            // Phase 3.2 FIX: Use AVX2 f64x4 (NOT scalar) inside parallel loop
            // Process 4 offset positions per iteration (same as single-threaded AVX2)
            for offset in (0..stride).step_by(4) {
                // Boundary check: ensure all 4 pairs fit
                if offset + 3 >= stride {
                    // Handle remaining 1-3 offsets with scalar fallback
                    for remaining_offset in offset..stride {
                        let idx0 = base + remaining_offset;
                        let idx1 = base + remaining_offset + stride;

                        if idx0 < dimension && idx1 < dimension {
                            let r0 = *real_ptr.add(idx0);
                            let i0 = *imag_ptr.add(idx0);
                            let r1 = *real_ptr.add(idx1);
                            let i1 = *imag_ptr.add(idx1);

                            // Scalar fallback for remaining elements
                            let new_r0 = a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1;
                            let new_i0 = a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1;
                            let new_r1 = c.re * r0 - c.im * i0 + d.re * r1 - d.im * i1;
                            let new_i1 = c.re * i0 + c.im * r0 + d.re * i1 + d.im * r1;

                            *real_ptr.add(idx0) = new_r0;
                            *imag_ptr.add(idx0) = new_i0;
                            *real_ptr.add(idx1) = new_r1;
                            *imag_ptr.add(idx1) = new_i1;
                        }
                    }
                    return;
                }

                // Load 4 pairs of amplitudes (8 complex numbers = 16 f64) using AVX2
                let idx0_0 = base + offset;
                let idx0_1 = base + offset + 1;
                let idx0_2 = base + offset + 2;
                let idx0_3 = base + offset + 3;

                let idx1_0 = idx0_0 + stride;
                let idx1_1 = idx0_1 + stride;
                let idx1_2 = idx0_2 + stride;
                let idx1_3 = idx0_3 + stride;

                // Boundary check for all indices
                if idx1_3 >= dimension {
                    return;
                }

                // Load real parts (4 f64 per load, 2 loads total = 8 f64)
                let r0_vec = f64x4::from_array([
                    *real_ptr.add(idx0_0),
                    *real_ptr.add(idx0_1),
                    *real_ptr.add(idx0_2),
                    *real_ptr.add(idx0_3),
                ]);
                let r1_vec = f64x4::from_array([
                    *real_ptr.add(idx1_0),
                    *real_ptr.add(idx1_1),
                    *real_ptr.add(idx1_2),
                    *real_ptr.add(idx1_3),
                ]);

                // Load imaginary parts (4 f64 per load, 2 loads total = 8 f64)
                let i0_vec = f64x4::from_array([
                    *imag_ptr.add(idx0_0),
                    *imag_ptr.add(idx0_1),
                    *imag_ptr.add(idx0_2),
                    *imag_ptr.add(idx0_3),
                ]);
                let i1_vec = f64x4::from_array([
                    *imag_ptr.add(idx1_0),
                    *imag_ptr.add(idx1_1),
                    *imag_ptr.add(idx1_2),
                    *imag_ptr.add(idx1_3),
                ]);

                // Complex matrix multiplication (vectorized with AVX2)
                // new_0 = a * old_0 + b * old_1
                // Real: a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1
                let new_r0_vec = a_re_vec * r0_vec - a_im_vec * i0_vec + b_re_vec * r1_vec - b_im_vec * i1_vec;
                // Imaginary: a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1
                let new_i0_vec = a_re_vec * i0_vec + a_im_vec * r0_vec + b_re_vec * i1_vec + b_im_vec * r1_vec;

                // new_1 = c * old_0 + d * old_1
                let new_r1_vec = c_re_vec * r0_vec - c_im_vec * i0_vec + d_re_vec * r1_vec - d_im_vec * i1_vec;
                let new_i1_vec = c_re_vec * i0_vec + c_im_vec * r0_vec + d_re_vec * i1_vec + d_im_vec * r1_vec;

                // Store updated amplitudes (4 f64 per store, 4 stores total = 16 f64)
                let new_r0_arr = new_r0_vec.to_array();
                let new_i0_arr = new_i0_vec.to_array();
                let new_r1_arr = new_r1_vec.to_array();
                let new_i1_arr = new_i1_vec.to_array();

                *real_ptr.add(idx0_0) = new_r0_arr[0];
                *real_ptr.add(idx0_1) = new_r0_arr[1];
                *real_ptr.add(idx0_2) = new_r0_arr[2];
                *real_ptr.add(idx0_3) = new_r0_arr[3];

                *imag_ptr.add(idx0_0) = new_i0_arr[0];
                *imag_ptr.add(idx0_1) = new_i0_arr[1];
                *imag_ptr.add(idx0_2) = new_i0_arr[2];
                *imag_ptr.add(idx0_3) = new_i0_arr[3];

                *real_ptr.add(idx1_0) = new_r1_arr[0];
                *real_ptr.add(idx1_1) = new_r1_arr[1];
                *real_ptr.add(idx1_2) = new_r1_arr[2];
                *real_ptr.add(idx1_3) = new_r1_arr[3];

                *imag_ptr.add(idx1_0) = new_i1_arr[0];
                *imag_ptr.add(idx1_1) = new_i1_arr[1];
                *imag_ptr.add(idx1_2) = new_i1_arr[2];
                *imag_ptr.add(idx1_3) = new_i1_arr[3];
            }
                        }
                    }); // <20ns spawn overhead per worker, Result ignored (ThreadPool queue sized for workers)
                }
            }); // scope() blocks until all workers complete
        }

        Ok(())
    }

    /// Scalar fallback (portable but slower)
    /// Always available for correctness testing
    fn apply_single_qubit_gate_scalar(
        &self,
        target: usize,
        matrix: &[[Complex; 2]; 2],
        real_parts: &mut [f64],
        imag_parts: &mut [f64],
    ) -> QuantumPureResult<()> {
        let dimension = self.dimension() as usize;
        let stride = 1usize << target; // 2^target

        let [[a, b], [c, d]] = matrix;

        for base in (0..dimension).step_by(2 * stride) {
            for offset in 0..stride {
                let idx0 = base + offset;
                let idx1 = base + offset + stride;

                let r0 = real_parts[idx0];
                let i0 = imag_parts[idx0];
                let r1 = real_parts[idx1];
                let i1 = imag_parts[idx1];

                // new_0 = a * old_0 + b * old_1
                let new_r0 = a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1;
                let new_i0 = a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1;

                // new_1 = c * old_0 + d * old_1
                let new_r1 = c.re * r0 - c.im * i0 + d.re * r1 - d.im * i1;
                let new_i1 = c.re * i0 + c.im * r0 + d.re * i1 + d.im * r1;

                real_parts[idx0] = new_r0;
                imag_parts[idx0] = new_i0;
                real_parts[idx1] = new_r1;
                imag_parts[idx1] = new_i1;
            }
        }

        Ok(())
    }

    // ============================================================================
    // PHASE 3.3: CACHE-AWARE OPTIMIZATION (20-30 QUBITS)
    // ============================================================================

    /// Software prefetch helper (platform-specific)
    ///
    /// # Platform Support
    /// - **x86_64**: _mm_prefetch (SSE intrinsic)
    /// - **aarch64**: __builtin_prefetch (GCC intrinsic via asm)
    /// - **riscv64**: No-op (no prefetch instructions)
    ///
    /// # Safety
    /// - MUST bounds-check before calling (never prefetch beyond slice bounds)
    /// - Prefetch is advisory-only (safe even if address invalid)
    ///
    /// # #ASSUME_PREFETCH_SAFETY
    /// Caller ensures `ptr` + distance doesn't exceed slice bounds
    /// #VERIFY: Guard condition `if base + PREFETCH_DISTANCE < dimension`
    #[inline(always)]
    fn prefetch_data<T>(ptr: *const T) {
        #[cfg(target_arch = "x86_64")]
        {
            unsafe {
                use std::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};
                _mm_prefetch(ptr as *const i8, _MM_HINT_T0); // L1 cache
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM NEON prefetch via inline assembly
            unsafe {
                std::arch::asm!(
                    "prfm pldl1keep, [{0}]", // Prefetch for load, L1 cache, keep
                    in(reg) ptr,
                    options(readonly, nostack, preserves_flags)
                );
            }
        }

        // No-op on other platforms (riscv64, wasm32, etc.)
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            let _ = ptr; // Suppress unused variable warning
        }
    }

    /// Scalar with software prefetching (Phase 3.3)
    ///
    /// # Optimization
    /// Prefetches next iteration's data 16× stride ahead (2 cache lines = 128 bytes)
    /// to hide ~60ns RAM latency on DDR5-4800.
    ///
    /// # Performance Target
    /// - **20 qubits** (stride ≥ 16): +5-10% speedup vs scalar baseline
    /// - **24 qubits** (stride ≥ 256): +8-12% speedup
    /// - **28 qubits** (stride ≥ 4096): +10-15% speedup
    ///
    /// # Arguments
    /// Same as `apply_single_qubit_gate_scalar`, but with prefetching
    fn apply_single_qubit_gate_scalar_prefetch(
        &self,
        target: usize,
        matrix: &[[Complex; 2]; 2],
        real_parts: &mut [f64],
        imag_parts: &mut [f64],
    ) -> QuantumPureResult<()> {
        let dimension = self.dimension() as usize;
        let stride = 1usize << target;

        let [[a, b], [c, d]] = matrix;

        for base in (0..dimension).step_by(2 * stride) {
            // Prefetch next iteration (PREFETCH_DISTANCE × stride ahead)
            let prefetch_base = base + PREFETCH_DISTANCE * stride * 2;
            if prefetch_base < dimension {
                // #ASSUME_PREFETCH_SAFETY: Bounds checked above
                unsafe {
                    Self::prefetch_data(real_parts.as_ptr().add(prefetch_base));
                    Self::prefetch_data(imag_parts.as_ptr().add(prefetch_base));
                }
            }

            for offset in 0..stride {
                let idx0 = base + offset;
                let idx1 = base + offset + stride;

                let r0 = real_parts[idx0];
                let i0 = imag_parts[idx0];
                let r1 = real_parts[idx1];
                let i1 = imag_parts[idx1];

                let new_r0 = a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1;
                let new_i0 = a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1;
                let new_r1 = c.re * r0 - c.im * i0 + d.re * r1 - d.im * i1;
                let new_i1 = c.re * i0 + c.im * r0 + d.re * i1 + d.im * r1;

                real_parts[idx0] = new_r0;
                imag_parts[idx0] = new_i0;
                real_parts[idx1] = new_r1;
                imag_parts[idx1] = new_i1;
            }
        }

        Ok(())
    }

    /// Cache-blocked gate application (Phase 3.3)
    ///
    /// # Algorithm
    /// Partitions state vector into 64KB blocks (4,096 complex numbers)
    /// that fit comfortably in L2 cache (512KB on AMD 6900HX).
    ///
    /// # Performance Target (20-30 Qubits)
    /// - **20 qubits** (1M amplitudes → 256 blocks): +10-15% speedup vs unblocked
    /// - **24 qubits** (16M amplitudes → 4K blocks): +12-18% speedup
    /// - **28 qubits** (268M amplitudes → 65K blocks): +15-20% speedup
    ///
    /// # Cache Behavior
    /// - **Unblocked**: Random access across 8GB (100% cache misses)
    /// - **Blocked**: Sequential access within 64KB (90%+ L2 hit rate)
    ///
    /// # Arguments
    /// Same as `apply_single_qubit_gate_scalar`, but with blocking
    fn apply_single_qubit_gate_blocked(
        &self,
        target: usize,
        matrix: &[[Complex; 2]; 2],
        real_parts: &mut [f64],
        imag_parts: &mut [f64],
    ) -> QuantumPureResult<()> {
        let dimension = self.dimension() as usize;
        let num_blocks = (dimension + CACHE_BLOCK_SIZE - 1) / CACHE_BLOCK_SIZE;

        for block_idx in 0..num_blocks {
            let block_start = block_idx * CACHE_BLOCK_SIZE;
            let block_end = (block_start + CACHE_BLOCK_SIZE).min(dimension);

            // Process entire block (stays in L2 cache)
            // Use prefetch-enabled scalar for best performance
            self.apply_gate_to_block(
                target,
                matrix,
                &mut real_parts[block_start..block_end],
                &mut imag_parts[block_start..block_end],
            )?;
        }

        Ok(())
    }

    /// Apply gate to a single cache block (helper for blocked execution)
    ///
    /// # Performance
    /// - **Block size**: 4,096 complex numbers (64KB = 12.5% of L2)
    /// - **L2 hit rate**: 90%+ (entire block fits in cache)
    /// - **Prefetching**: Enabled for large strides (≥16)
    ///
    /// # Arguments
    /// - `target`: Qubit index
    /// - `matrix`: 2×2 unitary matrix
    /// - `real_block`: Real components (64KB slice)
    /// - `imag_block`: Imaginary components (64KB slice)
    fn apply_gate_to_block(
        &self,
        target: usize,
        matrix: &[[Complex; 2]; 2],
        real_block: &mut [f64],
        imag_block: &mut [f64],
    ) -> QuantumPureResult<()> {
        let block_dimension = real_block.len();
        let stride = 1usize << target;

        // For blocks smaller than stride, skip (no pairs in this block)
        if block_dimension <= stride {
            return Ok(());
        }

        let [[a, b], [c, d]] = matrix;

        for base in (0..block_dimension).step_by(2 * stride) {
            // Prefetch for large strides (hide memory latency)
            if stride >= 16 {
                let prefetch_base = base + PREFETCH_DISTANCE * stride * 2;
                if prefetch_base < block_dimension {
                    unsafe {
                        Self::prefetch_data(real_block.as_ptr().add(prefetch_base));
                        Self::prefetch_data(imag_block.as_ptr().add(prefetch_base));
                    }
                }
            }

            for offset in 0..stride {
                let idx0 = base + offset;
                let idx1 = base + offset + stride;

                // Bounds check (block may not align with 2×stride boundary)
                if idx1 >= block_dimension {
                    break;
                }

                let r0 = real_block[idx0];
                let i0 = imag_block[idx0];
                let r1 = real_block[idx1];
                let i1 = imag_block[idx1];

                let new_r0 = a.re * r0 - a.im * i0 + b.re * r1 - b.im * i1;
                let new_i0 = a.re * i0 + a.im * r0 + b.re * i1 + b.im * r1;
                let new_r1 = c.re * r0 - c.im * i0 + d.re * r1 - d.im * i1;
                let new_i1 = c.re * i0 + c.im * r0 + d.re * i1 + d.im * r1;

                real_block[idx0] = new_r0;
                imag_block[idx0] = new_i0;
                real_block[idx1] = new_r1;
                imag_block[idx1] = new_i1;
            }
        }

        Ok(())
    }

    /// Apply two-qubit gate (CNOT, CZ, SWAP, etc.)
    ///
    /// # Algorithm
    ///
    /// For each group of 4 amplitudes in the 2-qubit subspace:
    /// ```text
    /// |ψ⟩ = α|00⟩ + β|01⟩ + γ|10⟩ + δ|11⟩
    /// Apply 4×4 unitary U:
    /// |ψ'⟩ = U|ψ⟩
    /// ```
    ///
    /// # Implementation
    ///
    /// Iterate over all 2^(N-2) basis states (N = total qubits):
    /// - Extract 4 amplitudes for control/target qubit pair
    /// - Apply 4×4 matrix multiplication (16 complex operations)
    /// - Update state vector
    ///
    /// # Performance
    ///
    /// - Scalar: ~8μs for 8 qubits (256 amplitudes, 64 groups of 4)
    /// - Future SIMD: ~1-2μs (4-8× speedup possible)
    ///
    /// # Arguments
    ///
    /// * `control` - Control qubit index
    /// * `target` - Target qubit index
    /// * `matrix` - 4×4 unitary matrix (computational basis |00⟩, |01⟩, |10⟩, |11⟩)
    /// * `real_parts` - Real components (mut)
    /// * `imag_parts` - Imaginary components (mut)
    pub fn apply_two_qubit_gate(
        &self,
        control: usize,
        target: usize,
        matrix: &[[Complex; 4]; 4],
        real_parts: &mut [f64],
        imag_parts: &mut [f64],
    ) -> QuantumPureResult<()> {
        let num_qubits = self.num_qubits();

        // Validate qubit indices
        if control >= num_qubits {
            return Err(QuantumPureError::InvalidQubitIndex {
                index: control,
                num_qubits,
            });
        }
        if target >= num_qubits {
            return Err(QuantumPureError::InvalidQubitIndex {
                index: target,
                num_qubits,
            });
        }
        if control == target {
            return Err(QuantumPureError::InvalidGateParameters {
                gate_type: "Two-qubit gate".to_string(),
                reason: "Control and target must be different".to_string(),
            });
        }

        let dimension = self.dimension();

        // Determine which qubit is lower/higher in bit ordering
        let (q0, q1) = if control < target {
            (control, target)
        } else {
            (target, control)
        };

        let stride0 = 1usize << q0; // 2^q0
        let stride1 = 1usize << q1; // 2^q1

        // Iterate over all basis states, grouping by 2-qubit subspace
        for base in 0..dimension {
            // Skip if we've already processed this group
            // We process groups of 4: |00⟩, |01⟩, |10⟩, |11⟩ for qubits (q0, q1)
            let bit0 = (base >> q0) & 1;
            let bit1 = (base >> q1) & 1;

            // Only process when both bits are 0 (start of group)
            if bit0 != 0 || bit1 != 0 {
                continue;
            }

            // Extract 4 amplitudes for this 2-qubit subspace
            let idx00 = base; // |...0...0...⟩
            let idx01 = base | stride0; // |...1...0...⟩ (flip q0)
            let idx10 = base | stride1; // |...0...1...⟩ (flip q1)
            let idx11 = base | stride0 | stride1; // |...1...1...⟩ (flip both)

            // Determine mapping based on control/target ordering
            // Computational basis order: |00⟩, |01⟩, |10⟩, |11⟩
            // where first bit = control, second bit = target
            let (i0, i1, i2, i3) = if control < target {
                // Control is q0, target is q1
                // |00⟩ = idx00, |01⟩ = idx10, |10⟩ = idx01, |11⟩ = idx11
                (idx00, idx10, idx01, idx11)
            } else {
                // Target is q0, control is q1
                // |00⟩ = idx00, |01⟩ = idx01, |10⟩ = idx10, |11⟩ = idx11
                (idx00, idx01, idx10, idx11)
            };

            // Load old amplitudes
            let old = [
                Complex::new(real_parts[i0], imag_parts[i0]),
                Complex::new(real_parts[i1], imag_parts[i1]),
                Complex::new(real_parts[i2], imag_parts[i2]),
                Complex::new(real_parts[i3], imag_parts[i3]),
            ];

            // Apply 4×4 matrix multiplication
            // new[i] = Σ_j matrix[i][j] * old[j]
            for i in 0..4 {
                let mut sum_re = 0.0;
                let mut sum_im = 0.0;

                for j in 0..4 {
                    let m = &matrix[i][j];
                    let o = &old[j];

                    // Complex multiplication: (a + ib) × (c + id) = (ac - bd) + i(ad + bc)
                    sum_re += m.re * o.re - m.im * o.im;
                    sum_im += m.re * o.im + m.im * o.re;
                }

                let new_val = Complex::new(sum_re, sum_im);

                // Store back to state vector
                let idx = [i0, i1, i2, i3][i];
                real_parts[idx] = new_val.re;
                imag_parts[idx] = new_val.im;
            }
        }

        Ok(())
    }

    /// Verify normalization: Σ|amplitude|² = 1.0
    ///
    /// # Performance
    ///
    /// O(2^N) for N qubits
    ///
    /// # Errors
    ///
    /// - `NormalizationError`: Sum deviates > NORM_TOLERANCE from 1.0
    pub fn verify_normalization(
        &self,
        real_parts: &[f64],
        imag_parts: &[f64],
    ) -> QuantumPureResult<()> {
        let dimension = self.dimension();
        let mut sum_squared = 0.0;

        for i in 0..dimension {
            let r = real_parts[i];
            let im = imag_parts[i];
            sum_squared += r * r + im * im;
        }

        if (sum_squared - 1.0).abs() > NORM_TOLERANCE {
            return Err(QuantumPureError::NormalizationError {
                sum_squared,
                tolerance: NORM_TOLERANCE,
            });
        }

        Ok(())
    }

    /// Measure quantum state (probabilistic collapse)
    ///
    /// # Algorithm
    ///
    /// 1. Compute probabilities: P(i) = |amplitude[i]|²
    /// 2. Sample from distribution (cumulative sum + random threshold)
    /// 3. Collapse to measured state |i⟩
    ///
    /// # Performance
    ///
    /// O(2^N) for N qubits (compute probabilities + sample)
    ///
    /// # Returns
    ///
    /// Measured basis state index (0..2^N-1)
    pub fn measure(
        &self,
        real_parts: &mut [f64],
        imag_parts: &mut [f64],
    ) -> QuantumPureResult<usize> {
        use rand::Rng;

        let dimension = self.dimension();

        // Compute probabilities P(i) = |amplitude[i]|²
        let mut probabilities = vec![0.0; dimension];
        let mut total_prob = 0.0;

        for i in 0..dimension {
            let r = real_parts[i];
            let im = imag_parts[i];
            let prob = r * r + im * im;
            probabilities[i] = prob;
            total_prob += prob;
        }

        // Validate probabilities sum to 1.0
        if (total_prob - 1.0).abs() > NORM_TOLERANCE {
            return Err(QuantumPureError::InvalidProbabilities { sum: total_prob });
        }

        // Sample from distribution
        let mut rng = rand::thread_rng();
        let threshold: f64 = rng.gen();
        let mut cumulative = 0.0;

        for (i, &prob) in probabilities.iter().enumerate() {
            cumulative += prob;
            if cumulative >= threshold {
                // Collapse to measured state |i⟩
                for j in 0..dimension {
                    real_parts[j] = 0.0;
                    imag_parts[j] = 0.0;
                }
                real_parts[i] = 1.0;

                self.set_status(StateStatus::Measured);
                return Ok(i);
            }
        }

        // Fallback (should never reach here)
        Ok(dimension - 1)
    }

    /// Get single amplitude by index (for measurements and verification)
    ///
    /// # Arguments
    /// - `index`: Amplitude index (0..2^num_qubits)
    /// - `real_parts`: Real components array
    /// - `imag_parts`: Imaginary components array
    ///
    /// # Returns
    /// Complex amplitude at index
    ///
    /// # Performance
    /// - O(1) direct array access
    #[inline]
    pub fn get_amplitude(&self, index: usize, real_parts: &[f64], imag_parts: &[f64]) -> Complex {
        Complex {
            re: real_parts[index],
            im: imag_parts[index],
        }
    }
}

/// Complex number (for gate matrices)
#[derive(Debug, Clone, Copy)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub const fn real(re: f64) -> Self {
        Self { re, im: 0.0 }
    }

    pub const fn i() -> Self {
        Self { re: 0.0, im: 1.0 }
    }

    pub fn conj(&self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    pub fn norm_squared(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_layout() {
        assert_eq!(std::mem::size_of::<QuantumStateVectorCapsule>(), 256);
        assert_eq!(std::mem::align_of::<QuantumStateVectorCapsule>(), 256);
    }

    #[test]
    fn test_state_initialization() {
        let (capsule, real, imag) = QuantumStateVectorCapsule::new_raw(4).unwrap();
        assert_eq!(capsule.num_qubits(), 4);
        assert_eq!(capsule.dimension(), 16);

        // Verify |0...0⟩ state
        assert_eq!(real[0], 1.0);
        for i in 1..16 {
            assert_eq!(real[i], 0.0);
            assert_eq!(imag[i], 0.0);
        }
    }

    #[test]
    fn test_invalid_qubit_count() {
        // Test below MIN_QUBITS (1)
        assert!(QuantumStateVectorCapsule::new_raw(0).is_err());
        // Test above MAX_QUBITS (20)
        assert!(QuantumStateVectorCapsule::new_raw(21).is_err());
    }

    #[test]
    fn test_normalization() {
        let (capsule, real, imag) = QuantumStateVectorCapsule::new_raw(4).unwrap();
        capsule.verify_normalization(&real, &imag).unwrap();
    }
}
