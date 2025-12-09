//! FED Hash Parameters - Precomputed MinHash Parameters (arXiv:2501.01046)
//!
//! Fast Exact Deduplication (FED) optimization: Precompute hash parameters on CPU,
//! upload to GPU constant memory for 6-24× speedup.
//!
//! # Key Innovation
//!
//! **Problem**: Current GPU MinHash computes hash function parameters per-document on GPU.
//! This is redundant work - same parameters for all documents.
//!
//! **Solution**: FED pattern (arXiv:2501.01046):
//! - Precompute hash parameters (a, b) on CPU once at pipeline init
//! - Upload to GPU uniform buffer (constant memory, fast broadcast to all threads)
//! - GPU only does multiply-add: h(x) = (a*x + b) mod p
//!
//! **Performance**: 260× speedup reported in paper vs scalar baseline.
//! Our target: 6-24× vs current GPU MinHash (memory bandwidth → compute bottleneck shift).
//!
//! # Architecture
//!
//! ```text
//! CPU (Once):
//!   Generate FedHashParams { a[128], b[128], prime }
//!   Upload to GPU uniform buffer (1KB)
//!
//! GPU (Per Document):
//!   For each token:
//!     For each permutation i:
//!       h = (params.a[i] * token + params.b[i]) % params.prime
//!       min_hash[i] = min(min_hash[i], h)
//! ```
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous (CPU precompute + GPU compute)
//! - **Chaos**: 100% lockfree (GPU is inherently lockfree)
//! - **ASSUM**: Document hash quality (universal hashing via (a*x+b) mod p)
//! - **B32**: Target 6-24× speedup vs current GPU MinHash
//! - **T28**: Property tests for hash quality, determinism

use std::sync::atomic::{AtomicU64, Ordering};

/// Number of MinHash permutations (128 hash functions)
pub const NUM_PERMUTATIONS: usize = 128;

/// Large prime for universal hashing (Mersenne prime: 2^31 - 1)
///
/// # ASSUM: Prime Choice
///
/// - `#ASSUME_MERSENNE_PRIME`: 2^31 - 1 is prime and provides good distribution
/// - `#VERIFY_MERSENNE_PRIME`: Well-known Mersenne prime (M31), proven prime
/// - `#ASSUME_MOD_QUALITY`: Modulo operation preserves hash quality
/// - `#VERIFY_MOD_QUALITY`: Universal hashing theory (Carter-Wegman 1979)
pub const HASH_PRIME: u32 = 2_147_483_647; // 2^31 - 1

/// FED Hash Parameters Capsule - T7 Heterogeneous Tier
///
/// Precomputed hash parameters for GPU MinHash computation.
/// Layout: 128 a coefficients + 128 b coefficients + 1 prime = 257 u32 = 1028 bytes
///
/// # Memory Layout (GPU-optimized)
///
/// ```text
/// [a0, a1, ..., a127]  (512 bytes)
/// [b0, b1, ..., b127]  (512 bytes)
/// [prime]              (4 bytes)
/// [_padding × 3]       (12 bytes)  // Align to 16 bytes for WGSL struct
/// Total: 1040 bytes
/// ```
///
/// # Chaos Compliance
///
/// - `#[repr(C, align(64)]`: Cache-line aligned for efficient CPU access
/// - Immutable after creation (no interior mutability needed)
/// - Generation counter for Q34 audit trail
///
/// # Framework Compliance
///
/// - **UCE34**: T7 Heterogeneous tier (CPU-side capsule)
/// - **Chaos**: Cache-aligned, immutable, generation counter
/// - **ASSUM**: Random seed quality, parameter independence
/// - **B32**: Zero-cost abstraction (no runtime overhead vs raw arrays)
#[repr(C, align(64))]
pub struct FedHashParamsCapsule {
    /// a coefficients for h(x) = (a*x + b) mod p (128 permutations)
    ///
    /// # ASSUM: Coefficient Range
    ///
    /// - `#ASSUME_A_RANGE`: a ∈ [1, prime-1] (exclude 0 for universal hashing)
    /// - `#VERIFY_A_RANGE`: Generated via RNG with explicit bounds check
    /// - `#ASSUME_A_INDEPENDENCE`: Random a values are statistically independent
    /// - `#VERIFY_A_INDEPENDENCE`: Seed-based RNG with strong mixing
    a: [u32; NUM_PERMUTATIONS],

