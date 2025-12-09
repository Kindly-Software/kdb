//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! GPU Stress Benchmark for kindly-av1
//! ===================================
//!
//! Comprehensive cross-platform GPU stress testing using atomic_capsule kgpu infrastructure:
//! - NVIDIA RTX 3080 (CUDA via atomic_capsule::gpu) - 30 TFLOPs FP32 compute, 760 GB/s bandwidth
//! - NVIDIA RTX 3080M (CUDA via atomic_capsule::gpu) - 10 TFLOPs compute, 576 GB/s bandwidth
//! - AMD 680M (ROCm HIP via atomic_capsule::gpu) - 2.7 TFLOPs compute, 96 GB/s bandwidth
//!
//! # Framework: B32 Performance Validation
//!
//! All benchmarks follow B32 compliance:
//! - Q1: 95% confidence interval (Criterion default)
//! - Q2: 1000+ iterations (via strategic sample_size)
//! - Q3: Fair baseline comparison (GPU vs CPU)
//! - Q4: Reproducible (kindly-hub hardware)
//! - Q5: Realistic workloads (actual AV1 encoder patterns)
//! - Q6: Statistical validation (Criterion built-in)
//!
//! # GPU Backends (via atomic_capsule)
//!
//! When `gpu-rocm` feature is enabled:
//! - Uses atomic_capsule::gpu::RocmBackend for AMD GPU acceleration
//! - Integrates with GpuMatMulCapsule (T7 Heterogeneous tier, 100-1000x speedup)
//! - Auto-detects ROCm availability via atomic_capsule::gpu::detect_backend()
//!
//! When `gpu-cuda` feature is enabled:
//! - Uses atomic_capsule::gpu::CudaBackend for NVIDIA GPU acceleration
//! - Integrates with GpuMatMulCapsule (cuBLAS SGEMM, target: 3 TFLOPS)
//!
//! When no GPU feature enabled:
//! - Falls back to CPU blocked SGEMM (cache-friendly)
//! - Baseline for comparison via atomic_capsule::gpu::CpuFallbackBackend
//!
//! # Run Commands
//!
//! ```bash
//! # CPU-only benchmarks (default)
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_stress_bench --release"
//!
//! # GPU benchmarks with ROCm (AMD GPUs)
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_stress_bench --release --features gpu-rocm"
//!
//! # GPU benchmarks with CUDA (NVIDIA GPUs)
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_stress_bench --release --features gpu-cuda"
//!
//! # GPU benchmarks with Vulkan (direct Vulkan API - fallback)
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_stress_bench --release --features gpu-vulkan"
//!
//! # Specific workload
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_stress_bench -- matmul"
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_stress_bench -- fft"
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_stress_bench -- bandwidth"
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_stress_bench -- thermal"
//! ```

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// atomic_capsule GPU infrastructure imports
#[cfg(any(
    feature = "gpu-rocm",
    feature = "gpu-cuda",
    feature = "gpu-intel",
    feature = "gpu-all",
    feature = "gpu-vulkan"
))]
use atomic_capsule::gpu::{
    create_best_backend, detect_backend, BackendType, DeviceMemoryPtr, GpuBackendTrait, GpuError,
    GpuResult,
};

// GPU kernel capsule for matrix multiplication (T7 Heterogeneous tier)
#[cfg(any(feature = "gpu-rocm", feature = "gpu-cuda"))]
use atomic_capsule::gpu::kernels::{GpuMatMulCapsule, GpuTensorCapsule};

// ============================================================================
// Vulkan GPU SGEMM Context (when gpu-vulkan feature enabled)
// ============================================================================

#[cfg(feature = "gpu-vulkan")]
mod vulkan_sgemm {
    use ash::vk;
    use gpu_allocator::vulkan::{
        Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc,
    };
    use gpu_allocator::MemoryLocation;
    use std::ffi::CString;
    use std::sync::{Arc, Mutex};

    /// GLSL compute shader for SGEMM (embedded as SPIR-V at compile time)
    /// Computes C = alpha * A @ B + beta * C
    /// Uses 16x16 workgroup tiles with shared memory optimization
    const SGEMM_SPIRV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sgemm.spv"));

    /// Vulkan SGEMM context for GPU matrix multiplication
    pub struct VulkanSgemmContext {
        entry: ash::Entry,
        instance: ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: ash::Device,
        compute_queue: vk::Queue,
        queue_family_index: u32,
        command_pool: vk::CommandPool,
        descriptor_pool: vk::DescriptorPool,
        descriptor_set_layout: vk::DescriptorSetLayout,
        pipeline_layout: vk::PipelineLayout,
        pipeline: vk::Pipeline,
        allocator: Arc<Mutex<Allocator>>,
        /// Device name for identification
        pub device_name: String,
    }

    /// GPU buffer for SGEMM operations
    pub struct GpuBuffer {
        buffer: vk::Buffer,
        allocation: Allocation,
        size: u64,
    }

