use leptos::prelude::*;
use crate::utils::{theme::*, build_style};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextVariant {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    Paragraph,
    Label,
    Caption,
}

#[component]
pub fn Text(
    #[prop(optional)] variant: TextVariant,
    #[prop(optional)] color: Option<&'static str>,
    #[prop(optional)] weight: Option<u16>,
    children: Children,
) -> impl IntoView {
    let variant = variant;
    let color_value = color.unwrap_or(COLOR_BLACK);

    let (tag, font_size, default_weight, line_height) = match variant {
        TextVariant::H1 => ("h1", FONT_SIZE_5XL, FONT_WEIGHT_BOLD, LINE_HEIGHT_TIGHT),
        TextVariant::H2 => ("h2", FONT_SIZE_4XL, FONT_WEIGHT_BOLD, LINE_HEIGHT_TIGHT),
        TextVariant::H3 => ("h3", FONT_SIZE_3XL, FONT_WEIGHT_SEMIBOLD, LINE_HEIGHT_TIGHT),
        TextVariant::H4 => ("h4", FONT_SIZE_2XL, FONT_WEIGHT_SEMIBOLD, LINE_HEIGHT_NORMAL),
        TextVariant::H5 => ("h5", FONT_SIZE_XL, FONT_WEIGHT_SEMIBOLD, LINE_HEIGHT_NORMAL),
        TextVariant::H6 => ("h6", FONT_SIZE_LG, FONT_WEIGHT_MEDIUM, LINE_HEIGHT_NORMAL),
        TextVariant::Paragraph => ("p", FONT_SIZE_BASE, FONT_WEIGHT_NORMAL, LINE_HEIGHT_RELAXED),
        TextVariant::Label => ("label", FONT_SIZE_SM, FONT_WEIGHT_MEDIUM, LINE_HEIGHT_NORMAL),
        TextVariant::Caption => ("span", FONT_SIZE_XS, FONT_WEIGHT_NORMAL, LINE_HEIGHT_NORMAL),
    };

    let weight_value = weight.unwrap_or(default_weight);

    let weight_str = weight_value.to_string();
    let styles = vec![
        ("font-family", FONT_FAMILY_PRIMARY),
        ("font-size", font_size),
        ("font-weight", weight_str.as_str()),
        ("color", color_value),
        ("line-height", line_height),
        ("margin", "0"),
    ];

    let style_string = build_style(&styles);

    match tag {
        "h1" => view! { <h1 style=style_string>{children()}</h1> }.into_any(),
        "h2" => view! { <h2 style=style_string>{children()}</h2> }.into_any(),
        "h3" => view! { <h3 style=style_string>{children()}</h3> }.into_any(),
        "h4" => view! { <h4 style=style_string>{children()}</h4> }.into_any(),
        "h5" => view! { <h5 style=style_string>{children()}</h5> }.into_any(),
        "h6" => view! { <h6 style=style_string>{children()}</h6> }.into_any(),
        "p" => view! { <p style=style_string>{children()}</p> }.into_any(),
        "label" => view! { <label style=style_string>{children()}</label> }.into_any(),
        "span" => view! { <span style=style_string>{children()}</span> }.into_any(),
        _ => view! { <p style=style_string>{children()}</p> }.into_any(),
    }
}

impl Default for TextVariant {
    fn default() -> Self {
        TextVariant::Paragraph
    }
}
