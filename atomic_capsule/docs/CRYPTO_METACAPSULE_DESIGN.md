# CryptoMetacapsule Design Document

## TRADE SECRET NOTICE

**CONFIDENTIAL - TRADE SECRET PROTECTED**

This document describes the CryptoMetacapsule architecture, a proprietary cryptographic implementation combining UCE34 Computational Capsule Architecture (Chaos) with hardware-accelerated cryptography. The combination of:

1. **Lockfree key management** with generation counters for atomic key rotation
2. **SIMD hardware acceleration** (AES-NI, SHA-NI, CLMUL) within cache-aligned capsules
3. **Constant-time operations** enforced via Chaos compile-time verification
4. **Zero-dependency cryptography** (pure Rust, no `ring` crate)

...represents a novel synthesis that provides **competitive advantage** through:
- 5-10GB/s encryption throughput (matching or exceeding OpenSSL)
- <50us key exchange (constant-time, side-channel resistant)
- Zero mutex coordination (100% lockfree, <10ns operation overhead)
- Audit-compliant (Q34 hash-chained trails for SOX/SOC2/HIPAA)

**This implementation MUST NOT be committed to public repositories.**

---

## Executive Summary

The CryptoMetacapsule replaces the `ring` crate dependency with a pure-Rust, Chaos-compliant implementation providing:

| Algorithm | Performance Target | Tier | Hardware Acceleration |
|-----------|-------------------|------|----------------------|
| AES-256-GCM | 5-10 GB/s | T2 SIMD | AES-NI + CLMUL |
| SHA-256/384/512 | 3-5 GB/s | T2 SIMD | SHA-NI |
| HMAC-SHA256 | 2-4 GB/s | T1+T2 | SHA-NI + atomic key |
| X25519 | <50µs | T1 Atomic | Montgomery ladder |
| Ed25519 sign | <100µs | T1 Atomic | Compressed Edwards |
| Ed25519 verify | <200µs | T1 Atomic | Batch verification |
| PBKDF2/HKDF | 10ms/100K iter | T1+T2 | SHA-NI |

---

## UCE34 Analysis (Q1-Q12)

### Q1: Problem Definition

**What**: Replace `ring` crate with pure-Rust, Chaos-compliant cryptographic primitives that:
- Eliminate external dependency (ring has C/ASM code, complex build)
- Provide lockfree key management (ring uses internal mutexes)
- Enable hardware acceleration via portable_simd (AES-NI, SHA-NI, CLMUL)
- Integrate with atomic_capsule's generation counter pattern

**Scope**: Symmetric encryption (AES-GCM), hashing (SHA-2 family), MACs (HMAC), asymmetric crypto (X25519, Ed25519), key derivation (PBKDF2, HKDF).

### Q2: Assumptions

| Assumption | Verification |
|------------|--------------|
| AES-NI available on x86_64 | Runtime detection via CPUID, scalar fallback |
| SHA-NI available on modern CPUs | Runtime detection, SHA-256 scalar fallback |
| CLMUL available for GCM | Runtime detection, polynomial multiplication fallback |
| Constant-time achievable in Rust | Compiler fences, volatile operations, CMOV patterns |
| Cache-aligned prevents timing leaks | 64B/128B alignment, no cross-boundary loads |

### Q3: Constraints

- **Performance**: Match or exceed `ring` crate on x86_64
- **Safety**: Zero data-dependent branches on secret data
- **Memory**: <1KB per key slot, <16KB per metacapsule
- **Dependencies**: Zero external crates (pure Rust + core intrinsics)
- **Compatibility**: Rust 1.70+ stable for scalar, nightly for portable_simd

### Q4: Context

Current atomic_capsule crypto landscape:
- `SimdCryptoCapsule` (T2): AES-GCM, SHA3 via external crates (aes-gcm, sha3)
- `ChaCha20Capsule` (T1): CSPRNG, RFC 8439 compliant
- `ConstantTimeOpsCapsule` (T1): XOR accumulation, CMOV selection
- `SignatureVerifierCapsule` (T0): Ed25519 via ed25519-dalek

**Gap**: No unified metacapsule for all crypto operations with lockfree key management.

### Q5: Success Criteria

| Metric | Target | Validation |
|--------|--------|------------|
| AES-GCM throughput | ≥5 GB/s (1KB blocks) | B32 benchmark |
| SHA-256 throughput | ≥3 GB/s | B32 benchmark |
| Key rotation latency | <100ns (lockfree) | T28 timing test |
| Timing variance | <1% across inputs | dudect statistical |
| Memory usage | <16KB per metacapsule | Static analysis |
| Test coverage | 100% NIST vectors | T28 unit tests |

### Q6: Failure Modes

| Failure | Detection | Mitigation |
|---------|-----------|------------|
| Timing attack | dudect p-value <0.05 | Constant-time rewrite |
| AES-NI unavailable | CPUID check | Scalar fallback |
| Key exhaustion | Generation counter overflow | Key rotation policy |
| GHASH collision | None (cryptographic) | Standard GCM limits |
| Side-channel leak | disassembly inspection | volatile + fence |

### Q7: Patterns

