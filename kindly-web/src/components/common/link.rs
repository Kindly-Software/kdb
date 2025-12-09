use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use crate::utils::{theme::*, build_style};

#[component]
pub fn Link(
    href: &'static str,
    #[prop(optional)] active: bool,
    #[prop(optional)] underline: bool,
    children: Children,
) -> impl IntoView {
    let text_color = if active {
        COLOR_GOLD_BRIGHT
    } else {
        COLOR_BYZANTINE_ROYAL
    };

    let hover_color = COLOR_GOLD_BRIGHT;
    let decoration = if underline { "underline" } else { "none" };
    let font_weight = "500"; // FONT_WEIGHT_MEDIUM as string

    let styles = vec![
        ("font-family", FONT_FAMILY_PRIMARY),
        ("font-size", FONT_SIZE_BASE),
        ("font-weight", font_weight),
        ("color", text_color),
        ("text-decoration", decoration),
        ("transition", TRANSITION_FAST),
        ("cursor", "pointer"),
    ];

    let style_string = build_style(&styles);

    view! {
        <a
            href=href
            style=style_string
            on:mouseenter=move |e| {
                if let Some(target) = e.target() {
                    let _ = target.unchecked_into::<web_sys::HtmlElement>()
                        .style()
                        .set_property("color", hover_color);
                }
            }
            on:mouseleave=move |e| {
                if let Some(target) = e.target() {
                    let _ = target.unchecked_into::<web_sys::HtmlElement>()
                        .style()
                        .set_property("color", text_color);
                }
            }
        >
            {children()}
        </a>
    }
}
