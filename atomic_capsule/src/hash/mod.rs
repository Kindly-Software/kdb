//! Hash function implementations and wrappers for capsule verification

pub mod atomic;
pub mod const_capsule;
pub mod const_hash;
#[cfg(feature = "keyed-hashing")] // Requires std (uses Box for key management)
pub mod keyed;
pub mod simd_hash;

// P2.2: SIMD batch SipHash for distributed cache
#[cfg(feature = "distributed")]
pub mod batch_siphash;

// Random SipHash keys for DoS-resistant hashing (cache feature)
#[cfg(feature = "cache")]
pub mod random_siphash;

// P2: SIMD Hash Capsule (8-lane parallel hashing)
#[cfg(feature = "simd-hashing")]
pub mod simd_hash_capsule;

// P2.4: SIMD MurmurHash3 (4× speedup for Bloom filters)
#[cfg(feature = "portable_simd")]
pub mod murmur3_simd;

// Re-export commonly used items
pub use atomic::{AtomicHash256, AtomicHash64};
pub use const_capsule::ConstHashCapsule;
pub use const_hash::{const_fast_hash, const_fast_hash_fields, ConstHashable};
pub use simd_hash::{best_hash, scalar_fast_hash};

#[cfg(feature = "simd-hashing")]
pub use simd_hash::simd_fast_hash_multi;

// P2.2: Batch SipHash exports
#[cfg(feature = "distributed")]
pub use batch_siphash::{
    batch_siphash_4_fixed, batch_siphash_8_fixed, batch_siphash_keys, siphash_single,
};

// Random SipHash exports (DoS-resistant hashing)
#[cfg(feature = "cache")]
pub use random_siphash::{compute_hash_random, random_siphash_keys};

// P2: SIMD Hash Capsule exports
#[cfg(feature = "simd-hashing")]
pub use simd_hash_capsule::{scalar_hash_single, simd_hash_8_keys, SimdHashCapsule};

// P2.4: SIMD MurmurHash3 exports
#[cfg(feature = "portable_simd")]
pub use murmur3_simd::{murmur3_hash_scalar, murmur3_hash_simd_x4, murmur3_hash_simd_x8};
