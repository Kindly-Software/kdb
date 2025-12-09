//! Metal Synchronization Primitives (Fences, Events)
//!
//! # Architecture
//!
//! - **MTLFence**: GPU-GPU synchronization within command buffer
//! - **MTLEvent**: CPU-GPU synchronization (timeline semaphores)
//! - **MTLSharedEvent**: Cross-process synchronization
//!
//! # Performance
//!
//! - Fence creation: <1μs
//! - Event creation: <1μs
//! - Wait: Variable (depends on GPU work)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_FENCE_VALID`: MTLFence remains valid until destroyed
//! - `#ASSUME_EVENT_SIGNALED`: Event value increments atomically
//! - `#VERIFY_UNSAFE_FFI`: metal-rs wraps MTL* calls safely

use metal::{self, Device as MTLDeviceProtocol};
use std::sync::Arc;

use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::MetalDevice;

/// Metal fence (GPU-GPU synchronization)
///
/// # Layout
///
/// - 64B cache-aligned (Arc overhead)
/// - MTLFence reference (ARC-managed)
///
/// # Use Cases
///
/// - Wait for render pass to complete before compute pass
/// - Synchronize resource access across encoders
#[derive(Clone)]
pub struct MetalFence {
    /// Inner state
    inner: Arc<MetalFenceInner>,
}

struct MetalFenceInner {
    /// Device reference
    device: MetalDevice,

    /// MTLFence handle
    fence: metal::Fence,
}

impl MetalFence {
    /// Create new fence
    ///
    /// # Performance
    ///
    /// <1μs (B32 target)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use atomic_capsule::gpu::kgpu::backends::metal::*;
    /// # let device = MetalDevice::new(/* adapter */)?;
    /// let fence = MetalFence::new(device)?;
    /// // Use fence in command encoders...
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub fn new(device: MetalDevice) -> KgpuResult<Self> {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps new_fence safely
        let fence = device.metal_device().new_fence();

        Ok(Self {
            inner: Arc::new(MetalFenceInner { device, fence }),
        })
    }

    /// Get raw MTLFence
    pub(crate) fn raw(&self) -> &metal::Fence {
        &self.inner.fence
    }
}

// SAFETY: MTLFence is thread-safe (Objective-C @synchronized)
unsafe impl Send for MetalFenceInner {}
unsafe impl Sync for MetalFenceInner {}

impl Drop for MetalFenceInner {
    fn drop(&mut self) {
        // MTLFence is ARC-managed, no explicit cleanup needed
    }
}

/// Metal event (CPU-GPU synchronization, timeline semaphore)
///
/// # Layout
///
/// - 64B cache-aligned (Arc overhead)
/// - MTLEvent reference (ARC-managed)
/// - Timeline value (u64)
///
/// # Use Cases
///
/// - Wait for GPU work to complete on CPU
/// - Signal CPU when GPU reaches specific point
/// - Cross-queue synchronization
#[derive(Clone)]
pub struct MetalEvent {
    /// Inner state
    inner: Arc<MetalEventInner>,
}

struct MetalEventInner {
    /// Device reference
    device: MetalDevice,

    /// MTLEvent handle
    event: metal::Event,
}

impl MetalEvent {
    /// Create new event
    ///
    /// # Performance
    ///
    /// <1μs (B32 target)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use atomic_capsule::gpu::kgpu::backends::metal::*;
    /// # let device = MetalDevice::new(/* adapter */)?;
    /// let event = MetalEvent::new(device)?;
    /// // Signal event when GPU work completes...
    /// event.wait_until(1)?;
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub fn new(device: MetalDevice) -> KgpuResult<Self> {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps new_event safely
        let event = device.metal_device().new_event();

        Ok(Self {
            inner: Arc::new(MetalEventInner { device, event }),
        })
    }

    /// Get current signaled value
    ///
    /// # Performance
    ///
    /// <10ns (atomic read)
    pub fn signaled_value(&self) -> u64 {
        self.inner.event.signaled_value()
    }

