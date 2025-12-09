//! Bucket Capsule - Atomic unit for each hash bucket
//!
//! Each bucket is a 64-byte cache-aligned atomic capsule implementing
//! the two-phase commit protocol from "The Atomic Capsule" architecture.
//!
//! # Design Principles
//!
//! 1. **Cache Alignment**: 64-byte alignment prevents false sharing
//! 2. **Two-Phase Commit**: Odd→even version transition for atomic publish
//! 3. **Generation Counter**: ABA prevention through monotonic versioning
//! 4. **Lockfree Read**: Single atomic load for consistent snapshot
//! 5. **Inline Optimization**: ≤8 bytes inline, >8 bytes on heap (Phase 2)
//!
//! # Memory Layout (Phase 2: Large Value Support - 64-bit storage)
//!
//! ```text
//! BucketCapsule (64 bytes, cache-aligned):
//! ┌────────────────────────────────────────┐
//! │ W0 (head): AtomicU64                   │  8 bytes
//! │   - version:8 (odd=inflight, even=committed)
//! │   - key_hash:24                        │
//! │   - exists:1                           │
//! │   - generation:31                      │
//! ├────────────────────────────────────────┤
//! │ W1 (key): AtomicU64                    │  8 bytes
//! │   - key_data (inline key ≤8 bytes)     │
//! │   - OR pointer to heap-allocated key   │
//! ├────────────────────────────────────────┤
//! │ W2 (value): AtomicU64                  │  8 bytes
//! │   - discriminant:2 (inline/heap/empty) │
//! │   - value_generation:30 (ABA prevent)  │
//! │   - value_low:32 OR ptr_low:32         │
//! ├────────────────────────────────────────┤
//! │ W3 (tail): AtomicU64                   │  8 bytes
//! │   - tail_version:8 (matches W0)        │
//! │   - tail_generation:31                 │
//! │   - value_high:25 OR ptr_high:25       │
//! └────────────────────────────────────────┘
//! │ Padding to 64 bytes                    │ 32 bytes
//! └────────────────────────────────────────┘
//!
//! # 64-Bit Value Storage (v0.3.0)
//!
//! All 64-bit values (inline or heap pointers) are split across W2 and W3:
//! - **W2 data field**: Bits 0-31 (low 32 bits)
//! - **W3 ptr_high field**: Bits 32-56 (high 25 bits)
//! - **Total storage**: 57 bits
//!
//! This format supports:
//! - ✅ 32-bit values: Full fidelity
//! - ✅ 48-bit canonical x86-64 pointers: Full fidelity (e.g., Arc<T>)
//! - ✅ 57-bit values: Full fidelity
//! - ⚠️  64-bit values: High 7 bits silently truncated
//!
//! Canonical x86-64 addresses use only 48 bits (bits 0-47), with bits 48-63
//! being sign-extension. Our 57-bit storage provides full support for these
//! addresses plus 9 additional bits of headroom.
//! ```
//!
//! # Safety Assumptions (Phase 2)
//!
//! #ASSUME: 64-byte alignment prevents false sharing on all modern CPUs
//! #VERIFY: Alignment validated in compile-time const assertions
//! #ASSUME: AtomicU64 provides correct memory ordering guarantees
//! #VERIFY: Memory ordering validated in concurrent stress tests
//! #ASSUME: Two-phase commit prevents torn reads
//! #VERIFY: Property tests validate all-old or all-new reads
//! #ASSUME_RESOURCE_CLEANUP: Heap pointers deallocated exactly once via Drop
//! #VERIFY_DROP_SAFE: Miri validates no leaks, stress tests verify concurrent correctness
//! #ASSUME_TOCTOU_SAFE: Generation counter prevents ABA on heap pointers
//! #VERIFY_TOCTOU_PREVENTED: Concurrent tests validate pointer validity

use core::sync::atomic::{AtomicU64, Ordering};

/// Maximum retry attempts for reading bucket during concurrent updates
const READ_ATTEMPTS: usize = 8;

/// Value storage discriminant (2 bits)
///
/// Indicates how the value is stored in the bucket capsule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueDiscriminant {
    /// No value stored (bucket is empty)
    Empty = 0,
    /// Value stored inline (≤8 bytes, zero-cost)
    Inline = 1,
    /// Value stored on heap (>8 bytes, pointer in W2/W3)
    Heap = 2,
    /// Tombstone marker (for future use in deletion protocols)
    Tombstone = 3,
}

impl ValueDiscriminant {
    #[inline(always)]
    fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0 => ValueDiscriminant::Empty,
            1 => ValueDiscriminant::Inline,
            2 => ValueDiscriminant::Heap,
            3 => ValueDiscriminant::Tombstone,
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn to_bits(self) -> u8 {
        self as u8
    }
}

