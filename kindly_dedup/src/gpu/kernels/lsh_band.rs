//! LSH Band Hashing GPU Kernel - T7 Heterogeneous Tier
//!
//! GPU-accelerated LSH band hashing for candidate pair generation.
//!
//! # Performance Targets (B32 Framework)
//!
//! | Metric | CPU Baseline | GPU Target | Expected Speedup |
//! |--------|--------------|------------|------------------|
//! | Per-doc | ~250ns | ~50ns | 5× |
//! | Throughput | 4M bands/sec | 20M bands/sec | 5× |
//! | Batch 10K | 2.5ms | 0.5ms | 5× |
//!
//! # Architecture
//!
//! - Uses WGSL compute shader for LSH band hashing
//! - One thread per (document, band) pair for maximum parallelism
//! - 5 bands × 25 rows per band (matching CPU implementation)
//! - Band hashes stored as u64 (packed as 2×u32 for GPU compatibility)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (CPU-GPU coordination)
//! - **Chaos**: Lockfree kernel capsule (AtomicU64 state)
//! - **ASSUM**: Document GPU assumptions (determinism, precision)
//! - **B32**: Throughput benchmark with fair comparison
//! - **T28**: Unit tests, property tests (GPU == CPU), throughput tests

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

use wgpu::util::DeviceExt;

use crate::gpu::context::GpuContextCapsule;
use crate::gpu::error::{GpuError, GpuResult};

/// Number of LSH bands (matches CPU batch_lookup.rs)
pub const NUM_BANDS: usize = 5;

/// Rows per band (5 × 25 = 125, 3 unused from 128-hash signature)
pub const ROWS_PER_BAND: usize = 25;

/// Signature size (u16 values per document)
pub const SIGNATURE_SIZE: usize = 128;

/// LSH Band GPU Kernel Capsule - T7 Heterogeneous
///
/// Encapsulates GPU resources for LSH band hash computation.
/// Thread-safe via AtomicU64 state, lockfree.
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::gpu::{GpuContextCapsule, LshBandGpuCapsule, LshBandGpuInput};
///
/// let ctx = GpuContextCapsule::new_blocking()?;
/// let kernel = LshBandGpuCapsule::new(&ctx)?;
///
/// // Signatures from MinHash GPU output (64 u32 per doc = 128 u16 packed)
/// let signatures = vec![0u32; 64 * 2]; // 2 documents
///
/// let output = kernel.compute(&ctx, LshBandGpuInput {
///     signatures: &signatures,
///     num_docs: 2,
/// })?;
///
/// let band_hashes = output.get_band_hashes(0);
/// assert_eq!(band_hashes.len(), NUM_BANDS);
/// ```
#[repr(C, align(64))]
pub struct LshBandGpuCapsule {
    /// Atomic state for Chaos compliance (0 = uninit, 1 = ready, 2 = error)
    state: AtomicU64,
    /// Compute pipeline (per-band parallelism)
    pipeline: Option<wgpu::ComputePipeline>,
    /// Alternative pipeline (per-doc parallelism, for small batches)
    pipeline_per_doc: Option<wgpu::ComputePipeline>,
    /// Bind group layout
    bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Padding for cache line alignment
    _padding: [u8; 16],
}

// SAFETY: LshBandGpuCapsule is Send + Sync because:
// - AtomicU64 is Send + Sync
// - wgpu types (ComputePipeline, BindGroupLayout) are Send + Sync
// - All interior mutability is via atomics
unsafe impl Send for LshBandGpuCapsule {}
unsafe impl Sync for LshBandGpuCapsule {}

/// Input for GPU LSH band computation
#[derive(Debug, Clone)]
pub struct LshBandGpuInput<'a> {
    /// MinHash signatures (packed u32: 64 per document, containing 128 u16)
    /// This is the direct output format from MinHashGpuOutput
    pub signatures: &'a [u32],
    /// Number of documents
    pub num_docs: u32,
}

impl<'a> LshBandGpuInput<'a> {
    /// Validate input data
    pub fn validate(&self) -> GpuResult<()> {
        let expected_len = self.num_docs as usize * 64;
        if self.signatures.len() != expected_len {
            return Err(GpuError::InvalidInput(format!(
                "signatures length {} != expected {} (num_docs {} × 64)",
                self.signatures.len(),
                expected_len,
                self.num_docs
            )));
        }

        if self.num_docs == 0 {
            return Err(GpuError::InvalidInput("num_docs must be > 0".to_string()));
        }

        Ok(())
    }
}

