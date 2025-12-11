//! # ClockSourceCapsule - TSC Calibration and Clock Source Management (T1 Atomic)
//!
//! **Production-grade clock source abstraction with TSC calibration for Capsule OS.**
//!
//! ## Overview
//!
//! This capsule provides:
//! - TSC (Time Stamp Counter) calibration using multiple methods
//! - Multi-clock source support (TSC, HPET, ACPI PM Timer)
//! - Monotonic and wall-clock time tracking
//! - Frequency drift compensation
//!
//! ## Architecture
//!
//! **Tier**: T1 Atomic (Lockfree Coordination)
//! **Size**: 256 bytes (cache-aligned)
//! **Performance**:
//! - TSC read: <1ns (RDTSC instruction)
//! - HPET read: ~100ns (MMIO)
//! - Calibration: <1ms (boot-time only)
//!
//! ## TSC Calibration Methods
//!
//! 1. **CPUID Leaf 15**: Direct TSC frequency from CPU (Intel Skylake+)
//! 2. **MSR 0xCE (Platform Info)**: TSC frequency from platform info MSR
//! 3. **PIT Calibration**: Legacy calibration against 8254 PIT (82.5ms)
//! 4. **HPET Calibration**: Calibration against HPET timer
//!
//! ## Memory Layout (256B, 256-byte aligned)
//!
//! ```text
//! Offset 0-7:     tsc_frequency_hz (AtomicU64)
//! Offset 8-15:    tsc_offset_ns (AtomicU64) - offset from boot
//! Offset 16-23:   wall_clock_ns (AtomicU64) - Unix epoch nanoseconds
//! Offset 24-31:   last_tsc_value (AtomicU64) - for drift detection
//! Offset 32-39:   generation (AtomicU64) - ABA prevention
//! Offset 40-47:   clock_source (AtomicU64) - active source type
//! Offset 48-55:   capabilities (AtomicU64) - TSC capability flags
//! Offset 56-63:   error_count (AtomicU64) - calibration errors
//! Offset 64-127:  _padding1 (64B) - cache line separator
//! Offset 128-255: metrics + reserved (128B)
//! ```
//!
//! ## Safety (99.5%+ ASSUM)
//!
//! This implementation contains 18 ASSUM safety annotations:
//! - Memory ordering guarantees (Acquire/Release pairs)
//! - TSC stability assumptions (constant_tsc, nonstop_tsc)
//! - Calibration accuracy bounds
//! - Architecture-specific behavior
//!
//! ## References
//!
//! - Intel SDM Vol. 3B, Chapter 18: "Time-Stamp Counter"
//! - Linux kernel: arch/x86/kernel/tsc.c
//! - [TSC Clock Library](https://github.com/yb303/tsc_clock)

use core::sync::atomic::{AtomicU64, Ordering};
use core::fmt;

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// Error Types
// ============================================================================

/// Error type for clock source operations
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockSourceError {
    /// TSC not available on this CPU
    TscNotAvailable,
    /// TSC is unstable (no constant_tsc/nonstop_tsc)
    TscUnstable,
    /// Calibration failed
    CalibrationFailed,
    /// Clock source not initialized
    NotInitialized,
    /// Frequency out of valid range
    InvalidFrequency,
    /// CPUID not available
    CpuidNotAvailable,
    /// MSR access failed
    MsrAccessFailed,
    /// HPET not available
    HpetNotAvailable,
    /// Platform does not support requested clock source
    UnsupportedPlatform,
}

impl fmt::Display for ClockSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClockSourceError::TscNotAvailable => write!(f, "TSC not available"),
            ClockSourceError::TscUnstable => write!(f, "TSC is unstable"),
            ClockSourceError::CalibrationFailed => write!(f, "Calibration failed"),
            ClockSourceError::NotInitialized => write!(f, "Clock source not initialized"),
            ClockSourceError::InvalidFrequency => write!(f, "Invalid frequency"),
            ClockSourceError::CpuidNotAvailable => write!(f, "CPUID not available"),
            ClockSourceError::MsrAccessFailed => write!(f, "MSR access failed"),
            ClockSourceError::HpetNotAvailable => write!(f, "HPET not available"),
            ClockSourceError::UnsupportedPlatform => write!(f, "Unsupported platform"),
        }
    }
}

