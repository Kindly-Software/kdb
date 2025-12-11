//! Platform Detector Capsule (T1 Atomic, 128B)
//!
//! Cross-platform OS and architecture detection with atomic caching.
//!
//! ## Performance
//! - Detection: <100ns (cached)
//! - First detection: <1ms (file system probes for WSL on Linux)
//!
//! ## UCE35 Compliance
//! - T1 Atomic tier (lockfree, cache-aligned)
//! - 64B alignment prevents false sharing
//! - Generation counter for TOCTOU prevention
//! - Deterministic detection (same result on same platform)
//!
//! ## Features
//! - OS detection: Linux, macOS, Windows, FreeBSD
//! - Architecture detection: x86_64, aarch64, x86, arm, riscv64
//! - WSL detection: Identifies Windows Subsystem for Linux environments
//! - Config directory validation: Verifies home/config paths exist

use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

/// Platform identifier (OS)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Platform {
    /// Linux (Ubuntu, Debian, Fedora, Arch, etc.)
    Linux = 0,
    /// macOS (Intel and Apple Silicon)
    MacOS = 1,
    /// Windows (10, 11, Server)
    Windows = 2,
    /// FreeBSD
    FreeBSD = 3,
    /// Unknown/unsupported platform
    Unknown = 255,
}

impl Platform {
    /// Get platform name as string slice
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Platform::Linux => "linux",
            Platform::MacOS => "macos",
            Platform::Windows => "windows",
            Platform::FreeBSD => "freebsd",
            Platform::Unknown => "unknown",
        }
    }

    /// Detect the current platform at runtime.
    ///
    /// Uses compile-time cfg attributes for zero-cost detection.
    #[inline]
    pub fn detect() -> Self {
        detect_platform()
    }

    /// Check if platform supports XDG Base Directory Specification
    #[inline]
    pub const fn supports_xdg(&self) -> bool {
        matches!(self, Platform::Linux | Platform::FreeBSD)
    }

    /// Check if platform uses XDG Base Directory Specification.
    ///
    /// Alias for `supports_xdg()` for path module compatibility.
    #[inline]
    pub const fn uses_xdg(&self) -> bool {
        matches!(
            self,
            Platform::Linux | Platform::FreeBSD | Platform::Unknown
        )
    }

    /// Check if platform uses Windows-style paths (backslash separator).
    #[inline]
    pub const fn uses_windows_paths(&self) -> bool {
        matches!(self, Platform::Windows)
    }

    /// Check if platform uses backslash path separators
    #[inline]
    pub const fn uses_backslash(&self) -> bool {
        matches!(self, Platform::Windows)
    }

    /// Get default config directory name for platform
    #[inline]
    pub const fn config_dir_name(&self) -> &'static str {
        match self {
            Platform::MacOS => "Application Support",
            _ => ".config",
        }
    }
}

impl Platform {
    /// Convert from u8 (const-compatible)
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Platform::Linux,
            1 => Platform::MacOS,
            2 => Platform::Windows,
            3 => Platform::FreeBSD,
            _ => Platform::Unknown,
        }
    }
}

impl From<u8> for Platform {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

/// Architecture identifier (CPU)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Architecture {
    /// x86_64 (Intel/AMD 64-bit)
    X86_64 = 0,
    /// aarch64 (ARM 64-bit, Apple Silicon, Raspberry Pi 4+)
    Aarch64 = 1,
    /// x86 (Intel/AMD 32-bit, legacy)
    X86 = 2,
    /// ARM 32-bit (embedded, Raspberry Pi 3)
    Arm = 3,
    /// RISC-V 64-bit
    Riscv64 = 4,
    /// Unknown/unsupported architecture
    Unknown = 255,
}

impl Architecture {
    /// Get architecture name as string slice
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Architecture::X86_64 => "x86_64",
            Architecture::Aarch64 => "aarch64",
            Architecture::X86 => "x86",
            Architecture::Arm => "arm",
            Architecture::Riscv64 => "riscv64",
            Architecture::Unknown => "unknown",
        }
    }

    /// Check if architecture is 64-bit
    #[inline]
    pub const fn is_64bit(&self) -> bool {
        matches!(
            self,
            Architecture::X86_64 | Architecture::Aarch64 | Architecture::Riscv64
        )
    }

    /// Check if architecture supports AVX2 SIMD (Intel/AMD 64-bit only)
    #[inline]
    pub const fn supports_avx2(&self) -> bool {
        matches!(self, Architecture::X86_64)
    }

    /// Check if architecture supports NEON SIMD (ARM only)
    #[inline]
    pub const fn supports_neon(&self) -> bool {
        matches!(self, Architecture::Aarch64 | Architecture::Arm)
    }
}

