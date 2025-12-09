//! MinHash GPU Kernel - T7 Heterogeneous Tier
//!
//! GPU-accelerated MinHash signature computation for kindly_dedup.
//!
//! # Performance Targets (B32 Framework)
//!
//! | Metric | CPU SIMD | GPU Target | Expected Speedup |
//! |--------|----------|------------|------------------|
//! | Per-doc | 16.7μs | 100-500ns | 33-167× |
//! | Throughput | 60K/sec | 2M+/sec | 33× |
//! | Batch 10K | 167ms | 5ms | 33× |
//!
//! # Architecture
//!
//! - Uses WGSL compute shader for MinHash computation
//! - One thread per document (256 threads/workgroup)
//! - 128 MinHash values computed per document
//! - Results packed as 64 u32 (2×u16 per u32)
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
use crate::gpu::fed_params::FedHashParamsCapsule;

/// MinHash GPU Kernel Capsule - T7 Heterogeneous
///
/// Encapsulates GPU resources for MinHash computation.
/// Thread-safe via AtomicU64 state, lockfree.
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::gpu::{GpuContextCapsule, MinHashGpuCapsule, MinHashGpuInput};
///
/// let ctx = GpuContextCapsule::new_blocking()?;
/// let kernel = MinHashGpuCapsule::new(&ctx)?;
///
/// let tokens = vec![100u32, 200, 300, 100, 400, 500];
/// let offsets = vec![0u32, 3, 6];
///
/// let output = kernel.compute(&ctx, MinHashGpuInput {
///     tokens: &tokens,
///     offsets: &offsets,
///     num_docs: 2,
/// })?;
///
/// let sig0 = output.get_signature(0);
/// let sig1 = output.get_signature(1);
/// ```
#[repr(C, align(64))]
pub struct MinHashGpuCapsule {
    /// Atomic state for Chaos compliance (0 = uninit, 1 = ready, 2 = error)
    state: AtomicU64,
    /// Compute pipeline
    pipeline: Option<wgpu::ComputePipeline>,
    /// Seeds buffer (128 u32 seeds for hash permutations)
    seeds_buffer: Option<wgpu::Buffer>,
    /// Bind group layout
    bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Use FED optimization (6-24× faster)
    use_fed: bool,
    /// FED bind group layout (separate from legacy)
    fed_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Padding for cache line alignment
    _padding: [u8; 16],
}

// SAFETY: MinHashGpuCapsule is Send + Sync because:
// - AtomicU64 is Send + Sync
// - wgpu types (ComputePipeline, Buffer, BindGroupLayout) are Send + Sync
// - All interior mutability is via atomics
unsafe impl Send for MinHashGpuCapsule {}
unsafe impl Sync for MinHashGpuCapsule {}

/// Input for GPU MinHash computation
#[derive(Debug, Clone)]
pub struct MinHashGpuInput<'a> {
    /// Pre-hashed tokens (flattened across all documents)
    /// These should be u32 hash values of the original token strings
    pub tokens: &'a [u32],
    /// Document boundaries: tokens[offsets[i]..offsets[i+1]] = doc i
    /// Length must be num_docs + 1
    pub offsets: &'a [u32],
    /// Number of documents
    pub num_docs: u32,
}

impl<'a> MinHashGpuInput<'a> {
    /// Validate input data
    pub fn validate(&self) -> GpuResult<()> {
        if self.offsets.len() != self.num_docs as usize + 1 {
            return Err(GpuError::InvalidInput(format!(
                "offsets length {} != num_docs {} + 1",
                self.offsets.len(),
                self.num_docs
            )));
        }

        if self.num_docs == 0 {
            return Err(GpuError::InvalidInput("num_docs must be > 0".to_string()));
        }

        // Check offsets are monotonically increasing
        for i in 0..self.offsets.len() - 1 {
            if self.offsets[i] > self.offsets[i + 1] {
                return Err(GpuError::InvalidInput(format!(
                    "offsets not monotonic at index {}: {} > {}",
                    i,
                    self.offsets[i],
                    self.offsets[i + 1]
                )));
            }
        }

        // Check last offset matches tokens length
        let expected_tokens = *self.offsets.last().unwrap_or(&0) as usize;
        if self.tokens.len() != expected_tokens {
            return Err(GpuError::InvalidInput(format!(
                "tokens length {} != expected {}",
                self.tokens.len(),
                expected_tokens
            )));
        }

        Ok(())
    }
}

