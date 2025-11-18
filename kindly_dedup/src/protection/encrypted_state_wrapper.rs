//! EncryptedState Wrapper - kindly_dedup Integration
//!
//! **Purpose**: Wrapper for atomic_capsule::protection::EncryptedStateCapsule
//!
//! # Architecture
//! - **T9 Persistent**: Mmap-backed AES-256-GCM encrypted state
//! - **T0 Auditable**: AtomicHash256 tamper detection
//! - **SeqLock**: TOCTOU prevention via generation counter
//!
//! # Integration (I20 Q1-Q20)
//! - Q1: Replaces plaintext state storage with encrypted persistence
//! - Q6: Lockfree (AtomicHash256, SeqLock), compatible with kindly_dedup
//! - Q7: <100ns read, <50ns write (<0.01% overhead)
//! - Q19: Big Bang deployment (deterministic capsule)
//!
//! # Usage
//! ```rust,no_run
//! use kindly_dedup::protection::EncryptedStateWrapper;
//!
//! // Derive key from hardware
//! let wrapper = EncryptedStateWrapper::new_from_hardware()?;
//!
//! // Write encrypted state (<50ns + 5ms fsync amortized)
//! wrapper.write_state(b"sensitive data")?;
//!
//! // Read encrypted state (<100ns page cache hit)
//! let data = wrapper.read_state()?;
//! ```

#![cfg(feature = "protection-encrypted-state")]

use super::{HardwareId, MetaCapsuleError, PufEntropy};
use std::path::{Path, PathBuf};

// Re-export from atomic_capsule
#[cfg(feature = "protection-encrypted-state")]
pub use atomic_capsule::protection::encrypted_state::{EncryptedStateCapsule, StateError};

/// Wrapper for EncryptedStateCapsule with kindly_dedup-specific key derivation
///
/// **UCE34 Q10**: T9 Persistent + T0 Auditable
/// **I20 Q6**: Lockfree, compatible with kindly_dedup
/// **I20 Q7**: <100ns read, <50ns write
pub struct EncryptedStateWrapper {
    capsule: EncryptedStateCapsule,
    key: [u8; 32],
    path: PathBuf,
}

impl EncryptedStateWrapper {
    /// Create new encrypted state with hardware-derived key
    ///
    /// **Key Derivation**: HKDF-SHA256(HardwareId || PufEntropy)
    ///
    /// # Errors
    /// - `MetaCapsuleError::HardwareIdFailed`: Hardware ID extraction failed
    /// - `MetaCapsuleError::PufFailed`: PUF extraction failed
    /// - `MetaCapsuleError::EncryptionFailed`: State file creation failed
    ///
    /// # Performance
    /// - ~6ms (hardware ID 500µs + PUF 5ms + file creation 500µs)
    pub fn new_from_hardware() -> Result<Self, MetaCapsuleError> {
        // Derive hardware ID
        let hw_id = HardwareId::derive()?;

        // Extract PUF entropy
        let puf = PufEntropy::extract()?;

        // Derive AES key
        let key = Self::derive_state_key(&hw_id, &puf)?;

        // Default state file path
        let path = Self::default_state_path()?;

        // Create/open encrypted state file
        let capsule = EncryptedStateCapsule::create(&path, &key)
            .map_err(|e| MetaCapsuleError::EncryptionFailed(format!("State file creation failed: {:?}", e)))?;

        Ok(Self { capsule, key, path })
    }

    /// Create encrypted state at specific path
    ///
    /// **Key Derivation**: Same as `new_from_hardware`
    ///
    /// # Errors
    /// - Same as `new_from_hardware`
    pub fn new_at_path<P: AsRef<Path>>(path: P) -> Result<Self, MetaCapsuleError> {
        let hw_id = HardwareId::derive()?;
        let puf = PufEntropy::extract()?;
        let key = Self::derive_state_key(&hw_id, &puf)?;

        let capsule = EncryptedStateCapsule::create(&path, &key)
            .map_err(|e| MetaCapsuleError::EncryptionFailed(format!("State file creation failed: {:?}", e)))?;

        Ok(Self {
            capsule,
            key,
            path: path.as_ref().to_path_buf(),
        })
    }

    /// Open existing encrypted state file
    ///
    /// **Requires**: Hardware ID + PUF match original system (or file won't decrypt)
    ///
    /// # Errors
    /// - `MetaCapsuleError::EncryptionFailed`: State file open/decrypt failed
    pub fn open_from_hardware<P: AsRef<Path>>(path: P) -> Result<Self, MetaCapsuleError> {
        let hw_id = HardwareId::derive()?;
        let puf = PufEntropy::extract()?;
        let key = Self::derive_state_key(&hw_id, &puf)?;

        let capsule = EncryptedStateCapsule::open(&path, &key)
            .map_err(|e| MetaCapsuleError::EncryptionFailed(format!("State file open failed: {:?}", e)))?;

        Ok(Self {
            capsule,
            key,
            path: path.as_ref().to_path_buf(),
        })
    }