impl Architecture {
    /// Convert from u8 (const-compatible)
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Architecture::X86_64,
            1 => Architecture::Aarch64,
            2 => Architecture::X86,
            3 => Architecture::Arm,
            4 => Architecture::Riscv64,
            _ => Architecture::Unknown,
        }
    }
}

impl From<u8> for Architecture {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

/// Platform information snapshot (immutable after detection)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformInfo {
    /// Operating system
    pub platform: Platform,
    /// CPU architecture
    pub arch: Architecture,
    /// Home directory exists (validated)
    pub home_valid: bool,
    /// Config directory exists or can be created
    pub config_valid: bool,
    /// Running under Windows Subsystem for Linux
    pub is_wsl: bool,
}

impl PlatformInfo {
    /// Create a new PlatformInfo with detection results
    #[inline]
    pub const fn new(
        platform: Platform,
        arch: Architecture,
        home_valid: bool,
        config_valid: bool,
        is_wsl: bool,
    ) -> Self {
        Self {
            platform,
            arch,
            home_valid,
            config_valid,
            is_wsl,
        }
    }

    /// Pack into u32 for atomic storage
    /// Layout: [platform:8][arch:8][home_valid:1][config_valid:1][is_wsl:1][reserved:13]
    #[inline]
    pub const fn pack(&self) -> u32 {
        let platform_byte = self.platform as u8 as u32;
        let arch_byte = self.arch as u8 as u32;
        let flags = ((self.home_valid as u32) << 16)
            | ((self.config_valid as u32) << 17)
            | ((self.is_wsl as u32) << 18);
        platform_byte | (arch_byte << 8) | flags
    }

    /// Unpack from u32 atomic storage
    #[inline]
    pub const fn unpack(packed: u32) -> Self {
        let platform = Platform::from_u8((packed & 0xFF) as u8);
        let arch = Architecture::from_u8(((packed >> 8) & 0xFF) as u8);
        let home_valid = ((packed >> 16) & 1) != 0;
        let config_valid = ((packed >> 17) & 1) != 0;
        let is_wsl = ((packed >> 18) & 1) != 0;
        Self {
            platform,
            arch,
            home_valid,
            config_valid,
            is_wsl,
        }
    }

    /// Get platform-specific path separator
    #[inline]
    pub const fn path_separator(&self) -> char {
        if self.platform.uses_backslash() {
            '\\'
        } else {
            '/'
        }
    }
}

impl Default for PlatformInfo {
    fn default() -> Self {
        Self {
            platform: Platform::Unknown,
            arch: Architecture::Unknown,
            home_valid: false,
            config_valid: false,
            is_wsl: false,
        }
    }
}

/// Platform Detector Capsule (T1 Atomic, 64B aligned)
///
/// Provides atomic, cached platform detection with TOCTOU prevention.
///
/// ## Layout (64 bytes)
/// - `packed_info`: AtomicU64 [u32 platform_info + u32 reserved]
/// - `generation`: AtomicU64 for TOCTOU prevention
/// - `detection_timestamp`: AtomicU64 (Unix timestamp of detection)
/// - `detection_count`: AtomicU64 (number of detections performed)
/// - `state`: AtomicU8 (detection state machine)
/// - `_padding`: 31 bytes for 64B alignment
#[repr(C, align(64))]
pub struct PlatformDetectorCapsule {
    /// Packed platform info (platform, arch, flags)
    packed_info: AtomicU64,
    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,
    /// Unix timestamp when detection was performed
    detection_timestamp: AtomicU64,
    /// Number of times detection has been run
    detection_count: AtomicU64,
    /// Detection state: 0=uninitialized, 1=detecting, 2=detected, 3=error
    state: AtomicU8,
    /// Padding for 64B alignment
    _padding: [u8; 31],
}

/// Detection state machine
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionState {
    /// Not yet detected
    Uninitialized = 0,
    /// Detection in progress
    Detecting = 1,
    /// Detection complete
    Detected = 2,
    /// Detection failed
    Error = 3,
}

impl From<u8> for DetectionState {
    fn from(value: u8) -> Self {
        match value {
            0 => DetectionState::Uninitialized,
            1 => DetectionState::Detecting,
            2 => DetectionState::Detected,
            3 => DetectionState::Error,
            _ => DetectionState::Error,
        }
    }
}

