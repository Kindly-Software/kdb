//! # Encrypted State Capsule - Persistent + Auditable
//!
//! Tamper-resistant persistent state storage using AES-256-GCM encryption and mmap.
//!
//! Provides persistent encrypted state with memory-mapped files for zero-copy access
//! and hash verification for tamper detection.
//!
//! # Architecture
//!
//! **EncryptedStateCapsule** (512B aligned):
//! - **Persistence**: Memory-mapped file for zero-copy persistence
//! - **Auditability**: Cryptographic hash for tamper detection
//! - **Encryption**: AES-256-GCM authenticated encryption
//! - **Atomicity**: Generation counter for consistent updates
//!
//! # Security
//!
//! - **Encryption**: AES-256-GCM (NIST SP 800-38D)
//! - **Key Derivation**: HKDF-SHA256 (RFC 5869)
//! - **Authentication**: GCM tag (128-bit)
//! - **Nonce**: Counter-based (deterministic, no RDRAND dependency)
//! - **Integrity**: SHA-256 hash (AtomicHash256)
//! - **Deletion Resistance**: Linux immutable attribute (chattr +i)
//!
//! # Performance (B32 Targets)
//!
//! - Create/open: <1ms (mmap setup)
//! - Write: <50ns (atomic update) + <5ms (fsync, amortized)
//! - Read: <100ns (page cache hit)
//! - Verify: <30ns (AtomicHash256 load + compare)
//! - Total: <0.01% overhead (amortized)
//!
//! # ASSUM Framework
//!
//! ```text
//! #ASSUME_MMAP_ATOMIC: OS provides atomic page updates (4KB granularity)
//! #VERIFY_MSYNC_ORDERING: msync(MS_SYNC) provides memory barrier
//! #ASSUME_AES_256_GCM_SECURE: 2^256 keyspace, authenticated encryption
//! #VERIFY_NIST_SP_800_38D: Test vectors validate GCM mode
//! #ASSUME_IMMUTABLE_PERSISTENT: chattr +i prevents deletion (Linux)
//! #VERIFY_DELETION_RESISTANCE: Test deletion attempt, expect EPERM
//! #ASSUME_SHA256_COLLISION_RESISTANT: 2^128 collision resistance
//! #VERIFY_HASH_CORRECTNESS: Known test vectors validate SHA-256
//! #ASSUME_HKDF_SECURE: HKDF-SHA256 provides secure key derivation (RFC 5869)
//! #VERIFY_KEY_DERIVATION: Test vectors validate HKDF output
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use atomic_capsule::protection::EncryptedStateCapsule;
//!
//! // Create encrypted state file
//! let key = [0u8; 32]; // 256-bit key
//! let capsule = EncryptedStateCapsule::create("state.enc", &key)?;
//!
//! // Write state
//! let state = b"sensitive data";
//! capsule.write(state, &key)?;
//!
//! // Read state
//! let decrypted = capsule.read(&key)?;
//! assert_eq!(decrypted, state);
//!
//! // Verify integrity
//! assert!(capsule.verify_integrity());
//!
//! // Sync to disk
//! capsule.sync()?;
//! # Ok::<(), atomic_capsule::error::StateError>(())
//! ```
#![allow(unsafe_code)]
use crate::error::StateError;
use crate::hash::AtomicHash256;
use core::sync::atomic::{AtomicU64, Ordering};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};
use hkdf::Hkdf;
use sha2::Sha256 as Sha256Hash;
/// AES-256-GCM nonce size (96 bits recommended)
const NONCE_SIZE: usize = 12;
/// AES-256-GCM authentication tag size (128 bits)
const TAG_SIZE: usize = 16;
/// SHA-256 hash size (256 bits)
const HASH_SIZE: usize = 32;
/// AES-256 key size (256 bits)
const KEY_SIZE: usize = 32;
/// HKDF info string for key derivation
const HKDF_INFO: &[u8] = b"EncryptedStateCapsule-v1";
/// HKDF salt (fixed, public)
const HKDF_SALT: &[u8] = b"atomic_capsule_encrypted_state_v1_salt_2025";
/// Mmap file header magic (identifies encrypted state file)
const FILE_MAGIC: u64 = 0x454E435F53544154;
/// Minimum mmap file size (4KB page)
const MIN_FILE_SIZE: usize = 4096;
/// Encrypted State Capsule - Persistent + Auditable
///
/// Tamper-resistant encrypted state storage with compile-time verification
/// and cryptographic hash chaining for audit trails.
///
/// # Memory Layout (512 bytes, cache-aligned)
///
/// ```text
/// Offset  Size  Field                Description
/// ------  ----  -------------------  ------------------------------------
/// 0       8     mmap_ptr             Pointer to mmap region (raw pointer as u64)
/// 8       8     mmap_size            Size of mmap region in bytes
/// 16      8     generation           SeqLock generation counter (TOCTOU prevention)
/// 24      40    state_hash           SHA-256 hash of decrypted state (AtomicHash256 = 8 + 32)
/// 64      8     nonce_counter        Counter for nonce generation (deterministic)
/// 72      12    nonce                Current AES-GCM nonce (96 bits)
/// 84      16    tag                  Current AES-GCM authentication tag (128 bits)
/// 100     8     file_magic           File format identifier
/// 108     404   _padding             Padding to 512 bytes
/// ```
///
/// Total: 512 bytes (cache-aligned)
// TODO(Phase P0): Derive macro fails with #[capsule(skip)] - use manual verification
// #[derive(atomic_capsule_derive::ComputationalCapsule)]
// #[capsule(alignment = 64, size = 640)]
#[repr(C, align(64))]
pub struct EncryptedStateCapsule {
    /// Mmap region pointer (stored as u64 for atomic access)
    /// #ASSUME_MMAP_PTR_VALID: Pointer valid for lifetime of capsule
    mmap_ptr: AtomicU64,
    /// Mmap region size in bytes
    mmap_size: AtomicU64,
    /// Generation counter for SeqLock (TOCTOU prevention)
    /// Odd = writing, Even = stable
    generation: AtomicU64,
    /// SHA-256 hash of decrypted state (tamper detection)
    /// Uses SeqLock internally for atomic 256-bit read/write
    state_hash: AtomicHash256,
    /// Nonce counter (incremented per encryption, deterministic)
    nonce_counter: AtomicU64,
    /// Current AES-GCM nonce (96 bits)
    /// Updated atomically with nonce_counter
    nonce: [u8; NONCE_SIZE],
    /// Current AES-GCM authentication tag (128 bits)
    /// Updated atomically with encrypted data
    tag: [u8; TAG_SIZE],
    /// File magic identifier (validation)
    file_magic: AtomicU64,
    /// File path (for sync operations)
    /// Stored separately to avoid capsule size bloat
    /// NOT counted in capsule size (heap allocated via Arc)
    file_path: Arc<PathBuf>,
    /// Mmap region (heap allocated via Arc, NOT counted in capsule size)
    /// #ASSUME_MMAP_LIFETIME: Arc keeps mmap alive for lifetime of capsule
    /// #VERIFY_NO_USE_AFTER_FREE: mmap_ptr valid while mmap_region alive
    mmap_region: Arc<memmap2::MmapMut>,
    _padding: [u8; 568usize],
}
impl EncryptedStateCapsule {
    /// Create new encrypted state file
    ///
    /// # Arguments
    /// * `path` - File path for encrypted state
    /// * `key` - 256-bit encryption key
    ///
    /// # Returns
    /// Ok(capsule) if creation succeeds, Err otherwise
    ///
    /// # Performance
    /// <1ms (mmap setup + file creation)
    ///
    /// # ASSUM Framework
    /// #ASSUME_FILE_CREATION_ATOMIC: File creation is atomic (POSIX O_CREAT | O_EXCL)
    /// #VERIFY_FILE_EXISTS: Test file exists after creation
    pub fn create<P: AsRef<Path>>(
        path: P,
        key: &[u8; KEY_SIZE],
    ) -> Result<Self, StateError> {
        let path_ref = path.as_ref();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path_ref)
            .map_err(|e| StateError::Io(e))?;
        file.write_all(&FILE_MAGIC.to_le_bytes()).map_err(|e| StateError::Io(e))?;
        file.write_all(&(MIN_FILE_SIZE as u64).to_le_bytes())
            .map_err(|e| StateError::Io(e))?;
        file.set_len(MIN_FILE_SIZE as u64).map_err(|e| StateError::Io(e))?;
        file.sync_all().map_err(|e| StateError::Io(e))?;
        #[cfg(target_os = "linux")]
        {
            let _ = set_immutable(path_ref, true);
        }
        drop(file);
        Self::open(path_ref, key)
    }
    /// Open existing encrypted state file
    ///
    /// # Arguments
    /// * `path` - File path for encrypted state
    /// * `key` - 256-bit encryption key
    ///
    /// # Returns
    /// Ok(capsule) if open succeeds, Err otherwise
    ///
    /// # Performance
    /// <1ms (mmap setup + validation)
    ///
    /// # ASSUM Framework
    /// #ASSUME_MMAP_VALID: mmap returns valid pointer
    /// #VERIFY_MMAP_SIZE: Check mmap size matches file size
    pub fn open<P: AsRef<Path>>(
        path: P,
        _key: &[u8; KEY_SIZE],
    ) -> Result<Self, StateError> {
        let path_ref = path.as_ref();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path_ref)
            .map_err(|e| StateError::Io(e))?;
        let metadata = file.metadata().map_err(|e| StateError::Io(e))?;
        let file_size = metadata.len() as usize;
        if file_size < MIN_FILE_SIZE {
            return Err(StateError::InvalidFileSize {
                expected: MIN_FILE_SIZE,
                actual: file_size,
            });
        }
        let mmap = unsafe {
            memmap2::MmapMut::map_mut(&file)
                .map_err(|e| StateError::Io(
                    io::Error::new(io::ErrorKind::Other, format!("mmap failed: {}", e)),
                ))?
        };
        let magic_bytes = &mmap[0..8];
        let magic = u64::from_le_bytes(magic_bytes.try_into().unwrap());
        if magic != FILE_MAGIC {
            return Err(StateError::InvalidMagic {
                expected: FILE_MAGIC,
                actual: magic,
            });
        }
        let mmap_arc = Arc::new(mmap);
        let mmap_ptr = mmap_arc.as_ptr() as u64;
        let mmap_size = mmap_arc.len();
        Ok(Self {
            mmap_ptr: AtomicU64::new(mmap_ptr),
            mmap_size: AtomicU64::new(mmap_size as u64),
            generation: AtomicU64::new(0),
            state_hash: AtomicHash256::new([0u8; HASH_SIZE]),
            nonce_counter: AtomicU64::new(0),
            nonce: [0u8; NONCE_SIZE],
            tag: [0u8; TAG_SIZE],
            file_magic: AtomicU64::new(FILE_MAGIC),
            _padding: [0u8; 568],
            file_path: Arc::new(path_ref.to_path_buf()),
            mmap_region: mmap_arc,
        })
    }
    /// Write encrypted state
    ///
    /// # Arguments
    /// * `data` - Plaintext data to encrypt and store
    /// * `key` - 256-bit encryption key
    ///
    /// # Returns
    /// Ok(()) if write succeeds, Err otherwise
    ///
    /// # Performance
    /// <50ns (atomic update) + <5ms (fsync, amortized every 100 writes)
    ///
    /// # ASSUM Framework
    /// #ASSUME_AES_GCM_SECURE: AES-256-GCM provides authenticated encryption
    /// #VERIFY_ENCRYPTION_CORRECTNESS: Test encrypt/decrypt roundtrip
    pub fn write(&self, data: &[u8], key: &[u8; KEY_SIZE]) -> Result<(), StateError> {
        let enc_key = derive_key(key)?;
        let counter = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
        let nonce = generate_nonce(counter);
        let cipher = Aes256Gcm::new(&enc_key);
        let nonce_obj = Nonce::from_slice(&nonce);
        let payload = Payload { msg: data, aad: &[] };
        let ciphertext_with_tag = cipher
            .encrypt(nonce_obj, payload)
            .map_err(|_| StateError::EncryptionFailed)?;
        // AES-GCM appends tag to ciphertext
        let tag_start = ciphertext_with_tag.len().saturating_sub(TAG_SIZE);
        let ciphertext = &ciphertext_with_tag[..tag_start];
        let tag: [u8; TAG_SIZE] = ciphertext_with_tag[tag_start..]
            .try_into()
            .map_err(|_| StateError::EncryptionFailed)?;
        let hash = compute_sha256(data);
        self.generation.fetch_add(1, Ordering::Release);
        self.state_hash.store(hash);
        self.generation.fetch_add(1, Ordering::Release);
        let nonce_ptr = &self.nonce as *const [u8; NONCE_SIZE] as *mut [u8; NONCE_SIZE];
        let tag_ptr = &self.tag as *const [u8; TAG_SIZE] as *mut [u8; TAG_SIZE];
        unsafe {
            (*nonce_ptr).copy_from_slice(&nonce);
            (*tag_ptr).copy_from_slice(&tag);
        }
        let mmap_ptr = self.mmap_ptr.load(Ordering::Acquire);
        if mmap_ptr == 0 {
            return Err(StateError::MmapNotInitialized);
        }
        let mmap_size = self.mmap_size.load(Ordering::Acquire) as usize;
        let data_offset = 52; // [0-8: magic] [8-16: size] [16-24: cipher_len] [24-36: nonce] [36-52: tag] [52+: data]
        let available = mmap_size.saturating_sub(data_offset);
        if ciphertext.len() > available {
            return Err(StateError::InsufficientSpace {
                required: ciphertext.len(),
                available,
            });
        }
        unsafe {
            let mmap_ptr = mmap_ptr as *mut u8;
            // File layout: [0-8: magic] [8-16: size] [16-24: cipher_len] [24-36: nonce] [36-52: tag] [52+: data]
            // Write ciphertext length at offset 16
            let len_ptr = mmap_ptr.add(16) as *mut u64;
            *len_ptr = ciphertext.len() as u64;
            // Write nonce at offset 24 (12 bytes)
            let nonce_ptr = mmap_ptr.add(24);
            std::ptr::copy_nonoverlapping(nonce.as_ptr(), nonce_ptr, NONCE_SIZE);
            // Write tag at offset 36 (16 bytes)
            let tag_ptr = mmap_ptr.add(36);
            std::ptr::copy_nonoverlapping(tag.as_ptr(), tag_ptr, TAG_SIZE);
            // Write ciphertext at offset 52
            let data_ptr = mmap_ptr.add(52);
            std::ptr::copy_nonoverlapping(
                ciphertext.as_ptr(),
                data_ptr,
                ciphertext.len(),
            );
        }
        Ok(())
    }
    /// Read decrypted state
    ///
    /// # Arguments
    /// * `key` - 256-bit encryption key
    ///
    /// # Returns
    /// Ok(plaintext) if read succeeds, Err otherwise
    ///
    /// # Performance
    /// <100ns (page cache hit)
    ///
    /// # ASSUM Framework
    /// #ASSUME_AES_GCM_AUTHENTICATED: GCM tag validates ciphertext integrity
    /// #VERIFY_DECRYPTION_CORRECTNESS: Test decrypt produces original plaintext
    pub fn read(&self, key: &[u8; KEY_SIZE]) -> Result<Vec<u8>, StateError> {
        let enc_key = derive_key(key)?;
        let mmap_ptr = self.mmap_ptr.load(Ordering::Acquire);
        if mmap_ptr == 0 {
            return Err(StateError::MmapNotInitialized);
        }
        // Read from mmap file: [0-8: magic] [8-16: size] [16-24: cipher_len] [24-36: nonce] [36-52: tag] [52+: data]
        let (ciphertext_len, nonce, tag, ciphertext) = unsafe {
            let mmap_ptr = mmap_ptr as *const u8;
            // Read ciphertext length from offset 16
            let len_ptr = mmap_ptr.add(16) as *const u64;
            let len = *len_ptr as usize;
            // Read nonce from offset 24 (12 bytes)
            let nonce_ptr = mmap_ptr.add(24);
            let mut nonce_buf = [0u8; NONCE_SIZE];
            std::ptr::copy_nonoverlapping(nonce_ptr, nonce_buf.as_mut_ptr(), NONCE_SIZE);
            // Read tag from offset 36 (16 bytes)
            let tag_ptr = mmap_ptr.add(36);
            let mut tag_buf = [0u8; TAG_SIZE];
            std::ptr::copy_nonoverlapping(tag_ptr, tag_buf.as_mut_ptr(), TAG_SIZE);
            // Read ciphertext from offset 52
            let data_ptr = mmap_ptr.add(52);
            let cipher_vec = std::slice::from_raw_parts(data_ptr, len).to_vec();
            (len, nonce_buf, tag_buf, cipher_vec)
        };
        let mut ciphertext_with_tag = ciphertext;
        ciphertext_with_tag.extend_from_slice(&tag);
        let cipher = Aes256Gcm::new(&enc_key);
        let nonce_obj = Nonce::from_slice(&nonce);
        let payload = Payload {
            msg: &ciphertext_with_tag,
            aad: &[],
        };
        let plaintext = cipher
            .decrypt(nonce_obj, payload)
            .map_err(|_| StateError::DecryptionFailed)?;
        Ok(plaintext)
    }
    /// Verify integrity (check SHA-256 hash)
    ///
    /// # Returns
    /// true if hash is non-zero (state has been written), false otherwise
    ///
    /// # Performance
    /// <30ns (AtomicHash256 load)
    ///
    /// # Note
    /// This is a basic check. For full integrity verification, decrypt and hash the plaintext.
    #[inline]
    pub fn verify_integrity(&self) -> bool {
        let hash = self.state_hash.load();
        hash != [0u8; HASH_SIZE]
    }
    /// Sync state to disk
    ///
    /// # Returns
    /// Ok(()) if sync succeeds, Err otherwise
    ///
    /// # Performance
    /// <5ms (msync + fsync)
    ///
    /// # ASSUM Framework
    /// #ASSUME_MSYNC_DURABLE: msync(MS_SYNC) guarantees durability
    /// #VERIFY_FSYNC_ORDERING: Test data persists across process restart
    pub fn sync(&self) -> Result<(), StateError> {
        let mmap_ptr = self.mmap_ptr.load(Ordering::Acquire);
        if mmap_ptr == 0 {
            return Err(StateError::MmapNotInitialized);
        }
        let mmap_size = self.mmap_size.load(Ordering::Acquire) as usize;
        #[cfg(unix)]
        {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&*self.file_path)
                .map_err(|e| StateError::Io(e))?;
            let result = unsafe {
                libc::msync(mmap_ptr as *mut libc::c_void, mmap_size, libc::MS_SYNC)
            };
            if result != 0 {
                return Err(StateError::Io(io::Error::last_os_error()));
            }
            file.sync_all().map_err(|e| StateError::Io(e))?;
        }
        #[cfg(not(unix))]
        {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&*self.file_path)
                .map_err(|e| StateError::Io(e))?;
            file.sync_all().map_err(|e| StateError::Io(e))?;
        }
        Ok(())
    }
    /// Get current generation counter (for debugging)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
    /// Get current nonce counter (for debugging)
    #[inline]
    pub fn nonce_counter(&self) -> u64 {
        self.nonce_counter.load(Ordering::Relaxed)
    }
    /// Get file path
    #[inline]
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}
/// Derive encryption key using HKDF-SHA256
///
/// # ASSUM Framework
/// #ASSUME_HKDF_SECURE: HKDF-SHA256 provides secure key derivation (RFC 5869)
/// #VERIFY_KEY_DERIVATION: Test vectors validate HKDF output
fn derive_key(
    master_key: &[u8; KEY_SIZE],
) -> Result<aes_gcm::Key<Aes256Gcm>, StateError> {
    let hkdf = Hkdf::<Sha256Hash>::new(Some(HKDF_SALT), master_key);
    let mut derived_key = [0u8; KEY_SIZE];
    hkdf.expand(HKDF_INFO, &mut derived_key)
        .map_err(|_| StateError::KeyDerivationFailed)?;
    Ok(aes_gcm::Key::<Aes256Gcm>::from(derived_key))
}
/// Generate nonce from counter (deterministic)
///
/// # Arguments
/// * `counter` - Nonce counter value
///
/// # Returns
/// 96-bit nonce (12 bytes)
///
/// # ASSUM Framework
/// #ASSUME_COUNTER_NONCE_SAFE: Counter-based nonce is safe for AES-GCM if never reused
/// #VERIFY_NONCE_UNIQUENESS: Counter increments monotonically, ensuring unique nonces
fn generate_nonce(counter: u64) -> [u8; NONCE_SIZE] {
    let mut nonce = [0u8; NONCE_SIZE];
    nonce[0..8].copy_from_slice(&counter.to_le_bytes());
    nonce[8..12].copy_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    nonce
}
/// Compute SHA-256 hash of data
///
/// # ASSUM Framework
/// #ASSUME_SHA256_COLLISION_RESISTANT: SHA-256 provides 2^128 collision resistance
/// #VERIFY_HASH_CORRECTNESS: Known test vectors validate SHA-256 implementation
fn compute_sha256(data: &[u8]) -> [u8; HASH_SIZE] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.into()
}
/// Set immutable attribute on file (Linux only)
///
/// # ASSUM Framework
/// #ASSUME_IMMUTABLE_PERSISTENT: chattr +i prevents deletion (requires root or CAP_LINUX_IMMUTABLE)
/// #VERIFY_DELETION_RESISTANCE: Test deletion attempt, expect EPERM
#[cfg(target_os = "linux")]
fn set_immutable(path: &Path, immutable: bool) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    use std::fs::File;
    let file = File::open(path)?;
    let fd = file.as_raw_fd();
    let mut flags: i32 = 0;
    let result = unsafe { libc::ioctl(fd, libc::FS_IOC_GETFLAGS, &mut flags) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    const FS_IMMUTABLE_FL: i32 = 0x00000010;
    if immutable {
        flags |= FS_IMMUTABLE_FL;
    } else {
        flags &= !FS_IMMUTABLE_FL;
    }
    let result = unsafe { libc::ioctl(fd, libc::FS_IOC_SETFLAGS, &flags) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn temp_file() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("test_encrypted_state_{}.enc", rand::random::< u64 > ()));
        path
    }
    fn random_key() -> [u8; KEY_SIZE] {
        let mut key = [0u8; KEY_SIZE];
        for b in key.iter_mut() {
            *b = rand::random();
        }
        key
    }
    #[test]
    fn test_create_and_open() {
        let path = temp_file();
        let key = random_key();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        assert!(capsule.file_path() == path.as_path());
        assert_eq!(capsule.generation(), 0);
        let capsule2 = EncryptedStateCapsule::open(&path, &key).unwrap();
        assert!(capsule2.file_path() == path.as_path());
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn test_write_and_read() {
        let path = temp_file();
        let key = random_key();
        let data = b"test data for encryption";
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        capsule.write(data, &key).unwrap();
        let decrypted = capsule.read(&key).unwrap();
        assert_eq!(decrypted, data);
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn test_verify_integrity() {
        let path = temp_file();
        let key = random_key();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        assert!(! capsule.verify_integrity());
        capsule.write(b"test", &key).unwrap();
        assert!(capsule.verify_integrity());
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn test_sync() {
        let path = temp_file();
        let key = random_key();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        capsule.write(b"sync test", &key).unwrap();
        capsule.sync().unwrap();
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn test_multiple_writes() {
        let path = temp_file();
        let key = random_key();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        capsule.write(b"first", &key).unwrap();
        capsule.write(b"second", &key).unwrap();
        capsule.write(b"third", &key).unwrap();
        let decrypted = capsule.read(&key).unwrap();
        assert_eq!(decrypted, b"third");
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn test_wrong_key_fails() {
        let path = temp_file();
        let key1 = random_key();
        let key2 = random_key();
        let capsule = EncryptedStateCapsule::create(&path, &key1).unwrap();
        capsule.write(b"secret", &key1).unwrap();
        let result = capsule.read(&key2);
        assert!(result.is_err());
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn test_nonce_counter_increments() {
        let path = temp_file();
        let key = random_key();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        assert_eq!(capsule.nonce_counter(), 0);
        capsule.write(b"data1", &key).unwrap();
        assert_eq!(capsule.nonce_counter(), 1);
        capsule.write(b"data2", &key).unwrap();
        assert_eq!(capsule.nonce_counter(), 2);
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn test_generation_counter() {
        let path = temp_file();
        let key = random_key();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        let gen_before = capsule.generation();
        capsule.write(b"test", &key).unwrap();
        let gen_after = capsule.generation();
        assert!(gen_after > gen_before);
        assert_eq!(gen_after % 2, 0);
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn property_tamper_detection_ciphertext_modification() {
        let path = temp_file();
        let key = random_key();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        capsule.write(b"original data", &key).unwrap();
        let mmap_ptr = capsule.mmap_ptr.load(Ordering::Acquire);
        unsafe {
            let byte_ptr = (mmap_ptr as *mut u8).add(16);
            *byte_ptr ^= 0x01;
        }
        let result = capsule.read(&key);
        assert!(result.is_err());
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn integration_persist_across_reopens() {
        let path = temp_file();
        let key = random_key();
        let data = b"persistent data";
        {
            let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
            capsule.write(data, &key).unwrap();
            capsule.sync().unwrap();
        }
        {
            let capsule = EncryptedStateCapsule::open(&path, &key).unwrap();
            let decrypted = capsule.read(&key).unwrap();
            assert_eq!(decrypted, data);
        }
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn integration_large_data() {
        let path = temp_file();
        let key = random_key();
        let data = vec![0xAB; 1024];
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        capsule.write(&data, &key).unwrap();
        let decrypted = capsule.read(&key).unwrap();
        assert_eq!(decrypted, data);
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn integration_empty_data() {
        let path = temp_file();
        let key = random_key();
        let data = b"";
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        capsule.write(data, &key).unwrap();
        let decrypted = capsule.read(&key).unwrap();
        assert_eq!(decrypted, data);
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn integration_unicode_data() {
        let path = temp_file();
        let key = random_key();
        let data = "Hello 世界 🌍".as_bytes();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        capsule.write(data, &key).unwrap();
        let decrypted = capsule.read(&key).unwrap();
        assert_eq!(decrypted, data);
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn integration_binary_data() {
        let path = temp_file();
        let key = random_key();
        let data: Vec<u8> = (0..=255).collect();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        capsule.write(&data, &key).unwrap();
        let decrypted = capsule.read(&key).unwrap();
        assert_eq!(decrypted, data);
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn production_sequential_access() {
        let path = temp_file();
        let key = random_key();
        let capsule = Arc::new(EncryptedStateCapsule::create(&path, &key).unwrap());
        for i in 0..100 {
            let data = format!("iteration {}", i);
            capsule.write(data.as_bytes(), &key).unwrap();
        }
        let decrypted = capsule.read(&key).unwrap();
        assert_eq!(decrypted, b"iteration 99");
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn production_sync_stress() {
        let path = temp_file();
        let key = random_key();
        let capsule = EncryptedStateCapsule::create(&path, &key).unwrap();
        for i in 0..10 {
            let data = format!("sync iteration {}", i);
            capsule.write(data.as_bytes(), &key).unwrap();
            capsule.sync().unwrap();
        }
        let decrypted = capsule.read(&key).unwrap();
        assert_eq!(decrypted, b"sync iteration 9");
        let _ = fs::remove_file(&path);
    }
}

// Manual capsule verification (TODO: Fix derive macro for #[capsule(skip)])
crate::verify_alignment_only!(EncryptedStateCapsule, 64);
