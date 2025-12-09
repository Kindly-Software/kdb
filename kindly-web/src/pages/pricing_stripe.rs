// [TRADE SECRET] Pricing page with license tier information
// Leptos component for kindly_dedup license sales

use leptos::prelude::*;

/// Pricing page component
#[component]
pub fn PricingPage() -> impl IntoView {
    view! {
        <div style="min-height: 100vh; padding: 6rem 2rem 4rem; background: linear-gradient(135deg, #1A0026 0%, #2D0052 50%, #1A0026 100%);">
            <div style="max-width: 1200px; margin: 0 auto;">
                {/* Header */}
                <div style="text-align: center; margin-bottom: 4rem;">
                    <h1 style="font-size: clamp(2rem, 4vw, 3.5rem); color: #FFD700; margin-bottom: 1rem; font-weight: 800;">
                        "Simple, Transparent Pricing"
                    </h1>
                    <p style="font-size: 1.25rem; color: rgba(255, 255, 255, 0.8); max-width: 600px; margin: 0 auto; line-height: 1.6;">
                        "Choose the plan that fits your needs. One-time purchase with lifetime updates."
                    </p>
                </div>

                {/* Pricing Tiers */}
                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 2rem; margin-bottom: 4rem;">
                    {/* Early Adopter Tier */}
                    <div style="background: rgba(75, 0, 130, 0.3); border: 2px solid #FFD700; border-radius: 16px; padding: 2.5rem; position: relative; transform: scale(1.05);">
                        <div style="position: absolute; top: -1rem; left: 1rem; background: linear-gradient(135deg, #FFD700 0%, #FFED4E 100%); color: #1A0026; padding: 0.5rem 1rem; border-radius: 8px; font-weight: 700; font-size: 0.875rem;">
                            "🔥 LIMITED TIME"
                        </div>
                        <h3 style="color: #FFD700; font-size: 1.75rem; margin-top: 1.5rem; margin-bottom: 0.5rem; font-weight: 800;">
                            "Pro License"
                        </h3>
                        <p style="color: rgba(255, 255, 255, 0.7); margin-bottom: 1.5rem; font-size: 0.95rem;">
                            "Early Adopter pricing - 7 of 10 spots remaining"
                        </p>
                        <div style="margin-bottom: 2rem;">
                            <div style="font-size: 2.5rem; color: #FFED4E; font-weight: 800; margin-bottom: 0.25rem;">
                                "$497"
                            </div>
                            <p style="color: rgba(255, 255, 255, 0.6); font-size: 0.9rem;">
                                "One-time license"
                            </p>
                        </div>
                        <ul style="list-style: none; padding: 0; margin: 0 0 2rem 0; color: rgba(255, 255, 255, 0.85);">
                            <li style="margin-bottom: 0.75rem;">
                                "✓ Unlimited deduplication"
                            </li>
                            <li style="margin-bottom: 0.75rem;">
                                "✓ Lifetime updates & support"
                            </li>
                            <li style="margin-bottom: 0.75rem;">
                                "✓ Priority support SLA"
                            </li>
                            <li style="margin-bottom: 0;">
                                "✓ All features included"
                            </li>
                        </ul>
                        <button style="width: 100%; padding: 1rem; background: linear-gradient(135deg, #FFD700 0%, #FFED4E 100%); color: #1A0026; border: none; border-radius: 8px; font-weight: 700; font-size: 1rem; cursor: pointer; transition: all 0.3s; display: flex; align-items: center; justify-content: center; gap: 0.5rem;">
                            "Buy Now - Save 50%"
                        </button>
                    </div>

                    {/* Regular Pro Tier */}
                    <div style="background: rgba(75, 0, 130, 0.2); border: 1px solid rgba(255, 215, 0, 0.3); border-radius: 16px; padding: 2.5rem;">
                        <h3 style="color: rgba(255, 255, 255, 0.9); font-size: 1.75rem; margin-bottom: 0.5rem; font-weight: 800;">
                            "Pro License"
                        </h3>
                        <p style="color: rgba(255, 255, 255, 0.7); margin-bottom: 1.5rem; font-size: 0.95rem;">
                            "Standard pricing after early adopter period"
                        </p>
                        <div style="margin-bottom: 2rem;">
                            <div style="font-size: 2.5rem; color: #FFD700; font-weight: 800; margin-bottom: 0.25rem;">
                                "$997"
                            </div>
                            <p style="color: rgba(255, 255, 255, 0.6); font-size: 0.9rem;">
                                "One-time license"
                            </p>
                        </div>
                        <ul style="list-style: none; padding: 0; margin: 0 0 2rem 0; color: rgba(255, 255, 255, 0.85);">
                            <li style="margin-bottom: 0.75rem;">
                                "✓ Unlimited deduplication"
                            </li>
                            <li style="margin-bottom: 0.75rem;">
                                "✓ Lifetime updates & support"
                            </li>
                            <li style="margin-bottom: 0.75rem;">
                                "✓ Priority support SLA"
                            </li>
                            <li style="margin-bottom: 0;">
                                "✓ All features included"
                            </li>
                        </ul>
                        <button style="width: 100%; padding: 1rem; background: rgba(255, 215, 0, 0.2); color: #FFD700; border: 1px solid rgba(255, 215, 0, 0.3); border-radius: 8px; font-weight: 700; font-size: 1rem; cursor: pointer; transition: all 0.3s;">
                            "Buy Now"
                        </button>
                    </div>

                    {/* Enterprise Tier */}
                    <div style="background: rgba(75, 0, 130, 0.2); border: 1px solid rgba(255, 215, 0, 0.3); border-radius: 16px; padding: 2.5rem;">
                        <h3 style="color: rgba(255, 255, 255, 0.9); font-size: 1.75rem; margin-bottom: 0.5rem; font-weight: 800;">
                            "Enterprise"
                        </h3>
                        <p style="color: rgba(255, 255, 255, 0.7); margin-bottom: 1.5rem; font-size: 0.95rem;">
                            "Custom deployment and support"
                        </p>
                        <div style="margin-bottom: 2rem;">
                            <div style="font-size: 2.5rem; color: #FFD700; font-weight: 800; margin-bottom: 0.25rem;">
                                "Custom"
                            </div>
                            <p style="color: rgba(255, 255, 255, 0.6); font-size: 0.9rem;">
                                "Contact sales for quote"
                            </p>
                        </div>
                        <ul style="list-style: none; padding: 0; margin: 0 0 2rem 0; color: rgba(255, 255, 255, 0.85);">
                            <li style="margin-bottom: 0.75rem;">
                                "✓ Unlimited everything"
                            </li>
                            <li style="margin-bottom: 0.75rem;">
                                "✓ On-premise deployment"
                            </li>
                            <li style="margin-bottom: 0.75rem;">
                                "✓ Dedicated support team"
                            </li>
                            <li style="margin-bottom: 0;">
                                "✓ Custom integrations"
                            </li>
                        </ul>
                        <a href="mailto:sales@kindly.software" style="display: block; width: 100%; padding: 1rem; text-align: center; background: rgba(255, 215, 0, 0.2); color: #FFD700; border: 1px solid rgba(255, 215, 0, 0.3); border-radius: 8px; font-weight: 700; font-size: 1rem; cursor: pointer; transition: all 0.3s; text-decoration: none;">
                            "Contact Sales"
                        </a>
                    </div>
                </div>

                {/* FAQ Section */}
                <div style="background: rgba(75, 0, 130, 0.15); border: 1px solid rgba(255, 215, 0, 0.2); border-radius: 16px; padding: 3rem; margin-bottom: 4rem;">
                    <h2 style="color: #FFD700; font-size: 2rem; margin-bottom: 2rem; text-align: center; font-weight: 800;">
                        "Frequently Asked Questions"
                    </h2>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 2rem;">
                        <div>
                            <h3 style="color: #FFED4E; margin-bottom: 0.75rem; font-weight: 700;">
                                "What's included in the license?"
                            </h3>
                            <p style="color: rgba(255, 255, 255, 0.8); line-height: 1.6;">
                                "Unlimited document deduplication, lifetime updates, priority email support, and all current and future features."
                            </p>
                        </div>
                        <div>
                            <h3 style="color: #FFED4E; margin-bottom: 0.75rem; font-weight: 700;">
                                "Can I use this on-premise?"
                            </h3>
                            <p style="color: rgba(255, 255, 255, 0.8); line-height: 1.6;">
                                "Yes! All licenses allow on-premise deployment. Enterprise includes additional deployment support."
                            </p>
                        </div>
                        <div>
                            <h3 style="color: #FFED4E; margin-bottom: 0.75rem; font-weight: 700;">
                                "How many cores can I use?"
                            </h3>
                            <p style="color: rgba(255, 255, 255, 0.8); line-height: 1.6;">
                                "Unlimited! All licenses support multi-core utilization. We've tested up to 16 cores/32 threads."
                            </p>
                        </div>
                        <div>
                            <h3 style="color: #FFED4E; margin-bottom: 0.75rem; font-weight: 700;">
                                "Is there a money-back guarantee?"
                            </h3>
                            <p style="color: rgba(255, 255, 255, 0.8); line-height: 1.6;">
                                "Yes! 30-day money-back guarantee on all Pro licenses. No questions asked."
                            </p>
                        </div>
                        <div>
                            <h3 style="color: #FFED4E; margin-bottom: 0.75rem; font-weight: 700;">
                                "Do you offer discounts for volume?"
                            </h3>
                            <p style="color: rgba(255, 255, 255, 0.8); line-height: 1.6;">
                                "Contact sales@kindly.software for volume pricing and enterprise discounts."
                            </p>
                        </div>
                        <div>
                            <h3 style="color: #FFED4E; margin-bottom: 0.75rem; font-weight: 700;">
                                "What's the difference between tiers?"
                            </h3>
                            <p style="color: rgba(255, 255, 255, 0.8); line-height: 1.6;">
                                "All Pro licenses are identical. Early Adopter is the same features at 50% off. Enterprise adds dedicated support."
                            </p>
                        </div>
                    </div>
                </div>

                {/* Final CTA */}
                <div style="background: rgba(102, 51, 153, 0.4); border: 1px solid rgba(255, 215, 0, 0.3); border-radius: 16px; padding: 3rem; text-align: center;">
                    <h2 style="color: #FFD700; font-size: 1.75rem; margin-bottom: 1rem; font-weight: 800;">
                        "Ready to 10× Your Deduplication Speed?"
                    </h2>
                    <p style="color: rgba(255, 255, 255, 0.8); font-size: 1.125rem; margin-bottom: 2rem; max-width: 600px; margin-left: auto; margin-right: auto; line-height: 1.6;">
                        "Join the first 10 early adopters and save 50%. Only 7 spots remaining!"
                    </p>
                    <div style="display: flex; gap: 1.5rem; justify-content: center; flex-wrap: wrap;">
                        <button style="padding: 1rem 2rem; background: linear-gradient(135deg, #FFD700 0%, #FFED4E 100%); color: #1A0026; border: none; border-radius: 8px; font-weight: 700; font-size: 1.1rem; cursor: pointer; transition: all 0.3s;">
                            "Buy Early Adopter ($497)"
                        </button>
                        <button style="padding: 1rem 2rem; background: rgba(255, 215, 0, 0.2); color: #FFD700; border: 1px solid rgba(255, 215, 0, 0.3); border-radius: 8px; font-weight: 700; font-size: 1.1rem; cursor: pointer; transition: all 0.3s;">
                            "Contact Sales"
                        </button>
                    </div>
                    <p style="color: rgba(255, 255, 255, 0.6); margin-top: 2rem; font-size: 0.95rem;">
                        "Need a free trial? Download the demo binary with 5M document limit."
                    </p>
                </div>
            </div>
        </div>
    }
}
