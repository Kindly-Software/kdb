//! Encoder Wiring Capsule - T6 Metacapsule Orchestration
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This module provides the wiring logic for connecting encoder sub-capsules
//! in the kindly-av1 metacapsule architecture.

use atomic_capsule::patterns::DualAtomicU64;
use core::sync::atomic::{AtomicU64, Ordering};

/// Encoder wiring capsule for T6 metacapsule orchestration.
///
/// This capsule coordinates communication between encoder sub-capsules
/// using lockfree DualAtomicU64 patterns.
#[repr(C, align(128))]
pub struct EncoderWiringCapsule {
    /// State coordination (DualAtomicU64 pattern)
    state: DualAtomicU64,

    /// Frame counter
    frame_count: AtomicU64,

    /// Tile coordination
    tile_state: DualAtomicU64,

    /// Padding to 128 bytes
    _padding: [u8; 128 - 64],
}

impl EncoderWiringCapsule {
    /// Create a new encoder wiring capsule.
    pub const fn new() -> Self {
        Self {
            state: DualAtomicU64::new(0, 0),
            frame_count: AtomicU64::new(0),
            tile_state: DualAtomicU64::new(0, 0),
            _padding: [0u8; 128 - 64],
        }
    }

    /// Get current frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count.load(Ordering::Acquire)
    }

    /// Increment frame count (lockfree).
    pub fn increment_frame(&self) -> u64 {
        self.frame_count.fetch_add(1, Ordering::AcqRel)
    }
}

impl Default for EncoderWiringCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wiring_capsule_size() {
        assert_eq!(core::mem::size_of::<EncoderWiringCapsule>(), 128);
        assert_eq!(core::mem::align_of::<EncoderWiringCapsule>(), 128);
    }

    #[test]
    fn test_frame_counter() {
        let wiring = EncoderWiringCapsule::new();
        assert_eq!(wiring.frame_count(), 0);
        assert_eq!(wiring.increment_frame(), 0);
        assert_eq!(wiring.frame_count(), 1);
    }
}
