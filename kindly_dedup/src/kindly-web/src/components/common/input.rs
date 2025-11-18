use crate::utils::{build_style, theme::*};
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputState {
    Default,
    Error,
    Success,
}

#[component]
pub fn Input(
    #[prop(optional)] placeholder: &'static str,
    #[prop(optional)] value: Signal<String>,
    #[prop(optional)] on_input: Option<Box<dyn Fn(String) + 'static>>,
    #[prop(optional)] state: InputState,
    #[prop(optional)] disabled: bool,
    #[prop(optional)] full_width: bool,
) -> impl IntoView {
    let border_color = match state {
        InputState::Default => COLOR_LIGHT_GRAY,
        InputState::Error => COLOR_ERROR,
        InputState::Success => COLOR_SUCCESS,
    };

    let focus_border_color = match state {
        InputState::Default => COLOR_BYZANTINE_ROYAL,
        InputState::Error => COLOR_ERROR,
        InputState::Success => COLOR_SUCCESS,
    };

    let padding_value = format!("{} {}", SPACING_3, SPACING_4);
    let border_value = format!("2px solid {}", border_color);
    let bg_color = if disabled { COLOR_LIGHT_GRAY } else { COLOR_WHITE };
    let opacity_value = if disabled { "0.6" } else { "1" };
    let cursor_value = if disabled { "not-allowed" } else { "text" };

    let mut styles = vec![
        ("font-family", FONT_FAMILY_PRIMARY),
        ("font-size", FONT_SIZE_BASE),
        ("color", COLOR_BLACK),
        ("padding", padding_value.as_str()),
        ("border", border_value.as_str()),
        ("border-radius", RADIUS_MD),
        ("background-color", bg_color),
        ("transition", TRANSITION_BASE),
        ("outline", "none"),
        ("opacity", opacity_value),
        ("cursor", cursor_value),
    ];

    if full_width {
        styles.push(("width", "100%"));
    }

    let style_string = build_style(&styles);

    view! {
        <input
            type="text"
            placeholder=placeholder
            value=move || value.get()
            on:input=move |e| {
                let input_value = event_target_value(&e);
                if let Some(ref callback) = on_input {
                    callback(input_value);
                }
            }
            on:focus=move |e| {
                if !disabled {
                    if let Some(target) = e.target() {
                        let _ = target.unchecked_into::<leptos::web_sys::HtmlElement>()
                            .style()
                            .set_property("border-color", focus_border_color);
                    }
                }
            }
            on:blur=move |e| {
                if !disabled {
                    if let Some(target) = e.target() {
                        let _ = target.unchecked_into::<leptos::web_sys::HtmlElement>()
                            .style()
                            .set_property("border-color", border_color);
                    }
                }
            }
            disabled=disabled
            style=style_string
        />
    }
}

impl Default for InputState {
    fn default() -> Self {
        InputState::Default
    }
}
