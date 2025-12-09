use leptos::prelude::*;
use crate::utils::theme::*;
use crate::utils::glassmorphism::{gold_button_style, purple_button_style};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonVariant {
    Primary,   // Gold gradient
    Secondary, // Purple glass
    Light,     // Light purple
    Tertiary,  // Transparent
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonSize {
    Small,
    Medium,
    Large,
}

#[component]
pub fn Button(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional)] disabled: bool,
    #[prop(optional)] full_width: bool,
    #[prop(optional)] on_click: Option<Box<dyn Fn() + 'static>>,
    children: Children,
) -> impl IntoView {
    // Variant-specific premium styles
    let variant_style = match variant {
        ButtonVariant::Primary => gold_button_style(),
        ButtonVariant::Secondary => purple_button_style(),
        ButtonVariant::Light => format!(
            "background: rgba(230, 213, 245, 0.2); \
             border: 1px solid rgba(102, 51, 153, 0.3); \
             color: {}; \
             font-weight: 600; \
             transition: all 0.3s ease;",
            COLOR_BYZANTINE_DEEP
        ),
        ButtonVariant::Tertiary => format!(
            "background: transparent; \
             border: none; \
             color: {}; \
             font-weight: 600; \
             transition: all 0.3s ease;",
            COLOR_BYZANTINE_ROYAL
        ),
    };

    // Size-specific styles
    let (padding, font_size, border_radius) = match size {
        ButtonSize::Small => (
            format!("{} {}", SPACING_2, SPACING_3),
            FONT_SIZE_SM,
            RADIUS_SM,
        ),
        ButtonSize::Medium => (
            format!("{} {}", SPACING_3, SPACING_6),
            FONT_SIZE_BASE,
            RADIUS_MD,
        ),
        ButtonSize::Large => (
            format!("{} {}", SPACING_4, SPACING_8),
            FONT_SIZE_LG,
            RADIUS_MD,
        ),
    };

    let width = if full_width { "100%" } else { "auto" };
    let opacity = if disabled { "0.5" } else { "1" };
    let cursor = if disabled { "not-allowed" } else { "pointer" };

    let base_styles = format!(
        "font-family: {}; \
         cursor: {}; \
         display: inline-flex; \
         align-items: center; \
         justify-content: center; \
         text-decoration: none; \
         opacity: {}; \
         padding: {}; \
         font-size: {}; \
         border-radius: {}; \
         width: {};",
        FONT_FAMILY_PRIMARY,
        cursor,
        opacity,
        padding,
        font_size,
        border_radius,
        width
    );

    let style_string = format!("{}; {}", variant_style, base_styles);

    view! {
        <button
            style=style_string
            disabled=disabled
            on:click=move |_| {
                if !disabled {
                    if let Some(ref callback) = on_click {
                        callback();
                    }
                }
            }
        >
            {children()}
        </button>
    }
}

impl Default for ButtonVariant {
    fn default() -> Self {
        ButtonVariant::Primary
    }
}

impl Default for ButtonSize {
    fn default() -> Self {
        ButtonSize::Medium
    }
}
