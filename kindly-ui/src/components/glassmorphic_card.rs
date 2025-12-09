//! Glassmorphic Card Component
//!
//! Reusable glassmorphic card with Byzantine purple glassmorphism.

use leptos::prelude::*;
use crate::theme::colors::*;

#[component]
pub fn GlassmorphicCard(
    #[prop(optional, into)] class: String,
    #[prop(optional)] hoverable: bool,
    children: Children,
) -> impl IntoView {
    let base_style = format!(
        "background: {};
         backdrop-filter: blur({});
         -webkit-backdrop-filter: blur({});
         border: 1px solid {};
         border-radius: {};
         padding: 2rem;
         {}",
        GLASS_BG,
        GLASS_BLUR,
        GLASS_BLUR,
        GLASS_BORDER,
        GLASS_RADIUS,
        if hoverable {
            "transition: transform 0.3s ease, box-shadow 0.3s ease, border-color 0.3s ease;"
        } else {
            ""
        }
    );

    let hover_class = if hoverable { "glass-card-hover" } else { "" };

    view! {
        <style>
            ".glass-card-hover:hover {
                transform: translateY(-8px) !important;
                box-shadow: 0 20px 40px rgba(255, 215, 0, 0.25) !important;
                border-color: rgba(255, 215, 0, 0.4) !important;
            }"
        </style>
        <div style=base_style class=format!("{} {}", class, hover_class)>
            {children()}
        </div>
    }
}