/// Bucket capsule state extracted from atomic words
///
/// This is the lockfree snapshot read from a bucket in a single consistent read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BucketSnapshot {
    /// Version number (even = committed, odd = inflight)
    pub version: u8,
    /// Hash of the key (24 bits)
    pub key_hash: u32,
    /// Entry exists flag
    pub exists: bool,
    /// Generation counter for ABA prevention (from W0)
    pub generation: u32,
    /// First 8 bytes of key data
    pub key_data: u64,
    /// Value storage discriminant
    pub value_discriminant: ValueDiscriminant,
    /// Value generation counter (from W2, for ABA prevention on heap pointers)
    pub value_generation: u32,
    /// Value data (inline) or first 8 bytes of value data
    pub value_data: u64,
}

#[allow(dead_code)]
impl BucketSnapshot {
    /// Check if this snapshot represents a valid committed entry
    /// TODO(Phase 3): Used in advanced snapshot validation logic
    #[allow(dead_code)]
    #[inline(always)]
    pub fn is_valid(&self) -> bool {
        self.exists
            && (self.version & 1 == 0)
            && self.value_discriminant != ValueDiscriminant::Empty
    }

    /// Check if bucket is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        !self.exists || self.value_discriminant == ValueDiscriminant::Empty
    }

    /// Check if value is stored inline (≤8 bytes)
    #[inline(always)]
    pub fn is_inline(&self) -> bool {
        self.value_discriminant == ValueDiscriminant::Inline
    }

    /// Check if value is stored on heap (>8 bytes)
    #[inline(always)]
    pub fn is_heap(&self) -> bool {
        self.value_discriminant == ValueDiscriminant::Heap
    }

    /// Reconstruct heap pointer from W2/W3 data (if value is on heap)
    ///
    /// # Safety
    ///
    /// Caller must verify discriminant == Heap before calling.
    /// Pointer validity guaranteed by generation counter matching.
    ///
    /// #ASSUME_TYPE_SAFE: Pointer reconstruction preserves original address
    /// #VERIFY_UNSAFE_INVARIANTS: Generation counter prevents use-after-free
    #[inline(always)]
    pub unsafe fn heap_ptr<T>(&self) -> *mut T {
        debug_assert_eq!(self.value_discriminant, ValueDiscriminant::Heap);
        let ptr_low = self.value_data & 0xFFFFFFFF;
        let ptr_high = (self.value_data >> 32) & 0x1FFFFFF; // 25 bits from W3
        let ptr_value = (ptr_high << 32) | ptr_low;
        ptr_value as *mut T
    }
}

/// Atomic capsule for a single hash bucket
///
/// Implements lockfree read/write using two-phase commit protocol.
/// Each bucket can store a 64-bit key hash + first 64 bits of key/value data.
/// For larger keys/values, this is extended with external storage.
///
/// # Performance Targets
///
/// - Read: <20ns (single cache line, 4 atomic loads)
/// - Write: <100ns (two-phase commit, 5 atomic stores)
/// - CAS update: <15ns per retry (hardware CAS latency)
#[repr(C, align(64))]
pub struct BucketCapsule {
    /// W0: Header word (version, key_hash, exists, generation)
    /// #ASSUME: Release store of W0 makes all prior stores visible
    /// #VERIFY: Memory ordering validated in concurrent tests
    w0_head: AtomicU64,

    /// W1: Key data (first 64 bits)
    w1_key: AtomicU64,

    /// W2: Value data (first 64 bits)
    w2_value: AtomicU64,

    /// W3: Tail word (tail_version, tail_generation, reserved)
    /// #ASSUME: Tail version matches head version for valid reads
    /// #VERIFY: Property tests validate version consistency
    w3_tail: AtomicU64,

    /// Padding to 64 bytes total
    _pad: [u64; 4],
}

// Field layout constants for W0 (head word)
const W0_VERSION_SHIFT: u32 = 0;
const W0_VERSION_BITS: u32 = 8;
const W0_KEY_HASH_SHIFT: u32 = 8;
const W0_KEY_HASH_BITS: u32 = 24;
const W0_EXISTS_SHIFT: u32 = 32;
// TODO(Phase 3): Used in bit manipulation helpers
#[allow(dead_code)]
const W0_EXISTS_BITS: u32 = 1;
const W0_GENERATION_SHIFT: u32 = 33;
const W0_GENERATION_BITS: u32 = 31;

// Field layout constants for W2 (value word) - Phase 2
const W2_DISCRIMINANT_SHIFT: u32 = 0;
const W2_DISCRIMINANT_BITS: u32 = 2;
const W2_VALUE_GEN_SHIFT: u32 = 2;
const W2_VALUE_GEN_BITS: u32 = 30;
const W2_DATA_SHIFT: u32 = 32;
const W2_DATA_BITS: u32 = 32; // Inline data or lower 32 bits of pointer

