//! Premium Hero Section
//!
//! Full-screen hero with gold shimmer headline and glassmorphism.

use leptos::prelude::*;

/// Premium hero section
#[component]
pub fn Hero() -> impl IntoView {
    let hero_style = "
        min-height: auto;
        display: flex;
        flex-direction: column;
        justify-content: flex-start;
        align-items: center;
        text-align: center;
        padding: 7rem 2rem 6rem;
        position: relative;
        z-index: 1;
    ";

    let _badge_style = "
        display: inline-flex;
        align-items: center;
        gap: 0.5rem;
        background: rgba(255, 215, 0, 0.1);
        border: 1px solid rgba(255, 215, 0, 0.3);
        padding: 0.5rem 1rem;
        border-radius: 100px;
        font-size: 0.875rem;
        color: #FFD700;
        margin-bottom: 2rem;
    ";

    let headline_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: clamp(1.8rem, 5vw, 3rem);
        font-weight: 700;
        line-height: 1.25;
        margin-bottom: 1.25rem;
        max-width: 850px;
    ";

    let shimmer_style = "
        background: linear-gradient(
            90deg,
            #FFD700 0%,
            #fff 25%,
            #FFD700 50%,
            #fff 75%,
            #FFD700 100%
        );
        background-size: 200% auto;
        -webkit-background-clip: text;
        background-clip: text;
        -webkit-text-fill-color: transparent;
        animation: shimmer 3s linear infinite;
    ";

    let subheadline_style = "
        font-size: clamp(1rem, 2.2vw, 1.2rem);
        color: rgba(255, 255, 255, 0.7);
        max-width: 600px;
        margin-bottom: 1.75rem;
        line-height: 1.6;
    ";

    let cta_container_style = "
        display: flex;
        gap: 1rem;
        flex-wrap: wrap;
        justify-content: center;
    ";

    let primary_button_style = "
        background: linear-gradient(135deg, #FFD700, #FFA500);
        color: #000;
        padding: 1rem clamp(1.5rem, 4vw, 2.5rem);
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
        background: rgba(255, 255, 255, 0.1);
        backdrop-filter: blur(10px);
        border: 1px solid rgba(255, 255, 255, 0.2);
        color: #fff;
        padding: 1rem clamp(1.5rem, 4vw, 2.5rem);
        border-radius: 16px;
        font-size: 1.125rem;
        font-weight: 500;
        text-decoration: none;
        transition: all 0.3s ease;
        display: inline-flex;
        align-items: center;
        gap: 0.5rem;
    ";

    let stats_container_style = "
        display: flex;
        gap: clamp(1.5rem, 5vw, 3rem);
        margin-top: 2.5rem;
        flex-wrap: wrap;
        justify-content: center;
    ";

    let stat_style = "
        text-align: center;
    ";

    let stat_value_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 2rem;
        font-weight: 700;
        color: #FFD700;
    ";

    let stat_label_style = "
        font-size: 0.875rem;
        color: rgba(255, 255, 255, 0.6);
        margin-top: 0.25rem;
    ";

    view! {
        <style>
            ".learn-more-btn:hover {
                background: rgba(255, 215, 0, 0.15) !important;
                border-color: rgba(255, 215, 0, 0.4) !important;
                color: #FFD700 !important;
                box-shadow: 0 4px 20px rgba(255, 215, 0, 0.3);
            }

            @keyframes float {
                0%, 100% { transform: translateY(0px); }
                50% { transform: translateY(-15px); }
            }

            @keyframes glow {
                0%, 100% { filter: drop-shadow(0 0 30px rgba(255, 215, 0, 0.5)); }
                50% { filter: drop-shadow(0 0 50px rgba(255, 215, 0, 0.8)); }
            }

            .hero-logo:hover {
                transform: scale(1.1) !important;
                animation: float 1s ease-in-out infinite, glow 1s ease-in-out infinite !important;
            }"
        </style>
        <section id="hero" style=hero_style>
            <img src="/kdb-logo.jpg" alt="Kindly Debugger" style="width: 110px; height: 110px; margin-bottom: 1.5rem; animation: float 3s ease-in-out infinite, glow 2s ease-in-out infinite; filter: drop-shadow(0 0 30px rgba(255, 215, 0, 0.5)); transition: transform 0.3s ease; border-radius: 50%; object-fit: cover;" class="hero-logo" />

            <h1 style=headline_style>
                <span style="color: #fff;">"Give your AI the superpower of "</span>
                <span style=shimmer_style class="gold-shimmer">"traveling back in time"</span>
                <br/>
                <span style="color: rgba(255, 255, 255, 0.9);">"to find what went wrong and fix the timeline."</span>
            </h1>

            <p style=subheadline_style>
                "Step forward. Step backward. "
                <strong style="color: #FFD700;">"Debug as if the bug never existed."</strong>
                " Platform-agnostic via MCP. Works with Claude Code, Cursor, and any AI assistant."
            </p>

            <div style=cta_container_style>
                <a href="#signup" style=primary_button_style class="hero-cta">
                    <span>"🎉"</span>
                    <span>"Start Free - 1 Week Unlimited"</span>
                </a>
                <a href="#features" style=secondary_button_style class="learn-more-btn hero-cta">
                    <span>"📖"</span>
                    <span>"Learn More"</span>
                </a>
            </div>

            <div style=stats_container_style class="stats-container">
                <div style=stat_style>
                    <div style=stat_value_style>"⏱️"</div>
                    <div style=stat_label_style>"Time Travel"</div>
                </div>
                <div style=stat_style>
                    <div style=stat_value_style>"🌐"</div>
                    <div style=stat_label_style>"Any Platform"</div>
                </div>
                <div style=stat_style>
                    <div style=stat_value_style>"🔐"</div>
                    <div style=stat_label_style>"Audit Ready"</div>
                </div>
                <div style=stat_style>
                    <div style=stat_value_style>"🤖"</div>
                    <div style=stat_label_style>"MCP-Native"</div>
                </div>
            </div>
        </section>
    }
}