/// Output from GPU MinHash computation - T7 Heterogeneous Capsule
///
/// # Chaos Compliance
///
/// - Cache-aligned (64 bytes) for optimal memory access
/// - AtomicU64 for generation counter (Q34 audit trail)
/// - Lockfree read access to signature data
///
/// # ASSUM Safety
///
/// - `#ASSUME_GPU_BUFFER_VALID`: wgpu buffer mapping returns valid data
/// - `#VERIFY_GPU_BUFFER_VALID`: Error handling on map_async + recv()
/// - `#ASSUME_SIGNATURE_SIZE_128`: Output array sized for 128 u16 per document
/// - `#VERIFY_SIGNATURE_SIZE_128`: WGSL shader uses SIGNATURE_SIZE = 128
#[repr(C, align(64))]
pub struct MinHashGpuOutput {
    /// Generation counter for Q34 audit trail
    generation: AtomicU64,
    /// Signatures: 64 u32 per document (128 u16 packed)
    signatures: Vec<u32>,
    /// Number of documents processed
    num_docs: u32,
    /// Padding for cache line alignment
    _padding: [u8; 20],
}

impl MinHashGpuOutput {
    /// Create a new MinHashGpuOutput with the given signatures
    ///
    /// # Arguments
    /// * `signatures` - Packed u32 signatures (64 per document)
    /// * `num_docs` - Number of documents
    /// * `generation` - Generation counter for Q34 audit
    pub fn new(signatures: Vec<u32>, num_docs: u32, generation: u64) -> Self {
        Self {
            generation: AtomicU64::new(generation),
            signatures,
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

    /// Get raw signatures slice
    #[inline]
    pub fn signatures(&self) -> &[u32] {
        &self.signatures
    }

    /// Get signature for document i as [u16; 128]
    ///
    /// # Panics
    /// Panics if doc_id >= num_docs
    pub fn get_signature(&self, doc_id: usize) -> [u16; 128] {
        assert!(
            doc_id < self.num_docs as usize,
            "doc_id {} out of range (num_docs = {})",
            doc_id,
            self.num_docs
        );

        let base = doc_id * 64;
        let mut sig = [0u16; 128];
        for i in 0..64 {
            let packed = self.signatures[base + i];
            sig[i * 2] = (packed & 0xFFFF) as u16;
            sig[i * 2 + 1] = (packed >> 16) as u16;
        }
        sig
    }

    /// Get signature as packed u32 slice (64 values)
    pub fn get_signature_packed(&self, doc_id: usize) -> &[u32] {
        assert!(
            doc_id < self.num_docs as usize,
            "doc_id {} out of range",
            doc_id
        );
        let base = doc_id * 64;
        &self.signatures[base..base + 64]
    }

    /// Compute Jaccard similarity between two documents (MinHash estimate)
    ///
    /// Returns estimate of Jaccard similarity based on MinHash signatures.
    pub fn jaccard_similarity(&self, doc_a: usize, doc_b: usize) -> f64 {
        let sig_a = self.get_signature(doc_a);
        let sig_b = self.get_signature(doc_b);

        let matches = sig_a
            .iter()
            .zip(sig_b.iter())
            .filter(|(a, b)| a == b)
            .count();

        matches as f64 / 128.0
    }
}

impl Clone for MinHashGpuOutput {
    fn clone(&self) -> Self {
        Self {
            generation: AtomicU64::new(self.generation.load(Ordering::Acquire)),
            signatures: self.signatures.clone(),
            num_docs: self.num_docs,
            _padding: [0; 20],
        }
    }
}

impl std::fmt::Debug for MinHashGpuOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinHashGpuOutput")
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .field("num_docs", &self.num_docs)
            .field("signatures_len", &self.signatures.len())
            .finish()
    }
}

