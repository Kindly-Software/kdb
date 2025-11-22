//! # DaemonLockCapsule (T1 Atomic) - Lockfree Daemon Coordination
//!
//! **UCE34 Tier 1 Atomic Capsule for inter-process daemon synchronization.**

use crate::alignment::AlignmentTier;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

use super::error::{DaemonError, DaemonResult};

fn timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time is before UNIX_EPOCH")
        .as_nanos() as u64
}

/// DaemonLockCapsule - T1 Atomic lockfree daemon coordination
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64, size = 64))]
#[repr(C, align(64))]
pub struct DaemonLockCapsule {
    /// Lock state: (holder_pid: u32 | generation: u32)
    state: AtomicU64,
    /// Last heartbeat timestamp
    heartbeat: AtomicU64,
    /// Staleness timeout in nanoseconds
    timeout_ns: u64,
    /// Statistics: Number of successful acquires
    acquires: AtomicU64,
    /// Statistics: Number of contention attempts
    contentions: AtomicU64,
    /// Statistics: Number of stale lock recoveries
    stale_recoveries: AtomicU64,
    /// Padding to complete 64-byte cache line
    _padding: [u8; 16],
}

impl AlignmentTier for DaemonLockCapsule {
    const TIER: &'static str = "hot";
    const ALIGNMENT: usize = 64;
}

#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(DaemonLockCapsule, 64, 64);

impl DaemonLockCapsule {
    pub const fn new(timeout_ns: u64) -> Self {
        Self {
            state: AtomicU64::new(0),
            heartbeat: AtomicU64::new(0),
            timeout_ns,
            acquires: AtomicU64::new(0),
            contentions: AtomicU64::new(0),
            stale_recoveries: AtomicU64::new(0),
            _padding: [0; 16],
        }
    }

    pub fn try_acquire(&self) -> DaemonResult<LockGuard> {
        let current_pid = std::process::id() as u64;
        let now = timestamp_ns();

        loop {
            let state = self.state.load(Ordering::Acquire);
            let holder = (state & 0xFFFFFFFF) as u32;
            let gen = (state >> 32) as u32;

            if holder == 0 {
                let new_state = ((gen as u64 + 1) << 32) | (current_pid & 0xFFFFFFFF);

                match self.state.compare_exchange(
                    state,
                    new_state,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        self.heartbeat.store(now, Ordering::Release);
                        self.acquires.fetch_add(1, Ordering::Relaxed);
                        return Ok(LockGuard { capsule: self });
                    }
                    Err(_) => {
                        self.contentions.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                }
            } else {
                let last_beat = self.heartbeat.load(Ordering::Acquire);

                if now.saturating_sub(last_beat) > self.timeout_ns {
                    let new_state = ((gen as u64 + 1) << 32) | (current_pid & 0xFFFFFFFF);

                    match self.state.compare_exchange(
                        state,
                        new_state,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            self.heartbeat.store(now, Ordering::Release);
                            self.stale_recoveries.fetch_add(1, Ordering::Relaxed);
                            self.acquires.fetch_add(1, Ordering::Relaxed);
                            return Ok(LockGuard { capsule: self });
                        }
                        Err(_) => continue,
                    }
                } else {
                    return Err(DaemonError::LockHeld {
                        holder_pid: holder,
                    });
                }
            }
        }
    }

    #[inline]
    pub fn is_locked(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);
        (state & 0xFFFFFFFF) != 0
    }

    #[inline]
    pub fn holder(&self) -> Option<u32> {
        let state = self.state.load(Ordering::Relaxed);
        let holder = (state & 0xFFFFFFFF) as u32;
        if holder != 0 {
            Some(holder)
        } else {
            None
        }
    }

    pub fn stats(&self) -> (u64, u64, u64) {
        (
            self.acquires.load(Ordering::Relaxed),
            self.contentions.load(Ordering::Relaxed),
            self.stale_recoveries.load(Ordering::Relaxed),
        )
    }

    #[inline]
    pub fn timeout_ns(&self) -> u64 {
        self.timeout_ns
    }
}

pub struct LockGuard<'a> {
    capsule: &'a DaemonLockCapsule,
}

impl<'a> Drop for LockGuard<'a> {
    fn drop(&mut self) {
        let current_pid = std::process::id() as u64;
        let state = self.capsule.state.load(Ordering::Acquire);
        let holder = (state & 0xFFFFFFFF) as u32;
        let gen = (state >> 32) as u32;

        if holder as u64 == (current_pid & 0xFFFFFFFF) {
            let new_state = ((gen as u64 + 1) << 32);
            self.capsule.state.store(new_state, Ordering::Release);
        }
    }
}

#[cfg(not(feature = "derive"))]
unsafe impl Send for DaemonLockCapsule {}
#[cfg(not(feature = "derive"))]
unsafe impl Sync for DaemonLockCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_and_alignment() {
        use core::mem::{align_of, size_of};
        assert_eq!(align_of::<DaemonLockCapsule>(), 64);
        assert_eq!(size_of::<DaemonLockCapsule>(), 64);
    }

    #[test]
    fn test_acquire_release() {
        let lock = DaemonLockCapsule::new(30_000_000_000);
        assert!(!lock.is_locked());
        {
            let _guard = lock.try_acquire().unwrap();
            assert!(lock.is_locked());
            assert_eq!(lock.holder(), Some(std::process::id()));
        }
        assert!(!lock.is_locked());
    }

    #[test]
    fn test_double_acquire_fails() {
        let lock = DaemonLockCapsule::new(30_000_000_000);
        let _guard1 = lock.try_acquire().unwrap();
        {
            let result = lock.try_acquire();
            match result {
                Err(DaemonError::LockHeld { holder_pid }) => {
                    assert_eq!(holder_pid, std::process::id());
                }
                _ => panic!("Expected LockHeld error"),
            }
        }
    }

    #[test]
    fn test_holder_none_when_free() {
        let lock = DaemonLockCapsule::new(30_000_000_000);
        assert_eq!(lock.holder(), None);
    }

    #[test]
    fn test_holder_pid_when_held() {
        let lock = DaemonLockCapsule::new(30_000_000_000);
        let _guard = lock.try_acquire().unwrap();
        assert_eq!(lock.holder(), Some(std::process::id()));
    }

    #[test]
    fn test_sequential_acquires() {
        let lock = DaemonLockCapsule::new(30_000_000_000);
        for _ in 0..10 {
            {
                let _guard = lock.try_acquire().unwrap();
                assert!(lock.is_locked());
            }
            assert!(!lock.is_locked());
        }
        let (acq, _, _) = lock.stats();
        assert_eq!(acq, 10);
    }

    #[test]
    fn test_timeout_accessor() {
        let timeout = 60_000_000_000u64;
        let lock = DaemonLockCapsule::new(timeout);
        assert_eq!(lock.timeout_ns(), timeout);
    }
}