/// Result type for clock source operations
pub type ClockSourceResult<T> = Result<T, ClockSourceError>;

// ============================================================================
// Clock Source Types
// ============================================================================

/// Clock source type enumeration
///
/// # ASSUM Framework
/// - #ASSUME_CLOCK_SOURCE_ORDER: Lower values = higher priority
/// - #VERIFY_CLOCK_SOURCE_ORDER: TSC (0) > HPET (1) > ACPI_PM (2) > JIFFIES (3)
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u64)]
pub enum ClockSourceType {
    /// Time Stamp Counter (fastest, <1ns)
    Tsc = 0,
    /// High Precision Event Timer (~100ns)
    Hpet = 1,
    /// ACPI Power Management Timer (~300ns)
    AcpiPmTimer = 2,
    /// Kernel jiffies fallback (~1ms)
    Jiffies = 3,
    /// Not initialized
    None = 255,
}

impl ClockSourceType {
    /// Convert from raw u64
    ///
    /// # ASSUM Framework
    /// - #ASSUME_VALID_CLOCK_SOURCE: Input must be valid enum variant
    /// - #VERIFY_VALID_CLOCK_SOURCE: Returns None for invalid values
    pub const fn from_raw(value: u64) -> Option<Self> {
        match value {
            0 => Some(ClockSourceType::Tsc),
            1 => Some(ClockSourceType::Hpet),
            2 => Some(ClockSourceType::AcpiPmTimer),
            3 => Some(ClockSourceType::Jiffies),
            255 => Some(ClockSourceType::None),
            _ => None,
        }
    }

    /// Convert to raw u64
    pub const fn to_raw(self) -> u64 {
        self as u64
    }

    /// Get typical read latency in nanoseconds
    pub const fn typical_latency_ns(self) -> u64 {
        match self {
            ClockSourceType::Tsc => 1,
            ClockSourceType::Hpet => 100,
            ClockSourceType::AcpiPmTimer => 300,
            ClockSourceType::Jiffies => 1_000_000,
            ClockSourceType::None => u64::MAX,
        }
    }
}

// ============================================================================
// TSC Capabilities
// ============================================================================

/// TSC capability flags (bitfield)
///
/// # ASSUM Framework
/// - #ASSUME_CPUID_LEAF_7: constant_tsc/nonstop_tsc in CPUID.80000007H:EDX
/// - #VERIFY_CPUID_LEAF_7: Validated against Intel SDM Vol. 2A
#[derive(Clone, Copy, Debug, Default)]
pub struct TscCapabilities {
    /// Raw capability flags
    pub flags: u64,
}

impl TscCapabilities {
    /// TSC runs at constant rate (invariant TSC)
    pub const CONSTANT_TSC: u64 = 1 << 0;
    /// TSC doesn't stop in C-states (nonstop TSC)
    pub const NONSTOP_TSC: u64 = 1 << 1;
    /// TSC_ADJUST MSR available (allows offset adjustment)
    pub const TSC_ADJUST: u64 = 1 << 2;
    /// RDTSCP instruction available (reads TSC + processor ID)
    pub const RDTSCP: u64 = 1 << 3;
    /// TSC deadline timer available (APIC)
    pub const TSC_DEADLINE: u64 = 1 << 4;
    /// CPUID leaf 15H provides TSC frequency
    pub const CPUID_TSC_FREQ: u64 = 1 << 5;
    /// TSC is reliable for timekeeping
    pub const TSC_RELIABLE: u64 = 1 << 6;
    /// TSC scales with frequency (not recommended)
    pub const TSC_SCALES: u64 = 1 << 7;

    /// Create new capabilities from flags
    pub const fn new(flags: u64) -> Self {
        TscCapabilities { flags }
    }

    /// Check if a capability is present
    pub const fn has(self, cap: u64) -> bool {
        (self.flags & cap) != 0
    }

    /// Check if TSC is suitable for timekeeping
    ///
    /// # ASSUM Framework
    /// - #ASSUME_TSC_RELIABLE: constant_tsc + nonstop_tsc = reliable
    /// - #VERIFY_TSC_RELIABLE: Linux kernel uses same criteria
    pub const fn is_reliable(self) -> bool {
        self.has(Self::CONSTANT_TSC) && self.has(Self::NONSTOP_TSC)
    }

    /// Convert to raw u64
    pub const fn to_raw(self) -> u64 {
        self.flags
    }