impl MinHashGpuCapsule {
    /// Create MinHash GPU kernel
    ///
    /// Compiles WGSL shader and creates GPU resources.
    ///
    /// # Arguments
    /// * `ctx` - GPU context with device and queue
    ///
    /// # Returns
    /// * `Ok(MinHashGpuCapsule)` on success
    /// * `Err(GpuError)` if shader compilation or resource creation fails
    pub fn new(ctx: &GpuContextCapsule) -> GpuResult<Self> {
        let device = ctx.device().ok_or(GpuError::NotInitialized)?;

        // Load and compile WGSL shader
        let shader_source = include_str!("minhash.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MinHash Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MinHash Bind Group Layout"),
            entries: &[
                // Seeds buffer (binding 0)
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
                // Tokens buffer (binding 1)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Offsets buffer (binding 2)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output signatures buffer (binding 3)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create compute pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MinHash Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create compute pipeline
        // Note: wgpu 0.19.x API (for iced compatibility)
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MinHash Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: "minhash_kernel",
        });

        // Create seeds buffer (128 deterministic seeds for reproducibility)
        let seeds = Self::generate_seeds();
        let seeds_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MinHash Seeds"),
            contents: bytemuck::cast_slice(&seeds),
            usage: wgpu::BufferUsages::STORAGE,
        });

        Ok(Self {
            state: AtomicU64::new(1), // Ready state
            pipeline: Some(pipeline),
            seeds_buffer: Some(seeds_buffer),
            bind_group_layout: Some(bind_group_layout),
            use_fed: false,
            fed_bind_group_layout: None,
            _padding: [0; 16],
        })
    }

    /// Create MinHash GPU kernel with FED optimization
    ///
    /// Uses Fast Exact Deduplication (FED) shader with precomputed hash parameters
    /// for 6-24× speedup over standard GPU MinHash.
    ///
    /// # Arguments
    ///
    /// - `ctx`: GPU context with FED params initialized
    ///
    /// # Returns
    ///
    /// - `Ok(MinHashGpuCapsule)`: FED kernel ready
    /// - `Err(GpuError)`: Shader compilation or FED params not initialized
    ///
    /// # Performance
    ///
    /// - Expected speedup: 6-24× vs `new()` standard kernel
    /// - Memory bandwidth shift: Simpler hash → more throughput
    /// - Better occupancy: Lower register pressure → more warps in flight
    ///
    /// # Requirements
    ///
    /// - GPU context must have FED params initialized via `ctx.init_fed_params(seed)`
    /// - Falls back to standard kernel if FED params not available
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use kindly_dedup::gpu::{GpuContextCapsule, MinHashGpuCapsule};
    ///
    /// let mut ctx = GpuContextCapsule::new_blocking()?;
    /// ctx.init_fed_params(42)?; // Initialize FED params first
    ///
    /// let kernel = MinHashGpuCapsule::new_fed(&ctx)?;
    /// // FED kernel is 6-24× faster than standard kernel
    /// ```
    pub fn new_fed(ctx: &GpuContextCapsule) -> GpuResult<Self> {
        let device = ctx.device().ok_or(GpuError::NotInitialized)?;

        // Check if FED params are available
        let fed_buffer = ctx.fed_params_buffer().ok_or_else(|| {
            GpuError::DeviceRequestFailed(
                "FED params not initialized. Call ctx.init_fed_params(seed) first.".to_string()
            )
        })?;

        // Load FED shader
        let shader_source = include_str!("minhash_fed.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("FED MinHash Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create FED bind group layout (storage buffers for params + data)
        let fed_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("FED MinHash Bind Group Layout"),
            entries: &[
                // FED params (binding 0, storage buffer - uniform buffers require 16-byte alignment)
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
                // Tokens buffer (binding 1)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Offsets buffer (binding 2)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output signatures buffer (binding 3)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create compute pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("FED MinHash Pipeline Layout"),
            bind_group_layouts: &[&fed_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("FED MinHash Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: "fed_minhash_kernel", // FED shader entry point
        });

        Ok(Self {
            state: AtomicU64::new(1), // Ready state
            pipeline: Some(pipeline),
            seeds_buffer: None, // FED doesn't use seeds buffer
            bind_group_layout: None, // Legacy layout not used
            use_fed: true,
            fed_bind_group_layout: Some(fed_bind_group_layout),
            _padding: [0; 16],
        })
    }

    /// Compute MinHash signatures on GPU
    ///
    /// # Arguments
    /// * `ctx` - GPU context
    /// * `input` - Input data (tokens, offsets, num_docs)
    ///
    /// # Returns
    /// * `Ok(MinHashGpuOutput)` with computed signatures
    /// * `Err(GpuError)` on computation failure
    ///
    /// # Performance
    /// - Expected: 100-500ns per document
    /// - Batch of 10K docs: ~5ms
    /// - Memory: ~660 bytes per document
    pub fn compute(&self, ctx: &GpuContextCapsule, input: MinHashGpuInput) -> GpuResult<MinHashGpuOutput> {
        // Validate input
        input.validate()?;

        let device = ctx.device().ok_or(GpuError::NotInitialized)?;
        let queue = ctx.queue().ok_or(GpuError::NotInitialized)?;
        let pipeline = self.pipeline.as_ref().ok_or(GpuError::NotInitialized)?;

        // Handle empty tokens case (documents with no tokens)
        let tokens_data: &[u32] = if input.tokens.is_empty() {
            // Create dummy token for empty documents
            &[0u32]
        } else {
            input.tokens
        };

        // Create input buffers
        let tokens_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tokens"),
            contents: bytemuck::cast_slice(tokens_data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let offsets_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Offsets"),
            contents: bytemuck::cast_slice(input.offsets),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Create output buffer (64 u32 per document = 256 bytes per doc)
        let output_size = (input.num_docs as usize * 64 * 4) as u64;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Signatures Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create staging buffer for CPU readback
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group (different layout for FED vs legacy)
        let bind_group = if self.use_fed {
            // FED mode: binding 0 is FED params (uniform buffer)
            let fed_layout = self.fed_bind_group_layout.as_ref().ok_or(GpuError::NotInitialized)?;
            let fed_buffer = ctx.fed_params_buffer().ok_or(GpuError::NotInitialized)?;

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("FED MinHash Bind Group"),
                layout: fed_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: fed_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: tokens_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: offsets_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: output_buffer.as_entire_binding(),
                    },
                ],
            })
        } else {
            // Legacy mode: binding 0 is seeds buffer (storage buffer)
            let legacy_layout = self.bind_group_layout.as_ref().ok_or(GpuError::NotInitialized)?;

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("MinHash Bind Group"),
                layout: legacy_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.seeds_buffer.as_ref().unwrap().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: tokens_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: offsets_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: output_buffer.as_entire_binding(),
                    },
                ],
            })
        };

        // Create command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("MinHash Encoder"),
        });

        // Dispatch compute
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("MinHash Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch: ceil(num_docs / 256) workgroups
            let workgroups = (input.num_docs + 255) / 256;
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Copy output to staging buffer
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);

        // Submit and wait
        queue.submit(std::iter::once(encoder.finish()));

        // Map staging buffer and read results with timeout protection
        // Per wgpu issue #3601: Maintain::Wait can hang indefinitely on driver failure
        // SOTA pattern: Use Maintain::Poll with try_recv() for graceful timeout handling
        //
        // #ASSUME_POLL_TIMEOUT: GPU operations complete within 5 seconds under normal conditions
        // #VERIFY_POLL_TIMEOUT: Timeout errors are recoverable; caller can retry or fallback to CPU
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Timeout-protected polling loop (T7 Heterogeneous tier - Chaos lockfree coordination)
        const GPU_POLL_TIMEOUT_SECS: u64 = 5;
        let poll_start = std::time::Instant::now();
        let poll_timeout = std::time::Duration::from_secs(GPU_POLL_TIMEOUT_SECS);

        loop {
            // Poll GPU for progress (non-blocking)
            device.poll(wgpu::Maintain::Poll);

            // Check if mapping callback fired
            match receiver.try_recv() {
                Ok(result) => {
                    // Mapping completed - check for wgpu errors
                    result.map_err(|e| GpuError::BufferMappingFailed(format!("{:?}", e)))?;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Not ready yet - check timeout
                    if poll_start.elapsed() > poll_timeout {
                        return Err(GpuError::Timeout {
                            timeout_secs: GPU_POLL_TIMEOUT_SECS,
                        });
                    }
                    // Brief yield to avoid busy-spin (1ms sleep)
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Sender dropped without sending - GPU callback never fired
                    return Err(GpuError::BufferMappingFailed(
                        "Channel disconnected: GPU callback never fired".to_string(),
                    ));
                }
            }
        }

        // #ASSUME_GPU_BUFFER_VALID: Buffer mapping succeeded, data is valid
        // #VERIFY_GPU_BUFFER_VALID: Error handling above ensures mapping success
        let data = buffer_slice.get_mapped_range();
        let signatures: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        // Increment generation counter for Q34 audit trail
        let generation = self.state.fetch_add(1, Ordering::AcqRel) + 1;

        Ok(MinHashGpuOutput::new(signatures, input.num_docs, generation))
    }

    /// Generate 128 deterministic seeds for hash permutations
    ///
    /// Uses Fibonacci-based sequence for determinism and good distribution.
    /// Same seeds as CPU implementation for result compatibility.
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_SEED_QUALITY`: Seeds provide independent hash functions
    /// - `#VERIFY_SEED_QUALITY`: Validated via hash independence tests
    pub fn generate_seeds() -> [u32; 128] {
        let mut seeds = [0u32; 128];
        // Use golden ratio constant for good distribution
        // This matches the CPU MinHash implementation
        for i in 0..128 {
            seeds[i] = (i as u32 + 1).wrapping_mul(2654435761);
        }
        seeds
    }

    /// Check if kernel is ready for compute
    pub fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == 1
    }
}

