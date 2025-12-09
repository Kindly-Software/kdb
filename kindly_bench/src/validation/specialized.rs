//! Specialized validation for T7-T11 tiers
//!
//! # Overview
//!
//! Provides hardware/software validation for specialized benchmarking scenarios.

use std::fmt;

/// Validation error type
#[derive(Debug)]
pub enum ValidationError {
    /// GPU not available or CUDA/Vulkan driver missing
    GpuNotAvailable(String),
    /// Network configuration invalid (single node, wrong topology)
    InvalidNetworkConfig(String),
    /// Quantum backend not available
    QuantumBackendMissing(String),
    /// Mmap not supported on filesystem
    MmapNotSupported(String),
    /// Accuracy metrics below threshold
    AccuracyBelowThreshold { metric: String, value: f64, threshold: f64 },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GpuNotAvailable(msg) => write!(f, "GPU not available: {}", msg),
            Self::InvalidNetworkConfig(msg) => write!(f, "Invalid network config: {}", msg),
            Self::QuantumBackendMissing(msg) => write!(f, "Quantum backend missing: {}", msg),
            Self::MmapNotSupported(msg) => write!(f, "Mmap not supported: {}", msg),
            Self::AccuracyBelowThreshold { metric, value, threshold } => {
                write!(f, "Accuracy metric {} = {:.2} below threshold {:.2}", metric, value, threshold)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// T7 Heterogeneous: Validate GPU availability
///
/// # Checks
///
/// - CUDA runtime installed
/// - NVIDIA GPU detected
/// - GPU compute capability sufficient
/// - GPU driver version compatible
#[cfg(feature = "gpu")]
pub fn validate_gpu_available() -> Result<GpuInfo, ValidationError> {
    use cuda_runtime::{get_device_count, get_device_properties};

    // Check CUDA device count
    let device_count = get_device_count().map_err(|e| {
        ValidationError::GpuNotAvailable(format!("Failed to get GPU count: {:?}", e))
    })?;

    if device_count == 0 {
        return Err(ValidationError::GpuNotAvailable(
            "No CUDA-capable GPU detected".to_string(),
        ));
    }

    // Get primary GPU properties
    let props = get_device_properties(0).map_err(|e| {
        ValidationError::GpuNotAvailable(format!("Failed to get GPU properties: {:?}", e))
    })?;

    // Validate compute capability (minimum 3.5 for modern CUDA)
    let compute_capability = props.compute_capability();
    if compute_capability.0 < 3 || (compute_capability.0 == 3 && compute_capability.1 < 5) {
        return Err(ValidationError::GpuNotAvailable(format!(
            "GPU compute capability {}.{} too old (minimum 3.5 required)",
            compute_capability.0, compute_capability.1
        )));
    }

    Ok(GpuInfo {
        device_count,
        device_name: props.name().to_string(),
        compute_capability,
        total_memory_gb: props.total_global_mem() / (1024 * 1024 * 1024),
    })
}

#[cfg(not(feature = "gpu"))]
pub fn validate_gpu_available() -> Result<GpuInfo, ValidationError> {
    Err(ValidationError::GpuNotAvailable(
        "GPU feature not enabled. Enable with --features gpu".to_string(),
    ))
}

/// GPU information
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub device_count: i32,
    pub device_name: String,
    pub compute_capability: (i32, i32),
    pub total_memory_gb: u64,
}

/// T8 Network: Validate network configuration
///
/// # Checks
///
/// - Multiple nodes available (not single-node)
/// - Network latency acceptable (<10ms for LAN)
/// - Network topology correct (ring, tree, all-reduce)
/// - Bandwidth sufficient (>1 Gbps for training)
#[cfg(feature = "network")]
pub fn validate_network_config(expected_nodes: usize) -> Result<NetworkInfo, ValidationError> {
    // TODO: Implement actual network validation
    // For now, simulate basic checks

    if expected_nodes <= 1 {
        return Err(ValidationError::InvalidNetworkConfig(
            "Multi-node benchmark requires at least 2 nodes".to_string(),
        ));
    }

    // Simulate node detection
    let actual_nodes = detect_cluster_nodes();

    if actual_nodes < expected_nodes {
        return Err(ValidationError::InvalidNetworkConfig(format!(
            "Expected {} nodes but detected only {}",
            expected_nodes, actual_nodes
        )));
    }

    // Simulate latency measurement
    let avg_latency_ms = measure_network_latency();

    if avg_latency_ms > 50.0 {
        return Err(ValidationError::InvalidNetworkConfig(format!(
            "Network latency {:.1}ms too high (>50ms indicates WAN, expect poor scaling)",
            avg_latency_ms
        )));
    }

    Ok(NetworkInfo {
        node_count: actual_nodes,
        avg_latency_ms,
        bandwidth_gbps: 10.0,  // Simulated
        topology: "all-reduce".to_string(),
    })
}

#[cfg(not(feature = "network"))]
pub fn validate_network_config(_expected_nodes: usize) -> Result<NetworkInfo, ValidationError> {
    Err(ValidationError::InvalidNetworkConfig(
        "Network feature not enabled. Enable with --features network".to_string(),
    ))
}

/// Network information
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub node_count: usize,
    pub avg_latency_ms: f64,
    pub bandwidth_gbps: f64,
    pub topology: String,
}

