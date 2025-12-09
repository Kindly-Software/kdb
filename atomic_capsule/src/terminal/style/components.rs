//! Widget Component Styles Capsule
//!
//! T1 Atomic tier for component-specific styling coordination.
//!
//! ## Design Principles
//!
//! - **UCE34 Framework**: T1 Atomic tier (<5ns component access)
//! - **Chaos Compliant**: 100% lockfree, cache-aligned 512B
//! - **Zero-Cost Styling**: Direct field access with atomic coordination
//! - **Theme Consistency**: Automatic derivation from ThemeColorsCapsule
//!
//! ## Performance
//!
//! - Component access: <5ns (direct field load)
//! - Bulk update: <100ns (12 component updates)
//! - Generation counter: <10ns (atomic increment)
//!
//! ## Example
//!
//! ```rust,ignore
//! use atomic_capsule::terminal::style::*;
//!
//! let theme = ThemeColorsCapsule::dark();
//! let components = ThemeComponentsCapsule::from_theme(&theme);
//!
//! // Fast component access (<5ns)
//! let btn = components.button(ButtonVariant::Primary);
//! println!("Button bg: #{:08X}", btn.bg);
//! ```

use super::theme::ThemeColorsCapsule;
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

// ============================================================================
// COMPONENT STYLE STRUCTURES
// ============================================================================

/// Component style with background, foreground, border
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ComponentStyle {
    /// Background color (RGBA8888)
    pub bg: u32,
    /// Foreground color (RGBA8888)
    pub fg: u32,
    /// Border color (RGBA8888)
    pub border: u32,
    /// Border radius (cells)
    pub border_radius: u8,
    /// Horizontal padding (cells)
    pub padding_h: u8,
    /// Vertical padding (cells)
    pub padding_v: u8,
    /// Reserved for alignment
    pub _reserved: u8,
}

impl ComponentStyle {
    /// Create new component style
    #[inline]
    pub const fn new(bg: u32, fg: u32, border: u32) -> Self {
        Self {
            bg,
            fg,
            border,
            border_radius: 0,
            padding_h: 1,
            padding_v: 0,
            _reserved: 0,
        }
    }

    /// Create with padding
    #[inline]
    pub const fn with_padding(mut self, h: u8, v: u8) -> Self {
        self.padding_h = h;
        self.padding_v = v;
        self
    }

    /// Create with border radius
    #[inline]
    pub const fn with_radius(mut self, radius: u8) -> Self {
        self.border_radius = radius;
        self
    }
}

/// Input field style
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct InputStyle {
    /// Background color (RGBA8888)
    pub bg: u32,
    /// Foreground color (RGBA8888)
    pub fg: u32,
    /// Border color (RGBA8888)
    pub border: u32,
    /// Placeholder text color (RGBA8888)
    pub placeholder_fg: u32,
}

impl InputStyle {
    /// Create new input style
    #[inline]
    pub const fn new(bg: u32, fg: u32, border: u32, placeholder_fg: u32) -> Self {
        Self { bg, fg, border, placeholder_fg }
    }
}

/// Panel style
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PanelStyle {
    /// Background color (RGBA8888)
    pub bg: u32,
    /// Border color (RGBA8888)
    pub border: u32,
    /// Shadow color (RGBA8888)
    pub shadow: u32,
    /// Border radius (cells)
    pub border_radius: u8,
    /// Shadow size (cells)
    pub shadow_size: u8,
    /// Reserved for alignment
    pub _reserved: [u8; 2],
}

impl PanelStyle {
    /// Create new panel style
    #[inline]
    pub const fn new(bg: u32, border: u32, shadow: u32) -> Self {
        Self {
            bg,
            border,
            shadow,
            border_radius: 0,
            shadow_size: 0,
            _reserved: [0; 2],
        }
    }
}

/// List item style
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ListItemStyle {
    /// Background color (RGBA8888)
    pub bg: u32,
    /// Foreground color (RGBA8888)
    pub fg: u32,
    /// Separator color (RGBA8888)
    pub separator: u32,
}

impl ListItemStyle {
    /// Create new list item style
    #[inline]
    pub const fn new(bg: u32, fg: u32, separator: u32) -> Self {
        Self { bg, fg, separator }
    }
}

/// Tab style
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct TabStyle {
    /// Background color (RGBA8888)
    pub bg: u32,
    /// Foreground color (RGBA8888)
    pub fg: u32,
    /// Indicator color (RGBA8888)
    pub indicator: u32,
}

