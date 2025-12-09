use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use crate::utils::{theme::*, build_style};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CardVariant {
    Default,
    Outlined,
    Elevated,
}

#[component]
pub fn Card(
    #[prop(optional)] variant: CardVariant,
    #[prop(optional)] padding: Option<&'static str>,
    children: Children,
) -> impl IntoView {
    let variant = variant;
    let padding_value = padding.unwrap_or(SPACING_6);

    let default_border = format!("1px solid {}", COLOR_LIGHT_GRAY);
    let outlined_border = format!("2px solid {}", COLOR_BYZANTINE_ROYAL);

    let mut styles = vec![
        ("font-family", FONT_FAMILY_PRIMARY),
        ("border-radius", RADIUS_LG),
        ("padding", padding_value),
        ("transition", TRANSITION_BASE),
    ];

    // Variant-specific styles
    match variant {
        CardVariant::Default => {
            styles.push(("background-color", COLOR_WHITE));
            styles.push(("border", default_border.as_str()));
            styles.push(("box-shadow", SHADOW_NONE));
        }
        CardVariant::Outlined => {
            styles.push(("background-color", "transparent"));
            styles.push(("border", outlined_border.as_str()));
            styles.push(("box-shadow", SHADOW_NONE));
        }
        CardVariant::Elevated => {
            styles.push(("background-color", COLOR_WHITE));
            styles.push(("border", "none"));
            styles.push(("box-shadow", SHADOW_LG));
        }
    }

    let style_string = build_style(&styles);

    view! {
        <div
            style=style_string
            on:mouseenter=move |e| {
                if matches!(variant, CardVariant::Elevated) {
                    if let Some(target) = e.target() {
                        let _ = target.unchecked_into::<leptos::web_sys::HtmlElement>()
                            .style()
                            .set_property("box-shadow", SHADOW_XL);
                    }
                }
            }
            on:mouseleave=move |e| {
                if matches!(variant, CardVariant::Elevated) {
                    if let Some(target) = e.target() {
                        let _ = target.unchecked_into::<leptos::web_sys::HtmlElement>()
                            .style()
                            .set_property("box-shadow", SHADOW_LG);
                    }
                }
            }
        >
            {children()}
        </div>
    }
}

impl Default for CardVariant {
    fn default() -> Self {
        CardVariant::Default
    }
}
