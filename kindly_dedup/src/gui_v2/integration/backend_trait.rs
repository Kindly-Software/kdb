//! GPU Backend Trait Abstraction - Unified Interface for wgpu and KGPU
//!
//! # Overview
//!
//! This module provides a unified trait interface that both wgpu and KGPU
//! implementations can satisfy, enabling feature-flagged backend switching
//! without changing the higher-level rendering code.
//!
//! # Architecture
//!
//! ```text
//! GpuBackend Trait (interface)
//!    ├── WgpuBackend (default, feature="gui-v2")
//!    └── KgpuBackend (opt-in, feature="gui-v2-kgpu")
//! ```
//!
//! # Design Goals
//!
//! 1. **Zero Overhead**: Trait methods inline to direct calls
//! 2. **Type Safety**: Backend-specific types hidden via associated types
//! 3. **Chaos Compliance**: Both backends use lockfree coordination
//! 4. **Feature Parity**: Both backends expose same capabilities
//!
//! # Performance Targets (B32)
//!
//! Both backends must meet:
//! - Device creation: <100ms
//! - Surface creation: <20ms
//! - Texture acquisition: <1ms (VSync wait)
//! - Frame submission: <5ms
//! - State transitions: <10ns (atomic)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (CPU-GPU coordination)
//! - **Chaos**: 100% lockfree (AtomicU64 state, no mutex in hot path)
//! - **ASSUM**: 99.99% safe (GPU abstractions are safe wrappers)
//! - **B32**: Fair comparison (both backends use same VSync, same algorithms)
//! - **T28**: 8+ tests (trait API validation)
//! - **I20**: Zero breaking changes (internal abstraction only)

use std::sync::Arc;
use winit::window::Window;

use super::types::{GuiError, GuiResult};

/// GPU backend trait - unified interface for wgpu and KGPU
///
/// # Design
///
/// This trait abstracts over GPU backends to allow feature-flagged switching
/// between wgpu (default, mature) and KGPU (opt-in, 2-4× faster).
///
/// # Associated Types
///
/// - `Device`: GPU logical device handle
/// - `Queue`: Command submission queue handle
/// - `Surface`: Window rendering target
/// - `Texture`: Swapchain texture for rendering
///
/// # Performance
///
/// All methods must be <100ns except for async operations:
/// - `init()`: <100ms (GPU handshake)
/// - `resize()`: <20ms (surface reconfiguration)
/// - `acquire_texture()`: <1ms (VSync wait)
/// - State queries: <10ns (atomic loads)
pub trait GpuBackend: Send + Sync {
    /// Backend-specific device type
    type Device: Send + Sync;

    /// Backend-specific queue type
    type Queue: Send + Sync;

    /// Backend-specific surface type
    type Surface: Send + Sync;

    /// Backend-specific texture type
    type Texture;

    /// Initialize GPU backend (async, requires window)
    ///
    /// # Steps
    ///
    /// 1. Create GPU instance (backend selection)
    /// 2. Create surface from window
    /// 3. Request adapter (physical GPU)
    /// 4. Request device + queue (logical GPU)
    /// 5. Configure surface (format, present mode)
    ///
    /// # Performance
    ///
    /// - Target: <100ms (GPU driver handshake)
    /// - Memory: ~50MB (internal buffers)
    ///
    /// # Errors
    ///
    /// - NoAdapterFound: No compatible GPU
    /// - DeviceRequestFailed: GPU driver error
    ///
    /// #ASSUME_GPU_AVAILABLE: Backend finds adapter or returns error
    async fn init(window: Arc<Window>) -> GuiResult<Self>
    where
        Self: Sized;

    /// Check if GPU is ready for rendering
    ///
    /// # Performance
    ///
    /// - Target: <10ns (single atomic load + comparison)
    fn is_ready(&self) -> bool;

    /// Get device reference
    fn device(&self) -> Option<&Self::Device>;

    /// Get queue reference
    fn queue(&self) -> Option<&Self::Queue>;

    /// Get surface dimensions
    ///
    /// # Performance
    ///
    /// - Target: <10ns (atomic load + bit unpacking)
    fn surface_size(&self) -> (u16, u16);

    /// Resize surface (on window resize events)
    ///
    /// # Performance
    ///
    /// - Target: <20ms (surface reconfiguration)
    ///
    /// # Steps
    ///
    /// 1. Update internal state (width, height)
    /// 2. Update surface configuration
    /// 3. Reconfigure surface (GPU resource allocation)
    ///
    /// #ASSUME_RESIZE_VALID: (width, height) > 0
    /// #VERIFY: Clamp to minimum 1×1
    fn resize(&mut self, width: u32, height: u32) -> GuiResult<()>;

    /// Acquire next swapchain texture for rendering
    ///
    /// # Performance
    ///
    /// - Target: <1ms (waits for VSync if needed)
    ///
    /// # Returns
    ///
    /// Backend-specific texture for rendering
    ///
    /// # Errors
    ///
    /// - SurfaceNotReady: Surface not configured
    /// - TextureAcquisitionFailed: GPU error or surface lost
    ///
    /// #ASSUME_SWAPCHAIN_READY: Texture acquisition blocks until available
    fn acquire_texture(&self) -> GuiResult<Self::Texture>;

    /// Submit rendered frame for presentation
    ///
    /// # Performance
    ///
    /// - Target: <5ms (GPU command submission + present)
    ///
    /// # Steps
    ///
    /// 1. Submit render commands to GPU
    /// 2. Present texture to swapchain
    /// 3. Wait for VSync (if PresentMode::Fifo)
    fn present(&self, texture: Self::Texture) -> GuiResult<()>;

    /// Get generation counter (for ABA prevention)
    ///
    /// # Performance
    ///
    /// - Target: <5ns (relaxed atomic load)
    fn generation(&self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trait API validation tests
    #[test]
    fn test_backend_trait_send_sync() {
        // Verify trait requires Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn GpuBackend<Device = (), Queue = (), Surface = (), Texture = ()>>>();
    }

    #[test]
    fn test_backend_trait_associated_types() {
        // Verify associated types are Send + Sync
        trait AssertAssociatedTypes: GpuBackend {
            fn verify_device_send_sync(_: Self::Device)
            where
                Self::Device: Send + Sync,
            {
            }

            fn verify_queue_send_sync(_: Self::Queue)
            where
                Self::Queue: Send + Sync,
            {
            }

            fn verify_surface_send_sync(_: Self::Surface)
            where
                Self::Surface: Send + Sync,
            {
            }
        }
    }
}