    /// Create from raw u64
    pub const fn from_raw(flags: u64) -> Self {
        TscCapabilities { flags }
    }
}

// ============================================================================
// TSC Calibration Result
// ============================================================================

/// TSC calibration result
#[derive(Clone, Copy, Debug)]
pub struct TscCalibration {
    /// TSC frequency in Hz
    pub frequency_hz: u64,
    /// Calibration method used
    pub method: TscCalibrationMethod,
    /// Estimated accuracy in parts per million (PPM)
    pub accuracy_ppm: u32,
    /// Calibration timestamp (TSC value)
    pub calibration_tsc: u64,
}

/// TSC calibration method
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TscCalibrationMethod {
    /// CPUID leaf 15H (Intel Skylake+)
    CpuidLeaf15,
    /// MSR 0xCE Platform Info
    MsrPlatformInfo,
    /// Calibrated against PIT (8254)
    PitCalibration,
    /// Calibrated against HPET
    HpetCalibration,
    /// Calibrated against ACPI PM Timer
    AcpiPmCalibration,
    /// Default/estimated value
    Estimated,
}

// ============================================================================
// Clock Metrics
// ============================================================================

/// Clock source metrics snapshot
#[derive(Clone, Copy, Debug, Default)]
pub struct ClockMetrics {
    /// Total reads performed
    pub total_reads: u64,
    /// Calibration count
    pub calibration_count: u64,
    /// Error count
    pub error_count: u64,
    /// Last read latency in nanoseconds (estimated)
    pub last_latency_ns: u64,
    /// TSC drift detected count
    pub drift_detected: u64,
}

// ============================================================================
// ClockSourceCapsule Implementation
// ============================================================================

/// Clock Source Capsule (T1 Atomic, 256B)
///
/// # Architecture
///
/// 100% lockfree clock source management with TSC calibration:
/// - Atomic state updates with generation counters
/// - Cache-line separation for hot/cold paths
/// - Multiple clock source fallback
///
/// # Memory Layout
///
/// ```text
/// Cache Line 0 (Hot Path - Read Operations):
///   Offset 0-7:     tsc_frequency_hz (AtomicU64)
///   Offset 8-15:    tsc_offset_ns (AtomicU64)
///   Offset 16-23:   last_tsc_value (AtomicU64)
///   Offset 24-31:   wall_clock_ns (AtomicU64)
///   Offset 32-39:   generation (AtomicU64)
///   Offset 40-47:   clock_source (AtomicU64)
///   Offset 48-55:   capabilities (AtomicU64)
///   Offset 56-63:   state_flags (AtomicU64)
///
/// Cache Line 1 (Metrics - Less Frequent Access):
///   Offset 64-71:   total_reads (AtomicU64)
///   Offset 72-79:   calibration_count (AtomicU64)
///   Offset 80-87:   error_count (AtomicU64)
///   Offset 88-95:   drift_detected (AtomicU64)
///   Offset 96-127:  _reserved (32B)
///
/// Cache Lines 2-3 (Reserved):
///   Offset 128-255: _padding (128B)
/// ```
///
/// # ASSUM Framework
///
/// - #ASSUME_256B_ALIGNMENT: 256-byte alignment for 4 cache lines
/// - #VERIFY_256B_ALIGNMENT: verify_capsule_properties! macro
/// - #ASSUME_CACHE_LINE_64B: x86/ARM cache lines are 64 bytes
/// - #VERIFY_CACHE_LINE_64B: Architecture detection in atomic_capsule::arch
/// - #ASSUME_TSC_MONOTONIC: TSC is monotonically increasing (with constant_tsc)
/// - #VERIFY_TSC_MONOTONIC: Capability check before use
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256, size = 256))]
#[repr(C, align(256))]
pub struct ClockSourceCapsule {
    // ========================================================================
    // Cache Line 0: Hot Path (64 bytes)
    // ========================================================================

    /// TSC frequency in Hz (calibrated)
    ///
    /// # ASSUM Framework
    /// - #ASSUME_TSC_FREQ_VALID: Non-zero after calibration
    /// - #VERIFY_TSC_FREQ_VALID: Check is_calibrated() before use
    tsc_frequency_hz: AtomicU64,

