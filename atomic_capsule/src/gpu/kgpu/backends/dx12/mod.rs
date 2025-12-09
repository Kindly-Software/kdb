//! DirectX 12 Backend for KGPU HAL
//!
//! **Tier**: T7 Heterogeneous (GPU acceleration via DirectX 12)
//! **Platform**: Windows 10 1903+ (D3D12_FEATURE_LEVEL_12_0)
//! **API Version**: DirectX 12 Ultimate (Agility SDK 1.7+)
//!
//! # Architecture
//!
//! Implements the KGPU HAL traits using windows-rs 0.58 bindings:
//!
//! - **Instance**: IDXGIFactory7 (adapter enumeration)
//! - **Adapter**: IDXGIAdapter4 (physical GPU)
//! - **Device**: ID3D12Device5 (logical device, DX12 Ultimate support)
//! - **Queue**: ID3D12CommandQueue (command submission)
//! - **CommandBuffer**: ID3D12GraphicsCommandList (command recording)
//! - **Fence**: ID3D12Fence (CPU-GPU synchronization)
//! - **Swapchain**: IDXGISwapChain4 (presentation, flip model)
//!
//! # DirectX 12 Ultimate Features (2025)
//!
//! - **DXR 1.2**: 40% faster raytracing (if supported)
//! - **Mesh Shaders**: GPU-driven LOD and culling
//! - **Variable Rate Shading**: Focus detail on important areas
//! - **Sampler Feedback**: Advanced texture streaming
//! - **Enhanced Barriers**: Reduced sync latency (Agility SDK 1.7+)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_WINDOWS_10_1903_PLUS`: Minimum OS version for D3D12_FEATURE_LEVEL_12_0
//! - `#ASSUME_WDDM_2_0_PLUS`: Driver model requirement
//! - `#ASSUME_DXGI_1_4_PLUS`: DXGI flip model support
//! - `#ASSUME_COM_THREAD_SAFE`: COM objects are thread-safe
//! - `#ASSUME_DEVICE_LOST_RARE`: Device loss is exceptional (handle gracefully)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (GPU backend)
//! - **Chaos**: HAL trait implementations, AtomicU64 for fence values
//! - **ASSUM**: All assumptions documented with #ASSUME/#VERIFY tags
//! - **B32**: Performance targets (device creation <100ms, fence signal <100ns)
//! - **T28**: Conditional tests (feature = "dx12")
//! - **I20**: Zero breaking changes (backend addition only)
//!
//! # Feature Flag
//!
//! Enable with: `features = ["dx12"]`
//!
//! # Example
//!
//! ```rust,ignore
//! use atomic_capsule::gpu::kgpu::backends::dx12::Dx12Backend;
//! use atomic_capsule::gpu::kgpu::hal::KgpuBackend;
//!
//! if Dx12Backend::is_available() {
//!     let instance = Dx12Instance::new()?;
//!     let adapters = instance.enumerate_adapters()?;
//!     // ... use DX12 backend
//! }
//! ```

#[cfg(target_os = "windows")]
pub mod device;
#[cfg(target_os = "windows")]
pub mod surface;
#[cfg(target_os = "windows")]
pub mod command;
#[cfg(target_os = "windows")]
pub mod pipeline;
#[cfg(target_os = "windows")]
pub mod sync;

#[cfg(target_os = "windows")]
pub use device::Dx12Device;
#[cfg(target_os = "windows")]
pub use surface::Dx12Surface;
#[cfg(target_os = "windows")]
pub use command::{Dx12CommandBuffer, Dx12CommandEncoder};
#[cfg(target_os = "windows")]
pub use pipeline::{Dx12RenderPipeline, Dx12ComputePipeline};
#[cfg(target_os = "windows")]
pub use sync::Dx12Fence;

#[cfg(target_os = "windows")]
use crate::gpu::kgpu::hal::{
    KgpuBackend, KgpuInstanceApi, AdapterList, AdapterOptions, HalResult,
    Features, BackendType,
};

#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Direct3D12::*;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dxgi::*;

// ============================================================================
// Dx12Backend (Root Backend Trait Implementation)
// ============================================================================

#[cfg(target_os = "windows")]
/// DirectX 12 backend implementation
///
/// Provides access to DirectX 12 API on Windows platforms.
pub struct Dx12Backend;