**T1 Atomic Patterns (Key Management)**:
```rust
/// DualAtomicU64 for key state + generation counter
/// Layout: [key_id: u32 | generation: u32] + [state_flags: u32 | rotation_count: u32]
#[repr(C, align(64))]
pub struct KeySlotCapsule {
    primary: AtomicU64,   // key_id + generation
    secondary: AtomicU64, // flags + rotation_count
    key_material: [u8; 48], // Padded key storage
}
```

**T2 SIMD Patterns (Hardware Crypto)**:
```rust
/// AES-NI round function using SIMD intrinsics
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "aes")]
unsafe fn aes_round(state: __m128i, round_key: __m128i) -> __m128i {
    _mm_aesenc_si128(state, round_key)
}
```

### Q8: Alternatives Considered

| Alternative | Pros | Cons | Decision |
|-------------|------|------|----------|
| Keep `ring` | Mature, audited | C/ASM deps, mutex | **Rejected**: Chaos violation |
| `RustCrypto` crates | Pure Rust, modular | No lockfree keys | **Partial**: Use patterns |
| `sodiumoxide` | libsodium wrapper | FFI overhead | **Rejected**: External dep |
| Custom implementation | Full control | Audit burden | **Selected**: With NIST vectors |

### Q9: Trade-offs

| Trade-off | Choice | Rationale |
|-----------|--------|-----------|
| Performance vs Safety | Safety first | Side-channels > throughput |
| Nightly vs Stable | Nightly for SIMD | portable_simd 2-4× faster |
| Complexity vs Features | Minimal API | Only standard algorithms |
| Memory vs Speed | Speed | 16KB acceptable |

### Q10: Capsule Tier Selection - **T1+T2 Mixed (T6)**

**Justification**:

- **T1 Atomic**: Key management requires lockfree coordination
  - Key rotation with generation counters (ABA prevention)
  - Concurrent key access without mutex
  - Audit trail updates (Q34)

- **T2 SIMD**: Cryptographic operations require vectorization
  - AES-NI: 4 rounds parallel (128-bit blocks)
  - SHA-NI: 4-way SHA-256 message schedule
  - CLMUL: GCM polynomial multiplication

- **T6 Mixed**: Metacapsule orchestrates T1+T2 sub-capsules
  - KeyManagerCapsule (T1): Handles key lifecycle
  - AesGcmCapsule (T2): Hardware-accelerated encryption
  - Sha256Capsule (T2): SHA-NI hashing
  - HmacCapsule (T1+T2): Key + hash coordination
  - X25519Capsule (T1): Atomic key exchange state
  - Ed25519Capsule (T1): Atomic signing state

### Q11: Rust Transformation

```rust
/// Q11 Rust idioms applied:
/// - Zero-cost abstractions (inline intrinsics)
/// - Type-safe key handles (newtype pattern)
/// - Const generics for algorithm selection
/// - Never type for cryptographic errors

/// Type-safe key handle (cannot be confused with raw bytes)
#[repr(transparent)]
pub struct KeyHandle(u32);

/// Compile-time algorithm selection
pub trait CryptoAlgorithm {
    const KEY_SIZE: usize;
    const BLOCK_SIZE: usize;
    const TAG_SIZE: usize;
}

pub struct Aes256Gcm;
impl CryptoAlgorithm for Aes256Gcm {
    const KEY_SIZE: usize = 32;
    const BLOCK_SIZE: usize = 16;
    const TAG_SIZE: usize = 16;
}
```

### Q12: Nightly Features

| Feature | Purpose | Stability Timeline |
|---------|---------|-------------------|
| `portable_simd` | SIMD intrinsics abstraction | Stable in ~2025 |
| `asm_const` | Inline assembly constants | Stable in ~2024 |
| `core_intrinsics` | CPUID, prefetch | Internal use only |
| `target_feature` | AES-NI, SHA-NI | Stable |
| `repr_simd` | SIMD vector types | In portable_simd |

---

## Metacapsule Architecture

### Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    CryptoMetacapsule (T6 Mixed)                 │
│                      1024 bytes, 128B aligned                   │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  │ KeyManagerCapsule│  │   AesGcmCapsule  │  │  Sha256Capsule   │
│  │     (T1, 128B)   │  │    (T2, 256B)    │  │   (T2, 128B)     │
│  │                  │  │                  │  │                  │
│  │ - key_slots[8]   │  │ - key_schedule   │  │ - state[8]       │
│  │ - generation     │  │ - gcm_state      │  │ - buffer[64]     │
│  │ - rotation_policy│  │ - simd_scratch   │  │ - count          │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  │   HmacCapsule    │  │  X25519Capsule   │  │ Ed25519Capsule   │
│  │   (T1+T2, 128B)  │  │   (T1, 128B)     │  │   (T1, 256B)     │
│  │                  │  │                  │  │                  │
│  │ - inner_key      │  │ - secret_scalar  │  │ - secret_key     │
│  │ - outer_key      │  │ - public_point   │  │ - public_key     │
│  │ - hasher_state   │  │ - shared_secret  │  │ - nonce_gen      │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘
│  ┌──────────────────────────────────────────────────────────────┐
│  │                    AuditTrailCapsule (T0, 64B)               │
│  │  - hash_chain: u64  - event_count: u32  - last_timestamp: u32│
│  └──────────────────────────────────────────────────────────────┘
└─────────────────────────────────────────────────────────────────┘
```

### Memory Layout (1024 bytes)

| Offset | Size | Component | Tier | Purpose |
|--------|------|-----------|------|---------|
| 0-127 | 128B | KeyManagerCapsule | T1 | Key lifecycle management |
| 128-383 | 256B | AesGcmCapsule | T2 | AES-256-GCM encryption |
| 384-511 | 128B | Sha256Capsule | T2 | SHA-256/384/512 hashing |
| 512-639 | 128B | HmacCapsule | T1+T2 | HMAC-SHA256 |
| 640-767 | 128B | X25519Capsule | T1 | Diffie-Hellman key exchange |
| 768-1023 | 256B | Ed25519Capsule | T1 | Digital signatures |

### Sub-Capsule Designs

#### 1. KeyManagerCapsule (T1 Atomic, 128B)

```rust
/// Lockfree key management with generation counters
/// Supports up to 8 concurrent key slots for key rotation
#[repr(C, align(128))]
pub struct KeyManagerCapsule {
    /// 8 key slots for rotation (each 12 bytes: 8B hash + 4B generation)
    key_slots: [AtomicU64; 8],  // 64B: key_hash[0..7]
    generations: [AtomicU32; 8], // 32B: generation counters