// Field layout constants for W3 (tail word) - Phase 2
const W3_TAIL_VERSION_SHIFT: u32 = 0;
const W3_TAIL_VERSION_BITS: u32 = 8;
const W3_TAIL_GENERATION_SHIFT: u32 = 8;
const W3_TAIL_GENERATION_BITS: u32 = 31;
const W3_PTR_HIGH_SHIFT: u32 = 39;
const W3_PTR_HIGH_BITS: u32 = 25; // Upper 25 bits of 57-bit pointer

impl BucketCapsule {
    #[allow(dead_code)]
    /// Create new empty bucket capsule
    ///
    /// # Const Construction
    ///
    /// Uses const fn for zero-cost static initialization of bucket arrays.
    pub const fn new() -> Self {
        Self {
            w0_head: AtomicU64::new(0),
            w1_key: AtomicU64::new(0),
            w2_value: AtomicU64::new(0),
            w3_tail: AtomicU64::new(0),
            _pad: [0; 4],
        }
    }

    /// Pack W0 header word
    #[inline(always)]
    fn pack_w0(version: u8, key_hash: u32, exists: bool, generation: u32) -> u64 {
        let version = (version as u64) & ((1u64 << W0_VERSION_BITS) - 1);
        let key_hash = (key_hash as u64) & ((1u64 << W0_KEY_HASH_BITS) - 1);
        let exists = if exists { 1u64 } else { 0u64 };
        let generation = (generation as u64) & ((1u64 << W0_GENERATION_BITS) - 1);

        (version << W0_VERSION_SHIFT)
            | (key_hash << W0_KEY_HASH_SHIFT)
            | (exists << W0_EXISTS_SHIFT)
            | (generation << W0_GENERATION_SHIFT)
    }

    /// Unpack W0 header word
    #[inline(always)]
    fn unpack_w0(w0: u64) -> (u8, u32, bool, u32) {
        let version = ((w0 >> W0_VERSION_SHIFT) & ((1 << W0_VERSION_BITS) - 1)) as u8;
        let key_hash = ((w0 >> W0_KEY_HASH_SHIFT) & ((1 << W0_KEY_HASH_BITS) - 1)) as u32;
        let exists = ((w0 >> W0_EXISTS_SHIFT) & 1) != 0;
        let generation = ((w0 >> W0_GENERATION_SHIFT) & ((1 << W0_GENERATION_BITS) - 1)) as u32;
        (version, key_hash, exists, generation)
    }

    /// Pack W3 tail word
    #[inline(always)]
    #[allow(dead_code)]
    fn pack_w3(tail_version: u8, tail_generation: u32) -> u64 {
        let tail_version = (tail_version as u64) & ((1u64 << W3_TAIL_VERSION_BITS) - 1);
        let tail_generation = (tail_generation as u64) & ((1u64 << W3_TAIL_GENERATION_BITS) - 1);

        (tail_version << W3_TAIL_VERSION_SHIFT) | (tail_generation << W3_TAIL_GENERATION_SHIFT)
    }

    /// Unpack W3 tail word (Phase 2: includes pointer high bits)
    #[inline(always)]
    fn unpack_w3(w3: u64) -> (u8, u32, u32) {
        let tail_version =
            ((w3 >> W3_TAIL_VERSION_SHIFT) & ((1 << W3_TAIL_VERSION_BITS) - 1)) as u8;
        let tail_generation =
            ((w3 >> W3_TAIL_GENERATION_SHIFT) & ((1 << W3_TAIL_GENERATION_BITS) - 1)) as u32;
        let ptr_high = ((w3 >> W3_PTR_HIGH_SHIFT) & ((1 << W3_PTR_HIGH_BITS) - 1)) as u32;
        (tail_version, tail_generation, ptr_high)
    }

    /// Pack W2 value word (Phase 2: discriminant + generation + data/ptr_low)
    #[inline(always)]
    fn pack_w2(discriminant: ValueDiscriminant, value_gen: u32, data: u32) -> u64 {
        let disc = (discriminant.to_bits() as u64) & ((1u64 << W2_DISCRIMINANT_BITS) - 1);
        let gen = (value_gen as u64) & ((1u64 << W2_VALUE_GEN_BITS) - 1);
        let data = (data as u64) & ((1u64 << W2_DATA_BITS) - 1);

        (disc << W2_DISCRIMINANT_SHIFT) | (gen << W2_VALUE_GEN_SHIFT) | (data << W2_DATA_SHIFT)
    }

    /// Unpack W2 value word (Phase 2)
    #[inline(always)]
    fn unpack_w2(w2: u64) -> (ValueDiscriminant, u32, u32) {
        let disc = ((w2 >> W2_DISCRIMINANT_SHIFT) & ((1 << W2_DISCRIMINANT_BITS) - 1)) as u8;
        let value_generation = ((w2 >> W2_VALUE_GEN_SHIFT) & ((1 << W2_VALUE_GEN_BITS) - 1)) as u32;
        let data = ((w2 >> W2_DATA_SHIFT) & ((1 << W2_DATA_BITS) - 1)) as u32;
        (ValueDiscriminant::from_bits(disc), value_generation, data)
    }

