//! Metal Command Buffer Implementation
//!
//! # Architecture
//!
//! MetalCommandBuffer wraps MTLCommandBuffer for GPU command recording.
//!
//! - **Command Buffer**: MTLCommandBuffer (NOT reusable, created per-frame)
//! - **Encoders**: MTLRenderCommandEncoder, MTLComputeCommandEncoder, MTLBlitCommandEncoder
//! - **Submission**: commit() pushes to GPU queue
//! - **Synchronization**: waitUntilCompleted(), addCompletedHandler()
//!
//! # Performance
//!
//! - Creation: <100μs (from command queue)
//! - Commit: <100μs (non-blocking)
//! - Encoder creation: <10μs (renderCommandEncoderWithDescriptor)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_COMMAND_BUFFER_SINGLE_USE`: Command buffers are NOT reusable
//! - `#ASSUME_ONE_ENCODER_ACTIVE`: Only one encoder active at a time
//! - `#ASSUME_COMMIT_ONCE`: commit() must be called exactly once
//! - `#VERIFY_UNSAFE_FFI`: metal-rs wraps MTL* calls safely

use metal::{self, CommandBufferRef, CommandEncoderRef, RenderPassDescriptor};
use std::sync::Arc;

use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::MetalDevice;

/// Metal command buffer capsule
///
/// # Layout
///
/// - 64B cache-aligned (Arc overhead)
/// - MTLCommandBuffer reference (ARC-managed)
/// - NOT reusable (must create new buffer each frame)
///
/// # Lifecycle
///
/// ```text
/// Created → Recording → Committed → Completed
///   ↓                       ↓
/// Drop (released)      GPU executing
/// ```
pub struct MetalCommandBuffer {
    /// Inner state
    inner: Arc<MetalCommandBufferInner>,
}

struct MetalCommandBufferInner {
    /// Device reference
    device: MetalDevice,

    /// MTLCommandBuffer handle
    command_buffer: metal::CommandBuffer,

    /// Command buffer committed
    committed: std::sync::atomic::AtomicBool,
}

impl MetalCommandBuffer {
    /// Create new command buffer
    ///
    /// # Performance
    ///
    /// <100μs (B32 target)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_QUEUE_NOT_FULL`: Command queue has space
    /// - `#ASSUME_BUFFER_NOT_NULL`: Command buffer is non-null
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use atomic_capsule::gpu::kgpu::backends::metal::*;
    /// # let device = MetalDevice::new(/* adapter */)?;
    /// let cmd_buffer = MetalCommandBuffer::new(device)?;
    /// // Record commands...
    /// cmd_buffer.commit()?;
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub fn new(device: MetalDevice) -> KgpuResult<Self> {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps new_command_buffer safely
        let command_buffer = device.command_queue().new_command_buffer();

        Ok(Self {
            inner: Arc::new(MetalCommandBufferInner {
                device,
                command_buffer,
                committed: std::sync::atomic::AtomicBool::new(false),
            }),
        })
    }

    /// Begin render encoder
    ///
    /// # Performance
    ///
    /// <10μs (encoder creation)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_NO_ACTIVE_ENCODER`: No encoder currently active
    /// - `#ASSUME_PASS_DESCRIPTOR_VALID`: Pass descriptor is valid
    ///
    /// # Returns
    ///
    /// Returns MetalRenderEncoder that must be end_encoding()'d before commit
    pub fn begin_render_pass(
        &self,
        descriptor: &RenderPassDescriptor,
    ) -> KgpuResult<MetalRenderEncoder> {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps render encoder creation safely
        let encoder = self.inner.command_buffer.new_render_command_encoder(descriptor);

        Ok(MetalRenderEncoder { encoder })
    }

    /// Begin compute encoder
    ///
    /// # Performance
    ///
    /// <10μs (encoder creation)
    pub fn begin_compute_pass(&self) -> KgpuResult<MetalComputeEncoder> {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps compute encoder creation safely
        let encoder = self.inner.command_buffer.new_compute_command_encoder();

        Ok(MetalComputeEncoder { encoder })
    }

    /// Begin blit encoder (copy operations)
    ///
    /// # Performance
    ///
    /// <10μs (encoder creation)
    pub fn begin_blit_pass(&self) -> KgpuResult<MetalBlitEncoder> {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps blit encoder creation safely
        let encoder = self.inner.command_buffer.new_blit_command_encoder();

        Ok(MetalBlitEncoder { encoder })
    }

    /// Commit command buffer to GPU queue
    ///
    /// # Performance
    ///
    /// <100μs (non-blocking, returns immediately)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_ALL_ENCODERS_ENDED`: All encoders must call end_encoding() first
    /// - `#ASSUME_COMMIT_ONCE`: commit() must be called exactly once
    ///
    /// # Errors
    ///
    /// Returns error if buffer already committed
    pub fn commit(&self) -> KgpuResult<()> {
        // Check if already committed
        if self.inner.committed.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return Err(KgpuError::OperationFailed(
                "Command buffer already committed".into(),
            ));
        }

        // #VERIFY_UNSAFE_FFI: metal-rs wraps commit safely
        self.inner.command_buffer.commit();

