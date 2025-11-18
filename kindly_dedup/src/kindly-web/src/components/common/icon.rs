use crate::utils::{build_style, theme::*};
use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IconSize {
    Small,
    Medium,
    Large,
}

#[component]
pub fn Icon(
    #[prop(optional)] size: IconSize,
    #[prop(optional)] color: Option<&'static str>,
    children: Children,
) -> impl IntoView {
    let size_value = match size {
        IconSize::Small => "16px",
        IconSize::Medium => "24px",
        IconSize::Large => "32px",
    };

    let color_value = color.unwrap_or(COLOR_BYZANTINE_ROYAL);

    let styles = vec![
        ("width", size_value),
        ("height", size_value),
        ("display", "inline-flex"),
        ("align-items", "center"),
        ("justify-content", "center"),
        ("color", color_value),
        ("fill", "currentColor"),
    ];

    let style_string = build_style(&styles);

    view! {
        <span
            style=style_string
            aria-hidden="true"
        >
            {children()}
        </span>
    }
}

impl Default for IconSize {
    fn default() -> Self {
        IconSize::Medium
    }
}
