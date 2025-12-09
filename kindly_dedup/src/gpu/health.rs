//! GPU Health Check Capsule - T1 Atomic Tier
//!
//! Fast (<20ns) GPU health monitoring using atomic bitmask for lockfree capability tracking.
//! Provides real-time health status for GPU operations with graceful degradation support.
//!
//! # Architecture (T1 Atomic)
//!
//! Single AtomicU64 packed state:
//! - Bits 0-7: Health flags (6 capability flags)
//! - Bits 8-31: Last check timestamp (24-bit, seconds mod 2^24)
//! - Bits 32-63: Generation counter (Q34 audit trail)
//!
//! # Performance Targets (B32)
//!
//! - check_health(): <20ns (atomic load)
//! - set_flag()/clear_flag(): <50ns (CAS)
//! - is_healthy(): <20ns (load + mask)
//!
//! # Framework Compliance
//!
//! - UCE34: T1 Atomic tier (lockfree bitmask)
//! - Chaos: 64B cache-aligned, no mutex, generation counter
//! - ASSUM: All assumptions documented (#ASSUME/#VERIFY tags)
//! - B32: <20ns latency target
//! - T28: Unit + property + integration tests

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// GPU Health capability flags (6 bits)
///
/// Each flag represents a specific GPU capability that can be
/// independently checked and tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuHealthFlags(u8);

impl GpuHealthFlags {
    /// GPU device is responding to commands
    pub const DEVICE_AVAILABLE: Self = Self(0b00000001);
    /// Sufficient VRAM available for operations
    pub const MEMORY_OK: Self = Self(0b00000010);
    /// Compute pipeline is functional
    pub const COMPUTE_OK: Self = Self(0b00000100);
    /// Buffer mapping operations working
    pub const BUFFER_MAP_OK: Self = Self(0b00001000);
    /// Shaders compiled successfully
    pub const SHADER_OK: Self = Self(0b00010000);
    /// No recent timeout errors
    pub const TIMEOUT_OK: Self = Self(0b00100000);

    /// All health checks passing
    pub const ALL_OK: Self = Self(0b00111111);

    /// No flags set (completely unhealthy)
    pub const NONE: Self = Self(0);

    /// Get raw flag value
    #[inline]
    pub fn bits(self) -> u8 {
        self.0
    }

    /// Create from raw bits
    #[inline]
    pub fn from_bits(bits: u8) -> Self {
        Self(bits & Self::ALL_OK.0) // Mask to valid flags only
    }

    /// Check if flag contains another flag
    #[inline]
    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Union of two flag sets
    #[inline]
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Intersection of two flag sets
    #[inline]
    pub fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Complement (flip all bits within ALL_OK mask)
    #[inline]
    pub fn complement(self) -> Self {
        Self(!self.0 & Self::ALL_OK.0)
    }

    /// Human-readable flag name
    pub fn name(&self) -> &'static str {
        match self.0 {
            0b00000001 => "DEVICE_AVAILABLE",
            0b00000010 => "MEMORY_OK",
            0b00000100 => "COMPUTE_OK",
            0b00001000 => "BUFFER_MAP_OK",
            0b00010000 => "SHADER_OK",
            0b00100000 => "TIMEOUT_OK",
            0b00111111 => "ALL_OK",
            0 => "NONE",
            _ => "MIXED",
        }
    }

    /// Iterator over individual set flags
    pub fn iter_set(self) -> impl Iterator<Item = GpuHealthFlags> {
        [
            Self::DEVICE_AVAILABLE,
            Self::MEMORY_OK,
            Self::COMPUTE_OK,
            Self::BUFFER_MAP_OK,
            Self::SHADER_OK,
            Self::TIMEOUT_OK,
        ]
        .into_iter()
        .filter(move |&flag| self.contains(flag))
    }
}

impl std::ops::BitOr for GpuHealthFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitAnd for GpuHealthFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        self.intersection(rhs)
    }
}

impl std::ops::Not for GpuHealthFlags {
    type Output = Self;
    fn not(self) -> Self {
        self.complement()
    }
}

