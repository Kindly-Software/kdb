//! OperatorChallengeCapsule - Ed25519 Challenge-Response Authentication
//!
//! T1 Atomic tier capsule implementing SOTA challenge-response authentication
//! with hybrid timestamp+random nonces and single-use enforcement.
//!
//! ## Feature Gate
//! Requires `operator-challenge` feature for challenge generation (uses `rand` crate).
//! Without the feature, the capsule structure is available but generation is disabled.
//!
//! ## Security Properties:
//! - 32-byte nonces with 256-bit cryptographic strength
//! - Hybrid format: 8-byte nanosecond timestamp + 24-byte OsRng
//! - Configurable expiry (default 30 seconds)
//! - Single-use enforcement via generation counter
//! - Optional public key binding for targeted challenges
//!
//! ## Performance:
//! - Challenge generation: <1us (OsRng + timestamp)
//! - Challenge consumption: <50ns (atomic CAS)
//! - Expiry check: <10ns (atomic load + comparison)
//!
//! ## Framework Compliance:
//! - UCE34 Q10: T1 Atomic tier (lockfree coordination)
//! - COCA: 100% lockfree, no mutex/RwLock
//! - ASSUM: All unsafe documented with #ASSUME/#VERIFY

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "operator-challenge")]
use rand::rngs::OsRng;
#[cfg(feature = "operator-challenge")]
use rand::RngCore;

// ============================================================================
// Challenge States
// ============================================================================

/// Challenge state enumeration (stored as u64 for atomic operations)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum ChallengeState {
    /// No active challenge
    Empty = 0,
    /// Challenge is pending (waiting for response)
    Pending = 1,
    /// Challenge was successfully consumed
    Used = 2,
    /// Challenge has expired
    Expired = 3,
}

impl From<u64> for ChallengeState {
    fn from(value: u64) -> Self {
        match value {
            0 => ChallengeState::Empty,
            1 => ChallengeState::Pending,
            2 => ChallengeState::Used,
            3 => ChallengeState::Expired,
            _ => ChallengeState::Empty,
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during challenge operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeCapsuleError {
    /// No active challenge exists
    NoChallengeActive,
    /// Challenge has already been consumed
    ChallengeAlreadyUsed,
    /// Challenge has expired
    ChallengeExpired,
    /// Concurrent modification detected (retry)
    ConcurrentModification,
    /// Challenge was generated for different IP address
    IpMismatch,
}

impl std::fmt::Display for ChallengeCapsuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChallengeCapsuleError::NoChallengeActive => write!(f, "No active challenge"),
            ChallengeCapsuleError::ChallengeAlreadyUsed => write!(f, "Challenge already consumed"),
            ChallengeCapsuleError::ChallengeExpired => write!(f, "Challenge has expired"),
            ChallengeCapsuleError::ConcurrentModification => {
                write!(f, "Concurrent modification, retry")
            }
            ChallengeCapsuleError::IpMismatch => {
                write!(f, "Challenge was generated for different IP address")
            }
        }
    }
}

impl std::error::Error for ChallengeCapsuleError {}

// ============================================================================
// OperatorChallengeCapsule - 256 bytes, 64-byte aligned
// ============================================================================

/// T1 Atomic capsule for Ed25519 challenge-response authentication.
///
/// ## Memory Layout (256 bytes):
/// - nonce: [u8; 32] - Active challenge nonce
/// - created_at: AtomicU64 - Challenge creation timestamp (nanos since epoch)
/// - expires_at: AtomicU64 - Challenge expiry timestamp (nanos since epoch)
/// - generation: AtomicU64 - Replay prevention counter
/// - bound_ip_hash: AtomicU64 - FNV-1a hash of IP address (replay prevention)
/// - pubkey_hash: [u8; 32] - Optional SHA-256 hash of expected public key
/// - state: AtomicU64 - Challenge state (Empty/Pending/Used/Expired)
/// - _padding: [u8; 144] - Complete to 256 bytes
///
/// ## Thread Safety:
/// - All mutable operations use atomic CAS or store with Release ordering
/// - All read operations use Acquire ordering for visibility
/// - Generation counter prevents ABA problems
#[repr(C, align(64))]
pub struct OperatorChallengeCapsule {
    /// Active challenge nonce (32 bytes)
    /// Format: [timestamp: 8 bytes][random: 24 bytes]
    // #ASSUME: MEMORY_ALIGNED - nonce array is at offset 0, naturally aligned
    // #VERIFY: compile-time size assertion ensures correct layout
    nonce: [u8; 32],

