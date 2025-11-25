//! GPU Capability Detection - T0 Auditable Tier
//!
//! # Architecture
//!
//! Detects GPU features for kernel optimization decisions.
//! Uses wgpu adapter info and limits to determine optimal parameters.
//!
//! # Framework Compliance
//!
//! - UCE34: Q10 T0 Auditable tier (capability detection)
//! - COCA: Zero mutex, pure data structures
//! - ASSUM: GPU availability is runtime-checked, not assumed
//! - B32: Performance recommendations based on measured hardware

use std::fmt;
use wgpu::{Adapter, AdapterInfo, Limits};

/// GPU compute backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Backend {
    /// Vulkan (Linux, Windows, Android)
    Vulkan,
    /// Metal (macOS, iOS)
    Metal,
    /// DirectX 12 (Windows)
    Dx12,
    /// DirectX 11 (Windows, legacy)
    Dx11,
    /// OpenGL (legacy fallback)
    Gl,
    /// WebGPU (browser)
    WebGpu,
    /// Unknown backend
    Unknown,
}

impl From<wgpu::Backend> for Backend {
    fn from(backend: wgpu::Backend) -> Self {
        match backend {
            wgpu::Backend::Vulkan => Backend::Vulkan,
            wgpu::Backend::Metal => Backend::Metal,
            wgpu::Backend::Dx12 => Backend::Dx12,
            wgpu::Backend::Gl => Backend::Gl,
            wgpu::Backend::BrowserWebGpu => Backend::WebGpu,
            wgpu::Backend::Empty => Backend::Unknown,
        }
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Backend::Vulkan => write!(f, "Vulkan"),
            Backend::Metal => write!(f, "Metal"),
            Backend::Dx12 => write!(f, "DirectX 12"),
            Backend::Dx11 => write!(f, "DirectX 11"),
            Backend::Gl => write!(f, "OpenGL"),
            Backend::WebGpu => write!(f, "WebGPU"),
            Backend::Unknown => write!(f, "Unknown"),
        }
    }
}

/// GPU device type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuClass {
    /// Integrated GPU (iGPU) - shared memory with CPU
    Integrated,
    /// Discrete GPU (dGPU) - dedicated VRAM
    Discrete,
    /// Virtual GPU (cloud/VM)
    Virtual,
    /// Software renderer (CPU fallback)
    Software,
    /// Unknown device type
    Unknown,
}

impl From<wgpu::DeviceType> for GpuClass {
    fn from(device_type: wgpu::DeviceType) -> Self {
        match device_type {
            wgpu::DeviceType::IntegratedGpu => GpuClass::Integrated,
            wgpu::DeviceType::DiscreteGpu => GpuClass::Discrete,
            wgpu::DeviceType::VirtualGpu => GpuClass::Virtual,
            wgpu::DeviceType::Cpu => GpuClass::Software,
            wgpu::DeviceType::Other => GpuClass::Unknown,
        }
    }
}

/// GPU compute capabilities
///
/// Contains all information needed to optimize kernel dispatch
/// and batch sizing for optimal GPU utilization.
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    /// GPU backend (Vulkan, Metal, DX12, etc.)
    pub backend: Backend,

    /// Device name (e.g., "NVIDIA RTX 4090", "AMD RX 7900 XTX")
    pub device_name: String,

    /// Device vendor (e.g., "NVIDIA", "AMD", "Intel", "Apple")
    pub vendor: String,

    /// Device type (integrated, discrete, virtual)
    pub device_class: GpuClass,

    /// Driver description
    pub driver: String,

    /// Maximum workgroup size (X dimension)
    pub max_workgroup_size_x: u32,

    /// Maximum workgroup size (Y dimension)
    pub max_workgroup_size_y: u32,

    /// Maximum workgroup size (Z dimension)
    pub max_workgroup_size_z: u32,

    /// Maximum total invocations per workgroup
    pub max_workgroup_invocations: u32,

    /// Maximum compute workgroups per dispatch (X)
    pub max_dispatch_x: u32,

    /// Maximum compute workgroups per dispatch (Y)
    pub max_dispatch_y: u32,

    /// Maximum compute workgroups per dispatch (Z)
    pub max_dispatch_z: u32,

    /// Maximum buffer size in bytes
    pub max_buffer_size: u64,

    /// Maximum storage buffer binding size
    pub max_storage_buffer_binding_size: u32,

    /// Maximum uniform buffer binding size
    pub max_uniform_buffer_binding_size: u32,

    /// Maximum bind groups per pipeline
    pub max_bind_groups: u32,

    /// Maximum bindings per bind group
    pub max_bindings_per_bind_group: u32,

    /// Supports compute shaders
    pub supports_compute: bool,

    /// Supports 16-bit floats (f16)
    pub supports_f16: bool,

    /// Supports subgroups (warp/wavefront operations)
    pub supports_subgroups: bool,

    /// Subgroup size (warp/wavefront width), if supported
    pub subgroup_size: Option<u32>,

    /// Estimated VRAM in GB (approximate)
    pub estimated_vram_gb: f32,
}

