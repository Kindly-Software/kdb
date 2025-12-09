use leptos::prelude::*;

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
    let card_class = if featured {
        "pricing-card pricing-card-featured"
    } else {
        "pricing-card"
    };

    view! {
        <div class={card_class}>
            <h3 class="pricing-title">{tier}</h3>
            <div class="pricing-price">
                <span class="price-amount">{price}</span>
                <span class="price-period">{"/"}{period}</span>
            </div>
            <ul class="pricing-features">
                {features.into_iter().map(|feature| {
                    view! {
                        <li class="pricing-feature">
                            <i class="icon icon-check"></i>
                            <span>{feature}</span>
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ul>
            {cta_text.map(|text| {
                view! {
                    <a href={cta_link.unwrap_or("/signup")} class="btn btn-primary pricing-cta">
                        {text}
                    </a>
                }
            })}
        </div>
    }
}