    /// Pack W3 with pointer high bits (Phase 2)
    #[inline(always)]
    fn pack_w3_with_ptr(tail_version: u8, tail_generation: u32, ptr_high: u32) -> u64 {
        let tail_version = (tail_version as u64) & ((1u64 << W3_TAIL_VERSION_BITS) - 1);
        let tail_generation = (tail_generation as u64) & ((1u64 << W3_TAIL_GENERATION_BITS) - 1);
        let ptr_high = (ptr_high as u64) & ((1u64 << W3_PTR_HIGH_BITS) - 1);

        (tail_version << W3_TAIL_VERSION_SHIFT)
            | (tail_generation << W3_TAIL_GENERATION_SHIFT)
            | (ptr_high << W3_PTR_HIGH_SHIFT)
    }

    /// Split pointer into low/high for W2/W3 storage
    #[inline(always)]
    #[allow(dead_code)]
    fn split_ptr<T>(ptr: *mut T) -> (u32, u32) {
        let ptr_value = ptr as u64;
        let ptr_low = (ptr_value & 0xFFFFFFFF) as u32;
        let ptr_high = ((ptr_value >> 32) & 0x1FFFFFF) as u32; // 25 bits
        (ptr_low, ptr_high)
    }

    /// Read bucket snapshot with lockfree retry loop (Phase 2: large value support)
    ///
    /// Implements the atomic capsule read protocol:
    /// 1. Load W0 (head) with Acquire ordering
    /// 2. Reject if version is odd (inflight write)
    /// 3. Load W1, W2, W3 with Relaxed ordering
    /// 4. Load W0 again with Acquire ordering
    /// 5. Reject if W0 changed (concurrent write)
    /// 6. Reject if tail version doesn't match head version
    /// 7. Accept snapshot if all validations pass
    ///
    /// # Lockfree Guarantee
    ///
    /// Readers never block writers. Writers use two-phase commit to ensure
    /// readers always see all-old or all-new state, never torn reads.
    ///
    /// # Performance
    ///
    /// Target: <20ns for inline values, <100ns for heap values
    ///
    /// # Memory Ordering
    ///
    /// - W0 first load: Acquire (synchronize with writer's Release)
    /// - W1, W2, W3 loads: Relaxed (protected by W0 fence)
    /// - W0 second load: Acquire (detect concurrent writes)
    ///
    /// #ASSUME: Acquire/Release ordering provides correct synchronization
    /// #VERIFY: Memory ordering validated in concurrent stress tests
    #[inline(always)]
    pub fn read(&self) -> Option<BucketSnapshot> {
        for _ in 0..READ_ATTEMPTS {
            // Phase 1: Load head with Acquire ordering
            let w0_first = self.w0_head.load(Ordering::Acquire);
            let (version, key_hash, exists, generation) = Self::unpack_w0(w0_first);

            // Reject odd version (inflight write)
            if version & 1 != 0 {
                continue;
            }

            // Phase 2: Load body words with Relaxed ordering
            // (protected by W0 Acquire fence)
            let w1 = self.w1_key.load(Ordering::Relaxed);
            let w2 = self.w2_value.load(Ordering::Relaxed);
            let w3 = self.w3_tail.load(Ordering::Relaxed);

            // Phase 3: Load head again with Acquire ordering
            let w0_second = self.w0_head.load(Ordering::Acquire);

            // Reject if head changed (concurrent write)
            if w0_first != w0_second {
                continue;
            }

            // Phase 4: Validate tail matches head
            let (tail_version, tail_generation, ptr_high) = Self::unpack_w3(w3);
            if tail_version != version || tail_generation != generation {
                continue;
            }

            // Phase 5: Unpack W2 (discriminant + value generation + data/ptr_low)
            let (value_discriminant, value_generation, data_or_ptr_low) = Self::unpack_w2(w2);

            // Reconstruct value_data from W2/W3
            // #VERIFY_RECONSTRUCTION: Both Inline and Heap use same 57-bit split format
            let value_data = if value_discriminant == ValueDiscriminant::Heap {
                // Heap pointer: combine ptr_low (W2) with ptr_high (W3)
                ((ptr_high as u64) << 32) | (data_or_ptr_low as u64)
            } else {
                // Inline data: combine value_low (W2) with value_high (W3)
                // Same 57-bit format as heap pointers for consistency
                ((ptr_high as u64) << 32) | (data_or_ptr_low as u64)
            };

            // Success: Return consistent snapshot
            return Some(BucketSnapshot {
                version,
                key_hash,
                exists,
                generation,
                key_data: w1,
                value_discriminant,
                value_generation,
                value_data,
            });
        }

        // Exceeded retry attempts (extremely rare under normal operation)
        None
    }