impl PlatformDetectorCapsule {
    /// Create a new uninitialized detector
    #[inline]
    pub const fn new() -> Self {
        Self {
            packed_info: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            detection_timestamp: AtomicU64::new(0),
            detection_count: AtomicU64::new(0),
            state: AtomicU8::new(0),
            _padding: [0; 31],
        }
    }

    /// Detect platform (cached after first call)
    ///
    /// Returns cached result if already detected.
    /// Thread-safe with atomic state transitions.
    #[inline]
    pub fn detect(&self) -> PlatformInfo {
        // Fast path: already detected
        let state = DetectionState::from(self.state.load(Ordering::Acquire));
        if state == DetectionState::Detected {
            let packed = self.packed_info.load(Ordering::Acquire) as u32;
            return PlatformInfo::unpack(packed);
        }

        // Slow path: perform detection
        self.detect_slow()
    }

    /// Slow path for platform detection
    #[cold]
    fn detect_slow(&self) -> PlatformInfo {
        // Try to transition from Uninitialized to Detecting
        let result = self.state.compare_exchange(
            DetectionState::Uninitialized as u8,
            DetectionState::Detecting as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        match result {
            Ok(_) => {
                // We won the race, perform detection
                let info = self.perform_detection();
                let packed = info.pack() as u64;

                // Store result atomically
                self.packed_info.store(packed, Ordering::Release);
                self.generation.fetch_add(1, Ordering::AcqRel);
                self.detection_count.fetch_add(1, Ordering::Relaxed);

                // Get current timestamp
                #[cfg(feature = "std")]
                {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
                        self.detection_timestamp
                            .store(duration.as_secs(), Ordering::Relaxed);
                    }
                }

                // Mark as detected
                self.state
                    .store(DetectionState::Detected as u8, Ordering::Release);
                info
            }
            Err(current) => {
                // Someone else is detecting or already detected
                let state = DetectionState::from(current);
                match state {
                    DetectionState::Detecting => {
                        // Spin wait for detection to complete
                        while DetectionState::from(self.state.load(Ordering::Acquire))
                            == DetectionState::Detecting
                        {
                            core::hint::spin_loop();
                        }
                        let packed = self.packed_info.load(Ordering::Acquire) as u32;
                        PlatformInfo::unpack(packed)
                    }
                    DetectionState::Detected => {
                        let packed = self.packed_info.load(Ordering::Acquire) as u32;
                        PlatformInfo::unpack(packed)
                    }
                    _ => PlatformInfo::default(),
                }
            }
        }
    }

    /// Perform actual platform detection
    #[inline]
    fn perform_detection(&self) -> PlatformInfo {
        let platform = detect_platform();
        let arch = detect_architecture();
        let home_valid = validate_home_directory();
        let config_valid = validate_config_directory();
        let is_wsl = detect_wsl();

        PlatformInfo::new(platform, arch, home_valid, config_valid, is_wsl)
    }

    /// Get current detection state
    #[inline]
    pub fn state(&self) -> DetectionState {
        DetectionState::from(self.state.load(Ordering::Acquire))
    }

    /// Get generation counter (for TOCTOU prevention)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get detection timestamp (Unix seconds)
    #[inline]
    pub fn detection_timestamp(&self) -> u64 {
        self.detection_timestamp.load(Ordering::Relaxed)
    }

    /// Get number of detections performed
    #[inline]
    pub fn detection_count(&self) -> u64 {
        self.detection_count.load(Ordering::Relaxed)
    }

    /// Force re-detection (for testing or config changes)
    ///
    /// Returns true if reset was successful.
    #[inline]
    pub fn reset(&self) -> bool {
        self.state
            .compare_exchange(
                DetectionState::Detected as u8,
                DetectionState::Uninitialized as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
}

impl Default for PlatformDetectorCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: PlatformDetectorCapsule uses only atomic operations
unsafe impl Send for PlatformDetectorCapsule {}
unsafe impl Sync for PlatformDetectorCapsule {}

/// Detect current platform using compile-time cfg
#[inline]
fn detect_platform() -> Platform {
    #[cfg(target_os = "linux")]
    {
        Platform::Linux
    }
    #[cfg(target_os = "macos")]
    {
        Platform::MacOS
    }
    #[cfg(target_os = "windows")]
    {
        Platform::Windows
    }
    #[cfg(target_os = "freebsd")]
    {
        Platform::FreeBSD
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows",
        target_os = "freebsd"
    )))]
    {
        Platform::Unknown
    }
}