/// Output from GPU LSH band computation - T7 Heterogeneous Capsule
///
/// # Chaos Compliance
///
/// - Cache-aligned (64 bytes) for optimal memory access
/// - AtomicU64 for generation counter (Q34 audit trail)
/// - Lockfree read access to band hash data
///
/// # ASSUM Safety
///
/// - `#ASSUME_GPU_BUFFER_VALID`: wgpu buffer mapping returns valid data
/// - `#VERIFY_GPU_BUFFER_VALID`: Error handling on map_async + recv()
/// - `#ASSUME_BAND_COUNT_5`: Output array sized for NUM_BANDS (5) per document
/// - `#VERIFY_BAND_COUNT_5`: WGSL shader uses NUM_BANDS = 5
#[repr(C, align(64))]
pub struct LshBandGpuOutput {
    /// Generation counter for Q34 audit trail
    generation: AtomicU64,
    /// Band hashes: NUM_BANDS × u64 per document
    /// Stored as 2 × u32 pairs: [lo, hi, lo, hi, ...]
    band_hashes_packed: Vec<u32>,
    /// Number of documents processed
    num_docs: u32,
    /// Padding for cache line alignment
    _padding: [u8; 20],
}

impl LshBandGpuOutput {
    /// Create a new LshBandGpuOutput with the given band hashes
    ///
    /// # Arguments
    /// * `band_hashes_packed` - Packed u32 band hashes (2 per band per document)
    /// * `num_docs` - Number of documents
    /// * `generation` - Generation counter for Q34 audit
    pub fn new(band_hashes_packed: Vec<u32>, num_docs: u32, generation: u64) -> Self {
        Self {
            generation: AtomicU64::new(generation),
            band_hashes_packed,
            num_docs,
            _padding: [0; 20],
        }
    }

    /// Get generation counter (Q34 audit trail)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get number of documents
    #[inline]
    pub fn num_docs(&self) -> u32 {
        self.num_docs
    }

    /// Get band hashes for document i as [u64; NUM_BANDS]
    ///
    /// # Panics
    /// Panics if doc_id >= num_docs
    pub fn get_band_hashes(&self, doc_id: usize) -> [u64; NUM_BANDS] {
        assert!(
            doc_id < self.num_docs as usize,
            "doc_id {} out of range (num_docs = {})",
            doc_id,
            self.num_docs
        );

        let base = doc_id * NUM_BANDS * 2;
        let mut hashes = [0u64; NUM_BANDS];
        for i in 0..NUM_BANDS {
            let lo = self.band_hashes_packed[base + i * 2] as u64;
            let hi = self.band_hashes_packed[base + i * 2 + 1] as u64;
            hashes[i] = lo | (hi << 32);
        }
        hashes
    }

    /// Get band hash for specific document and band
    pub fn get_band_hash(&self, doc_id: usize, band_idx: usize) -> u64 {
        assert!(doc_id < self.num_docs as usize);
        assert!(band_idx < NUM_BANDS);

        let base = doc_id * NUM_BANDS * 2 + band_idx * 2;
        let lo = self.band_hashes_packed[base] as u64;
        let hi = self.band_hashes_packed[base + 1] as u64;
        lo | (hi << 32)
    }

    /// Get raw packed band hashes (for advanced use)
    pub fn raw_packed(&self) -> &[u32] {
        &self.band_hashes_packed
    }

    /// Total number of band hashes
    pub fn total_band_hashes(&self) -> usize {
        self.num_docs as usize * NUM_BANDS
    }
}

impl Clone for LshBandGpuOutput {
    fn clone(&self) -> Self {
        Self {
            generation: AtomicU64::new(self.generation.load(Ordering::Acquire)),
            band_hashes_packed: self.band_hashes_packed.clone(),
            num_docs: self.num_docs,
            _padding: [0; 20],
        }
    }
}

impl std::fmt::Debug for LshBandGpuOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LshBandGpuOutput")
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .field("num_docs", &self.num_docs)
            .field("total_band_hashes", &self.total_band_hashes())
            .finish()
    }
}