impl TabStyle {
    /// Create new tab style
    #[inline]
    pub const fn new(bg: u32, fg: u32, indicator: u32) -> Self {
        Self { bg, fg, indicator }
    }
}

/// Menu item style
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct MenuItemStyle {
    /// Background color (RGBA8888)
    pub bg: u32,
    /// Foreground color (RGBA8888)
    pub fg: u32,
    /// Keyboard shortcut color (RGBA8888)
    pub shortcut_fg: u32,
}

impl MenuItemStyle {
    /// Create new menu item style
    #[inline]
    pub const fn new(bg: u32, fg: u32, shortcut_fg: u32) -> Self {
        Self { bg, fg, shortcut_fg }
    }
}

// ============================================================================
// VARIANT ENUMS
// ============================================================================

/// Button variant
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Primary action button
    Primary,
    /// Secondary action button
    Secondary,
    /// Ghost button (transparent background)
    Ghost,
    /// Danger/destructive action button
    Danger,
}

/// Input variant
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InputVariant {
    /// Default input state
    Default,
    /// Error state
    Error,
}

/// Panel variant
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PanelVariant {
    /// Default panel
    Default,
    /// Elevated panel (higher z-index)
    Elevated,
}

// ============================================================================
// THEME COMPONENTS CAPSULE
// ============================================================================

/// Theme components capsule (T1 Atomic, 512B)
///
/// Widget-specific styling with lockfree atomic coordination.
///
/// ## Memory Layout
///
/// ```text
/// [Button variants: 64B]
/// [Input variants:  32B]
/// [Panel variants:  32B]
/// [List items:      32B]
/// [Tabs:            32B]
/// [Menu:            32B]
/// [Modal:           16B]
/// [Generation:       8B]
/// [Padding:        264B]
/// Total:           512B (cache-aligned)
/// ```
///
/// ## Performance
///
/// - Component access: <5ns (direct field load)
/// - Bulk update: <100ns (12 component updates)
/// - Generation counter: <10ns (atomic increment)
#[repr(C, align(64))]
pub struct ThemeComponentsCapsule {
    // Button variants (64B: 4 × 16B)
    button_primary: ComponentStyle,
    button_secondary: ComponentStyle,
    button_ghost: ComponentStyle,
    button_danger: ComponentStyle,

    // Input variants (32B: 2 × 16B)
    input_default: InputStyle,
    input_error: InputStyle,

    // Panel variants (32B: 2 × 16B)
    panel_default: PanelStyle,
    panel_elevated: PanelStyle,

    // List items (24B: 2 × 12B)
    list_item: ListItemStyle,
    list_item_selected: ListItemStyle,

    // Tabs (24B: 2 × 12B)
    tab_default: TabStyle,
    tab_active: TabStyle,

    // Menu (24B: 2 × 12B)
    menu_item: MenuItemStyle,
    menu_item_hover: MenuItemStyle,

    // Modal (16B)
    modal_backdrop: AtomicU32,
    modal_shadow_size: AtomicU8,
    _modal_padding: [u8; 3],

    // State (8B)
    generation: AtomicU64,

    // Padding to 512B
    _padding: [u8; 264],
}

