use leptos::prelude::*;
use crate::components::molecular::PricingCard;
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
                         margin-bottom: 3rem;",
                        gold_gradient_text()
                    )
                >
                    "Pricing"
                </h2>
                <div
                    class="pricing-grid"
                    style="display: grid; \
                           grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); \
                           gap: 2rem;"
                >
                    <PricingCard
                        tier="Free"
                        price="$0"
                        period="forever"
                        features=vec![
                            "10M documents per month".to_string(),
                            "All performance features".to_string(),
                            "GitHub community support".to_string(),
                            "No credit card required".to_string(),
                        ]
                    />
                    <PricingCard
                        tier="Pay As You Go"
                        price="$0.01"
                        period="per 1,000 docs"
                        featured=true
                        features=vec![
                            "Unlimited documents".to_string(),
                            "SIMD + parallel acceleration".to_string(),
                            "Email support".to_string(),
                            "Monthly billing".to_string(),
                        ]
                    />
                    <PricingCard
                        tier="Enterprise"
                        price="Custom"
                        period="contact sales"
                        features=vec![
                            "On-premise deployment".to_string(),
                            "Dedicated support SLA".to_string(),
                            "Custom integrations".to_string(),
                            "Volume discounts".to_string(),
                        ]
                    />
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
