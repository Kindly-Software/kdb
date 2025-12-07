//! API Hero Section
//!
//! Hero section for API documentation with live stats.

use leptos::prelude::*;
use kindly_ui::theme::colors::*;

#[component]
pub fn ApiHero() -> impl IntoView {
    let hero_style = "
        min-height: 70vh;
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        text-align: center;
        padding: 10rem 2rem 4rem;
        position: relative;
        z-index: 1;
    ";

    let badge_style = format!(
        "display: inline-flex;
         align-items: center;
         gap: 0.5rem;
         background: rgba(255, 215, 0, 0.1);
         border: 1px solid rgba(255, 215, 0, 0.3);
         padding: 0.5rem 1rem;
         border-radius: 100px;
         font-size: 0.875rem;
         color: {};
         margin-bottom: 2rem;",
        GOLD_PRIMARY
    );

    let title_style = format!(
        "font-family: {};
         font-size: clamp(2.5rem, 6vw, 4rem);
         font-weight: 700;
         margin-bottom: 1rem;",
        FONT_HEADING
    );

    let shimmer_style = format!(
        "background: {};
         background-size: 200% auto;
         -webkit-background-clip: text;
         background-clip: text;
         -webkit-text-fill-color: transparent;
         animation: shimmer 3s linear infinite;",
        GOLD_SHIMMER_GRADIENT
    );

    let subtitle_style = format!(
        "font-size: clamp(1.125rem, 2vw, 1.5rem);
         color: {};
         max-width: 700px;
         margin-bottom: 3rem;
         line-height: 1.6;",
        TEXT_SECONDARY
    );

    let stats_container_style = "
        display: flex;
        gap: 2rem;
        flex-wrap: wrap;
        justify-content: center;
        margin-top: 3rem;
    ";

    let stat_card_style = format!(
        "background: {};
         backdrop-filter: blur({});
         border: 1px solid {};
         border-radius: 12px;
         padding: 1rem 1.5rem;
         min-width: 100px;
         flex: 1;
         max-width: 150px;",
        GLASS_BG,
        GLASS_BLUR,
        GLASS_BORDER
    );

    let stat_value_style = format!(
        "font-family: {};
         font-size: 2rem;
         font-weight: 700;
         color: {};",
        FONT_HEADING,
        GOLD_PRIMARY
    );

    let stat_label_style = format!(
        "font-size: 0.875rem;
         color: {};
         margin-top: 0.25rem;",
        TEXT_MUTED
    );

    let logo_style = "
        width: 120px;
        height: 120px;
        margin-bottom: 2rem;
        animation: float 3s ease-in-out infinite, glow 2s ease-in-out infinite;
        filter: drop-shadow(0 0 30px rgba(255, 215, 0, 0.5));
        transition: transform 0.3s ease;
    ";

    view! {
        <style>
            "@keyframes shimmer {
                0% { background-position: -200% center; }
                100% { background-position: 200% center; }
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
        <section style=hero_style>
            <img src="/kdb-logo.png" alt="KDB Logo" style=logo_style class="hero-logo" />

            <h1 style=title_style>
                <span style="color: #fff;">"KDB "</span>
                <span style=shimmer_style class="gold-shimmer">"Debug API"</span>
            </h1>

            <p style=subtitle_style>
                "Time-travel debugging as a service. "
                <strong style="color: #FFD700;">"Bidirectional replay."</strong>
                " Instant snapshots. Audit-ready."
            </p>

            <div style=stats_container_style>
                <div style=stat_card_style.clone()>
                    <div style=stat_value_style.clone()>"10"</div>
                    <div style=stat_label_style.clone()>"Endpoints"</div>
                </div>
                <div style=stat_card_style.clone()>
                    <div style=stat_value_style.clone()>"✓"</div>
                    <div style=stat_label_style.clone()>"Live"</div>
                </div>
                <div style=stat_card_style.clone()>
                    <div style=stat_value_style.clone()>"REST"</div>
                    <div style=stat_label_style.clone()>"JSON API"</div>
                </div>
            </div>
        </section>
    }
}
