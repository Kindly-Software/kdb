use crate::utils::{build_style, theme::*};
use leptos::prelude::*;

#[component]
pub fn Tooltip(text: &'static str, children: Children) -> impl IntoView {
    let (is_visible, set_visible) = signal(false);

    let padding_value = format!("{} {}", SPACING_2, SPACING_3);
    let opacity_value = if is_visible.get() { "1" } else { "0" };
    let visibility_value = if is_visible.get() { "visible" } else { "hidden" };
    let border_top_value = format!("6px solid {}", COLOR_DARK_GRAY);

    let container_styles = vec![("position", "relative"), ("display", "inline-flex")];

    let tooltip_styles = vec![
        ("position", "absolute"),
        ("bottom", "100%"),
        ("left", "50%"),
        ("transform", "translateX(-50%)"),
        ("margin-bottom", SPACING_2),
        ("padding", padding_value.as_str()),
        ("background-color", COLOR_DARK_GRAY),
        ("color", COLOR_WHITE),
        ("font-family", FONT_FAMILY_PRIMARY),
        ("font-size", FONT_SIZE_SM),
        ("border-radius", RADIUS_SM),
        ("white-space", "nowrap"),
        ("pointer-events", "none"),
        ("opacity", opacity_value),
        ("visibility", visibility_value),
        ("transition", "opacity 150ms ease-in-out, visibility 150ms ease-in-out"),
        ("z-index", "1000"),
    ];

    let arrow_styles = vec![
        ("position", "absolute"),
        ("top", "100%"),
        ("left", "50%"),
        ("transform", "translateX(-50%)"),
        ("width", "0"),
        ("height", "0"),
        ("border-left", "6px solid transparent"),
        ("border-right", "6px solid transparent"),
        ("border-top", border_top_value.as_str()),
    ];

    let container_style_string = build_style(&container_styles);
    let tooltip_style_string = build_style(&tooltip_styles);
    let arrow_style_string = build_style(&arrow_styles);

    view! {
        <div
            style=container_style_string
            on:mouseenter=move |_| set_visible.set(true)
            on:mouseleave=move |_| set_visible.set(false)
        >
            {children()}
            <div style=tooltip_style_string role="tooltip">
                {text}
                <div style=arrow_style_string></div>
            </div>
        </div>
    }
}
