//! KGPU Backend Implementations
//!
//! # Supported Backends
//!
//! - **Vulkan** (via ash 0.38) - Linux/Windows/macOS/Android
//! - **DirectX 12** (via windows-rs 0.58) - Windows 10 1903+
//! - **Metal** (via metal-rs 0.32) - macOS/iOS (PRODUCTION READY)
//! - **WebGPU** (future) - Web/cross-platform

#[cfg(feature = "vulkan")]
pub mod vulkan;

#[cfg(all(feature = "dx12", target_os = "windows"))]
pub mod dx12;

#[cfg(all(feature = "metal", any(target_os = "macos", target_os = "ios")))]
pub mod metal;

#[cfg(feature = "vulkan")]
pub use vulkan::VulkanBackend;

#[cfg(all(feature = "dx12", target_os = "windows"))]
pub use dx12::Dx12Backend;

#[cfg(all(feature = "metal", any(target_os = "macos", target_os = "ios")))]
pub use metal::MetalBackend;
