//! Widget modules for kindly_dedup gui_v2
//!
//! Chaos-compliant widgets using AtomicU64 state and atomic_capsule::gui integration.
//!
//! # Base Widget Capsules (Phase 3.2)
//! - `WidgetIdCapsule`: Unique widget IDs with generation counters (T0 Auditable, 64B)
//! - `WidgetStateCapsule`: Lockfree state flags (visible, enabled, focused, hovered, pressed) (T1 Atomic, 64B)
//! - `WidgetBoundsCapsule`: Lockfree geometry (x, y, width, height) (T1 Atomic, 64B)
//! - `WidgetStyleCapsule`: Lockfree styling (colors, border, padding) (T1 Atomic, 128B)
//!
//! # Application Widgets
//! - `FileInputWidget`: File selection with drag-drop
//! - `SettingsWidget`: Threshold slider and mode selection
//! - `ProgressBarCapsule`: Animated progress bar with Q16.16 precision (128B, T1+T3)
//! - `ResultsWidget`: Statistics and output display
//! - `HeaderWidget`: Title with glow animation
//! - `ErrorBoxWidget`: Error message display
//! - `LabelCapsule`: Text label with Q8.8 font sizing (192B, T1 Atomic)
//! - `ButtonCapsule`: Interactive button widget with state management (256B, T6 Mixed)
//!
//! # Theme
//! Byzantine Royal theme (purple/gold) from existing Iced GUI

// Base widget capsules (Phase 3.2 - T0/T1)
pub mod id;
pub mod state;
pub mod bounds;
pub mod style;

// Application widgets
pub mod button;
pub mod error_box;
pub mod file_input;
pub mod header;
pub mod label;
pub mod progress;
pub mod results;
pub mod settings;

// Re-export base capsules
pub use bounds::WidgetBoundsCapsule;
pub use id::WidgetIdCapsule;
pub use state::WidgetStateCapsule;
pub use style::WidgetStyleCapsule;

// Re-export application widgets
pub use button::{ButtonCapsule, ButtonColors, ButtonVertices};
pub use error_box::ErrorBoxWidget;
pub use file_input::FileInputWidget;
pub use header::HeaderWidget;
pub use label::{LabelCapsule, LabelGlyphs, TextAlignment};
pub use progress::{ProgressBarCapsule, ProgressVertices, Vertex};
pub use results::ResultsWidget;
pub use settings::SettingsWidget;

/// RGBA color (0-255 per channel)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create from hex value (e.g., 0x6C2E7C for PURPLE_DEEP)
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
            a: 255,
        }
    }

    pub const WHITE: Color = Color::rgb(255, 255, 255);
}

/// Byzantine theme colors (from existing Iced GUI)
pub mod theme {
    use super::Color;

    pub const PURPLE_DEEP: Color = Color::from_hex(0x6C2E7C);
    pub const PURPLE_ROYAL: Color = Color::from_hex(0x8C46A8);
    pub const PURPLE_LIGHT: Color = Color::from_hex(0xB366D9);
    pub const GOLD_BRIGHT: Color = Color::from_hex(0xFFD700);
    pub const GOLD_ACCENT: Color = Color::from_hex(0xFFA500);
    pub const GOLD_DARK: Color = Color::from_hex(0xCC8800);

    pub const TEXT_PRIMARY: Color = Color::WHITE;
    pub const TEXT_SECONDARY: Color = Color::rgb(204, 204, 204);
    pub const BACKGROUND: Color = Color::rgb(25, 25, 31);
    pub const SURFACE: Color = Color::rgb(38, 38, 46);
}

// ============================================================================
// WIDGET TRAIT (Phase 3.2)
// ============================================================================

use crate::gui_v2::events::GuiEvent;

/// Widget Renderer trait for rendering widgets to GPU
///
/// # Purpose
///
/// Abstracts rendering backend (OpenGL, Vulkan, wgpu, software) from widget logic.
/// Widgets call renderer methods to emit shapes, text, and images.
///
/// # Example
///
/// ```ignore
/// fn render(&self, renderer: &mut dyn WidgetRenderer) {
///     renderer.rect(self.bounds(), Color::rgb(255, 0, 0));
///     renderer.text(self.bounds(), "Hello", Color::WHITE);
/// }
/// ```
pub trait WidgetRenderer {
    /// Draw filled rectangle
    fn rect(&mut self, bounds: (u16, u16, u16, u16), color: Color);

    /// Draw rectangle with border
    fn rect_with_border(
        &mut self,
        bounds: (u16, u16, u16, u16),
        fill: Color,
        border: Color,
        border_width: u8,
    );

    /// Draw rounded rectangle
    fn rounded_rect(
        &mut self,
        bounds: (u16, u16, u16, u16),
        color: Color,
        radius: u8,
    );

