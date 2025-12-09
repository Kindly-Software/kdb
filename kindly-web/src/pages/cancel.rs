// [TRADE SECRET] Payment cancelled page (Leptos 0.7)

use leptos::prelude::*;

/// Payment cancelled page component
#[component]
pub fn CancelPage() -> impl IntoView {
    view! {
        <div style="min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 2rem; background: linear-gradient(135deg, #fef2f2 0%, #fff 100%);">
            <div style="background: white; border-radius: 12px; padding: 3rem; max-width: 600px; width: 100%; box-shadow: 0 12px 24px rgba(0, 0, 0, 0.1); text-align: center;">
                <div style="font-size: 4rem; color: #ef4444; margin-bottom: 1rem;">
                    "✕"
                </div>

                <h1 style="font-size: 2rem; color: #1a1a1a; margin-bottom: 1rem;">
                    "Payment Cancelled"
                </h1>

                <p style="font-size: 1.1rem; color: #666; margin-bottom: 2rem;">
                    "No worries! Your payment has been cancelled and no charges have been made."
                </p>

                <div style="text-align: left; background-color: #f9fafb; padding: 2rem; border-radius: 8px; margin-bottom: 2rem;">
                    <h2 style="font-size: 1.25rem; margin-bottom: 1.5rem; color: #1a1a1a;">
                        "What happens now?"
                    </h2>

                    <p style="color: #666; margin-bottom: 1rem;">
                        "If you experienced any issues during checkout, please:"
                    </p>

                    <ul style="list-style: none; padding: 0; margin: 0 0 1rem 0; color: #666;">
                        <li style="margin-bottom: 0.5rem;">
                            "Check that your payment information is correct"
                        </li>
                        <li style="margin-bottom: 0.5rem;">
                            "Try again - sometimes payment issues are temporary"
                        </li>
                        <li style="margin-bottom: 0;">
                            "Contact us if the problem persists"
                        </li>
                    </ul>

                    <p style="color: #666; margin-top: 1rem;">
                        "We offer a 30-day money-back guarantee on all Pro licenses, so you can "
                        "purchase with confidence."
                    </p>
                </div>

                <div style="background-color: #f0fdf4; border-left: 4px solid #22c55e; padding: 1.5rem; border-radius: 8px; margin-bottom: 2rem; text-align: left;">
                    <h3 style="color: #166534; margin-bottom: 1rem; font-size: 1.125rem;">
                        "Early Adopter Pricing Available!"
                    </h3>
                    <p style="color: #166534; margin: 0;">
                        <strong>"$497"</strong> " for the Pro License (limited to first 10 buyers)"
                        <br />
                        <strong>"$997"</strong> " for the regular Pro License after early adopter period"
                    </p>
                </div>

                <div style="display: flex; flex-direction: column; gap: 1rem; margin-bottom: 2rem;">
                    <a href="/pricing" style="display: inline-block; padding: 1rem 1.5rem; border-radius: 8px; text-decoration: none; font-weight: 600; text-align: center; transition: all 0.3s; background: linear-gradient(135deg, #4B0082 0%, #6d28d9 100%); color: white;">
                        "Try Again"
                    </a>
                    <a href="/" style="display: inline-block; padding: 1rem 1.5rem; border-radius: 8px; text-decoration: none; font-weight: 600; text-align: center; transition: all 0.3s; background-color: #f3f4f6; color: #1a1a1a; border: 1px solid #e5e7eb;">
                        "Back to Homepage"
                    </a>
                </div>

                <div style="background-color: #f0f9ff; border-left: 4px solid #0284c7; padding: 1.5rem; border-radius: 8px; text-align: left;">
                    <p style="margin: 0; color: #0c4a6e;">
                        <strong>"Questions? We're here to help!"</strong>
                        <br />
                        "Email us at "
                        <a href="mailto:support@kindly.software" style="color: #0284c7; text-decoration: none; font-weight: 600;">
                            "support@kindly.software"
                        </a>
                        " or visit "
                        <a href="https://docs.kindly.software" style="color: #0284c7; text-decoration: none; font-weight: 600;">
                            "our documentation"
                        </a>
                    </p>
                </div>
            </div>
        </div>
    }
}

// Styles
const CANCEL_STYLES: &str = r#"
.cancel-page {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    background: linear-gradient(135deg, #fef2f2 0%, #fff 100%);
}

.cancel-container {
    background: white;
    border-radius: 12px;
    padding: 3rem;
    max-width: 600px;
    width: 100%;
    box-shadow: 0 12px 24px rgba(0, 0, 0, 0.1);
    text-align: center;
}

.cancel-icon {
    font-size: 4rem;
    color: #ef4444;
    margin-bottom: 1rem;
}

.cancel-container h1 {
    font-size: 2rem;
    color: #1a1a1a;
    margin-bottom: 1rem;
}

.main-message {
    font-size: 1.1rem;
    color: #666;
    margin-bottom: 2rem;
}

.cancel-details {
    text-align: left;
    background-color: #fef2f2;
    padding: 2rem;
    border-radius: 8px;
    margin-bottom: 2rem;
    border-left: 4px solid #dc2626;
}

.cancel-details h2 {
    font-size: 1.25rem;
    margin-bottom: 1rem;
    color: #1a1a1a;
}

.cancel-details p {
    color: #666;
    line-height: 1.6;
    margin-bottom: 1rem;
}

.cancel-details ul {
    list-style: none;
    padding: 0;
    margin: 1rem 0;
}

.cancel-details li {
    padding: 0.5rem 0 0.5rem 1.5rem;
    position: relative;
    color: #666;
}

.cancel-details li:before {
    content: "→";
    position: absolute;
    left: 0;
    color: #dc2626;
    font-weight: bold;
}

.pricing-reminder {
    background-color: #f0fdf4;
    border-left: 4px solid #16a34a;
    padding: 2rem;
    border-radius: 8px;
    margin-bottom: 2rem;
    text-align: left;
}

.pricing-reminder h3 {
    color: #16a34a;
    margin-top: 0;
    margin-bottom: 1rem;
}

.pricing-reminder p {
    color: #166534;
    line-height: 1.8;
    margin: 0;
}

.pricing-reminder strong {
    font-size: 1.1rem;
}

.cta-buttons {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 2rem;
}

.btn {
    display: inline-block;
    padding: 1rem 1.5rem;
    border-radius: 8px;
    text-decoration: none;
    font-weight: 600;
    text-align: center;
    transition: all 0.3s;
}

.btn-primary {
    background: linear-gradient(135deg, #4B0082 0%, #6d28d9 100%);
    color: white;
}

.btn-primary:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(75, 0, 130, 0.3);
}

.btn-secondary {
    background-color: #f3f4f6;
    color: #1a1a1a;
    border: 1px solid #e5e7eb;
}

.btn-secondary:hover {
    background-color: #e5e7eb;
}

.support-note {
    background-color: #f0f9ff;
    border-left: 4px solid #0284c7;
    padding: 1.5rem;
    border-radius: 8px;
    text-align: left;
}

.support-note p {
    margin: 0;
    color: #0c4a6e;
    line-height: 1.6;
}

.support-note a {
    color: #0284c7;
    text-decoration: none;
    font-weight: 600;
}

.support-note a:hover {
    text-decoration: underline;
}

@media (max-width: 600px) {
    .cancel-container {
        padding: 1.5rem;
    }

    .cancel-container h1 {
        font-size: 1.5rem;
    }

    .cancel-details,
    .pricing-reminder {
        padding: 1.5rem;
    }

    .cta-buttons {
        flex-direction: column;
    }
}
"#;
