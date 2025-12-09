use leptos::prelude::*;
use leptos_router::components::A;
use crate::utils::*;

/// Home page with hero and feature showcase
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div style=format!(
            "min-height: 100vh;
             background: {};
             padding: {} {};
             animation: fadeInOnLoad 0.8s ease-out;",
            gradient_hero(),
            SPACING_2XL,
            SPACING_MD
        )>
            <style>
                {r#"
                @keyframes fadeInOnLoad {
                    from { opacity: 0; }
                    to { opacity: 1; }
                }

                @keyframes glowPulse {
                    0%, 100% { filter: drop-shadow(0 0 20px rgba(255, 215, 0, 0.5)); }
                    50% { filter: drop-shadow(0 0 40px rgba(255, 215, 0, 0.8)); }
                }

                .crown-icon {
                    animation: glowPulse 2s ease-in-out infinite;
                }

                .feature-card-stagger-1 { animation: slideInUp 0.6s ease-out 0.1s both; }
                .feature-card-stagger-2 { animation: slideInUp 0.6s ease-out 0.2s both; }
                .feature-card-stagger-3 { animation: slideInUp 0.6s ease-out 0.3s both; }
                .feature-card-stagger-4 { animation: slideInUp 0.6s ease-out 0.4s both; }
                .feature-card-stagger-5 { animation: slideInUp 0.6s ease-out 0.5s both; }
                .feature-card-stagger-6 { animation: slideInUp 0.6s ease-out 0.6s both; }

                @keyframes slideInUp {
                    from {
                        opacity: 0;
                        transform: translateY(30px);
                    }
                    to {
                        opacity: 1;
                        transform: translateY(0);
                    }
                }
                "#}
            </style>

            // Hero Section with Imperial Crown
            <section style=format!(
                "max-width: 1200px;
                 margin: 0 auto;
                 text-align: center;
                 padding: {} 0;",
                SPACING_5XL
            )>
                // Imperial Crown Icon
                <div style=format!(
                    "font-size: 5rem;
                     margin-bottom: {};
                     {}
                     class='crown-icon'",
                    SPACING_2XL,
                    glow_gold()
                )>
                    "👑"
                </div>

                <h1 style=text_heading_xl()>
                    "Kindly Verified"
                </h1>
                <h2 style=format!(
                    "{}
                     margin-top: {};
                     background: {};
                     -webkit-background-clip: text;
                     -webkit-text-fill-color: transparent;
                     background-clip: text;",
                    text_heading_md(),
                    SPACING_MD,
                    gradient_purple_shimmer()
                )>
                    "Imperial-Grade AI Image Detection"
                </h2>
                <p style=format!(
                    "{}
                     margin-top: {};
                     max-width: 700px;
                     margin-left: auto;
                     margin-right: auto;
                     font-size: 1.125rem;",
                    text_body(),
                    SPACING_LG
                )>
                    "Powered by Byzantine Computational Capsules"
                    <br/>
                    "10 Forensic Detectors • 40-150ms Latency • 90-95% Accuracy"
                </p>

                // CTA Button with Enhanced Gold Styling
                <A href="/test">
                    <button style=format!(
                        "margin-top: {};
                         background: {};
                         color: {};
                         font-size: 1.25rem;
                         padding: {} {};
                         border-radius: 16px;
                         border: none;
                         cursor: pointer;
                         font-weight: 700;
                         transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
                         text-transform: uppercase;
                         letter-spacing: 0.05em;
                         box-shadow: 0 0 20px rgba(255, 215, 0, 0.5),
                                     0 0 40px rgba(255, 215, 0, 0.3);",
                        SPACING_2XL,
                        gradient_gold(),
                        COLOR_BG_DARK,
                        SPACING_LG,
                        SPACING_3XL
                    )
                    on:mouseenter=move |_| {
                        // Hover effect handled by CSS
                    }>
                        "✨ Test Your Image →"
                    </button>
                </A>
            </section>

            // Features Grid (Imperial Tribunal)
            <section style=format!(
                "max-width: 1200px;
                 margin: 0 auto;
                 padding: {} 0;",
                SPACING_4XL
            )
            id="features">
                <h3 style=format!(
                    "{}
                     text-align: center;
                     margin-bottom: {};
                     text-transform: uppercase;
                     letter-spacing: 0.1em;",
                    text_heading_lg(),
                    SPACING_3XL
                )>
                    "Imperial Byzantine Tribunal"
                </h3>

                <div style=format!(
                    "display: grid;
                     grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
                     gap: {};",
                    SPACING_XL
                )>
                    <FeatureCard
                        title="Imperial Forensic Tribunal"
                        description="10 forensic detectors: PRNU, Benford's Law, Chromatic Aberration, Demosaicing, EXIF metadata, and more."
                        icon="⚖️"
                        index=1
                    />
                    <FeatureCard
                        title="Byzantine Speed"
                        description="40-150ms latency. Fast path with EXIF cache, full ensemble for deep analysis."
                        icon="⚡"
                        index=2
                    />
                    <FeatureCard
                        title="Universal Imperial Standards"
                        description="7 image formats supported: JPEG, PNG, BMP, GIF, WebP, TIFF, AVIF/HEIC. Automatic detection."
                        icon="📋"
                        index=3
                    />
                    <FeatureCard
                        title="Imperial Precision"
                        description="90-95% accuracy. Detects camera sensor fingerprints and AI artifacts with forensic-grade analysis."
                        icon="🎯"
                        index=4
                    />
                    <FeatureCard
                        title="Royal Privacy Decree"
                        description="All processing happens in your browser. No uploads, no tracking, 100% local execution."
                        icon="👑"
                        index=5
                    />
                    <FeatureCard
                        title="Byzantine Computational Capsules"
                        description="Built with Rust + WebAssembly. 100% lockfree, zero-copy atomic operations. Open-source Imperial technology."
                        icon="⚙️"
                        index=6
                    />
                </div>
            </section>
        </div>
    }
}

#[component]
fn FeatureCard(
    title: &'static str,
    description: &'static str,
    icon: &'static str,
    index: u8,
) -> impl IntoView {
    let stagger_class = format!("feature-card-stagger-{}", index);
    let is_gold_accent = index % 2 == 1; // Alternate gold/purple accents

    let border_style = if is_gold_accent {
        "border-top: 2px solid rgba(255, 215, 0, 0.4);"
    } else {
        "border-top: 2px solid rgba(139, 92, 246, 0.4);"
    };

    view! {
        <div style=format!(
            "{}
             text-align: center;
             cursor: default;
             position: relative;
             overflow: hidden;
             {} ",
            card_glass(),
            border_style
        )
        class=stagger_class>
            <div style=format!(
                "font-size: 4rem;
                 margin-bottom: {};
                 transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);",
                SPACING_MD
            )
            on:mouseenter=move |_| {
                // Icon grows on hover via CSS
            }>
                {icon}
            </div>
            <h4 style=format!(
                "{}
                 margin-bottom: {};
                 text-transform: uppercase;
                 letter-spacing: 0.05em;
                 font-size: 1.25rem;",
                if is_gold_accent {
                    format!("color: {};", COLOR_GOLD)
                } else {
                    format!("color: {};", COLOR_PURPLE_LIGHT)
                },
                SPACING_SM
            )>
                {title}
            </h4>
            <p style=format!(
                "{}
                 line-height: 1.7;",
                text_body()
            )>
                {description}
            </p>
        </div>
    }
}
