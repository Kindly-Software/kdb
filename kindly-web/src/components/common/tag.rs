use leptos::prelude::*;
use crate::utils::{theme::*, build_style};

#[component]
pub fn Tag(
    #[prop(optional)] dismissible: bool,
    #[prop(optional)] on_dismiss: Option<Box<dyn Fn() + 'static>>,
    children: Children,
) -> impl IntoView {
    let padding_value = format!("{} {}", SPACING_1, SPACING_3);
    let font_weight = "500"; // FONT_WEIGHT_MEDIUM as string

    let styles = vec![
        ("font-family", FONT_FAMILY_PRIMARY),
        ("font-size", FONT_SIZE_SM),
        ("font-weight", font_weight),
        ("color", COLOR_BYZANTINE_DEEP),
        ("background-color", COLOR_BYZANTINE_LIGHT),
        ("padding", padding_value.as_str()),
        ("border-radius", RADIUS_FULL),
        ("display", "inline-flex"),
        ("align-items", "center"),
        ("gap", SPACING_2),
    ];

    let close_button_styles = vec![
        ("background", "none"),
        ("border", "none"),
        ("color", COLOR_BYZANTINE_DEEP),
        ("cursor", "pointer"),
        ("padding", "0"),
        ("margin", "0"),
        ("font-size", FONT_SIZE_SM),
        ("line-height", "1"),
        ("display", "flex"),
        ("align-items", "center"),
        ("justify-content", "center"),
        ("transition", TRANSITION_FAST),
    ];

    let style_string = build_style(&styles);
    let close_style_string = build_style(&close_button_styles);

    view! {
        <span style=style_string>
            {children()}
            {if dismissible {
                Some(view! {
                    <button
                        style=close_style_string
                        on:click=move |_| {
                            if let Some(ref callback) = on_dismiss {
                                callback();
                            }
                        }
                        aria-label="Remove tag"
                    >
                        "×"
                    </button>
                })
            } else {
                None
            }}
        </span>
    }
}