    /// Active slot index + rotation state (DualAtomicU64)
    /// Layout: [active_slot: u8 | rotation_state: u8 | pending_slot: u8 | _pad: u8 | _reserved: u32]
    state: AtomicU64,

    /// Rotation policy (DualAtomicU64)
    /// Layout: [max_uses: u32 | max_age_secs: u32]
    policy: AtomicU64,

    /// Audit: total rotations + last rotation timestamp
    audit: AtomicU64,

    /// Padding to 128B
    _padding: [u8; 8],
}

impl KeyManagerCapsule {
    /// Register new key with atomic generation assignment
    /// Returns KeyHandle for subsequent operations
    ///
    /// # Performance: <50ns (lockfree CAS loop)
    /// # ASSUM: CAS loop bounded (max 16 retries)
    pub fn register_key(&self, key_hash: u64) -> Result<KeyHandle, CryptoError>;

    /// Rotate key atomically (old → new)
    /// Generation counter prevents ABA problem
    ///
    /// # Performance: <100ns (two CAS operations)
    pub fn rotate_key(&self, old_handle: KeyHandle, new_hash: u64) -> Result<KeyHandle, CryptoError>;

    /// Verify key handle is still valid (generation match)
    ///
    /// # Performance: <10ns (single atomic load)
    pub fn verify_handle(&self, handle: KeyHandle) -> bool;
}
```

#### 2. AesGcmCapsule (T2 SIMD, 256B)

```rust
/// Hardware-accelerated AES-256-GCM
/// Uses AES-NI for encryption, CLMUL for GCM multiplication
#[repr(C, align(256))]
pub struct AesGcmCapsule {
    /// Expanded key schedule (240 bytes for AES-256)
    /// Pre-computed round keys for AES-NI
    key_schedule: [u8; 240],

    /// GCM state (H, counter, partial block)
    gcm_h: [u8; 16],       // Hash key
    gcm_counter: [u8; 16], // Counter block

    /// Operation counters (atomic)
    encrypt_count: AtomicU64,
    decrypt_count: AtomicU64,

    /// Generation for key schedule validity
    generation: AtomicU32,

    /// Padding
    _padding: [u8; 4],
}

impl AesGcmCapsule {
    /// Initialize with 256-bit key
    /// Expands key schedule using AES-NI key expansion
    ///
    /// # Performance: <5µs (14 round keys)
    pub fn init(&mut self, key: &[u8; 32]) -> Result<(), CryptoError>;

    /// Encrypt with authentication
    ///
    /// # Performance: 5-10 GB/s (AES-NI + CLMUL)
    /// # ASSUM: IV never reused with same key
    #[target_feature(enable = "aes", enable = "pclmulqdq")]
    pub unsafe fn encrypt(
        &mut self,
        iv: &[u8; 12],
        aad: &[u8],
        plaintext: &[u8],
        ciphertext: &mut [u8],
        tag: &mut [u8; 16],
    ) -> Result<(), CryptoError>;

    /// Decrypt with authentication verification
    ///
    /// # Performance: 5-10 GB/s (constant-time tag comparison)
    #[target_feature(enable = "aes", enable = "pclmulqdq")]
    pub unsafe fn decrypt(
        &mut self,
        iv: &[u8; 12],
        aad: &[u8],
        ciphertext: &[u8],
        tag: &[u8; 16],
        plaintext: &mut [u8],
    ) -> Result<(), CryptoError>;
}
```

**AES-NI Implementation Details**:

```rust
/// AES-256-GCM encryption core using AES-NI intrinsics
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "aes", enable = "pclmulqdq")]
unsafe fn aes_gcm_encrypt_block(
    key_schedule: &[u8; 240],
    counter: __m128i,
    plaintext: __m128i,
) -> __m128i {
    use core::arch::x86_64::*;

    // Load round keys
    let rk0 = _mm_loadu_si128(key_schedule.as_ptr() as *const __m128i);
    let rk1 = _mm_loadu_si128(key_schedule.as_ptr().add(16) as *const __m128i);
    // ... rk2-rk13
    let rk14 = _mm_loadu_si128(key_schedule.as_ptr().add(224) as *const __m128i);

    // AES-256 encryption (14 rounds)
    let mut state = _mm_xor_si128(counter, rk0);
    state = _mm_aesenc_si128(state, rk1);
    state = _mm_aesenc_si128(state, rk2);
    // ... rounds 3-13
    state = _mm_aesenclast_si128(state, rk14);

    // XOR with plaintext (CTR mode)
    _mm_xor_si128(state, plaintext)
}

