//! Visual Effects Module
//!
//! Premium WebGL effects for the landing page:
//! - Animated mesh gradient background
//! - Floating particles
//! - Gold shimmer text effects

pub mod mesh_gradient;
pub mod particles;

pub use mesh_gradient::{MeshGradient, RenderBackend};
pub use particles::ParticleSystem;