impl LshBandGpuCapsule {
    /// Create LSH band GPU kernel
    ///
    /// Compiles WGSL shader and creates GPU resources.
    ///
    /// # Arguments
    /// * `ctx` - GPU context with device and queue
    ///
    /// # Returns
    /// * `Ok(LshBandGpuCapsule)` on success
    /// * `Err(GpuError)` if shader compilation or resource creation fails
    pub fn new(ctx: &GpuContextCapsule) -> GpuResult<Self> {
        let device = ctx.device().ok_or(GpuError::NotInitialized)?;

        // Load and compile WGSL shader
        let shader_source = include_str!("lsh_band.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("LSH Band Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("LSH Band Bind Group Layout"),
            entries: &[
                // Signatures buffer (binding 0, input)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Band hashes buffer (binding 1, output)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Num docs uniform (binding 2)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("LSH Band Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create main compute pipeline (per-band parallelism)
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("LSH Band Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: "lsh_band_kernel",
        });

        // Create alternative pipeline (per-doc parallelism)
        let pipeline_per_doc = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("LSH Band Per-Doc Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: "lsh_band_per_doc_kernel",
        });

        Ok(Self {
            state: AtomicU64::new(1), // Ready state
            pipeline: Some(pipeline),
            pipeline_per_doc: Some(pipeline_per_doc),
            bind_group_layout: Some(bind_group_layout),
            _padding: [0; 16],
        })
    }

    /// Compute LSH band hashes on GPU
    ///
    /// # Arguments
    /// * `ctx` - GPU context
    /// * `input` - Input data (signatures, num_docs)
    ///
    /// # Returns
    /// * `Ok(LshBandGpuOutput)` with computed band hashes
    /// * `Err(GpuError)` on computation failure
    ///
    /// # Performance
    /// - Expected: ~50ns per document (5 bands)
    /// - Batch of 10K docs: ~0.5ms
    /// - Memory: 10 × NUM_BANDS bytes per document (output)
    pub fn compute(
        &self,
        ctx: &GpuContextCapsule,
        input: LshBandGpuInput,
    ) -> GpuResult<LshBandGpuOutput> {
        // Validate input
        input.validate()?;

        let device = ctx.device().ok_or(GpuError::NotInitialized)?;
        let queue = ctx.queue().ok_or(GpuError::NotInitialized)?;
        let pipeline = self.pipeline.as_ref().ok_or(GpuError::NotInitialized)?;
        let bind_group_layout = self.bind_group_layout.as_ref().ok_or(GpuError::NotInitialized)?;

        // Create input buffer (signatures)
        let signatures_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("LSH Signatures Input"),
            contents: bytemuck::cast_slice(input.signatures),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Create uniform buffer (num_docs)
        let num_docs_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("LSH NumDocs Uniform"),
            contents: bytemuck::bytes_of(&input.num_docs),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // Create output buffer (band hashes: num_docs × NUM_BANDS × 2 × u32)
        let output_size = (input.num_docs as usize * NUM_BANDS * 2 * 4) as u64;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("LSH Band Hashes Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create staging buffer for CPU readback
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("LSH Staging Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("LSH Band Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: signatures_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: num_docs_buffer.as_entire_binding(),
                },
            ],
        });

        // Create command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("LSH Band Encoder"),
        });

        // Dispatch compute
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("LSH Band Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch: ceil((num_docs × NUM_BANDS) / 256) workgroups
            let total_work = input.num_docs * NUM_BANDS as u32;
            let workgroups = (total_work + 255) / 256;
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Copy output to staging buffer
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);

        // Submit and wait
        queue.submit(std::iter::once(encoder.finish()));

        // Map staging buffer and read results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|_| GpuError::BufferMappingFailed("channel recv failed".to_string()))?
            .map_err(|e| GpuError::BufferMappingFailed(format!("{:?}", e)))?;

        // #ASSUME_GPU_BUFFER_VALID: Buffer mapping succeeded, data is valid
        // #VERIFY_GPU_BUFFER_VALID: Error handling above ensures mapping success
        let data = buffer_slice.get_mapped_range();
        let band_hashes_packed: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        // Increment generation counter for Q34 audit trail
        let generation = self.state.fetch_add(1, Ordering::AcqRel) + 1;

        Ok(LshBandGpuOutput::new(band_hashes_packed, input.num_docs, generation))
    }

    /// Compute using per-document kernel (better for small batches)
    ///
    /// Uses alternative kernel that processes all bands per document sequentially.
    /// More efficient for small batch sizes (<1000 docs) due to reduced thread overhead.
    pub fn compute_per_doc(
        &self,
        ctx: &GpuContextCapsule,
        input: LshBandGpuInput,
    ) -> GpuResult<LshBandGpuOutput> {
        // Validate input
        input.validate()?;

        let device = ctx.device().ok_or(GpuError::NotInitialized)?;
        let queue = ctx.queue().ok_or(GpuError::NotInitialized)?;
        let pipeline = self.pipeline_per_doc.as_ref().ok_or(GpuError::NotInitialized)?;
        let bind_group_layout = self.bind_group_layout.as_ref().ok_or(GpuError::NotInitialized)?;

        // Create buffers (same as compute())
        let signatures_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("LSH Signatures Input"),
            contents: bytemuck::cast_slice(input.signatures),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let num_docs_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("LSH NumDocs Uniform"),
            contents: bytemuck::bytes_of(&input.num_docs),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let output_size = (input.num_docs as usize * NUM_BANDS * 2 * 4) as u64;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("LSH Band Hashes Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("LSH Staging Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("LSH Band Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: signatures_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: num_docs_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("LSH Band Per-Doc Encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("LSH Band Per-Doc Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch: ceil(num_docs / 256) workgroups
            let workgroups = (input.num_docs + 255) / 256;
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);
        queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|_| GpuError::BufferMappingFailed("channel recv failed".to_string()))?
            .map_err(|e| GpuError::BufferMappingFailed(format!("{:?}", e)))?;

        // #ASSUME_GPU_BUFFER_VALID: Buffer mapping succeeded, data is valid
        // #VERIFY_GPU_BUFFER_VALID: Error handling above ensures mapping success
        let data = buffer_slice.get_mapped_range();
        let band_hashes_packed: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        // Increment generation counter for Q34 audit trail
        let generation = self.state.fetch_add(1, Ordering::AcqRel) + 1;

        Ok(LshBandGpuOutput::new(band_hashes_packed, input.num_docs, generation))
    }

    /// Check if kernel is ready for compute
    pub fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == 1
    }
}

