use leptos::prelude::*;

#[component]
pub fn Footer() -> impl IntoView {
    let current_year = 2025; // In production, use chrono or js_sys

    view! {
        <footer
            class="footer"
            style="background: rgba(26, 0, 38, 0.8); \
                   backdrop-filter: blur(16px); \
                   -webkit-backdrop-filter: blur(16px); \
                   padding: 4rem 2rem 2rem 2rem; \
                   border-top: 1px solid rgba(255, 237, 78, 0.2);"
        >
            <div
                class="footer-content"
                style="max-width: 1200px; \
                       margin: 0 auto; \
                       display: grid; \
                       grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); \
                       gap: 3rem; \
                       margin-bottom: 3rem;"
            >
                <div class="footer-section">
                    <h3
                        style="color: #FFD700; \
                               font-size: 1.25rem; \
                               margin-bottom: 1rem; \
                               font-weight: 700;"
                    >
                        "kindly_dedup"
                    </h3>
                    <p style="color: rgba(255, 255, 255, 0.7); line-height: 1.6;">
                        "Lightning-fast LLM dataset deduplication. Built with Rust for maximum performance."
                    </p>
                </div>

                <div class="footer-section">
                    <h4
                        style="color: #FFED4E; \
                               font-size: 1rem; \
                               margin-bottom: 1rem; \
                               font-weight: 600; \
                               text-transform: uppercase; \
                               letter-spacing: 0.05em;"
                    >
                        "Product"
                    </h4>
                    <ul style="list-style: none; padding: 0; display: flex; flex-direction: column; gap: 0.75rem;">
                        <li>
                            <a
                                href="#features"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "Features"
                            </a>
                        </li>
                        <li>
                            <a
                                href="#pricing"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "Pricing"
                            </a>
                        </li>
                        <li>
                            <a
                                href="#demo"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "Demo"
                            </a>
                        </li>
                        <li>
                            <a
                                href="https://docs.rs/kindly_dedup"
                                target="_blank"
                                rel="noopener"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "Documentation"
                            </a>
                        </li>
                    </ul>
                </div>

                <div class="footer-section">
                    <h4
                        style="color: #FFED4E; \
                               font-size: 1rem; \
                               margin-bottom: 1rem; \
                               font-weight: 600; \
                               text-transform: uppercase; \
                               letter-spacing: 0.05em;"
                    >
                        "Support"
                    </h4>
                    <ul style="list-style: none; padding: 0; display: flex; flex-direction: column; gap: 0.75rem;">
                        <li>
                            <a
                                href="mailto:sales@kindly.software"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "Sales"
                            </a>
                        </li>
                        <li>
                            <a
                                href="mailto:support@kindly.software"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "Support"
                            </a>
                        </li>
                        <li>
                            <a
                                href="#faq"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "FAQ"
                            </a>
                        </li>
                        <li>
                            <a
                                href="https://github.com/kindly-ai"
                                target="_blank"
                                rel="noopener"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "GitHub"
                            </a>
                        </li>
                    </ul>
                </div>

                <div class="footer-section">
                    <h4
                        style="color: #FFED4E; \
                               font-size: 1rem; \
                               margin-bottom: 1rem; \
                               font-weight: 600; \
                               text-transform: uppercase; \
                               letter-spacing: 0.05em;"
                    >
                        "Legal"
                    </h4>
                    <ul style="list-style: none; padding: 0; display: flex; flex-direction: column; gap: 0.75rem;">
                        <li>
                            <a
                                href="#privacy"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "Privacy Policy"
                            </a>
                        </li>
                        <li>
                            <a
                                href="#terms"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "Terms of Service"
                            </a>
                        </li>
                        <li>
                            <a
                                href="#compliance"
                                style="color: rgba(255, 255, 255, 0.7); \
                                       text-decoration: none; \
                                       transition: color 0.2s ease;"
                            >
                                "Compliance"
                            </a>
                        </li>
                    </ul>
                </div>
            </div>

            <div
                class="footer-disclaimer"
                style="background: rgba(102, 51, 153, 0.2); \
                       padding: 1.5rem; \
                       border-radius: 8px; \
                       margin: 0 auto 2rem auto; \
                       max-width: 1200px; \
                       border-left: 4px solid #703C8B;"
            >
                <p
                    class="disclaimer-text"
                    style="color: rgba(255, 255, 255, 0.6); \
                           font-size: 0.875rem; \
                           line-height: 1.6; \
                           margin: 0;"
                >
                    "Performance Disclaimer: All performance metrics measured on AMD Ryzen 9 6900HX (16 cores @ 3.3GHz base, 64GB DDR5-4800). "
                    "Actual performance varies by CPU architecture, memory bandwidth, core count, and dataset characteristics. "
                    "Single-threaded: 60K docs/sec. Multi-threaded: Scales efficiently to available cores. "
                    "Benchmarks validated with rigorous statistical methods (95% confidence intervals, 1000+ iterations). "
                    "Your mileage may vary. Contact "
                    <a
                        href="mailto:sales@kindly.software"
                        style="color: #FFD700; \
                               text-decoration: underline;"
                    >
                        "sales@kindly.software"
                    </a>
                    " for performance estimates on your hardware."
                </p>
            </div>

            <div
                class="footer-bottom"
                style="text-align: center; \
                       padding-top: 2rem; \
                       border-top: 1px solid rgba(255, 255, 255, 0.1); \
                       color: rgba(255, 255, 255, 0.5); \
                       font-size: 0.875rem;"
            >
                <p>
                    {format!("© {} Kindly Software. All rights reserved.", current_year)}
                    " | Built with "
                    <a
                        href="https://www.rust-lang.org/"
                        target="_blank"
                        rel="noopener"
                        style="color: #703C8B; \
                               text-decoration: none;"
                    >
                        "Rust"
                    </a>
                    " and "
                    <a
                        href="https://leptos.dev/"
                        target="_blank"
                        rel="noopener"
                        style="color: #703C8B; \
                               text-decoration: none;"
                    >
                        "Leptos"
                    </a>
                </p>
            </div>
        </footer>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_footer_compiles() {
        // Ensures component compiles
    }
}
