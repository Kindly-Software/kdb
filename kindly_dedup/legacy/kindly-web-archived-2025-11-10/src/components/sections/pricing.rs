use leptos::prelude::*;
use crate::components::molecular::PricingCard;

#[component]
pub fn Pricing() -> impl IntoView {
    view! {
        <section class="pricing">
            <div class="pricing-container">
                <h2>"Pricing"</h2>
                <div class="pricing-grid">
                    <PricingCard
                        tier="Starter"
                        price="$99"
                        period="month"
                        features=vec![
                            "Core capsule library".to_string(),
                            "Basic SIMD support".to_string(),
                            "Community support".to_string(),
                        ]
                    />
                    <PricingCard
                        tier="Professional"
                        price="$299"
                        period="month"
                        features=vec![
                            "Advanced capsules".to_string(),
                            "Full SIMD optimization".to_string(),
                            "Priority support".to_string(),
                            "Custom integrations".to_string(),
                        ]
                    />
                    <PricingCard
                        tier="Enterprise"
                        price="Custom"
                        period="contact us"
                        features=vec![
                            "All Professional features".to_string(),
                            "Dedicated support".to_string(),
                            "On-premise deployment".to_string(),
                            "Custom development".to_string(),
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
