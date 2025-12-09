//! Validation infrastructure for specialized tiers
//!
//! # Overview
//!
//! Phase 3 tiers require specialized validation beyond standard B32 checks:
//!
//! - **T7 Heterogeneous**: GPU availability, CUDA/Vulkan detection
//! - **T8 Network**: Multi-node setup, network topology validation
//! - **T9 Persistent**: Filesystem support, mmap capabilities
//! - **T10 Probabilistic**: Accuracy metrics (recall, precision, F1)
//! - **T11 QuantumHybrid**: Quantum backend detection

pub mod specialized;

pub use specialized::{
    validate_gpu_available, validate_network_config, validate_quantum_backend,
    validate_mmap_support, validate_accuracy_metrics,
};
