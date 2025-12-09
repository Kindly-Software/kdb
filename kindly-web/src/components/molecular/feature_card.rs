use leptos::prelude::*;
use crate::utils::glassmorphism::{card_style, gold_gradient_text};

#[component]
pub fn FeatureCard(
    #[prop(optional)] icon: Option<&'static str>,
    title: &'static str,
    description: &'static str,
) -> impl IntoView {
    view! {
        <div
            class="feature-card"
            style=move || format!(
                "{}; \
                 padding: 2rem; \
                 transition: all 0.3s ease; \
                 cursor: pointer;",
                card_style()
            )
        >
            {icon.map(|i| {
                view! {
                    <div
                        class="feature-icon"
                        style="margin-bottom: 1rem; font-size: 2rem;"
                    >
                        <i class={format!("icon {}", i)}></i>
                    </div>
                }
            })}
            <h3
                class="feature-title"
                style=move || format!(
                    "{}; \
                     font-size: 1.5rem; \
                     margin-bottom: 1rem;",
                    gold_gradient_text()
                )
            >
                {title}
            </h3>
            <p
                class="feature-description"
                style="color: rgba(255, 255, 255, 0.85); \
                       line-height: 1.6; \
                       font-size: 1rem;"
            >
                {description}
            </p>
        </div>
    }
}
