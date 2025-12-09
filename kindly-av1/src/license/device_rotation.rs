//! Device Rotation Capsule (T9 Persistent)
//! [TRADE SECRET]
//!
//! Automatic device rotation for offline license validation.
//! When device limit+1 activated, oldest device is replaced automatically.
//!
//! # Memory Layout (384B, cache-aligned)
//!
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       8     device_count (AtomicU8 + 7B padding)
//! 8       8     generation (AtomicU64)
//! 16      8     last_rotation_timestamp (AtomicU64)
//! 24      8     rotation_count (AtomicU64)
//! 32      320   devices [5 × 64B slots]
//!               Each slot:
//!                 0-31   fingerprint_hash ([u8; 32])
//!                 32-39  timestamp (AtomicU64)
//!                 40-63  _padding (24B)
//! 352     32    _padding (repr(C) aligns to 384B = 6 cache lines)
//! ------  ----
//! Total:  384B (6 cache lines, 64B aligned)
//! ```
//!
//! # Disk Format
//!
//! ```text
//! Offset  Size  Description
//! 0       4     Magic bytes "KDLY"
//! 4       1     Version (currently 1)
//! 5       1     Device count
//! 6       2     _padding
//! 8       8     Generation counter
//! 16      8     Last rotation timestamp
//! 24      8     Rotation count
//! 32      320   Device slots (5 × 64B)
//! ------  ----
//! Total:  352 bytes
//! ```
//!
//! # Framework Compliance
//!
//! - UCE34 Q10: T9 Persistent + T1 Atomic
//! - Chaos: 100% lockfree, cache-aligned, generation counters
//! - ASSUM: All assumptions documented with #ASSUME tags

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::config::{APP_NAME, LICENSE_MAGIC, LICENSE_VERSION};
use super::fingerprint::HardwareFingerprint;

/// Maximum number of devices (Enterprise tier limit)
pub const MAX_DEVICES: usize = 5;

/// Device slot size (64B for cache alignment)
const DEVICE_SLOT_SIZE: usize = 64;

/// Device rotation errors
#[derive(Debug)]
pub enum DeviceError {
    DeviceLimitExceeded { limit: u8 },
    IoError(std::io::Error),
    InvalidFormat,
    NotFound,
    IntegrityFailed,
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceLimitExceeded { limit } => {
                write!(f, "Device limit of {} exceeded", limit)
            }
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::InvalidFormat => write!(f, "Invalid device file format"),
            Self::NotFound => write!(f, "Device file not found"),
            Self::IntegrityFailed => {
                write!(f, "Device integrity check failed - tampering detected")
            }
        }
    }
}

impl std::error::Error for DeviceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DeviceError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

/// Device slot (64B cache-aligned)
///
/// Stores fingerprint hash and activation timestamp for a single device.
#[repr(C, align(64))]
struct DeviceSlot {
    /// Blake3 hash of hardware fingerprint
    fingerprint_hash: [u8; 32],

    /// Unix timestamp when device was activated
    timestamp: AtomicU64,

    /// Padding for 64B alignment
    _padding: [u8; 24],
}

impl DeviceSlot {
    /// Create empty slot
    const fn new() -> Self {
        Self {
            fingerprint_hash: [0u8; 32],
            timestamp: AtomicU64::new(0),
            _padding: [0u8; 24],
        }
    }

    /// Check if slot is empty
    fn is_empty(&self) -> bool {
        self.timestamp.load(Ordering::Acquire) == 0
    }

    /// Get timestamp
    fn timestamp(&self) -> u64 {
        self.timestamp.load(Ordering::Acquire)
    }

    /// Set device data
    fn set(&mut self, fingerprint_hash: &[u8; 32], timestamp: u64) {
        self.fingerprint_hash.copy_from_slice(fingerprint_hash);
        self.timestamp.store(timestamp, Ordering::Release);
    }

    /// Clear slot
    fn clear(&mut self) {
        self.fingerprint_hash.fill(0);
        self.timestamp.store(0, Ordering::Release);
    }

    /// Check if fingerprint matches
    fn matches(&self, fingerprint_hash: &[u8; 32]) -> bool {
        !self.is_empty() && &self.fingerprint_hash == fingerprint_hash
    }
}

// Compile-time size verification
const _: () = assert!(std::mem::size_of::<DeviceSlot>() == DEVICE_SLOT_SIZE);
const _: () = assert!(std::mem::align_of::<DeviceSlot>() == 64);

