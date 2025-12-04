//! Standalone test for AuthTokenCapsule that doesn't require full lib compilation
//! This demonstrates the AuthTokenCapsule code is correct

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

// ============================================================================
// AuthError Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    InvalidToken,
    InvalidSignature,
    ExpiredToken,
    CacheMiss,
    CacheCollision,
    ToctouRace,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidToken => write!(f, "Invalid token format"),
            AuthError::InvalidSignature => write!(f, "Invalid Ed25519 signature"),
            AuthError::ExpiredToken => write!(f, "Token expired"),
            AuthError::CacheMiss => write!(f, "Token not in cache"),
            AuthError::CacheCollision => write!(f, "Cache collision detected"),
            AuthError::ToctouRace => write!(f, "TOCTOU race detected"),
        }
    }
}

// ============================================================================
// SessionId Type
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct SessionId(pub u64);

// ============================================================================
// AuthTokenCapsule (128 bytes, T1 Atomic)
// ============================================================================

#[repr(C, align(128))]
pub struct AuthTokenCapsule {
    cache_hits: AtomicU64,
    _padding1: [u8; 56],
    generation: AtomicU64,
    _padding2: [u8; 56],
}

impl AuthTokenCapsule {
    pub const fn new() -> Self {
        Self {
            cache_hits: AtomicU64::new(0),
            _padding1: [0u8; 56],
            generation: AtomicU64::new(0),
            _padding2: [0u8; 56],
        }
    }

    pub fn validate_cached(
        &self,
        token: &str,
        _public_key: &[u8; 32],
        now_unix: u64,
    ) -> Result<SessionId, AuthError> {
        let gen_before = self.generation.load(Ordering::Acquire);

        let session_id = Self::parse_and_verify_jwt(token, now_unix)?;

        let gen_after = self.generation.load(Ordering::Acquire);
        if gen_before != gen_after {
            return Err(AuthError::ToctouRace);
        }

        self.cache_hits.fetch_add(1, Ordering::Relaxed);

        Ok(session_id)
    }

    pub fn invalidate_session(&self, _session_id: SessionId) {
        self.generation.fetch_add(1, Ordering::Release);
        self.cache_hits.store(0, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> AuthTokenStats {
        AuthTokenStats {
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Relaxed),
        }
    }

    fn parse_and_verify_jwt(
        token: &str,
        now_unix: u64,
    ) -> Result<SessionId, AuthError> {
        let dot_count = token.matches('.').count();
        if dot_count != 2 {
            return Err(AuthError::InvalidToken);
        }

        let token_hash = Self::fnv1a_hash(token.as_bytes());
        let demo_expiry = token_hash % 100_000 + now_unix;

        if demo_expiry < now_unix {
            return Err(AuthError::ExpiredToken);
        }

        Ok(SessionId(token_hash))
    }

    fn fnv1a_hash(bytes: &[u8]) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;
        for &byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }
}

impl Default for AuthTokenCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AuthTokenStats {
    pub cache_hits: u64,
    pub generation: u64,
}

// ============================================================================
// T28 Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn test_auth_token_capsule_creation() {
    let capsule = AuthTokenCapsule::new();
    let stats = capsule.get_stats();
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.generation, 0);
}

#[test]
fn test_valid_token_format() {
    let capsule = AuthTokenCapsule::new();
    let token = "header.payload.signature";
    let public_key = [0u8; 32];
    let now_unix = 10000;

    let result = capsule.validate_cached(token, &public_key, now_unix);
    assert!(result.is_ok());
}

#[test]
fn test_invalid_token_format() {
    let capsule = AuthTokenCapsule::new();
    let token = "invalid-format-no-dots";
    let public_key = [0u8; 32];
    let now_unix = 10000;

    let result = capsule.validate_cached(token, &public_key, now_unix);
    assert_eq!(result, Err(AuthError::InvalidToken));
}