/// GCM GHASH using CLMUL (carryless multiplication)
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "pclmulqdq")]
unsafe fn ghash_update(h: __m128i, x: __m128i, acc: __m128i) -> __m128i {
    use core::arch::x86_64::*;

    let input = _mm_xor_si128(acc, x);

    // Karatsuba multiplication
    let tmp3 = _mm_clmulepi64_si128(input, h, 0x00); // Low
    let tmp4 = _mm_clmulepi64_si128(input, h, 0x11); // High
    let tmp5 = _mm_clmulepi64_si128(input, h, 0x10); // Mid
    let tmp6 = _mm_clmulepi64_si128(input, h, 0x01); // Mid

    // Combine and reduce mod x^128 + x^7 + x^2 + x + 1
    let tmp5 = _mm_xor_si128(tmp5, tmp6);
    // ... reduction steps

    result
}
```

#### 3. Sha256Capsule (T2 SIMD, 128B)

```rust
/// Hardware-accelerated SHA-256 using SHA-NI
#[repr(C, align(128))]
pub struct Sha256Capsule {
    /// SHA-256 state (8 × 32-bit words)
    state: [u32; 8],

    /// Message buffer (64 bytes)
    buffer: [u8; 64],

    /// Bytes in buffer
    buffer_len: u32,

    /// Total bytes processed
    total_len: u64,

    /// Operation counter (atomic)
    hash_count: AtomicU64,

    /// Padding
    _padding: [u8; 28],
}

impl Sha256Capsule {
    /// Initialize for new hash computation
    pub fn init(&mut self);

    /// Update hash with data
    ///
    /// # Performance: 3-5 GB/s (SHA-NI)
    #[target_feature(enable = "sha")]
    pub unsafe fn update(&mut self, data: &[u8]);

    /// Finalize and return digest
    pub fn finalize(&mut self, digest: &mut [u8; 32]);

    /// One-shot hash
    ///
    /// # Performance: <100µs per KB
    pub fn hash(data: &[u8], digest: &mut [u8; 32]);
}
```

**SHA-NI Implementation**:

```rust
/// SHA-256 round using SHA-NI
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha")]
unsafe fn sha256_round(
    state0: &mut __m128i,
    state1: &mut __m128i,
    msg: __m128i,
    k: __m128i,
) {
    use core::arch::x86_64::*;

    let msg_k = _mm_add_epi32(msg, k);
    *state1 = _mm_sha256rnds2_epu32(*state1, *state0, msg_k);
    let msg_k_hi = _mm_shuffle_epi32(msg_k, 0x0E);
    *state0 = _mm_sha256rnds2_epu32(*state0, *state1, msg_k_hi);
}

/// SHA-256 message schedule using SHA-NI
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha")]
unsafe fn sha256_msg_schedule(
    msg0: __m128i,
    msg1: __m128i,
    msg2: __m128i,
    msg3: __m128i,
) -> __m128i {
    use core::arch::x86_64::*;

    let tmp = _mm_sha256msg1_epu32(msg0, msg1);
    let tmp = _mm_add_epi32(tmp, _mm_alignr_epi8(msg3, msg2, 4));
    _mm_sha256msg2_epu32(tmp, msg3)
}
```

#### 4. HmacCapsule (T1+T2 Mixed, 128B)

```rust
/// HMAC-SHA256 with lockfree key management
#[repr(C, align(128))]
pub struct HmacCapsule {
    /// Inner key (XOR'd with ipad)
    inner_key: [u8; 64],

    /// Outer key (XOR'd with opad)
    outer_key: [u8; 64],

    /// Key generation (for validity checking)
    generation: AtomicU32,

    /// Operation counter
    mac_count: AtomicU64,

    /// Padding
    _padding: [u8; 52],
}

impl HmacCapsule {
    /// Initialize with key
    ///
    /// # Performance: <1µs
    pub fn init(&mut self, key: &[u8]) -> Result<(), CryptoError>;

    /// Compute HMAC
    ///
    /// # Performance: 2-4 GB/s
    pub fn compute(&self, message: &[u8], mac: &mut [u8; 32]);

    /// Verify HMAC (constant-time)
    pub fn verify(&self, message: &[u8], mac: &[u8; 32]) -> bool;
}
```

#### 5. X25519Capsule (T1 Atomic, 128B)

```rust
/// X25519 Diffie-Hellman key exchange
/// Montgomery ladder implementation (constant-time)
#[repr(C, align(128))]
pub struct X25519Capsule {
    /// Secret scalar (32 bytes, clamped)
    secret_scalar: [u8; 32],

    /// Public point (32 bytes)
    public_point: [u8; 32],

    /// Shared secret (after exchange)
    shared_secret: [u8; 32],

    /// State: [initialized: u8 | has_peer: u8 | _pad: u48] + generation
    state: AtomicU64,

