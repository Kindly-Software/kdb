use leptos::prelude::*;
use crate::components::sections::{
    Hero, Performance, Features, Comparison, Demo, Pricing, ApiPreview, FAQ, CallToAction, Footer
};

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <main class="home-page">
            // 1. Hero - First impression with headline and CTA
            <Hero />

            // 2. Performance - Immediate proof of speed claims
            <Performance />

            // 3. Features - Key capabilities and benefits
            <Features />

            // 4. Comparison - Position against competitors
            <Comparison />

            // 5. Demo - Interactive proof (before pricing!)
            <Demo />

            // 6. Pricing - Commercial terms after value demonstrated
            <Pricing />

            // 7. API Preview - Developer resources
            <ApiPreview />

            // 8. FAQ - Address common concerns
            <FAQ />

            // 9. Call to Action - Final conversion opportunity
            <CallToAction />

            // 10. Footer - Contact info and legal
            <Footer />
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_page_compiles() {
        // Ensures component compiles
    }
}