impl ThemeComponentsCapsule {
    /// Create default component styles (dark theme)
    pub const fn new() -> Self {
        // Default dark theme colors
        const PRIMARY: u32 = 0x6366F1FF;        // Indigo
        const SECONDARY: u32 = 0x8B5CF6FF;      // Purple
        const DANGER: u32 = 0xEF4444FF;         // Red
        const BG_BASE: u32 = 0x0F172AFF;        // Slate 900
        const BG_SURFACE: u32 = 0x1E293BFF;     // Slate 800
        const BG_ELEVATED: u32 = 0x334155FF;    // Slate 700
        const TEXT_PRIMARY: u32 = 0xF1F5F9FF;   // Slate 100
        const TEXT_SECONDARY: u32 = 0x94A3B8FF; // Slate 400
        const TEXT_INVERSE: u32 = 0x0F172AFF;   // Slate 900
        const BORDER_DEFAULT: u32 = 0x475569FF; // Slate 600
        const BORDER_FOCUS: u32 = 0x6366F1FF;   // Indigo

        Self {
            // Button variants
            button_primary: ComponentStyle::new(PRIMARY, TEXT_INVERSE, PRIMARY)
                .with_padding(2, 1),
            button_secondary: ComponentStyle::new(SECONDARY, TEXT_PRIMARY, SECONDARY)
                .with_padding(2, 1),
            button_ghost: ComponentStyle::new(0x00000000, TEXT_PRIMARY, 0x00000000)
                .with_padding(2, 1),
            button_danger: ComponentStyle::new(DANGER, TEXT_INVERSE, DANGER)
                .with_padding(2, 1),

            // Input variants
            input_default: InputStyle::new(BG_SURFACE, TEXT_PRIMARY, BORDER_DEFAULT, TEXT_SECONDARY),
            input_error: InputStyle::new(BG_SURFACE, TEXT_PRIMARY, DANGER, TEXT_SECONDARY),

            // Panel variants
            panel_default: PanelStyle::new(BG_SURFACE, BORDER_DEFAULT, 0x00000080),
            panel_elevated: PanelStyle::new(BG_ELEVATED, BORDER_DEFAULT, 0x000000C0),

            // List items
            list_item: ListItemStyle::new(0x00000000, TEXT_PRIMARY, BORDER_DEFAULT),
            list_item_selected: ListItemStyle::new(PRIMARY, TEXT_INVERSE, PRIMARY),

            // Tabs
            tab_default: TabStyle::new(0x00000000, TEXT_SECONDARY, 0x00000000),
            tab_active: TabStyle::new(BG_SURFACE, TEXT_PRIMARY, PRIMARY),

            // Menu
            menu_item: MenuItemStyle::new(0x00000000, TEXT_PRIMARY, TEXT_SECONDARY),
            menu_item_hover: MenuItemStyle::new(BG_ELEVATED, TEXT_PRIMARY, TEXT_SECONDARY),

            // Modal
            modal_backdrop: AtomicU32::new(0x00000080), // Semi-transparent black
            modal_shadow_size: AtomicU8::new(2),
            _modal_padding: [0; 3],

            // State
            generation: AtomicU64::new(0),

            // Padding
            _padding: [0; 264],
        }
    }

    /// Derive component styles from theme colors
    pub fn from_theme(theme: &ThemeColorsCapsule) -> Self {
        let primary = theme.primary();
        let secondary = theme.secondary();
        let error = theme.error();
        let bg_surface = theme.bg_surface();
        let bg_elevated = theme.bg_elevated();
        let text_primary = theme.text_primary();
        let text_secondary = theme.text_secondary();
        let text_inverse = theme.text_inverse();
        let border_default = theme.border_default();
        let border_focus = theme.border_focus();

        Self {
            // Button variants
            button_primary: ComponentStyle::new(primary, text_inverse, primary)
                .with_padding(2, 1),
            button_secondary: ComponentStyle::new(secondary, text_primary, secondary)
                .with_padding(2, 1),
            button_ghost: ComponentStyle::new(0x00000000, text_primary, 0x00000000)
                .with_padding(2, 1),
            button_danger: ComponentStyle::new(error, text_inverse, error)
                .with_padding(2, 1),

            // Input variants
            input_default: InputStyle::new(bg_surface, text_primary, border_default, text_secondary),
            input_error: InputStyle::new(bg_surface, text_primary, error, text_secondary),

            // Panel variants
            panel_default: PanelStyle::new(bg_surface, border_default, 0x00000080),
            panel_elevated: PanelStyle::new(bg_elevated, border_default, 0x000000C0),

            // List items
            list_item: ListItemStyle::new(0x00000000, text_primary, border_default),
            list_item_selected: ListItemStyle::new(primary, text_inverse, primary),

            // Tabs
            tab_default: TabStyle::new(0x00000000, text_secondary, 0x00000000),
            tab_active: TabStyle::new(bg_surface, text_primary, border_focus),

            // Menu
            menu_item: MenuItemStyle::new(0x00000000, text_primary, text_secondary),
            menu_item_hover: MenuItemStyle::new(bg_elevated, text_primary, text_secondary),

            // Modal
            modal_backdrop: AtomicU32::new(0x00000080),
            modal_shadow_size: AtomicU8::new(2),
            _modal_padding: [0; 3],

            // State
            generation: AtomicU64::new(0),

            // Padding
            _padding: [0; 264],
        }
    }

    // ========================================================================
    // COMPONENT ACCESS (<5ns)
    // ========================================================================

    /// Get button style by variant
    #[inline]
    pub fn button(&self, variant: ButtonVariant) -> ComponentStyle {
        match variant {
            ButtonVariant::Primary => self.button_primary,
            ButtonVariant::Secondary => self.button_secondary,
            ButtonVariant::Ghost => self.button_ghost,
            ButtonVariant::Danger => self.button_danger,
        }
    }