    /// Exchange count
    exchange_count: AtomicU64,

    /// Padding
    _padding: [u8; 24],
}

impl X25519Capsule {
    /// Generate key pair from seed
    ///
    /// # Performance: <20µs
    pub fn generate(&mut self, seed: &[u8; 32]);

    /// Compute shared secret
    ///
    /// # Performance: <50µs (Montgomery ladder, constant-time)
    /// # ASSUM: Peer public key validated (not low-order point)
    pub fn exchange(&mut self, peer_public: &[u8; 32]) -> Result<(), CryptoError>;

    /// Get public key
    pub fn public_key(&self) -> &[u8; 32];

    /// Get shared secret (after exchange)
    pub fn shared_secret(&self) -> Result<&[u8; 32], CryptoError>;
}
```

**Montgomery Ladder Implementation**:

```rust
/// Constant-time Montgomery ladder for X25519
/// Reference: RFC 7748 Section 5
fn montgomery_ladder(
    scalar: &[u8; 32],
    base_point: &[u8; 32],
) -> [u8; 32] {
    // Field element operations (mod p = 2^255 - 19)
    let mut x_1 = FieldElement::from_bytes(base_point);
    let mut x_2 = FieldElement::one();
    let mut z_2 = FieldElement::zero();
    let mut x_3 = x_1.clone();
    let mut z_3 = FieldElement::one();

    let mut swap = 0u8;

    // 255 iterations (constant-time)
    for t in (0..255).rev() {
        let k_t = (scalar[t >> 3] >> (t & 7)) & 1;
        swap ^= k_t;

        // Conditional swap (constant-time)
        x_2.cswap(&mut x_3, swap);
        z_2.cswap(&mut z_3, swap);
        swap = k_t;

        // Montgomery step (addition-subtraction chain)
        let a = x_2.add(&z_2);
        let aa = a.square();
        let b = x_2.sub(&z_2);
        let bb = b.square();
        let e = aa.sub(&bb);
        let c = x_3.add(&z_3);
        let d = x_3.sub(&z_3);
        let da = d.mul(&a);
        let cb = c.mul(&b);
        x_3 = da.add(&cb).square();
        z_3 = x_1.mul(&da.sub(&cb).square());
        x_2 = aa.mul(&bb);
        z_2 = e.mul(&aa.add(&e.mul_121666()));
    }

    // Final conditional swap
    x_2.cswap(&mut x_3, swap);
    z_2.cswap(&mut z_3, swap);

    // Compute result: x_2 * z_2^{-1}
    x_2.mul(&z_2.invert()).to_bytes()
}
```

#### 6. Ed25519Capsule (T1 Atomic, 256B)

```rust
/// Ed25519 digital signatures
/// Compressed Edwards point representation
#[repr(C, align(256))]
pub struct Ed25519Capsule {
    /// Secret key seed (32 bytes)
    seed: [u8; 32],

    /// Expanded secret key (64 bytes)
    /// [0:32] = secret scalar, [32:64] = prefix for nonce derivation
    secret_key: [u8; 64],

    /// Public key (32 bytes, compressed Edwards point)
    public_key: [u8; 32],

    /// Last signature (for audit)
    last_signature: [u8; 64],

    /// State + generation
    state: AtomicU64,

    /// Sign/verify counters
    sign_count: AtomicU64,
    verify_count: AtomicU64,

    /// Padding
    _padding: [u8; 16],
}

impl Ed25519Capsule {
    /// Generate key pair from seed
    ///
    /// # Performance: <50µs
    pub fn generate(&mut self, seed: &[u8; 32]);

    /// Sign message
    ///
    /// # Performance: <100µs (deterministic nonce, constant-time scalar mult)
    pub fn sign(&mut self, message: &[u8], signature: &mut [u8; 64]);

    /// Verify signature
    ///
    /// # Performance: <200µs (batch verification available)
    /// # ASSUM: Public key validated (on curve, not low-order)
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool;

    /// Batch verify multiple signatures
    ///
    /// # Performance: ~100µs per signature (batched)
    pub fn batch_verify(
        messages: &[&[u8]],
        signatures: &[&[u8; 64]],
        public_keys: &[&[u8; 32]],
    ) -> bool;
}
```

---

## API Surface

### High-Level API

```rust
/// CryptoMetacapsule - Unified cryptographic operations
impl CryptoMetacapsule {
    // === Construction ===

    /// Create new metacapsule
    pub fn new() -> Self;

    // === Key Management (T1 Atomic) ===

    /// Import symmetric key (AES-256)
    pub fn import_aes_key(&mut self, key: &[u8; 32]) -> Result<KeyHandle, CryptoError>;

    /// Import HMAC key
    pub fn import_hmac_key(&mut self, key: &[u8]) -> Result<KeyHandle, CryptoError>;

    /// Generate X25519 key pair
    pub fn generate_x25519(&mut self) -> Result<KeyHandle, CryptoError>;

    /// Generate Ed25519 key pair
    pub fn generate_ed25519(&mut self) -> Result<KeyHandle, CryptoError>;

    /// Rotate key (atomic, lockfree)
    pub fn rotate_key(&mut self, old: KeyHandle, new_material: &[u8]) -> Result<KeyHandle, CryptoError>;

    // === Symmetric Encryption (T2 SIMD) ===

