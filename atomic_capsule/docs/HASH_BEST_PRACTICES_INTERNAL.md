# Hash Capsules - Internal Best Practices

**Version**: 1.0.0 **Date**: 2025-10-19
**Audience**: Internal development team
**Classification**: INTERNAL - Trade Secret Protection

---

## Table of Contents

1. [Security Best Practices](#security-best-practices)
2. [Performance Guidelines](#performance-guidelines)
3. [Trade-offs & Decision Making](#trade-offs--decision-making)
4. [Testing Requirements](#testing-requirements)
5. [Benchmarking Standards (B32)](#benchmarking-standards-b32)
6. [Compliance Implementation](#compliance-implementation)
7. [Anti-Patterns & Common Mistakes](#anti-patterns--common-mistakes)

---

## Security Best Practices

### Rule 1: const_hash is NOT Cryptographic

**Critical Understanding**: FNV-1a (used in `const_hash`) is a **non-cryptographic** hash function.

#### Safe Uses
```rust
// ✅ SAFE: Static IDs (known at compile-time, non-adversarial)
const BUDGET_ID: u64 = const_fast_hash(b"budget_marketing");

// ✅ SAFE: Internal type discrimination
const TYPE_HASH: u64 = const_fast_hash(b"MyStructType");

// ✅ SAFE: Configuration keys (trusted sources)
const CONFIG_KEY: u64 = const_fast_hash(b"config.database.url");
```

#### Unsafe Uses (DO NOT DO THIS)
```rust
// ❌ UNSAFE: User-controlled inputs (adversarial attack risk)
let user_input = req.body.get("key");
let hash = const_fast_hash(user_input.as_bytes());  // VULNERABLE!

// ❌ UNSAFE: Password hashing (use argon2/bcrypt)
let password_hash = const_fast_hash(password.as_bytes());  // CRITICAL VULNERABILITY!

// ❌ UNSAFE: Cryptographic signatures (use ed25519)
let signature = const_fast_hash(message.as_bytes());  // NOT SECURE!
```

#### Why It's Unsafe
- **No cryptographic guarantees**: Collisions can be found intentionally
- **Deterministic**: Same input → same output (no salt, no randomness)
- **Fast**: Speed is the opposite of security (no work factor)

#### Mitigation Strategy

| **Use Case** | **Hash Function** | **Feature Flag** |
|--------------|-------------------|------------------|
| Password hashing | `argon2` or `bcrypt` | External crate |
| Crypto signatures | `ed25519` or `ECDSA` | External crate |
| Audit trails (tamper-evident) | BLAKE3 or SHA-256 | `audit-trail` or `fips-compliant` |
| Non-repudiation (SOX/SOC2) | HMAC-SHA256 | `keyed-hashing` |
| Internal IDs (trusted) | `const_hash` or `fast-hash` | `const-hashing` or `fast-hash` |

### Rule 2: HMAC Key Management

**CRITICAL**: Improper key management destroys all security guarantees.

#### Secure Key Generation
```rust
use rand::RngCore;

fn generate_secure_hmac_key() -> [u8; 32] {
    let mut key = [0u8; 32];

    // ✅ CORRECT: Crypto-secure RNG
    rand::thread_rng().fill_bytes(&mut key);

    key
}

// ❌ WRONG: Predictable key
fn insecure_key() -> [u8; 32] {
    [0x42u8; 32]  // NEVER use constants in production!
}
```

#### Key Storage
```rust
// ✅ CORRECT: Encrypted at rest (production)
fn store_key_securely(key: &[u8; 32]) {
    // AWS KMS example
    aws_kms::encrypt_and_store(key, "hmac-key-v1")?;

    // HashiCorp Vault example
    vault::kv::write("secret/hmac-key", key)?;

    // Azure Key Vault example
    azure_kv::set_secret("hmac-key", key)?;
}

// ❌ WRONG: Plaintext storage
fn insecure_storage(key: &[u8; 32]) {
    std::fs::write("/tmp/key.txt", key)?;  // CRITICAL VULNERABILITY!
}
```

#### Key Rotation (90-day recommended)
```rust
use std::time::Duration;

struct KeyRotationPolicy {
    rotation_period: Duration,
    key_history: Vec<(u64, [u8; 32])>,  // (timestamp, key)
}

impl KeyRotationPolicy {
    fn new() -> Self {
        Self {
            rotation_period: Duration::from_secs(90 * 24 * 60 * 60),  // 90 days
            key_history: Vec::new(),
        }
    }

    fn should_rotate(&self, last_rotation: u64) -> bool {
        let now = current_timestamp();
        now - last_rotation > self.rotation_period.as_secs()
    }

    fn rotate_key(&mut self) -> Result<(), KeyRotationError> {
        // Generate new key
        let new_key = generate_secure_hmac_key();

        // Rotate global key (returns old key)
        let old_key = HmacKey::rotate(&new_key);

        // Archive old key for historical verification
        let timestamp = current_timestamp();
        self.key_history.push((timestamp, old_key));

        // Store new key encrypted
        store_key_securely(&new_key)?;

        Ok(())
    }

    fn verify_historical(&self, data: &[u8], timestamp: u64, stored_hash: &[u8; 32]) -> bool {
        // Find key active at timestamp
        let key = self.key_history.iter()
            .rev()  // Search backwards (most recent first)
            .find(|(ts, _)| *ts <= timestamp)
            .map(|(_, k)| k);

        if let Some(key) = key {
            let computed = compute_hmac_with_key(data, key);
            &computed == stored_hash
        } else {
            false  // No key found for timestamp
        }
    }
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn compute_hmac_with_key(data: &[u8], key: &[u8; 32]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(data);
    mac.finalize().into_bytes().into()
}

#[derive(Debug)]
enum KeyRotationError {
    GenerationFailed,
    StorageFailed,
}
```

### Rule 3: Torn Read Prevention (AtomicHash256)

**Critical Understanding**: `AtomicHash256` uses **SeqLock pattern** which requires **Single Writer, Many Readers (SWeMR)**.

#### Correct Usage (SWeMR)
```rust
use std::sync::Arc;
use std::thread;

// ✅ CORRECT: ONE writer, multiple readers
let hash = Arc::new(AtomicHash256::new([0u8; 32]));

// Single writer thread
let writer = {
    let h = Arc::clone(&hash);
    thread::spawn(move || {
        for i in 0..1000 {
            let pattern = if i % 2 == 0 { [0xFF; 32] } else { [0x00; 32] };
            h.store(pattern);  // SeqLock protocol
        }
    })
};

// Multiple reader threads (safe!)
let readers: Vec<_> = (0..8).map(|_| {
    let h = Arc::clone(&hash);
    thread::spawn(move || {
        for _ in 0..1000 {
            let value = h.load();  // Torn-read safe
            // Process value...
        }
    })
}).collect();

writer.join().unwrap();
for r in readers {
    r.join().unwrap();
}
```

#### Incorrect Usage (Multiple Writers - RACE CONDITION)
```rust
// ❌ WRONG: Multiple writers (UNDEFINED BEHAVIOR)
let hash = Arc::new(AtomicHash256::new([0u8; 32]));

let writer1 = {
    let h = Arc::clone(&hash);
    thread::spawn(move || h.store([0xFF; 32]))
};

let writer2 = {
    let h = Arc::clone(&hash);
    thread::spawn(move || h.store([0x00; 32]))  // RACE! Generation counter corrupted!
};

// ⚠️ RESULT: Torn reads, corrupted generation counter, undefined behavior
```

#### Mitigation for Multiple Writers
```rust
use std::sync::Mutex;

// Option 1: Mutex around writer (serializes writes)
struct SyncedHash256 {
    hash: Arc<AtomicHash256>,
    write_lock: Mutex<()>,
}

impl SyncedHash256 {
    fn store(&self, value: [u8; 32]) {
        let _guard = self.write_lock.lock().unwrap();
        self.hash.store(value);
    }

    fn load(&self) -> [u8; 32] {
        self.hash.load()  // No lock needed (many readers)
    }
}

// Option 2: Single writer task (message passing)
use std::sync::mpsc;

struct HashWriter {
    hash: Arc<AtomicHash256>,
    rx: mpsc::Receiver<[u8; 32]>,
}

impl HashWriter {
    fn run(mut self) {
        while let Ok(value) = self.rx.recv() {
            self.hash.store(value);  // Single writer
        }
    }
}

// Multiple threads send to single writer
let (tx, rx) = mpsc::channel();
let writer = HashWriter { hash: Arc::clone(&hash), rx };
thread::spawn(move || writer.run());

// Thread 1
let tx1 = tx.clone();
thread::spawn(move || tx1.send([0xFF; 32]).unwrap());

// Thread 2
thread::spawn(move || tx.send([0x00; 32]).unwrap());
```

---

## Performance Guidelines

### Rule 4: SIMD Threshold Awareness

**Critical Understanding**: SIMD has **~10ns setup overhead**. Below **4 fields**, scalar is faster.

#### Performance Matrix (B32 Validated)

| **Field Count** | **Scalar** | **SIMD** | **Winner** | **Why** |
|-----------------|------------|----------|------------|---------|
| 1 | 4ns | 14ns | Scalar | SIMD overhead > benefit |
| 2 | 8ns | 12ns | Scalar | Overhead still dominates |
| 3 | 12ns | 10ns | Marginal | Close, prefer scalar |
| **4** | **16ns** | **8ns** | **SIMD** | **Threshold** |
| 8 | 32ns | 12ns | SIMD | 2.7× speedup |
| 16 | 64ns | 20ns | SIMD | 3.2× speedup |
| 32 | 128ns | 32ns | SIMD | 4.0× speedup |

#### Automatic Dispatcher (Recommended)
```rust
use atomic_capsule::hash::simd_hash::best_hash;

fn hash_fields(fields: &[u64]) -> u64 {
    // ✅ CORRECT: Automatically chooses optimal implementation
    best_hash(fields)  // <4 fields → scalar, 4+ → SIMD
}

// ❌ WRONG: Always use SIMD (slower for <4 fields)
#[cfg(feature = "simd-hashing")]
fn always_simd(fields: &[u64]) -> u64 {
    simd_fast_hash_multi(fields)  // Slow for 2 fields!
}
```

#### Manual Threshold Check (Advanced)
```rust
const SIMD_THRESHOLD: usize = 4;

fn hash_with_threshold(fields: &[u64]) -> u64 {
    if fields.len() < SIMD_THRESHOLD {
        scalar_fast_hash(fields)
    } else {
        #[cfg(feature = "simd-hashing")]
        {
            simd_fast_hash_multi(fields)
        }
        #[cfg(not(feature = "simd-hashing"))]
        {
            scalar_fast_hash(fields)
        }
    }
}
```

### Rule 5: Const Hash Compile-Time Budget

**Critical Understanding**: Compile-time hash adds **<5ms per hash** to build time.

#### Reasonable Limits
```rust
// ✅ ACCEPTABLE: 10-100 const hashes (<500ms total overhead)
pub mod budget_ids {
    const MARKETING: u64 = const_fast_hash(b"budget_marketing");
    const ENGINEERING: u64 = const_fast_hash(b"budget_engineering");
    // ... 98 more IDs
}

// ⚠️ WARNING: 1000+ const hashes (>5 seconds overhead)
pub mod massive_ids {
    // ... 1000+ const hashes
    // Build time: +5-10 seconds (may be acceptable if rarely changed)
}

// ❌ UNACCEPTABLE: Dynamic const hash generation (macro abuse)
macro_rules! generate_1000_ids {
    () => {
        // Generates 1000 const hashes via macro expansion
        // Build time: +5-10 seconds EVERY build (incremental doesn't help)
    };
}
```

#### Incremental Compilation Helps
```bash
# First build: 523ms (with 100 const hashes)
cargo build --features const-hashing

# Incremental rebuild: 16ms (only changed files)
cargo build --features const-hashing
```

#### Recommendation
- **<100 const hashes**: Acceptable (<500ms overhead)
- **100-500 const hashes**: Consider if worth trade-off (1-3s overhead)
- **>500 const hashes**: Use runtime hash map instead

### Rule 6: Binary Size Trade-offs

| **Feature** | **Binary Size** | **When Worth It** |
|-------------|-----------------|-------------------|
| Base (no hash) | 0 bytes | Always |
| `const-hashing` | +8B per hash | Static IDs (<100 IDs) |
| `simd-hashing` | +2KB | Multi-field hashing (4+ fields) |
| `fast-hash` | +8KB | Development/internal hashing |
| `audit-trail` | +23KB | Production audit trails |
| `highway-hash` | +15KB | High-performance (2-4× faster) |
| `fips-compliant` | +50KB | Government/regulated only |
| `keyed-hashing` | +15KB | Compliance (SOX/SOC2) only |

#### Decision Matrix
```rust
// Mobile app: Minimize binary size
// Use: No features, scalar_fast_hash only
atomic_capsule = { version = "0.5" }  // +0KB

// Web service: Balance size/performance
// Use: profile-production
atomic_capsule = { version = "0.5", features = ["profile-production"] }  // +23KB

// High-frequency trading: Maximum performance
// Use: profile-high-performance
atomic_capsule = { version = "0.5", features = ["profile-high-performance"] }  // +27KB

// Government compliance: FIPS required
// Use: profile-government
atomic_capsule = { version = "0.5", features = ["profile-government"] }  // +50KB
```

---

## Trade-offs & Decision Making

### Decision Framework

```
START: Need to hash data?

1. Is it security-critical?
   YES → Use crypto hash (BLAKE3/SHA-256/HMAC)
   NO  → Continue to 2

2. Is it compliance-related (SOX/SOC2/GDPR)?
   YES → Use keyed_hash (HMAC-SHA256)
   NO  → Continue to 3

3. Is data known at compile-time?
   YES → Use const_hash (0ns runtime)
   NO  → Continue to 4

4. How many fields?
   <4  → Use scalar_hash (faster)
   4+  → Use simd_hash (2-8× speedup)

5. Need thread-safe storage?
   YES (u64)    → AtomicHash64
   YES (256-bit) → AtomicHash256
   NO           → Direct hash value
```

### Trade-off Table

| **Requirement** | **Option A** | **Option B** | **Trade-off** |
|-----------------|--------------|--------------|---------------|
| Static IDs | `const_hash` (0ns) | HashMap (25ns) | +5ms build vs 25ns runtime |
| Multi-field (8) | `simd_hash` (12ns) | `scalar_hash` (32ns) | +2KB binary vs 2.7× slower |
| Audit trail | BLAKE3 (100ns) | xxHash64 (5ns) | +23KB binary vs not secure |
| Compliance | HMAC-SHA256 (500ns) | BLAKE3 (100ns) | Non-repudiation vs speed |
| Thread-safe | AtomicHash64 (5ns) | Mutex (30ns) | Lockfree vs simple |
| 256-bit storage | AtomicHash256 (30ns) | Mutex (50ns) | SeqLock vs mutex overhead |

### When to Choose What

#### Choose `const_hash` when:
- ✅ Data known at compile-time (static IDs)
- ✅ Small ID set (<100 IDs)
- ✅ 0ns runtime cost critical
- ✅ Acceptable +5ms build time per hash

#### Choose `simd_hash` when:
- ✅ Multi-field capsule (4+ u64 fields)
- ✅ 2-8× speedup needed
- ✅ Nightly Rust available
- ✅ Acceptable +2KB binary size

#### Choose `keyed_hash` when:
- ✅ Compliance required (SOX/SOC2/GDPR/HIPAA)
- ✅ Non-repudiation needed (timestamp + signer ID)
- ✅ Tamper-evident audit trail
- ✅ Acceptable +15KB binary + <500ns latency

#### Choose `AtomicHash256` when:
- ✅ 256-bit crypto hash storage (BLAKE3/SHA-256)
- ✅ Concurrent readers + single writer (SWeMR)
- ✅ Torn-read prevention critical
- ✅ Acceptable <30ns latency

---

## Testing Requirements

### T28 Testing Framework Application

All hash implementations MUST pass T28 testing tiers:

#### Tier 1: Unit Tests (Required)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic() {
        let hash1 = compute_hash(data);
        let hash2 = compute_hash(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_inputs() {
        let hash1 = compute_hash(data1);
        let hash2 = compute_hash(data2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_empty_input() {
        let hash = compute_hash(&[]);
        assert_ne!(hash, 0);  // Or specific expected value
    }
}
```

#### Tier 2: Property Tests (Recommended)
```rust
#[cfg(all(test, feature = "proptest"))]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_deterministic(data: Vec<u8>) {
            let hash1 = compute_hash(&data);
            let hash2 = compute_hash(&data);
            prop_assert_eq!(hash1, hash2);
        }

        #[test]
        fn prop_collision_resistance(data1: Vec<u8>, data2: Vec<u8>) {
            prop_assume!(data1 != data2);
            let hash1 = compute_hash(&data1);
            let hash2 = compute_hash(&data2);
            // Note: Collisions possible but rare
            if hash1 == hash2 {
                println!("Collision found (rare but expected): {:?} vs {:?}", data1, data2);
            }
        }
    }
}
```

#### Tier 3: Concurrent Tests (Required for Atomic*)
```rust
#[test]
fn test_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let hash = Arc::new(AtomicHash64::new(0));
    let mut handles = vec![];

    for i in 0..10 {
        let h = Arc::clone(&hash);
        handles.push(thread::spawn(move || {
            for j in 0..1000 {
                h.store((i * 1000 + j) as u64);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify no panics, final value valid
    let final_val = hash.load();
    assert!(final_val < 10_000);
}
```

#### Tier 4: Torn Read Tests (Required for AtomicHash256)
```rust
#[test]
fn test_no_torn_reads() {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    let hash = Arc::new(AtomicHash256::new([0u8; 32]));
    let stop = Arc::new(AtomicBool::new(false));
    let torn_count = Arc::new(AtomicU64::new(0));

    // Writer: Alternates [0xFF; 32] and [0x00; 32]
    let writer = {
        let h = Arc::clone(&hash);
        let s = Arc::clone(&stop);
        thread::spawn(move || {
            for i in 0..100_000 {
                let pattern = if i % 2 == 0 { [0xFF; 32] } else { [0x00; 32] };
                h.store(pattern);
            }
            s.store(true, Ordering::Release);
        })
    };

    // Readers: Detect torn reads
    let readers: Vec<_> = (0..8).map(|_| {
        let h = Arc::clone(&hash);
        let s = Arc::clone(&stop);
        let t = Arc::clone(&torn_count);
        thread::spawn(move || {
            while !s.load(Ordering::Acquire) {
                let value = h.load();
                let all_ff = value.iter().all(|&b| b == 0xFF);
                let all_00 = value.iter().all(|&b| b == 0x00);
                if !all_ff && !all_00 {
                    t.fetch_add(1, Ordering::Relaxed);
                }
            }
        })
    }).collect();

    writer.join().unwrap();
    for r in readers {
        r.join().unwrap();
    }

    let torn = torn_count.load(Ordering::Relaxed);
    assert_eq!(torn, 0, "Torn reads detected: {}", torn);
}
```

---

## Benchmarking Standards (B32)

### B32 Framework Requirements

All performance claims MUST be validated with:

1. **Baseline comparison** (same hardware/compiler)
2. **95% confidence interval** (1000+ iterations)
3. **Optimized baseline** (not strawman comparison)
4. **Reproducibility** (documented setup)

#### Example Benchmark
```rust
#[cfg(test)]
mod benches {
    use super::*;
    use std::time::Instant;

    #[test]
    fn bench_simd_vs_scalar() {
        let fields = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let iterations = 100_000;

        // Scalar baseline
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = scalar_fast_hash(&fields);
        }
        let scalar_ns = start.elapsed().as_nanos() / iterations;

        // SIMD optimized
        #[cfg(feature = "simd-hashing")]
        {
            let start = Instant::now();
            for _ in 0..iterations {
                let _ = simd_fast_hash_multi(&fields);
            }
            let simd_ns = start.elapsed().as_nanos() / iterations;

            let speedup = scalar_ns as f64 / simd_ns as f64;

            println!("Scalar: {} ns", scalar_ns);
            println!("SIMD:   {} ns", simd_ns);
            println!("Speedup: {:.2}×", speedup);

            // B32 Honest Reporting: Document expected range
            assert!(speedup >= 2.0 && speedup <= 3.5,
                "Speedup {} outside expected 2.0-3.5× range (8 fields)", speedup);
        }
    }
}
```

### Reporting Template
```markdown
## Performance Claim

**Operation**: SIMD hash (8 fields)
**Hardware**: Intel Ultra 7 155H
**Compiler**: rustc 1.76.0
**Iterations**: 100,000
**Confidence**: 95% CI

**Results**:
- Scalar baseline: 32.1ns ± 1.2ns
- SIMD optimized: 11.8ns ± 0.8ns
- Speedup: 2.72× (95% CI: 2.55-2.91×)

**Validation**: Expected 2.0-3.5× for 8 fields (within range ✅)
```

---

## Compliance Implementation

### SOX (Sarbanes-Oxley) Requirements

**Requirement**: Tamper-evident audit trail for financial transactions.

#### Implementation Checklist
- [ ] Use `keyed_hash` (HMAC-SHA256)
- [ ] Include timestamp in hash input
- [ ] Include signer ID (non-repudiation)
- [ ] Hash chain links (sequential integrity)
- [ ] Key rotation every 90 days
- [ ] Store keys encrypted at rest (AWS KMS/Vault)
- [ ] Audit trail archival (7-year retention)

#### Code Example
```rust
#[cfg(feature = "keyed-hashing")]
fn sox_compliant_transaction_hash(
    txn: &FinancialTransaction,
    prev_hash: &[u8; 32],
) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;

    let key = HmacKey::get_global();
    let mut mac = HmacSha256::new_from_slice(key).unwrap();

    // SOX requirement: Include all transaction details
    mac.update(&txn.txn_id.to_le_bytes());
    mac.update(&txn.account_id.to_le_bytes());
    mac.update(&txn.amount_cents.to_le_bytes());
    mac.update(txn.description.as_bytes());

    // Non-repudiation: Timestamp + signer
    mac.update(&txn.timestamp.to_le_bytes());
    mac.update(&txn.signer.as_u64().to_le_bytes());

    // Chain link: Include previous hash
    mac.update(prev_hash);

    mac.finalize().into_bytes().into()
}
```

### GDPR (Data Processing Accountability)

**Requirement**: Track who processed personal data and when.

#### Implementation
```rust
#[cfg(feature = "keyed-hashing")]
struct GdprAuditEntry {
    data_subject_id: u64,
    processor_id: SignerId,
    operation: GdprOperation,
    timestamp: u64,
    hash: [u8; 32],
}

enum GdprOperation {
    Access,
    Rectification,
    Erasure,
    Export,
}

impl GdprAuditEntry {
    fn new(data_subject_id: u64, processor_id: SignerId, operation: GdprOperation) -> Self {
        let timestamp = current_timestamp();
        let hash = Self::compute_hash(data_subject_id, processor_id, &operation, timestamp);

        Self {
            data_subject_id,
            processor_id,
            operation,
            timestamp,
            hash,
        }
    }

    fn compute_hash(
        subject_id: u64,
        processor: SignerId,
        operation: &GdprOperation,
        timestamp: u64,
    ) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<Sha256>;

        let key = HmacKey::get_global();
        let mut mac = HmacSha256::new_from_slice(key).unwrap();

        mac.update(&subject_id.to_le_bytes());
        mac.update(&processor.as_u64().to_le_bytes());
        mac.update(&operation_to_bytes(operation));
        mac.update(&timestamp.to_le_bytes());

        mac.finalize().into_bytes().into()
    }
}

fn operation_to_bytes(op: &GdprOperation) -> [u8; 8] {
    match op {
        GdprOperation::Access => 1u64.to_le_bytes(),
        GdprOperation::Rectification => 2u64.to_le_bytes(),
        GdprOperation::Erasure => 3u64.to_le_bytes(),
        GdprOperation::Export => 4u64.to_le_bytes(),
    }
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}
```

---

## Anti-Patterns & Common Mistakes

### Anti-Pattern 1: Using const_hash for Passwords

```rust
// ❌ CRITICAL VULNERABILITY
const PASSWORD_HASH: u64 = const_fast_hash(b"admin123");

fn verify_password(input: &str) -> bool {
    const_fast_hash(input.as_bytes()) == PASSWORD_HASH  // INSECURE!
}

// ✅ CORRECT: Use argon2 or bcrypt
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::rngs::OsRng;

fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2.hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

fn verify_password_secure(password: &str, hash_str: &str) -> bool {
    let parsed_hash = PasswordHash::new(hash_str).unwrap();
    Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
}
```

### Anti-Pattern 2: Multiple Writers to AtomicHash256

```rust
// ❌ RACE CONDITION (violates SWeMR)
let hash = Arc::new(AtomicHash256::new([0; 32]));

thread::spawn({
    let h = Arc::clone(&hash);
    move || h.store([0xFF; 32])  // Writer 1
});

thread::spawn({
    let h = Arc::clone(&hash);
    move || h.store([0x00; 32])  // Writer 2 (RACE!)
});

// ✅ CORRECT: Single writer via channel
let (tx, rx) = mpsc::channel();

thread::spawn(move || {
    while let Ok(value) = rx.recv() {
        hash.store(value);  // Single writer
    }
});

// Multiple senders
let tx1 = tx.clone();
thread::spawn(move || tx1.send([0xFF; 32]).unwrap());
thread::spawn(move || tx.send([0x00; 32]).unwrap());
```

### Anti-Pattern 3: Always Using SIMD (Ignoring Threshold)

```rust
// ❌ SLOWER for <4 fields
#[cfg(feature = "simd-hashing")]
fn hash_2_fields(a: u64, b: u64) -> u64 {
    simd_fast_hash_multi(&[a, b])  // 12ns (SLOWER than 8ns scalar!)
}

// ✅ CORRECT: Auto dispatcher
fn hash_2_fields_correct(a: u64, b: u64) -> u64 {
    best_hash(&[a, b])  // 8ns (chooses scalar for <4 fields)
}
```

### Anti-Pattern 4: Forgetting HMAC Key Initialization

```rust
// ❌ PANIC at runtime
#[cfg(feature = "keyed-hashing")]
fn compute_audit_hash(data: &[u8]) -> [u8; 32] {
    let key = HmacKey::get_global();  // PANIC: key not initialized!
    // ...
}

// ✅ CORRECT: Initialize at startup
fn main() {
    #[cfg(feature = "keyed-hashing")]
    {
        let key = generate_secure_hmac_key();
        HmacKey::init_global(&key);
    }

    // Now safe to use keyed hashing
    run_application();
}
```

### Anti-Pattern 5: Trusting Hash Equality for Security

```rust
// ❌ TIMING ATTACK VULNERABLE
fn verify_hash(stored: u64, computed: u64) -> bool {
    stored == computed  // Vulnerable to timing attacks!
}

// ✅ CORRECT: Constant-time comparison (for crypto hashes)
fn verify_hash_crypto(stored: &[u8; 32], computed: &[u8; 32]) -> bool {
    use subtle::ConstantTimeEq;
    stored.ct_eq(computed).into()
}
```

---

## Summary Checklist

### Security
- [ ] NOT using `const_hash` for passwords
- [ ] NOT using `const_hash` for user-controlled inputs
- [ ] HMAC key generated with crypto-secure RNG
- [ ] HMAC key stored encrypted at rest
- [ ] Key rotation implemented (90-day schedule)
- [ ] SWeMR pattern enforced for `AtomicHash256`
- [ ] Constant-time comparison for crypto hashes

### Performance
- [ ] SIMD threshold respected (4+ fields)
- [ ] Auto dispatcher used (`best_hash`)
- [ ] Const hash count reasonable (<100 IDs)
- [ ] Binary size trade-offs documented
- [ ] B32 benchmarks validated (95% CI, 1000+ iterations)

### Testing
- [ ] Unit tests (determinism, different inputs)
- [ ] Property tests (if applicable)
- [ ] Concurrent tests (for Atomic*)
- [ ] Torn read tests (for AtomicHash256)
- [ ] Collision detection (compile-time assertions)

### Compliance
- [ ] SOX: HMAC-SHA256 + timestamp + signer
- [ ] SOC2: Audit trail integrity
- [ ] GDPR: Processing accountability
- [ ] HIPAA: Data integrity verification
- [ ] Key rotation documented and implemented

---

**End of Document** - Version 1.0.0 (2025-10-19)
**Classification**: INTERNAL - Trade Secret Protection
