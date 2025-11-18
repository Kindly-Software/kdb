use crate::components::molecular::FeatureCard;
use leptos::prelude::*;

#[component]
pub fn Features() -> impl IntoView {
    let features = vec![
        (
            "Lockfree Architecture",
            "100% lockfree coordination using atomic capsules for predictable sub-100ns latency",
        ),
        (
            "SIMD Acceleration",
            "2-19× speedups with vectorized computation and fixed-point arithmetic",
        ),
        (
            "Zero Dependencies",
            "Pure Rust implementation with no external runtime dependencies",
        ),
        (
            "Type Safety",
            "Compile-time verification with computational capsule architecture",
        ),
    ];

    view! {
        <section class="features">
            <div class="features-container">
                <h2>"Features"</h2>
                <div class="features-grid">
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