/// Detect current architecture using compile-time cfg
#[inline]
fn detect_architecture() -> Architecture {
    #[cfg(target_arch = "x86_64")]
    {
        Architecture::X86_64
    }
    #[cfg(target_arch = "aarch64")]
    {
        Architecture::Aarch64
    }
    #[cfg(target_arch = "x86")]
    {
        Architecture::X86
    }
    #[cfg(target_arch = "arm")]
    {
        Architecture::Arm
    }
    #[cfg(target_arch = "riscv64")]
    {
        Architecture::Riscv64
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "x86",
        target_arch = "arm",
        target_arch = "riscv64"
    )))]
    {
        Architecture::Unknown
    }
}

/// Validate that home directory exists
#[cfg(feature = "std")]
fn validate_home_directory() -> bool {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(|home| std::path::Path::new(&home).is_dir())
        .unwrap_or(false)
}

#[cfg(not(feature = "std"))]
fn validate_home_directory() -> bool {
    false
}

/// Validate that config directory exists or can be created
#[cfg(feature = "std")]
fn validate_config_directory() -> bool {
    // Use the auto-detecting get_config_dir() from paths module
    if let Some(config_dir) = super::paths::get_config_dir() {
        config_dir.is_dir() || config_dir.parent().map(|p| p.is_dir()).unwrap_or(false)
    } else {
        false
    }
}

#[cfg(not(feature = "std"))]
fn validate_config_directory() -> bool {
    false
}

/// Detect if running under Windows Subsystem for Linux (WSL).
///
/// On Linux, checks /proc/version for "microsoft" or "wsl" strings.
/// On other platforms, always returns false.
///
/// # Performance
/// - Linux: ~1ms (file read)
/// - Other: <1ns (compile-time constant false)
///
/// #ASSUME: WSL sets /proc/version to contain "microsoft" or "wsl" (case-insensitive)
/// #VERIFY: test_wsl_detection validates on WSL if available
#[cfg(all(target_os = "linux", feature = "std"))]
fn detect_wsl() -> bool {
    std::fs::read_to_string("/proc/version")
        .map(|v| {
            let lower = v.to_lowercase();
            lower.contains("microsoft") || lower.contains("wsl")
        })
        .unwrap_or(false)
}

