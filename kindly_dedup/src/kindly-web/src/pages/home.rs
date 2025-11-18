use crate::components::sections::{CallToAction, Features, Footer, Hero, Pricing};
use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <main class="home-page">
            <Hero />
            <Features />
            <Pricing />
            <CallToAction />
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
