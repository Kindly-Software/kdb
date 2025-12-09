use leptos::prelude::*;
use crate::components::molecular::FeatureCard;
use crate::utils::glassmorphism::{byzantine_background, gold_gradient_text};

#[component]
pub fn Features() -> impl IntoView {
    let features = vec![
        (
            "High Performance",
            "Optimized algorithms for fast hashing and duplicate detection on modern hardware",
        ),
        (
            "Parallel Processing",
            "Scales efficiently across multiple CPU cores for maximum throughput",
        ),
        (
            "Memory Safe",
            "100% safe Rust with zero undefined behavior and compile-time verification",
        ),
        (
            "Freemium Pricing",
            "$0.01 per 1,000 documents after 10M free monthly quota - no upfront costs",
        ),
    ];

    view! {
        <section
            class="features"
            id="features"
            style=move || format!(
                "{}; \
                 padding: 80px 2rem; \
                 position: relative;",
                byzantine_background()
            )
        >
            <div
                class="features-container"
                style="max-width: 1200px; \
                       margin: 0 auto;"
            >
                <h2
                    style=move || format!(
                        "{}; \
                         font-size: clamp(2rem, 4vw, 3rem); \
                         text-align: center; \
                         margin-bottom: 3rem;",
                        gold_gradient_text()
                    )
                >
                    "Features"
                </h2>
                <div
                    class="features-grid"
                    style="display: grid; \
                           grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); \
                           gap: 2rem; \
                           margin-top: 2rem;"
                >
                    {features
                        .into_iter()
                        .map(|(title, description)| {
                            view! { <FeatureCard title=title description=description /> }
                        })
                        .collect_view()}
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_features_compiles() {
        // Ensures component compiles
    }
}