/// Device Rotation Capsule (384B, T9+T1)
///
/// Cache-aligned capsule for automatic device rotation with persistent storage.
/// When device limit+1 activated, oldest device is automatically replaced.
///
/// # Thread Safety
///
/// All state modifications use atomic operations. The capsule is safe to
/// share across threads. Disk persistence uses atomic write-then-rename.
///
/// # Anti-Piracy
///
/// - Generation counter increments on every state change
/// - Fingerprint hashes are one-way (cannot reverse to hardware ID)
/// - Automatic rotation prevents manual device slot manipulation
/// - Disk format versioned for future compatibility
#[repr(C, align(64))]
pub struct DeviceRotationCapsule {
    /// Current number of active devices
    device_count: AtomicU8,

    /// Padding for cache alignment
    _padding1: [u8; 7],

    /// Generation counter for tamper detection
    generation: AtomicU64,

    /// Last rotation timestamp
    last_rotation_timestamp: AtomicU64,

    /// Total number of rotations performed
    rotation_count: AtomicU64,

    /// Device slots (5 × 64B = 320B)
    devices: [DeviceSlot; 5],
}

// Compile-time size verification
// Actual size is 384B due to repr(C, align(64)) with DeviceSlot align(64)
// Layout: 0-31 (header) + 32-63 (auto-padding) + 64-383 (devices) = 384B (6 cache lines)
// #ASSUME: repr(C, align(64)) adds padding before devices array to align to 64B boundary
// #VERIFY: DeviceSlot align(64) causes devices to start at offset 64, not 32
const _: () = assert!(std::mem::size_of::<DeviceRotationCapsule>() == 384);
const _: () = assert!(std::mem::align_of::<DeviceRotationCapsule>() == 64);

impl DeviceRotationCapsule {
    /// Create new capsule
    pub const fn new() -> Self {
        Self {
            device_count: AtomicU8::new(0),
            _padding1: [0u8; 7],
            generation: AtomicU64::new(0),
            last_rotation_timestamp: AtomicU64::new(0),
            rotation_count: AtomicU64::new(0),
            devices: [
                DeviceSlot::new(),
                DeviceSlot::new(),
                DeviceSlot::new(),
                DeviceSlot::new(),
                DeviceSlot::new(),
            ],
        }
    }

    /// Activate device with automatic rotation
    ///
    /// If device limit is exceeded, the oldest device is automatically replaced.
    /// Returns true if rotation occurred.
    ///
    /// # Arguments
    ///
    /// * `fingerprint` - Hardware fingerprint of device to activate
    /// * `limit` - Maximum allowed devices for tier
    ///
    /// # Errors
    ///
    /// Returns error if integrity check fails or I/O error occurs during persist.
    pub fn activate_device(
        &mut self,
        fingerprint: &HardwareFingerprint,
        limit: u8,
    ) -> Result<bool, DeviceError> {
        // Verify integrity before modification
        if !self.verify_integrity() {
            return Err(DeviceError::IntegrityFailed);
        }

        // #ASSUME: limit is within MAX_DEVICES
        // #VERIFY: Caller (TierEnforcementCapsule) ensures limit ≤ MAX_DEVICES
        let limit = limit.min(MAX_DEVICES as u8);

        // Hash fingerprint for storage
        let fingerprint_hash = Self::hash_fingerprint(fingerprint);

        // Check if device already activated
        for slot in &self.devices[..limit as usize] {
            if slot.matches(&fingerprint_hash) {
                // Device already active - no action needed
                return Ok(false);
            }
        }

        let count = self.device_count.load(Ordering::Acquire);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let rotated = if count >= limit {
            // Rotation required - find oldest device
            self.rotate_oldest_device(&fingerprint_hash, now);
            true
        } else {
            // Add to first empty slot
            self.add_device(&fingerprint_hash, now, limit);
            false
        };

        // Increment generation counter
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Persist to disk
        self.persist_to_disk()?;

        Ok(rotated)
    }

    /// Deactivate current device
    ///
    /// Removes the current device from the active device list.
    pub fn deactivate_device(&mut self, fingerprint: &HardwareFingerprint) -> Result<(), DeviceError> {
        let fingerprint_hash = Self::hash_fingerprint(fingerprint);

        // Find and clear matching slot
        let mut found = false;
        for slot in &mut self.devices {
            if slot.matches(&fingerprint_hash) {
                slot.clear();
                found = true;
                break;
            }
        }

        if found {
            // Decrement count
            let count = self.device_count.load(Ordering::Acquire);
            if count > 0 {
                self.device_count.store(count - 1, Ordering::Release);
            }

            // Increment generation
            self.generation.fetch_add(1, Ordering::AcqRel);

            // Persist
            self.persist_to_disk()?;
        }

        Ok(())
    }