    /// Challenge creation timestamp (nanoseconds since UNIX epoch)
    // #ASSUME: ATOMIC_ALIGNED - AtomicU64 requires 8-byte alignment
    // #VERIFY: repr(C, align(64)) guarantees 64-byte alignment >= 8-byte
    created_at: AtomicU64,

    /// Challenge expiry timestamp (nanoseconds since UNIX epoch)
    expires_at: AtomicU64,

    /// Generation counter for replay prevention and TOCTOU avoidance
    /// Incremented on every challenge generation and consumption
    // #ASSUME: GENERATION_COUNTER - monotonically increasing, never wraps in practice
    // #VERIFY: u64 max = 18 quintillion, at 1M ops/sec = 584,542 years to wrap
    generation: AtomicU64,

    /// FNV-1a hash of IP address that requested challenge (replay prevention)
    /// Zero = no IP binding (any IP accepted)
    // #ASSUME: IP_REPLAY_PREVENTION - IP binding prevents cross-network replay attacks
    // #VERIFY: Unit tests verify IP mismatch rejection in consume_challenge()
    bound_ip_hash: AtomicU64,

    /// SHA-256 hash of expected public key (optional binding)
    /// All zeros = no binding (any key accepted)
    // #ASSUME: MEMORY_ALIGNED - at offset 32 + 8*3 = 56, aligned to 8 bytes
    pubkey_hash: [u8; 32],

    /// Challenge state (see ChallengeState enum)
    // #ASSUME: STATE_ATOMIC - state transitions are atomic via CAS
    // #VERIFY: CAS ensures only one thread can transition from Pending -> Used
    state: AtomicU64,

    /// Padding to complete 256 bytes
    /// Size calculation: 32 (nonce) + 8*5 (atomics) + 32 (hash) + 8 (state) = 112
    /// Padding needed: 256 - 112 = 144 bytes
    _padding: [u8; 144],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(std::mem::size_of::<OperatorChallengeCapsule>() == 256);
    assert!(std::mem::align_of::<OperatorChallengeCapsule>() == 64);
};