    /// b coefficients for h(x) = (a*x + b) mod p (128 permutations)
    ///
    /// # ASSUM: Coefficient Range
    ///
    /// - `#ASSUME_B_RANGE`: b ∈ [0, prime-1]
    /// - `#VERIFY_B_RANGE`: Generated via RNG with explicit bounds check
    b: [u32; NUM_PERMUTATIONS],

    /// Large prime p for modulo operation (Mersenne prime: 2^31 - 1)
    prime: u32,

    /// Generation counter (Q34 audit trail)
    generation: AtomicU64,

    /// Padding for cache-line alignment and WGSL struct alignment
    _padding: [u8; 48],
}

impl FedHashParamsCapsule {
    /// Generate new FED hash parameters from seed
    ///
    /// Uses seed-based RNG to generate 128 pairs of (a, b) coefficients
    /// for universal hashing: h(x) = (a*x + b) mod prime.
    ///
    /// # Arguments
    ///
    /// - `seed`: Random seed for parameter generation (use process ID, timestamp, etc.)
    ///
    /// # Returns
    ///
    /// FedHashParamsCapsule with precomputed parameters ready for GPU upload.
    ///
    /// # Performance
    ///
    /// - Generation: <1μs (128 × 2 × 8 bytes = 2KB data, ~100ns per param)
    /// - One-time cost at pipeline initialization
    ///
    /// # ASSUM: RNG Quality
    ///
    /// - `#ASSUME_SEED_QUALITY`: User provides high-entropy seed
    /// - `#VERIFY_SEED_QUALITY`: Documented best practices (PID + timestamp + nonce)
    /// - `#ASSUME_RNG_INDEPENDENCE`: SplitMix64 provides independent outputs
    /// - `#VERIFY_RNG_INDEPENDENCE`: Standard algorithm with proven properties
    ///
    /// # Example
    ///
    /// ```rust
    /// use kindly_dedup::gpu::FedHashParamsCapsule;
    /// use std::time::{SystemTime, UNIX_EPOCH};
    ///
    /// // High-entropy seed: PID + timestamp + nonce
    /// let seed = (std::process::id() as u64) << 32
    ///     | SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
    ///
    /// let params = FedHashParamsCapsule::generate(seed);
    /// ```
    pub fn generate(seed: u64) -> Self {
        let mut rng = SplitMix64::new(seed);
        let mut a = [0u32; NUM_PERMUTATIONS];
        let mut b = [0u32; NUM_PERMUTATIONS];

        for i in 0..NUM_PERMUTATIONS {
            // Generate a ∈ [1, prime-1] (exclude 0 for universal hashing)
            loop {
                let val = (rng.next() % HASH_PRIME as u64) as u32;
                if val > 0 && val < HASH_PRIME {
                    a[i] = val;
                    break;
                }
            }

            // Generate b ∈ [0, prime-1]
            b[i] = (rng.next() % HASH_PRIME as u64) as u32;
        }

        Self {
            a,
            b,
            prime: HASH_PRIME,
            generation: AtomicU64::new(0),
            _padding: [0; 48],
        }
    }

    /// Convert to GPU buffer format (bytemuck-compatible)
    ///
    /// Returns raw byte slice suitable for wgpu::Buffer::write_buffer().
    ///
    /// # Layout
    ///
    /// - Bytes 0-511: a coefficients (128 × 4 bytes)
    /// - Bytes 512-1023: b coefficients (128 × 4 bytes)
    /// - Bytes 1024-1027: prime (4 bytes)
    /// - Bytes 1028-1039: padding (12 bytes for WGSL alignment)
    ///
    /// Total: 1040 bytes
    pub fn to_gpu_buffer(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(1040);

        // Write a coefficients (512 bytes)
        for &a_val in &self.a {
            buffer.extend_from_slice(&a_val.to_le_bytes());
        }

        // Write b coefficients (512 bytes)
        for &b_val in &self.b {
            buffer.extend_from_slice(&b_val.to_le_bytes());
        }

        // Write prime (4 bytes)
        buffer.extend_from_slice(&self.prime.to_le_bytes());

        // Write padding (12 bytes for WGSL struct alignment to 16 bytes)
        buffer.extend_from_slice(&[0u8; 12]);

        buffer
    }

