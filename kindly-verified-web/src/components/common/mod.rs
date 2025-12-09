/// Byzantine-themed common components for consistent layout
use leptos::prelude::*;
use leptos_router::components::A;
use crate::utils::*;

// ============================================================================
// HEADER COMPONENT
// ============================================================================

/// Byzantine-themed navigation header with imperial styling
#[component]
pub fn ByzantineHeader() -> impl IntoView {
    view! {
        <header style=format!(
            "{}
             padding: {} {};
             border-bottom: 2px solid rgba(255, 215, 0, 0.15);
             position: sticky;
             top: 0;
             z-index: 100;
             backdrop-filter: blur(24px);
             -webkit-backdrop-filter: blur(24px);",
            glassmorphism(GlassBlur::Heavy, 0.2),
            SPACING_LG,
            SPACING_2XL
        )>
            <nav style="max-width: 1200px; margin: 0 auto; display: flex; justify-content: space-between; align-items: center;">
                // Imperial Logo with Crown
                <A href="/">
                    <div style=format!(
                        "{}
                         text-decoration: none;
                         display: flex;
                         align-items: center;
                         gap: {};
                         cursor: pointer;
                         transition: all 0.3s ease;",
                        text_heading_md(),
                        SPACING_SM
                    )
                    on:mouseenter=move |_| {
                        // Hover glow
                    }>
                        <span style="font-size: 2rem;">👑</span>
                        <span style=format!("background: {}; -webkit-background-clip: text; -webkit-text-fill-color: transparent; background-clip: text;", gradient_gold())>
                            "Kindly Verified"
                        </span>
                    </div>
                </A>

                // Navigation Links
                <div style=format!(
                    "display: flex;
                     gap: {};
                     align-items: center;",
                    SPACING_2XL
                )>
                    <A href="/#features">
                        <a style=format!(
                            "{}
                             text-decoration: none;
                             cursor: pointer;
                             transition: all 0.3s ease;",
                            text_body()
                        )
                        on:mouseenter=move |_| {
                            // Hover effect
                        }>
                            "Features"
                        </a>
                    </A>
                    <A href="/test">
                        <button style=format!(
                            "{}
                             padding: {} {};
                             border-radius: 12px;
                             border: 2px solid {};
                             background: transparent;
                             cursor: pointer;
                             font-weight: 600;
                             transition: all 0.3s ease;
                             text-transform: uppercase;
                             letter-spacing: 0.05em;
                             font-size: 0.875rem;",
                            text_caption(),
                            SPACING_SM,
                            SPACING_LG,
                            COLOR_GOLD
                        )
                        on:mouseenter=move |_| {
                            // Hover: fill with gold
                        }>
                            "✨ Test Now"
                        </button>
                    </A>
                </div>
            </nav>
        </header>
    }
}

// ============================================================================
// FOOTER COMPONENT
// ============================================================================

/// Byzantine-themed footer with imperial messaging
#[component]
pub fn ByzantineFooter() -> impl IntoView {
    view! {
        <footer style=format!(
            "{}
             padding: {} {};
             text-align: center;
             margin-top: {};
             border-top: 2px solid rgba(255, 215, 0, 0.15);",
            glassmorphism(GlassBlur::Medium, 0.1),
            SPACING_3XL,
            SPACING_2XL,
            SPACING_5XL
        )>
            <div style="max-width: 800px; margin: 0 auto;">
                <p style=format!(
                    "{}
                     margin-bottom: {};",
                    text_caption(),
                    SPACING_LG
                )>
                    "⚜️ Powered by Byzantine Computational Capsules ⚜️"
                </p>
                <p style=format!(
                    "{}
                     font-size: 0.75rem;",
                    text_caption()
                )>
                    "Imperial-Grade AI Detection • 100% Local Processing • Rust + WebAssembly"
                    <br/>
                    "100% Lockfree • Zero-Copy Atomic Operations • Open-Source Technology"
                </p>
                <p style=format!(
                    "{}
                     margin-top: {};
                     color: rgba(255, 215, 0, 0.6);",
                    text_caption(),
                    SPACING_LG
                )>
                    "© 2025 Kindly Verified. Forensic Excellence. Imperial Precision."
                </p>
            </div>
        </footer>
    }
}

