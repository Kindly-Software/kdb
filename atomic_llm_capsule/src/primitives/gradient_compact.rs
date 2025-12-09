//! Gradient compact quantization

/// Number of gradients per capsule
pub const GRADIENTS_PER_CAPSULE: usize = 64;

/// Compact gradient capsule
pub struct CompactGradientCapsule;