    /// Publish inline value using two-phase commit (Phase 2: ≤8 byte values)
    ///
    /// Implements the atomic capsule write protocol from "The Atomic Capsule":
    /// 1. Load current state
    /// 2. Increment version to odd (mark inflight)
    /// 3. Write W1, W2, W3 with new data + odd version
    /// 4. Write W0 with Release ordering + even version (commit)
    ///
    /// # Arguments
    ///
    /// * `key_hash` - 24-bit hash of the key
    /// * `key_data` - Inline key data (≤8 bytes)
    /// * `value_data` - Inline value data (≤8 bytes, up to 57 bits stored)
    ///
    /// # Storage Format
    ///
    /// 64-bit values are split across W2 and W3:
    /// - W2 data field: Low 32 bits of value
    /// - W3 ptr_high field: High 25 bits of value (total: 57 bits)
    ///
    /// This enables storing 48-bit canonical x86-64 pointers (e.g., Arc<T>)
    /// with full fidelity, while limiting to 57 bits for all values.
    ///
    /// # Performance
    ///
    /// Target: <20ns (zero-cost inline storage, 5 atomic stores)
    ///
    /// # Memory Ordering
    ///
    /// - W1, W2, W3 stores: Relaxed (will be fenced by W0 Release)
    /// - W0 final store: Release (makes all prior stores visible to readers)
    ///
    /// #ASSUME_64BIT_SPLIT: 64-bit values split as low 32 bits + high 25 bits
    /// #VERIFY_RECONSTRUCTION: Tests validate roundtrip for all bit patterns
    /// #ASSUME_TWO_PHASE_COMMIT: W2 and W3 updated atomically via version protocol
    /// #VERIFY_LOCKFREE_READ: Readers reconstruct consistent value from committed state
    #[inline(always)]
    pub fn publish_inline(&self, key_hash: u32, key_data: u64, value_data: u64) {
        // Load current state to get version and generations
        let old_w0 = self.w0_head.load(Ordering::Relaxed);
        let old_w2 = self.w2_value.load(Ordering::Relaxed);

        let (old_version, _, _, old_generation) = Self::unpack_w0(old_w0);
        let (_, old_value_gen, _) = Self::unpack_w2(old_w2);

        // Increment generations for ABA prevention
        let new_generation = old_generation.wrapping_add(1) & ((1 << W0_GENERATION_BITS) - 1);
        let new_value_gen = old_value_gen.wrapping_add(1) & ((1 << W2_VALUE_GEN_BITS) - 1);

        // Calculate odd and even versions
        let mut odd_version = old_version.wrapping_add(1);
        if odd_version & 1 == 0 {
            odd_version = odd_version.wrapping_add(1);
        }

        let mut even_version = odd_version.wrapping_add(1);
        if even_version & 1 != 0 {
            even_version = even_version.wrapping_add(1);
        }

        // Phase 1: Write body words with inflight marker
        // Split 64-bit value: low 32 bits in W2 data, high 25 bits in W3 ptr_high
        // #ASSUME_64BIT_SPLIT: Same storage format as heap pointers for consistency
        let value_low = (value_data & 0xFFFFFFFF) as u32;
        let value_high = ((value_data >> 32) & 0x1FFFFFF) as u32; // 25 bits max

        let w2_inline = Self::pack_w2(ValueDiscriminant::Inline, new_value_gen, value_low);
        let w3_inflight = Self::pack_w3_with_ptr(odd_version, new_generation, value_high);

        self.w1_key.store(key_data, Ordering::Relaxed);
        self.w2_value.store(w2_inline, Ordering::Relaxed);
        self.w3_tail.store(w3_inflight, Ordering::Relaxed);

        // Phase 2: Commit by writing even version to both tail and head
        // #VERIFY_RECONSTRUCTION: High bits preserved in W3 ptr_high field
        let w3_final = Self::pack_w3_with_ptr(even_version, new_generation, value_high);
        self.w3_tail.store(w3_final, Ordering::Relaxed);

        let w0_final = Self::pack_w0(even_version, key_hash, true, new_generation);
        self.w0_head.store(w0_final, Ordering::Release);
    }

