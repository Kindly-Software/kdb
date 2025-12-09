//! InstanceCapsule - Process instance tracking.
//!
//! Tracks current process instance ID and generation for lock coordination.

use std::sync::atomic::{AtomicU32, Ordering};
use std::process;

/// Instance capsule for process tracking
///
/// # T1 Atomic Properties
/// - Process ID (u32)
/// - Generation counter (u32)
/// - Cache-aligned (64B)
#[repr(C, align(64))]
pub struct InstanceCapsule {
    /// Instance ID (process ID)
    instance_id: AtomicU32,

    /// Generation counter (incremented on each lock acquisition)
    generation: AtomicU32,

    /// Padding to 64 bytes
    _padding: [u8; 56],
}

impl InstanceCapsule {
    /// Get current instance (thread-local singleton pattern)
    pub fn current() -> &'static Self {
        // Thread-local instance
        thread_local! {
            static INSTANCE: InstanceCapsule = InstanceCapsule::new();
        }

        // SAFETY: Thread-local storage is safe
        INSTANCE.with(|inst| unsafe {
            // Get reference to thread-local
            std::mem::transmute::<&InstanceCapsule, &'static InstanceCapsule>(inst)
        })
    }

    /// Create new instance capsule
    pub fn new() -> Self {
        let pid = process::id();

        Self {
            instance_id: AtomicU32::new(pid),
            generation: AtomicU32::new(0),
            _padding: [0; 56],
        }
    }

    /// Get instance ID
    pub fn instance_id(&self) -> u32 {
        self.instance_id.load(Ordering::Relaxed)
    }

    /// Bump generation and return new value
    pub fn bump_generation(&self) -> u32 {
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Get current generation
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Relaxed)
    }
}

impl Default for InstanceCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Verification
const _: () = {
    assert!(std::mem::size_of::<InstanceCapsule>() == 64);
    assert!(std::mem::align_of::<InstanceCapsule>() == 64);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_new() {
        let instance = InstanceCapsule::new();
        assert_eq!(instance.instance_id(), process::id());
        assert_eq!(instance.generation(), 0);
    }

    #[test]
    fn test_instance_bump_generation() {
        let instance = InstanceCapsule::new();
        assert_eq!(instance.bump_generation(), 1);
        assert_eq!(instance.bump_generation(), 2);
        assert_eq!(instance.generation(), 2);
    }

    #[test]
    fn test_instance_current() {
        let inst1 = InstanceCapsule::current();
        let inst2 = InstanceCapsule::current();
        assert_eq!(inst1.instance_id(), inst2.instance_id());
    }
}