/// GPU Health Capsule - T1 Atomic Tier
///
/// 64-byte cache-aligned health monitor for GPU capability tracking.
/// Uses atomic bitmask for lockfree flag operations.
///
/// # Layout (64 bytes)
///
/// - Bytes 0-7: packed_state (AtomicU64)
///   - Bits 0-7: health flags
///   - Bits 8-31: timestamp (seconds mod 2^24)
///   - Bits 32-63: generation counter
/// - Bytes 8-15: failure_counts (AtomicU64) - per-flag failure counters
/// - Bytes 16-23: last_healthy_ns (AtomicU64) - last time ALL_OK
/// - Bytes 24-31: check_count (AtomicU64) - total health checks
/// - Bytes 32-63: _padding for cache line alignment
///
/// # ASSUM Safety
///
/// - `#ASSUME_FLAGS_ATOMIC`: AtomicU64 provides lockfree flag updates
/// - `#VERIFY_FLAGS_ATOMIC`: Atomic operations ensure consistency
/// - `#ASSUME_TIMESTAMP_MOD`: 24-bit timestamp wraps every ~194 days
/// - `#VERIFY_TIMESTAMP_MOD`: Only used for relative timing
#[repr(C, align(64))]
pub struct GpuHealthCapsule {
    /// Packed state: flags (8) + timestamp (24) + generation (32)
    packed_state: AtomicU64,

    /// Packed failure counts: 8 bits per flag (6 flags = 48 bits used)
    failure_counts: AtomicU64,

    /// Last time all flags were healthy (nanoseconds since epoch)
    last_healthy_ns: AtomicU64,

    /// Total health check count
    check_count: AtomicU64,

    /// Padding to 64-byte cache line
    _padding: [u8; 32],
}

// Bit-packing constants
const FLAGS_MASK: u64 = 0xFF;
const TIMESTAMP_SHIFT: u64 = 8;
const TIMESTAMP_MASK: u64 = 0xFFFFFF;
const GEN_SHIFT: u64 = 32;

impl GpuHealthCapsule {
    /// Create new health capsule with all flags cleared
    ///
    /// Starts in unhealthy state - flags must be set as capabilities are verified.
    pub const fn new() -> Self {
        Self {
            packed_state: AtomicU64::new(0),
            failure_counts: AtomicU64::new(0),
            last_healthy_ns: AtomicU64::new(0),
            check_count: AtomicU64::new(0),
            _padding: [0; 32],
        }
    }

    /// Create new health capsule with all flags set (assume healthy)
    pub fn new_healthy() -> Self {
        let capsule = Self::new();
        capsule.set_all_flags();
        capsule
    }

