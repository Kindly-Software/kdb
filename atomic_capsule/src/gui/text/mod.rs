//! Text Rendering Module
//!
//! # Overview
//!
//! Text shaping, font atlas management, and rendering for GUI framework.
//!
//! # Tier Classification
//!
//! - **T1 (Atomic)**: Font atlas management, text shaping coordination
//! - **T3 (Fixed-Point)**: Q16.16 glyph positioning
//! - **T7 (Heterogeneous)**: GPU texture atlas, shader-based rendering
//!
//! # Modules
//!
//! - `glyph_cache`: GlyphCacheCapsule (T1, lockfree glyph metrics cache)
//! - `shaping`: TextShapingCapsule (T1+T3, simple text shaping)
//! - `font_atlas`: FontAtlasCapsule (T1+T7, GPU texture atlas management)
//!
//! # Future Modules
//!
//! - `font`: FontCapsule (font loading and metrics)
//! - `rasterizer`: GlyphRasterizer (CPU rasterization)
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 (T1+T3+T7 tier selection)
//! - **Chaos**: 100% lockfree, cache-aligned atomics
//! - **ASSUM**: 99.99%+ safe
//! - **T28**: Comprehensive unit/property/concurrent testing

pub mod font_atlas;
pub mod glyph_cache;
pub mod shaping;

pub use font_atlas::{AtlasRegion, FontAtlasCapsule, RegionFlags};
pub use glyph_cache::{GlyphCacheCapsule, GlyphFlags, GlyphKey, GlyphMetrics};
pub use shaping::{ShapedGlyph, ShapedGlyphFlags, TextShapingCapsule};
