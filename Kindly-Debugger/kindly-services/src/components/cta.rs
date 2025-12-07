//! Call to Action Section
//!
//! Final CTA with gradient background and gold shimmer.

use leptos::prelude::*;

/// Premium call-to-action section
#[component]
pub fn Cta() -> impl IntoView {
    let section_style = "
        padding: 6rem 2rem;
        position: relative;
        z-index: 1;
        text-align: center;
    ";

    let container_style = "
        max-width: 800px;
        margin: 0 auto;
        background: linear-gradient(135deg, rgba(75, 0, 130, 0.3), rgba(138, 43, 226, 0.2));
        backdrop-filter: blur(20px);
        -webkit-backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 215, 0, 0.2);
        border-radius: 32px;
        padding: 4rem 3rem;
        position: relative;
        overflow: hidden;
    ";

    let glow_style = "
        position: absolute;
        top: -50%;
        left: -50%;
        width: 200%;
        height: 200%;
        background: radial-gradient(
            circle at center,
            rgba(255, 215, 0, 0.1) 0%,
            transparent 50%
        );
        pointer-events: none;
    ";

    let title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: clamp(1.75rem, 4vw, 2.5rem);
        font-weight: 700;
        color: #fff;
        margin-bottom: 1rem;
        position: relative;
        z-index: 1;
    ";

    let subtitle_style = "
        font-size: 1.125rem;
        color: rgba(255, 255, 255, 0.8);
        margin-bottom: 2rem;
        max-width: 500px;
        margin-left: auto;
        margin-right: auto;
        position: relative;
        z-index: 1;
    ";

    let button_container_style = "
        display: flex;
        gap: 1rem;
        justify-content: center;
        flex-wrap: wrap;
        position: relative;
        z-index: 1;
    ";

    let primary_button_style = "
        background: linear-gradient(135deg, #FFD700, #FFA500);
        color: #000;
        padding: 1rem 2.5rem;
        border-radius: 16px;
        font-size: 1.125rem;
        font-weight: 600;
        text-decoration: none;
        transition: transform 0.2s ease, box-shadow 0.2s ease;
        box-shadow: 0 8px 30px rgba(255, 215, 0, 0.4);
        display: inline-flex;
        align-items: center;
        gap: 0.5rem;
    ";

    let secondary_button_style = "
        background: transparent;
        border: 2px solid rgba(255, 255, 255, 0.3);
        color: #fff;
        padding: 1rem 2.5rem;
        border-radius: 16px;
        font-size: 1.125rem;
        font-weight: 500;
        text-decoration: none;
        transition: background 0.2s ease, border-color 0.2s ease;
        display: inline-flex;
        align-items: center;
        gap: 0.5rem;
    ";

    let guarantee_style = "
        margin-top: 2rem;
        font-size: 0.875rem;
        color: rgba(255, 255, 255, 0.6);
        position: relative;
        z-index: 1;
    ";

    view! {
        <style>
            ".cta-secondary-btn:hover {
                background: rgba(255, 215, 0, 0.15) !important;
                border-color: rgba(255, 215, 0, 0.5) !important;
                color: #FFD700 !important;
                box-shadow: 0 4px 20px rgba(255, 215, 0, 0.3);
            }"
        </style>
        <section id="cta" style=section_style>
            <div style=container_style>
                <div style=glow_style></div>

                <h2 style=title_style>
                    "Ready to Debug Smarter?"
                </h2>

                <p style=subtitle_style>
                    "Join developers using the first audit-compliant time-travel debugger. "
                    "MCP-native for AI workflows."
                </p>

                <div style=button_container_style>
                    <a href="#pricing" style=primary_button_style>
                        <span>"⏱️"</span>
                        <span>"Start Free Trial"</span>
                    </a>
                    <a href="https://github.com/kindly-software/kdb" style=secondary_button_style class="cta-secondary-btn" target="_blank">
                        <span>"📦"</span>
                        <span>"View on GitHub"</span>
                    </a>
                </div>

                <p style=guarantee_style>
                    "🔐 Audit Compliance • SOX/SOC2/GDPR/HIPAA Ready"
                </p>
            </div>
        </section>
    }
}