    /// AES-256-GCM encrypt
    pub fn encrypt(
        &mut self,
        key: KeyHandle,
        iv: &[u8; 12],
        aad: &[u8],
        plaintext: &[u8],
        ciphertext: &mut [u8],
        tag: &mut [u8; 16],
    ) -> Result<(), CryptoError>;

    /// AES-256-GCM decrypt
    pub fn decrypt(
        &mut self,
        key: KeyHandle,
        iv: &[u8; 12],
        aad: &[u8],
        ciphertext: &[u8],
        tag: &[u8; 16],
        plaintext: &mut [u8],
    ) -> Result<(), CryptoError>;

    // === Hashing (T2 SIMD) ===

    /// SHA-256 hash
    pub fn sha256(&mut self, data: &[u8], digest: &mut [u8; 32]);

    /// SHA-384 hash
    pub fn sha384(&mut self, data: &[u8], digest: &mut [u8; 48]);

    /// SHA-512 hash
    pub fn sha512(&mut self, data: &[u8], digest: &mut [u8; 64]);

    // === MACs (T1+T2) ===

    /// HMAC-SHA256
    pub fn hmac(&mut self, key: KeyHandle, message: &[u8], mac: &mut [u8; 32]) -> Result<(), CryptoError>;

    /// Verify HMAC (constant-time)
    pub fn hmac_verify(&self, key: KeyHandle, message: &[u8], mac: &[u8; 32]) -> Result<bool, CryptoError>;

    // === Key Exchange (T1 Atomic) ===

    /// X25519 key exchange
    pub fn x25519_exchange(
        &mut self,
        key: KeyHandle,
        peer_public: &[u8; 32],
        shared_secret: &mut [u8; 32],
    ) -> Result<(), CryptoError>;

    // === Digital Signatures (T1 Atomic) ===

    /// Ed25519 sign
    pub fn sign(
        &mut self,
        key: KeyHandle,
        message: &[u8],
        signature: &mut [u8; 64],
    ) -> Result<(), CryptoError>;

    /// Ed25519 verify
    pub fn verify(
        &self,
        public_key: &[u8; 32],
        message: &[u8],
        signature: &[u8; 64],
    ) -> bool;

    // === Key Derivation (T1+T2) ===

    /// HKDF-SHA256 extract + expand
    pub fn hkdf(
        &mut self,
        salt: &[u8],
        ikm: &[u8],
        info: &[u8],
        okm: &mut [u8],
    ) -> Result<(), CryptoError>;

    /// PBKDF2-HMAC-SHA256
    pub fn pbkdf2(
        &mut self,
        password: &[u8],
        salt: &[u8],
        iterations: u32,
        derived_key: &mut [u8],
    ) -> Result<(), CryptoError>;

    // === Audit (T0 Auditable) ===

    /// Get audit trail hash chain
    pub fn audit_chain(&self) -> u64;

    /// Get operation counts
    pub fn stats(&self) -> CryptoStats;
}
```

---

## T28 Test Plan

### Q1-Q7: Unit Tests

| Test | Vectors | Expected |
|------|---------|----------|
| AES-256-GCM encrypt | NIST GCM Test Vectors (800-38D) | Exact match |
| AES-256-GCM decrypt | NIST GCM Test Vectors | Exact match |
| SHA-256 | NIST CAVP SHA Test Vectors | Exact match |
| SHA-384 | NIST CAVP SHA Test Vectors | Exact match |
| SHA-512 | NIST CAVP SHA Test Vectors | Exact match |
| HMAC-SHA256 | RFC 4231 Test Vectors | Exact match |
| X25519 | RFC 7748 Test Vectors | Exact match |
| Ed25519 sign | RFC 8032 Test Vectors | Exact match |
| Ed25519 verify | RFC 8032 Test Vectors | Pass/Fail |
| HKDF-SHA256 | RFC 5869 Test Vectors | Exact match |
| PBKDF2-SHA256 | RFC 6070 Test Vectors | Exact match |

### Q8-Q14: Property Tests

```rust
#[proptest]
fn encrypt_decrypt_roundtrip(plaintext: Vec<u8>, key: [u8; 32], iv: [u8; 12]) {
    let mut crypto = CryptoMetacapsule::new();
    let handle = crypto.import_aes_key(&key)?;

    let mut ciphertext = vec![0u8; plaintext.len()];
    let mut tag = [0u8; 16];
    crypto.encrypt(handle, &iv, &[], &plaintext, &mut ciphertext, &mut tag)?;

    let mut decrypted = vec![0u8; plaintext.len()];
    crypto.decrypt(handle, &iv, &[], &ciphertext, &tag, &mut decrypted)?;

    prop_assert_eq!(plaintext, decrypted);
}

#[proptest]
fn hmac_verify_correct(key: Vec<u8>, message: Vec<u8>) {
    let mut crypto = CryptoMetacapsule::new();
    let handle = crypto.import_hmac_key(&key)?;

    let mut mac = [0u8; 32];
    crypto.hmac(handle, &message, &mut mac)?;

    prop_assert!(crypto.hmac_verify(handle, &message, &mac)?);
}

