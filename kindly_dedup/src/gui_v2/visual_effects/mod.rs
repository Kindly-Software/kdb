// Copyright (c) 2025 Kindly Dedup Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// gui_v2/visual_effects/mod.rs - Visual Rendering Effects Module
//
// Phase 5: Cool Effects - Byzantine border, glassmorphic cards, noise texture
//
// UCE34 Compliance:
// - Q10: T2 SIMD tier for effect computation (gradient interpolation)
// - Q33: 100% lockfree (AtomicU64 for animation phase)
// - Q34: Auditable effect parameters (no hidden state)
//
// Chaos Compliance:
// - Cache-aligned capsules (64B-128B)
// - AtomicU64 for animation state coordination
// - Zero mutex (lockfree animation updates)

pub mod byzantine_border;
pub mod glassmorphic;
pub mod noise_texture;
pub mod noise;      // G4.1: Procedural noise (GPU compute)
pub mod gradient;   // G4.2: Linear/radial gradients
pub mod shadow;     // G4.3: Drop shadows with Gaussian blur

// Re-exports
pub use byzantine_border::ByzantineBorderCapsule;
pub use glassmorphic::GlassmorphicCapsule;
pub use noise_texture::NoiseTextureCapsule;
pub use noise::{NoiseEffectCapsule, NoiseParams};
pub use gradient::{GradientCapsule, GradientType, ColorStop, LinearGradientParams, RadialGradientParams};
pub use shadow::ShadowCapsule;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all capsules are exported
        let _ = std::mem::size_of::<ByzantineBorderCapsule>();
        let _ = std::mem::size_of::<GlassmorphicCapsule>();
        let _ = std::mem::size_of::<NoiseTextureCapsule>();
        let _ = std::mem::size_of::<NoiseEffectCapsule>();
        let _ = std::mem::size_of::<GradientCapsule>();
        let _ = std::mem::size_of::<ShadowCapsule>();
    }
}
