use leptos::prelude::*;
use crate::utils::glassmorphism::{card_style, gold_gradient_text};

#[component]
pub fn PricingCard(
    tier: &'static str,
    price: &'static str,
    period: &'static str,
    features: Vec<String>,
    #[prop(optional)] featured: bool,
    #[prop(optional)] cta_text: Option<&'static str>,
    #[prop(optional)] cta_link: Option<&'static str>,
) -> impl IntoView {
    let border_style = if featured {
        "border: 2px solid #FFD700; box-shadow: 0 0 40px rgba(255, 215, 0, 0.4);"
    } else {
        ""
    };

    view! {
        <div
            class="pricing-card"
            style=move || format!(
                "{}; \
                 {}; \
                 padding: 2.5rem; \
                 text-align: center; \
                 transition: all 0.3s ease;",
                card_style(),
                border_style
            )
        >
            <h3
                class="pricing-title"
                style=move || format!(
                    "{}; \
                     font-size: 1.5rem; \
                     margin-bottom: 1.5rem;",
                    gold_gradient_text()
                )
            >
                {tier}
            </h3>
            <div class="pricing-price" style="margin-bottom: 2rem;">
                <span
                    class="price-amount"
                    style="font-size: 3rem; \
                           font-weight: 800; \
                           color: #FFED4E;"
                >
                    {price}
                </span>
                <span
                    class="price-period"
                    style="color: rgba(255, 255, 255, 0.7); \
                           font-size: 1rem; \
                           display: block; \
                           margin-top: 0.5rem;"
                >
                    {"/"}{period}
                </span>
            </div>
            <ul
                class="pricing-features"
                style="list-style: none; \
                       padding: 0; \
                       margin-bottom: 2rem;"
            >
                {features.into_iter().map(|feature| {
                    view! {
                        <li
                            class="pricing-feature"
                            style="padding: 0.75rem; \
                                   color: rgba(255, 255, 255, 0.85); \
                                   border-bottom: 1px solid rgba(255, 255, 255, 0.1); \
                                   display: flex; \
                                   align-items: center; \
                                   gap: 0.75rem;"
                        >
                            <span style="color: #FFD700; font-size: 1.25rem;">"✓"</span>
                            <span>{feature}</span>
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ul>
            {cta_text.map(|text| {
                view! {
                    <a
                        href={cta_link.unwrap_or("/signup")}
                        style="display: inline-block; \
                               padding: 1rem 2rem; \
                               background: linear-gradient(135deg, #FFD700 0%, #FFA500 100%); \
                               color: #2D0052; \
                               font-weight: 700; \
                               border-radius: 8px; \
                               text-decoration: none; \
                               transition: all 0.3s ease; \
                               box-shadow: 0 4px 14px rgba(255, 215, 0, 0.4);"
                    >
                        {text}
                    </a>
                }
            })}
        </div>
    }
}
