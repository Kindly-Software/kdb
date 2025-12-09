//! Terminal styling and animation primitives
//!
//! This module provides CSS-like styling capabilities for terminal UI:
//!
//! - **animation**: CSS transitions and animations with Q16.16 timing (T1+T3)
//! - **theme**: Semantic color theming with 24 colors (T1, Byzantine/Solarized/HighContrast)
//! - **components**: Widget-specific styling (T1, buttons/inputs/panels/etc.)
//! - **uniforms**: GPU shader uniform upload with lockfree CPU-GPU sync (T7)
//! - **sheet**: StyleSheetCapsule for CSS rule parsing and matching (T0+T1)

pub mod animation;
pub mod types;

#[cfg(feature = "terminal-gpu")]
pub mod theme;
#[cfg(feature = "terminal-gpu")]
pub mod components;
#[cfg(feature = "terminal-gpu")]
pub mod uniforms;
#[cfg(feature = "terminal-gpu")]
pub mod sheet;
#[cfg(feature = "terminal-gpu")]
pub mod icons;

// Re-export Color and Rect from local types (decoupled from broken widget module)
pub use types::{Color, Rect};

pub use animation::{
    AnimationCapsule, AnimationDirection, AnimationState, AnimatedProperties,
    EasingFunction, FillMode,
};

#[cfg(feature = "terminal-gpu")]
pub use theme::{
    ThemeColorsCapsule, ThemeColor, BuiltinTheme, ThemeSnapshot,
};

#[cfg(feature = "terminal-gpu")]
pub use components::{
    ThemeComponentsCapsule,
    ComponentStyle, InputStyle, PanelStyle, ListItemStyle, TabStyle, MenuItemStyle,
    ButtonVariant, InputVariant, PanelVariant,
};

#[cfg(feature = "terminal-gpu")]
pub use uniforms::{
    StyleUniformsCapsule,
    GlobalUniforms,
    WidgetUniforms,
    f32_to_color,
    WIDGET_FLAG_FOCUSED,
    WIDGET_FLAG_HOVERED,
    WIDGET_FLAG_DISABLED,
    WIDGET_FLAG_SELECTED,
};

#[cfg(feature = "terminal-gpu")]
pub use sheet::{
    StyleSheetCapsule, StyleRule, StyleProperty, PseudoState,
    MatchedRules, StyleError, parse_selector, parse_pseudo_state, calculate_specificity,
    BorderStyle,
};
