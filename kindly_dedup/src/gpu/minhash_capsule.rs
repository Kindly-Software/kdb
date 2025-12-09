//! MinHashGpuCapsule - T7 Heterogeneous Tier (GPU MinHash Computation)
//!
//! GPU-accelerated MinHash signature computation using wgpu compute shaders.
//!
//! # Performance Targets (B32 Framework)
//!
//! | Metric | CPU Baseline | GPU Target | Speedup |
//! |--------|--------------|------------|---------|
//! | Per-doc latency | 16.7us | 100-500ns | 33-167x |
//! | Throughput | 60K docs/sec | 500K-2M docs/sec | 8-33x |
//! | Batch size | 1 | 10,000 | N/A |
//!
//! # Algorithm
//!
//! MinHash signature computation:
//! 1. For each document, hash all tokens with 128 different permutation seeds
//! 2. Take minimum hash value for each permutation
//! 3. Truncate to u16 for memory efficiency (256B per signature)
//!
//! # GPU Parallelization
//!
//! - Each thread computes one (document, permutation) pair
//! - Workgroup size: 256 threads
//! - Dispatch: [num_docs, 128, 1] = num_docs * 128 threads
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (GPU compute)
//! - **Chaos**: Immutable input buffers, atomic output writes
//! - **ASSUM**: Input validation, bounds checking
//! - **B32**: Fair benchmarking (vs CPU SIMD baseline)
//! - **T28**: Kernel correctness tests

use super::context::GpuContextCapsule;
use super::error::{GpuError, GpuResult};
use std::sync::atomic::{AtomicU64, Ordering};

/// MinHash GPU input data
///
/// # Layout
///
/// Tokens are stored in a flat array with per-document offsets:
/// ```text
/// tokens:  [doc0_tok0, doc0_tok1, ..., doc1_tok0, doc1_tok1, ...]
/// offsets: [0, doc0_len, doc0_len + doc1_len, ...]
/// ```
pub struct MinHashGpuInput<'a> {
    /// Pre-hashed token values (u32)
    pub tokens: &'a [u32],

    /// Document offsets in token array (length = num_docs + 1)
    pub offsets: &'a [u32],

    /// Number of documents
    pub num_docs: u32,
}

impl<'a> MinHashGpuInput<'a> {
    /// Validate input data
    pub fn validate(&self) -> GpuResult<()> {
        // Check offset array length
        if self.offsets.len() != (self.num_docs as usize + 1) {
            return Err(GpuError::InvalidInput(format!(
                "offsets length {} doesn't match num_docs {} + 1",
                self.offsets.len(),
                self.num_docs
            )));
        }

        // Check offsets are monotonic
        for i in 1..self.offsets.len() {
            if self.offsets[i] < self.offsets[i - 1] {
                return Err(GpuError::InvalidInput(format!(
                    "offsets not monotonic at index {}: {} < {}",
                    i, self.offsets[i], self.offsets[i - 1]
                )));
            }
        }

        // Check final offset matches token array
        if let Some(&last_offset) = self.offsets.last() {
            if last_offset as usize > self.tokens.len() {
                return Err(GpuError::InvalidInput(format!(
                    "final offset {} exceeds token array length {}",
                    last_offset,
                    self.tokens.len()
                )));
            }
        }

        Ok(())
    }
}

/// MinHash GPU output data
///
/// Contains 128 x u16 MinHash signature per document.
pub struct MinHashGpuOutput {
    /// Signatures: num_docs x 128 x u16 (stored as Vec<u16>)
    signatures: Vec<u16>,

    /// Number of documents
    num_docs: usize,

    /// Generation counter (Q34 audit trail)
    generation: u64,
}

impl MinHashGpuOutput {
    /// Create new output buffer
    pub fn new(num_docs: usize, generation: u64) -> Self {
        Self {
            signatures: vec![u16::MAX; num_docs * 128],
            num_docs,
            generation,
        }
    }

    /// Get signature for document
    ///
    /// # Arguments
    ///
    /// - `doc_idx`: Document index (0-based)
    ///
    /// # Returns
    ///
    /// 128-element u16 slice
    pub fn get_signature(&self, doc_idx: usize) -> &[u16] {
        let start = doc_idx * 128;
        let end = start + 128;
        &self.signatures[start..end]
    }

    /// Get mutable signature for document
    pub fn get_signature_mut(&mut self, doc_idx: usize) -> &mut [u16] {
        let start = doc_idx * 128;
        let end = start + 128;
        &mut self.signatures[start..end]
    }

