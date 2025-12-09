// [TRADE SECRET] Payment success page (Leptos 0.7)

use leptos::prelude::*;

/// Payment success page component
#[component]
pub fn SuccessPage() -> impl IntoView {
    view! {
        <div class="success-page" style="min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 2rem; background: linear-gradient(135deg, #f5f3ff 0%, #fff 100%);">
            <div class="success-container" style="background: white; border-radius: 12px; padding: 3rem; max-width: 600px; width: 100%; box-shadow: 0 12px 24px rgba(0, 0, 0, 0.1); text-align: center;">
                <div class="success-icon" style="font-size: 4rem; color: #10b981; margin-bottom: 1rem;">
                    "✓"
                </div>

                <h1 style="font-size: 2rem; color: #1a1a1a; margin-bottom: 1rem;">
                    "Payment Successful!"
                </h1>

                <p style="font-size: 1.1rem; color: #666; margin-bottom: 2rem;">
                    "Thank you for purchasing kindly_dedup Pro License."
                </p>

                <div style="text-align: left; background-color: #f9fafb; padding: 2rem; border-radius: 8px; margin-bottom: 2rem;">
                    <h2 style="font-size: 1.25rem; margin-bottom: 1.5rem; color: #1a1a1a;">
                        "Next Steps:"
                    </h2>

                    <ol style="list-style: none; padding: 0; margin: 0;">
                        <li style="margin-bottom: 1.5rem; padding-bottom: 1.5rem; border-bottom: 1px solid #e5e7eb;">
                            <strong style="display: block; color: #4B0082; margin-bottom: 0.5rem;">
                                "Check your email"
                            </strong>
                            <p style="margin: 0.5rem 0 0; color: #666; line-height: 1.6;">
                                "We've sent your license key to the email address you provided. "
                                "If you don't see it, check your spam folder."
                            </p>
                        </li>

                        <li style="margin-bottom: 1.5rem; padding-bottom: 1.5rem; border-bottom: 1px solid #e5e7eb;">
                            <strong style="display: block; color: #4B0082; margin-bottom: 0.5rem;">
                                "Download the CLI"
                            </strong>
                            <p style="margin: 0.5rem 0 0; color: #666; line-height: 1.6;">
                                "Visit "
                                <a href="https://kindly.software/download" style="color: #4B0082; text-decoration: none; font-weight: 500;">
                                    "our download page"
                                </a>
                                " to get the kindly_dedup command-line tool."
                            </p>
                        </li>

                        <li style="margin-bottom: 1.5rem; padding-bottom: 1.5rem; border-bottom: 1px solid #e5e7eb;">
                            <strong style="display: block; color: #4B0082; margin-bottom: 0.5rem;">
                                "Install your license"
                            </strong>
                            <p style="margin: 0.5rem 0 0; color: #666; line-height: 1.6;">
                                <code style="background-color: #f3f4f6; padding: 0.25rem 0.5rem; border-radius: 4px; font-family: 'Courier New', monospace; color: #1a1a1a; font-size: 0.9rem;">
                                    "kindly_dedup --license-key YOUR_KEY deduplicate ./your_data.txt"
                                </code>
                            </p>
                        </li>

                        <li style="margin-bottom: 0; padding-bottom: 0; border-bottom: none;">
                            <strong style="display: block; color: #4B0082; margin-bottom: 0.5rem;">
                                "Start deduplicating"
                            </strong>
                            <p style="margin: 0.5rem 0 0; color: #666; line-height: 1.6;">
                                "Enjoy unlimited, lightning-fast deduplication!"
                            </p>
                        </li>
                    </ol>
                </div>

                <div style="display: flex; flex-direction: column; gap: 1rem; margin-bottom: 2rem;">
                    <a href="https://docs.kindly.software/license" style="display: inline-block; padding: 1rem 1.5rem; border-radius: 8px; text-decoration: none; font-weight: 600; text-align: center; transition: all 0.3s; background: linear-gradient(135deg, #4B0082 0%, #6d28d9 100%); color: white;">
                        "Read License Documentation"
                    </a>
                    <a href="/" style="display: inline-block; padding: 1rem 1.5rem; border-radius: 8px; text-decoration: none; font-weight: 600; text-align: center; transition: all 0.3s; background-color: #f3f4f6; color: #1a1a1a; border: 1px solid #e5e7eb;">
                        "Back to Homepage"
                    </a>
                </div>

                <div style="background-color: #f0f9ff; border-left: 4px solid #0284c7; padding: 1.5rem; border-radius: 8px; text-align: left;">
                    <p style="margin: 0; color: #0c4a6e;">
                        <strong>"Need help?"</strong>
                        <br />
                        "Contact us at "
                        <a href="mailto:support@kindly.software" style="color: #0284c7; text-decoration: none; font-weight: 600;">
                            "support@kindly.software"
                        </a>
                    </p>
                </div>
            </div>
        </div>
    }
}

// Styles
const SUCCESS_STYLES: &str = r#"
.success-page {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    background: linear-gradient(135deg, #f5f3ff 0%, #fff 100%);
}

.success-container {
    background: white;
    border-radius: 12px;
    padding: 3rem;
    max-width: 600px;
    width: 100%;
    box-shadow: 0 12px 24px rgba(0, 0, 0, 0.1);
    text-align: center;
}

.success-icon {
    font-size: 4rem;
    color: #10b981;
    margin-bottom: 1rem;
    animation: bounce 0.6s ease-in-out;
}

@keyframes bounce {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-20px); }
}

.success-container h1 {
    font-size: 2rem;
    color: #1a1a1a;
    margin-bottom: 1rem;
}

.main-message {
    font-size: 1.1rem;
    color: #666;
    margin-bottom: 2rem;
}

.success-details {
    text-align: left;
    background-color: #f9fafb;
    padding: 2rem;
    border-radius: 8px;
    margin-bottom: 2rem;
}

.success-details h2 {
    font-size: 1.25rem;
    margin-bottom: 1.5rem;
    color: #1a1a1a;
}

.steps {
    list-style: none;
    padding: 0;
    margin: 0;
}

.steps li {
    margin-bottom: 1.5rem;
    padding-bottom: 1.5rem;
    border-bottom: 1px solid #e5e7eb;
}

.steps li:last-child {
    margin-bottom: 0;
    padding-bottom: 0;
    border-bottom: none;
}

.steps strong {
    display: block;
    color: #4B0082;
    margin-bottom: 0.5rem;
}

.steps p {
    margin: 0.5rem 0 0;
    color: #666;
    line-height: 1.6;
}

.steps a {
    color: #4B0082;
    text-decoration: none;
    font-weight: 500;
}

.steps a:hover {
    text-decoration: underline;
}

.steps code {
    background-color: #f3f4f6;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-family: 'Courier New', monospace;
    color: #1a1a1a;
    font-size: 0.9rem;
}

.session-id {
    margin-bottom: 2rem;
}

.session-id small {
    color: #999;
}

.session-id code {
    background-color: #f3f4f6;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-family: 'Courier New', monospace;
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
    .success-container {
        padding: 1.5rem;
    }

    .success-container h1 {
        font-size: 1.5rem;
    }

    .success-details {
        padding: 1.5rem;
    }

    .cta-buttons {
        flex-direction: column;
    }
}
"#;
