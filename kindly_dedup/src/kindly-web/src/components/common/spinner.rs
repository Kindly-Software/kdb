use crate::utils::{build_style, theme::*};
use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpinnerSize {
    Small,
    Medium,
    Large,
}

#[component]
pub fn Spinner(#[prop(optional)] size: SpinnerSize, #[prop(optional)] color: Option<&'static str>) -> impl IntoView {
    let size_value = match size {
        SpinnerSize::Small => "16px",
        SpinnerSize::Medium => "32px",
        SpinnerSize::Large => "48px",
    };

    let color_value = color.unwrap_or(COLOR_BYZANTINE_ROYAL);

    let container_styles = vec![
        ("display", "inline-flex"),
        ("align-items", "center"),
        ("justify-content", "center"),
    ];

    let border_value = format!("3px solid {}", COLOR_LIGHT_GRAY);

    let spinner_styles = vec![
        ("width", size_value),
        ("height", size_value),
        ("border", border_value.as_str()),
        ("border-top-color", color_value),
        ("border-radius", RADIUS_FULL),
        ("animation", "spin 0.8s linear infinite"),
    ];

    let container_style_string = build_style(&container_styles);
    let spinner_style_string = build_style(&spinner_styles);

    // Inject keyframes animation
    let keyframes = r#"
        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }
    "#;

    view! {
        <style>{keyframes}</style>
        <div style=container_style_string aria-label="Loading">
            <div style=spinner_style_string></div>
        </div>
    }
}

impl Default for SpinnerSize {
    fn default() -> Self {
        SpinnerSize::Medium
    }
}
