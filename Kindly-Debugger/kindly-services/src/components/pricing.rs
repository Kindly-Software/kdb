//! Pricing Section
//!
//! Premium pricing cards with gold shimmer effects.

use leptos::prelude::*;

/// Pricing tier data
struct PricingTier {
    name: &'static str,
    price: &'static str,
    period: &'static str,
    description: &'static str,
    features: Vec<&'static str>,
    cta_text: &'static str,
    cta_url: &'static str,
    is_featured: bool,
}

/// Premium pricing section
#[component]
pub fn Pricing() -> impl IntoView {
    let section_style = "
        padding: 6rem 2rem;
        position: relative;
        z-index: 1;
    ";

    let container_style = "
        max-width: 1200px;
        margin: 0 auto;
    ";

    let header_style = "
        text-align: center;
        margin-bottom: 4rem;
    ";

    let section_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: clamp(2rem, 5vw, 3rem);
        font-weight: 700;
        color: #fff;
        margin-bottom: 1rem;
    ";

    let section_subtitle_style = "
        font-size: 1.125rem;
        color: rgba(255, 255, 255, 0.7);
        max-width: 600px;
        margin: 0 auto;
    ";

    let grid_style = "
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(min(100%, 280px), 1fr));
        gap: 1.5rem;
        align-items: stretch;
        max-width: 1400px;
        margin: 0 auto;
    ";

    let tiers = vec![
        PricingTier {
            name: "Hobby",
            price: "$0",
            period: "forever",
            description: "MCP handshake for casual debugging",
            features: vec![
                "5 debugging sessions/month",
                "Basic time-travel debugging",
                "MCP protocol support",
                "Local debugging only",
                "Community support",
                "GitHub issues",
            ],
            cta_text: "Get Started",
            cta_url: "https://github.com/kindly-software/kdb",
            is_featured: false,
        },
        PricingTier {
            name: "Starter",
            price: "$15",
            period: "/month",
            description: "Regular debugging for side projects",
            features: vec![
                "20 debugging sessions/month",
                "Everything in Hobby",
                "Enhanced audit trails",
                "Email support",
                "7-day snapshot retention",
            ],
            cta_text: "Subscribe",
            cta_url: "mailto:support@kindly.software?subject=Starter%20License",
            is_featured: false,
        },
        PricingTier {
            name: "Developer",
            price: "$39",
            period: "/month",
            description: "Professional debugging for individuals",
            features: vec![
                "100 debugging sessions/month",
                "Everything in Starter",
                "Remote debugging API",
                "REST API access",
                "Priority support",
                "1 seat license",
            ],
            cta_text: "Get Started",
            cta_url: "mailto:support@kindly.software?subject=Developer%20License",
            is_featured: true,
        },
        PricingTier {
            name: "Professional",
            price: "$199",
            period: "/month",
            description: "Unlimited debugging for teams",
            features: vec![
                "Unlimited debugging sessions",
                "Everything in Developer",
                "5 seat licenses",
                "SOC2/GDPR compliance",
                "Shared audit logs",
                "Priority support",
                "SLA guarantee",
            ],
            cta_text: "Get Started",
            cta_url: "mailto:support@kindly.software?subject=Professional%20License",
            is_featured: false,
        },
        PricingTier {
            name: "Enterprise",
            price: "Custom",
            period: "contact us",
            description: "Full compliance for regulated industries",
            features: vec![
                "Everything in Professional",
                "Unlimited seats",
                "HIPAA/SOX/FINRA",
                "Dedicated infrastructure",
                "Dedicated support",
                "Custom integrations",
                "Audit attestation",
            ],
            cta_text: "Contact Sales",
            cta_url: "mailto:sales@kindly.software?subject=Enterprise%20License",
            is_featured: false,
        },
    ];

    let enterprise_card_style = "
        max-width: 900px;
        margin: 3rem auto 0;
        background: rgba(255, 255, 255, 0.05);
        backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 24px;
        padding: 3rem;
        display: grid;
        grid-template-columns: 1fr 2fr auto;
        gap: 3rem;
        align-items: center;
    ";

    view! {
        <section id="pricing" style=section_style>
            <div style=container_style>
                <div style=header_style>
                    <h2 style=section_title_style>"Simple, Transparent Pricing"</h2>
                    <p style=section_subtitle_style>
                        "Compliance-ready from day one. Rate limits: 60 snapshots/min, 100/session."
                    </p>
                </div>

                // First 4 tiers in grid
                <div style=grid_style class="pricing-grid">
                    {tiers[..4].iter().map(|tier| {
                        let card_style = if tier.is_featured {
                            "
                                background: linear-gradient(135deg, rgba(255, 215, 0, 0.1), rgba(255, 165, 0, 0.05));
                                backdrop-filter: blur(20px);
                                -webkit-backdrop-filter: blur(20px);
                                border: 2px solid rgba(255, 215, 0, 0.3);
                                border-radius: 24px;
                                padding: clamp(1.25rem, 4vw, 2.5rem);
                                position: relative;
                                overflow: hidden;
                                display: flex;
                                flex-direction: column;
                                height: 100%;
                            "
                        } else {
                            "
                                background: rgba(255, 255, 255, 0.05);
                                backdrop-filter: blur(20px);
                                -webkit-backdrop-filter: blur(20px);
                                border: 1px solid rgba(255, 255, 255, 0.1);
                                border-radius: 24px;
                                padding: clamp(1.25rem, 4vw, 2.5rem);
                                display: flex;
                                flex-direction: column;
                                height: 100%;
                            "
                        };

                        let badge_style = "
                            position: absolute;
                            top: -1px;
                            right: 2rem;
                            background: linear-gradient(135deg, #FFD700, #FFA500);
                            color: #000;
                            padding: 0.5rem 1rem;
                            border-radius: 0 0 12px 12px;
                            font-size: 0.75rem;
                            font-weight: 700;
                            text-transform: uppercase;
                        ";

                        let tier_name_style = "
                            font-family: 'Space Grotesk', sans-serif;
                            font-size: 1.25rem;
                            font-weight: 600;
                            color: rgba(255, 255, 255, 0.8);
                            margin-bottom: 0.5rem;
                        ";

                        let price_style = "
                            font-family: 'Space Grotesk', sans-serif;
                            font-size: clamp(2rem, 6vw, 3rem);
                            font-weight: 700;
                            color: #fff;
                            margin-bottom: 0.25rem;
                        ";

                        let period_style = "
                            font-size: 0.875rem;
                            color: rgba(255, 255, 255, 0.6);
                            margin-bottom: 1rem;
                        ";

                        let description_style = "
                            font-size: 1rem;
                            color: rgba(255, 255, 255, 0.7);
                            margin-bottom: 2rem;
                            padding-bottom: 2rem;
                            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                        ";

                        let feature_list_style = "
                            list-style: none;
                            padding: 0;
                            margin: 0 0 1.5rem 0;
                            flex-grow: 1;
                        ";

                        let button_wrapper_style = "
                            margin-top: auto;
                            padding-top: 1rem;
                        ";

                        let feature_item_style = "
                            display: flex;
                            align-items: center;
                            gap: 0.75rem;
                            padding: 0.5rem 0;
                            color: rgba(255, 255, 255, 0.8);
                            font-size: 0.9375rem;
                        ";

                        let checkmark_style = "
                            color: #FFD700;
                            font-size: 1rem;
                        ";

                        let button_style = if tier.is_featured {
                            "
                                display: block;
                                width: 100%;
                                background: linear-gradient(135deg, #FFD700, #FFA500);
                                color: #000;
                                padding: 1rem;
                                border-radius: 12px;
                                font-size: 1rem;
                                font-weight: 600;
                                text-decoration: none;
                                text-align: center;
                                transition: transform 0.2s ease, box-shadow 0.2s ease;
                                box-shadow: 0 8px 30px rgba(255, 215, 0, 0.3);
                            "
                        } else {
                            "
                                display: block;
                                width: 100%;
                                background: rgba(255, 255, 255, 0.1);
                                color: #fff;
                                padding: 1rem;
                                border-radius: 12px;
                                font-size: 1rem;
                                font-weight: 600;
                                text-decoration: none;
                                text-align: center;
                                transition: background 0.2s ease;
                                border: 1px solid rgba(255, 255, 255, 0.2);
                            "
                        };

                        view! {
                            <div style=card_style class="pricing-card">
                                {if tier.is_featured {
                                    Some(view! { <div style=badge_style>"Most Popular"</div> })
                                } else {
                                    None
                                }}
                                <div style=tier_name_style>{tier.name}</div>
                                <div style=price_style class="price-amount">{tier.price}</div>
                                <div style=period_style>{tier.period}</div>
                                <p style=description_style>{tier.description}</p>
                                <ul style=feature_list_style>
                                    {tier.features.iter().map(|feature| {
                                        view! {
                                            <li style=feature_item_style>
                                                <span style=checkmark_style>"✓"</span>
                                                <span>{*feature}</span>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                                <div style=button_wrapper_style>
                                    <a href=tier.cta_url style=button_style target="_blank">
                                        {tier.cta_text}
                                    </a>
                                </div>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                // Enterprise tier (horizontal card)
                {
                    let enterprise = &tiers[4];
                    view! {
                        <div style=enterprise_card_style class="pricing-card enterprise-card">
                            <div>
                                <div style="font-family: 'Space Grotesk', sans-serif; font-size: 1.5rem; font-weight: 700; color: #fff; margin-bottom: 1rem; text-align: center;">
                                    {enterprise.name}
                                </div>
                                <div style="font-size: 2.5rem; font-weight: 700; color: #FFD700; margin-bottom: 0.25rem; text-align: center;">
                                    {enterprise.price}
                                </div>
                                <div style="color: rgba(255,215,0,0.7); font-size: 1rem; margin-bottom: 1.5rem; text-align: center;">
                                    {enterprise.period}
                                </div>
                                <p style="color: rgba(255,255,255,0.7); font-size: 0.9375rem; line-height: 1.6; text-align: center;">
                                    {enterprise.description}
                                </p>
                            </div>

                            <div>
                                <ul style="list-style: none; padding: 0; margin: 0; display: grid; grid-template-columns: repeat(2, 1fr); gap: 0.75rem 1.5rem;">
                                    {enterprise.features.iter().map(|feature| {
                                        view! {
                                            <li style="display: flex; align-items: center; gap: 0.75rem; color: rgba(255,255,255,0.8); font-size: 0.9375rem;">
                                                <span style="color: #FFD700; font-size: 1rem; flex-shrink: 0;">"✓"</span>
                                                <span>{*feature}</span>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            </div>

                            <div style="display: flex; align-items: center; justify-content: center;">
                                <a href=enterprise.cta_url style="display: inline-block; white-space: nowrap; background: linear-gradient(135deg, #FFD700, #FFA500); color: #000; padding: 1rem 2rem; border-radius: 12px; font-size: 1rem; font-weight: 600; text-decoration: none; text-align: center; transition: transform 0.2s ease, box-shadow 0.2s ease; box-shadow: 0 8px 30px rgba(255, 215, 0, 0.3);" target="_blank">
                                    {enterprise.cta_text}
                                </a>
                            </div>
                        </div>
                    }
                }
            </div>
        </section>
    }
}