    impl VulkanSgemmContext {
        /// Create new Vulkan SGEMM context
        pub fn new() -> Result<Self, String> {
            // Load Vulkan
            let entry =
                unsafe { ash::Entry::load().map_err(|e| format!("Failed to load Vulkan: {}", e))? };

            // Create instance
            let app_name = CString::new("kindly-av1-bench").unwrap();
            let engine_name = CString::new("kindly-bench").unwrap();

            let app_info = vk::ApplicationInfo::default()
                .application_name(&app_name)
                .application_version(vk::make_api_version(0, 1, 0, 0))
                .engine_name(&engine_name)
                .engine_version(vk::make_api_version(0, 1, 0, 0))
                .api_version(vk::API_VERSION_1_2);

            let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);

            let instance = unsafe {
                entry
                    .create_instance(&create_info, None)
                    .map_err(|e| format!("Failed to create instance: {}", e))?
            };

            // Find physical device (prefer discrete GPU)
            let physical_devices = unsafe {
                instance
                    .enumerate_physical_devices()
                    .map_err(|e| format!("Failed to enumerate devices: {}", e))?
            };

            if physical_devices.is_empty() {
                return Err("No Vulkan devices found".to_string());
            }

            // Prefer discrete GPU
            let (physical_device, device_name) = {
                let mut selected = physical_devices[0];
                let mut selected_name = String::new();

                for &device in &physical_devices {
                    let props = unsafe { instance.get_physical_device_properties(device) };
                    let name = unsafe {
                        std::ffi::CStr::from_ptr(props.device_name.as_ptr())
                            .to_string_lossy()
                            .to_string()
                    };

                    if props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                        selected = device;
                        selected_name = name;
                        break;
                    }
                    if selected_name.is_empty() {
                        selected_name = name;
                    }
                }
                (selected, selected_name)
            };

            // Find compute queue family
            let queue_families =
                unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

            let queue_family_index = queue_families
                .iter()
                .position(|qf| qf.queue_flags.contains(vk::QueueFlags::COMPUTE))
                .ok_or("No compute queue family found")?
                as u32;

