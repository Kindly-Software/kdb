use leptos::prelude::*;
use crate::utils::theme::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonVariant {
    Primary,   // Gold
    Secondary, // Purple
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
    // Variant-specific styles
    let (bg_color, text_color, border_radius) = match variant {
        ButtonVariant::Primary => (
            COLOR_GOLD_BRIGHT,
            COLOR_BLACK,
            RADIUS_MD,
        ),
        ButtonVariant::Secondary => (
            COLOR_BYZANTINE_ROYAL,
            COLOR_WHITE,
            RADIUS_MD,
        ),
        ButtonVariant::Light => (
            COLOR_LIGHT_GRAY,
            COLOR_BYZANTINE_DEEP,
            RADIUS_MD,
        ),
        ButtonVariant::Tertiary => (
            "transparent",
            COLOR_BYZANTINE_ROYAL,
            RADIUS_SM,
        ),
    };

    // Size-specific styles
    let (padding, font_size) = match size {
        ButtonSize::Small => (
            format!("{} {}", SPACING_2, SPACING_3),
            FONT_SIZE_SM,
        ),
        ButtonSize::Medium => (
            format!("{} {}", SPACING_3, SPACING_6),
            FONT_SIZE_BASE,
        ),
        ButtonSize::Large => (
            format!("{} {}", SPACING_4, SPACING_8),
            FONT_SIZE_LG,
        ),
    };

    let font_weight = FONT_WEIGHT_SEMIBOLD.to_string();
    let width = if full_width { "100%" } else { "auto" };
    let opacity = if disabled { "0.5" } else { "1" };
    let cursor = if disabled { "not-allowed" } else { "pointer" };

    let styles = vec![
        ("font-family", FONT_FAMILY_PRIMARY.to_string()),
        ("font-weight", font_weight),
        ("border", "none".to_string()),
        ("cursor", cursor.to_string()),
        ("transition", TRANSITION_BASE.to_string()),
        ("display", "inline-flex".to_string()),
        ("align-items", "center".to_string()),
        ("justify-content", "center".to_string()),
        ("text-decoration", "none".to_string()),
        ("opacity", opacity.to_string()),
        ("background-color", bg_color.to_string()),
        ("color", text_color.to_string()),
        ("border-radius", border_radius.to_string()),
        ("padding", padding),
        ("font-size", font_size.to_string()),
        ("width", width.to_string()),
    ];

    let style_string = styles
        .iter()
        .map(|(key, value)| format!("{}: {};", key, value))
        .collect::<Vec<_>>()
        .join(" ");

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
