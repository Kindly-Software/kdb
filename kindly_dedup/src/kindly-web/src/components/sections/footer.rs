use leptos::prelude::*;

#[component]
pub fn Footer() -> impl IntoView {
    let current_year = 2025; // In production, use chrono or js_sys

    view! {
        <footer class="footer">
            <div class="footer-content">
                <div class="footer-section">
                    <h3>"Kindly Software"</h3>
                    <p>"Pure Rust WASM solutions for high-performance web applications"</p>
                </div>

                <div class="footer-section">
                    <h4>"Product"</h4>
                    <ul>
                        <li>
                            <a href="#features">"Features"</a>
                        </li>
                        <li>
                            <a href="#pricing">"Pricing"</a>
                        </li>
                        <li>
                            <a href="#docs">"Documentation"</a>
                        </li>
                    </ul>
                </div>

                <div class="footer-section">
                    <h4>"Company"</h4>
                    <ul>
                        <li>
                            <a href="#about">"About"</a>
                        </li>
                        <li>
                            <a href="#contact">"Contact"</a>
                        </li>
                        <li>
                            <a href="#blog">"Blog"</a>
                        </li>
                    </ul>
                </div>

                <div class="footer-section">
                    <h4>"Legal"</h4>
                    <ul>
                        <li>
                            <a href="#privacy">"Privacy Policy"</a>
                        </li>
                        <li>
                            <a href="#terms">"Terms of Service"</a>
                        </li>
                    </ul>
                </div>
            </div>

            <div class="footer-bottom">
                <p>{format!("© {} Kindly Software. All rights reserved.", current_year)}</p>
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