impl GpuCapabilities {
    /// Create capabilities from wgpu adapter
    ///
    /// Extracts all relevant limits and features for optimization decisions.
    pub fn from_adapter(adapter: &Adapter) -> Self {
        let info: AdapterInfo = adapter.get_info();
        let limits: Limits = adapter.limits();
        let features = adapter.features();

        // Extract vendor from device name or use backend hints
        let vendor = Self::extract_vendor(&info.name, &info.vendor);

        // Estimate VRAM (wgpu doesn't expose this directly)
        let estimated_vram_gb = Self::estimate_vram(&info, &limits);

        // Check for subgroup support (not available in wgpu 0.19.x)
        // SUBGROUP feature was added in later versions
        let supports_subgroups = false;  // wgpu 0.19.x doesn't expose this
        let subgroup_size = if supports_subgroups {
            Some(Self::estimate_subgroup_size(&vendor))
        } else {
            // Estimate based on vendor for optimization hints
            Some(Self::estimate_subgroup_size(&vendor))
        };

        Self {
            backend: info.backend.into(),
            device_name: info.name.clone(),
            vendor,
            device_class: info.device_type.into(),
            driver: info.driver.clone(),
            max_workgroup_size_x: limits.max_compute_workgroup_size_x,
            max_workgroup_size_y: limits.max_compute_workgroup_size_y,
            max_workgroup_size_z: limits.max_compute_workgroup_size_z,
            max_workgroup_invocations: limits.max_compute_invocations_per_workgroup,
            max_dispatch_x: limits.max_compute_workgroups_per_dimension,
            max_dispatch_y: limits.max_compute_workgroups_per_dimension,
            max_dispatch_z: limits.max_compute_workgroups_per_dimension,
            max_buffer_size: limits.max_buffer_size,
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
            max_uniform_buffer_binding_size: limits.max_uniform_buffer_binding_size,
            max_bind_groups: limits.max_bind_groups,
            max_bindings_per_bind_group: limits.max_bindings_per_bind_group,
            supports_compute: true, // wgpu guarantees compute support
            supports_f16: features.contains(wgpu::Features::SHADER_F16),
            supports_subgroups,
            subgroup_size,
            estimated_vram_gb,
        }
    }