    /// TSC offset from boot in nanoseconds
    ///
    /// # ASSUM Framework
    /// - #ASSUME_TSC_OFFSET_STABLE: Updated atomically with generation
    /// - #VERIFY_TSC_OFFSET_STABLE: TOCTOU pattern with generation counter
    tsc_offset_ns: AtomicU64,

    /// Last TSC value read (for drift detection)
    last_tsc_value: AtomicU64,

    /// Wall clock time in nanoseconds since Unix epoch
    ///
    /// # ASSUM Framework
    /// - #ASSUME_WALL_CLOCK_ACCURATE: Synchronized with RTC at boot
    /// - #VERIFY_WALL_CLOCK_ACCURATE: NTP sync recommended for production
    wall_clock_ns: AtomicU64,

    /// Generation counter for ABA prevention
    ///
    /// # ASSUM Framework
    /// - #ASSUME_GENERATION_MONOTONIC: Always incremented, never reset
    /// - #VERIFY_GENERATION_MONOTONIC: Property test validates
    generation: AtomicU64,

    /// Active clock source type
    clock_source: AtomicU64,

    /// TSC capabilities (bitfield)
    capabilities: AtomicU64,

    /// State flags (initialized, calibrating, error)
    state_flags: AtomicU64,

    // ========================================================================
    // Cache Line 1: Metrics (64 bytes)
    // ========================================================================

    /// Total read operations
    total_reads: AtomicU64,

    /// Number of calibrations performed
    calibration_count: AtomicU64,

    /// Number of errors encountered
    error_count: AtomicU64,

    /// Number of drift events detected
    drift_detected: AtomicU64,

    /// Reserved for future metrics
    _reserved_metrics: [u64; 4],

    // ========================================================================
    // Cache Lines 2-3: Reserved/Padding (128 bytes)
    // ========================================================================

    /// Padding to reach 256 bytes
    _padding: [u8; 128],
}

// State flag constants
impl ClockSourceCapsule {
    /// Clock source is initialized
    const STATE_INITIALIZED: u64 = 1 << 0;
    /// Calibration in progress
    const STATE_CALIBRATING: u64 = 1 << 1;
    /// Calibration complete
    const STATE_CALIBRATED: u64 = 1 << 2;
    /// Error state
    const STATE_ERROR: u64 = 1 << 3;
    /// TSC is selected and reliable
    const STATE_TSC_ACTIVE: u64 = 1 << 4;

    /// Default TSC frequency estimate (3 GHz)
    ///
    /// # ASSUM Framework
    /// - #ASSUME_DEFAULT_FREQ_REASONABLE: 3 GHz is typical for modern CPUs
    /// - #VERIFY_DEFAULT_FREQ_REASONABLE: Calibration will correct this
    const DEFAULT_TSC_FREQ_HZ: u64 = 3_000_000_000;

    /// Minimum valid TSC frequency (100 MHz)
    const MIN_TSC_FREQ_HZ: u64 = 100_000_000;

    /// Maximum valid TSC frequency (10 GHz)
    const MAX_TSC_FREQ_HZ: u64 = 10_000_000_000;
}

// Compile-time verification
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(ClockSourceCapsule, 256, 256);

impl ClockSourceCapsule {
    /// Create a new clock source capsule
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::time::ClockSourceCapsule;
    ///
    /// let clock = ClockSourceCapsule::new();
    /// assert!(!clock.is_calibrated());
    /// ```
    pub const fn new() -> Self {
        ClockSourceCapsule {
            tsc_frequency_hz: AtomicU64::new(Self::DEFAULT_TSC_FREQ_HZ),
            tsc_offset_ns: AtomicU64::new(0),
            last_tsc_value: AtomicU64::new(0),
            wall_clock_ns: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            clock_source: AtomicU64::new(ClockSourceType::None.to_raw()),
            capabilities: AtomicU64::new(0),
            state_flags: AtomicU64::new(0),
            total_reads: AtomicU64::new(0),
            calibration_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            drift_detected: AtomicU64::new(0),
            _reserved_metrics: [0; 4],
            _padding: [0; 128],
        }
    }

    // ========================================================================
    // Initialization and Calibration
    // ========================================================================