#[cfg(not(all(target_os = "linux", feature = "std")))]
fn detect_wsl() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_as_str() {
        assert_eq!(Platform::Linux.as_str(), "linux");
        assert_eq!(Platform::MacOS.as_str(), "macos");
        assert_eq!(Platform::Windows.as_str(), "windows");
    }

    #[test]
    fn test_architecture_as_str() {
        assert_eq!(Architecture::X86_64.as_str(), "x86_64");
        assert_eq!(Architecture::Aarch64.as_str(), "aarch64");
    }

    #[test]
    fn test_platform_info_pack_unpack() {
        // Test without WSL flag
        let info = PlatformInfo::new(Platform::Linux, Architecture::X86_64, true, true, false);
        let packed = info.pack();
        let unpacked = PlatformInfo::unpack(packed);
        assert_eq!(info, unpacked);

        // Test with WSL flag
        let info_wsl = PlatformInfo::new(Platform::Linux, Architecture::X86_64, true, true, true);
        let packed_wsl = info_wsl.pack();
        let unpacked_wsl = PlatformInfo::unpack(packed_wsl);
        assert_eq!(info_wsl, unpacked_wsl);
        assert!(unpacked_wsl.is_wsl);
    }

    #[test]
    fn test_detector_capsule_size() {
        assert_eq!(core::mem::size_of::<PlatformDetectorCapsule>(), 64);
        assert_eq!(core::mem::align_of::<PlatformDetectorCapsule>(), 64);
    }

    #[test]
    fn test_detector_initial_state() {
        let detector = PlatformDetectorCapsule::new();
        assert_eq!(detector.state(), DetectionState::Uninitialized);
        assert_eq!(detector.generation(), 0);
        assert_eq!(detector.detection_count(), 0);
    }

    #[test]
    fn test_detector_detect() {
        let detector = PlatformDetectorCapsule::new();
        let info = detector.detect();

        // Should be detected now
        assert_eq!(detector.state(), DetectionState::Detected);
        assert_eq!(detector.detection_count(), 1);

        // Second detection should return cached result
        let info2 = detector.detect();
        assert_eq!(info, info2);
        assert_eq!(detector.detection_count(), 1); // Still 1, cached

        // Platform should match compile-time detection
        #[cfg(target_os = "linux")]
        assert_eq!(info.platform, Platform::Linux);
        #[cfg(target_os = "macos")]
        assert_eq!(info.platform, Platform::MacOS);
        #[cfg(target_os = "windows")]
        assert_eq!(info.platform, Platform::Windows);

        // Arch should match compile-time detection
        #[cfg(target_arch = "x86_64")]
        assert_eq!(info.arch, Architecture::X86_64);
        #[cfg(target_arch = "aarch64")]
        assert_eq!(info.arch, Architecture::Aarch64);
    }

    #[test]
    fn test_detector_reset() {
        let detector = PlatformDetectorCapsule::new();
        let _ = detector.detect();
        assert_eq!(detector.state(), DetectionState::Detected);

        // Reset should succeed
        assert!(detector.reset());
        assert_eq!(detector.state(), DetectionState::Uninitialized);

        // Re-detect should work
        let _ = detector.detect();
        assert_eq!(detector.state(), DetectionState::Detected);
        assert_eq!(detector.detection_count(), 2);
    }

    #[test]
    fn test_platform_supports_xdg() {
        assert!(Platform::Linux.supports_xdg());
        assert!(Platform::FreeBSD.supports_xdg());
        assert!(!Platform::MacOS.supports_xdg());
        assert!(!Platform::Windows.supports_xdg());
    }

    #[test]
    fn test_architecture_is_64bit() {
        assert!(Architecture::X86_64.is_64bit());
        assert!(Architecture::Aarch64.is_64bit());
        assert!(Architecture::Riscv64.is_64bit());
        assert!(!Architecture::X86.is_64bit());
        assert!(!Architecture::Arm.is_64bit());
    }

    /// Test WSL detection (only meaningful on Linux).
    ///
    /// This test verifies that WSL detection works without panicking.
    /// The actual value depends on the execution environment.
    #[test]
    #[cfg(all(target_os = "linux", feature = "std"))]
    fn test_wsl_detection() {
        let detector = PlatformDetectorCapsule::new();
        let info = detector.detect();

        // We can't assert the value since it depends on the environment,
        // but we can verify it doesn't panic and returns a boolean
        let _is_wsl: bool = info.is_wsl;

        // If we're on WSL, /proc/version should contain "microsoft" or "wsl"
        if info.is_wsl {
            let version = std::fs::read_to_string("/proc/version").unwrap_or_default();
            let lower = version.to_lowercase();
            assert!(
                lower.contains("microsoft") || lower.contains("wsl"),
                "WSL detection should match /proc/version content"
            );
        }
    }

    /// Test detection count increments correctly.
    ///
    /// Verifies that:
    /// - Initial count is 0
    /// - First detect() increments to 1
    /// - Cached detect() does NOT increment (stays at 1)
    /// - Reset followed by detect() increments to 2
    #[test]
    fn test_detection_count() {
        let detector = PlatformDetectorCapsule::new();

        // Initial count should be 0
        assert_eq!(detector.detection_count(), 0, "Initial count should be 0");

        // First detection should increment count
        let _ = detector.detect();
        assert_eq!(
            detector.detection_count(),
            1,
            "Count should be 1 after first detect"
        );

        // Cached detection should NOT increment count
        let _ = detector.detect();
        assert_eq!(
            detector.detection_count(),
            1,
            "Count should still be 1 after cached detect"
        );

        // Reset and detect again should increment
        assert!(detector.reset());
        let _ = detector.detect();
        assert_eq!(
            detector.detection_count(),
            2,
            "Count should be 2 after reset and detect"
        );
    }

    /// Test that caching works correctly.
    ///
    /// Verifies that the second call to detect() uses the cache
    /// (returns identical result without incrementing detection count).
    #[test]
    fn test_detect_caching() {
        let detector = PlatformDetectorCapsule::new();

        // First detection
        let info1 = detector.detect();
        let count1 = detector.detection_count();
        assert_eq!(count1, 1, "First detection should increment count");
        assert_eq!(
            detector.state(),
            DetectionState::Detected,
            "State should be Detected"
        );

        // Second detection should return identical result from cache
        let info2 = detector.detect();
        let count2 = detector.detection_count();
        assert_eq!(info1, info2, "Cached result should be identical");
        assert_eq!(count2, 1, "Count should still be 1 (cached)");
    }
}