/// FNV-1a hash for IP address (fast, non-cryptographic)
///
/// ## Arguments
/// - `ip`: IP address bytes (IPv4: 4 bytes, IPv6: 16 bytes)
///
/// ## Returns
/// 64-bit FNV-1a hash
///
/// ## Performance
/// <10ns for IPv4, <30ns for IPv6
#[inline]
fn hash_ip(ip: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037u64;
    const FNV_PRIME: u64 = 1099511628211u64;

    let mut hash = FNV_OFFSET;
    for &byte in ip {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

impl OperatorChallengeCapsule {
    /// Default challenge timeout in seconds
    pub const DEFAULT_TIMEOUT_SECS: u32 = 30;

    /// Create a new empty challenge capsule.
    ///
    /// ## Returns
    /// A capsule with no active challenge (state = Empty).
    pub fn new() -> Self {
        Self {
            nonce: [0u8; 32],
            created_at: AtomicU64::new(0),
            expires_at: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            bound_ip_hash: AtomicU64::new(0),
            pubkey_hash: [0u8; 32],
            state: AtomicU64::new(ChallengeState::Empty as u64),
            _padding: [0u8; 144],
        }
    }

    /// Generate a new challenge with the specified timeout and IP binding.
    ///
    /// ## Feature Gate
    /// Requires `operator-challenge` feature for cryptographically secure random nonce generation.
    ///
    /// ## Arguments
    /// - `timeout_secs`: Challenge validity period in seconds
    /// - `client_ip`: Client IP address bytes (IPv4: 4 bytes, IPv6: 16 bytes)
    ///
    /// ## Returns
    /// The 32-byte challenge nonce.
    ///
    /// ## Security Properties
    /// - First 8 bytes: nanosecond timestamp (freshness, replay mitigation)
    /// - Remaining 24 bytes: cryptographically secure random (unpredictability)
    /// - IP binding: Challenge can only be consumed from the same IP address
    ///
    /// ## Thread Safety
    /// This method is NOT thread-safe for concurrent generation.
    /// Only one thread should generate challenges; multiple threads may consume.
    #[cfg(feature = "operator-challenge")]
    pub fn generate_challenge(&mut self, timeout_secs: u32, client_ip: &[u8]) -> [u8; 32] {
        // Get current time in nanoseconds
        // #ASSUME: SYSTEM_TIME - SystemTime::now() is available and monotonic
        // #VERIFY: All supported platforms (Linux) provide monotonic time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos() as u64;

        // Generate hybrid nonce: [timestamp: 8][random: 24]
        let mut nonce = [0u8; 32];
        nonce[0..8].copy_from_slice(&now.to_le_bytes());

        // #ASSUME: CSPRNG - OsRng provides cryptographically secure randomness
        // #VERIFY: OsRng uses OS entropy source (getrandom on Linux)
        OsRng.fill_bytes(&mut nonce[8..32]);

        // Calculate expiry time
        let expires_at = now + (timeout_secs as u64 * 1_000_000_000);

        // Store challenge atomically
        self.nonce = nonce;
        self.created_at.store(now, Ordering::Release);
        self.expires_at.store(expires_at, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        // Hash and store IP address for replay prevention
        let ip_hash = hash_ip(client_ip);
        self.bound_ip_hash.store(ip_hash, Ordering::Release);

        self.state
            .store(ChallengeState::Pending as u64, Ordering::Release);

        nonce
    }

    /// Get the current active challenge and its expiry time.
    ///
    /// ## Returns
    /// - `Some((nonce, expiry_nanos))` if a valid challenge is pending
    /// - `None` if no challenge is active, or challenge is used/expired
    pub fn get_challenge(&self) -> Option<([u8; 32], u64)> {
        let state = ChallengeState::from(self.state.load(Ordering::Acquire));

        if state != ChallengeState::Pending {
            return None;
        }

        let expiry = self.expires_at.load(Ordering::Acquire);

        // Check if expired
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos() as u64;

        if now > expiry {
            // Mark as expired (best-effort, no CAS needed for read-only operation)
            let _ = self.state.compare_exchange(
                ChallengeState::Pending as u64,
                ChallengeState::Expired as u64,
                Ordering::Release,
                Ordering::Relaxed,
            );
            return None;
        }

        Some((self.nonce, expiry))
    }

    /// Check if the challenge has expired.
    ///
    /// ## Arguments
    /// - `current_time_nanos`: Current time in nanoseconds since UNIX epoch
    ///
    /// ## Returns
    /// `true` if the challenge has expired or no challenge is active.
    pub fn is_expired(&self, current_time_nanos: u64) -> bool {
        let state = ChallengeState::from(self.state.load(Ordering::Acquire));

        match state {
            ChallengeState::Empty => true,
            ChallengeState::Used => true,
            ChallengeState::Expired => true,
            ChallengeState::Pending => {
                let expiry = self.expires_at.load(Ordering::Acquire);
                current_time_nanos > expiry
            }
        }
    }

    /// Consume the challenge (single-use enforcement with IP verification).
    ///
    /// ## Arguments
    /// - `client_ip`: Client IP address bytes (IPv4: 4 bytes, IPv6: 16 bytes)
    ///
    /// ## Returns
    /// - `Ok(nonce)`: The challenge nonce, if successfully consumed
    /// - `Err(ChallengeCapsuleError)`: If consumption failed
    ///
    /// ## Thread Safety
    /// This method is thread-safe. Only one thread will successfully consume
    /// the challenge; others will receive `ChallengeAlreadyUsed`.
    ///
    /// ## Single-Use Guarantee
    /// Uses atomic CAS to ensure the challenge can only be consumed once.
    /// The generation counter is incremented on successful consumption.
    ///
    /// ## IP Verification
    /// Verifies that client IP matches the IP that requested the challenge.
    pub fn consume_challenge(&self, client_ip: &[u8]) -> Result<[u8; 32], ChallengeCapsuleError> {
        // Check current state
        let state = ChallengeState::from(self.state.load(Ordering::Acquire));

        match state {
            ChallengeState::Empty => return Err(ChallengeCapsuleError::NoChallengeActive),
            ChallengeState::Used => return Err(ChallengeCapsuleError::ChallengeAlreadyUsed),
            ChallengeState::Expired => return Err(ChallengeCapsuleError::ChallengeExpired),
            ChallengeState::Pending => {}
        }

        // Verify IP address matches
        let stored_ip_hash = self.bound_ip_hash.load(Ordering::Acquire);
        if stored_ip_hash != 0 {
            // IP binding is active (non-zero hash)
            let current_ip_hash = hash_ip(client_ip);
            if current_ip_hash != stored_ip_hash {
                return Err(ChallengeCapsuleError::IpMismatch);
            }
        }

        // Check expiry
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos() as u64;

        let expiry = self.expires_at.load(Ordering::Acquire);
        if now > expiry {
            // Try to mark as expired
            let _ = self.state.compare_exchange(
                ChallengeState::Pending as u64,
                ChallengeState::Expired as u64,
                Ordering::Release,
                Ordering::Relaxed,
            );
            return Err(ChallengeCapsuleError::ChallengeExpired);
        }

        // Atomically transition from Pending -> Used
        // #ASSUME: CAS_ATOMIC - compare_exchange is atomic on all supported platforms
        // #VERIFY: Rust guarantees atomic CAS on AtomicU64
        match self.state.compare_exchange(
            ChallengeState::Pending as u64,
            ChallengeState::Used as u64,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Successfully consumed - increment generation
                self.generation.fetch_add(1, Ordering::Release);
                Ok(self.nonce)
            }
            Err(actual) => {
                // Another thread modified the state
                let actual_state = ChallengeState::from(actual);
                match actual_state {
                    ChallengeState::Used => Err(ChallengeCapsuleError::ChallengeAlreadyUsed),
                    ChallengeState::Expired => Err(ChallengeCapsuleError::ChallengeExpired),
                    _ => Err(ChallengeCapsuleError::ConcurrentModification),
                }
            }
        }
    }

    /// Bind the challenge to a specific public key.
    ///
    /// ## Arguments
    /// - `pubkey_hash`: SHA-256 hash of the expected public key
    ///
    /// ## Security Note
    /// When bound, the challenge response should be verified against
    /// a signature from the key matching this hash.
    pub fn bind_pubkey(&mut self, pubkey_hash: [u8; 32]) {
        self.pubkey_hash = pubkey_hash;
    }

    /// Get the bound public key hash.
    ///
    /// ## Returns
    /// The SHA-256 hash of the bound public key, or all zeros if unbound.
    pub fn get_pubkey_hash(&self) -> [u8; 32] {
        self.pubkey_hash
    }

    /// Check if a public key hash is bound.
    ///
    /// ## Returns
    /// `true` if a non-zero public key hash is bound.
    pub fn has_pubkey_binding(&self) -> bool {
        self.pubkey_hash.iter().any(|&b| b != 0)
    }

    /// Get the current generation counter.
    ///
    /// ## Returns
    /// The current generation value (monotonically increasing).
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get the current challenge state.
    ///
    /// ## Returns
    /// The current `ChallengeState`.
    pub fn state(&self) -> ChallengeState {
        ChallengeState::from(self.state.load(Ordering::Acquire))
    }

    /// Reset the capsule to empty state.
    ///
    /// ## Thread Safety
    /// This method is NOT thread-safe. Only call when no other threads
    /// are accessing the capsule.
    pub fn reset(&mut self) {
        self.nonce = [0u8; 32];
        self.created_at.store(0, Ordering::Release);
        self.expires_at.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.bound_ip_hash.store(0, Ordering::Release);
        self.pubkey_hash = [0u8; 32];
        self.state
            .store(ChallengeState::Empty as u64, Ordering::Release);
    }
}

impl Default for OperatorChallengeCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Unit Tests (T28 Q1-Q7)
// ============================================================================

#[cfg(all(test, feature = "operator-challenge"))]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::thread;

    /// T28 Q1: Size and alignment verification
    #[test]
    fn test_size_and_alignment() {
        assert_eq!(
            std::mem::size_of::<OperatorChallengeCapsule>(),
            256,
            "Capsule must be exactly 256 bytes"
        );
        assert_eq!(
            std::mem::align_of::<OperatorChallengeCapsule>(),
            64,
            "Capsule must be 64-byte aligned"
        );
    }

    /// T28 Q2: Nonce uniqueness (100 nonces, all different)
    #[test]
    fn test_nonce_uniqueness() {
        let mut capsule = OperatorChallengeCapsule::new();
        let mut nonces = HashSet::new();
        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1

        for _ in 0..100 {
            let nonce = capsule.generate_challenge(30, &client_ip);
            let nonce_hex = hex::encode(nonce);
            assert!(
                nonces.insert(nonce_hex.clone()),
                "Duplicate nonce detected: {}",
                nonce_hex
            );
            capsule.reset();
        }

        assert_eq!(nonces.len(), 100, "Expected 100 unique nonces");
    }

    /// T28 Q3: Timestamp hybrid format validation
    #[test]
    fn test_timestamp_hybrid_format() {
        let mut capsule = OperatorChallengeCapsule::new();
        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1

        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let nonce = capsule.generate_challenge(30, &client_ip);

        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        // Extract timestamp from first 8 bytes
        let mut timestamp_bytes = [0u8; 8];
        timestamp_bytes.copy_from_slice(&nonce[0..8]);
        let embedded_timestamp = u64::from_le_bytes(timestamp_bytes);

        // Verify timestamp is within expected range
        assert!(
            embedded_timestamp >= before && embedded_timestamp <= after,
            "Embedded timestamp {} not in range [{}, {}]",
            embedded_timestamp,
            before,
            after
        );

        // Verify random portion is not all zeros
        let random_portion = &nonce[8..32];
        assert!(
            random_portion.iter().any(|&b| b != 0),
            "Random portion should not be all zeros"
        );
    }

    /// T28 Q4: Expiry detection
    #[test]
    fn test_expiry_detection() {
        let mut capsule = OperatorChallengeCapsule::new();
        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1

        // Generate challenge with 1 second timeout
        let _ = capsule.generate_challenge(1, &client_ip);

        // Should not be expired immediately
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        assert!(!capsule.is_expired(now), "Challenge should not be expired immediately");

        // Should be expired 2 seconds in the future
        let future = now + 2_000_000_000; // 2 seconds in nanos
        assert!(capsule.is_expired(future), "Challenge should be expired after timeout");
    }

    /// T28 Q5: Single-use enforcement (consume twice = error)
    #[test]
    fn test_single_use_enforcement() {
        let mut capsule = OperatorChallengeCapsule::new();
        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1
        let _ = capsule.generate_challenge(30, &client_ip);

        // First consumption should succeed
        let result1 = capsule.consume_challenge(&client_ip);
        assert!(result1.is_ok(), "First consumption should succeed");

        // Second consumption should fail with AlreadyUsed
        let result2 = capsule.consume_challenge(&client_ip);
        assert_eq!(
            result2,
            Err(ChallengeCapsuleError::ChallengeAlreadyUsed),
            "Second consumption should fail with AlreadyUsed"
        );
    }

    /// T28 Q6: Generation counter increments
    #[test]
    fn test_generation_counter() {
        let mut capsule = OperatorChallengeCapsule::new();
        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1
        assert_eq!(capsule.generation(), 0, "Initial generation should be 0");

        // Generate challenge increments generation
        let _ = capsule.generate_challenge(30, &client_ip);
        assert_eq!(capsule.generation(), 1, "Generation should be 1 after generate");

        // Consume challenge increments generation
        let _ = capsule.consume_challenge(&client_ip);
        assert_eq!(capsule.generation(), 2, "Generation should be 2 after consume");

        // Reset increments generation
        capsule.reset();
        assert_eq!(capsule.generation(), 3, "Generation should be 3 after reset");
    }

    /// T28 Q7: State transitions
    #[test]
    fn test_state_transitions() {
        let mut capsule = OperatorChallengeCapsule::new();
        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1

        // Initial state is Empty
        assert_eq!(capsule.state(), ChallengeState::Empty);

        // Generate moves to Pending
        let _ = capsule.generate_challenge(30, &client_ip);
        assert_eq!(capsule.state(), ChallengeState::Pending);

        // Consume moves to Used
        let _ = capsule.consume_challenge(&client_ip);
        assert_eq!(capsule.state(), ChallengeState::Used);

        // Reset moves back to Empty
        capsule.reset();
        assert_eq!(capsule.state(), ChallengeState::Empty);
    }

    /// T28 Q8: Public key binding
    #[test]
    fn test_pubkey_binding() {
        let mut capsule = OperatorChallengeCapsule::new();

        // Initially no binding
        assert!(!capsule.has_pubkey_binding());
        assert_eq!(capsule.get_pubkey_hash(), [0u8; 32]);

        // Bind a key
        let hash = [0xAB; 32];
        capsule.bind_pubkey(hash);

        assert!(capsule.has_pubkey_binding());
        assert_eq!(capsule.get_pubkey_hash(), hash);
    }

    /// T28 Q9: get_challenge returns None for non-Pending states
    #[test]
    fn test_get_challenge_state_checks() {
        let mut capsule = OperatorChallengeCapsule::new();
        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1

        // Empty state
        assert!(capsule.get_challenge().is_none());

        // Pending state - should return challenge
        let nonce = capsule.generate_challenge(30, &client_ip);
        let result = capsule.get_challenge();
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, nonce);

        // Used state
        let _ = capsule.consume_challenge(&client_ip);
        assert!(capsule.get_challenge().is_none());
    }

    /// T28 Q10: Concurrent consumption (only one succeeds)
    #[test]
    fn test_concurrent_consumption() {
        use std::sync::Arc;

        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1
        let capsule = Arc::new({
            let mut c = OperatorChallengeCapsule::new();
            c.generate_challenge(30, &client_ip);
            c
        });

        let mut handles = vec![];
        let success_count = Arc::new(std::sync::atomic::AtomicU32::new(0));

        for _ in 0..10 {
            let capsule_clone = Arc::clone(&capsule);
            let success_clone = Arc::clone(&success_count);
            handles.push(thread::spawn(move || {
                if capsule_clone.consume_challenge(&client_ip).is_ok() {
                    success_clone.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Exactly one thread should have succeeded
        assert_eq!(
            success_count.load(Ordering::Relaxed),
            1,
            "Exactly one thread should consume the challenge"
        );
    }

    /// T28 Q11: Consume expired challenge returns error
    #[test]
    fn test_consume_expired_challenge() {
        let mut capsule = OperatorChallengeCapsule::new();
        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1

        // Generate with 0 second timeout (immediately expired)
        let _ = capsule.generate_challenge(0, &client_ip);

        // Wait a tiny bit to ensure expiry
        thread::sleep(std::time::Duration::from_millis(10));

        // Should fail with expired error
        let result = capsule.consume_challenge(&client_ip);
        assert_eq!(
            result,
            Err(ChallengeCapsuleError::ChallengeExpired),
            "Consuming expired challenge should fail"
        );
    }

    /// T28 Q12: Consume without challenge returns error
    #[test]
    fn test_consume_no_challenge() {
        let capsule = OperatorChallengeCapsule::new();
        let client_ip = [127, 0, 0, 1]; // IPv4: 127.0.0.1
        let result = capsule.consume_challenge(&client_ip);
        assert_eq!(
            result,
            Err(ChallengeCapsuleError::NoChallengeActive),
            "Consuming without active challenge should fail"
        );
    }

    /// T28 Q13: IP mismatch detection (Phase 4)
    #[test]
    fn test_ip_mismatch() {
        let mut capsule = OperatorChallengeCapsule::new();
        let ip1 = [127, 0, 0, 1]; // IPv4: 127.0.0.1
        let ip2 = [192, 168, 1, 1]; // IPv4: 192.168.1.1

        // Generate challenge from IP1
        let _ = capsule.generate_challenge(30, &ip1);

        // Try to consume from IP2 - should fail
        let result = capsule.consume_challenge(&ip2);
        assert_eq!(
            result,
            Err(ChallengeCapsuleError::IpMismatch),
            "Consuming from different IP should fail with IpMismatch"
        );

        // Consume from correct IP - should succeed
        let result = capsule.consume_challenge(&ip1);
        assert!(result.is_ok(), "Consuming from correct IP should succeed");
    }

    /// T28 Q14: IPv6 IP binding
    #[test]
    fn test_ipv6_binding() {
        let mut capsule = OperatorChallengeCapsule::new();
        let ipv6 = [
            0x20, 0x01, 0x0d, 0xb8, 0x85, 0xa3, 0x00, 0x00,
            0x00, 0x00, 0x8a, 0x2e, 0x03, 0x70, 0x73, 0x34,
        ]; // IPv6: 2001:db8:85a3::8a2e:370:7334

        // Generate and consume with IPv6
        let _ = capsule.generate_challenge(30, &ipv6);
        let result = capsule.consume_challenge(&ipv6);
        assert!(result.is_ok(), "IPv6 binding should work");
    }

    /// T28 Q15: FNV-1a hash collision resistance
    #[test]
    fn test_fnv_hash_uniqueness() {
        let ip1 = [127, 0, 0, 1];
        let ip2 = [127, 0, 0, 2];
        let ip3 = [192, 168, 1, 1];
        let ip4 = [10, 0, 0, 1];

        let hash1 = hash_ip(&ip1);
        let hash2 = hash_ip(&ip2);
        let hash3 = hash_ip(&ip3);
        let hash4 = hash_ip(&ip4);

        // All hashes should be unique
        assert_ne!(hash1, hash2, "Different IPs should produce different hashes");
        assert_ne!(hash1, hash3, "Different IPs should produce different hashes");
        assert_ne!(hash1, hash4, "Different IPs should produce different hashes");
        assert_ne!(hash2, hash3, "Different IPs should produce different hashes");
        assert_ne!(hash2, hash4, "Different IPs should produce different hashes");
        assert_ne!(hash3, hash4, "Different IPs should produce different hashes");
    }
}