    /// Draw text
    fn text(&mut self, bounds: (u16, u16, u16, u16), text: &str, color: Color);

    /// Draw line
    fn line(&mut self, x1: u16, y1: u16, x2: u16, y2: u16, color: Color, width: u8);
}

/// Widget trait for all GUI widgets
///
/// # Purpose
///
/// Defines common interface for all widgets: ID, bounds, state, style, events, rendering.
/// Enables composition, layout, and generic widget handling.
///
/// # Required Methods
///
/// - `id()`: Get unique widget ID
/// - `bounds()`: Get widget geometry
/// - `state()`: Get widget state (visible, enabled, focused, hovered, pressed)
/// - `style()`: Get widget style (colors, border, padding)
/// - `handle_event()`: Process GUI event, returns true if event consumed
/// - `render()`: Render widget to GPU via WidgetRenderer
///
/// # Example
///
/// ```ignore
/// struct MyWidget {
///     id: WidgetIdCapsule,
///     bounds: WidgetBoundsCapsule,
///     state: WidgetStateCapsule,
///     style: WidgetStyleCapsule,
/// }
///
/// impl Widget for MyWidget {
///     fn id(&self) -> u64 { self.id.id() }
///     fn bounds(&self) -> (u16, u16, u16, u16) { self.bounds.bounds() }
///     fn state(&self) -> &WidgetStateCapsule { &self.state }
///     fn style(&self) -> &WidgetStyleCapsule { &self.style }
///
///     fn handle_event(&mut self, event: &GuiEvent) -> bool {
///         match event {
///             GuiEvent::MouseMove { x, y } => {
///                 let hovered = self.bounds.contains(*x as u16, *y as u16);
///                 self.state.set_hovered(hovered);
///                 true
///             }
///             _ => false
///         }
///     }
///
///     fn render(&self, renderer: &mut dyn WidgetRenderer) {
///         let (x, y, w, h) = self.bounds();
///         let bg = self.style().background_color();
///         renderer.rect((x, y, w, h), bg);
///     }
/// }
/// ```
pub trait Widget {
    /// Get unique widget ID
    ///
    /// # Performance
    ///
    /// - **Target**: <5ns (field access)
    fn id(&self) -> u64;

    /// Get widget bounds (x, y, width, height)
    ///
    /// # Performance
    ///
    /// - **Target**: <5ns (atomic load)
    fn bounds(&self) -> (u16, u16, u16, u16);

    /// Get widget state
    ///
    /// # Performance
    ///
    /// - **Target**: <1ns (reference)
    fn state(&self) -> &WidgetStateCapsule;

    /// Get widget style
    ///
    /// # Performance
    ///
    /// - **Target**: <1ns (reference)
    fn style(&self) -> &WidgetStyleCapsule;

    /// Handle GUI event
    ///
    /// # Returns
    ///
    /// `true` if event was consumed (stops propagation), `false` otherwise
    ///
    /// # Performance
    ///
    /// - **Target**: <100ns (event dispatch + state update)
    fn handle_event(&mut self, event: &GuiEvent) -> bool;

    /// Render widget to GPU
    ///
    /// # Performance
    ///
    /// - **Target**: <1µs (emit shapes + text)
    fn render(&self, renderer: &mut dyn WidgetRenderer);
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_colors() {
        // Validate color values
        assert_eq!(theme::PURPLE_DEEP.r, 0x6C);
        assert_eq!(theme::GOLD_BRIGHT.g, 0xD7);
    }

    #[test]
    fn test_module_exports() {
        // Verify all widgets are exported
        let _ = FileInputWidget::new();
        let _ = SettingsWidget::new();
        let _ = ProgressBarCapsule::new(1);
        let _ = ResultsWidget::new();
        let _ = HeaderWidget::new();
        let _ = ErrorBoxWidget::new();
        let _ = LabelCapsule::new(1, "Test");
    }

    #[test]
    fn test_color_creation() {
        let color = Color::rgb(255, 128, 64);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 64);
        assert_eq!(color.a, 255);

        let color2 = Color::from_hex(0xFF8040);
        assert_eq!(color2.r, 255);
        assert_eq!(color2.g, 128);
        assert_eq!(color2.b, 64);
    }

    #[test]
    fn test_base_capsules_exported() {
        // Verify base capsules are accessible (Phase 3.2)
        let id = WidgetIdCapsule::new();
        let state = WidgetStateCapsule::new();
        let bounds = WidgetBoundsCapsule::new(0, 0, 100, 100);
        let style = WidgetStyleCapsule::new();

        assert!(id.id() > 0);
        assert!(state.is_visible());
        assert_eq!(bounds.area(), 10000);
        assert_eq!(style.corner_radius(), 8);
    }
}