    /// Get input style by variant
    #[inline]
    pub fn input(&self, variant: InputVariant) -> InputStyle {
        match variant {
            InputVariant::Default => self.input_default,
            InputVariant::Error => self.input_error,
        }
    }

    /// Get panel style by variant
    #[inline]
    pub fn panel(&self, variant: PanelVariant) -> PanelStyle {
        match variant {
            PanelVariant::Default => self.panel_default,
            PanelVariant::Elevated => self.panel_elevated,
        }
    }

    /// Get list item style
    #[inline]
    pub fn list_item(&self, selected: bool) -> ListItemStyle {
        if selected {
            self.list_item_selected
        } else {
            self.list_item
        }
    }

    /// Get tab style
    #[inline]
    pub fn tab(&self, active: bool) -> TabStyle {
        if active {
            self.tab_active
        } else {
            self.tab_default
        }
    }

    /// Get menu item style
    #[inline]
    pub fn menu_item(&self, hover: bool) -> MenuItemStyle {
        if hover {
            self.menu_item_hover
        } else {
            self.menu_item
        }
    }

    /// Get modal backdrop color
    #[inline]
    pub fn modal_backdrop(&self) -> u32 {
        self.modal_backdrop.load(Ordering::Relaxed)
    }

    /// Get modal shadow size
    #[inline]
    pub fn modal_shadow_size(&self) -> u8 {
        self.modal_shadow_size.load(Ordering::Relaxed)
    }

    // ========================================================================
    // BULK UPDATES
    // ========================================================================

    /// Apply theme colors to all components
    ///
    /// Updates all component styles to match the theme.
    /// Generation counter incremented once at the end.
    pub fn apply_theme(&mut self, theme: &ThemeColorsCapsule) {
        let primary = theme.primary();
        let secondary = theme.secondary();
        let error = theme.error();
        let bg_surface = theme.bg_surface();
        let bg_elevated = theme.bg_elevated();
        let text_primary = theme.text_primary();
        let text_secondary = theme.text_secondary();
        let text_inverse = theme.text_inverse();
        let border_default = theme.border_default();
        let border_focus = theme.border_focus();

        // Update all components
        self.button_primary = ComponentStyle::new(primary, text_inverse, primary)
            .with_padding(2, 1);
        self.button_secondary = ComponentStyle::new(secondary, text_primary, secondary)
            .with_padding(2, 1);
        self.button_ghost = ComponentStyle::new(0x00000000, text_primary, 0x00000000)
            .with_padding(2, 1);
        self.button_danger = ComponentStyle::new(error, text_inverse, error)
            .with_padding(2, 1);

        self.input_default = InputStyle::new(bg_surface, text_primary, border_default, text_secondary);
        self.input_error = InputStyle::new(bg_surface, text_primary, error, text_secondary);

        self.panel_default = PanelStyle::new(bg_surface, border_default, 0x00000080);
        self.panel_elevated = PanelStyle::new(bg_elevated, border_default, 0x000000C0);

        self.list_item = ListItemStyle::new(0x00000000, text_primary, border_default);
        self.list_item_selected = ListItemStyle::new(primary, text_inverse, primary);

        self.tab_default = TabStyle::new(0x00000000, text_secondary, 0x00000000);
        self.tab_active = TabStyle::new(bg_surface, text_primary, border_focus);

        self.menu_item = MenuItemStyle::new(0x00000000, text_primary, text_secondary);
        self.menu_item_hover = MenuItemStyle::new(bg_elevated, text_primary, text_secondary);

        // Increment generation once
        self.generation.fetch_add(1, Ordering::Release);
    }

    // ========================================================================
    // CUSTOM OVERRIDES
    // ========================================================================