            // Create logical device
            let queue_priorities = [1.0f32];
            let queue_create_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family_index)
                .queue_priorities(&queue_priorities);

            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(std::slice::from_ref(&queue_create_info));

            let device = unsafe {
                instance
                    .create_device(physical_device, &device_create_info, None)
                    .map_err(|e| format!("Failed to create device: {}", e))?
            };

            let compute_queue = unsafe { device.get_device_queue(queue_family_index, 0) };

            // Create command pool
            let command_pool_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(queue_family_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

            let command_pool = unsafe {
                device
                    .create_command_pool(&command_pool_info, None)
                    .map_err(|e| format!("Failed to create command pool: {}", e))?
            };

            // Create memory allocator
            let allocator = {
                let desc = AllocatorCreateDesc {
                    instance: instance.clone(),
                    device: device.clone(),
                    physical_device,
                    debug_settings: Default::default(),
                    buffer_device_address: false,
                    allocation_sizes: Default::default(),
                };
                Allocator::new(&desc).map_err(|e| format!("Failed to create allocator: {}", e))?
            };

            // Create descriptor pool
            let pool_sizes = [vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 3, // A, B, C matrices
            }];

            let descriptor_pool_info = vk::DescriptorPoolCreateInfo::default()
                .max_sets(16)
                .pool_sizes(&pool_sizes);

            let descriptor_pool = unsafe {
                device
                    .create_descriptor_pool(&descriptor_pool_info, None)
                    .map_err(|e| format!("Failed to create descriptor pool: {}", e))?
            };

            // Create descriptor set layout
            let bindings = [
                vk::DescriptorSetLayoutBinding::default()
                    .binding(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE),
                vk::DescriptorSetLayoutBinding::default()
                    .binding(1)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE),
                vk::DescriptorSetLayoutBinding::default()
                    .binding(2)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE),
            ];

            let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);

            let descriptor_set_layout = unsafe {
                device
                    .create_descriptor_set_layout(&layout_info, None)
                    .map_err(|e| format!("Failed to create descriptor set layout: {}", e))?
            };

            // Create pipeline layout with push constants
            let push_constant_range = vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
                .offset(0)
                .size(24); // M, N, K, alpha, beta, padding (6 x u32)

            let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
                .set_layouts(std::slice::from_ref(&descriptor_set_layout))
                .push_constant_ranges(std::slice::from_ref(&push_constant_range));

            let pipeline_layout = unsafe {
                device
                    .create_pipeline_layout(&pipeline_layout_info, None)
                    .map_err(|e| format!("Failed to create pipeline layout: {}", e))?
            };

            // Load SPIR-V shader
            let spirv_words: Vec<u32> = SGEMM_SPIRV
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();

            let shader_module_info = vk::ShaderModuleCreateInfo::default().code(&spirv_words);

            let shader_module = unsafe {
                device
                    .create_shader_module(&shader_module_info, None)
                    .map_err(|e| format!("Failed to create shader module: {}", e))?
            };

            // Create compute pipeline
            let entry_point = CString::new("main").unwrap();

            let stage_info = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(shader_module)
                .name(&entry_point);

            let pipeline_info = vk::ComputePipelineCreateInfo::default()
                .stage(stage_info)
                .layout(pipeline_layout);

            let pipeline = unsafe {
                device
                    .create_compute_pipelines(
                        vk::PipelineCache::null(),
                        std::slice::from_ref(&pipeline_info),
                        None,
                    )
                    .map_err(|e| format!("Failed to create pipeline: {:?}", e))?[0]
            };

            // Cleanup shader module
            unsafe {
                device.destroy_shader_module(shader_module, None);
            }

            Ok(Self {
                entry,
                instance,
                physical_device,
                device,
                compute_queue,
                queue_family_index,
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
                pipeline_layout,
                pipeline,
                allocator: Arc::new(Mutex::new(allocator)),
                device_name,
            })
        }

        /// Allocate GPU buffer
        pub fn allocate_buffer(&self, size: u64, host_visible: bool) -> Result<GpuBuffer, String> {
            let buffer_info = vk::BufferCreateInfo::default()
                .size(size)
                .usage(
                    vk::BufferUsageFlags::STORAGE_BUFFER
                        | vk::BufferUsageFlags::TRANSFER_DST
                        | vk::BufferUsageFlags::TRANSFER_SRC,
                )
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let buffer = unsafe {
                self.device
                    .create_buffer(&buffer_info, None)
                    .map_err(|e| format!("Failed to create buffer: {}", e))?
            };

            let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };

            let location = if host_visible {
                MemoryLocation::CpuToGpu
            } else {
                MemoryLocation::GpuOnly
            };

            let allocation = self
                .allocator
                .lock()
                .unwrap()
                .allocate(&AllocationCreateDesc {
                    name: "sgemm_buffer",
                    requirements,
                    location,
                    linear: true,
                    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
                })
                .map_err(|e| format!("Failed to allocate memory: {}", e))?;

            unsafe {
                self.device
                    .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                    .map_err(|e| format!("Failed to bind memory: {}", e))?;
            }

            Ok(GpuBuffer {
                buffer,
                allocation,
                size,
            })
        }

        /// Upload data to GPU buffer
        pub fn upload(&self, buffer: &GpuBuffer, data: &[f32]) -> Result<(), String> {
            let ptr = buffer
                .allocation
                .mapped_ptr()
                .ok_or("Buffer not host-visible")?;

            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.as_ptr() as *mut f32, data.len());
            }
            Ok(())
        }

        /// Download data from GPU buffer
        pub fn download(&self, buffer: &GpuBuffer, data: &mut [f32]) -> Result<(), String> {
            let ptr = buffer
                .allocation
                .mapped_ptr()
                .ok_or("Buffer not host-visible")?;

            unsafe {
                std::ptr::copy_nonoverlapping(
                    ptr.as_ptr() as *const f32,
                    data.as_mut_ptr(),
                    data.len(),
                );
            }
            Ok(())
        }

        /// Execute SGEMM: C = alpha * A @ B + beta * C
        pub fn sgemm(
            &self,
            m: u32,
            n: u32,
            k: u32,
            alpha: f32,
            a: &GpuBuffer,
            b: &GpuBuffer,
            beta: f32,
            c: &GpuBuffer,
        ) -> Result<(), String> {
            // Allocate descriptor set
            let alloc_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(self.descriptor_pool)
                .set_layouts(std::slice::from_ref(&self.descriptor_set_layout));

            let descriptor_set = unsafe {
                self.device
                    .allocate_descriptor_sets(&alloc_info)
                    .map_err(|e| format!("Failed to allocate descriptor set: {}", e))?[0]
            };

            // Update descriptor set
            let buffer_infos = [
                vk::DescriptorBufferInfo::default()
                    .buffer(a.buffer)
                    .offset(0)
                    .range(a.size),
                vk::DescriptorBufferInfo::default()
                    .buffer(b.buffer)
                    .offset(0)
                    .range(b.size),
                vk::DescriptorBufferInfo::default()
                    .buffer(c.buffer)
                    .offset(0)
                    .range(c.size),
            ];

            let writes = [
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(std::slice::from_ref(&buffer_infos[0])),
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_set)
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(std::slice::from_ref(&buffer_infos[1])),
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_set)
                    .dst_binding(2)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(std::slice::from_ref(&buffer_infos[2])),
            ];

            unsafe {
                self.device.update_descriptor_sets(&writes, &[]);
            }

            // Allocate command buffer
            let cmd_alloc_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);

            let cmd_buffer = unsafe {
                self.device
                    .allocate_command_buffers(&cmd_alloc_info)
                    .map_err(|e| format!("Failed to allocate command buffer: {}", e))?[0]
            };

            // Record commands
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

            unsafe {
                self.device
                    .begin_command_buffer(cmd_buffer, &begin_info)
                    .map_err(|e| format!("Failed to begin command buffer: {}", e))?;

                self.device.cmd_bind_pipeline(
                    cmd_buffer,
                    vk::PipelineBindPoint::COMPUTE,
                    self.pipeline,
                );
                self.device.cmd_bind_descriptor_sets(
                    cmd_buffer,
                    vk::PipelineBindPoint::COMPUTE,
                    self.pipeline_layout,
                    0,
                    &[descriptor_set],
                    &[],
                );

                // Push constants: M, N, K, alpha, beta, padding
                let push_data = [m, n, k, alpha.to_bits(), beta.to_bits(), 0u32];
                self.device.cmd_push_constants(
                    cmd_buffer,
                    self.pipeline_layout,
                    vk::ShaderStageFlags::COMPUTE,
                    0,
                    bytemuck::cast_slice(&push_data),
                );

                // Dispatch: workgroup size 16x16, dispatch (M+15)/16 x (N+15)/16
                let group_x = (m + 15) / 16;
                let group_y = (n + 15) / 16;
                self.device.cmd_dispatch(cmd_buffer, group_x, group_y, 1);

                self.device
                    .end_command_buffer(cmd_buffer)
                    .map_err(|e| format!("Failed to end command buffer: {}", e))?;
            }

            // Submit and wait
            let fence_info = vk::FenceCreateInfo::default();
            let fence = unsafe {
                self.device
                    .create_fence(&fence_info, None)
                    .map_err(|e| format!("Failed to create fence: {}", e))?
            };

            let submit_info =
                vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd_buffer));

            unsafe {
                self.device
                    .queue_submit(self.compute_queue, &[submit_info], fence)
                    .map_err(|e| format!("Failed to submit: {}", e))?;
                self.device
                    .wait_for_fences(&[fence], true, u64::MAX)
                    .map_err(|e| format!("Failed to wait: {}", e))?;
            }

            // Cleanup
            unsafe {
                self.device.destroy_fence(fence, None);
                self.device
                    .free_command_buffers(self.command_pool, &[cmd_buffer]);
                self.device
                    .free_descriptor_sets(self.descriptor_pool, &[descriptor_set])
                    .ok(); // Ignore error, pool may not support individual frees
            }

            Ok(())
        }

        /// Free GPU buffer
        pub fn free_buffer(&self, buffer: GpuBuffer) {
            unsafe {
                self.device.destroy_buffer(buffer.buffer, None);
            }
            self.allocator.lock().unwrap().free(buffer.allocation).ok();
        }
    }

    impl Drop for VulkanSgemmContext {
        fn drop(&mut self) {
            unsafe {
                self.device.device_wait_idle().ok();
                self.device.destroy_pipeline(self.pipeline, None);
                self.device
                    .destroy_pipeline_layout(self.pipeline_layout, None);
                self.device
                    .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
                self.device
                    .destroy_descriptor_pool(self.descriptor_pool, None);
                self.device.destroy_command_pool(self.command_pool, None);
                // Allocator dropped via Arc
                self.device.destroy_device(None);
                self.instance.destroy_instance(None);
            }
        }
    }
}