#[proptest]
fn x25519_shared_secret_symmetric(seed_a: [u8; 32], seed_b: [u8; 32]) {
    let mut crypto_a = CryptoMetacapsule::new();
    let mut crypto_b = CryptoMetacapsule::new();

    let handle_a = crypto_a.generate_x25519_from_seed(&seed_a)?;
    let handle_b = crypto_b.generate_x25519_from_seed(&seed_b)?;

    let pub_a = crypto_a.get_public_key(handle_a)?;
    let pub_b = crypto_b.get_public_key(handle_b)?;

    let mut shared_a = [0u8; 32];
    let mut shared_b = [0u8; 32];

    crypto_a.x25519_exchange(handle_a, &pub_b, &mut shared_a)?;
    crypto_b.x25519_exchange(handle_b, &pub_a, &mut shared_b)?;

    prop_assert_eq!(shared_a, shared_b);
}
```

### Q15-Q21: Integration Tests

```rust
#[test]
fn test_full_tls_like_handshake() {
    // Simulates TLS 1.3 handshake pattern:
    // 1. X25519 key exchange
    // 2. HKDF key derivation
    // 3. AES-GCM encrypted application data
    // 4. HMAC integrity
}

#[test]
fn test_concurrent_key_rotation() {
    // 8 threads rotating keys concurrently
    // Verify no lost operations, generation counters correct
}

#[test]
fn test_audit_trail_integrity() {
    // Perform 1000 operations
    // Verify hash chain is continuous
    // Verify no events lost
}
```

### Q22-Q28: Production Tests

```rust
#[test]
fn test_timing_attack_resistance() {
    // dudect statistical testing
    // Compare timing variance across different inputs
    // p-value > 0.05 required
}

#[test]
fn test_aes_ni_fallback() {
    // Force scalar fallback
    // Verify correct results
    // Measure performance degradation
}

#[test]
fn test_memory_zeroization() {
    // Verify key material zeroed on drop
    // Use miri for memory safety
}
```

### Q29-Q35: Determinism Tests (T28 Tier 5)

```rust
#[test]
fn test_deterministic_encryption() {
    // Same key + IV + plaintext = same ciphertext
    // Required for reproducible testing
}

#[test]
fn test_deterministic_signing() {
    // Ed25519 uses deterministic nonce (RFC 8032)
    // Same message + key = same signature
}

#[test]
fn test_generation_counter_monotonic() {
    // Generation counters never decrease
    // Even under concurrent access
}
```

---

## B32 Benchmark Plan

### Benchmark Matrix

| Algorithm | Payload Sizes | Iterations | Baseline |
|-----------|---------------|------------|----------|
| AES-256-GCM encrypt | 16B, 256B, 1KB, 16KB, 1MB | 10000 | ring::aead |
| AES-256-GCM decrypt | 16B, 256B, 1KB, 16KB, 1MB | 10000 | ring::aead |
| SHA-256 | 64B, 256B, 1KB, 16KB, 1MB | 10000 | ring::digest |
| SHA-512 | 64B, 256B, 1KB, 16KB, 1MB | 10000 | ring::digest |
| HMAC-SHA256 | 64B, 256B, 1KB, 16KB | 10000 | ring::hmac |
| X25519 | N/A (fixed) | 10000 | ring::agreement |
| Ed25519 sign | 64B, 256B, 1KB | 10000 | ring::signature |
| Ed25519 verify | 64B, 256B, 1KB | 10000 | ring::signature |

### Performance Validation

```rust
// B32 Framework: 95% CI, 1000+ iterations, fair baseline

#[bench]
fn bench_aes_gcm_1kb(b: &mut Bencher) {
    let mut crypto = CryptoMetacapsule::new();
    let key = crypto.import_aes_key(&[0u8; 32]).unwrap();
    let plaintext = vec![0u8; 1024];
    let mut ciphertext = vec![0u8; 1024];
    let mut tag = [0u8; 16];
    let iv = [0u8; 12];

    b.iter(|| {
        crypto.encrypt(key, &iv, &[], &plaintext, &mut ciphertext, &mut tag).unwrap();
    });

    // Expected: >5 GB/s = <200ns for 1KB
}

#[bench]
fn bench_sha256_1kb(b: &mut Bencher) {
    let mut crypto = CryptoMetacapsule::new();
    let data = vec![0u8; 1024];
    let mut digest = [0u8; 32];

    b.iter(|| {
        crypto.sha256(&data, &mut digest);
    });

    // Expected: >3 GB/s = <350ns for 1KB
}
```

### Hardware Detection Benchmarks

```rust
#[bench]
fn bench_with_aesni(b: &mut Bencher) {
    assert!(is_x86_feature_detected!("aes"));
    // ... benchmark
}

