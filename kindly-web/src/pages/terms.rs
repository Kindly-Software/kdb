//! Terms of Service Page
//!
//! Standalone page for the Kindly Debugger Terms of Service.
//! Route: /terms

use leptos::prelude::*;
use crate::components::sections::Terms;
use crate::components::sections::Footer;

/// Terms of Service Page component
#[component]
pub fn TermsPage() -> impl IntoView {
    view! {
        <main class="terms-page">
            <Terms />
            <Footer />
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terms_page_compiles() {
        // Ensures component compiles
    }

    #[test]
    fn test_terms_page_renders() {
        let _ = TermsPage();
    }
}