    /// Set button style for variant
    pub fn set_button(&mut self, variant: ButtonVariant, style: ComponentStyle) {
        match variant {
            ButtonVariant::Primary => self.button_primary = style,
            ButtonVariant::Secondary => self.button_secondary = style,
            ButtonVariant::Ghost => self.button_ghost = style,
            ButtonVariant::Danger => self.button_danger = style,
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set input style for variant
    pub fn set_input(&mut self, variant: InputVariant, style: InputStyle) {
        match variant {
            InputVariant::Default => self.input_default = style,
            InputVariant::Error => self.input_error = style,
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set panel style for variant
    pub fn set_panel(&mut self, variant: PanelVariant, style: PanelStyle) {
        match variant {
            PanelVariant::Default => self.panel_default = style,
            PanelVariant::Elevated => self.panel_elevated = style,
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set list item style
    pub fn set_list_item(&mut self, selected: bool, style: ListItemStyle) {
        if selected {
            self.list_item_selected = style;
        } else {
            self.list_item = style;
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set tab style
    pub fn set_tab(&mut self, active: bool, style: TabStyle) {
        if active {
            self.tab_active = style;
        } else {
            self.tab_default = style;
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set menu item style
    pub fn set_menu_item(&mut self, hover: bool, style: MenuItemStyle) {
        if hover {
            self.menu_item_hover = style;
        } else {
            self.menu_item = style;
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set modal backdrop color
    pub fn set_modal_backdrop(&self, color: u32) {
        self.modal_backdrop.store(color, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set modal shadow size
    pub fn set_modal_shadow_size(&self, size: u8) {
        self.modal_shadow_size.store(size, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    // ========================================================================
    // STATE
    // ========================================================================

    /// Get generation counter (tracks style updates)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

impl Default for ThemeComponentsCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// COMPILE-TIME VERIFICATION
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size() {
        assert_eq!(core::mem::size_of::<ThemeComponentsCapsule>(), 512);
        assert_eq!(core::mem::align_of::<ThemeComponentsCapsule>(), 64);
    }

    #[test]
    fn test_default_styles() {
        let components = ThemeComponentsCapsule::new();

        // Button primary
        let btn = components.button(ButtonVariant::Primary);
        assert_eq!(btn.bg, 0x6366F1FF);
        assert_eq!(btn.padding_h, 2);
        assert_eq!(btn.padding_v, 1);

        // Input default
        let input = components.input(InputVariant::Default);
        assert_eq!(input.bg, 0x1E293BFF);

        // Panel default
        let panel = components.panel(PanelVariant::Default);
        assert_eq!(panel.bg, 0x1E293BFF);
    }

    #[test]
    fn test_from_theme() {
        let theme = ThemeColorsCapsule::byzantine_dark();
        let components = ThemeComponentsCapsule::from_theme(&theme);

        let btn = components.button(ButtonVariant::Primary);
        // Byzantine purple primary
        assert_eq!(btn.bg, theme.primary());
    }

    #[test]
    fn test_component_access() {
        let components = ThemeComponentsCapsule::new();

        // All variants
        let _primary = components.button(ButtonVariant::Primary);
        let _secondary = components.button(ButtonVariant::Secondary);
        let _ghost = components.button(ButtonVariant::Ghost);
        let _danger = components.button(ButtonVariant::Danger);

        let _default_input = components.input(InputVariant::Default);
        let _error_input = components.input(InputVariant::Error);

        let _default_panel = components.panel(PanelVariant::Default);
        let _elevated_panel = components.panel(PanelVariant::Elevated);

        let _list = components.list_item(false);
        let _list_sel = components.list_item(true);

        let _tab = components.tab(false);
        let _tab_active = components.tab(true);

        let _menu = components.menu_item(false);
        let _menu_hover = components.menu_item(true);
    }

    #[test]
    fn test_generation_counter() {
        let mut components = ThemeComponentsCapsule::new();
        let gen1 = components.generation();

        components.set_button(
            ButtonVariant::Primary,
            ComponentStyle::new(0xFF0000FF, 0xFFFFFFFF, 0xFF0000FF),
        );
        let gen2 = components.generation();
        assert_eq!(gen2, gen1 + 1);

        components.set_input(
            InputVariant::Default,
            InputStyle::new(0x00FF00FF, 0xFFFFFFFF, 0x00FF00FF, 0x808080FF),
        );
        let gen3 = components.generation();
        assert_eq!(gen3, gen2 + 1);
    }

    #[test]
    fn test_bulk_apply_theme() {
        let theme = ThemeColorsCapsule::byzantine_dark();
        let mut components = ThemeComponentsCapsule::new();
        let gen1 = components.generation();

        components.apply_theme(&theme);
        let gen2 = components.generation();

        // Generation incremented once
        assert_eq!(gen2, gen1 + 1);

        // Verify colors updated
        let btn = components.button(ButtonVariant::Primary);
        assert_eq!(btn.bg, theme.primary());
    }

    #[test]
    fn test_modal_settings() {
        let components = ThemeComponentsCapsule::new();

        let backdrop = components.modal_backdrop();
        assert_eq!(backdrop, 0x00000080);

        let shadow_size = components.modal_shadow_size();
        assert_eq!(shadow_size, 2);
    }
}
