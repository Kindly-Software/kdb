//! Features Section
//!
//! Glassmorphism feature cards with icons.

use leptos::prelude::*;

/// Feature card data
struct Feature {
    icon: &'static str,
    title: &'static str,
    description: &'static str,
}

/// Premium features section
#[component]
pub fn Features() -> impl IntoView {
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
        gap: 2rem;
    ";

    let card_style = "
        background: rgba(255, 255, 255, 0.05);
        backdrop-filter: blur(20px);
        -webkit-backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 24px;
        padding: clamp(1.25rem, 4vw, 2rem);
        transition: transform 0.3s ease, box-shadow 0.3s ease;
    ";

    let icon_container_style = "
        width: 60px;
        height: 60px;
        background: linear-gradient(135deg, rgba(255, 215, 0, 0.2), rgba(255, 165, 0, 0.1));
        border-radius: 16px;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 1.75rem;
        margin-bottom: 1.5rem;
    ";

    let card_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.25rem;
        font-weight: 600;
        color: #fff;
        margin-bottom: 0.75rem;
    ";

    let card_description_style = "
        font-size: 1rem;
        color: rgba(255, 255, 255, 0.7);
        line-height: 1.6;
    ";

    let features = vec![
        Feature {
            icon: "⏱️",
            title: "Time-Travel Debugging",
            description: "Step backward and forward through your code. Rewind to any moment, replay the exact sequence. Debug as if the bug never happened.",
        },
        Feature {
            icon: "🔐",
            title: "Audit-Ready",
            description: "Every debugging session is recorded with tamper-evident audit trails. Ready for compliance reviews out of the box.",
        },
        Feature {
            icon: "🤖",
            title: "MCP-Native",
            description: "Built for Claude Code and AI assistants from day one. Your AI pair programmer can debug alongside you, no shell parsing needed.",
        },
        Feature {
            icon: "⚡",
            title: "Blazingly Fast",
            description: "So fast you'll forget you're debugging. Instant breakpoints, instant snapshots, instant replay. No more waiting.",
        },
        Feature {
            icon: "🔬",
            title: "Deep Stack Traces",
            description: "See the full picture instantly. Accelerated stack unwinding gives you complete visibility without the usual slowdown.",
        },
        Feature {
            icon: "🛡️",
            title: "Rock Solid",
            description: "Reproducible debugging sessions, every single time.",
        },
    ];

    view! {
        <section id="features" style=section_style>
            <div style=container_style>
                <div style=header_style>
                    <h2 style=section_title_style>"Why Kindly Debugger?"</h2>
                    <p style=section_subtitle_style>
                        "The first debugger built for AI workflows "
                        "with compliance-grade audit trails."
                    </p>
                </div>

                <div style=grid_style class="features-grid">
                    {features.into_iter().map(|feature| {
                        view! {
                            <div style=card_style class="feature-card">
                                <div style=icon_container_style>
                                    {feature.icon}
                                </div>
                                <h3 style=card_title_style>{feature.title}</h3>
                                <p style=card_description_style>{feature.description}</p>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            </div>
        </section>
    }
}
