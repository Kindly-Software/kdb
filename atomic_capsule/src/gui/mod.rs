//! Chaos-Compliant GUI Framework
//!
//! # Overview
//!
//! A 100% lockfree, deterministic GUI framework built on computational capsule architecture.
//!
//! # Tier Classification
//!
//! - **T0 (Auditable)**: Error types, core geometry types
//! - **T1 (Atomic)**: Widget state management (future)
//! - **T2 (SIMD)**: Text rendering, image processing (future)
//! - **T3 (Fixed-Point)**: Q16.16 coordinates for deterministic layout
//! - **T5 (Streaming)**: Effect queue, event queue (lockfree ring buffer)
//! - **T7 (Heterogeneous)**: GPU-accelerated rendering (future)
//!
//! # Modules
//!
//! - `types`: Core geometric types (Point, Rect, Size, Color) with Q16.16 fixed-point
//! - `error`: GuiError types (T0 Auditable)
//! - `effect_queue`: EffectQueueCapsule (deferred effect queue, GPUI pattern, <20ns enqueue)
//! - `event_queue`: EventQueueCapsule (lockfree event handling)
//! - `text`: Text rendering (TextShapingCapsule, FontAtlasCapsule, T1+T3+T7)
//! - `theme`: ThemeCapsule (Byzantine purple + gold branding, T1 Atomic, <5ns color access)
//! - `widgets`: Widget primitives (ButtonCapsule)
//! - `render`: GPU rendering infrastructure (BufferPoolCapsule, GpuContextCapsule, T1+T7)
//!
//! # Design Principles
//!
//! - **Deterministic**: Q16.16 fixed-point for exact reproducibility
//! - **FFI-Safe**: All types are `#[repr(C)]` for cross-language usage
//! - **Cache-Aligned**: Types with atomics use 64B/128B alignment
//! - **Zero-Copy**: Designed for direct GPU buffer uploads
//! - **Lockfree**: No mutex, no Arc, atomic coordination only
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 (T0/T1/T3/T5 tier selection), Q33 (zero runtime overhead)
//! - **Chaos**: 100% lockfree, no mutex/Arc, cache-aligned atomics
//! - **ASSUM**: 99.99%+ safe (minimal unsafe for SIMD/GPU FFI)
//! - **B32**: Fair benchmarking (compare to imgui, egui, iced)
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)
//! - **I20**: Integration validation (backward compatibility)

// Temporarily disabled effect_queue due to padding overflow (Effect is 20 bytes, 128 × 20 = 2560 > 128B alignment)
// TODO: Redesign EffectQueueCapsule with larger alignment or smaller Effect enum
// pub mod effect_queue;
pub mod error;
pub mod event_queue;
pub mod layout;
pub mod render;
pub mod text;
pub mod theme;
pub mod types;
pub mod widgets;

// pub use effect_queue::{Effect, EffectQueueCapsule};
pub use error::{GuiError, GuiResult};
pub use event_queue::{EventQueueCapsule, GuiEvent, KeyCode, Modifiers, MouseButton, MouseEventKind};
pub use layout::{ContainerCapsule, LayoutConstraints, LayoutEngineCapsule, LayoutNode, Overflow};
pub use render::{BufferPoolCapsule, BufferState, GpuBackend, GpuContextCapsule, GpuState, ShapeCapsule, ShapeFlags, ShapeType};
pub use text::{
    AtlasRegion, FontAtlasCapsule, GlyphCacheCapsule, GlyphFlags, GlyphKey, GlyphMetrics, RegionFlags, ShapedGlyph,
    ShapedGlyphFlags, TextShapingCapsule,
};
pub use theme::{
    ThemeCapsule, ThemeMode,
    // Byzantine Purple Palette
    PURPLE_DEEP, PURPLE_ROYAL, PURPLE_MEDIUM, PURPLE_LIGHT,
    // Gold Palette
    GOLD_DARK, GOLD_BRIGHT, GOLD_LIGHT,
    // Neutral Palette
    BG_DARK, BG_LIGHT, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
    // Semantic Colors
    SUCCESS, WARNING, ERROR,
    // Color utilities
    rgba, from_rgba,
    // Style types
    StyleCapsule, StyleBuilder, FontWeight, TextAlign,
};
pub use types::{Color, Coord, Point, Rect, Size};
pub use widgets::{ButtonCapsule, ButtonState, ButtonStyle, PressState};

// Prelude for convenient imports
pub mod prelude {
    //! Convenient re-exports for common GUI types and traits
    // pub use super::effect_queue::{Effect, EffectQueueCapsule};
    pub use super::error::{GuiError, GuiResult};
    pub use super::event_queue::{EventQueueCapsule, GuiEvent, KeyCode, Modifiers, MouseButton, MouseEventKind};
    pub use super::layout::{ContainerCapsule, LayoutConstraints, LayoutEngineCapsule, LayoutNode, Overflow};
    pub use super::render::{BufferPoolCapsule, BufferState, GpuBackend, GpuContextCapsule, GpuState, ShapeCapsule, ShapeFlags, ShapeType};
    pub use super::text::{
        AtlasRegion, FontAtlasCapsule, GlyphCacheCapsule, GlyphFlags, GlyphKey, GlyphMetrics, RegionFlags, ShapedGlyph,
        ShapedGlyphFlags, TextShapingCapsule,
    };
    pub use super::theme::{
        ThemeCapsule, ThemeMode,
        PURPLE_DEEP, PURPLE_ROYAL, PURPLE_MEDIUM, PURPLE_LIGHT,
        GOLD_DARK, GOLD_BRIGHT, GOLD_LIGHT,
        BG_DARK, BG_LIGHT, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
        SUCCESS, WARNING, ERROR,
        rgba, from_rgba,
        StyleCapsule, StyleBuilder, FontWeight, TextAlign,
    };
    pub use super::types::{Color, Coord, Point, Rect, Size};
    pub use super::widgets::{ButtonCapsule, ButtonState, ButtonStyle, PressState};
}
