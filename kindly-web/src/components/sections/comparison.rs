use leptos::prelude::*;
use crate::utils::glassmorphism::{dark_section_background, gold_gradient_text};

#[component]
pub fn Comparison() -> impl IntoView {
    let comparisons = vec![
        ("Single-threaded", "1,572 docs/sec", "60,000 docs/sec (38× faster)"),
        ("Multi-threaded (8c/16t)", "Not available", "300K docs/sec (190× faster)"),
        ("Memory Safety", "Runtime checks", "Compile-time verified"),
        ("Accuracy (F1)", "85-90%", "98.89% (near-perfect)"),
        ("Dependencies", "NumPy, SciPy", "Zero external runtime deps"),
        ("Deployment", "Python + pip", "Single static binary"),
    ];

    view! {
        <section
            id="comparison"
            class="comparison-section"
            style=move || format!(
                "{}; \
                 padding: 80px 2rem; \
                 position: relative;",
                dark_section_background()
            )
        >
            <div style="max-width: 1200px; margin: 0 auto;">
                <h2
                    class="section-title"
                    style=move || format!(
                        "{}; \
                         font-size: clamp(2rem, 4vw, 3rem); \
                         text-align: center; \
                         margin-bottom: 3rem;",
                        gold_gradient_text()
                    )
                >
                    "Why kindly_dedup?"
                </h2>

                <div
                    class="comparison-table"
                    style="background: rgba(26, 0, 40, 0.95); \
                           border: 1px solid rgba(75, 0, 130, 0.4); \
                           border-radius: 16px; \
                           padding: 2rem; \
                           overflow-x: auto; \
                           box-shadow: 0 20px 40px -5px rgba(0, 0, 0, 0.5);"
                >
                    // Table Header
                    <div
                        style="display: grid; \
                               grid-template-columns: 2fr 2fr 2fr; \
                               gap: 1rem; \
                               padding-bottom: 1rem; \
                               border-bottom: 1px solid rgba(255, 237, 78, 0.3); \
                               margin-bottom: 1rem;"
                    >
                        <div style="font-weight: 700; color: #FFED4E; font-size: 1.1rem;">
                            "Feature"
                        </div>
                        <div style="font-weight: 700; color: rgba(255, 255, 255, 0.8); font-size: 1.1rem; text-align: center;">
                            "Python (datasketch)"
                        </div>
                        <div
                            style=move || format!(
                                "{}; \
                                 font-weight: 700; \
                                 font-size: 1.1rem; \
                                 text-align: center;",
                                gold_gradient_text()
                            )
                        >
                            "kindly_dedup (Rust)"
                        </div>
                    </div>

                    // Table Rows
                    {comparisons
                        .into_iter()
                        .enumerate()
                        .map(|(idx, (feature, python, rust))| {
                            let bg_color = if idx % 2 == 0 {
                                "rgba(75, 0, 130, 0.25)"
                            } else {
                                "rgba(26, 0, 40, 0.5)"
                            };

                            view! {
                                <div
                                    style=format!(
                                        "display: grid; \
                                         grid-template-columns: 2fr 2fr 2fr; \
                                         gap: 1rem; \
                                         padding: 1rem; \
                                         background: {}; \
                                         border-radius: 8px; \
                                         margin-bottom: 0.5rem; \
                                         transition: all 0.2s ease;",
                                        bg_color
                                    )
                                >
                                    <div style="color: rgba(255, 255, 255, 0.9); font-weight: 600;">
                                        {feature}
                                    </div>
                                    <div style="color: rgba(255, 255, 255, 0.7); text-align: center;">
                                        {python}
                                    </div>
                                    <div style="color: #FFD700; font-weight: 700; text-align: center;">
                                        {rust}
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>

                <p
                    class="comparison-footnote"
                    style="color: rgba(255, 255, 255, 0.6); \
                           font-size: 0.875rem; \
                           font-style: italic; \
                           text-align: center; \
                           margin-top: 2rem;"
                >
                    "*Performance measured on AMD Ryzen 9 6900HX (8 cores/16 threads). Results may vary based on hardware, dataset, and workload characteristics."
                </p>
            </div>
        </section>
    }
}
