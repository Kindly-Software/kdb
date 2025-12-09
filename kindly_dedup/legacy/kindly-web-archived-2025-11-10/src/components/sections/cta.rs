use leptos::prelude::*;
use crate::components::common::{Button, ButtonVariant, ButtonSize};

#[component]
pub fn CallToAction() -> impl IntoView {
    view! {
        <section class="cta">
            <div class="cta-content">
                <h2>"Ready to Build with Pure Rust?"</h2>
                <p>"Start building high-performance web applications today"</p>
                <Button variant=ButtonVariant::Primary size=ButtonSize::Large>
                    "Get Started Now"
                </Button>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cta_compiles() {
        // Ensures component compiles
    }
}