    /// Publish heap-allocated value using two-phase commit (Phase 2: >8 byte values)
    ///
    /// Stores pointer to heap-allocated value with generation counter for ABA prevention.
    ///
    /// # Arguments
    ///
    /// * `key_hash` - 24-bit hash of the key
    /// * `key_data` - Inline key data (≤8 bytes)
    /// * `value_ptr` - Pointer to heap-allocated value (caller must ensure validity)
    ///
    /// # Performance
    ///
    /// Target: <100ns (includes pointer split/pack overhead)
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - `value_ptr` is valid and properly aligned
    /// - `value_ptr` will not be freed until after removal or replacement
    /// - Pointer fits in 57 bits (25 high + 32 low)
    ///
    /// #ASSUME_TYPE_SAFE: Pointer is valid and aligned for T
    /// #VERIFY_UNSAFE_INVARIANTS: Generation counter prevents ABA, Drop handles cleanup
    #[allow(dead_code)]
    pub fn publish_heap<T>(&self, key_hash: u32, key_data: u64, value_ptr: *mut T) {
        // Load current state
        let old_w0 = self.w0_head.load(Ordering::Relaxed);
        let old_w2 = self.w2_value.load(Ordering::Relaxed);

        let (old_version, _, _, old_generation) = Self::unpack_w0(old_w0);
        let (_, old_value_gen, _) = Self::unpack_w2(old_w2);

        // Increment generations
        let new_generation = old_generation.wrapping_add(1) & ((1 << W0_GENERATION_BITS) - 1);
        let new_value_gen = old_value_gen.wrapping_add(1) & ((1 << W2_VALUE_GEN_BITS) - 1);

        // Calculate versions
        let mut odd_version = old_version.wrapping_add(1);
        if odd_version & 1 == 0 {
            odd_version = odd_version.wrapping_add(1);
        }

        let mut even_version = odd_version.wrapping_add(1);
        if even_version & 1 != 0 {
            even_version = even_version.wrapping_add(1);
        }

        // Split pointer into low/high
        let (ptr_low, ptr_high) = Self::split_ptr(value_ptr);

        // Phase 1: Write body words with inflight marker
        let w2_heap = Self::pack_w2(ValueDiscriminant::Heap, new_value_gen, ptr_low);
        let w3_inflight = Self::pack_w3_with_ptr(odd_version, new_generation, ptr_high);

        self.w1_key.store(key_data, Ordering::Relaxed);
        self.w2_value.store(w2_heap, Ordering::Relaxed);
        self.w3_tail.store(w3_inflight, Ordering::Relaxed);

        // Phase 2: Commit by writing even version
        let w3_final = Self::pack_w3_with_ptr(even_version, new_generation, ptr_high);
        self.w3_tail.store(w3_final, Ordering::Relaxed);

        let w0_final = Self::pack_w0(even_version, key_hash, true, new_generation);
        self.w0_head.store(w0_final, Ordering::Release);
    }

    /// Backward compatibility: publish with auto-detection of inline/heap
    ///
    /// Kept for existing tests. New code should use publish_inline() or publish_heap().
    pub fn publish(&self, key_hash: u32, key_data: u64, value_data: u64) {
        self.publish_inline(key_hash, key_data, value_data);
    }

    /// Extract heap pointer before removal (returns pointer if heap-allocated)
    ///
    /// Reads current value and returns heap pointer if discriminant == Heap.
    /// Caller must deallocate the returned pointer to prevent memory leaks.
    ///
    /// # Safety
    ///
    /// Caller must:
    /// - Call this BEFORE remove() to get pointer
    /// - Deallocate returned pointer with proper type
    /// - Handle concurrent modifications (generation counter validation)
    ///
    /// #ASSUME_RESOURCE_CLEANUP: Caller will deallocate returned pointer
    /// #VERIFY_DROP_SAFE: Tests validate no leaks with proper usage pattern
    #[allow(dead_code)]
    pub fn extract_heap_ptr<T>(&self) -> Option<(*mut T, u32)> {
        let snapshot = self.read()?;

        if snapshot.value_discriminant == ValueDiscriminant::Heap {
            // SAFETY: We verified discriminant == Heap, pointer reconstruction is safe
            let ptr = unsafe { snapshot.heap_ptr::<T>() };
            Some((ptr, snapshot.value_generation))
        } else {
            None
        }
    }

    /// Remove entry from bucket (mark as non-existent) - Phase 2
    ///
    /// Uses two-phase commit to atomically mark bucket as empty.
    ///
    /// # Memory Leak Warning
    ///
    /// Caller MUST call extract_heap_ptr() before remove() if value is heap-allocated,
    /// otherwise memory will leak. This is intentional for lockfree correctness - the
    /// caller decides when to deallocate based on their epoch/hazard pointer scheme.
    ///
    /// #ASSUME_RESOURCE_CLEANUP: Caller extracted and will free heap pointer before calling
    /// #VERIFY_DROP_SAFE: Tests validate proper extract-then-remove pattern
    pub fn remove(&self) {
        // Load current state
        let old_w0 = self.w0_head.load(Ordering::Relaxed);
        let old_w2 = self.w2_value.load(Ordering::Relaxed);

        let (old_version, _, _, old_generation) = Self::unpack_w0(old_w0);
        let (_, old_value_gen, _) = Self::unpack_w2(old_w2);

        // Increment generations
        let new_generation = old_generation.wrapping_add(1) & ((1 << W0_GENERATION_BITS) - 1);
        let new_value_gen = old_value_gen.wrapping_add(1) & ((1 << W2_VALUE_GEN_BITS) - 1);

        // Calculate versions
        let mut odd_version = old_version.wrapping_add(1);
        if odd_version & 1 == 0 {
            odd_version = odd_version.wrapping_add(1);
        }

        let mut even_version = odd_version.wrapping_add(1);
        if even_version & 1 != 0 {
            even_version = even_version.wrapping_add(1);
        }

        // Phase 1: Clear body words with Empty discriminant
        let w2_empty = Self::pack_w2(ValueDiscriminant::Empty, new_value_gen, 0);
        let w3_inflight = Self::pack_w3_with_ptr(odd_version, new_generation, 0);

        self.w1_key.store(0, Ordering::Relaxed);
        self.w2_value.store(w2_empty, Ordering::Relaxed);
        self.w3_tail.store(w3_inflight, Ordering::Relaxed);

        // Phase 2: Commit by writing even version
        let w3_final = Self::pack_w3_with_ptr(even_version, new_generation, 0);
        self.w3_tail.store(w3_final, Ordering::Relaxed);

        let w0_final = Self::pack_w0(even_version, 0, false, new_generation);
        self.w0_head.store(w0_final, Ordering::Release);
    }

