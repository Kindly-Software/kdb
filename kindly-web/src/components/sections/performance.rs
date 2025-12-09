use leptos::prelude::*;
use crate::utils::glassmorphism::{dark_section_background, dark_card_style, gold_gradient_text, cyan_gradient_text, purple_gradient_text};

#[component]
pub fn Performance() -> impl IntoView {
    let metrics = vec![
        ("190×", "Overall speedup", "vs Python datasketch baseline"),
        ("98.89%", "F1 accuracy score", "99%+ precision, 99%+ recall"),
        ("300K", "Documents per second", "@ 8 cores/16 threads (measured)"),
        ("<1ms", "Per-document latency", "End-to-end processing"),
    ];

    view! {
        <section
            class="performance"
            id="performance"
            style=move || format!(
                "{}; \
                 padding: 80px 2rem; \
                 position: relative;",
                dark_section_background()
            )
        >
            <div style="max-width: 1200px; margin: 0 auto;">
                <div class="performance-header" style="text-align: center; margin-bottom: 3rem;">
                    <h2
                        style=move || format!(
                            "{}; \
                             font-size: clamp(2rem, 4vw, 3rem); \
                             margin-bottom: 1rem;",
                            gold_gradient_text()
                        )
                    >
                        "Lightning-Fast Deduplication"
                    </h2>
                    <p
                        class="performance-subtitle"
                        style="color: rgba(255, 255, 255, 0.85); \
                               font-size: 1.125rem; \
                               max-width: 800px; \
                               margin: 0 auto; \
                               line-height: 1.6;"
                    >
                        "Production-tested on AMD Ryzen 9 6900HX. Independently validated with 95% confidence intervals."
                    </p>
                </div>

                <div
                    class="performance-grid"
                    style="display: grid; \
                           grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); \
                           gap: 2rem; \
                           margin-bottom: 3rem;"
                >
                    {metrics
                        .into_iter()
                        .enumerate()
                        .map(|(idx, (value, label, context))| {
                            view! {
                                <div
                                    class="metric-card"
                                    style=move || format!(
                                        "{}; \
                                         padding: 2rem; \
                                         text-align: center; \
                                         transition: all 0.3s ease;",
                                        dark_card_style()
                                    )
                                >
                                    <div
                                        class="metric-value"
                                        style=move || {
                                            let gradient = match value {
                                                "98.89%" | "<1ms" => purple_gradient_text(),
                                                _ if idx % 2 == 0 => gold_gradient_text(),
                                                _ => cyan_gradient_text(),
                                            };
                                            format!(
                                                "{}; \
                                                 font-size: 3rem; \
                                                 font-weight: 800; \
                                                 margin-bottom: 0.5rem;",
                                                gradient
                                            )
                                        }
                                    >
                                        {value}
                                    </div>
                                    <div
                                        class="metric-label"
                                        style="color: rgba(255, 255, 255, 0.9); \
                                               font-size: 1.125rem; \
                                               font-weight: 600; \
                                               margin-bottom: 0.5rem;"
                                    >
                                        {label}
                                    </div>
                                    <div
                                        class="metric-context"
                                        style="color: rgba(255, 255, 255, 0.7); \
                                               font-size: 0.875rem;"
                                    >
                                        {context}
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>

                <div
                    class="performance-disclaimer"
                    style="color: rgba(255, 255, 255, 0.6); \
                           font-size: 0.875rem; \
                           font-style: italic; \
                           text-align: center; \
                           max-width: 900px; \
                           margin: 0 auto; \
                           line-height: 1.6;"
                >
                    <p>
                        "Performance measured on AMD Ryzen 9 6900HX (8 cores/16 threads). "
                        "Results vary by CPU architecture, memory bandwidth, and dataset characteristics. "
                        "Single-threaded validated: 60K docs/sec. Multi-threaded measured: 300K docs/sec (190× vs Python baseline)."
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
    fn test_performance_compiles() {
        // Ensures component compiles
    }

    #[test]
    fn test_performance_renders() {
        let _ = Performance();
    }
}