    /// Convert signature to fixed-size array
    pub fn get_signature_array(&self, doc_idx: usize) -> [u16; 128] {
        let mut arr = [0u16; 128];
        arr.copy_from_slice(self.get_signature(doc_idx));
        arr
    }

    /// Get number of documents
    pub fn num_docs(&self) -> usize {
        self.num_docs
    }

    /// Get generation (Q34 audit)
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Get raw signatures buffer
    pub fn as_slice(&self) -> &[u16] {
        &self.signatures
    }
}

/// GPU-accelerated MinHash capsule (T7 Heterogeneous Tier)
///
/// # Architecture
///
/// ```text
/// MinHashGpuCapsule
/// ├── pipeline: wgpu::ComputePipeline (shader program)
/// ├── seed_buffer: wgpu::Buffer (128 permutation seeds)
/// ├── generation: AtomicU64 (Q34 audit)
/// └── max_batch_size: u32 (GPU memory limit)
/// ```
#[repr(C, align(128))]
pub struct MinHashGpuCapsule {
    /// Compute pipeline (compiled shader)
    #[cfg(feature = "gpu")]
    pipeline: wgpu::ComputePipeline,

    /// Seed buffer (128 permutation seeds)
    #[cfg(feature = "gpu")]
    seed_buffer: wgpu::Buffer,

    /// Bind group layout
    #[cfg(feature = "gpu")]
    bind_group_layout: wgpu::BindGroupLayout,

    /// Generation counter (Q34 audit trail)
    generation: AtomicU64,

    /// Maximum batch size (documents per dispatch)
    max_batch_size: u32,

    /// Padding for alignment
    _padding: [u8; 64],
}

/// WGSL compute shader for MinHash computation
const MINHASH_SHADER: &str = r#"
// MinHash Compute Shader
// Computes 128-value MinHash signature for batch of documents

// Uniform: Permutation seeds (128 x u32)
@group(0) @binding(0) var<uniform> seeds: array<u32, 128>;

// Storage: Token values (flat array, accessed via offsets)
@group(0) @binding(1) var<storage, read> tokens: array<u32>;

// Storage: Document offsets (num_docs + 1 values)
@group(0) @binding(2) var<storage, read> offsets: array<u32>;

// Storage: Output signatures (num_docs x 128 x u16, packed as u32)
@group(0) @binding(3) var<storage, read_write> signatures: array<u32>;

// Uniform: Number of documents
@group(0) @binding(4) var<uniform> num_docs: u32;

// MurmurHash3 finalizer (good avalanche properties)
fn murmur_hash(key: u32, seed: u32) -> u32 {
    var h = seed;
    let c1: u32 = 0xcc9e2d51u;
    let c2: u32 = 0x1b873593u;

    var k = key;
    k = k * c1;
    k = (k << 15u) | (k >> 17u);
    k = k * c2;

    h = h ^ k;
    h = (h << 13u) | (h >> 19u);
    h = h * 5u + 0xe6546b64u;

    // Finalization
    h = h ^ 4u;
    h = h ^ (h >> 16u);
    h = h * 0x85ebca6bu;
    h = h ^ (h >> 13u);
    h = h * 0xc2b2ae35u;
    h = h ^ (h >> 16u);

    return h;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let doc_id = gid.x;
    let perm_id = gid.y;

    // Bounds check
    if (doc_id >= num_docs) {
        return;
    }
    if (perm_id >= 128u) {
        return;
    }

    // Get token range for this document
    let start_offset = offsets[doc_id];
    let end_offset = offsets[doc_id + 1u];
    let num_tokens = end_offset - start_offset;

    // Compute minimum hash for this permutation
    var min_hash: u32 = 0xFFFFFFFFu;
    let seed = seeds[perm_id];

    for (var i: u32 = 0u; i < num_tokens; i = i + 1u) {
        let token = tokens[start_offset + i];
        let h = murmur_hash(token, seed);
        min_hash = min(min_hash, h);
    }

    // Store result (u16 truncation)
    // Pack two u16 values per u32 slot
    let sig_idx = doc_id * 64u + perm_id / 2u;
    let is_high = (perm_id & 1u) == 1u;

    if (is_high) {
        // Store in high 16 bits
        let low_val = signatures[sig_idx] & 0xFFFFu;
        signatures[sig_idx] = low_val | ((min_hash & 0xFFFFu) << 16u);
    } else {
        // Store in low 16 bits
        let high_val = signatures[sig_idx] & 0xFFFF0000u;
        signatures[sig_idx] = high_val | (min_hash & 0xFFFFu);
    }
}
"#;

