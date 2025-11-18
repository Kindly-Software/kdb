use leptos::prelude::*;

#[component]
pub fn Navbar() -> impl IntoView {
    view! {
        <nav class="navbar">
            <div class="navbar-container">
                <div class="navbar-logo">
                    <a href="/" class="logo-link">
                        <span class="logo-text">"Kindly"</span>
                    </a>
                </div>
                <div class="navbar-links">
                    <a href="#features" class="nav-link">"Features"</a>
                    <a href="#pricing" class="nav-link">"Pricing"</a>
                    <a href="#docs" class="nav-link">"Docs"</a>
                    <a href="#about" class="nav-link">"About"</a>
                </div>
                <div class="navbar-cta">
                    <a href="/signup" class="btn btn-primary">"Get Started"</a>
                </div>
            </div>
        </nav>
    }
}