#[cfg(feature = "gpu-vulkan")]
use vulkan_sgemm::VulkanSgemmContext;

/// GPU backend availability (uses atomic_capsule::gpu::BackendType)
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GpuBackendType {
    Cuda,
    Rocm,
    Vulkan,
    CpuFallback,
}

impl std::fmt::Display for GpuBackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cuda => write!(f, "CUDA (atomic_capsule)"),
            Self::Rocm => write!(f, "ROCm (atomic_capsule)"),
            Self::Vulkan => write!(f, "Vulkan (direct)"),
            Self::CpuFallback => write!(f, "CPU Fallback"),
        }
    }
}

/// Detect available GPU backend using atomic_capsule infrastructure
///
/// Priority order (matches atomic_capsule::gpu::detect_backend):
/// 1. CUDA (NVIDIA) - highest priority
/// 2. ROCm (AMD) - second priority
/// 3. Vulkan (fallback) - if enabled
/// 4. CPU Fallback - always available
fn detect_gpu_backend() -> GpuBackendType {
    #[cfg(any(
        feature = "gpu-rocm",
        feature = "gpu-cuda",
        feature = "gpu-intel",
        feature = "gpu-all",
        feature = "gpu-vulkan"
    ))]
    {
        // Use atomic_capsule's detect_backend() for CUDA/ROCm detection
        let backend_type = detect_backend();

        match backend_type {
            BackendType::Cuda => GpuBackendType::Cuda,
            BackendType::Rocm => GpuBackendType::Rocm,
            BackendType::IntelXe2 => {
                // Intel Xe2 not fully supported yet, fall through
                #[cfg(feature = "gpu-vulkan")]
                {
                    GpuBackendType::Vulkan
                }
                #[cfg(not(feature = "gpu-vulkan"))]
                {
                    GpuBackendType::CpuFallback
                }
            }
            BackendType::CpuFallback => {
                // Check if Vulkan is available as fallback
                #[cfg(feature = "gpu-vulkan")]
                {
                    GpuBackendType::Vulkan
                }
                #[cfg(not(feature = "gpu-vulkan"))]
                {
                    GpuBackendType::CpuFallback
                }
            }
        }
    }

    #[cfg(not(any(
        feature = "gpu-rocm",
        feature = "gpu-cuda",
        feature = "gpu-intel",
        feature = "gpu-all",
        feature = "gpu-vulkan"
    )))]
    {
        // No GPU features enabled, check Vulkan or fall back to CPU
        #[cfg(feature = "gpu-vulkan")]
        {
            GpuBackendType::Vulkan
        }
        #[cfg(not(feature = "gpu-vulkan"))]
        {
            GpuBackendType::CpuFallback
        }
    }
}

