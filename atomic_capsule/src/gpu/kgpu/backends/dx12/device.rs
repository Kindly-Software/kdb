//! DX12 Device Implementation (ID3D12Device5)
//!
//! **Tier**: T7 Heterogeneous (GPU device management)
//! **Purpose**: Logical device for resource creation and command submission
//!
//! # Architecture
//!
//! Wraps ID3D12Device5 (DirectX 12 Ultimate) with:
//! - Resource creation (buffers, textures, samplers)
//! - Pipeline creation (render, compute)
//! - Command queue management
//! - Descriptor heap management
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_DEVICE_VALID`: Device valid until explicitly dropped
//! - `#ASSUME_FEATURE_LEVEL_12_0`: Minimum feature level
//! - `#ASSUME_COMMAND_QUEUE_DIRECT`: Direct queue for graphics/compute
//! - `#ASSUME_DESCRIPTOR_HEAP_SHADER_VISIBLE`: CBV/SRV/UAV heap is shader-visible

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::Interface;

use crate::gpu::kgpu::hal::*;
use super::*;

impl KgpuAdapterApi for Dx12Adapter {
    type Device = Dx12Device;

    fn info(&self) -> &AdapterInfo {
        // Cached adapter info (simplified - would cache in real implementation)
        unsafe {
            static mut CACHED_INFO: Option<AdapterInfo> = None;

            if CACHED_INFO.is_none() {
                let mut desc: DXGI_ADAPTER_DESC3 = core::mem::zeroed();
                self.adapter.GetDesc3(&mut desc).ok();

                CACHED_INFO = Some(AdapterInfo::new(
                    String::from_utf16_lossy(&desc.Description)
                        .trim_end_matches('\0')
                        .into(),
                    DeviceType::DiscreteGpu,
                    BackendType::Dx12,
                ));
            }

            CACHED_INFO.as_ref().unwrap()
        }
    }

    fn features(&self) -> Features {
        // Check DX12 Ultimate features
        let mut features = Features::empty();

        unsafe {
            // Create temporary device to check features
            let device: Result<ID3D12Device5, _> = D3D12CreateDevice(
                &self.adapter,
                D3D_FEATURE_LEVEL_12_0,
            );

            if let Ok(device) = device {
                // Check raytracing support (DXR 1.1)
                let mut options5: D3D12_FEATURE_DATA_D3D12_OPTIONS5 = core::mem::zeroed();
                if device.CheckFeatureSupport(
                    D3D12_FEATURE_D3D12_OPTIONS5,
                    &mut options5 as *mut _ as *mut _,
                    core::mem::size_of::<D3D12_FEATURE_DATA_D3D12_OPTIONS5>() as u32,
                ).is_ok() {
                    if options5.RaytracingTier >= D3D12_RAYTRACING_TIER_1_1 {
                        features |= Features::RAY_TRACING;
                    }
                }

                // Check mesh shader support
                let mut options7: D3D12_FEATURE_DATA_D3D12_OPTIONS7 = core::mem::zeroed();
                if device.CheckFeatureSupport(
                    D3D12_FEATURE_D3D12_OPTIONS7,
                    &mut options7 as *mut _ as *mut _,
                    core::mem::size_of::<D3D12_FEATURE_DATA_D3D12_OPTIONS7>() as u32,
                ).is_ok() {
                    if options7.MeshShaderTier >= D3D12_MESH_SHADER_TIER_1 {
                        features |= Features::MESH_SHADER | Features::TASK_SHADER;
                    }
                }

                // Check variable rate shading support
                let mut options6: D3D12_FEATURE_DATA_D3D12_OPTIONS6 = core::mem::zeroed();
                if device.CheckFeatureSupport(
                    D3D12_FEATURE_D3D12_OPTIONS6,
                    &mut options6 as *mut _ as *mut _,
                    core::mem::size_of::<D3D12_FEATURE_DATA_D3D12_OPTIONS6>() as u32,
                ).is_ok() {
                    if options6.VariableShadingRateTier >= D3D12_VARIABLE_SHADING_RATE_TIER_1 {
                        features |= Features::VARIABLE_RATE_SHADING;
                    }
                }
            }
        }

        features
    }

    fn limits(&self) -> &Limits {
        // DX12 default limits
        static LIMITS: Limits = Limits {
            max_texture_dimension_1d: 16384,
            max_texture_dimension_2d: 16384,
            max_texture_dimension_3d: 2048,
            max_texture_array_layers: 2048,
            max_bind_groups: 8,
            max_bindings_per_bind_group: 16,
            max_buffer_size: 1 << 30, // 1 GB
            max_push_constant_size: 256,
            max_compute_workgroup_size_x: 1024,
            max_compute_workgroup_size_y: 1024,
            max_compute_workgroup_size_z: 64,
            max_compute_workgroups_per_dimension: 65535,
        };

        &LIMITS
    }