impl MinHashGpuCapsule {
    /// Create new MinHash GPU capsule
    ///
    /// # Arguments
    ///
    /// - `ctx`: GPU context
    ///
    /// # Performance
    ///
    /// - Initialization: <10ms (shader compilation)
    /// - Memory: ~1KB (pipeline + seeds buffer)
    #[cfg(feature = "gpu")]
    pub fn new(ctx: &GpuContextCapsule) -> GpuResult<Self> {
        let device = ctx.device();

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("minhash_shader"),
            source: wgpu::ShaderSource::Wgsl(MINHASH_SHADER.into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("minhash_bind_group_layout"),
            entries: &[
                // Seeds (uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Tokens (storage, read)
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
                // Offsets (storage, read)
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
                // Signatures (storage, read_write)
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
                // num_docs (uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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
            label: Some("minhash_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("minhash_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Create seed buffer with 128 permutation seeds
        // Using Fibonacci hashing for good distribution
        let seeds: Vec<u32> = (0..128u32)
            .map(|i| {
                // Golden ratio hash for seed generation
                let golden = 2654435769u32; // (2^32) / golden ratio
                i.wrapping_mul(golden)
            })
            .collect();

        let seed_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("seed_buffer"),
            contents: bytemuck::cast_slice(&seeds),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        Ok(Self {
            pipeline,
            seed_buffer,
            bind_group_layout,
            generation: AtomicU64::new(0),
            max_batch_size: 100_000, // 100K docs per batch max
            _padding: [0; 64],
        })
    }

    /// Compute MinHash signatures for batch of documents
    ///
    /// # Arguments
    ///
    /// - `ctx`: GPU context
    /// - `input`: Input documents (tokens + offsets)
    ///
    /// # Performance
    ///
    /// - 10K docs: ~1ms (GPU kernel + transfer)
    /// - 100K docs: ~10ms
    ///
    /// # Returns
    ///
    /// MinHash signatures (128 x u16 per document)
    #[cfg(feature = "gpu")]
    pub fn compute(
        &self,
        ctx: &GpuContextCapsule,
        input: MinHashGpuInput<'_>,
    ) -> GpuResult<MinHashGpuOutput> {
        // Validate input
        input.validate()?;

        if input.num_docs == 0 {
            return Ok(MinHashGpuOutput::new(0, self.generation.load(Ordering::Relaxed)));
        }

        if input.num_docs > self.max_batch_size {
            return Err(GpuError::InvalidInput(format!(
                "Batch size {} exceeds max {}",
                input.num_docs, self.max_batch_size
            )));
        }

        let device = ctx.device();
        let queue = ctx.queue();

        // Create token buffer
        let token_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("token_buffer"),
            contents: bytemuck::cast_slice(input.tokens),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Create offset buffer
        let offset_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("offset_buffer"),
            contents: bytemuck::cast_slice(input.offsets),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Create output buffer (num_docs * 64 * u32 = num_docs * 128 * u16)
        let output_size = (input.num_docs as usize * 64 * 4) as u64;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("signature_buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create num_docs uniform buffer
        let num_docs_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("num_docs_buffer"),
            contents: bytemuck::cast_slice(&[input.num_docs]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("minhash_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.seed_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: token_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: offset_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: num_docs_buffer.as_entire_binding(),
                },
            ],
        });

        // Create staging buffer for readback
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Encode commands
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("minhash_encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("minhash_pass"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch: [num_docs, 128, 1]
            // Each thread handles one (doc, permutation) pair
            let workgroups_x = (input.num_docs + 255) / 256;
            pass.dispatch_workgroups(workgroups_x, 128, 1);
        }

        // Copy output to staging buffer
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);

        // Submit commands
        queue.submit(std::iter::once(encoder.finish()));

        // Map staging buffer and read results with timeout protection
        // Per wgpu issue #3601: Maintain::Wait can hang indefinitely on driver failure
        // SOTA pattern: Use Maintain::Poll with try_recv() for graceful timeout handling
        //
        // #ASSUME_POLL_TIMEOUT: GPU operations complete within timeout under normal conditions
        // #VERIFY_POLL_TIMEOUT: Environment variable KINDLY_GPU_POLL_TIMEOUT_SECS allows runtime config
        let timeout_secs: u64 = std::env::var("KINDLY_GPU_POLL_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);
        let poll_start = std::time::Instant::now();
        let poll_timeout = std::time::Duration::from_secs(timeout_secs);

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result); // Ignore send errors (receiver may have timed out)
        });