    /// Write encrypted state
    ///
    /// **Encryption**: AES-256-GCM authenticated encryption
    ///
    /// # Performance
    /// - <50ns (atomic update) + <5ms (fsync, amortized)
    ///
    /// # Errors
    /// - `MetaCapsuleError::EncryptionFailed`: Write/encryption failed
    pub fn write_state(&self, data: &[u8]) -> Result<(), MetaCapsuleError> {
        self.capsule.write(data, &self.key)?;
        Ok(())
    }

    /// Read encrypted state
    ///
    /// **Decryption**: AES-256-GCM authenticated decryption + integrity check
    ///
    /// # Performance
    /// - <100ns (page cache hit)
    ///
    /// # Errors
    /// - `MetaCapsuleError::EncryptionFailed`: Read/decryption failed
    pub fn read_state(&self) -> Result<Vec<u8>, MetaCapsuleError> {
        Ok(self.capsule.read(&self.key)?)
    }

    /// Verify state integrity (<30ns, AtomicHash256)
    ///
    /// **Verification**: SHA-256 hash comparison
    ///
    /// # Performance
    /// - <30ns (atomic load + compare)
    #[inline]
    pub fn verify_integrity(&self) -> bool {
        self.capsule.verify_integrity()
    }

    /// Sync state to disk (<5ms)
    ///
    /// **Durability**: msync(MS_SYNC) memory barrier
    ///
    /// # Errors
    /// - `MetaCapsuleError::EncryptionFailed`: Sync failed
    pub fn sync(&self) -> Result<(), MetaCapsuleError> {
        self.capsule.sync()?;
        Ok(())
    }

    /// Get state file path
    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Derive AES-256 key from hardware ID + PUF entropy
    ///
    /// **Algorithm**: HKDF-SHA256(HardwareId || PufEntropy)
    ///
    /// # ASSUM Framework
    /// - #ASSUME_HKDF_SECURE: HKDF-SHA256 provides secure key derivation (RFC 5869)
    /// - #VERIFY_KEY_UNIQUENESS: Test different hardware produces different keys
    fn derive_state_key(hw_id: &HardwareId, puf: &PufEntropy) -> Result<[u8; 32], MetaCapsuleError> {
        use hkdf::Hkdf;
        use sha2::Sha256;

        // Combine hardware ID + PUF entropy
        let mut input = Vec::with_capacity(64);
        input.extend_from_slice(hw_id.as_bytes());
        input.extend_from_slice(puf.as_bytes());

        // HKDF-SHA256 key derivation
        let hkdf = Hkdf::<Sha256>::new(Some(b"kindly_dedup_encrypted_state_salt_v1"), &input);

        let mut key = [0u8; 32];
        hkdf.expand(b"EncryptedStateCapsule-v1", &mut key)
            .map_err(|e| MetaCapsuleError::EncryptionFailed(format!("Key derivation failed: {:?}", e)))?;

        Ok(key)
    }

    /// Get default state file path (~/.kindly_dedup/encrypted_state.bin)
    fn default_state_path() -> Result<PathBuf, MetaCapsuleError> {
        let home = dirs::home_dir()
            .ok_or_else(|| MetaCapsuleError::EncryptionFailed("Home directory not found".to_string()))?;

        let kindly_dir = home.join(".kindly_dedup");
        std::fs::create_dir_all(&kindly_dir)
            .map_err(|e| MetaCapsuleError::EncryptionFailed(format!("Failed to create config directory: {}", e)))?;

        Ok(kindly_dir.join("encrypted_state.bin"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_encrypted_state_wrapper_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_state.bin");

        let wrapper = EncryptedStateWrapper::new_at_path(&path).unwrap();

        // Write data
        let original = b"test sensitive data";
        wrapper.write_state(original).unwrap();

        // Read data
        let decrypted = wrapper.read_state().unwrap();
        assert_eq!(decrypted, original);

        // Verify integrity
        assert!(wrapper.verify_integrity());
    }

    #[test]
    fn test_encrypted_state_wrapper_integrity() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_state2.bin");

        let wrapper = EncryptedStateWrapper::new_at_path(&path).unwrap();

        // Initial integrity should be valid (empty state)
        assert!(wrapper.verify_integrity());

        // Write and verify
        wrapper.write_state(b"data").unwrap();
        assert!(wrapper.verify_integrity());
    }
}