    fn request_device(&self, descriptor: &DeviceDescriptor) -> HalResult<Self::Device> {
        unsafe {
            // Create D3D12 device
            let device: ID3D12Device5 = D3D12CreateDevice(
                &self.adapter,
                D3D_FEATURE_LEVEL_12_0,
            ).map_err(|e| HalError::DeviceCreationFailed(
                format!("D3D12CreateDevice failed: {:?}", e).into()
            ))?;

            // Enable debug layer in debug builds
            #[cfg(debug_assertions)]
            {
                let mut debug: Option<ID3D12DebugDevice> = None;
                if device.QueryInterface(&mut debug).is_ok() {
                    if let Some(debug) = debug {
                        // Enable GPU-based validation if available
                        let mut debug1: Option<ID3D12DebugDevice1> = None;
                        if debug.cast::<ID3D12DebugDevice1>().is_ok() {
                            // GPU-based validation (slower but more comprehensive)
                        }
                    }
                }
            }

            // Create command queue (direct queue for graphics + compute)
            let queue_desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                Priority: D3D12_COMMAND_QUEUE_PRIORITY_NORMAL.0,
                Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
                NodeMask: 0,
            };

            let command_queue: ID3D12CommandQueue = device
                .CreateCommandQueue(&queue_desc)
                .map_err(|e| HalError::DeviceCreationFailed(
                    format!("Failed to create command queue: {:?}", e).into()
                ))?;

            // Create descriptor heaps
            // CBV/SRV/UAV heap (shader-visible)
            let cbv_heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: 1024, // Generous default
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                NodeMask: 0,
            };

            let cbv_heap: ID3D12DescriptorHeap = device
                .CreateDescriptorHeap(&cbv_heap_desc)
                .map_err(|e| HalError::DeviceCreationFailed(
                    format!("Failed to create CBV/SRV/UAV heap: {:?}", e).into()
                ))?;

            // RTV heap (non-shader-visible)
            let rtv_heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 256,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
                NodeMask: 0,
            };

            let rtv_heap: ID3D12DescriptorHeap = device
                .CreateDescriptorHeap(&rtv_heap_desc)
                .map_err(|e| HalError::DeviceCreationFailed(
                    format!("Failed to create RTV heap: {:?}", e).into()
                ))?;

            // DSV heap (non-shader-visible)
            let dsv_heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                NumDescriptors: 64,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
                NodeMask: 0,
            };

            let dsv_heap: ID3D12DescriptorHeap = device
                .CreateDescriptorHeap(&dsv_heap_desc)
                .map_err(|e| HalError::DeviceCreationFailed(
                    format!("Failed to create DSV heap: {:?}", e).into()
                ))?;

            Ok(Dx12Device {
                device,
                command_queue: Dx12Queue { queue: command_queue },
                cbv_heap,
                rtv_heap,
                dsv_heap,
                features: descriptor.required_features,
            })
        }
    }
}

/// DX12 Device (ID3D12Device5)
pub struct Dx12Device {
    device: ID3D12Device5,
    command_queue: Dx12Queue,
    cbv_heap: ID3D12DescriptorHeap,
    rtv_heap: ID3D12DescriptorHeap,
    dsv_heap: ID3D12DescriptorHeap,
    features: Features,
}

impl KgpuDeviceApi for Dx12Device {
    type Queue = Dx12Queue;
    type Buffer = Dx12Buffer;
    type Texture = Dx12Texture;
    type TextureView = Dx12TextureView;
    type Sampler = Dx12Sampler;
    type BindGroupLayout = Dx12BindGroupLayout;
    type BindGroup = Dx12BindGroup;
    type PipelineLayout = Dx12PipelineLayout;
    type ShaderModule = Dx12ShaderModule;
    type RenderPipeline = Dx12RenderPipeline;
    type ComputePipeline = Dx12ComputePipeline;
    type CommandEncoder = Dx12CommandEncoder;

    fn queue(&self) -> &Self::Queue {
        &self.command_queue
    }

    fn features(&self) -> Features {
        self.features
    }