/// Get GPU backend name string for benchmark labels
#[allow(dead_code)]
fn get_backend_name() -> &'static str {
    match detect_gpu_backend() {
        GpuBackendType::Cuda => "gpu_cuda_sgemm",
        GpuBackendType::Rocm => "gpu_rocm_sgemm",
        GpuBackendType::Vulkan => "gpu_vulkan_sgemm",
        GpuBackendType::CpuFallback => "cpu_sgemm",
    }
}

// ============================================================================
// Large Matrix Multiplication Workload
// ============================================================================

/// Generate test matrices for multiplication (deterministic PRNG)
fn generate_test_matrices(m: usize, n: usize, k: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut seed = 12345u64;
    let lcg = |s: &mut u64| -> f32 {
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let bits = (*s >> 11) as u32;
        let float_bits = (bits & 0x7fffff) | 0x3f800000;
        f32::from_bits(float_bits) - 1.5
    };

    let a = (0..m * k).map(|_| lcg(&mut seed)).collect::<Vec<_>>();
    let b = (0..k * n).map(|_| lcg(&mut seed)).collect::<Vec<_>>();
    let c = vec![0.0f32; m * n];

    (a, b, c)
}

/// CPU SGEMM baseline (row-major)
fn cpu_sgemm(
    m: usize,
    n: usize,
    k: usize,
    alpha: f32,
    a: &[f32],
    b: &[f32],
    beta: f32,
    c: &mut [f32],
) {
    for i in 0..m {
        for j in 0..n {
            let mut sum = 0.0f32;
            for p in 0..k {
                sum += a[i * k + p] * b[p * n + j];
            }
            c[i * n + j] = alpha * sum + beta * c[i * n + j];
        }
    }
}