        Ok(())
    }

    /// Wait for command buffer to complete execution
    ///
    /// # Performance
    ///
    /// Variable (depends on GPU workload)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_BUFFER_COMMITTED`: commit() must be called first
    pub fn wait_until_completed(&self) {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps wait_until_completed safely
        self.inner.command_buffer.wait_until_completed();
    }

    /// Get raw MTLCommandBuffer
    pub(crate) fn raw(&self) -> &metal::CommandBuffer {
        &self.inner.command_buffer
    }
}

/// Metal render command encoder
pub struct MetalRenderEncoder {
    encoder: metal::RenderCommandEncoder,
}

impl MetalRenderEncoder {
    /// Set render pipeline state
    pub fn set_render_pipeline_state(&self, pipeline: &metal::RenderPipelineState) {
        self.encoder.set_render_pipeline_state(pipeline);
    }

    /// Set vertex buffer
    pub fn set_vertex_buffer(&self, index: u64, buffer: Option<&metal::BufferRef>, offset: u64) {
        self.encoder.set_vertex_buffer(index, buffer, offset);
    }

    /// Set fragment buffer
    pub fn set_fragment_buffer(&self, index: u64, buffer: Option<&metal::BufferRef>, offset: u64) {
        self.encoder.set_fragment_buffer(index, buffer, offset);
    }

    /// Draw primitives
    pub fn draw_primitives(
        &self,
        primitive_type: metal::MTLPrimitiveType,
        vertex_start: u64,
        vertex_count: u64,
    ) {
        self.encoder.draw_primitives(primitive_type, vertex_start, vertex_count);
    }

    /// Draw indexed primitives
    pub fn draw_indexed_primitives(
        &self,
        primitive_type: metal::MTLPrimitiveType,
        index_count: u64,
        index_type: metal::MTLIndexType,
        index_buffer: &metal::BufferRef,
        index_buffer_offset: u64,
    ) {
        self.encoder.draw_indexed_primitives(
            primitive_type,
            index_count,
            index_type,
            index_buffer,
            index_buffer_offset,
        );
    }

    /// End encoding (MUST be called before commit)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_END_ENCODING_ONCE`: end_encoding() must be called exactly once
    pub fn end_encoding(self) {
        self.encoder.end_encoding();
    }
}

/// Metal compute command encoder
pub struct MetalComputeEncoder {
    encoder: metal::ComputeCommandEncoder,
}

impl MetalComputeEncoder {
    /// Set compute pipeline state
    pub fn set_compute_pipeline_state(&self, pipeline: &metal::ComputePipelineState) {
        self.encoder.set_compute_pipeline_state(pipeline);
    }

    /// Set buffer
    pub fn set_buffer(&self, index: u64, buffer: Option<&metal::BufferRef>, offset: u64) {
        self.encoder.set_buffer(index, buffer, offset);
    }

    /// Dispatch threadgroups
    pub fn dispatch_thread_groups(
        &self,
        threadgroups_per_grid: metal::MTLSize,
        threads_per_threadgroup: metal::MTLSize,
    ) {
        self.encoder.dispatch_thread_groups(threadgroups_per_grid, threads_per_threadgroup);
    }

    /// End encoding (MUST be called before commit)
    pub fn end_encoding(self) {
        self.encoder.end_encoding();
    }
}

/// Metal blit command encoder (copy operations)
pub struct MetalBlitEncoder {
    encoder: metal::BlitCommandEncoder,
}

impl MetalBlitEncoder {
    /// Copy buffer to buffer
    pub fn copy_from_buffer(
        &self,
        source_buffer: &metal::BufferRef,
        source_offset: u64,
        destination_buffer: &metal::BufferRef,
        destination_offset: u64,
        size: u64,
    ) {
        self.encoder.copy_from_buffer(
            source_buffer,
            source_offset,
            destination_buffer,
            destination_offset,
            size,
        );
    }

    /// End encoding (MUST be called before commit)
    pub fn end_encoding(self) {
        self.encoder.end_encoding();
    }
}

// SAFETY: MTLCommandBuffer and encoders are thread-safe (internally synchronized)
unsafe impl Send for MetalCommandBufferInner {}
unsafe impl Sync for MetalCommandBufferInner {}

impl Drop for MetalCommandBufferInner {
    fn drop(&mut self) {
        // MTLCommandBuffer is ARC-managed, no explicit cleanup needed
        // If not committed, buffer is automatically released
    }
}

#[cfg(test)]
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod tests {
    use super::*;
    use super::super::{MetalInstance, MetalDevice};

    #[test]
    #[ignore] // Requires Metal support
    fn test_command_buffer_creation() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let cmd_buffer = MetalCommandBuffer::new(device);
        assert!(cmd_buffer.is_ok(), "Failed to create command buffer");
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_command_buffer_commit() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let cmd_buffer = MetalCommandBuffer::new(device).unwrap();
        cmd_buffer.commit().unwrap();

        // Second commit should fail
        let result = cmd_buffer.commit();
        assert!(result.is_err(), "Double commit should fail");
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_command_buffer_wait() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let cmd_buffer = MetalCommandBuffer::new(device).unwrap();
        cmd_buffer.commit().unwrap();
        cmd_buffer.wait_until_completed();
        // Should return (no work to do)
    }
}