    fn limits(&self) -> &Limits {
        // Same as adapter limits
        static LIMITS: Limits = Limits {
            max_texture_dimension_1d: 16384,
            max_texture_dimension_2d: 16384,
            max_texture_dimension_3d: 2048,
            max_texture_array_layers: 2048,
            max_bind_groups: 8,
            max_bindings_per_bind_group: 16,
            max_buffer_size: 1 << 30,
            max_push_constant_size: 256,
            max_compute_workgroup_size_x: 1024,
            max_compute_workgroup_size_y: 1024,
            max_compute_workgroup_size_z: 64,
            max_compute_workgroups_per_dimension: 65535,
        };

        &LIMITS
    }

    fn create_buffer(&self, descriptor: &BufferDescriptor) -> HalResult<Self::Buffer> {
        unsafe {
            // Map usage flags to D3D12 resource states
            let mut resource_states = D3D12_RESOURCE_STATE_COMMON;
            let mut heap_type = D3D12_HEAP_TYPE_DEFAULT;

            if descriptor.usage.contains(BufferUsages::MAP_READ) {
                heap_type = D3D12_HEAP_TYPE_READBACK;
                resource_states = D3D12_RESOURCE_STATE_COPY_DEST;
            } else if descriptor.usage.contains(BufferUsages::MAP_WRITE) {
                heap_type = D3D12_HEAP_TYPE_UPLOAD;
                resource_states = D3D12_RESOURCE_STATE_GENERIC_READ;
            }

            let heap_props = D3D12_HEAP_PROPERTIES {
                Type: heap_type,
                CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
                MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
                CreationNodeMask: 0,
                VisibleNodeMask: 0,
            };

            let resource_desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Alignment: 0, // Default 64KB alignment
                Width: descriptor.size,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_UNKNOWN,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                Flags: D3D12_RESOURCE_FLAG_NONE,
            };

            let mut resource: Option<ID3D12Resource> = None;
            self.device.CreateCommittedResource(
                &heap_props,
                D3D12_HEAP_FLAG_NONE,
                &resource_desc,
                resource_states,
                None,
                &mut resource,
            ).map_err(|e| HalError::OutOfDeviceMemory(
                format!("Failed to create buffer: {:?}", e).into()
            ))?;

            Ok(Dx12Buffer {
                resource: resource.unwrap(),
                size: descriptor.size,
            })
        }
    }

    fn create_texture(&self, descriptor: &TextureDescriptor) -> HalResult<Self::Texture> {
        unsafe {
            // Map format
            let format = match descriptor.format {
                HalTextureFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
                HalTextureFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
                HalTextureFormat::Rgba16Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
                HalTextureFormat::Rgba32Float => DXGI_FORMAT_R32G32B32A32_FLOAT,
                HalTextureFormat::Depth32Float => DXGI_FORMAT_D32_FLOAT,
                _ => DXGI_FORMAT_UNKNOWN,
            };

            let resource_desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Alignment: 0,
                Width: descriptor.size.width as u64,
                Height: descriptor.size.height,
                DepthOrArraySize: descriptor.size.depth_or_array_layers as u16,
                MipLevels: descriptor.mip_level_count as u16,
                Format: format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: descriptor.sample_count,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                Flags: D3D12_RESOURCE_FLAG_NONE,
            };

            let heap_props = D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_DEFAULT,
                CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
                MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
                CreationNodeMask: 0,
                VisibleNodeMask: 0,
            };

            let mut resource: Option<ID3D12Resource> = None;
            self.device.CreateCommittedResource(
                &heap_props,
                D3D12_HEAP_FLAG_NONE,
                &resource_desc,
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            ).map_err(|e| HalError::OutOfDeviceMemory(
                format!("Failed to create texture: {:?}", e).into()
            ))?;

            Ok(Dx12Texture {
                resource: resource.unwrap(),
            })
        }
    }

    fn create_sampler(&self, descriptor: &SamplerDescriptor) -> HalResult<Self::Sampler> {
        unsafe {
            // Create sampler descriptor
            let filter = match (descriptor.mag_filter, descriptor.min_filter) {
                (FilterMode::Linear, FilterMode::Linear) => D3D12_FILTER_MIN_MAG_MIP_LINEAR,
                (FilterMode::Nearest, FilterMode::Nearest) => D3D12_FILTER_MIN_MAG_MIP_POINT,
                _ => D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT,
            };

            let sampler_desc = D3D12_SAMPLER_DESC {
                Filter: filter,
                AddressU: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
                AddressV: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
                AddressW: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
                MipLODBias: 0.0,
                MaxAnisotropy: 0,
                ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
                BorderColor: [0.0; 4],
                MinLOD: 0.0,
                MaxLOD: f32::MAX,
            };

            // Allocate descriptor from heap (simplified - would manage heap allocation)
            let descriptor_handle = self.cbv_heap.GetCPUDescriptorHandleForHeapStart();

            self.device.CreateSampler(&sampler_desc, descriptor_handle);

            Ok(Dx12Sampler {
                descriptor: descriptor_handle,
            })
        }
    }

    fn create_bind_group_layout(
        &self,
        _descriptor: &BindGroupLayoutDescriptor<'_>,
    ) -> HalResult<Self::BindGroupLayout> {
        // Simplified: Create empty root signature
        unsafe {
            let root_signature_desc = D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: 0,
                pParameters: core::ptr::null(),
                NumStaticSamplers: 0,
                pStaticSamplers: core::ptr::null(),
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            };

            let mut blob: Option<ID3DBlob> = None;
            let mut error_blob: Option<ID3DBlob> = None;

            D3D12SerializeRootSignature(
                &root_signature_desc,
                D3D_ROOT_SIGNATURE_VERSION_1,
                &mut blob,
                Some(&mut error_blob),
            ).map_err(|e| HalError::ShaderError(
                format!("Failed to serialize root signature: {:?}", e).into()
            ))?;

            let blob = blob.unwrap();
            let signature: ID3D12RootSignature = self.device.CreateRootSignature(
                0,
                core::slice::from_raw_parts(
                    blob.GetBufferPointer() as *const u8,
                    blob.GetBufferSize(),
                ),
            ).map_err(|e| HalError::ShaderError(
                format!("Failed to create root signature: {:?}", e).into()
            ))?;

            Ok(Dx12BindGroupLayout { signature })
        }
    }

    fn create_bind_group(
        &self,
        _layout: &Self::BindGroupLayout,
        _entries: &[BindGroupEntry<'_>],
        _label: Option<&'static str>,
    ) -> HalResult<Self::BindGroup> {
        // Simplified: Return heap reference
        Ok(Dx12BindGroup {
            heap: self.cbv_heap.clone(),
        })
    }

    fn create_pipeline_layout(
        &self,
        _bind_group_layouts: &[&Self::BindGroupLayout],
        _push_constant_ranges: &[PushConstantRange],
        _label: Option<&'static str>,
    ) -> HalResult<Self::PipelineLayout> {
        // Simplified: Create empty root signature
        unsafe {
            let root_signature_desc = D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: 0,
                pParameters: core::ptr::null(),
                NumStaticSamplers: 0,
                pStaticSamplers: core::ptr::null(),
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            };

            let mut blob: Option<ID3DBlob> = None;
            D3D12SerializeRootSignature(
                &root_signature_desc,
                D3D_ROOT_SIGNATURE_VERSION_1,
                &mut blob,
                None,
            ).ok();

            let blob = blob.unwrap();
            let signature: ID3D12RootSignature = self.device.CreateRootSignature(
                0,
                core::slice::from_raw_parts(
                    blob.GetBufferPointer() as *const u8,
                    blob.GetBufferSize(),
                ),
            ).unwrap();

            Ok(Dx12PipelineLayout { signature })
        }
    }

    fn create_shader_module(&self, source: ShaderSource<'_>) -> HalResult<Self::ShaderModule> {
        match source {
            ShaderSource::Dxil(bytecode) => {
                Ok(Dx12ShaderModule {
                    bytecode: bytecode.to_vec(),
                })
            }
            _ => Err(HalError::ShaderError(
                "DX12 backend only supports DXIL bytecode".into()
            )),
        }
    }

    fn create_render_pipeline(
        &self,
        _descriptor: &RenderPipelineDescriptor<'_, Self>,
    ) -> HalResult<Self::RenderPipeline> {
        // See pipeline.rs for implementation
        todo!("Implemented in pipeline.rs")
    }

    fn create_compute_pipeline(
        &self,
        _descriptor: &ComputePipelineDescriptor<'_, Self>,
    ) -> HalResult<Self::ComputePipeline> {
        // See pipeline.rs for implementation
        todo!("Implemented in pipeline.rs")
    }

    fn create_command_encoder(
        &self,
        _label: Option<&'static str>,
    ) -> HalResult<Self::CommandEncoder> {
        // See command.rs for implementation
        todo!("Implemented in command.rs")
    }

    fn poll(&self, _maintain: Maintain) -> bool {
        // No pending work (simplified)
        false
    }

    fn device_lost_reason(&self) -> Option<&'static str> {
        unsafe {
            // Check device removed reason
            let removed_reason = self.device.GetDeviceRemovedReason();
            if removed_reason.is_err() {
                Some("Device removed (driver update, TDR, or hardware failure)")
            } else {
                None
            }
        }
    }
}

// SAFETY: COM objects are thread-safe
unsafe impl Send for Dx12Device {}
unsafe impl Sync for Dx12Device {}