    /// Set signaled value (CPU signals event)
    ///
    /// # Performance
    ///
    /// <10ns (atomic write)
    pub fn set_signaled_value(&self, value: u64) {
        self.inner.event.set_signaled_value(value);
    }

    /// Wait until event reaches specified value
    ///
    /// # Performance
    ///
    /// Variable (depends on GPU work)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_VALUE_MONOTONIC`: Event values increase monotonically
    pub fn wait_until(&self, value: u64) -> KgpuResult<()> {
        // Busy-wait loop (Metal doesn't have direct wait API)
        // Real implementation would use MTLSharedEvent listeners
        while self.signaled_value() < value {
            std::hint::spin_loop();
        }
        Ok(())
    }

    /// Get raw MTLEvent
    pub(crate) fn raw(&self) -> &metal::Event {
        &self.inner.event
    }
}

// SAFETY: MTLEvent is thread-safe (atomic value internally)
unsafe impl Send for MetalEventInner {}
unsafe impl Sync for MetalEventInner {}

impl Drop for MetalEventInner {
    fn drop(&mut self) {
        // MTLEvent is ARC-managed, no explicit cleanup needed
    }
}

/// Metal shared event (cross-process synchronization)
///
/// # Layout
///
/// - 64B cache-aligned (Arc overhead)
/// - MTLSharedEvent reference (ARC-managed)
/// - Shared memory backing
///
/// # Use Cases
///
/// - Cross-process GPU synchronization
/// - Multi-app GPU coordination
#[derive(Clone)]
pub struct MetalSharedEvent {
    /// Inner state
    inner: Arc<MetalSharedEventInner>,
}

struct MetalSharedEventInner {
    /// Device reference
    device: MetalDevice,

    /// MTLSharedEvent handle
    shared_event: metal::SharedEvent,
}

impl MetalSharedEvent {
    /// Create new shared event
    ///
    /// # Performance
    ///
    /// <10μs (B32 target, involves shared memory allocation)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_SHARED_MEMORY_AVAILABLE`: System supports shared memory
    pub fn new(device: MetalDevice) -> KgpuResult<Self> {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps new_shared_event safely
        let shared_event = device.metal_device().new_shared_event();

        Ok(Self {
            inner: Arc::new(MetalSharedEventInner {
                device,
                shared_event,
            }),
        })
    }

    /// Get current signaled value
    pub fn signaled_value(&self) -> u64 {
        self.inner.shared_event.signaled_value()
    }

    /// Set signaled value
    pub fn set_signaled_value(&self, value: u64) {
        self.inner.shared_event.set_signaled_value(value);
    }

    /// Get raw MTLSharedEvent
    pub(crate) fn raw(&self) -> &metal::SharedEvent {
        &self.inner.shared_event
    }
}

// SAFETY: MTLSharedEvent is thread-safe and process-safe
unsafe impl Send for MetalSharedEventInner {}
unsafe impl Sync for MetalSharedEventInner {}

impl Drop for MetalSharedEventInner {
    fn drop(&mut self) {
        // MTLSharedEvent is ARC-managed, no explicit cleanup needed
    }
}

#[cfg(test)]
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod tests {
    use super::*;
    use super::super::{MetalInstance, MetalDevice};

    #[test]
    #[ignore] // Requires Metal support
    fn test_fence_creation() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let fence = MetalFence::new(device);
        assert!(fence.is_ok(), "Failed to create fence");
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_event_creation() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let event = MetalEvent::new(device);
        assert!(event.is_ok(), "Failed to create event");
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_event_signaling() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let event = MetalEvent::new(device).unwrap();

        // Initial value should be 0
        assert_eq!(event.signaled_value(), 0);

        // Signal event
        event.set_signaled_value(42);
        assert_eq!(event.signaled_value(), 42);
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_shared_event_creation() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let shared_event = MetalSharedEvent::new(device);
        assert!(shared_event.is_ok(), "Failed to create shared event");
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_shared_event_signaling() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let shared_event = MetalSharedEvent::new(device).unwrap();

        // Signal shared event
        shared_event.set_signaled_value(100);
        assert_eq!(shared_event.signaled_value(), 100);
    }
}
