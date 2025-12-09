//! Navbar Component
//!
//! Top navigation bar with logo and links.

use leptos::prelude::*;
use kindly_ui::theme::colors::*;

#[component]
pub fn Navbar() -> impl IntoView {
    let navbar_style = format!(
        "position: fixed;
         top: 0;
         left: 0;
         right: 0;
         z-index: 100;
         padding: 1rem 1.5rem;
         display: flex;
         justify-content: space-between;
         align-items: center;
         gap: 1rem;
         background: rgba(75, 0, 130, 0.2);
         backdrop-filter: blur(20px);
         -webkit-backdrop-filter: blur(20px);
         border-bottom: 1px solid {};",
        GLASS_BORDER
    );

    let logo_container_style = "
        display: flex;
        align-items: center;
        gap: 0.75rem;
        text-decoration: none;
    ";

    let logo_style = "
        width: 40px;
        height: 40px;
        border-radius: 50%;
    ";

    let logo_text_style = format!(
        "font-family: {};
         font-size: 1.25rem;
         font-weight: 700;
         color: {};",
        FONT_HEADING,
        TEXT_PRIMARY
    );

    let nav_links_style = "
        display: flex;
        gap: 1.5rem;
        align-items: center;
        margin-left: auto;
    ";

    let link_style = format!(
        "color: {};
         text-decoration: none;
         font-weight: 500;
         font-size: 0.9375rem;
         transition: color 0.2s ease;",
        TEXT_SECONDARY
    );

    view! {
        <style>
            ".nav-link:hover {
                color: #FFD700 !important;
            }"
        </style>
        <nav style=navbar_style>
            <a href="/" style=logo_container_style>
                <img src="/kdb-logo-simple.png" alt="KDB Logo" style=logo_style />
                <div>
                    <span style=logo_text_style>"KDB "</span>
                    <span style="color: #FFD700; font-family: 'Space Grotesk', sans-serif; font-weight: 700; font-size: 1.25rem;">
                        "API"
                    </span>
                </div>
            </a>

            <div style=nav_links_style>
                <a href="#stats" style=link_style.clone() class="nav-link">"Stats"</a>
                <a href="#endpoints" style=link_style.clone() class="nav-link">"Endpoints"</a>
                <a href="https://kindly.software" style=link_style.clone() class="nav-link" target="_blank">"Docs"</a>
            </div>
        </nav>
    }
}
