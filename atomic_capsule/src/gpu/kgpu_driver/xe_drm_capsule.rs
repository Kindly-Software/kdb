// Intel Xe2 DRM Device Management Capsule
// T1 Atomic Tier: 256B cache-aligned, 100% lockfree
// Phase 1: DRM device lifecycle management (open/close/ioctl coordination)

use core::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};

// State encoding
const STATE_CLOSED: u32 = 0;
const STATE_OPENING: u32 = 1;
const STATE_OPEN: u32 = 2;
const STATE_ERROR: u32 = 3;

/// Intel Xe DRM Device Management Capsule (T1 Atomic, 256B cache-aligned)
///
/// Manages the lifecycle of Intel xe DRM device file descriptors.
/// Coordinates open/close/ioctl operations with lockfree atomic state transitions.
///
/// # Safety
/// - All DRM operations are Linux-specific and require kernel driver support
/// - File descriptor lifecycle must prevent use-after-close via atomic coordination
/// - Generation counter prevents ABA problems in multi-threaded scenarios
#[repr(C, align(256))]
pub struct XeDrmCapsule {
    // File descriptor management
    fd: AtomicI32, // DRM file descriptor (-1 if not open)

    // Device path (64 bytes for paths like "/dev/dri/card0")
    device_path: [u8; 64],

    // State coordination (lockfree atomic state machine)
    state: AtomicU32,      // CLOSED -> OPENING -> OPEN -> ERROR
    generation: AtomicU64, // ABA prevention counter

    // GPU capabilities (discovered during open)
    capabilities: AtomicU64,  // Bitmask of supported DRM features
    vm_id: AtomicU32,         // GPU virtual memory context ID
    exec_queue_id: AtomicU32, // Execution queue ID

    // Statistics (lockfree counters)
    open_count: AtomicU64,
    close_count: AtomicU64,
    ioctl_count: AtomicU64,

    // Padding to exactly 256 bytes
    // Current size without padding:
    //   fd: 4 bytes
    //   device_path: 64 bytes
    //   state: 4 bytes
    //   generation: 8 bytes (aligned to 8)
    //   capabilities: 8 bytes
    //   vm_id: 4 bytes
    //   exec_queue_id: 4 bytes
    //   open_count: 8 bytes (aligned to 8)
    //   close_count: 8 bytes
    //   ioctl_count: 8 bytes
    // Total: 4 + 64 + 4 + 8 + 8 + 4 + 4 + 8 + 8 + 8 = 120 bytes
    //
    // With repr(C) implicit padding:
    //   - 4 bytes after state (to align generation to 8)
    //   - 4 bytes after exec_queue_id (to align open_count to 8)
    // Total with implicit padding: 120 + 4 + 4 = 128 bytes
    //
    // Explicit padding needed: 256 - 128 = 128 bytes
    _padding: [u8; 128],
}

/// Intel Xe DRM specific errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XeDrmError {
    /// Device is already open
    AlreadyOpen,
    /// Device is not open
    NotOpen,
    /// Failed to open device
    OpenFailed,
    /// Failed to close device
    CloseFailed,
    /// ioctl operation failed
    IoctlFailed,
}

impl XeDrmCapsule {
    /// Create new closed DRM capsule
    #[inline]
    pub fn new() -> Self {
        // #ASSUME: Cache-aligned allocation by caller
        // #VERIFY: #[repr(C, align(256))] enforces alignment
        Self {
            fd: AtomicI32::new(-1), // -1 = not open
            device_path: [0u8; 64],
            state: AtomicU32::new(STATE_CLOSED),
            generation: AtomicU64::new(0),
            capabilities: AtomicU64::new(0),
            vm_id: AtomicU32::new(0),
            exec_queue_id: AtomicU32::new(0),
            open_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
            ioctl_count: AtomicU64::new(0),
            _padding: [0u8; 128],
        }
    }