#[cfg(target_os = "windows")]
impl KgpuBackend for Dx12Backend {
    type Instance = Dx12Instance;
    type Adapter = Dx12Adapter;
    type Device = Dx12Device;
    type Queue = Dx12Queue;
    type Buffer = Dx12Buffer;
    type Texture = Dx12Texture;
    type TextureView = Dx12TextureView;
    type Sampler = Dx12Sampler;
    type BindGroup = Dx12BindGroup;
    type BindGroupLayout = Dx12BindGroupLayout;
    type PipelineLayout = Dx12PipelineLayout;
    type RenderPipeline = Dx12RenderPipeline;
    type ComputePipeline = Dx12ComputePipeline;
    type ShaderModule = Dx12ShaderModule;
    type CommandEncoder = Dx12CommandEncoder;
    type CommandBuffer = Dx12CommandBuffer;

    fn name() -> &'static str {
        "DirectX 12"
    }

    fn api_version() -> (u32, u32, u32) {
        // DirectX 12 Ultimate (2020+)
        // Major = 12, Minor = 2 (Ultimate), Patch = 0
        (12, 2, 0)
    }

    fn is_available() -> bool {
        // Check if DX12 is available on the current system
        #[cfg(not(target_os = "windows"))]
        {
            false
        }

        #[cfg(target_os = "windows")]
        {
            // #VERIFY_DX12_AVAILABLE: Try to create a device to check availability
            // This is safe because we're only checking, not actually using the device
            unsafe {
                // Try to load D3D12 DLL
                let result = D3D12CreateDevice(
                    None, // Use default adapter
                    D3D_FEATURE_LEVEL_12_0,
                    &ID3D12Device::IID,
                );

                result.is_ok()
            }
        }
    }
}

#[cfg(target_os = "windows")]
/// DX12 Instance (IDXGIFactory7)
pub struct Dx12Instance {
    factory: IDXGIFactory7,
}

#[cfg(target_os = "windows")]
impl KgpuInstanceApi for Dx12Instance {
    type Adapter = Dx12Adapter;

    fn new() -> HalResult<Self> {
        unsafe {
            // #ASSUME_DXGI_1_6_PLUS: Windows 10 1903+ has DXGI 1.6
            // Create DXGI factory with debug layer if available
            let flags = if cfg!(debug_assertions) {
                DXGI_CREATE_FACTORY_DEBUG
            } else {
                0
            };

            let factory: IDXGIFactory7 = CreateDXGIFactory2(flags)
                .map_err(|e| crate::gpu::kgpu::hal::HalError::InitializationFailed(
                    format!("Failed to create DXGI factory: {:?}", e).into()
                ))?;

            Ok(Self { factory })
        }
    }

    fn enumerate_adapters(&self) -> HalResult<AdapterList> {
        let mut list = AdapterList::default();

        unsafe {
            // Enumerate up to 8 adapters
            for i in 0..8 {
                let adapter: Result<IDXGIAdapter4, _> = self.factory.EnumAdapterByGpuPreference(
                    i,
                    DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
                );

                match adapter {
                    Ok(adapter) => {
                        // Get adapter description
                        let mut desc: DXGI_ADAPTER_DESC3 = core::mem::zeroed();
                        adapter.GetDesc3(&mut desc).ok();

                        // Convert to AdapterInfo
                        let info = crate::gpu::kgpu::hal::AdapterInfo::new(
                            // Convert wide string to UTF-8
                            String::from_utf16_lossy(&desc.Description)
                                .trim_end_matches('\0')
                                .into(),
                            crate::gpu::kgpu::hal::DeviceType::DiscreteGpu, // Simplified
                            BackendType::Dx12,
                        );

                        if !list.push(info) {
                            break; // List full
                        }
                    }
                    Err(_) => break, // No more adapters
                }
            }
        }

        Ok(list)
    }

    fn request_adapter(&self, _options: &AdapterOptions) -> HalResult<Self::Adapter> {
        unsafe {
            // Get high-performance adapter (discrete GPU preferred)
            let adapter: IDXGIAdapter4 = self.factory
                .EnumAdapterByGpuPreference(0, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE)
                .map_err(|e| crate::gpu::kgpu::hal::HalError::AdapterNotFound(
                    format!("No suitable adapter: {:?}", e).into()
                ))?;

            Ok(Dx12Adapter { adapter })
        }
    }

    fn surface_formats(&self) -> &[crate::gpu::kgpu::hal::HalTextureFormat] {
        use crate::gpu::kgpu::hal::HalTextureFormat;

        // Common DX12 swapchain formats (flip model compatible)
        &[
            HalTextureFormat::Bgra8Unorm,     // Most common (DXGI_FORMAT_B8G8R8A8_UNORM)
            HalTextureFormat::Rgba8Unorm,     // Alternative (DXGI_FORMAT_R8G8B8A8_UNORM)
            HalTextureFormat::Rgba16Float,    // HDR (DXGI_FORMAT_R16G16B16A16_FLOAT)
            HalTextureFormat::Rgb10a2Unorm,   // HDR 10-bit (DXGI_FORMAT_R10G10B10A2_UNORM)
        ]
    }
}