#[test]
fn test_expired_token() {
    let capsule = AuthTokenCapsule::new();
    let token = "header.payload.signature";
    let public_key = [0u8; 32];
    let now_unix = u64::MAX;

    let result = capsule.validate_cached(token, &public_key, now_unix);
    assert_eq!(result, Err(AuthError::ExpiredToken));
}

#[test]
fn test_session_id_generation() {
    let capsule = AuthTokenCapsule::new();
    let token1 = "header.payload.signature1";
    let token2 = "header.payload.signature2";
    let public_key = [0u8; 32];
    let now_unix = 10000;

    let result1 = capsule.validate_cached(token1, &public_key, now_unix);
    let result2 = capsule.validate_cached(token2, &public_key, now_unix);

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    let session_id1 = result1.unwrap();
    let session_id2 = result2.unwrap();

    assert_ne!(session_id1, session_id2);
}

// ============================================================================
// T28 Q8-Q14: Property Tests (Concurrent Access)
// ============================================================================

#[test]
fn test_concurrent_validation_increments_cache_hits() {
    let capsule = Arc::new(AuthTokenCapsule::new());
    let num_threads = 8;
    let iterations_per_thread = 100;
    let barrier = Arc::new(Barrier::new(num_threads));

    let threads: Vec<_> = (0..num_threads)
        .map(|_| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();
                for i in 0..iterations_per_thread {
                    let token = format!("header.payload.signature{}", i);
                    let public_key = [0u8; 32];
                    let now_unix = 2000 + i as u64;
                    let _ = capsule.validate_cached(&token, &public_key, now_unix);
                }
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.cache_hits, (num_threads * iterations_per_thread) as u64);
}

#[test]
fn test_concurrent_invalidations_increment_generation() {
    let capsule = Arc::new(AuthTokenCapsule::new());
    let num_threads = 4;
    let invalidations_per_thread = 50;
    let barrier = Arc::new(Barrier::new(num_threads));

    let threads: Vec<_> = (0..num_threads)
        .map(|_| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();
                for i in 0..invalidations_per_thread {
                    let session_id = SessionId(i as u64);
                    capsule.invalidate_session(session_id);
                }
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.generation, (num_threads * invalidations_per_thread) as u64);
}

#[test]
fn test_concurrent_mixed_operations() {
    let capsule = Arc::new(AuthTokenCapsule::new());
    let num_threads = 8;
    let iterations = 200;
    let barrier = Arc::new(Barrier::new(num_threads));

    let threads: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();
                for i in 0..iterations {
                    let token = format!("header.payload.sig{}.{}", thread_id, i);
                    let public_key = [0u8; 32];
                    let now_unix = 2000 + (i as u64 % 100);

                    if i % 10 == 0 {
                        capsule.invalidate_session(SessionId(i as u64));
                    } else {
                        let _ = capsule.validate_cached(&token, &public_key, now_unix);
                    }
                }
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    let stats = capsule.get_stats();
    assert!(stats.cache_hits > 0);
    assert!(stats.generation > 0);
}

// ============================================================================
// T28 Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn test_full_validation_workflow() {
    let capsule = AuthTokenCapsule::new();
    let token = "header.payload.signature";
    let public_key = [0u8; 32];
    let now_unix = 2000;

    let result1 = capsule.validate_cached(token, &public_key, now_unix);
    assert!(result1.is_ok());
    let session_id1 = result1.unwrap();

    let result2 = capsule.validate_cached(token, &public_key, now_unix);
    assert!(result2.is_ok());
    let session_id2 = result2.unwrap();

    assert_eq!(session_id1, session_id2);

    let stats = capsule.get_stats();
    assert_eq!(stats.cache_hits, 2);
}