    /// Check if current device is activated
    pub fn is_device_activated(&self, fingerprint: &HardwareFingerprint) -> bool {
        let fingerprint_hash = Self::hash_fingerprint(fingerprint);

        for slot in &self.devices {
            if slot.matches(&fingerprint_hash) {
                return true;
            }
        }

        false
    }

    /// Get current device count
    #[inline]
    pub fn device_count(&self) -> u8 {
        self.device_count.load(Ordering::Acquire)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get rotation count
    #[inline]
    pub fn rotation_count(&self) -> u64 {
        self.rotation_count.load(Ordering::Acquire)
    }

    /// Get last rotation timestamp
    #[inline]
    pub fn last_rotation_time(&self) -> u64 {
        self.last_rotation_timestamp.load(Ordering::Acquire)
    }

    /// Verify integrity
    ///
    /// Checks that device count matches actual active slots.
    #[inline]
    pub fn verify_integrity(&self) -> bool {
        let count = self.device_count.load(Ordering::Acquire) as usize;

        // Count non-empty slots
        let active_slots = self.devices.iter().filter(|s| !s.is_empty()).count();

        count == active_slots
    }

    /// Activate device (in-memory only, no disk persistence)
    ///
    /// Test-only version that skips disk I/O.
    #[cfg(test)]
    pub fn activate_device_in_memory(
        &mut self,
        fingerprint: &HardwareFingerprint,
        limit: u8,
    ) -> Result<bool, DeviceError> {
        // Verify integrity before modification
        if !self.verify_integrity() {
            return Err(DeviceError::IntegrityFailed);
        }

        let limit = limit.min(MAX_DEVICES as u8);
        let fingerprint_hash = Self::hash_fingerprint(fingerprint);

        // Check if device already activated
        for slot in &self.devices[..limit as usize] {
            if slot.matches(&fingerprint_hash) {
                return Ok(false);
            }
        }

        let count = self.device_count.load(Ordering::Acquire);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let rotated = if count >= limit {
            self.rotate_oldest_device(&fingerprint_hash, now);
            true
        } else {
            self.add_device(&fingerprint_hash, now, limit);
            false
        };

        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(rotated)
    }

    /// Deactivate device (in-memory only, no disk persistence)
    #[cfg(test)]
    pub fn deactivate_device_in_memory(
        &mut self,
        fingerprint: &HardwareFingerprint,
    ) -> Result<(), DeviceError> {
        let fingerprint_hash = Self::hash_fingerprint(fingerprint);

        let mut found = false;
        for slot in &mut self.devices {
            if slot.matches(&fingerprint_hash) {
                slot.clear();
                found = true;
                break;
            }
        }

        if found {
            let count = self.device_count.load(Ordering::Acquire);
            if count > 0 {
                self.device_count.store(count - 1, Ordering::Release);
            }
            self.generation.fetch_add(1, Ordering::AcqRel);
        }

        Ok(())
    }

    /// Persist to disk for offline validation
    ///
    /// Uses atomic write-then-rename to prevent corruption.
    pub fn persist_to_disk(&self) -> Result<(), DeviceError> {
        let path = Self::device_path()?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut buffer = Vec::with_capacity(352);

        // Magic bytes
        buffer.extend_from_slice(&LICENSE_MAGIC);

        // Version
        buffer.push(LICENSE_VERSION);

        // Device count
        buffer.push(self.device_count.load(Ordering::Acquire));

        // Padding
        buffer.extend_from_slice(&[0u8; 2]);

        // Generation
        buffer.extend_from_slice(&self.generation.load(Ordering::Acquire).to_le_bytes());

        // Last rotation timestamp
        buffer.extend_from_slice(
            &self
                .last_rotation_timestamp
                .load(Ordering::Acquire)
                .to_le_bytes(),
        );

        // Rotation count
        buffer.extend_from_slice(&self.rotation_count.load(Ordering::Acquire).to_le_bytes());

        // Device slots
        for slot in &self.devices {
            buffer.extend_from_slice(&slot.fingerprint_hash);
            buffer.extend_from_slice(&slot.timestamp().to_le_bytes());
            buffer.extend_from_slice(&[0u8; 24]); // padding
        }

        // Write atomically (write to temp, then rename)
        let temp_path = path.with_extension("tmp");
        let mut file = File::create(&temp_path)?;
        file.write_all(&buffer)?;
        file.sync_all()?;

        fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Load from disk
    pub fn load_from_disk(&mut self) -> Result<(), DeviceError> {
        let path = Self::device_path()?;

        if !path.exists() {
            return Err(DeviceError::NotFound);
        }

        let mut file = File::open(&path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        // Verify minimum size
        if buffer.len() < 352 {
            return Err(DeviceError::InvalidFormat);
        }

        // Verify magic bytes
        if buffer[0..4] != LICENSE_MAGIC {
            return Err(DeviceError::InvalidFormat);
        }

        // Verify version
        if buffer[4] != LICENSE_VERSION {
            return Err(DeviceError::InvalidFormat);
        }

        // Parse fields
        let device_count = buffer[5];
        let generation = u64::from_le_bytes(buffer[8..16].try_into().unwrap());
        let last_rotation = u64::from_le_bytes(buffer[16..24].try_into().unwrap());
        let rotation_count = u64::from_le_bytes(buffer[24..32].try_into().unwrap());

        // Restore state
        self.device_count.store(device_count, Ordering::Release);
        self.generation.store(generation, Ordering::Release);
        self.last_rotation_timestamp
            .store(last_rotation, Ordering::Release);
        self.rotation_count
            .store(rotation_count, Ordering::Release);

        // Restore device slots
        let mut offset = 32;
        for slot in &mut self.devices {
            slot.fingerprint_hash
                .copy_from_slice(&buffer[offset..offset + 32]);
            let timestamp = u64::from_le_bytes(
                buffer[offset + 32..offset + 40]
                    .try_into()
                    .unwrap(),
            );
            slot.timestamp.store(timestamp, Ordering::Release);
            offset += DEVICE_SLOT_SIZE;
        }

        Ok(())
    }

    /// Hash fingerprint for storage
    fn hash_fingerprint(fingerprint: &HardwareFingerprint) -> [u8; 32] {
        // Already a Blake3 hash, just return the bytes
        *fingerprint.as_bytes()
    }

    /// Add device to first empty slot
    fn add_device(&mut self, fingerprint_hash: &[u8; 32], timestamp: u64, limit: u8) {
        for slot in &mut self.devices[..limit as usize] {
            if slot.is_empty() {
                slot.set(fingerprint_hash, timestamp);

                // Increment count
                let count = self.device_count.load(Ordering::Acquire);
                self.device_count.store(count + 1, Ordering::Release);
                break;
            }
        }
    }

    /// Rotate oldest device
    fn rotate_oldest_device(&mut self, fingerprint_hash: &[u8; 32], timestamp: u64) {
        // Find oldest slot
        let mut oldest_idx = 0;
        let mut oldest_time = u64::MAX;

        for (i, slot) in self.devices.iter().enumerate() {
            if !slot.is_empty() {
                let ts = slot.timestamp();
                if ts < oldest_time {
                    oldest_time = ts;
                    oldest_idx = i;
                }
            }
        }

        // Replace oldest
        self.devices[oldest_idx].set(fingerprint_hash, timestamp);

        // Update rotation metadata
        self.rotation_count.fetch_add(1, Ordering::AcqRel);
        self.last_rotation_timestamp
            .store(timestamp, Ordering::Release);
    }

    /// Get platform-specific device file path
    fn device_path() -> Result<PathBuf, DeviceError> {
        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME").map_err(|_| {
                DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "HOME not set",
                ))
            })?;
            Ok(PathBuf::from(home)
                .join(".config")
                .join(APP_NAME)
                .join("devices.bin"))
        }

        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").map_err(|_| {
                DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "HOME not set",
                ))
            })?;
            Ok(PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join(APP_NAME)
                .join("devices.bin"))
        }

        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("APPDATA").map_err(|_| {
                DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "APPDATA not set",
                ))
            })?;
            Ok(PathBuf::from(appdata).join(APP_NAME).join("devices.bin"))
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(DeviceError::IoError(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Unsupported platform",
            )))
        }
    }
}