#[cfg(target_os = "windows")]
/// DX12 Adapter (IDXGIAdapter4)
pub struct Dx12Adapter {
    adapter: IDXGIAdapter4,
}

#[cfg(target_os = "windows")]
/// DX12 Queue (ID3D12CommandQueue)
pub struct Dx12Queue {
    queue: ID3D12CommandQueue,
}

#[cfg(target_os = "windows")]
/// DX12 Buffer (ID3D12Resource)
pub struct Dx12Buffer {
    resource: ID3D12Resource,
    size: u64,
}

#[cfg(target_os = "windows")]
/// DX12 Texture (ID3D12Resource)
pub struct Dx12Texture {
    resource: ID3D12Resource,
}

#[cfg(target_os = "windows")]
/// DX12 Texture View (descriptor)
pub struct Dx12TextureView {
    descriptor: D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[cfg(target_os = "windows")]
/// DX12 Sampler (descriptor)
pub struct Dx12Sampler {
    descriptor: D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[cfg(target_os = "windows")]
/// DX12 Bind Group (descriptor table)
pub struct Dx12BindGroup {
    heap: ID3D12DescriptorHeap,
}

#[cfg(target_os = "windows")]
/// DX12 Bind Group Layout (root signature)
pub struct Dx12BindGroupLayout {
    signature: ID3D12RootSignature,
}

#[cfg(target_os = "windows")]
/// DX12 Pipeline Layout (root signature)
pub struct Dx12PipelineLayout {
    signature: ID3D12RootSignature,
}

#[cfg(target_os = "windows")]
/// DX12 Shader Module (compiled DXIL bytecode)
pub struct Dx12ShaderModule {
    bytecode: Vec<u8>,
}

// ============================================================================
// Send/Sync Implementations (Chaos Compliance)
// ============================================================================

#[cfg(target_os = "windows")]
// SAFETY: COM objects are thread-safe (AddRef/Release are atomic)
unsafe impl Send for Dx12Instance {}
unsafe impl Sync for Dx12Instance {}
unsafe impl Send for Dx12Adapter {}
unsafe impl Sync for Dx12Adapter {}
unsafe impl Send for Dx12Queue {}
unsafe impl Sync for Dx12Queue {}
unsafe impl Send for Dx12Buffer {}
unsafe impl Sync for Dx12Buffer {}
unsafe impl Send for Dx12Texture {}
unsafe impl Sync for Dx12Texture {}
unsafe impl Send for Dx12TextureView {}
unsafe impl Sync for Dx12TextureView {}
unsafe impl Send for Dx12Sampler {}
unsafe impl Sync for Dx12Sampler {}
unsafe impl Send for Dx12BindGroup {}
unsafe impl Sync for Dx12BindGroup {}
unsafe impl Send for Dx12BindGroupLayout {}
unsafe impl Sync for Dx12BindGroupLayout {}
unsafe impl Send for Dx12PipelineLayout {}
unsafe impl Sync for Dx12PipelineLayout {}
unsafe impl Send for Dx12ShaderModule {}
unsafe impl Sync for Dx12ShaderModule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn test_backend_name() {
        assert_eq!(Dx12Backend::name(), "DirectX 12");
    }

    #[test]
    fn test_backend_version() {
        let (major, minor, patch) = Dx12Backend::api_version();
        assert_eq!(major, 12);
        assert_eq!(minor, 2); // DX12 Ultimate
        assert_eq!(patch, 0);
    }

    #[test]
    #[ignore] // Requires Windows 10 1903+
    fn test_backend_available() {
        // This test requires actual DX12 support
        // Skip on CI or systems without DX12
        if Dx12Backend::is_available() {
            println!("DX12 is available on this system");
        } else {
            println!("DX12 is NOT available on this system");
        }
    }

    #[test]
    #[ignore] // Requires DX12 support
    fn test_instance_creation() {
        if !Dx12Backend::is_available() {
            return;
        }

        let instance = Dx12Instance::new();
        assert!(instance.is_ok(), "Failed to create DX12 instance");
    }

    #[test]
    #[ignore] // Requires DX12 support
    fn test_enumerate_adapters() {
        if !Dx12Backend::is_available() {
            return;
        }

        let instance = Dx12Instance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();

        println!("Found {} DX12 adapters", adapters.count);
        for info in adapters.iter() {
            println!("  - {} ({:?})", info.name_str(), info.device_type());
        }
    }
}
