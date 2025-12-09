use leptos::prelude::*;
use crate::utils::glassmorphism::{byzantine_background, card_style, gold_gradient_text};

#[derive(Clone)]
struct FaqItem {
    question: &'static str,
    answer: &'static str,
}

#[component]
pub fn FAQ() -> impl IntoView {
    let faqs = vec![
        FaqItem {
            question: "How does the free tier work?",
            answer: "10M documents per month, completely free. No credit card required. Perfect for experimentation and small projects.",
        },
        FaqItem {
            question: "What happens if I exceed the free tier?",
            answer: "Pay-as-you-go at $0.01 per 1,000 documents. No surprises, no hidden fees. You only pay for what you use above 10M/month.",
        },
        FaqItem {
            question: "Can I run this on my own infrastructure?",
            answer: "Yes! Download the demo binary (5M document limit) or purchase a production license for unlimited on-premise deployment.",
        },
        FaqItem {
            question: "How do you achieve 38-580× speedup?",
            answer: "Advanced optimization techniques including vectorized operations, efficient parallel processing, and modern algorithms. All performance claims independently validated.",
        },
        FaqItem {
            question: "What accuracy can I expect?",
            answer: "95% F1 score average. Tier 1 demo proves 100% precision and recall on 100K sample. Configurable thresholds for precision vs recall tradeoff.",
        },
        FaqItem {
            question: "Is this production-ready?",
            answer: "Yes. 100% safe Rust, zero undefined behavior, comprehensive test suite (33+ tests), and rigorous quality standards. Used in production by paying customers.",
        },
        FaqItem {
            question: "What if I need help integrating?",
            answer: "Email support@kindly.software. Standard support included with all paid tiers. Enterprise customers get priority support with SLA.",
        },
        FaqItem {
            question: "Can I use this with Python/JavaScript/other languages?",
            answer: "Currently Rust-native API only. Python bindings planned for Q1 2026. Contact sales@kindly.software for custom language bindings.",
        },
    ];

    view! {
        <section
            class="faq"
            id="faq"
            style=move || format!(
                "{}; \
                 padding: 80px 2rem; \
                 position: relative;",
                byzantine_background()
            )
        >
            <div style="max-width: 1000px; margin: 0 auto;">
                <div class="faq-header" style="text-align: center; margin-bottom: 3rem;">
                    <h2
                        style=move || format!(
                            "{}; \
                             font-size: clamp(2rem, 4vw, 3rem); \
                             margin-bottom: 1rem;",
                            gold_gradient_text()
                        )
                    >
                        "Frequently Asked Questions"
                    </h2>
                    <p
                        class="faq-subtitle"
                        style="color: rgba(255, 255, 255, 0.85); \
                               font-size: 1.125rem; \
                               line-height: 1.6;"
                    >
                        "Everything you need to know about kindly_dedup"
                    </p>
                </div>

                <div class="faq-list" style="display: flex; flex-direction: column; gap: 1.5rem; margin-bottom: 3rem;">
                    {faqs
                        .into_iter()
                        .map(|faq| {
                            view! {
                                <div
                                    class="faq-item"
                                    style=move || format!(
                                        "{}; \
                                         padding: 2rem; \
                                         transition: all 0.3s ease;",
                                        card_style()
                                    )
                                >
                                    <h3
                                        class="faq-question"
                                        style="color: #FFED4E; \
                                               font-size: 1.25rem; \
                                               font-weight: 600; \
                                               margin-bottom: 0.75rem;"
                                    >
                                        {faq.question}
                                    </h3>
                                    <p
                                        class="faq-answer"
                                        style="color: rgba(255, 255, 255, 0.85); \
                                               line-height: 1.6;"
                                    >
                                        {faq.answer}
                                    </p>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>

                <div
                    class="faq-contact"
                    style="text-align: center; \
                           color: rgba(255, 255, 255, 0.85); \
                           font-size: 1rem;"
                >
                    <p>
                        "Still have questions? Email us at "
                        <a
                            href="mailto:support@kindly.software"
                            style="color: #FFD700; \
                                   text-decoration: underline; \
                                   font-weight: 600;"
                        >
                            "support@kindly.software"
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
    fn test_faq_compiles() {
        // Ensures component compiles
    }

    #[test]
    fn test_faq_renders() {
        let _ = FAQ();
    }
}