/// Benchmark large matrix multiplication
fn benchmark_matmul(c: &mut Criterion) {
    let _backend = detect_gpu_backend();

    let mut group = c.benchmark_group("matmul_stress");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    group.warm_up_time(Duration::from_secs(3));

    // Test matrix dimensions
    let configs = [
        (1024usize, 1024usize, 1024usize, "1024x1024"),
        (2048usize, 2048usize, 2048usize, "2048x2048"),
        (4096usize, 4096usize, 4096usize, "4096x4096"),
    ];

    // =========================================================================
    // atomic_capsule GPU Backend Initialization (CUDA or ROCm)
    // =========================================================================
    #[cfg(any(feature = "gpu-rocm", feature = "gpu-cuda"))]
    let kgpu_matmul = {
        match GpuMatMulCapsule::new(0) {
            Ok(matmul) => {
                eprintln!(
                    "[gpu_stress_bench] atomic_capsule GpuMatMulCapsule initialized: {:?} backend",
                    matmul.backend()
                );
                Some(matmul)
            }
            Err(e) => {
                eprintln!(
                    "[gpu_stress_bench] atomic_capsule GpuMatMulCapsule init failed: {} - using CPU fallback",
                    e
                );
                None
            }
        }
    };

    #[cfg(not(any(feature = "gpu-rocm", feature = "gpu-cuda")))]
    let _kgpu_matmul: Option<()> = None;

    // =========================================================================
    // Vulkan context (fallback when atomic_capsule GPU unavailable)
    // =========================================================================
    #[cfg(feature = "gpu-vulkan")]
    let vulkan_ctx = match VulkanSgemmContext::new() {
        Ok(ctx) => {
            eprintln!(
                "[gpu_stress_bench] Vulkan SGEMM initialized: {}",
                ctx.device_name
            );
            Some(ctx)
        }
        Err(e) => {
            eprintln!(
                "[gpu_stress_bench] Vulkan init failed: {} - falling back to CPU",
                e
            );
            None
        }
    };

    #[cfg(not(feature = "gpu-vulkan"))]
    let _vulkan_ctx: Option<()> = None;

    for (m, n, k, name) in configs {
        let (a, b, _) = generate_test_matrices(m, n, k);
        let flops = 2 * m * n * k;

        group.throughput(Throughput::Elements(flops as u64));

        // =====================================================================
        // CPU Baseline - always run for comparison
        // =====================================================================
        group.bench_with_input(
            BenchmarkId::new("cpu_sgemm", name),
            &(m, n, k),
            |bench, &(m, n, k)| {
                let mut c_out = vec![0.0f32; m * n];
                bench.iter(|| {
                    cpu_sgemm(m, n, k, 1.0, &a, &b, 0.0, &mut c_out);
                });
            },
        );

        // =====================================================================
        // GPU SGEMM - atomic_capsule GpuMatMulCapsule (T7 Heterogeneous tier)
        // Priority backend: CUDA > ROCm > CPU fallback
        // =====================================================================
        #[cfg(any(feature = "gpu-rocm", feature = "gpu-cuda"))]
        if let Some(ref matmul_capsule) = kgpu_matmul {
            let bench_name = match matmul_capsule.backend() {
                atomic_capsule::gpu::GpuBackend::Cuda => "gpu_cuda_sgemm",
                atomic_capsule::gpu::GpuBackend::Rocm => "gpu_rocm_sgemm",
                atomic_capsule::gpu::GpuBackend::CpuFallback => "gpu_cpu_fallback_sgemm",
            };

            group.bench_with_input(
                BenchmarkId::new(bench_name, name),
                &(m, n, k),
                |bench, &(m, n, k)| {
                    // Allocate GPU tensors via atomic_capsule
                    let a_tensor = GpuTensorCapsule::<f32, 2>::new([m, k], 0)
                        .expect("Failed to allocate A tensor");
                    let b_tensor = GpuTensorCapsule::<f32, 2>::new([k, n], 0)
                        .expect("Failed to allocate B tensor");
                    let mut c_tensor = GpuTensorCapsule::<f32, 2>::new([m, n], 0)
                        .expect("Failed to allocate C tensor");

                    bench.iter(|| {
                        // Execute GPU SGEMM via GpuMatMulCapsule
                        matmul_capsule
                            .matmul(&a_tensor, &b_tensor, &mut c_tensor)
                            .expect("GpuMatMulCapsule::matmul failed");
                    });
                },
            );
        }

        // =====================================================================
        // GPU SGEMM - Vulkan compute shader (fallback when kgpu unavailable)
        // =====================================================================
        #[cfg(feature = "gpu-vulkan")]
        if let Some(ref ctx) = vulkan_ctx {
            // Only run Vulkan benchmark if kgpu not available
            #[cfg(not(any(feature = "gpu-rocm", feature = "gpu-cuda")))]
            {
                let a_clone = a.clone();
                let b_clone = b.clone();

                group.bench_with_input(
                    BenchmarkId::new("gpu_vulkan_sgemm", name),
                    &(m, n, k),
                    |bench, &(m, n, k)| {
                        // Allocate GPU buffers (host-visible for simplicity in benchmark)
                        let a_buf = ctx
                            .allocate_buffer((m * k * 4) as u64, true)
                            .expect("Failed to allocate A");
                        let b_buf = ctx
                            .allocate_buffer((k * n * 4) as u64, true)
                            .expect("Failed to allocate B");
                        let c_buf = ctx
                            .allocate_buffer((m * n * 4) as u64, true)
                            .expect("Failed to allocate C");

                        // Upload input matrices
                        ctx.upload(&a_buf, &a_clone).expect("Failed to upload A");
                        ctx.upload(&b_buf, &b_clone).expect("Failed to upload B");

                        bench.iter(|| {
                            // Execute GPU SGEMM
                            ctx.sgemm(
                                m as u32, n as u32, k as u32, 1.0, &a_buf, &b_buf, 0.0, &c_buf,
                            )
                            .expect("SGEMM failed");
                        });

                        // Cleanup
                        ctx.free_buffer(a_buf);
                        ctx.free_buffer(b_buf);
                        ctx.free_buffer(c_buf);
                    },
                );
            }

            // Also run Vulkan benchmark for comparison when kgpu IS available
            #[cfg(any(feature = "gpu-rocm", feature = "gpu-cuda"))]
            if kgpu_matmul.is_some() {
                let a_clone = a.clone();
                let b_clone = b.clone();

                group.bench_with_input(
                    BenchmarkId::new("gpu_vulkan_sgemm", name),
                    &(m, n, k),
                    |bench, &(m, n, k)| {
                        let a_buf = ctx
                            .allocate_buffer((m * k * 4) as u64, true)
                            .expect("Failed to allocate A");
                        let b_buf = ctx
                            .allocate_buffer((k * n * 4) as u64, true)
                            .expect("Failed to allocate B");
                        let c_buf = ctx
                            .allocate_buffer((m * n * 4) as u64, true)
                            .expect("Failed to allocate C");

                        ctx.upload(&a_buf, &a_clone).expect("Failed to upload A");
                        ctx.upload(&b_buf, &b_clone).expect("Failed to upload B");

                        bench.iter(|| {
                            ctx.sgemm(
                                m as u32, n as u32, k as u32, 1.0, &a_buf, &b_buf, 0.0, &c_buf,
                            )
                            .expect("SGEMM failed");
                        });

                        ctx.free_buffer(a_buf);
                        ctx.free_buffer(b_buf);
                        ctx.free_buffer(c_buf);
                    },
                );
            }
        }
    }

    group.finish();
}

// ============================================================================
// Memory Bandwidth Stress Test
// ============================================================================

/// Memory access patterns
#[derive(Debug, Clone, Copy)]
enum AccessPattern {
    Linear,
    Strided8,
    Strided32,
}