impl Default for DeviceRotationCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: All fields are atomic or protected by &mut self
// #ASSUME: DeviceSlot atomics are Send + Sync
// #VERIFY: Modification requires &mut self, all reads use atomic ordering
unsafe impl Send for DeviceRotationCapsule {}
unsafe impl Sync for DeviceRotationCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        // 384B = 8B (count + padding) + 8B (gen) + 8B (ts) + 8B (rot_cnt) + 320B (5×64B slots) + 32B padding
        assert_eq!(std::mem::size_of::<DeviceRotationCapsule>(), 384);
        assert_eq!(std::mem::align_of::<DeviceRotationCapsule>(), 64);
    }

    #[test]
    fn test_device_slot_size_and_alignment() {
        assert_eq!(std::mem::size_of::<DeviceSlot>(), 64);
        assert_eq!(std::mem::align_of::<DeviceSlot>(), 64);
    }

    #[test]
    fn test_new_capsule_empty() {
        let capsule = DeviceRotationCapsule::new();
        assert_eq!(capsule.device_count(), 0);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.rotation_count(), 0);
    }

    #[test]
    fn test_activate_first_device() {
        let mut capsule = DeviceRotationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        let rotated = capsule.activate_device_in_memory(&fp, 2).unwrap();
        assert!(!rotated);
        assert_eq!(capsule.device_count(), 1);
        assert!(capsule.is_device_activated(&fp));
    }

    #[test]
    fn test_activate_multiple_devices() {
        let mut capsule = DeviceRotationCapsule::new();
        let fp1 = HardwareFingerprint::from_bytes([0xAA; 32]);
        let fp2 = HardwareFingerprint::from_bytes([0xBB; 32]);

        capsule.activate_device_in_memory(&fp1, 2).unwrap();
        capsule.activate_device_in_memory(&fp2, 2).unwrap();

        assert_eq!(capsule.device_count(), 2);
        assert!(capsule.is_device_activated(&fp1));
        assert!(capsule.is_device_activated(&fp2));
    }

    #[test]
    fn test_automatic_rotation() {
        let mut capsule = DeviceRotationCapsule::new();
        let fp1 = HardwareFingerprint::from_bytes([0xAA; 32]);
        let fp2 = HardwareFingerprint::from_bytes([0xBB; 32]);
        let fp3 = HardwareFingerprint::from_bytes([0xCC; 32]);

        // Activate 2 devices (limit is 2)
        capsule.activate_device_in_memory(&fp1, 2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        capsule.activate_device_in_memory(&fp2, 2).unwrap();

        assert_eq!(capsule.device_count(), 2);
        assert_eq!(capsule.rotation_count(), 0);

        // Third device should trigger rotation
        std::thread::sleep(std::time::Duration::from_millis(10));
        let rotated = capsule.activate_device_in_memory(&fp3, 2).unwrap();

        assert!(rotated);
        assert_eq!(capsule.device_count(), 2);
        assert_eq!(capsule.rotation_count(), 1);

        // First device should be removed (oldest)
        assert!(!capsule.is_device_activated(&fp1));
        assert!(capsule.is_device_activated(&fp2));
        assert!(capsule.is_device_activated(&fp3));
    }

    #[test]
    fn test_deactivate_device() {
        let mut capsule = DeviceRotationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        capsule.activate_device_in_memory(&fp, 2).unwrap();
        assert_eq!(capsule.device_count(), 1);

        capsule.deactivate_device_in_memory(&fp).unwrap();
        assert_eq!(capsule.device_count(), 0);
        assert!(!capsule.is_device_activated(&fp));
    }

    #[test]
    fn test_activate_same_device_twice() {
        let mut capsule = DeviceRotationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        capsule.activate_device_in_memory(&fp, 2).unwrap();
        let rotated = capsule.activate_device_in_memory(&fp, 2).unwrap();

        // Should not rotate, count should not change
        assert!(!rotated);
        assert_eq!(capsule.device_count(), 1);
    }

    #[test]
    fn test_integrity_verification() {
        let mut capsule = DeviceRotationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        capsule.activate_device_in_memory(&fp, 2).unwrap();
        assert!(capsule.verify_integrity());

        // Simulate tampering: increment count without adding device
        let count = capsule.device_count.load(Ordering::Acquire);
        capsule.device_count.store(count + 1, Ordering::Release);
        assert!(!capsule.verify_integrity());
    }

    #[test]
    fn test_generation_counter_increments() {
        let mut capsule = DeviceRotationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        assert_eq!(capsule.generation(), 0);

        capsule.activate_device_in_memory(&fp, 2).unwrap();
        assert_eq!(capsule.generation(), 1);

        capsule.deactivate_device_in_memory(&fp).unwrap();
        assert_eq!(capsule.generation(), 2);
    }
}
