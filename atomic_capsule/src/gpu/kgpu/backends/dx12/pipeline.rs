//! DX12 Pipeline Implementation
//!
//! Wraps ID3D12PipelineState for graphics and compute pipelines.

use windows::Win32::Graphics::Direct3D12::*;
use crate::gpu::kgpu::hal::*;
use super::*;

/// DX12 Render Pipeline (ID3D12PipelineState)
pub struct Dx12RenderPipeline {
    pso: ID3D12PipelineState,
}

/// DX12 Compute Pipeline (ID3D12PipelineState)
pub struct Dx12ComputePipeline {
    pso: ID3D12PipelineState,
}

impl KgpuDeviceApi for Dx12Device {
    // ... other methods ...

    fn create_render_pipeline(
        &self,
        descriptor: &RenderPipelineDescriptor<'_, Self>,
    ) -> HalResult<Dx12RenderPipeline> {
        unsafe {
            // Build pipeline state descriptor
            let pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: if let Some(layout) = descriptor.layout {
                    // Use provided layout
                    core::mem::transmute_copy(&layout.signature)
                } else {
                    // Create default empty root signature
                    core::mem::zeroed()
                },
                VS: D3D12_SHADER_BYTECODE {
                    pShaderBytecode: descriptor.vertex.module.bytecode.as_ptr() as *const _,
                    BytecodeLength: descriptor.vertex.module.bytecode.len(),
                },
                PS: D3D12_SHADER_BYTECODE::default(),
                DS: D3D12_SHADER_BYTECODE::default(),
                HS: D3D12_SHADER_BYTECODE::default(),
                GS: D3D12_SHADER_BYTECODE::default(),
                StreamOutput: D3D12_STREAM_OUTPUT_DESC::default(),
                BlendState: D3D12_BLEND_DESC {
                    AlphaToCoverageEnable: false.into(),
                    IndependentBlendEnable: false.into(),
                    RenderTarget: [D3D12_RENDER_TARGET_BLEND_DESC::default(); 8],
                },
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC {
                    FillMode: D3D12_FILL_MODE_SOLID,
                    CullMode: D3D12_CULL_MODE_BACK,
                    FrontCounterClockwise: false.into(),
                    DepthBias: 0,
                    DepthBiasClamp: 0.0,
                    SlopeScaledDepthBias: 0.0,
                    DepthClipEnable: true.into(),
                    MultisampleEnable: false.into(),
                    AntialiasedLineEnable: false.into(),
                    ForcedSampleCount: 0,
                    ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
                },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC::default(),
                InputLayout: D3D12_INPUT_LAYOUT_DESC::default(),
                IBStripCutValue: D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                RTVFormats: [DXGI_FORMAT_R8G8B8A8_UNORM; 8],
                DSVFormat: DXGI_FORMAT_UNKNOWN,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                NodeMask: 0,
                CachedPSO: D3D12_CACHED_PIPELINE_STATE::default(),
                Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
            };

            let pso: ID3D12PipelineState = self.device
                .CreateGraphicsPipelineState(&pso_desc)
                .map_err(|e| HalError::PipelineError(
                    format!("Failed to create graphics pipeline: {:?}", e).into()
                ))?;

            Ok(Dx12RenderPipeline { pso })
        }
    }

    fn create_compute_pipeline(
        &self,
        descriptor: &ComputePipelineDescriptor<'_, Self>,
    ) -> HalResult<Dx12ComputePipeline> {
        unsafe {
            let pso_desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
                pRootSignature: if let Some(layout) = descriptor.layout {
                    core::mem::transmute_copy(&layout.signature)
                } else {
                    core::mem::zeroed()
                },
                CS: D3D12_SHADER_BYTECODE {
                    pShaderBytecode: descriptor.module.bytecode.as_ptr() as *const _,
                    BytecodeLength: descriptor.module.bytecode.len(),
                },
                NodeMask: 0,
                CachedPSO: D3D12_CACHED_PIPELINE_STATE::default(),
                Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
            };

            let pso: ID3D12PipelineState = self.device
                .CreateComputePipelineState(&pso_desc)
                .map_err(|e| HalError::PipelineError(
                    format!("Failed to create compute pipeline: {:?}", e).into()
                ))?;

            Ok(Dx12ComputePipeline { pso })
        }
    }
}

impl KgpuRenderPipelineApi for Dx12RenderPipeline {
    fn get_bind_group_layout(&self, _index: u32) -> Option<()> {
        // Simplified - would return actual layout
        None
    }
}

impl KgpuComputePipelineApi for Dx12ComputePipeline {
    fn get_bind_group_layout(&self, _index: u32) -> Option<()> {
        // Simplified - would return actual layout
        None
    }
}

// SAFETY: COM objects are thread-safe
unsafe impl Send for Dx12RenderPipeline {}
unsafe impl Sync for Dx12RenderPipeline {}
unsafe impl Send for Dx12ComputePipeline {}
unsafe impl Sync for Dx12ComputePipeline {}
