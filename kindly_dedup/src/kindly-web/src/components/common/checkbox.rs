use crate::utils::{build_style, theme::*};
use leptos::prelude::*;

#[component]
pub fn Checkbox(
    #[prop(optional)] label: &'static str,
    #[prop(optional)] checked: Signal<bool>,
    #[prop(optional)] on_change: Option<Box<dyn Fn(bool) + 'static>>,
    #[prop(optional)] disabled: bool,
) -> impl IntoView {
    let cursor_value = if disabled { "not-allowed" } else { "pointer" };
    let opacity_value = if disabled { "0.5" } else { "1" };
    let border_value = format!("2px solid {}", COLOR_BYZANTINE_ROYAL);
    let bg_color = if checked.get() {
        COLOR_BYZANTINE_ROYAL
    } else {
        COLOR_WHITE
    };

    let container_styles = vec![
        ("display", "flex"),
        ("align-items", "center"),
        ("gap", SPACING_2),
        ("cursor", cursor_value),
        ("opacity", opacity_value),
    ];

    let checkbox_styles = vec![
        ("width", "20px"),
        ("height", "20px"),
        ("border", border_value.as_str()),
        ("border-radius", RADIUS_SM),
        ("cursor", cursor_value),
        ("transition", TRANSITION_FAST),
        ("appearance", "none"),
        ("-webkit-appearance", "none"),
        ("background-color", bg_color),
        ("position", "relative"),
    ];

    let label_styles = vec![
        ("font-family", FONT_FAMILY_PRIMARY),
        ("font-size", FONT_SIZE_BASE),
        ("color", COLOR_BLACK),
        ("user-select", "none"),
    ];

    let container_style_string = build_style(&container_styles);
    let checkbox_style_string = build_style(&checkbox_styles);
    let label_style_string = build_style(&label_styles);

    view! {
        <label style=container_style_string>
            <input
                type="checkbox"
                checked=move || checked.get()
                on:change=move |e| {
                    if !disabled {
                        let is_checked = event_target_checked(&e);
                        if let Some(ref callback) = on_change {
                            callback(is_checked);
                        }
                    }
                }
                disabled=disabled
                style=checkbox_style_string
            />
            {if !label.is_empty() {
                Some(view! { <span style=label_style_string>{label}</span> })
            } else {
                None
            }}
        </label>
    }
}