    /// Get current timestamp in seconds mod 2^24
    #[inline]
    fn now_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::ZERO)
            .as_secs()
            & TIMESTAMP_MASK
    }

    /// Get current time in nanoseconds
    #[inline]
    fn now_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::ZERO)
            .as_nanos() as u64
    }

    /// Pack flags, timestamp, and generation into u64
    #[inline]
    fn pack(flags: u8, timestamp: u64, generation: u32) -> u64 {
        (flags as u64)
            | ((timestamp & TIMESTAMP_MASK) << TIMESTAMP_SHIFT)
            | ((generation as u64) << GEN_SHIFT)
    }

    /// Unpack flags from packed state
    #[inline]
    fn unpack_flags(packed: u64) -> GpuHealthFlags {
        GpuHealthFlags::from_bits((packed & FLAGS_MASK) as u8)
    }

    /// Unpack generation from packed state
    #[inline]
    fn unpack_generation(packed: u64) -> u32 {
        (packed >> GEN_SHIFT) as u32
    }

    /// Check current health flags (<20ns)
    ///
    /// Returns the current set of healthy capabilities.
    #[inline]
    pub fn check_health(&self) -> GpuHealthFlags {
        self.check_count.fetch_add(1, Ordering::Relaxed);
        let packed = self.packed_state.load(Ordering::Acquire);
        Self::unpack_flags(packed)
    }

    /// Set a health flag (mark capability as healthy)
    ///
    /// Uses atomic OR for lockfree flag set.
    pub fn set_flag(&self, flag: GpuHealthFlags) {
        let mut current = self.packed_state.load(Ordering::Acquire);
        loop {
            let flags = Self::unpack_flags(current);
            let gen = Self::unpack_generation(current);
            let new_flags = flags.union(flag);
            let new_packed = Self::pack(new_flags.bits(), Self::now_timestamp(), gen.wrapping_add(1));

            match self.packed_state.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Check if now fully healthy
                    if new_flags == GpuHealthFlags::ALL_OK {
                        self.last_healthy_ns.store(Self::now_ns(), Ordering::Release);
                    }
                    break;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Clear a health flag (mark capability as unhealthy)
    ///
    /// Uses atomic AND NOT for lockfree flag clear.
    pub fn clear_flag(&self, flag: GpuHealthFlags) {
        let mut current = self.packed_state.load(Ordering::Acquire);
        loop {
            let flags = Self::unpack_flags(current);
            let gen = Self::unpack_generation(current);
            let new_flags = GpuHealthFlags::from_bits(flags.bits() & !flag.bits());
            let new_packed = Self::pack(new_flags.bits(), Self::now_timestamp(), gen.wrapping_add(1));

            match self.packed_state.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Increment failure count for this flag
                    self.increment_failure_count(flag);
                    break;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Set all flags to healthy
    pub fn set_all_flags(&self) {
        self.set_flag(GpuHealthFlags::ALL_OK);
    }

    /// Clear all flags (mark all unhealthy)
    pub fn clear_all_flags(&self) {
        let current = self.packed_state.load(Ordering::Acquire);
        let gen = Self::unpack_generation(current);
        let new_packed = Self::pack(0, Self::now_timestamp(), gen.wrapping_add(1));
        self.packed_state.store(new_packed, Ordering::Release);
    }

    /// Increment failure count for a flag
    fn increment_failure_count(&self, flag: GpuHealthFlags) {
        // Each flag gets 8 bits of failure counter
        let shift = match flag.bits() {
            0b00000001 => 0,  // DEVICE_AVAILABLE
            0b00000010 => 8,  // MEMORY_OK
            0b00000100 => 16, // COMPUTE_OK
            0b00001000 => 24, // BUFFER_MAP_OK
            0b00010000 => 32, // SHADER_OK
            0b00100000 => 40, // TIMEOUT_OK
            _ => return,      // Mixed flags - don't increment
        };
        self.failure_counts.fetch_add(1 << shift, Ordering::Relaxed);
    }

    /// Check if GPU is fully healthy (ALL_OK)
    #[inline]
    pub fn is_healthy(&self) -> bool {
        let packed = self.packed_state.load(Ordering::Acquire);
        Self::unpack_flags(packed) == GpuHealthFlags::ALL_OK
    }

    /// Check if a specific capability is healthy
    #[inline]
    pub fn has_capability(&self, flag: GpuHealthFlags) -> bool {
        self.check_health().contains(flag)
    }

    /// Get list of failed health checks
    pub fn failed_checks(&self) -> Vec<&'static str> {
        let current = self.check_health();
        let missing = GpuHealthFlags::ALL_OK.bits() & !current.bits();

        let mut failures = Vec::new();
        if missing & GpuHealthFlags::DEVICE_AVAILABLE.bits() != 0 {
            failures.push("GPU device not available");
        }
        if missing & GpuHealthFlags::MEMORY_OK.bits() != 0 {
            failures.push("Insufficient GPU memory");
        }
        if missing & GpuHealthFlags::COMPUTE_OK.bits() != 0 {
            failures.push("Compute pipeline not functional");
        }
        if missing & GpuHealthFlags::BUFFER_MAP_OK.bits() != 0 {
            failures.push("Buffer mapping not working");
        }
        if missing & GpuHealthFlags::SHADER_OK.bits() != 0 {
            failures.push("Shader compilation failed");
        }
        if missing & GpuHealthFlags::TIMEOUT_OK.bits() != 0 {
            failures.push("Recent timeout detected");
        }
        failures
    }

    /// Get generation counter (for Q34 audit trail)
    #[inline]
    pub fn generation(&self) -> u32 {
        let packed = self.packed_state.load(Ordering::Acquire);
        Self::unpack_generation(packed)
    }

    /// Get total check count
    #[inline]
    pub fn check_count(&self) -> u64 {
        self.check_count.load(Ordering::Relaxed)
    }

    /// Get failure count for a specific flag
    pub fn failure_count(&self, flag: GpuHealthFlags) -> u8 {
        let counts = self.failure_counts.load(Ordering::Relaxed);
        let shift = match flag.bits() {
            0b00000001 => 0,
            0b00000010 => 8,
            0b00000100 => 16,
            0b00001000 => 24,
            0b00010000 => 32,
            0b00100000 => 40,
            _ => return 0,
        };
        ((counts >> shift) & 0xFF) as u8
    }

    /// Get time since last fully healthy state (in seconds)
    pub fn seconds_since_healthy(&self) -> u64 {
        let last = self.last_healthy_ns.load(Ordering::Acquire);
        if last == 0 {
            return u64::MAX; // Never been healthy
        }
        let now = Self::now_ns();
        (now.saturating_sub(last)) / 1_000_000_000
    }

    /// Get health summary as string
    pub fn summary(&self) -> String {
        let flags = self.check_health();
        let healthy_count = flags.bits().count_ones();
        format!(
            "GPU Health: {}/6 checks passing ({})",
            healthy_count,
            if self.is_healthy() { "HEALTHY" } else { "DEGRADED" }
        )
    }
}

impl Default for GpuHealthCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Verify 64-byte size at compile time
const _: () = assert!(std::mem::size_of::<GpuHealthCapsule>() == 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let health = GpuHealthCapsule::new();
        assert!(!health.is_healthy());
        assert_eq!(health.check_health(), GpuHealthFlags::NONE);
    }

    #[test]
    fn test_new_healthy() {
        let health = GpuHealthCapsule::new_healthy();
        assert!(health.is_healthy());
        assert_eq!(health.check_health(), GpuHealthFlags::ALL_OK);
    }

    #[test]
    fn test_set_flag() {
        let health = GpuHealthCapsule::new();
        health.set_flag(GpuHealthFlags::DEVICE_AVAILABLE);
        assert!(health.has_capability(GpuHealthFlags::DEVICE_AVAILABLE));
        assert!(!health.is_healthy()); // Only 1 of 6 flags
    }

    #[test]
    fn test_clear_flag() {
        let health = GpuHealthCapsule::new_healthy();
        assert!(health.is_healthy());

        health.clear_flag(GpuHealthFlags::MEMORY_OK);
        assert!(!health.is_healthy());
        assert!(!health.has_capability(GpuHealthFlags::MEMORY_OK));
        assert!(health.has_capability(GpuHealthFlags::DEVICE_AVAILABLE));
    }

    #[test]
    fn test_all_flags() {
        let health = GpuHealthCapsule::new();

        // Set each flag individually
        health.set_flag(GpuHealthFlags::DEVICE_AVAILABLE);
        health.set_flag(GpuHealthFlags::MEMORY_OK);
        health.set_flag(GpuHealthFlags::COMPUTE_OK);
        health.set_flag(GpuHealthFlags::BUFFER_MAP_OK);
        health.set_flag(GpuHealthFlags::SHADER_OK);
        health.set_flag(GpuHealthFlags::TIMEOUT_OK);

        assert!(health.is_healthy());
    }

    #[test]
    fn test_failed_checks() {
        let health = GpuHealthCapsule::new();
        health.set_flag(GpuHealthFlags::DEVICE_AVAILABLE);
        health.set_flag(GpuHealthFlags::COMPUTE_OK);

        let failures = health.failed_checks();
        assert_eq!(failures.len(), 4);
        assert!(failures.contains(&"Insufficient GPU memory"));
        assert!(failures.contains(&"Buffer mapping not working"));
    }

    #[test]
    fn test_generation_increments() {
        let health = GpuHealthCapsule::new();
        assert_eq!(health.generation(), 0);

        health.set_flag(GpuHealthFlags::DEVICE_AVAILABLE);
        assert_eq!(health.generation(), 1);

        health.clear_flag(GpuHealthFlags::DEVICE_AVAILABLE);
        assert_eq!(health.generation(), 2);
    }

    #[test]
    fn test_failure_count() {
        let health = GpuHealthCapsule::new_healthy();

        health.clear_flag(GpuHealthFlags::TIMEOUT_OK);
        health.set_flag(GpuHealthFlags::TIMEOUT_OK);
        health.clear_flag(GpuHealthFlags::TIMEOUT_OK);

        assert_eq!(health.failure_count(GpuHealthFlags::TIMEOUT_OK), 2);
        assert_eq!(health.failure_count(GpuHealthFlags::DEVICE_AVAILABLE), 0);
    }

    #[test]
    fn test_flag_operations() {
        let a = GpuHealthFlags::DEVICE_AVAILABLE;
        let b = GpuHealthFlags::MEMORY_OK;

        let union = a | b;
        assert!(union.contains(a));
        assert!(union.contains(b));

        let intersection = union & a;
        assert!(intersection.contains(a));
        assert!(!intersection.contains(b));
    }

    #[test]
    fn test_cache_alignment() {
        assert_eq!(std::mem::size_of::<GpuHealthCapsule>(), 64);
        assert_eq!(std::mem::align_of::<GpuHealthCapsule>(), 64);
    }

    #[test]
    fn test_check_count() {
        let health = GpuHealthCapsule::new();
        assert_eq!(health.check_count(), 0);

        health.check_health();
        health.check_health();
        health.check_health();

        assert_eq!(health.check_count(), 3);
    }

    #[test]
    fn test_summary() {
        let health = GpuHealthCapsule::new_healthy();
        let summary = health.summary();
        assert!(summary.contains("6/6"));
        assert!(summary.contains("HEALTHY"));

        health.clear_flag(GpuHealthFlags::MEMORY_OK);
        let summary = health.summary();
        assert!(summary.contains("5/6"));
        assert!(summary.contains("DEGRADED"));
    }
}