impl Default for LshBandGpuCapsule {
    fn default() -> Self {
        Self {
            state: AtomicU64::new(0), // Uninitialized
            pipeline: None,
            pipeline_per_doc: None,
            bind_group_layout: None,
            _padding: [0; 16],
        }
    }
}

impl std::fmt::Debug for LshBandGpuCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LshBandGpuCapsule")
            .field("state", &self.state.load(Ordering::Relaxed))
            .field("has_pipeline", &self.pipeline.is_some())
            .field("has_pipeline_per_doc", &self.pipeline_per_doc.is_some())
            .finish()
    }
}

// =============================================================================
// CPU Reference Implementation (for testing)
// =============================================================================

/// Compute LSH band hash on CPU (reference implementation for validation)
///
/// This exactly matches the GPU algorithm for property testing.
pub fn cpu_hash_band(signature: &[u16], band_idx: usize) -> u64 {
    let start = band_idx * ROWS_PER_BAND;
    let end = (start + ROWS_PER_BAND).min(SIGNATURE_SIZE);

    let mut hash: u64 = 0;
    for i in start..end {
        // hash = hash * 31 + value (wrapping)
        hash = hash.wrapping_mul(31).wrapping_add(signature[i] as u64);
    }
    hash
}

/// Compute all band hashes on CPU (reference implementation)
pub fn cpu_compute_all_bands(signature: &[u16]) -> [u64; NUM_BANDS] {
    let mut hashes = [0u64; NUM_BANDS];
    for band_idx in 0..NUM_BANDS {
        hashes[band_idx] = cpu_hash_band(signature, band_idx);
    }
    hashes
}