    /// Open Intel Xe DRM device
    ///
    /// # Arguments
    /// * `path` - Device path (e.g., "/dev/dri/card0")
    ///
    /// # Safety
    /// Linux-specific operation. Requires xe kernel driver loaded.
    #[cfg(target_os = "linux")]
    pub fn open(&mut self, path: &str) -> Result<(), XeDrmError> {
        use std::ffi::CString;
        use std::os::unix::io::RawFd;

        // Atomic state transition: CLOSED -> OPENING
        let old_state = self.state.load(Ordering::Acquire);
        if old_state != STATE_CLOSED {
            return Err(XeDrmError::AlreadyOpen);
        }

        // Transition to OPENING state and increment generation
        self.state.store(STATE_OPENING, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        // Copy device path (truncate if too long)
        let path_bytes = path.as_bytes();
        let copy_len = core::cmp::min(path_bytes.len(), 63); // Leave room for null terminator
        self.device_path[..copy_len].copy_from_slice(&path_bytes[..copy_len]);
        self.device_path[copy_len] = 0; // Null terminator

        // Open device file
        let c_path = match CString::new(path) {
            Ok(p) => p,
            Err(_) => {
                self.state.store(STATE_ERROR, Ordering::Release);
                return Err(XeDrmError::OpenFailed);
            }
        };

        // SAFETY: open() syscall with valid C string
        // O_RDWR = 0x0002, O_CLOEXEC = 0x80000
        let raw_fd: RawFd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };

        if raw_fd < 0 {
            // Open failed
            self.state.store(STATE_ERROR, Ordering::Release);
            return Err(XeDrmError::OpenFailed);
        }

        // Store file descriptor
        self.fd.store(raw_fd, Ordering::Release);

        // TODO: Query device capabilities via DRM ioctls
        // For Phase 1, set placeholder capabilities
        self.capabilities.store(0xFFFF, Ordering::Relaxed);
        self.vm_id.store(0, Ordering::Relaxed);
        self.exec_queue_id.store(0, Ordering::Relaxed);

        // Increment open counter
        self.open_count.fetch_add(1, Ordering::Relaxed);

        // Atomic state transition: OPENING -> OPEN
        self.state.store(STATE_OPEN, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Open Intel Xe DRM device (no-op on non-Linux)
    #[cfg(not(target_os = "linux"))]
    pub fn open(&mut self, _path: &str) -> Result<(), XeDrmError> {
        Err(XeDrmError::OpenFailed)
    }

    /// Close Intel Xe DRM device
    #[cfg(target_os = "linux")]
    pub fn close(&mut self) -> Result<(), XeDrmError> {
        // Atomic state check: must be OPEN
        let old_state = self.state.load(Ordering::Acquire);
        if old_state != STATE_OPEN {
            return Err(XeDrmError::NotOpen);
        }

        // Load file descriptor
        let raw_fd = self.fd.load(Ordering::Acquire);
        if raw_fd < 0 {
            return Err(XeDrmError::NotOpen);
        }

        // SAFETY: close() syscall with valid file descriptor
        let result = unsafe { libc::close(raw_fd) };

        if result < 0 {
            self.state.store(STATE_ERROR, Ordering::Release);
            return Err(XeDrmError::CloseFailed);
        }

        // Mark as closed
        self.fd.store(-1, Ordering::Release);
        self.close_count.fetch_add(1, Ordering::Relaxed);

        // Atomic state transition: OPEN -> CLOSED
        self.state.store(STATE_CLOSED, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Close Intel Xe DRM device (no-op on non-Linux)
    #[cfg(not(target_os = "linux"))]
    pub fn close(&mut self) -> Result<(), XeDrmError> {
        Err(XeDrmError::NotOpen)
    }

    /// Check if device is open
    #[inline]
    pub fn is_open(&self) -> bool {
        self.state.load(Ordering::Acquire) == STATE_OPEN
    }

    /// Get file descriptor
    ///
    /// Returns -1 if device is not open
    #[inline]
    pub fn fd(&self) -> i32 {
        self.fd.load(Ordering::Acquire)
    }

    /// Get ioctl count
    #[inline]
    pub fn ioctl_count(&self) -> u64 {
        self.ioctl_count.load(Ordering::Relaxed)
    }

    /// Get current state
    #[inline]
    fn state(&self) -> u32 {
        self.state.load(Ordering::Acquire)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Get capabilities bitmask
    #[inline]
    pub fn capabilities(&self) -> u64 {
        self.capabilities.load(Ordering::Relaxed)
    }

    /// Get VM context ID
    #[inline]
    pub fn vm_id(&self) -> u32 {
        self.vm_id.load(Ordering::Relaxed)
    }

    /// Get execution queue ID
    #[inline]
    pub fn exec_queue_id(&self) -> u32 {
        self.exec_queue_id.load(Ordering::Relaxed)
    }

    /// Get open count
    #[inline]
    pub fn open_count(&self) -> u64 {
        self.open_count.load(Ordering::Relaxed)
    }

    /// Get close count
    #[inline]
    pub fn close_count(&self) -> u64 {
        self.close_count.load(Ordering::Relaxed)
    }
}

impl Default for XeDrmCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        // T28 Q1: Verify 256B cache alignment
        assert_eq!(
            core::mem::size_of::<XeDrmCapsule>(),
            256,
            "XeDrmCapsule must be exactly 256 bytes"
        );
        assert_eq!(
            core::mem::align_of::<XeDrmCapsule>(),
            256,
            "XeDrmCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_new_capsule() {
        // T28 Q2: Verify initial state
        let capsule = XeDrmCapsule::new();
        assert_eq!(capsule.state(), STATE_CLOSED);
        assert!(!capsule.is_open());
        assert_eq!(capsule.fd(), -1);
        assert_eq!(capsule.open_count(), 0);
        assert_eq!(capsule.close_count(), 0);
        assert_eq!(capsule.ioctl_count(), 0);
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn test_default() {
        // T28 Q3: Verify Default trait
        let capsule = XeDrmCapsule::default();
        assert_eq!(capsule.state(), STATE_CLOSED);
        assert_eq!(capsule.fd(), -1);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_open_nonexistent_device() {
        // T28 Q4: Verify open fails for nonexistent device
        let mut capsule = XeDrmCapsule::new();
        let result = capsule.open("/dev/nonexistent_device_12345");
        assert!(matches!(result, Err(XeDrmError::OpenFailed)));
        assert_eq!(capsule.state(), STATE_ERROR);
        assert!(!capsule.is_open());
        assert_eq!(capsule.fd(), -1);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_close_without_open() {
        // T28 Q5: Verify close fails when not open
        let mut capsule = XeDrmCapsule::new();
        let result = capsule.close();
        assert!(matches!(result, Err(XeDrmError::NotOpen)));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_double_open_fails() {
        // T28 Q6: Verify double open fails
        let mut capsule = XeDrmCapsule::new();

        // Try to open /dev/null as a test (always exists)
        let result1 = capsule.open("/dev/null");

        // If first open succeeded, try second open
        if result1.is_ok() {
            let result2 = capsule.open("/dev/null");
            assert!(matches!(result2, Err(XeDrmError::AlreadyOpen)));

            // Clean up
            let _ = capsule.close();
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_open_close_lifecycle() {
        // T28 Q7: Verify full open/close lifecycle
        let mut capsule = XeDrmCapsule::new();

        // Open /dev/null (always exists on Linux)
        let open_result = capsule.open("/dev/null");

        if open_result.is_ok() {
            // Verify open state
            assert!(capsule.is_open());
            assert_eq!(capsule.state(), STATE_OPEN);
            assert!(capsule.fd() >= 0);
            assert_eq!(capsule.open_count(), 1);

            // Close device
            let close_result = capsule.close();
            assert!(close_result.is_ok());

            // Verify closed state
            assert!(!capsule.is_open());
            assert_eq!(capsule.state(), STATE_CLOSED);
            assert_eq!(capsule.fd(), -1);
            assert_eq!(capsule.close_count(), 1);
        }
    }

    #[test]
    fn test_generation_increments() {
        // T28 Q8: Verify generation counter increments
        #[cfg(target_os = "linux")]
        {
            let mut capsule = XeDrmCapsule::new();
            let gen0 = capsule.generation();

            // Try to open /dev/null
            let result = capsule.open("/dev/null");

            if result.is_ok() {
                // Generation should increment twice (CLOSED->OPENING->OPEN)
                let gen1 = capsule.generation();
                assert_eq!(gen1, gen0 + 2);

                // Close should increment again (OPEN->CLOSED)
                let _ = capsule.close();
                let gen2 = capsule.generation();
                assert_eq!(gen2, gen1 + 1);
            }
        }
    }

    #[test]
    fn test_accessors() {
        // T28 Q9: Verify accessor methods
        let capsule = XeDrmCapsule::new();

        // All accessors should work without panicking
        let _ = capsule.is_open();
        let _ = capsule.fd();
        let _ = capsule.ioctl_count();
        let _ = capsule.generation();
        let _ = capsule.capabilities();
        let _ = capsule.vm_id();
        let _ = capsule.exec_queue_id();
        let _ = capsule.open_count();
        let _ = capsule.close_count();
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn test_non_linux_fails() {
        // T28 Q10: Verify operations fail on non-Linux platforms
        let mut capsule = XeDrmCapsule::new();

        let open_result = capsule.open("/dev/null");
        assert!(matches!(open_result, Err(XeDrmError::OpenFailed)));

        let close_result = capsule.close();
        assert!(matches!(close_result, Err(XeDrmError::NotOpen)));
    }
}