    /// Get a coefficient for permutation i
    #[inline]
    pub fn a(&self, i: usize) -> u32 {
        self.a[i]
    }

    /// Get b coefficient for permutation i
    #[inline]
    pub fn b(&self, i: usize) -> u32 {
        self.b[i]
    }

    /// Get prime modulus
    #[inline]
    pub fn prime(&self) -> u32 {
        self.prime
    }

    /// Get generation counter (Q34 audit)
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Increment generation (Q34 audit, call when re-uploading to GPU)
    pub fn increment_generation(&self) {
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Compute hash value for testing (CPU reference)
    ///
    /// Matches GPU computation: h(x) = (a*x + b) mod prime
    ///
    /// # Use Case
    ///
    /// - Unit tests verifying GPU output matches CPU reference
    /// - Property tests checking hash quality
    ///
    /// # Performance
    ///
    /// - Per-hash: ~5ns (multiply-add-mod on modern CPU)
    /// - For testing only, not production path
    #[inline]
    pub fn hash_token(&self, token: u32, permutation: usize) -> u32 {
        let a = self.a[permutation] as u64;
        let b = self.b[permutation] as u64;
        let token = token as u64;
        let prime = self.prime as u64;

        // h = (a * token + b) % prime
        ((a.wrapping_mul(token).wrapping_add(b)) % prime) as u32
    }

    /// Compute full MinHash signature for document (CPU reference)
    ///
    /// Used for testing/validation only.
    ///
    /// # Performance
    ///
    /// - Per-doc: ~640ns (128 permutations × 5ns per hash)
    /// - 1000× slower than GPU (GPU is 6-24× faster than current implementation)
    pub fn compute_signature_cpu(&self, tokens: &[u32]) -> [u16; NUM_PERMUTATIONS] {
        let mut signature = [u16::MAX; NUM_PERMUTATIONS];

        for &token in tokens {
            for perm in 0..NUM_PERMUTATIONS {
                let hash = self.hash_token(token, perm);
                // Truncate to u16 (lower 16 bits)
                let hash_u16 = (hash & 0xFFFF) as u16;
                signature[perm] = signature[perm].min(hash_u16);
            }
        }

        signature
    }
}

// SAFETY: FedHashParamsCapsule is Send + Sync because:
// - All arrays are immutable after creation (no interior mutability except generation)
// - AtomicU64 is Send + Sync
// - No raw pointers or unsafe access
unsafe impl Send for FedHashParamsCapsule {}
unsafe impl Sync for FedHashParamsCapsule {}

/// Simple SplitMix64 RNG for parameter generation
///
/// High-quality, fast PRNG with 2^64 period.
/// Reference: https://prng.di.unimi.it/splitmix64.c
///
/// # ASSUM: RNG Quality
///
/// - `#ASSUME_SPLITMIX64_QUALITY`: SplitMix64 provides statistically independent outputs
/// - `#VERIFY_SPLITMIX64_QUALITY`: Standard algorithm, widely tested, passed BigCrush
/// - `#ASSUME_SEED_UNIQUENESS`: Caller provides unique seed per instance
/// - `#VERIFY_SEED_UNIQUENESS`: Documented in FedHashParamsCapsule::generate()
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Create new RNG from seed
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Generate next random u64
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fed_params_generation() {
        let params = FedHashParamsCapsule::generate(12345);

        // Verify all a values are in range [1, prime-1]
        for i in 0..NUM_PERMUTATIONS {
            let a = params.a(i);
            assert!(a > 0, "a[{}] must be > 0", i);
            assert!(a < HASH_PRIME, "a[{}] must be < prime", i);
        }

        // Verify all b values are in range [0, prime-1]
        for i in 0..NUM_PERMUTATIONS {
            let b = params.b(i);
            assert!(b < HASH_PRIME, "b[{}] must be < prime", i);
        }

        // Verify prime is correct
        assert_eq!(params.prime(), HASH_PRIME);
    }

    #[test]
    fn test_fed_params_deterministic() {
        // Same seed → same parameters
        let params1 = FedHashParamsCapsule::generate(42);
        let params2 = FedHashParamsCapsule::generate(42);

        for i in 0..NUM_PERMUTATIONS {
            assert_eq!(params1.a(i), params2.a(i), "a[{}] mismatch", i);
            assert_eq!(params1.b(i), params2.b(i), "b[{}] mismatch", i);
        }
    }