#[bench]
fn bench_without_aesni(b: &mut Bencher) {
    // Force scalar fallback via environment variable
    std::env::set_var("CRYPTO_FORCE_SCALAR", "1");
    // ... benchmark
}
```

---

## ASSUM Safety Documentation

### Cryptographic Assumptions

| Tag | Assumption | Verification |
|-----|------------|--------------|
| #ASSUME_AES256_SECURE | AES-256 provides 256-bit security | NIST FIPS 197 |
| #ASSUME_GCM_SECURE | GCM mode provides authenticated encryption | NIST SP 800-38D |
| #ASSUME_SHA256_SECURE | SHA-256 collision-resistant | NIST FIPS 180-4 |
| #ASSUME_X25519_SECURE | X25519 secure ECDH | RFC 7748, Curve25519 paper |
| #ASSUME_ED25519_SECURE | Ed25519 secure signatures | RFC 8032, EdDSA analysis |
| #ASSUME_HKDF_SECURE | HKDF secure KDF | RFC 5869, HKDF paper |

### Timing Assumptions

| Tag | Assumption | Verification |
|-----|------------|--------------|
| #ASSUME_CONSTANT_TIME_AES | AES-NI constant-time | Hardware guarantee |
| #ASSUME_CONSTANT_TIME_CLMUL | CLMUL constant-time | Hardware guarantee |
| #ASSUME_CONSTANT_TIME_SHA | SHA-NI constant-time | Hardware guarantee |
| #ASSUME_CONSTANT_TIME_LADDER | Montgomery ladder constant-time | Code inspection + dudect |
| #ASSUME_CONSTANT_TIME_COMPARE | Tag comparison constant-time | XOR accumulation pattern |

### Memory Assumptions

| Tag | Assumption | Verification |
|-----|------------|--------------|
| #ASSUME_KEY_ZEROIZED | Key material zeroed on drop | Drop impl + miri |
| #ASSUME_NO_SWAP | Sensitive memory not swapped | mlock recommendation |
| #ASSUME_CACHE_ALIGNED | Capsules cache-aligned | Static assert |

### Concurrency Assumptions

| Tag | Assumption | Verification |
|-----|------------|--------------|
| #ASSUME_LOCKFREE_KEYS | Key operations lockfree | No Mutex/RwLock |
| #ASSUME_GENERATION_MONOTONIC | Generations never decrease | Atomic increment only |
| #ASSUME_NO_ABA | Generation counters prevent ABA | Counter comparison |

---

## Implementation Roadmap

### Phase 1: Core Primitives (Week 1)

1. **KeyManagerCapsule**: DualAtomicU64 key slots, generation counters
2. **Sha256Capsule**: SHA-NI + scalar fallback
3. **ConstantTimeOpsCapsule**: Integration (already exists)

### Phase 2: Symmetric Crypto (Week 2)

1. **AesGcmCapsule**: AES-NI + CLMUL + scalar fallback
2. **HmacCapsule**: SHA-NI integration
3. Unit tests with NIST vectors

### Phase 3: Asymmetric Crypto (Week 3)

1. **X25519Capsule**: Montgomery ladder, field arithmetic
2. **Ed25519Capsule**: Compressed Edwards, deterministic nonce
3. Property tests for key exchange symmetry

### Phase 4: Metacapsule Integration (Week 4)

1. **CryptoMetacapsule**: Orchestration layer
2. **AuditTrailCapsule**: Q34 hash chain
3. Full T28 test suite
4. B32 benchmarks vs ring

### Phase 5: Production Hardening (Week 5)

1. dudect timing validation
2. Memory zeroization verification
3. WASM compatibility (scalar-only)
4. Documentation and examples

---

## Trade Secret Justification

The CryptoMetacapsule represents trade secret protected innovation because:

### 1. Novel Synthesis

No existing implementation combines:
- **Chaos lockfree patterns** with cryptographic key management
- **Hardware SIMD crypto** (AES-NI, SHA-NI, CLMUL) within capsule constraints
- **Generation counter key rotation** for ABA-safe concurrent access
- **Q34 audit trails** for compliance (SOX/SOC2/HIPAA)

### 2. Competitive Advantage

| Capability | CryptoMetacapsule | ring | RustCrypto |
|------------|-------------------|------|------------|
| Lockfree keys | Yes | No (internal mutex) | No |
| Chaos compliant | Yes | No | No |
| Generation counters | Yes | No | No |
| Audit trails | Yes (Q34) | No | No |
| Zero C/ASM deps | Yes | No (C/ASM) | Yes |
| Hardware crypto | Yes | Yes | Limited |

### 3. Investment Protection

Development cost estimate:
- 4-5 weeks engineering time
- NIST vector validation
- dudect timing analysis
- B32 benchmark validation

This represents significant R&D investment that provides:
- **Performance parity** with ring (5-10 GB/s)
- **Superior architecture** (lockfree, auditable)
- **Reduced dependency** (no C/ASM build complexity)

### 4. Enforcement

- **NO crates.io publication**
- **NO public GitHub**
- **NO blog posts** describing implementation
- **[TRADE SECRET] tags** on all commits
- **NDA required** for any external review

---

## Appendix: Feature Flags

```toml
[features]
# Core crypto (scalar fallback, always available)
crypto-core = ["std"]

# Hardware acceleration (requires x86_64)
crypto-aesni = ["crypto-core", "nightly"]
crypto-shani = ["crypto-core", "nightly"]
crypto-clmul = ["crypto-core", "nightly"]

# Full metacapsule with all algorithms
crypto-metacapsule = [
    "crypto-core",
    "crypto-aesni",
    "crypto-shani",
    "crypto-clmul",
]

# Audit trail integration
crypto-audit = ["crypto-metacapsule", "audit-trail"]

# Default: Full hardware acceleration
default = ["crypto-metacapsule"]
```

---

**Document Version**: 1.0.0
**Author**: UCE34 Framework
**Classification**: TRADE SECRET
**Date**: 2025-11-24
