//! Metal Instance (Entry Point) Implementation
//!
//! # Architecture
//!
//! MetalInstance provides entry point to Metal GPU backend.
//!
//! - **Instance**: Singleton for device discovery
//! - **Adapter Enumeration**: Discovers all Metal-capable GPUs
//! - **Validation**: No validation layers (Metal has automatic validation in debug builds)
//!
//! # Performance
//!
//! - Creation: <1ms (B32 target)
//! - Enumeration: <10ms (query all GPUs)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_METAL_AVAILABLE`: macOS 10.15+ or iOS 13+
//! - `#ASSUME_AT_LEAST_ONE_GPU`: Metal guarantees at least one device
//! - `#VERIFY_UNSAFE_FFI`: metal-rs wraps MTLCopyAllDevices safely

use metal::{self, Device as MTLDeviceProtocol};
use std::sync::Arc;

use crate::gpu::kgpu::error::{KgpuError, KgpuResult};
use crate::gpu::kgpu::hal::{AdapterInfo, AdapterList, DeviceType, BackendType};

use super::MetalAdapter;

/// Metal instance (entry point)
///
/// # Layout
///
/// - 64B cache-aligned (Arc overhead)
/// - Singleton pattern (can create multiple instances, all refer to same Metal runtime)
pub struct MetalInstance {
    /// Inner state (Arc for cheap cloning)
    inner: Arc<MetalInstanceInner>,
}

struct MetalInstanceInner {
    /// Metal devices (cached at creation)
    devices: Vec<metal::Device>,
}

impl MetalInstance {
    /// Create new Metal instance
    ///
    /// # Performance
    ///
    /// <1ms (B32 target, much faster than Vulkan)
    ///
    /// # Errors
    ///
    /// Returns error if Metal is not available (non-Apple platform)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::metal::MetalInstance;
    ///
    /// let instance = MetalInstance::new()?;
    /// let adapters = instance.enumerate_adapters()?;
    /// println!("Found {} Metal devices", adapters.count);
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub fn new() -> KgpuResult<Self> {
        // Check Metal availability
        if !super::MetalBackend::is_available() {
            return Err(KgpuError::InitializationFailed(
                "Metal is not available on this platform".into(),
            ));
        }

        // Enumerate all Metal devices
        // #VERIFY_UNSAFE_FFI: metal-rs wraps MTLCopyAllDevices safely
        let devices = metal::Device::all();

        if devices.is_empty() {
            return Err(KgpuError::InitializationFailed(
                "No Metal devices found".into(),
            ));
        }

        Ok(Self {
            inner: Arc::new(MetalInstanceInner { devices }),
        })
    }

    /// Enumerate all Metal adapters
    ///
    /// # Performance
    ///
    /// <10ms (B32 target, queries device properties)
    ///
    /// # Returns
    ///
    /// AdapterList with up to 8 adapters
    pub fn enumerate_adapters(&self) -> KgpuResult<AdapterList> {
        let mut adapter_list = AdapterList::default();

        for (i, device) in self.inner.devices.iter().take(8).enumerate() {
            let name = device.name().to_string();

            // Determine device type
            let device_type = if device.is_low_power() {
                DeviceType::IntegratedGpu
            } else {
                DeviceType::DiscreteGpu
            };

            let info = AdapterInfo::new(&name, device_type, BackendType::Metal);

            if !adapter_list.push(info) {
                break; // Max 8 adapters
            }
        }

        Ok(adapter_list)
    }

    /// Request specific adapter
    ///
    /// # Arguments
    ///
    /// - `index`: Adapter index (from enumerate_adapters)
    ///
    /// # Returns
    ///
    /// MetalAdapter for creating logical device
    pub fn request_adapter(&self, index: usize) -> KgpuResult<MetalAdapter> {
        if index >= self.inner.devices.len() {
            return Err(KgpuError::AdapterNotFound);
        }

        let device = self.inner.devices[index].clone();
        MetalAdapter::new(device)
    }

    /// Get default adapter (highest-performance GPU)
    ///
    /// # Performance
    ///
    /// <100μs (cached query)
    ///
    /// # Returns
    ///
    /// Default adapter (discrete GPU if available, otherwise integrated)
    pub fn default_adapter(&self) -> KgpuResult<MetalAdapter> {
        // Prefer discrete GPU over integrated
        for device in &self.inner.devices {
            if !device.is_low_power() {
                return MetalAdapter::new(device.clone());
            }
        }

        // Fallback to first device (integrated GPU)
        self.request_adapter(0)
    }
}

// SAFETY: Metal objects are thread-safe (ARC-managed)
unsafe impl Send for MetalInstanceInner {}
unsafe impl Sync for MetalInstanceInner {}

impl Drop for MetalInstanceInner {
    fn drop(&mut self) {
        // Metal devices are ARC-managed, no explicit cleanup needed
    }
}

#[cfg(test)]
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires Metal support
    fn test_instance_creation() {
        let instance = MetalInstance::new();
        assert!(instance.is_ok(), "Failed to create Metal instance");
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_enumerate_adapters() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters();

        assert!(adapters.is_ok(), "Failed to enumerate adapters");

        if let Ok(list) = adapters {
            println!("Found {} Metal devices:", list.count);
            for adapter in list.iter() {
                println!("  - {} ({:?})", adapter.name_str(), adapter.device_type);
            }
            assert!(list.count > 0, "No adapters found");
        }
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_default_adapter() {
        let instance = MetalInstance::new().unwrap();
        let adapter = instance.default_adapter();

        assert!(adapter.is_ok(), "Failed to get default adapter");
    }
}
