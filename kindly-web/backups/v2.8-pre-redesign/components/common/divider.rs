use leptos::prelude::*;
use crate::utils::{theme::*, build_style};

#[component]
pub fn Divider(
    #[prop(optional)] margin: Option<&'static str>,
) -> impl IntoView {
    let default_margin = format!("{} 0", SPACING_4);
    let margin_value = margin.unwrap_or(&default_margin);
    let border_top = format!("1px solid {}", COLOR_LIGHT_GRAY);

    let styles = vec![
        ("border", "none"),
        ("border-top", border_top.as_str()),
        ("margin", margin_value),
        ("width", "100%"),
    ];

    let style_string = build_style(&styles);

    view! {
        <hr style=style_string />
    }
}