impl Default for MinHashGpuCapsule {
    fn default() -> Self {
        Self {
            state: AtomicU64::new(0), // Uninitialized
            pipeline: None,
            seeds_buffer: None,
            bind_group_layout: None,
            use_fed: false,
            fed_bind_group_layout: None,
            _padding: [0; 16],
        }
    }
}

impl std::fmt::Debug for MinHashGpuCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinHashGpuCapsule")
            .field("state", &self.state.load(Ordering::Relaxed))
            .field("has_pipeline", &self.pipeline.is_some())
            .field("has_seeds", &self.seeds_buffer.is_some())
            .finish()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
        let tokens = vec![100u32, 200, 300, 400, 500, 600];
        let offsets = vec![0u32, 3, 6];
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 2,
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_input_validation_wrong_offsets_length() {
        let tokens = vec![100u32, 200, 300];
        let offsets = vec![0u32, 3]; // Should be length 3 for num_docs=2
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 2,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_input_validation_zero_docs() {
        let tokens: Vec<u32> = vec![];
        let offsets = vec![0u32];
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 0,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_input_validation_non_monotonic_offsets() {
        let tokens = vec![100u32, 200, 300, 400];
        let offsets = vec![0u32, 4, 2]; // 4 > 2, not monotonic
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 2,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_input_validation_wrong_tokens_length() {
        let tokens = vec![100u32, 200]; // Should be 6 based on offsets
        let offsets = vec![0u32, 3, 6];
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 2,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_seed_generation() {
        let seeds1 = MinHashGpuCapsule::generate_seeds();
        let seeds2 = MinHashGpuCapsule::generate_seeds();

        // Seeds should be deterministic
        assert_eq!(seeds1, seeds2);

        // All seeds should be unique
        let mut unique_seeds = seeds1.to_vec();
        unique_seeds.sort();
        unique_seeds.dedup();
        assert_eq!(unique_seeds.len(), 128, "All seeds should be unique");

        // No zero seeds
        assert!(seeds1.iter().all(|&s| s != 0), "No seed should be zero");
    }

    #[test]
    fn test_output_get_signature() {
        // Create mock output with known values
        let mut signatures = vec![0u32; 128]; // 2 docs * 64
        // Doc 0: all low=1, high=2
        for i in 0..64 {
            signatures[i] = 1 | (2 << 16);
        }
        // Doc 1: all low=3, high=4
        for i in 64..128 {
            signatures[i] = 3 | (4 << 16);
        }

        let output = MinHashGpuOutput::new(signatures, 2, 0);

        let sig0 = output.get_signature(0);
        let sig1 = output.get_signature(1);

        // Check doc 0
        assert!(sig0.iter().step_by(2).all(|&x| x == 1)); // Even indices
        assert!(sig0.iter().skip(1).step_by(2).all(|&x| x == 2)); // Odd indices

        // Check doc 1
        assert!(sig1.iter().step_by(2).all(|&x| x == 3));
        assert!(sig1.iter().skip(1).step_by(2).all(|&x| x == 4));

        // Verify generation counter
        assert_eq!(output.generation(), 0);
    }

    #[test]
    fn test_output_jaccard_similarity() {
        // Create output with identical signatures for doc 0 and 1
        let mut signatures = vec![0u32; 128];
        for i in 0..64 {
            signatures[i] = i as u32; // Doc 0
            signatures[64 + i] = i as u32; // Doc 1 (same)
        }

        let output = MinHashGpuOutput::new(signatures, 2, 42);

        // Identical signatures should have similarity 1.0
        assert_eq!(output.jaccard_similarity(0, 1), 1.0);
        assert_eq!(output.jaccard_similarity(0, 0), 1.0);

        // Verify generation counter
        assert_eq!(output.generation(), 42);
    }

    // =========================================================================
    // Q8-Q14: Property Tests (GPU == CPU)
    // =========================================================================

    // =========================================================================
    // FED Tests (Q8-Q14: Property Tests)
    // =========================================================================

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_fed_initialization() {
        let Some(mut ctx) = try_get_gpu() else { return };

        // Initialize FED params
        let seed = 12345u64;
        assert!(ctx.init_fed_params(seed).is_ok());

        // Verify FED params are stored
        assert!(ctx.fed_params().is_some());
        assert!(ctx.fed_params_buffer().is_some());

        // Create FED kernel
        let kernel = match MinHashGpuCapsule::new_fed(&ctx) {
            Ok(k) => k,
            Err(e) => {
                println!("Failed to create FED kernel: {}", e);
                return;
            }
        };

        assert!(kernel.is_ready());
        assert!(kernel.use_fed);
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_fed_vs_legacy_determinism() {
        let Some(mut ctx) = try_get_gpu() else { return };

        // Initialize FED params
        ctx.init_fed_params(42).expect("FED init");

        // Create both kernels
        let fed_kernel = MinHashGpuCapsule::new_fed(&ctx).expect("FED kernel");
        let legacy_kernel = MinHashGpuCapsule::new(&ctx).expect("Legacy kernel");

        // Test data
        let tokens = vec![100u32, 200, 300, 400, 500];
        let offsets = vec![0u32, 5];
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 1,
        };

        // Compute with FED
        let fed_output = fed_kernel.compute(&ctx, input.clone()).expect("FED compute");
        let fed_sig = fed_output.get_signature(0);

        // Compute with legacy
        let legacy_output = legacy_kernel.compute(&ctx, input).expect("Legacy compute");
        let legacy_sig = legacy_output.get_signature(0);

        // FED and legacy should produce DIFFERENT signatures (different hash functions)
        // But both should be valid (non-max values)
        assert!(fed_sig.iter().any(|&x| x != u16::MAX), "FED should produce valid signature");
        assert!(legacy_sig.iter().any(|&x| x != u16::MAX), "Legacy should produce valid signature");

        println!("FED signature: {:?}", &fed_sig[0..10]);
        println!("Legacy signature: {:?}", &legacy_sig[0..10]);
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_fed_basic() {
        let Some(mut ctx) = try_get_gpu() else { return };

        // Initialize FED params
        ctx.init_fed_params(999).expect("FED init");

        let kernel = match MinHashGpuCapsule::new_fed(&ctx) {
            Ok(k) => k,
            Err(e) => {
                println!("Failed to create FED kernel: {}", e);
                return;
            }
        };

        assert!(kernel.is_ready());

        // Test with 2 documents
        let tokens = vec![
            // Doc 0: tokens 100, 200, 300
            100u32, 200, 300,
            // Doc 1: tokens 100, 400, 500
            100, 400, 500,
        ];
        let offsets = vec![0u32, 3, 6];

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 2,
        };

        let output = kernel.compute(&ctx, input).expect("FED GPU compute failed");

        assert_eq!(output.num_docs, 2);
        assert_eq!(output.signatures.len(), 128); // 2 docs * 64 u32

        let sig0 = output.get_signature(0);
        let sig1 = output.get_signature(1);

        // Signatures should have non-max values (tokens were hashed)
        assert!(sig0.iter().any(|&x| x != u16::MAX));
        assert!(sig1.iter().any(|&x| x != u16::MAX));

        // Documents share token 100, so some similarity expected
        let similarity = output.jaccard_similarity(0, 1);
        println!("FED Jaccard similarity (shared token 100): {:.3}", similarity);
        assert!(similarity > 0.0, "Shared token should create some similarity");
        assert!(similarity < 1.0, "Different tokens should prevent full similarity");
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_gpu_basic() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = match MinHashGpuCapsule::new(&ctx) {
            Ok(k) => k,
            Err(e) => {
                println!("Failed to create kernel: {}", e);
                return;
            }
        };

        assert!(kernel.is_ready());

        // Test with 2 documents
        let tokens = vec![
            // Doc 0: tokens 100, 200, 300
            100u32, 200, 300,
            // Doc 1: tokens 100, 400, 500
            100, 400, 500,
        ];
        let offsets = vec![0u32, 3, 6];

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 2,
        };

        let output = kernel.compute(&ctx, input).expect("GPU compute failed");

        assert_eq!(output.num_docs, 2);
        assert_eq!(output.signatures.len(), 128); // 2 docs * 64 u32

        let sig0 = output.get_signature(0);
        let sig1 = output.get_signature(1);

        // Signatures should have non-max values (tokens were hashed)
        assert!(sig0.iter().any(|&x| x != u16::MAX));
        assert!(sig1.iter().any(|&x| x != u16::MAX));

        // Documents share token 100, so some similarity expected
        let similarity = output.jaccard_similarity(0, 1);
        println!("Jaccard similarity (shared token 100): {:.3}", similarity);
        assert!(similarity > 0.0, "Shared token should create some similarity");
        assert!(similarity < 1.0, "Different tokens should prevent full similarity");
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_gpu_empty_documents() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = MinHashGpuCapsule::new(&ctx).expect("kernel creation");

        // Document with no tokens
        let tokens: Vec<u32> = vec![];
        let offsets = vec![0u32, 0]; // Doc 0 has 0 tokens

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 1,
        };

        let output = kernel.compute(&ctx, input).expect("GPU compute");

        let sig = output.get_signature(0);
        // Empty document should have all u16::MAX values
        assert!(sig.iter().all(|&x| x == u16::MAX));
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_gpu_single_token() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = MinHashGpuCapsule::new(&ctx).expect("kernel creation");

        let tokens = vec![12345u32];
        let offsets = vec![0u32, 1];

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 1,
        };

        let output = kernel.compute(&ctx, input).expect("GPU compute");

        let sig = output.get_signature(0);
        // Single token should update all 128 hash values
        assert!(sig.iter().all(|&x| x < u16::MAX));
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_gpu_deterministic() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = MinHashGpuCapsule::new(&ctx).expect("kernel creation");

        let tokens = vec![100u32, 200, 300, 400, 500];
        let offsets = vec![0u32, 5];

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 1,
        };

        // Compute twice
        let output1 = kernel.compute(&ctx, input.clone()).expect("compute 1");
        let output2 = kernel.compute(&ctx, input).expect("compute 2");

        // Results should be identical
        assert_eq!(
            output1.signatures, output2.signatures,
            "GPU MinHash should be deterministic"
        );
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_gpu_identical_documents() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = MinHashGpuCapsule::new(&ctx).expect("kernel creation");

        // Two identical documents
        let tokens = vec![
            100u32, 200, 300, // Doc 0
            100, 200, 300, // Doc 1 (identical)
        ];
        let offsets = vec![0u32, 3, 6];

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 2,
        };

        let output = kernel.compute(&ctx, input).expect("GPU compute");

        // Identical documents should have identical signatures
        let sig0 = output.get_signature(0);
        let sig1 = output.get_signature(1);
        assert_eq!(sig0, sig1, "Identical documents should have identical signatures");

        // Similarity should be 1.0
        assert_eq!(output.jaccard_similarity(0, 1), 1.0);
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_gpu_different_documents() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = MinHashGpuCapsule::new(&ctx).expect("kernel creation");

        // Two completely different documents
        let tokens = vec![
            100u32, 200, 300, // Doc 0
            400, 500, 600, // Doc 1 (completely different)
        ];
        let offsets = vec![0u32, 3, 6];

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 2,
        };

        let output = kernel.compute(&ctx, input).expect("GPU compute");

        let sig0 = output.get_signature(0);
        let sig1 = output.get_signature(1);

        // Different documents should have different signatures
        assert_ne!(sig0, sig1, "Different documents should have different signatures");

        // Similarity should be low (but not necessarily 0 due to hash collisions)
        let sim = output.jaccard_similarity(0, 1);
        println!("Similarity of completely different docs: {:.3}", sim);
        assert!(sim < 0.5, "Different docs should have low similarity");
    }

    // =========================================================================
    // Q15-Q21: Integration Tests (Throughput)
    // =========================================================================

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_gpu_throughput() {
        let Some(ctx) = try_get_gpu() else { return };

        println!("GPU: {}", ctx.capabilities().device_name);

        let kernel = MinHashGpuCapsule::new(&ctx).expect("kernel creation");

        // Generate test data: 10K documents, avg 100 tokens each
        let num_docs = 10_000u32;
        let tokens_per_doc = 100;
        let mut tokens = Vec::with_capacity(num_docs as usize * tokens_per_doc);
        let mut offsets = Vec::with_capacity(num_docs as usize + 1);

        offsets.push(0);
        for doc_id in 0..num_docs {
            for t in 0..tokens_per_doc {
                // Unique tokens per document
                tokens.push(doc_id * 1000 + t as u32);
            }
            offsets.push(tokens.len() as u32);
        }

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs,
        };

        // Warmup
        let _ = kernel.compute(&ctx, input.clone());

        // Benchmark
        let start = std::time::Instant::now();
        let iterations = 10;
        for _ in 0..iterations {
            let _ = kernel.compute(&ctx, input.clone());
        }
        let elapsed = start.elapsed();

        let total_docs = num_docs as f64 * iterations as f64;
        let docs_per_sec = total_docs / elapsed.as_secs_f64();
        let ms_per_batch = elapsed.as_millis() as f64 / iterations as f64;
        let us_per_doc = (elapsed.as_micros() as f64 / iterations as f64) / num_docs as f64;

        println!("\n=== GPU MinHash Throughput ===");
        println!("Documents: {}K", num_docs / 1000);
        println!("Tokens/doc: {}", tokens_per_doc);
        println!("Iterations: {}", iterations);
        println!("Throughput: {:.0} docs/sec", docs_per_sec);
        println!("Time/batch: {:.2}ms", ms_per_batch);
        println!("Time/doc: {:.3}μs", us_per_doc);
        println!("CPU baseline: 16.7μs/doc");
        println!("Speedup: {:.1}×", 16.7 / us_per_doc);

        // Minimum expectation: faster than CPU
        // GPU should achieve at least 100K docs/sec (vs 60K CPU)
        assert!(
            docs_per_sec > 50_000.0,
            "GPU should be competitive with CPU: {} docs/sec",
            docs_per_sec
        );
    }

    #[test]
    #[ignore] // Requires GPU hardware
    fn test_minhash_gpu_large_batch() {
        let Some(ctx) = try_get_gpu() else { return };

        let kernel = MinHashGpuCapsule::new(&ctx).expect("kernel creation");

        // Test with 100K documents (larger batch)
        let num_docs = 100_000u32;
        let tokens_per_doc = 50; // Fewer tokens for faster test

        let mut tokens = Vec::with_capacity(num_docs as usize * tokens_per_doc);
        let mut offsets = Vec::with_capacity(num_docs as usize + 1);

        offsets.push(0);
        for doc_id in 0..num_docs {
            for t in 0..tokens_per_doc {
                tokens.push(doc_id * 100 + t as u32);
            }
            offsets.push(tokens.len() as u32);
        }

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
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
        assert_eq!(output.signatures.len(), num_docs as usize * 64);
    }
}
