//! DX12 Surface Implementation (IDXGISwapChain4)
//!
//! Implements DXGI flip model swapchains with HDR support.

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::gpu::kgpu::hal::*;
use super::*;

/// DX12 Surface (IDXGISwapChain4)
pub struct Dx12Surface {
    swapchain: IDXGISwapChain4,
    format: DXGI_FORMAT,
    width: u32,
    height: u32,
}

impl Dx12Surface {
    /// Creates a new swapchain from window handle
    ///
    /// Uses DXGI_SWAP_EFFECT_FLIP_DISCARD for best performance.
    #[allow(dead_code)]
    pub fn new(
        device: &Dx12Device,
        factory: &IDXGIFactory7,
        hwnd: isize,
        width: u32,
        height: u32,
        format: HalTextureFormat,
    ) -> HalResult<Self> {
        unsafe {
            let dxgi_format = match format {
                HalTextureFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
                HalTextureFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
                HalTextureFormat::Rgba16Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
                _ => DXGI_FORMAT_B8G8R8A8_UNORM,
            };

            let swapchain_desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: dxgi_format,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2, // Double buffering (minimum for flip model)
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD, // Best performance
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                Flags: DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0 as u32, // Variable refresh rate
            };

            let swapchain1: IDXGISwapChain1 = factory.CreateSwapChainForHwnd(
                &device.command_queue.queue,
                windows::Win32::Foundation::HWND(hwnd),
                &swapchain_desc,
                None,
                None,
            ).map_err(|e| HalError::SurfaceError(
                format!("Failed to create swapchain: {:?}", e).into()
            ))?;

            let swapchain: IDXGISwapChain4 = swapchain1.cast()
                .map_err(|e| HalError::SurfaceError(
                    format!("Failed to query IDXGISwapChain4: {:?}", e).into()
                ))?;

            Ok(Self {
                swapchain,
                format: dxgi_format,
                width,
                height,
            })
        }
    }

    /// Gets current backbuffer index
    #[allow(dead_code)]
    pub fn current_backbuffer_index(&self) -> u32 {
        unsafe { self.swapchain.GetCurrentBackBufferIndex() }
    }

    /// Presents the current frame
    #[allow(dead_code)]
    pub fn present(&self, sync_interval: u32) -> Result<(), windows::core::Error> {
        unsafe {
            self.swapchain.Present(sync_interval, 0)?;
            Ok(())
        }
    }

    /// Resizes the swapchain
    #[allow(dead_code)]
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), windows::core::Error> {
        unsafe {
            self.swapchain.ResizeBuffers(
                0, // Keep buffer count
                width,
                height,
                self.format,
                DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0 as u32,
            )?;
            self.width = width;
            self.height = height;
            Ok(())
        }
    }

    /// Gets backbuffer resource
    #[allow(dead_code)]
    pub fn get_backbuffer(&self, buffer_index: u32) -> Result<ID3D12Resource, windows::core::Error> {
        unsafe {
            self.swapchain.GetBuffer(buffer_index)
        }
    }
}

// SAFETY: COM objects are thread-safe
unsafe impl Send for Dx12Surface {}
unsafe impl Sync for Dx12Surface {}