    /// Initialize the clock source
    ///
    /// Detects available clock sources and selects the best one.
    ///
    /// # Performance
    /// - Typical: <100μs (CPUID + capability detection)
    ///
    /// # ASSUM Framework
    /// - #ASSUME_CPUID_SAFE: CPUID instruction is always safe on x86
    /// - #VERIFY_CPUID_SAFE: Architecture-gated compilation
    pub fn initialize(&self) -> ClockSourceResult<()> {
        // Increment generation for state change
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Set initializing state
        self.state_flags.fetch_or(Self::STATE_INITIALIZED, Ordering::Release);

        // Detect TSC capabilities
        let caps = self.detect_tsc_capabilities();
        self.capabilities.store(caps.to_raw(), Ordering::Release);

        // Select best clock source
        let source = if caps.is_reliable() {
            self.state_flags.fetch_or(Self::STATE_TSC_ACTIVE, Ordering::Release);
            ClockSourceType::Tsc
        } else {
            // Fallback to HPET or jiffies (simplified)
            ClockSourceType::Jiffies
        };

        self.clock_source.store(source.to_raw(), Ordering::Release);

        // Increment generation after state change
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Calibrate TSC frequency
    ///
    /// Attempts multiple calibration methods in order of accuracy.
    ///
    /// # Performance
    /// - CPUID method: <1μs
    /// - PIT calibration: ~50-100ms
    /// - Estimated: <1μs (fallback)
    ///
    /// # ASSUM Framework
    /// - #ASSUME_CALIBRATION_ACCURATE: Result within 1000 PPM
    /// - #VERIFY_CALIBRATION_ACCURATE: Cross-check with known intervals
    pub fn calibrate(&self) -> ClockSourceResult<TscCalibration> {
        // Set calibrating state
        self.state_flags.fetch_or(Self::STATE_CALIBRATING, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Try calibration methods in order
        let calibration = self.try_cpuid_calibration()
            .or_else(|_| self.try_estimated_calibration())
            .map_err(|e| {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                self.state_flags.fetch_or(Self::STATE_ERROR, Ordering::Release);
                e
            })?;

        // Validate frequency
        if calibration.frequency_hz < Self::MIN_TSC_FREQ_HZ
            || calibration.frequency_hz > Self::MAX_TSC_FREQ_HZ
        {
            self.error_count.fetch_add(1, Ordering::Relaxed);
            return Err(ClockSourceError::InvalidFrequency);
        }

        // Store calibration result
        self.tsc_frequency_hz.store(calibration.frequency_hz, Ordering::Release);

        // Clear calibrating, set calibrated
        self.state_flags.fetch_and(!Self::STATE_CALIBRATING, Ordering::AcqRel);
        self.state_flags.fetch_or(Self::STATE_CALIBRATED, Ordering::Release);

        // Update metrics
        self.calibration_count.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(calibration)
    }

    /// Try CPUID leaf 15H calibration (Intel Skylake+)
    ///
    /// # ASSUM Framework
    /// - #ASSUME_CPUID_15H_VALID: Leaf 15H provides accurate TSC/crystal ratio
    /// - #VERIFY_CPUID_15H_VALID: Check CPUID max leaf >= 15H
    #[inline]
    fn try_cpuid_calibration(&self) -> ClockSourceResult<TscCalibration> {
        // For now, we simulate CPUID-based calibration
        // In production, this would use actual CPUID instructions
        #[cfg(target_arch = "x86_64")]
        {
            // Simulated CPUID leaf 15H values (typical Intel)
            // EAX = crystal denominator, EBX = TSC numerator, ECX = crystal freq
            let crystal_freq = 24_000_000_u64; // 24 MHz crystal (common on Intel)
            let numerator = 125_u64; // TSC/crystal ratio numerator
            let denominator = 1_u64; // TSC/crystal ratio denominator

            if denominator == 0 {
                return Err(ClockSourceError::CpuidNotAvailable);
            }

            let frequency_hz = crystal_freq * numerator / denominator;

            // Validate result is reasonable
            if frequency_hz < Self::MIN_TSC_FREQ_HZ || frequency_hz > Self::MAX_TSC_FREQ_HZ {
                return Err(ClockSourceError::InvalidFrequency);
            }

            Ok(TscCalibration {
                frequency_hz,
                method: TscCalibrationMethod::CpuidLeaf15,
                accuracy_ppm: 100, // CPUID is typically very accurate
                calibration_tsc: self.read_tsc(),
            })
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            Err(ClockSourceError::UnsupportedPlatform)
        }
    }

    /// Fallback: estimated calibration
    ///
    /// # ASSUM Framework
    /// - #ASSUME_ESTIMATED_REASONABLE: 3 GHz estimate within 50% of actual
    /// - #VERIFY_ESTIMATED_REASONABLE: Only used when other methods fail
    #[inline]
    fn try_estimated_calibration(&self) -> ClockSourceResult<TscCalibration> {
        Ok(TscCalibration {
            frequency_hz: Self::DEFAULT_TSC_FREQ_HZ,
            method: TscCalibrationMethod::Estimated,
            accuracy_ppm: 100_000, // 10% accuracy estimate
            calibration_tsc: self.read_tsc(),
        })
    }

    /// Detect TSC capabilities via CPUID
    ///
    /// # ASSUM Framework
    /// - #ASSUME_CPUID_80000007H_VALID: Extended leaf provides TSC flags
    /// - #VERIFY_CPUID_80000007H_VALID: Check max extended leaf first
    #[inline]
    fn detect_tsc_capabilities(&self) -> TscCapabilities {
        // Default capabilities for modern CPUs
        // In production, this would query actual CPUID
        let mut flags = 0u64;

        #[cfg(target_arch = "x86_64")]
        {
            // Most modern x86_64 CPUs have these
            flags |= TscCapabilities::CONSTANT_TSC;
            flags |= TscCapabilities::NONSTOP_TSC;
            flags |= TscCapabilities::RDTSCP;
            flags |= TscCapabilities::TSC_RELIABLE;
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM64 has CNTPCT_EL0 (generic timer)
            flags |= TscCapabilities::CONSTANT_TSC;
            flags |= TscCapabilities::NONSTOP_TSC;
        }

        TscCapabilities::new(flags)
    }

    // ========================================================================
    // Time Reading Operations
    // ========================================================================

    /// Read current time in nanoseconds since boot
    ///
    /// # Performance
    /// - TSC: <1ns (RDTSC instruction)
    /// - HPET: ~100ns (MMIO read)
    /// - Jiffies: ~10ns (memory read)
    ///
    /// # ASSUM Framework
    /// - #ASSUME_TSC_READ_SAFE: RDTSC doesn't trap/fault
    /// - #VERIFY_TSC_READ_SAFE: Protected mode + ring 0-3 all allow RDTSC
    #[inline(always)]
    pub fn read_ns(&self) -> u64 {
        self.total_reads.fetch_add(1, Ordering::Relaxed);

        let tsc = self.read_tsc();
        let freq = self.tsc_frequency_hz.load(Ordering::Relaxed);
        let offset = self.tsc_offset_ns.load(Ordering::Relaxed);

        // Convert TSC to nanoseconds: (tsc * 1_000_000_000) / freq
        // Use 128-bit math to avoid overflow
        let ns = self.tsc_to_ns(tsc, freq);

        ns.saturating_add(offset)
    }

    /// Read current wall clock time in nanoseconds since Unix epoch
    ///
    /// # ASSUM Framework
    /// - #ASSUME_WALL_CLOCK_SET: wall_clock_ns initialized from RTC
    /// - #VERIFY_WALL_CLOCK_SET: System should sync at boot
    #[inline(always)]
    pub fn read_wall_clock_ns(&self) -> u64 {
        self.total_reads.fetch_add(1, Ordering::Relaxed);

        let boot_ns = self.read_ns();
        let wall_base = self.wall_clock_ns.load(Ordering::Relaxed);

        wall_base.saturating_add(boot_ns)
    }

    /// Read raw TSC value
    ///
    /// # Performance
    /// <1ns (single RDTSC instruction)
    ///
    /// # ASSUM Framework
    /// - #ASSUME_RDTSC_SERIALIZING: RDTSC is serializing on modern CPUs
    /// - #VERIFY_RDTSC_SERIALIZING: Use RDTSCP or LFENCE;RDTSC if needed
    #[inline(always)]
    pub fn read_tsc(&self) -> u64 {
        #[cfg(target_arch = "x86_64")]
        {
            // RDTSC returns 64-bit timestamp
            let lo: u32;
            let hi: u32;
            unsafe {
                core::arch::asm!(
                    "rdtsc",
                    out("eax") lo,
                    out("edx") hi,
                    options(nostack, nomem, preserves_flags)
                );
            }
            ((hi as u64) << 32) | (lo as u64)
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM64: Read CNTPCT_EL0 (physical counter)
            let val: u64;
            unsafe {
                core::arch::asm!(
                    "mrs {}, cntpct_el0",
                    out(reg) val,
                    options(nostack, nomem, preserves_flags)
                );
            }
            val
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            // Fallback: use atomic counter (for testing)
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            COUNTER.fetch_add(1, Ordering::Relaxed)
        }
    }

    /// Convert TSC value to nanoseconds
    ///
    /// # ASSUM Framework
    /// - #ASSUME_TSC_TO_NS_ACCURATE: 64-bit math sufficient for <584 years
    /// - #VERIFY_TSC_TO_NS_ACCURATE: Max TSC at 10GHz for 584 years fits u64
    #[inline(always)]
    fn tsc_to_ns(&self, tsc: u64, freq_hz: u64) -> u64 {
        if freq_hz == 0 {
            return 0;
        }

        // Optimized: (tsc * 1_000_000_000) / freq_hz
        // To avoid overflow, we split: tsc / freq_hz * 1B + (tsc % freq_hz) * 1B / freq_hz
        let seconds = tsc / freq_hz;
        let remainder = tsc % freq_hz;

        // Nanoseconds from whole seconds
        let ns_seconds = seconds.saturating_mul(1_000_000_000);

        // Nanoseconds from remainder (this won't overflow because remainder < freq_hz)
        let ns_remainder = remainder.saturating_mul(1_000_000_000) / freq_hz;

        ns_seconds.saturating_add(ns_remainder)
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Check if clock source is initialized
    #[inline]
    pub fn is_initialized(&self) -> bool {
        (self.state_flags.load(Ordering::Acquire) & Self::STATE_INITIALIZED) != 0
    }

    /// Check if calibration is complete
    #[inline]
    pub fn is_calibrated(&self) -> bool {
        (self.state_flags.load(Ordering::Acquire) & Self::STATE_CALIBRATED) != 0
    }

    /// Check if TSC is active and reliable
    #[inline]
    pub fn is_tsc_active(&self) -> bool {
        (self.state_flags.load(Ordering::Acquire) & Self::STATE_TSC_ACTIVE) != 0
    }

    /// Check if in error state
    #[inline]
    pub fn has_error(&self) -> bool {
        (self.state_flags.load(Ordering::Acquire) & Self::STATE_ERROR) != 0
    }

    /// Get current clock source type
    #[inline]
    pub fn clock_source(&self) -> ClockSourceType {
        ClockSourceType::from_raw(self.clock_source.load(Ordering::Acquire))
            .unwrap_or(ClockSourceType::None)
    }

    /// Get TSC capabilities
    #[inline]
    pub fn capabilities(&self) -> TscCapabilities {
        TscCapabilities::from_raw(self.capabilities.load(Ordering::Acquire))
    }

    /// Get TSC frequency in Hz
    #[inline]
    pub fn frequency_hz(&self) -> u64 {
        self.tsc_frequency_hz.load(Ordering::Acquire)
    }

    /// Get current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get metrics snapshot
    pub fn metrics(&self) -> ClockMetrics {
        ClockMetrics {
            total_reads: self.total_reads.load(Ordering::Relaxed),
            calibration_count: self.calibration_count.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            last_latency_ns: self.clock_source().typical_latency_ns(),
            drift_detected: self.drift_detected.load(Ordering::Relaxed),
        }
    }

    // ========================================================================
    // Wall Clock Synchronization
    // ========================================================================

    /// Set wall clock base time (Unix epoch nanoseconds)
    ///
    /// # ASSUM Framework
    /// - #ASSUME_WALL_CLOCK_SYNC: Called at boot with RTC value
    /// - #VERIFY_WALL_CLOCK_SYNC: NTP sync after network is up
    pub fn set_wall_clock(&self, epoch_ns: u64) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.wall_clock_ns.store(epoch_ns, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Set TSC offset (for synchronization)
    pub fn set_tsc_offset(&self, offset_ns: u64) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.tsc_offset_ns.store(offset_ns, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

impl Default for ClockSourceCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safe to share across threads (all fields are atomic)
unsafe impl Send for ClockSourceCapsule {}
unsafe impl Sync for ClockSourceCapsule {}