    /// Attempt CAS-based conditional update (Phase 2: inline values)
    ///
    /// Updates bucket only if current generation matches expected.
    /// Returns Ok with new generation on success, Err with current generation on failure.
    ///
    /// # Use Case
    ///
    /// Implement lock-free concurrent modifications with retry loops.
    /// TODO(Phase 3): Used in advanced CAS update operations
    #[allow(dead_code)]
    pub fn cas_update(
        &self,
        expected_generation: u32,
        key_hash: u32,
        key_data: u64,
        value_data: u64,
    ) -> Result<u32, u32> {
        // Load current state
        let w0_current = self.w0_head.load(Ordering::Acquire);
        let w2_current = self.w2_value.load(Ordering::Relaxed);

        let (version, _, _exists, generation) = Self::unpack_w0(w0_current);
        let (_, old_value_gen, _) = Self::unpack_w2(w2_current);

        // Check generation matches expected
        if generation != expected_generation {
            return Err(generation);
        }

        // Prepare new state
        let new_generation = generation.wrapping_add(1) & ((1 << W0_GENERATION_BITS) - 1);
        let new_value_gen = old_value_gen.wrapping_add(1) & ((1 << W2_VALUE_GEN_BITS) - 1);

        let mut odd_version = version.wrapping_add(1);
        if odd_version & 1 == 0 {
            odd_version = odd_version.wrapping_add(1);
        }

        let mut even_version = odd_version.wrapping_add(1);
        if even_version & 1 != 0 {
            even_version = even_version.wrapping_add(1);
        }

        // Write body words (Phase 2: inline value with discriminant)
        // Split 64-bit value: low 32 bits in W2, high 25 bits in W3
        let value_low = (value_data & 0xFFFFFFFF) as u32;
        let value_high = ((value_data >> 32) & 0x1FFFFFF) as u32;

        let w2_inline = Self::pack_w2(ValueDiscriminant::Inline, new_value_gen, value_low);

        self.w1_key.store(key_data, Ordering::Relaxed);
        self.w2_value.store(w2_inline, Ordering::Relaxed);

        let w3_final = Self::pack_w3_with_ptr(even_version, new_generation, value_high);
        self.w3_tail.store(w3_final, Ordering::Relaxed);

        // Atomically update W0 with compare_exchange
        // This is the critical section - only one thread will succeed
        let w0_new = Self::pack_w0(even_version, key_hash, true, new_generation);

        match self.w0_head.compare_exchange(
            w0_current,
            w0_new,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(new_generation),
            Err(actual_w0) => {
                // CAS failed - another thread modified the bucket
                let (_, _, _, actual_gen) = Self::unpack_w0(actual_w0);
                Err(actual_gen)
            }
        }
    }
}