    #[test]
    fn test_fed_params_different_seeds() {
        // Different seeds → different parameters
        let params1 = FedHashParamsCapsule::generate(100);
        let params2 = FedHashParamsCapsule::generate(200);

        let mut diffs = 0;
        for i in 0..NUM_PERMUTATIONS {
            if params1.a(i) != params2.a(i) || params1.b(i) != params2.b(i) {
                diffs += 1;
            }
        }

        // At least 90% of parameters should differ
        assert!(
            diffs >= 115,
            "Only {} out of 128 parameters differ (expected ≥115)",
            diffs
        );
    }

    #[test]
    fn test_fed_hash_token_cpu() {
        let params = FedHashParamsCapsule::generate(999);
        let token = 12345u32;

        // Compute hash for permutation 0
        let hash = params.hash_token(token, 0);

        // Verify result is in valid range [0, prime)
        assert!(hash < HASH_PRIME, "Hash must be < prime");

        // Verify determinism
        let hash2 = params.hash_token(token, 0);
        assert_eq!(hash, hash2, "Hash must be deterministic");
    }

    #[test]
    fn test_fed_compute_signature_cpu() {
        let params = FedHashParamsCapsule::generate(777);
        let tokens = vec![100u32, 200, 300, 100, 400]; // Note: 100 repeated

        let signature = params.compute_signature_cpu(&tokens);

        // Verify signature has 128 values
        assert_eq!(signature.len(), NUM_PERMUTATIONS);

        // Verify all values are < u16::MAX (at least one token was hashed)
        for (i, &val) in signature.iter().enumerate() {
            assert!(
                val < u16::MAX,
                "Signature[{}] should be < u16::MAX (at least one hash computed)",
                i
            );
        }

        // Verify determinism
        let signature2 = params.compute_signature_cpu(&tokens);
        assert_eq!(signature, signature2, "Signature must be deterministic");
    }

    #[test]
    fn test_fed_params_to_gpu_buffer() {
        let params = FedHashParamsCapsule::generate(555);
        let buffer = params.to_gpu_buffer();

        // Verify buffer size: 512 (a) + 512 (b) + 4 (prime) + 12 (padding) = 1040
        assert_eq!(buffer.len(), 1040, "Buffer size must be 1040 bytes");

        // Verify we can reconstruct a[0] from buffer
        let a0_bytes = [buffer[0], buffer[1], buffer[2], buffer[3]];
        let a0 = u32::from_le_bytes(a0_bytes);
        assert_eq!(a0, params.a(0), "Buffer encoding must match a[0]");

        // Verify we can reconstruct b[0] from buffer
        let b0_bytes = [buffer[512], buffer[513], buffer[514], buffer[515]];
        let b0 = u32::from_le_bytes(b0_bytes);
        assert_eq!(b0, params.b(0), "Buffer encoding must match b[0]");

        // Verify prime encoding
        let prime_bytes = [buffer[1024], buffer[1025], buffer[1026], buffer[1027]];
        let prime = u32::from_le_bytes(prime_bytes);
        assert_eq!(prime, HASH_PRIME, "Buffer encoding must match prime");
    }

    #[test]
    fn test_fed_params_generation_counter() {
        let params = FedHashParamsCapsule::generate(123);

        assert_eq!(params.generation(), 0);

        params.increment_generation();
        assert_eq!(params.generation(), 1);

        params.increment_generation();
        assert_eq!(params.generation(), 2);
    }

    #[test]
    fn test_splitmix64_rng() {
        let mut rng = SplitMix64::new(42);

        // Generate 10 values
        let vals: Vec<u64> = (0..10).map(|_| rng.next()).collect();

        // Verify no duplicates (probability ~0 for 10 values in 2^64 space)
        for i in 0..vals.len() {
            for j in (i + 1)..vals.len() {
                assert_ne!(vals[i], vals[j], "RNG should not produce duplicates");
            }
        }
    }

    #[test]
    fn test_splitmix64_determinism() {
        let mut rng1 = SplitMix64::new(999);
        let mut rng2 = SplitMix64::new(999);

        for _ in 0..100 {
            assert_eq!(rng1.next(), rng2.next(), "RNG must be deterministic");
        }
    }
}
