//! Adaptive Hardware Detection
//!
//! **Architecture:** Auto-detect CPU, RAM, GPU, NPU and optimize computation graph
//! **Performance:** Uses ALL available resources (CPU+RAM+GPU simultaneously)
//! **Framework:** UCE34 Q11 (Rust transforms), Q12 (Nightly features)

use crate::error::Result;

/// Hardware capabilities
#[derive(Debug, Clone)]
pub struct HardwareInfo {
    /// CPU cores
    pub cpu_cores: usize,
    /// SIMD width (8, 16, etc.)
    pub simd_width: usize,
    /// RAM capacity (GB)
    pub ram_gb: usize,
    /// L3 cache size (MB)
    pub l3_cache_mb: usize,
    /// GPU detected
    pub has_gpu: bool,
    /// GPU memory (GB)
    pub gpu_memory_gb: Option<usize>,
    /// NPU detected (Apple Neural Engine, Intel Movidius)
    pub has_npu: bool,
}

/// Execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// CPU-only (no GPU)
    CpuOnly,
    /// Hybrid CPU+GPU
    Hybrid,
    /// Distributed multi-node
    Distributed,
}

/// Hardware detector
pub struct HardwareDetector;

impl HardwareDetector {
    /// Detect hardware capabilities
    pub fn detect() -> Result<HardwareInfo> {
        // To be implemented in Phase 1 (Month 5)
        unimplemented!("Hardware detection will be implemented in Phase 1")
    }

    /// Choose optimal execution mode
    pub fn choose_execution_mode(_info: &HardwareInfo) -> ExecutionMode {
        // To be implemented in Phase 1 (Month 5)
        ExecutionMode::CpuOnly  // Default fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_modes() {
        let mode = ExecutionMode::CpuOnly;
        assert_eq!(mode, ExecutionMode::CpuOnly);
    }
}