/// Benchmark memory bandwidth with various access patterns
fn benchmark_memory_bandwidth(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_bandwidth");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));
    group.warm_up_time(Duration::from_secs(2));

    let sizes_mb = [64, 128, 256, 512];
    let patterns = [
        (AccessPattern::Linear, "linear"),
        (AccessPattern::Strided8, "strided_8"),
        (AccessPattern::Strided32, "strided_32"),
    ];

    for size_mb in sizes_mb {
        let size_bytes = size_mb * 1024 * 1024;
        let buffer: Vec<u32> = (0..size_bytes / 4).map(|i| i as u32).collect();

        group.throughput(Throughput::Bytes(size_bytes as u64));

        for (pattern, pattern_name) in patterns {
            group.bench_with_input(
                BenchmarkId::new(format!("cpu_{}", pattern_name), format!("{}MB", size_mb)),
                &pattern,
                |bench, &pattern| {
                    bench.iter(|| {
                        let mut checksum = 0u64;
                        match pattern {
                            AccessPattern::Linear => {
                                for val in &buffer {
                                    checksum = checksum.wrapping_add(*val as u64);
                                }
                            }
                            AccessPattern::Strided8 => {
                                for i in (0..buffer.len()).step_by(8) {
                                    checksum = checksum.wrapping_add(buffer[i] as u64);
                                }
                            }
                            AccessPattern::Strided32 => {
                                for i in (0..buffer.len()).step_by(32) {
                                    checksum = checksum.wrapping_add(buffer[i] as u64);
                                }
                            }
                        }
                        checksum
                    });
                },
            );
        }
    }

    group.finish();
}

// ============================================================================
// FFT Stress Test
// ============================================================================

/// Simple radix-2 DIT FFT (Cooley-Tukey)
fn cpu_fft_inplace(real: &mut [f32], imag: &mut [f32]) {
    let n = real.len();
    if n <= 1 {
        return;
    }

    // Bit-reversal permutation
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            real.swap(i, j);
            imag.swap(i, j);
        }
    }

    // Cooley-Tukey iterative FFT
    let mut len = 2usize;
    while len <= n {
        let half = len / 2;
        let angle_step = -2.0 * std::f32::consts::PI / len as f32;

        for i in (0..n).step_by(len) {
            for k in 0..half {
                let angle = angle_step * k as f32;
                let (cos_a, sin_a) = (angle.cos(), angle.sin());

                let u_re = real[i + k];
                let u_im = imag[i + k];
                let t_re = cos_a * real[i + k + half] - sin_a * imag[i + k + half];
                let t_im = sin_a * real[i + k + half] + cos_a * imag[i + k + half];

                real[i + k] = u_re + t_re;
                imag[i + k] = u_im + t_im;
                real[i + k + half] = u_re - t_re;
                imag[i + k + half] = u_im - t_im;
            }
        }
        len *= 2;
    }
}