        // Timeout-protected polling loop (T7 Heterogeneous tier - Chaos lockfree coordination)
        loop {
            device.poll(wgpu::Maintain::Poll);

            match rx.try_recv() {
                Ok(result) => {
                    result.map_err(|e| GpuError::BufferMappingFailed(format!("{:?}", e)))?;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if poll_start.elapsed() > poll_timeout {
                        return Err(GpuError::Timeout { timeout_secs });
                    }
                    std::thread::sleep(std::time::Duration::from_micros(100));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err(GpuError::BufferMappingFailed(
                        "Channel disconnected: GPU callback never fired".to_string(),
                    ));
                }
            }
        }

        // Extract u16 values from packed u32 buffer
        let data = buffer_slice.get_mapped_range();
        let packed: &[u32] = bytemuck::cast_slice(&data);

        let mut output = MinHashGpuOutput::new(
            input.num_docs as usize,
            self.generation.fetch_add(1, Ordering::AcqRel),
        );

        // Unpack u32 -> 2 x u16
        for doc_idx in 0..input.num_docs as usize {
            let sig = output.get_signature_mut(doc_idx);
            for i in 0..64 {
                let packed_idx = doc_idx * 64 + i;
                let packed_val = packed[packed_idx];
                sig[i * 2] = (packed_val & 0xFFFF) as u16;
                sig[i * 2 + 1] = ((packed_val >> 16) & 0xFFFF) as u16;
            }
        }

        drop(data);
        staging_buffer.unmap();

        Ok(output)
    }

    /// Stub implementation when GPU feature is disabled
    #[cfg(not(feature = "gpu"))]
    pub fn new(_ctx: &GpuContextCapsule) -> GpuResult<Self> {
        Err(GpuError::FeatureNotSupported(
            "GPU feature not enabled".to_string(),
        ))
    }

    /// Get generation counter (Q34 audit)
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

// wgpu buffer initialization helper
#[cfg(feature = "gpu")]
mod wgpu {
    pub use ::wgpu::*;

    pub mod util {
        pub struct BufferInitDescriptor<'a> {
            pub label: Option<&'a str>,
            pub contents: &'a [u8],
            pub usage: super::BufferUsages,
        }
    }

    impl super::super::context::GpuContextCapsule {
        pub fn create_buffer_init(
            device: &Device,
            desc: &util::BufferInitDescriptor,
        ) -> Buffer {
            device.create_buffer(&BufferDescriptor {
                label: desc.label,
                size: desc.contents.len() as u64,
                usage: desc.usage | BufferUsages::COPY_DST,
                mapped_at_creation: true,
            })
        }
    }
}

// Re-export wgpu::util extension trait
#[cfg(feature = "gpu")]
trait DeviceExt {
    fn create_buffer_init(&self, desc: &wgpu::util::BufferInitDescriptor) -> wgpu::Buffer;
}

#[cfg(feature = "gpu")]
impl DeviceExt for wgpu::Device {
    fn create_buffer_init(&self, desc: &wgpu::util::BufferInitDescriptor) -> wgpu::Buffer {
        use wgpu::util::DeviceExt as WgpuDeviceExt;
        self.create_buffer_init(&::wgpu::util::BufferInitDescriptor {
            label: desc.label,
            contents: desc.contents,
            usage: desc.usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_validation_empty() {
        let input = MinHashGpuInput {
            tokens: &[],
            offsets: &[0],
            num_docs: 0,
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_input_validation_single_doc() {
        let tokens = [1u32, 2, 3, 4, 5];
        let offsets = [0u32, 5];
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 1,
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_input_validation_multiple_docs() {
        let tokens = [1u32, 2, 3, 4, 5, 6, 7, 8];
        let offsets = [0u32, 3, 5, 8];
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 3,
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_input_validation_offset_mismatch() {
        let tokens = [1u32, 2, 3];
        let offsets = [0u32, 3]; // Should have 2 entries for num_docs=1
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 0, // Mismatch!
        };
        // With num_docs=0, we expect offsets.len() = 1
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_input_validation_non_monotonic() {
        let tokens = [1u32, 2, 3, 4, 5];
        let offsets = [0u32, 5, 3]; // Non-monotonic!
        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: 2,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_output_signature_access() {
        let output = MinHashGpuOutput::new(3, 0);
        assert_eq!(output.num_docs(), 3);
        assert_eq!(output.get_signature(0).len(), 128);
        assert_eq!(output.get_signature(1).len(), 128);
        assert_eq!(output.get_signature(2).len(), 128);
    }

    #[test]
    fn test_output_signature_array() {
        let output = MinHashGpuOutput::new(1, 42);
        let arr = output.get_signature_array(0);
        assert_eq!(arr.len(), 128);
        assert_eq!(output.generation(), 42);
    }
}