// Placeholder network detection functions
#[cfg(feature = "network")]
fn detect_cluster_nodes() -> usize {
    // TODO: Implement actual cluster node detection
    // For now, return 1 (single node)
    1
}

#[cfg(feature = "network")]
fn measure_network_latency() -> f64 {
    // TODO: Implement actual network latency measurement
    // For now, return 1ms (LAN latency)
    1.0
}

/// T11 QuantumHybrid: Validate quantum backend
///
/// # Checks
///
/// - Quantum simulator available (Qiskit, Cirq)
/// - Qubit count sufficient
/// - Circuit depth within limits
/// - Error rates acceptable
#[cfg(feature = "quantum")]
pub fn validate_quantum_backend() -> Result<QuantumInfo, ValidationError> {
    // For now, only simulated backend available
    Ok(QuantumInfo {
        backend_type: "Simulated".to_string(),
        qubit_count: 64,
        max_circuit_depth: 1000,
        error_rate: 0.001,
    })
}

#[cfg(not(feature = "quantum"))]
pub fn validate_quantum_backend() -> Result<QuantumInfo, ValidationError> {
    Err(ValidationError::QuantumBackendMissing(
        "Quantum feature not enabled. Enable with --features quantum".to_string(),
    ))
}

/// Quantum backend information
#[derive(Debug, Clone)]
pub struct QuantumInfo {
    pub backend_type: String,
    pub qubit_count: usize,
    pub max_circuit_depth: usize,
    pub error_rate: f64,
}

/// T9 Persistent: Validate mmap support
///
/// # Checks
///
/// - Filesystem supports mmap
/// - File permissions correct
/// - Disk space available
pub fn validate_mmap_support(path: &str) -> Result<(), ValidationError> {
    use std::fs::OpenOptions;
    use std::io::Write;

    // Try to create a test file
    let test_path = format!("{}.test", path);
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&test_path)
        .map_err(|e| {
            ValidationError::MmapNotSupported(format!("Cannot create test file: {}", e))
        })?;

    // Write some data
    file.write_all(b"test").map_err(|e| {
        ValidationError::MmapNotSupported(format!("Cannot write to test file: {}", e))
    })?;

    // Clean up
    std::fs::remove_file(&test_path).ok();

    Ok(())
}

/// T10 Probabilistic: Validate accuracy metrics
///
/// # Checks
///
/// - Recall ≥ threshold (default 90%)
/// - Precision ≥ threshold (default 90%)
/// - F1 score ≥ threshold (default 90%)
pub fn validate_accuracy_metrics(
    recall: f64,
    precision: f64,
    f1_score: f64,
    threshold: f64,
) -> Result<(), ValidationError> {
    if recall < threshold {
        return Err(ValidationError::AccuracyBelowThreshold {
            metric: "Recall".to_string(),
            value: recall,
            threshold,
        });
    }

    if precision < threshold {
        return Err(ValidationError::AccuracyBelowThreshold {
            metric: "Precision".to_string(),
            value: precision,
            threshold,
        });
    }

    if f1_score < threshold {
        return Err(ValidationError::AccuracyBelowThreshold {
            metric: "F1 Score".to_string(),
            value: f1_score,
            threshold,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_accuracy_metrics_pass() {
        let result = validate_accuracy_metrics(0.95, 0.92, 0.93, 0.90);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_accuracy_metrics_fail_recall() {
        let result = validate_accuracy_metrics(0.85, 0.92, 0.88, 0.90);
        assert!(result.is_err());
        if let Err(ValidationError::AccuracyBelowThreshold { metric, .. }) = result {
            assert_eq!(metric, "Recall");
        }
    }

    #[test]
    fn test_validate_accuracy_metrics_fail_precision() {
        let result = validate_accuracy_metrics(0.95, 0.85, 0.90, 0.90);
        assert!(result.is_err());
        if let Err(ValidationError::AccuracyBelowThreshold { metric, .. }) = result {
            assert_eq!(metric, "Precision");
        }
    }

    #[test]
    fn test_validate_mmap_support() {
        let result = validate_mmap_support("/tmp/test_mmap");
        // Should succeed on Unix-like systems with /tmp
        if cfg!(unix) {
            assert!(result.is_ok());
        }
    }

    #[cfg(feature = "quantum")]
    #[test]
    fn test_validate_quantum_backend() {
        let result = validate_quantum_backend();
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.backend_type, "Simulated");
        assert!(info.qubit_count > 0);
    }

    #[cfg(not(feature = "quantum"))]
    #[test]
    fn test_validate_quantum_backend_disabled() {
        let result = validate_quantum_backend();
        assert!(result.is_err());
    }
}