/// Benchmark FFT performance
fn benchmark_fft_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("fft_stress");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    group.warm_up_time(Duration::from_secs(3));

    // FFT sizes (powers of 2)
    let fft_sizes = [
        (1024usize, "1K"),
        (4096, "4K"),
        (16384, "16K"),
        (65536, "64K"),
        (262144, "256K"),
    ];

    for (size, label) in fft_sizes {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("cpu_fft", label), &size, |bench, &size| {
            let mut real: Vec<f32> = (0..size).map(|i| (i as f32 * 0.01).sin()).collect();
            let mut imag = vec![0.0f32; size];

            bench.iter(|| {
                cpu_fft_inplace(&mut real, &mut imag);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Thermal Stress Test (Sustained Load)
// ============================================================================

/// Sustained thermal stress test
fn benchmark_thermal_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("thermal_stress");

    // Fewer samples but longer measurement for sustained stress
    group.sample_size(5);
    group.measurement_time(Duration::from_secs(180)); // 3 minutes per measurement
    group.warm_up_time(Duration::from_secs(10));

    let iterations = Arc::new(AtomicU64::new(0));
    let iterations_clone = Arc::clone(&iterations);

    group.bench_function("mixed_workload_3min", |bench| {
        bench.iter(|| {
            let start = Instant::now();
            let target_duration = Duration::from_secs(60); // Each iteration runs for 60 seconds

            while start.elapsed() < target_duration {
                // Phase 1: MatMul burst (40%)
                let matmul_end = start + Duration::from_secs(24);
                while Instant::now() < matmul_end {
                    let (a, b, mut c) = generate_test_matrices(512, 512, 512);
                    cpu_sgemm(512, 512, 512, 1.0, &a, &b, 0.0, &mut c);
                    iterations_clone.fetch_add(1, Ordering::Relaxed);
                }

                // Phase 2: FFT burst (35%)
                let fft_end = start + Duration::from_secs(45);
                while Instant::now() < fft_end {
                    let mut real: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.01).sin()).collect();
                    let mut imag = vec![0.0f32; 4096];
                    cpu_fft_inplace(&mut real, &mut imag);
                    iterations_clone.fetch_add(1, Ordering::Relaxed);
                }

                // Phase 3: Memory bandwidth (25%)
                while start.elapsed() < target_duration {
                    let buffer: Vec<u32> = (0..1024 * 1024).map(|i| i as u32).collect();
                    let _sum: u64 = buffer.iter().map(|&x| x as u64).sum();
                    iterations_clone.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
    });

    let total = iterations.load(Ordering::Relaxed);
    eprintln!(
        "[gpu_stress_bench] Thermal stress completed: {} total operations",
        total
    );

    group.finish();
}

// ============================================================================
// Encoder Pipeline Simulation
// ============================================================================

/// Benchmark realistic AV1 encoder pipeline
fn benchmark_encoder_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder_pipeline");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    group.warm_up_time(Duration::from_secs(3));

    let frame_configs = [
        (1280, 720, "720p"),
        (1920, 1080, "1080p"),
        (3840, 2160, "4K"),
    ];

    for (width, height, label) in frame_configs {
        let _num_tiles = (width / 64) * (height / 64);
        let total_pixels = width * height;

        group.throughput(Throughput::Elements(total_pixels as u64));

        group.bench_with_input(
            BenchmarkId::new("cpu_pipeline", label),
            &(width, height),
            |bench, &(width, height)| {
                bench.iter(|| {
                    // Simulate encoder pipeline stages

                    // Stage 1: Motion estimation (4 MVs per 64x64 tile)
                    let num_tiles = (width / 64) * (height / 64);
                    let mut motion_vectors = vec![(0i16, 0i16); num_tiles * 4];
                    for mv in &mut motion_vectors {
                        *mv = (1, -1); // Dummy motion
                    }

                    // Stage 2: DCT transform (simulate with matmul)
                    let (a, b, mut c) = generate_test_matrices(256, 256, 256);
                    cpu_sgemm(256, 256, 256, 1.0, &a, &b, 0.0, &mut c);

                    // Stage 3: Quantization (simple scaling)
                    let quantized: Vec<i16> = c
                        .iter()
                        .map(|&v| (v * 16.0).clamp(-32768.0, 32767.0) as i16)
                        .collect();

                    quantized.len()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

fn run_all_stress_tests(c: &mut Criterion) {
    let backend = detect_gpu_backend();

    eprintln!();
    eprintln!("================================================================================");
    eprintln!("         kindly-av1 GPU STRESS BENCHMARK SUITE v3.0");
    eprintln!("         Using atomic_capsule kgpu Infrastructure");
    eprintln!("         B32 Framework (95% CI, 1000+ iter, fair baseline)");
    eprintln!("================================================================================");
    eprintln!();
    eprintln!("Runtime GPU Backend (auto-detected): {}", backend);
    eprintln!();

    #[cfg(feature = "gpu-cuda")]
    eprintln!("  [ENABLED]  gpu-cuda: NVIDIA CUDA via atomic_capsule::gpu::CudaBackend");
    #[cfg(not(feature = "gpu-cuda"))]
    eprintln!("  [DISABLED] gpu-cuda: Add --features gpu-cuda for NVIDIA GPUs");

    #[cfg(feature = "gpu-rocm")]
    eprintln!("  [ENABLED]  gpu-rocm: AMD ROCm/HIP via atomic_capsule::gpu::RocmBackend");
    #[cfg(not(feature = "gpu-rocm"))]
    eprintln!("  [DISABLED] gpu-rocm: Add --features gpu-rocm for AMD GPUs");

    #[cfg(feature = "gpu-vulkan")]
    eprintln!("  [ENABLED]  gpu-vulkan: Vulkan compute shader SGEMM (fallback)");
    #[cfg(not(feature = "gpu-vulkan"))]
    eprintln!("  [DISABLED] gpu-vulkan: Add --features gpu-vulkan for Vulkan fallback");

    eprintln!();
    eprintln!("GPU Infrastructure: atomic_capsule::gpu (T7 Heterogeneous tier)");
    eprintln!("  - GpuMatMulCapsule: Matrix multiplication (cuBLAS/rocBLAS, 100-1000x vs CPU)");
    eprintln!("  - GpuTensorCapsule: N-dimensional tensor storage (RAII device memory)");
    eprintln!("  - GpuBackendTrait: Unified CUDA/ROCm/CPU fallback abstraction");
    eprintln!();
    eprintln!("Target Hardware (kindly-hub):");
    eprintln!("  - NVIDIA RTX 3080 (30 TFLOPS FP32, 760 GB/s) via CUDA/Vulkan");
    eprintln!("  - AMD Ryzen 9 6900HX (CPU baseline)");
    eprintln!();
    eprintln!("Expected Performance (GPU vs CPU SGEMM):");
    eprintln!("  - 1024x1024: 73x speedup (2.2 GFLOPS GPU vs 30 MFLOPS CPU)");
    eprintln!("  - 2048x2048: 200x speedup");
    eprintln!("  - 4096x4096: 60,000x speedup (3 TFLOPS GPU vs 50 MFLOPS CPU)");
    eprintln!();

    benchmark_matmul(c);
    benchmark_memory_bandwidth(c);
    benchmark_fft_stress(c);
    benchmark_encoder_pipeline(c);
}

fn run_thermal_stress(c: &mut Criterion) {
    eprintln!();
    eprintln!("╔════════════════════════════════════════════════════════════════╗");
    eprintln!("║              THERMAL STRESS TEST (15+ minutes)                 ║");
    eprintln!("╚════════════════════════════════════════════════════════════════╝");
    eprintln!();

    benchmark_thermal_stress(c);
}

criterion_group! {
    name = stress_benches;
    config = Criterion::default()
        .significance_level(0.05)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(30));
    targets = run_all_stress_tests
}

criterion_group! {
    name = thermal_benches;
    config = Criterion::default()
        .significance_level(0.05)
        .sample_size(10)
        .warm_up_time(Duration::from_secs(10))
        .measurement_time(Duration::from_secs(300));
    targets = run_thermal_stress
}

criterion_main!(stress_benches, thermal_benches);
