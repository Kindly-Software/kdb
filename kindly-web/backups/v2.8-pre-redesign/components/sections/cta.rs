use leptos::prelude::*;
use crate::utils::glassmorphism::{byzantine_background, card_style, gold_gradient_text};

#[component]
pub fn CallToAction() -> impl IntoView {
    view! {
        <section
            class="cta"
            id="get-started"
            style=move || format!(
                "{}; \
                 padding: 100px 2rem; \
                 position: relative;",
                byzantine_background()
            )
        >
            <div
                class="cta-content"
                style=move || format!(
                    "{}; \
                     max-width: 900px; \
                     margin: 0 auto; \
                     padding: 4rem 3rem; \
                     text-align: center;",
                    card_style()
                )
            >
                <h2
                    style=move || format!(
                        "{}; \
                         font-size: clamp(2rem, 5vw, 3.5rem); \
                         margin-bottom: 1.5rem; \
                         line-height: 1.2;",
                        gold_gradient_text()
                    )
                >
                    "Ready to Deduplicate at Lightning Speed?"
                </h2>
                <p
                    style="color: rgba(255, 255, 255, 0.85); \
                           font-size: 1.25rem; \
                           margin-bottom: 2.5rem; \
                           line-height: 1.6;"
                >
                    "Start with 10M documents free. No credit card required."
                </p>
                <div style="display: flex; gap: 1.5rem; justify-content: center; flex-wrap: wrap;">
                    <a
                        href="#demo"
                        style="display: inline-block; \
                               padding: 1.25rem 2.5rem; \
                               background: linear-gradient(135deg, #FFD700 0%, #FFA500 100%); \
                               color: #2D0052; \
                               font-weight: 700; \
                               font-size: 1.125rem; \
                               border-radius: 8px; \
                               text-decoration: none; \
                               transition: all 0.3s ease; \
                               box-shadow: 0 4px 14px rgba(255, 215, 0, 0.4);"
                    >
                        "Download Demo"
                    </a>
                    <a
                        href="mailto:sales@kindly.software"
                        style="display: inline-block; \
                               padding: 1.25rem 2.5rem; \
                               background: rgba(102, 51, 153, 0.4); \
                               backdrop-filter: blur(8px); \
                               -webkit-backdrop-filter: blur(8px); \
                               color: rgba(255, 255, 255, 0.9); \
                               border: 1px solid rgba(255, 237, 78, 0.3); \
                               font-weight: 600; \
                               font-size: 1.125rem; \
                               border-radius: 8px; \
                               text-decoration: none; \
                               transition: all 0.3s ease;"
                    >
                        "Contact Sales"
                    </a>
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cta_compiles() {
        // Ensures component compiles
    }
}