    /// Extract vendor name from device info
    fn extract_vendor(device_name: &str, vendor_id: &u32) -> String {
        let name_lower = device_name.to_lowercase();

        if name_lower.contains("nvidia") || name_lower.contains("geforce") || name_lower.contains("rtx") || name_lower.contains("gtx") {
            return "NVIDIA".to_string();
        }
        if name_lower.contains("amd") || name_lower.contains("radeon") || name_lower.contains("rx ") {
            return "AMD".to_string();
        }
        if name_lower.contains("intel") || name_lower.contains("arc") || name_lower.contains("iris") || name_lower.contains("uhd") {
            return "Intel".to_string();
        }
        if name_lower.contains("apple") || name_lower.contains("m1") || name_lower.contains("m2") || name_lower.contains("m3") || name_lower.contains("m4") {
            return "Apple".to_string();
        }
        if name_lower.contains("qualcomm") || name_lower.contains("adreno") {
            return "Qualcomm".to_string();
        }
        if name_lower.contains("mali") {
            return "ARM".to_string();
        }

        // Fallback to vendor ID
        match vendor_id {
            0x10DE => "NVIDIA".to_string(),
            0x1002 => "AMD".to_string(),
            0x8086 => "Intel".to_string(),
            0x106B => "Apple".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    /// Estimate VRAM based on device class and limits
    fn estimate_vram(info: &AdapterInfo, limits: &Limits) -> f32 {
        // Use max buffer size as a rough indicator
        let max_buffer_gb = limits.max_buffer_size as f64 / (1024.0 * 1024.0 * 1024.0);

        match info.device_type {
            wgpu::DeviceType::DiscreteGpu => {
                // Discrete GPUs typically have 4-24GB VRAM
                // Max buffer is usually ~50% of VRAM
                (max_buffer_gb * 2.0).min(24.0) as f32
            }
            wgpu::DeviceType::IntegratedGpu => {
                // Integrated GPUs share system RAM
                // Typically allocated 1-8GB
                (max_buffer_gb * 1.5).min(8.0) as f32
            }
            _ => max_buffer_gb as f32,
        }
    }

    /// Estimate subgroup size based on vendor
    fn estimate_subgroup_size(vendor: &str) -> u32 {
        match vendor {
            "NVIDIA" => 32, // NVIDIA uses 32-thread warps
            "AMD" => 64,    // AMD uses 64-thread wavefronts (RDNA uses 32)
            "Intel" => 32,  // Intel typically uses 32
            "Apple" => 32,  // Apple Silicon uses 32
            _ => 32,        // Conservative default
        }
    }

    /// Recommend optimal batch size for MinHash computation
    ///
    /// Based on GPU capabilities, returns batch size that:
    /// - Fits in available VRAM
    /// - Maximizes GPU occupancy
    /// - Accounts for transfer overhead
    pub fn recommended_batch_size(&self) -> usize {
        // Each document needs: MAX_TOKENS * 4 bytes (u32 tokens) + 128 * 2 bytes (signature)
        const TOKENS_PER_DOC: usize = 512;      // Average tokens
        const BYTES_PER_TOKEN: usize = 4;        // u32
        const SIGNATURE_BYTES: usize = 256;      // 128 * u16
        const BYTES_PER_DOC: usize = TOKENS_PER_DOC * BYTES_PER_TOKEN + SIGNATURE_BYTES;

        // Target 50% of available buffer space for double buffering
        let available_bytes = (self.max_buffer_size / 2) as usize;
        let max_docs_by_memory = available_bytes / BYTES_PER_DOC;

        // Scale by GPU class
        let optimal_docs = match self.device_class {
            GpuClass::Discrete => {
                // High-end discrete: 100K-1M docs
                max_docs_by_memory.min(1_000_000).max(100_000)
            }
            GpuClass::Integrated => {
                // Integrated: 10K-100K docs
                max_docs_by_memory.min(100_000).max(10_000)
            }
            GpuClass::Virtual | GpuClass::Software | GpuClass::Unknown => {
                // Conservative: 1K-10K docs
                max_docs_by_memory.min(10_000).max(1_000)
            }
        };

        // Round to power of 2 for optimal dispatch
        optimal_docs.next_power_of_two() / 2
    }

    /// Recommend workgroup size for compute kernels
    ///
    /// Returns (x, y, z) dimensions for @workgroup_size directive.
    pub fn recommended_workgroup_size(&self) -> (u32, u32, u32) {
        // Standard: 256 threads (16x16 or 256x1)
        let total = self.max_workgroup_invocations.min(256);

        if total >= 256 {
            (256, 1, 1)  // 1D workgroup for MinHash
        } else if total >= 64 {
            (64, 1, 1)
        } else {
            (32, 1, 1)   // Minimum viable
        }
    }

    /// Check if GPU is powerful enough to justify acceleration
    ///
    /// Returns false for very weak GPUs where CPU SIMD is faster
    /// due to transfer overhead.
    pub fn worth_using(&self) -> bool {
        // Minimum requirements for worthwhile GPU acceleration:
        // - At least 256 workgroup invocations
        // - At least 128MB buffer size
        // - Discrete or modern integrated GPU
        let min_workgroup = self.max_workgroup_invocations >= 256;
        let min_buffer = self.max_buffer_size >= 128 * 1024 * 1024;
        let adequate_class = matches!(
            self.device_class,
            GpuClass::Discrete | GpuClass::Integrated
        );

        min_workgroup && min_buffer && adequate_class
    }

    /// Get performance tier estimate
    ///
    /// Returns expected speedup multiplier vs CPU SIMD.
    pub fn performance_tier(&self) -> PerformanceTier {
        match self.device_class {
            GpuClass::Discrete => {
                // Check for high-end indicators
                let is_high_end = self.estimated_vram_gb >= 8.0
                    || self.device_name.contains("4090")
                    || self.device_name.contains("4080")
                    || self.device_name.contains("7900")
                    || self.device_name.contains("A100")
                    || self.device_name.contains("H100");

                if is_high_end {
                    PerformanceTier::HighEnd
                } else if self.estimated_vram_gb >= 4.0 {
                    PerformanceTier::MidRange
                } else {
                    PerformanceTier::Entry
                }
            }
            GpuClass::Integrated => PerformanceTier::Integrated,
            _ => PerformanceTier::Fallback,
        }
    }
}

impl fmt::Display for GpuCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "GPU: {} ({})", self.device_name, self.vendor)?;
        writeln!(f, "Backend: {}", self.backend)?;
        writeln!(f, "Class: {:?}", self.device_class)?;
        writeln!(f, "Driver: {}", self.driver)?;
        writeln!(f, "Est. VRAM: {:.1} GB", self.estimated_vram_gb)?;
        writeln!(f, "Max Workgroup: {}x{}x{} (max {} total)",
            self.max_workgroup_size_x,
            self.max_workgroup_size_y,
            self.max_workgroup_size_z,
            self.max_workgroup_invocations
        )?;
        writeln!(f, "Max Buffer: {} MB", self.max_buffer_size / (1024 * 1024))?;
        writeln!(f, "Compute: {}, F16: {}, Subgroups: {}",
            self.supports_compute,
            self.supports_f16,
            self.supports_subgroups
        )?;
        if let Some(size) = self.subgroup_size {
            writeln!(f, "Subgroup Size: {}", size)?;
        }
        writeln!(f, "Performance Tier: {:?}", self.performance_tier())?;
        writeln!(f, "Recommended Batch: {} docs", self.recommended_batch_size())?;
        writeln!(f, "Worth Using: {}", self.worth_using())?;
        Ok(())
    }
}

