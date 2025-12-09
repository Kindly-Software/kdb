use leptos::prelude::*;
use crate::utils::glassmorphism::{byzantine_background, gold_gradient_text};

#[component]
pub fn Pricing() -> impl IntoView {
    view! {
        <section
            class="pricing"
            id="pricing"
            style=move || format!(
                "{}; \
                 padding: 80px 2rem; \
                 position: relative;",
                byzantine_background()
            )
        >
            <div style="max-width: 1200px; margin: 0 auto;">
                <h2
                    style=move || format!(
                        "{}; \
                         font-size: clamp(2rem, 4vw, 3rem); \
                         text-align: center; \
                         margin-bottom: 1rem;",
                        gold_gradient_text()
                    )
                >
                    "Simple, Transparent Pricing"
                </h2>
                <p
                    style="color: rgba(255, 255, 255, 0.8); \
                           font-size: 1.125rem; \
                           text-align: center; \
                           max-width: 800px; \
                           margin: 0 auto 3rem auto; \
                           line-height: 1.6;"
                >
                    "One-time license purchase with lifetime updates. Join the first 10 early adopters and save 50%."
                </p>
                <div
                    class="pricing-cta"
                    style="display: flex; \
                           gap: 1.5rem; \
                           justify-content: center; \
                           flex-wrap: wrap; \
                           align-items: center;"
                >
                    <a
                        href="/pricing"
                        style="display: inline-block; \
                               background: linear-gradient(135deg, #FFD700 0%, #FFED4E 100%); \
                               color: #1A0026; \
                               padding: 1rem 2.5rem; \
                               border-radius: 12px; \
                               font-weight: 700; \
                               font-size: 1.1rem; \
                               text-decoration: none; \
                               transition: all 0.3s ease; \
                               box-shadow: 0 8px 16px rgba(255, 215, 0, 0.3); \
                               cursor: pointer;"
                        onmouseenter="this.style.boxShadow='0 12px 24px rgba(255, 215, 0, 0.5)'; this.style.transform='translateY(-2px)'"
                        onmouseleave="this.style.boxShadow='0 8px 16px rgba(255, 215, 0, 0.3)'; this.style.transform='translateY(0)'"
                    >
                        "View Pricing Plans"
                    </a>
                    <p
                        style="color: rgba(255, 255, 255, 0.7); \
                               font-size: 1rem; \
                               font-style: italic;"
                    >
                        "Early Adopter: $497 | Pro: $997 | Enterprise: Custom"
                    </p>
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pricing_compiles() {
        // Ensures component compiles
    }
}
