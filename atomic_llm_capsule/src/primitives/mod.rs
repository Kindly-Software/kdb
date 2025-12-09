//! Quantization primitives module

pub mod quant_microblock;
pub mod quant_adaptive;
// TODO: Implement kv_ssd module (UCE-D7: disabled to fix compilation)
// pub mod kv_ssd;

// Additional modules to be implemented
// pub mod quant_tiered;
// pub mod gradient_compact;

// Re-export key types
pub use quant_microblock::{MicroBlockQuantCapsule, QuantizationError, QuantizedCapsule};
pub use quant_adaptive::AdaptiveQuantCapsule;
// TODO: Re-enable when kv_ssd is implemented
// pub use kv_ssd::StreamingKVCapsule;