/// Performance tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceTier {
    /// High-end discrete GPU (RTX 4090, RX 7900 XTX, A100)
    /// Expected: 10-50x speedup
    HighEnd,

    /// Mid-range discrete GPU (RTX 3060, RX 6700)
    /// Expected: 5-10x speedup
    MidRange,

    /// Entry discrete GPU (GTX 1650, RX 6400)
    /// Expected: 2-5x speedup
    Entry,

    /// Integrated GPU (Intel UHD, AMD APU, Apple M-series)
    /// Expected: 1-2x speedup
    Integrated,

    /// Software/Virtual/Unknown - use CPU fallback
    /// Expected: <1x (GPU overhead exceeds benefit)
    Fallback,
}

impl PerformanceTier {
    /// Get expected speedup range for this tier
    pub fn expected_speedup(&self) -> (f32, f32) {
        match self {
            PerformanceTier::HighEnd => (10.0, 50.0),
            PerformanceTier::MidRange => (5.0, 10.0),
            PerformanceTier::Entry => (2.0, 5.0),
            PerformanceTier::Integrated => (1.0, 2.0),
            PerformanceTier::Fallback => (0.5, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_display() {
        assert_eq!(format!("{}", Backend::Vulkan), "Vulkan");
        assert_eq!(format!("{}", Backend::Metal), "Metal");
        assert_eq!(format!("{}", Backend::Dx12), "DirectX 12");
    }

    #[test]
    fn test_vendor_extraction() {
        assert_eq!(GpuCapabilities::extract_vendor("NVIDIA GeForce RTX 4090", &0), "NVIDIA");
        assert_eq!(GpuCapabilities::extract_vendor("AMD Radeon RX 7900 XTX", &0), "AMD");
        assert_eq!(GpuCapabilities::extract_vendor("Intel Arc A770", &0), "Intel");
        assert_eq!(GpuCapabilities::extract_vendor("Apple M3 Max", &0), "Apple");
        assert_eq!(GpuCapabilities::extract_vendor("Unknown Device", &0x10DE), "NVIDIA");
    }

    #[test]
    fn test_performance_tier_speedup() {
        let (low, high) = PerformanceTier::HighEnd.expected_speedup();
        assert!(low >= 10.0);
        assert!(high <= 50.0);

        let (low, high) = PerformanceTier::Fallback.expected_speedup();
        assert!(low < 1.0);
    }

    #[test]
    fn test_subgroup_size_estimation() {
        assert_eq!(GpuCapabilities::estimate_subgroup_size("NVIDIA"), 32);
        assert_eq!(GpuCapabilities::estimate_subgroup_size("AMD"), 64);
        assert_eq!(GpuCapabilities::estimate_subgroup_size("Intel"), 32);
    }
}