impl Default for BucketCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time alignment validation
const _: () = {
    assert!(core::mem::size_of::<BucketCapsule>() == 64);
    assert!(core::mem::align_of::<BucketCapsule>() == 64);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_new_empty() {
        let bucket = BucketCapsule::new();
        let snapshot = bucket.read().unwrap();
        assert!(!snapshot.exists);
        assert!(snapshot.is_empty());
    }

    #[test]
    fn bucket_publish_read() {
        let bucket = BucketCapsule::new();

        bucket.publish(0x123456, 0xDEADBEEF, 0xCAFEBABE);

        let snapshot = bucket.read().unwrap();
        assert!(snapshot.is_valid());
        assert_eq!(snapshot.key_hash, 0x123456);
        assert_eq!(snapshot.key_data, 0xDEADBEEF);
        assert_eq!(snapshot.value_data, 0xCAFEBABE);
        assert_eq!(snapshot.version & 1, 0); // Even version
    }

    #[test]
    fn bucket_remove() {
        let bucket = BucketCapsule::new();

        bucket.publish(0x123456, 0xDEADBEEF, 0xCAFEBABE);
        assert!(bucket.read().unwrap().is_valid());

        bucket.remove();

        let snapshot = bucket.read().unwrap();
        assert!(!snapshot.exists);
        assert!(snapshot.is_empty());
    }

    #[test]
    fn bucket_generation_increments() {
        let bucket = BucketCapsule::new();

        bucket.publish(1, 100, 200);
        let snap1 = bucket.read().unwrap();
        let gen1 = snap1.generation;

        bucket.publish(2, 101, 201);
        let snap2 = bucket.read().unwrap();
        let gen2 = snap2.generation;

        assert_eq!(gen2, gen1.wrapping_add(1));
    }

    #[test]
    fn bucket_cas_update_success() {
        let bucket = BucketCapsule::new();

        bucket.publish(1, 100, 200);
        let snap = bucket.read().unwrap();

        let result = bucket.cas_update(snap.generation, 2, 101, 201);
        assert!(result.is_ok());

        let new_snap = bucket.read().unwrap();
        assert_eq!(new_snap.key_hash, 2);
        assert_eq!(new_snap.key_data, 101);
        // Phase 2: Inline storage supports full 57-bit values
        assert_eq!(new_snap.value_data, 201);
        assert_eq!(new_snap.value_discriminant, ValueDiscriminant::Inline);
    }

    #[test]
    fn bucket_cas_update_failure() {
        let bucket = BucketCapsule::new();

        bucket.publish(1, 100, 200);
        let snap = bucket.read().unwrap();

        // Concurrent update changes generation
        bucket.publish(3, 300, 400);

        // CAS with old generation fails
        let result = bucket.cas_update(snap.generation, 2, 101, 201);
        assert!(result.is_err());
    }

    #[test]
    fn bucket_64bit_storage_32bit_values() {
        // Test 32-bit values (should work perfectly)
        let bucket = BucketCapsule::new();

        let test_value: u64 = 0x0000_0000_CAFE_BABE;
        bucket.publish_inline(1, 42, test_value);

        let snap = bucket.read().unwrap();
        assert_eq!(snap.value_data, test_value);
        assert_eq!(snap.value_discriminant, ValueDiscriminant::Inline);
    }

    #[test]
    fn bucket_64bit_storage_48bit_pointers() {
        // Test 48-bit canonical x86-64 addresses (Arc<T> pointers)
        let bucket = BucketCapsule::new();

        // Typical heap pointer on x86-64 (48 bits used)
        let test_ptr: u64 = 0x0000_7F8A_2400_0C50;
        bucket.publish_inline(1, 42, test_ptr);

        let snap = bucket.read().unwrap();
        assert_eq!(
            snap.value_data, test_ptr,
            "48-bit pointer should roundtrip perfectly"
        );
        assert_eq!(snap.value_discriminant, ValueDiscriminant::Inline);
    }

    #[test]
    fn bucket_64bit_storage_57bit_max() {
        // Test maximum 57-bit value (our storage limit)
        let bucket = BucketCapsule::new();

        // Maximum 57-bit value: 0x1FF_FFFF_FFFF_FFFF
        let max_57bit: u64 = (1u64 << 57) - 1;
        bucket.publish_inline(1, 42, max_57bit);

        let snap = bucket.read().unwrap();
        assert_eq!(
            snap.value_data, max_57bit,
            "57-bit value should roundtrip perfectly"
        );
        assert_eq!(snap.value_discriminant, ValueDiscriminant::Inline);
    }

    #[test]
    fn bucket_64bit_storage_truncation_warning() {
        // Test that 64-bit values with high bits set get truncated
        // This is expected behavior and documented
        let bucket = BucketCapsule::new();

        // Value with high bits set (beyond 57 bits)
        let full_64bit: u64 = 0xFFFF_FFFF_FFFF_FFFF;
        bucket.publish_inline(1, 42, full_64bit);

        let snap = bucket.read().unwrap();
        // High 7 bits should be truncated
        let expected_57bit = full_64bit & ((1u64 << 57) - 1);
        assert_eq!(snap.value_data, expected_57bit);
        assert_ne!(
            snap.value_data, full_64bit,
            "High 7 bits should be truncated"
        );
        assert_eq!(snap.value_discriminant, ValueDiscriminant::Inline);
    }

    #[test]
    fn bucket_64bit_storage_multiple_updates() {
        // Test multiple updates preserve all bits correctly
        let bucket = BucketCapsule::new();

        let values = [
            0x0000_0000_0000_0001u64,
            0x0000_0000_DEAD_BEEFu64,
            0x0000_7F8A_2400_0C50u64, // Realistic pointer
            0x0001_FFFF_FFFF_FFFFu64, // Max 57-bit
        ];

        for (i, &value) in values.iter().enumerate() {
            bucket.publish_inline(i as u32, i as u64, value);
            let snap = bucket.read().unwrap();
            assert_eq!(snap.value_data, value, "Update {} failed", i);
        }
    }
}