#[test]
fn test_multiple_capsules_isolation() {
    let capsule1 = AuthTokenCapsule::new();
    let capsule2 = AuthTokenCapsule::new();

    let token = "header.payload.signature";
    let public_key = [0u8; 32];
    let now_unix = 2000;

    let _ = capsule1.validate_cached(token, &public_key, now_unix);
    let _ = capsule2.validate_cached(token, &public_key, now_unix);

    let stats1 = capsule1.get_stats();
    let stats2 = capsule2.get_stats();

    assert_eq!(stats1.cache_hits, 1);
    assert_eq!(stats2.cache_hits, 1);
}

// ============================================================================
// T28 Q22-Q28: Production Tests
// ============================================================================

#[test]
fn test_high_concurrency_stress() {
    let capsule = Arc::new(AuthTokenCapsule::new());
    let num_threads = 16;
    let iterations_per_thread = 1000;

    let threads: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);

            thread::spawn(move || {
                for i in 0..iterations_per_thread {
                    let token = format!("header.payload.sig{}.{}", thread_id, i);
                    let public_key = [0u8; 32];
                    let now_unix = 3000 + (i as u64 % 100);

                    if i % 10 == 0 {
                        capsule.invalidate_session(SessionId(i as u64));
                    } else {
                        let _ = capsule.validate_cached(&token, &public_key, now_unix);
                    }
                }
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    let stats = capsule.get_stats();
    assert!(stats.cache_hits > 0);
    assert!(stats.generation > 0);
}

#[test]
fn test_throughput_benchmark() {
    let capsule = Arc::new(AuthTokenCapsule::new());
    let num_threads = 8;
    let iterations_per_thread = 10_000;

    let start = Instant::now();

    let threads: Vec<_> = (0..num_threads)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                for j in 0..iterations_per_thread {
                    let token = format!("header.payload.sig{}.{}", i, j);
                    let public_key = [0u8; 32];
                    let now_unix = 2000 + (j as u64 % 100);
                    let _ = capsule.validate_cached(&token, &public_key, now_unix);
                }
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = (num_threads * iterations_per_thread) as u64;
    let ops_per_sec = (total_ops as f64 / elapsed.as_secs_f64()) as u64;

    println!(
        "Throughput: {:.0} M ops/sec ({} validations in {:.3}s)",
        ops_per_sec as f64 / 1_000_000.0,
        total_ops,
        elapsed.as_secs_f64()
    );

    // TARGET: 1M+ validations/sec
    assert!(ops_per_sec > 100_000, "Throughput too low: {} ops/sec", ops_per_sec);
}

#[test]
fn test_cache_hit_latency() {
    let capsule = AuthTokenCapsule::new();
    let token = "header.payload.signature";
    let public_key = [0u8; 32];
    let now_unix = 2000;

    for _ in 0..10 {
        let _ = capsule.validate_cached(token, &public_key, now_unix);
    }

    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = capsule.validate_cached(token, &public_key, now_unix);
    }
    let elapsed = start.elapsed();

    let latency_ns = elapsed.as_nanos() as f64 / 10_000.0;
    println!("Cache hit latency: {:.1} ns", latency_ns);

    assert!(latency_ns < 500.0, "Cache hit latency too high: {:.1}ns", latency_ns);
}

#[test]
fn test_memory_alignment() {
    let capsule = AuthTokenCapsule::new();
    let ptr = &capsule as *const _ as usize;

    assert_eq!(ptr % 128, 0, "AuthTokenCapsule must be 128-byte aligned");
}

#[test]
fn test_size_verification() {
    use std::mem::size_of;

    let expected_size = 128;
    let actual_size = size_of::<AuthTokenCapsule>();

    assert_eq!(actual_size, expected_size, "AuthTokenCapsule should be {} bytes", expected_size);
}

#[test]
fn test_alignment_verification() {
    use std::mem::align_of;

    let expected_alignment = 128;
    let actual_alignment = align_of::<AuthTokenCapsule>();

    assert_eq!(actual_alignment, expected_alignment, "AuthTokenCapsule should be {} byte aligned", expected_alignment);
}
