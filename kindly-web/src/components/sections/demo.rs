use leptos::prelude::*;
use web_sys;
use crate::components::common::{Button, ButtonVariant, ButtonSize};
use crate::utils::glassmorphism::{dark_section_background, featured_card_style, dark_card_style, gold_gradient_text};

#[component]
pub fn Demo() -> impl IntoView {
    let tiers = vec![
        (
            "Tier 1: Accuracy Proof",
            "100K documents",
            "~17 minutes",
            "98.89% F1 score (99%+ precision, 99%+ recall)",
        ),
        (
            "Tier 2: Speed Demo",
            "1M documents",
            "~17 seconds",
            "60,000+ docs/sec single-threaded (38× speedup)",
        ),
        (
            "Tier 3: Scale Demo",
            "10M documents",
            "~33 seconds",
            "300K docs/sec @ 8 cores/16 threads (190× speedup)",
        ),
    ];

    view! {
        <section
            class="demo"
            id="demo"
            style=move || format!(
                "{}; \
                 padding: 80px 2rem; \
                 position: relative;",
                dark_section_background()
            )
        >
            <div style="max-width: 1200px; margin: 0 auto;">
                <div class="demo-header" style="text-align: center; margin-bottom: 3rem;">
                    <h2
                        style=move || format!(
                            "{}; \
                             font-size: clamp(2rem, 4vw, 3rem); \
                             margin-bottom: 1rem;",
                            gold_gradient_text()
                        )
                    >
                        "Free Demo Binary"
                    </h2>
                    <p
                        class="demo-subtitle"
                        style="color: rgba(255, 255, 255, 0.85); \
                               font-size: 1.125rem; \
                               max-width: 600px; \
                               margin: 0 auto 2rem auto; \
                               line-height: 1.6;"
                    >
                        "Experience the speed yourself. No signup required. Hardware-bound to prevent abuse."
                    </p>
                </div>

                // Docker Pull Command
                <div
                    class="docker-command"
                    style=move || format!(
                        "{}; \
                         padding: 1.5rem; \
                         margin-bottom: 3rem; \
                         font-family: 'Courier New', monospace;",
                        featured_card_style()
                    )
                >
                    <div style="color: #FFED4E; font-weight: 700; margin-bottom: 0.5rem; font-size: 0.875rem; text-transform: uppercase; letter-spacing: 0.05em;">
                        "Docker Pull Command"
                    </div>
                    <code
                        style="display: block; \
                               background: rgba(0, 0, 0, 0.6); \
                               padding: 1rem; \
                               border-radius: 8px; \
                               color: #FFD700; \
                               font-size: 1.1rem; \
                               font-weight: 600; \
                               overflow-x: auto; \
                               border: 1px solid rgba(255, 215, 0, 0.4);"
                    >
                        "docker pull samuelduchaine/kindly-dedup:trial"
                    </code>
                </div>

                <div
                    class="demo-tiers"
                    style="display: grid; \
                           grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); \
                           gap: 2rem; \
                           margin-bottom: 3rem;"
                >
                    {tiers
                        .into_iter()
                        .map(|(name, docs, time, result)| {
                            view! {
                                <div
                                    class="demo-tier-card"
                                    style=move || format!(
                                        "{}; \
                                         padding: 2rem; \
                                         transition: all 0.3s ease;",
                                        dark_card_style()
                                    )
                                >
                                    <h3
                                        style=move || format!(
                                            "{}; \
                                             font-size: 1.25rem; \
                                             margin-bottom: 1.5rem;",
                                            gold_gradient_text()
                                        )
                                    >
                                        {name}
                                    </h3>
                                    <div class="demo-tier-stats" style="margin-bottom: 1.5rem;">
                                        <div
                                            class="demo-stat"
                                            style="display: flex; \
                                                   justify-content: space-between; \
                                                   align-items: center; \
                                                   padding: 0.75rem; \
                                                   background: rgba(102, 51, 153, 0.2); \
                                                   border-radius: 8px; \
                                                   margin-bottom: 0.75rem;"
                                        >
                                            <span
                                                class="demo-stat-label"
                                                style="color: rgba(255, 255, 255, 0.7); font-size: 0.875rem;"
                                            >
                                                "Documents"
                                            </span>
                                            <span
                                                class="demo-stat-value"
                                                style="color: #FFED4E; font-weight: 700; font-size: 1rem;"
                                            >
                                                {docs}
                                            </span>
                                        </div>
                                        <div
                                            class="demo-stat"
                                            style="display: flex; \
                                                   justify-content: space-between; \
                                                   align-items: center; \
                                                   padding: 0.75rem; \
                                                   background: rgba(102, 51, 153, 0.2); \
                                                   border-radius: 8px;"
                                        >
                                            <span
                                                class="demo-stat-label"
                                                style="color: rgba(255, 255, 255, 0.7); font-size: 0.875rem;"
                                            >
                                                "Time"
                                            </span>
                                            <span
                                                class="demo-stat-value"
                                                style="color: #FFED4E; font-weight: 700; font-size: 1rem;"
                                            >
                                                {time}
                                            </span>
                                        </div>
                                    </div>
                                    <p
                                        class="demo-tier-result"
                                        style="color: rgba(255, 255, 255, 0.85); \
                                               line-height: 1.5; \
                                               font-size: 0.9375rem;"
                                    >
                                        {result}
                                    </p>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>

                <div class="demo-actions" style="text-align: center; margin-bottom: 2rem;">
                    <p
                        class="demo-limit"
                        style="color: rgba(255, 255, 255, 0.85); \
                               font-size: 1rem; \
                               font-style: italic;"
                    >
                        "5M document limit. Hardware-bound. Contact "
                        <a
                            href="mailto:sales@kindly.software"
                            style="color: #FFD700; text-decoration: underline; font-weight: 600;"
                        >
                            "sales@kindly.software"
                        </a>
                        " for production license."
                    </p>
                </div>

                <div
                    class="demo-disclaimer"
                    style="color: rgba(255, 255, 255, 0.6); \
                           font-size: 0.875rem; \
                           font-style: italic; \
                           text-align: center; \
                           max-width: 800px; \
                           margin: 0 auto; \
                           line-height: 1.6;"
                >
                    <p>
                        "Demo performance measured on AMD Ryzen 9 6900HX (8 cores/16 threads). "
                        "Tier 3 requires ≥8 GB RAM. Performance claims validated with 95% confidence intervals."
                    </p>
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_compiles() {
        // Ensures component compiles
    }

    #[test]
    fn test_demo_renders() {
        let _ = Demo();
    }
}