// ============================================================================
// SECTION DIVIDER COMPONENT
// ============================================================================

/// Byzantine-themed section divider with gold accent
#[component]
pub fn ByzantineDivider() -> impl IntoView {
    view! {
        <div style=format!(
            "height: 2px;
             background: linear-gradient(90deg, transparent 0%, {} 50%, transparent 100%);
             margin: {} 0;
             position: relative;",
            COLOR_GOLD,
            SPACING_3XL
        )>
            <div style=format!(
                "position: absolute;
                 top: 50%;
                 left: 50%;
                 transform: translate(-50%, -50%);
                 width: 40px;
                 height: 40px;
                 background: {};
                 border-radius: 50%;
                 font-size: 1.5rem;
                 display: flex;
                 align-items: center;
                 justify-content: center;",
                gradient_hero()
            )>
                "⚜️"
            </div>
        </div>
    }
}

// ============================================================================
// BADGE COMPONENT
// ============================================================================

#[derive(Clone, Copy)]
pub enum BadgeType {
    Gold,
    Purple,
    Success,
    Warning,
}

/// Byzantine-themed badge for labels and badges
#[component]
pub fn ByzantineBadge(
    label: &'static str,
    #[prop(default = BadgeType::Gold)]
    badge_type: BadgeType,
) -> impl IntoView {
    let (background, color) = match badge_type {
        BadgeType::Gold => (COLOR_GOLD, COLOR_BG_DARK),
        BadgeType::Purple => (COLOR_PURPLE_LIGHT, COLOR_BG_DARK),
        BadgeType::Success => (COLOR_SUCCESS, COLOR_BG_DARK),
        BadgeType::Warning => (COLOR_WARNING, COLOR_BG_DARK),
    };

    view! {
        <span style=format!(
            "background: {};
             color: {};
             padding: {} {};
             border-radius: 8px;
             font-weight: 600;
             font-size: 0.75rem;
             text-transform: uppercase;
             letter-spacing: 0.05em;
             display: inline-block;",
            background,
            color,
            SPACING_XS,
            SPACING_SM
        )>
            {label}
        </span>
    }
}

// ============================================================================
// LOADING COMPONENT
// ============================================================================

/// Byzantine-themed loading spinner
#[component]
pub fn ByzantineLoader() -> impl IntoView {
    view! {
        <div style=format!(
            "display: flex;
             flex-direction: column;
             align-items: center;
             justify-content: center;
             padding: {} {};",
            SPACING_5XL,
            SPACING_2XL
        )>
            <style>
                {r#"
                .byzantine-spinner {
                    width: 60px;
                    height: 60px;
                    border: 4px solid rgba(255, 215, 0, 0.2);
                    border-top-color: #FFD700;
                    border-radius: 50%;
                    animation: spin 1s linear infinite;
                }

                @keyframes spin {
                    to { transform: rotate(360deg); }
                }

                .loader-crown {
                    font-size: 3rem;
                    margin-bottom: 1.5rem;
                    animation: glowPulse 2s ease-in-out infinite;
                }

                @keyframes glowPulse {
                    0%, 100% { filter: drop-shadow(0 0 20px rgba(255, 215, 0, 0.5)); }
                    50% { filter: drop-shadow(0 0 40px rgba(255, 215, 0, 0.8)); }
                }
                "#}
            </style>

            <div class="loader-crown">👑</div>
            <div class="byzantine-spinner"></div>
            <p style=format!(
                "{}
                 margin-top: {};
                 text-transform: uppercase;
                 letter-spacing: 0.05em;",
                text_caption(),
                SPACING_LG
            )>
                "Loading Imperial Systems..."
            </p>
        </div>
    }
}

// ============================================================================
// CARD COMPONENT (ENHANCED)
// ============================================================================

#[component]
pub fn ByzantineCard(
    children: Children,
    #[prop(default = true)]
    has_gold_border: bool,
) -> impl IntoView {
    view! {
        <div style=format!(
            "{}
             {}",
            card_glass(),
            if has_gold_border {
                format!("border-left: 3px solid {};", COLOR_GOLD)
            } else {
                format!("border-left: 3px solid {};", COLOR_PURPLE_LIGHT)
            }
        )>
            {children()}
        </div>
    }
}
