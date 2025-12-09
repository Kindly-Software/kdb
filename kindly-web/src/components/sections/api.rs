use leptos::prelude::*;
use crate::utils::glassmorphism::{byzantine_background, card_style, gold_gradient_text};

#[component]
pub fn ApiPreview() -> impl IntoView {
    let code_sample = r#"use kindly_dedup::DedupPipeline;

// Initialize pipeline
let mut pipeline = DedupPipeline::new(1_000_000);

// Add documents
for (doc_id, text) in documents {
    pipeline.add_document(doc_id, text);
}

// Find duplicates (Jaccard threshold 0.85)
let clusters = pipeline.find_duplicates(0.85)?;

println!("Found {} duplicate clusters", clusters.len());"#;

    let features = vec![
        ("Simple API", "3-line integration into your Rust project"),
        ("Type Safe", "Compile-time verification, zero runtime errors"),
        ("Efficient", "Optimized memory usage for large datasets"),
        ("Parallel Ready", "Scales to 16+ cores with excellent efficiency"),
    ];

    view! {
        <section
            class="api-preview"
            id="api"
            style=move || format!(
                "{}; \
                 padding: 80px 2rem; \
                 position: relative;",
                byzantine_background()
            )
        >
            <div style="max-width: 1200px; margin: 0 auto;">
                <div class="api-header" style="text-align: center; margin-bottom: 3rem;">
                    <h2
                        style=move || format!(
                            "{}; \
                             font-size: clamp(2rem, 4vw, 3rem); \
                             margin-bottom: 1rem;",
                            gold_gradient_text()
                        )
                    >
                        "API Preview"
                    </h2>
                    <p
                        class="api-subtitle"
                        style="color: rgba(255, 255, 255, 0.85); \
                               font-size: 1.125rem; \
                               max-width: 600px; \
                               margin: 0 auto; \
                               line-height: 1.6;"
                    >
                        "Clean, type-safe Rust API. Integrate in minutes, not days."
                    </p>
                </div>

                <div
                    class="api-content"
                    style="display: grid; \
                           grid-template-columns: 1fr 1fr; \
                           gap: 2rem; \
                           margin-bottom: 2rem;"
                >
                    <div
                        class="api-code"
                        style=move || format!(
                            "{}; \
                             padding: 0; \
                             overflow: hidden;",
                            card_style()
                        )
                    >
                        <div
                            class="code-header"
                            style="background: rgba(102, 51, 153, 0.3); \
                                   padding: 1rem 1.5rem; \
                                   display: flex; \
                                   justify-content: space-between; \
                                   align-items: center; \
                                   border-bottom: 1px solid rgba(255, 237, 78, 0.2);"
                        >
                            <span
                                class="code-language"
                                style="color: #FFD700; \
                                       font-weight: 700; \
                                       text-transform: uppercase; \
                                       font-size: 0.875rem; \
                                       letter-spacing: 0.05em;"
                            >
                                "Rust"
                            </span>
                        </div>
                        <pre
                            class="code-block"
                            style="padding: 1.5rem; \
                                   margin: 0; \
                                   overflow-x: auto; \
                                   background: rgba(0, 0, 0, 0.3);"
                        >
                            <code style="font-family: 'Courier New', monospace; \
                                        color: rgba(255, 255, 255, 0.9); \
                                        font-size: 0.875rem; \
                                        line-height: 1.6;">
                                {code_sample}
                            </code>
                        </pre>
                    </div>

                    <div class="api-features">
                        <h3
                            style=move || format!(
                                "{}; \
                                 font-size: 1.5rem; \
                                 margin-bottom: 1.5rem;",
                                gold_gradient_text()
                            )
                        >
                            "Why Developers Love It"
                        </h3>
                        <div class="api-features-list" style="display: flex; flex-direction: column; gap: 1.5rem;">
                            {features
                                .into_iter()
                                .map(|(title, description)| {
                                    view! {
                                        <div class="api-feature-item">
                                            <h4
                                                style="color: #FFED4E; \
                                                       font-size: 1.125rem; \
                                                       margin-bottom: 0.5rem; \
                                                       font-weight: 600;"
                                            >
                                                {title}
                                            </h4>
                                            <p style="color: rgba(255, 255, 255, 0.85); line-height: 1.6;">
                                                {description}
                                            </p>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    </div>
                </div>

                <div
                    class="api-docs-link"
                    style="text-align: center; \
                           color: rgba(255, 255, 255, 0.85); \
                           font-size: 1rem;"
                >
                    <p>
                        "Full documentation: "
                        <a
                            href="https://docs.rs/kindly_dedup"
                            target="_blank"
                            rel="noopener"
                            style="color: #FFD700; \
                                   text-decoration: underline; \
                                   font-weight: 600;"
                        >
                            "docs.rs/kindly_dedup"
                        </a>
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
    fn test_api_preview_compiles() {
        // Ensures component compiles
    }

    #[test]
    fn test_api_preview_renders() {
        let _ = ApiPreview();
    }
}
