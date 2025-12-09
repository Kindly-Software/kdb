use leptos::prelude::*;
use crate::components::common::{Button, ButtonVariant, ButtonSize};

#[component]
pub fn Hero() -> impl IntoView {
    view! {
        <section class="hero">
            <div class="hero-content">
                <h1>"Pure Rust WASM Solutions"</h1>
                <p class="hero-subtitle">
                    "High-performance web applications built with computational capsules and lockfree architecture"
                </p>
                <div class="hero-actions">
                    <Button variant=ButtonVariant::Primary size=ButtonSize::Large>
                        "Get Started"
                    </Button>
                    <Button variant=ButtonVariant::Secondary size=ButtonSize::Large>
                        "Learn More"
                    </Button>
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hero_compiles() {
        // Ensures component compiles
    }
}
