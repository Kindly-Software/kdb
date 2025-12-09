use leptos::prelude::*;
use crate::utils::{theme::*, build_style};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AvatarSize {
    Small,
    Medium,
    Large,
}

#[component]
pub fn Avatar(
    #[prop(optional)] size: AvatarSize,
    #[prop(optional)] src: Option<&'static str>,
    #[prop(optional)] initials: &'static str,
    #[prop(optional)] alt: &'static str,
) -> impl IntoView {
    let size_value = match size {
        AvatarSize::Small => "32px",
        AvatarSize::Medium => "48px",
        AvatarSize::Large => "64px",
    };

    let font_size = match size {
        AvatarSize::Small => FONT_SIZE_SM,
        AvatarSize::Medium => FONT_SIZE_BASE,
        AvatarSize::Large => FONT_SIZE_XL,
    };

    let font_weight = "600"; // FONT_WEIGHT_SEMIBOLD as string

    let container_styles = vec![
        ("width", size_value),
        ("height", size_value),
        ("border-radius", RADIUS_FULL),
        ("overflow", "hidden"),
        ("display", "flex"),
        ("align-items", "center"),
        ("justify-content", "center"),
        ("background-color", COLOR_BYZANTINE_ROYAL),
        ("color", COLOR_WHITE),
        ("font-family", FONT_FAMILY_PRIMARY),
        ("font-size", font_size),
        ("font-weight", font_weight),
        ("user-select", "none"),
    ];

    let img_styles = vec![
        ("width", "100%"),
        ("height", "100%"),
        ("object-fit", "cover"),
    ];

    let container_style_string = build_style(&container_styles);
    let img_style_string = build_style(&img_styles);

    view! {
        <div style=container_style_string>
            {if let Some(image_src) = src {
                view! {
                    <img
                        src=image_src
                        alt=alt
                        style=img_style_string
                    />
                }.into_any()
            } else {
                view! {
                    <span>{initials}</span>
                }.into_any()
            }}
        </div>
    }
}

impl Default for AvatarSize {
    fn default() -> Self {
        AvatarSize::Medium
    }
}