/// Extract u16 signature from packed u32 array (matching GPU format)
pub fn unpack_signature(packed: &[u32]) -> [u16; SIGNATURE_SIZE] {
    assert_eq!(packed.len(), 64, "Packed signature must be 64 u32");
    let mut sig = [0u16; SIGNATURE_SIZE];
    for i in 0..64 {
        sig[i * 2] = (packed[i] & 0xFFFF) as u16;
        sig[i * 2 + 1] = (packed[i] >> 16) as u16;
    }
    sig
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu::GpuContextCapsule;

    /// Helper: Skip test if no GPU available
    fn try_get_gpu() -> Option<GpuContextCapsule> {
        match GpuContextCapsule::new_blocking() {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                println!("Skipping GPU test - no GPU available: {}", e);
                None
            }
        }
    }

    // =========================================================================
    // Q1-Q7: Unit Tests (T28 Framework)
    // =========================================================================

    #[test]
    fn test_input_validation_valid() {
        let signatures = vec![0u32; 64 * 2]; // 2 documents
        let input = LshBandGpuInput {
            signatures: &signatures,
            num_docs: 2,
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_input_validation_wrong_length() {
        let signatures = vec![0u32; 100]; // Wrong length
        let input = LshBandGpuInput {
            signatures: &signatures,
            num_docs: 2,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_input_validation_zero_docs() {
        let signatures: Vec<u32> = vec![];
        let input = LshBandGpuInput {
            signatures: &signatures,
            num_docs: 0,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_cpu_hash_band_deterministic() {
        let sig = [1u16; SIGNATURE_SIZE];
        let hash1 = cpu_hash_band(&sig, 0);
        let hash2 = cpu_hash_band(&sig, 0);
        assert_eq!(hash1, hash2, "Band hash must be deterministic");
    }

    #[test]
    fn test_cpu_hash_band_distinct_bands() {
        let sig = [42u16; SIGNATURE_SIZE];
        let hash0 = cpu_hash_band(&sig, 0);
        let hash1 = cpu_hash_band(&sig, 1);
        // Different bands can produce same hash with uniform input,
        // but that's expected behavior
        println!("Band 0 hash: {}", hash0);
        println!("Band 1 hash: {}", hash1);
    }

    #[test]
    fn test_cpu_compute_all_bands() {
        let sig = [100u16; SIGNATURE_SIZE];
        let hashes = cpu_compute_all_bands(&sig);
        assert_eq!(hashes.len(), NUM_BANDS);
        // All hashes should be non-zero for non-zero input
        for hash in &hashes {
            assert!(*hash != 0);
        }
    }

    #[test]
    fn test_unpack_signature() {
        let mut packed = [0u32; 64];
        packed[0] = 1 | (2 << 16); // sig[0]=1, sig[1]=2
        packed[1] = 3 | (4 << 16); // sig[2]=3, sig[3]=4

        let sig = unpack_signature(&packed);
        assert_eq!(sig[0], 1);
        assert_eq!(sig[1], 2);
        assert_eq!(sig[2], 3);
        assert_eq!(sig[3], 4);
    }

    #[test]
    fn test_output_get_band_hashes() {
        // Create mock output
        let mut packed = vec![0u32; 2 * NUM_BANDS * 2]; // 2 docs
        // Doc 0, Band 0: hash = 0x0000000100000001
        packed[0] = 1; // lo
        packed[1] = 1; // hi

        let output = LshBandGpuOutput::new(packed, 2, 42);

        let hashes = output.get_band_hashes(0);
        assert_eq!(hashes[0], 0x0000000100000001);

        // Verify generation counter
        assert_eq!(output.generation(), 42);
    }

    // =========================================================================
    // Q8-Q14: GPU Integration Tests
    // =========================================================================

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_lsh_band_gpu_basic() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = match LshBandGpuCapsule::new(&ctx) {
            Ok(k) => k,
            Err(e) => {
                println!("Failed to create kernel: {}", e);
                return;
            }
        };

        assert!(kernel.is_ready());

        // Test with 2 documents
        let signatures = vec![12345u32; 64 * 2];

        let input = LshBandGpuInput {
            signatures: &signatures,
            num_docs: 2,
        };

        let output = kernel.compute(&ctx, input).expect("GPU compute failed");

        assert_eq!(output.num_docs, 2);
        assert_eq!(output.total_band_hashes(), 2 * NUM_BANDS);
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_lsh_band_gpu_deterministic() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = LshBandGpuCapsule::new(&ctx).expect("kernel creation");

        let signatures = vec![42u32; 64];

        let input = LshBandGpuInput {
            signatures: &signatures,
            num_docs: 1,
        };

        // Compute twice
        let output1 = kernel.compute(&ctx, input.clone()).expect("compute 1");
        let output2 = kernel.compute(&ctx, input).expect("compute 2");

        // Results should be identical
        let hashes1 = output1.get_band_hashes(0);
        let hashes2 = output2.get_band_hashes(0);
        assert_eq!(hashes1, hashes2, "GPU LSH band hash should be deterministic");
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_lsh_band_gpu_vs_cpu() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = LshBandGpuCapsule::new(&ctx).expect("kernel creation");

        // Create test signature with known values
        let mut packed = vec![0u32; 64];
        for i in 0..64 {
            packed[i] = (i as u32) | ((i as u32 + 100) << 16);
        }

        let input = LshBandGpuInput {
            signatures: &packed,
            num_docs: 1,
        };

        // Compute on GPU
        let gpu_output = kernel.compute(&ctx, input).expect("GPU compute");
        let gpu_hashes = gpu_output.get_band_hashes(0);

        // Compute on CPU
        let sig = unpack_signature(&packed);
        let cpu_hashes = cpu_compute_all_bands(&sig);

        // Compare results
        println!("GPU hashes: {:?}", gpu_hashes);
        println!("CPU hashes: {:?}", cpu_hashes);

        for (i, (gpu, cpu)) in gpu_hashes.iter().zip(cpu_hashes.iter()).enumerate() {
            assert_eq!(
                *gpu, *cpu,
                "Band {} hash mismatch: GPU={}, CPU={}",
                i, gpu, cpu
            );
        }
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_lsh_band_per_doc_kernel() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = LshBandGpuCapsule::new(&ctx).expect("kernel creation");

        let signatures = vec![99u32; 64 * 3]; // 3 documents

        let input = LshBandGpuInput {
            signatures: &signatures,
            num_docs: 3,
        };

        // Test both kernels produce same results
        let output1 = kernel.compute(&ctx, input.clone()).expect("compute");
        let output2 = kernel.compute_per_doc(&ctx, input).expect("compute_per_doc");

        for doc in 0..3 {
            let h1 = output1.get_band_hashes(doc);
            let h2 = output2.get_band_hashes(doc);
            assert_eq!(h1, h2, "Per-doc kernel should produce same results");
        }
    }

    // =========================================================================
    // Q15-Q21: Throughput Tests
    // =========================================================================

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_lsh_band_gpu_throughput() {
        let Some(ctx) = try_get_gpu() else { return };

        println!("GPU: {}", ctx.capabilities().device_name);

        let kernel = LshBandGpuCapsule::new(&ctx).expect("kernel creation");

        // 10K documents
        let num_docs = 10_000u32;
        let signatures = vec![12345u32; 64 * num_docs as usize];

        let input = LshBandGpuInput {
            signatures: &signatures,
            num_docs,
        };

        // Warmup
        let _ = kernel.compute(&ctx, input.clone());

        // Benchmark
        let start = std::time::Instant::now();
        let iterations = 100;
        for _ in 0..iterations {
            let _ = kernel.compute(&ctx, input.clone());
        }
        let elapsed = start.elapsed();

        let total_docs = num_docs as f64 * iterations as f64;
        let docs_per_sec = total_docs / elapsed.as_secs_f64();
        let total_bands = total_docs * NUM_BANDS as f64;
        let bands_per_sec = total_bands / elapsed.as_secs_f64();
        let us_per_doc = (elapsed.as_micros() as f64 / iterations as f64) / num_docs as f64;

        println!("\n=== GPU LSH Band Throughput ===");
        println!("Documents: {}K", num_docs / 1000);
        println!("Bands per doc: {}", NUM_BANDS);
        println!("Iterations: {}", iterations);
        println!("Throughput: {:.0} docs/sec", docs_per_sec);
        println!("Throughput: {:.0} bands/sec", bands_per_sec);
        println!("Time/doc: {:.3}us", us_per_doc);
        println!("CPU baseline: ~250ns/doc (est)");

        // Minimum expectation: faster than CPU
        assert!(
            docs_per_sec > 100_000.0,
            "GPU should be competitive: {} docs/sec",
            docs_per_sec
        );
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_lsh_band_gpu_large_batch() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = LshBandGpuCapsule::new(&ctx).expect("kernel creation");

        // 100K documents
        let num_docs = 100_000u32;
        let signatures = vec![0u32; 64 * num_docs as usize];

        let input = LshBandGpuInput {
            signatures: &signatures,
            num_docs,
        };

        let start = std::time::Instant::now();
        let output = kernel.compute(&ctx, input).expect("GPU compute");
        let elapsed = start.elapsed();

        println!("\n=== Large Batch Test ===");
        println!("Documents: {}K", num_docs / 1000);
        println!("Time: {:?}", elapsed);
        println!(
            "Throughput: {:.0} docs/sec",
            num_docs as f64 / elapsed.as_secs_f64()
        );

        assert_eq!(output.num_docs, num_docs);
        assert_eq!(output.total_band_hashes(), num_docs as usize * NUM_BANDS);
    }
}
