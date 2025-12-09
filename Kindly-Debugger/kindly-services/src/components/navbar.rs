//! Premium Navbar Component
//!
//! Glassmorphic navbar with scroll effects and mobile hamburger menu.

use leptos::prelude::*;

/// Premium navigation bar with mobile hamburger menu
#[component]
pub fn Navbar() -> impl IntoView {
    // Mobile menu state
    let (menu_open, set_menu_open) = signal(false);

    let navbar_style = "
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        z-index: 100;
        padding: 1rem 2rem;
        display: flex;
        justify-content: space-between;
        align-items: center;
        background: rgba(75, 0, 130, 0.2);
        backdrop-filter: blur(20px);
        -webkit-backdrop-filter: blur(20px);
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    ";

    let logo_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.5rem;
        font-weight: 700;
        color: #fff;
        text-decoration: none;
        display: flex;
        align-items: center;
        gap: 0.5rem;
    ";

    let nav_links_style = "
        display: flex;
        gap: 2rem;
        align-items: center;
    ";

    let link_style = "
        color: rgba(255, 255, 255, 0.8);
        text-decoration: none;
        font-weight: 500;
        transition: color 0.2s ease;
    ";

    let cta_button_style = "
        background: linear-gradient(135deg, #FFD700, #FFA500);
        color: #000;
        padding: 0.75rem 1.5rem;
        border-radius: 12px;
        font-weight: 600;
        text-decoration: none;
        transition: transform 0.2s ease, box-shadow 0.2s ease;
        box-shadow: 0 4px 15px rgba(255, 215, 0, 0.3);
    ";

    // Hamburger button style (visible on mobile only via CSS)
    let hamburger_style = "
        display: none;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        width: 48px;
        height: 48px;
        background: transparent;
        border: none;
        cursor: pointer;
        padding: 0;
        z-index: 110;
    ";

    let hamburger_line_style = "
        width: 24px;
        height: 2px;
        background: #fff;
        margin: 3px 0;
        transition: all 0.3s ease;
        border-radius: 2px;
    ";

    // Mobile menu overlay style
    let mobile_menu_style = move || {
        format!(
            "
            position: fixed;
            top: 0;
            right: 0;
            width: 280px;
            height: 100vh;
            background: rgba(26, 0, 40, 0.98);
            backdrop-filter: blur(20px);
            -webkit-backdrop-filter: blur(20px);
            transform: translateX({});
            transition: transform 0.3s ease;
            z-index: 105;
            padding: 5rem 2rem 2rem;
            display: flex;
            flex-direction: column;
            gap: 1rem;
            border-left: 1px solid rgba(255, 215, 0, 0.2);
            ",
            if menu_open.get() { "0" } else { "100%" }
        )
    };

    // Mobile menu backdrop
    let backdrop_style = move || {
        format!(
            "
            position: fixed;
            top: 0;
            left: 0;
            width: 100vw;
            height: 100vh;
            background: rgba(0, 0, 0, 0.5);
            opacity: {};
            visibility: {};
            transition: opacity 0.3s ease, visibility 0.3s ease;
            z-index: 104;
            ",
            if menu_open.get() { "1" } else { "0" },
            if menu_open.get() { "visible" } else { "hidden" }
        )
    };

    let mobile_link_style = "
        color: #fff;
        text-decoration: none;
        font-size: 1.25rem;
        font-weight: 500;
        padding: 1rem 0;
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        transition: color 0.2s ease;
        display: block;
    ";

    let mobile_cta_style = "
        background: linear-gradient(135deg, #FFD700, #FFA500);
        color: #000;
        padding: 1rem 1.5rem;
        border-radius: 12px;
        font-weight: 600;
        text-decoration: none;
        text-align: center;
        margin-top: 1rem;
        display: block;
    ";

    // Close menu handler
    let close_menu = move |_| set_menu_open.set(false);
    let toggle_menu = move |_| set_menu_open.update(|v| *v = !*v);

    view! {
        <style>
            ".nav-link:hover {
                color: #FFD700 !important;
            }
            .cta-btn:hover {
                transform: translateY(-2px);
                box-shadow: 0 6px 20px rgba(255, 215, 0, 0.5) !important;
            }
            .mobile-link:hover {
                color: #FFD700 !important;
            }
            /* Show hamburger on mobile */
            @media (max-width: 768px) {
                .hamburger-btn {
                    display: flex !important;
                }
                .nav-links-desktop {
                    display: none !important;
                }
            }
            /* Hamburger animation when open */
            .hamburger-btn.open .line1 {
                transform: rotate(45deg) translate(5px, 5px);
            }
            .hamburger-btn.open .line2 {
                opacity: 0;
            }
            .hamburger-btn.open .line3 {
                transform: rotate(-45deg) translate(5px, -5px);
            }"
        </style>
        <nav style=navbar_style>
            <a href="/" style="display: flex; align-items: center; gap: 0.25rem; text-decoration: none;">
                <img src="/navbar-logo.png" alt="K" style="width: 44px; height: 44px; object-fit: contain;" />
                <div style=logo_style>
                    <span style="color: #521c68; letter-spacing: 0.05em;">"indly "</span>
                    <span style="color: #FFD700;">"Debugger"</span>
                </div>
            </a>

            // Desktop navigation
            <div style=nav_links_style class="nav-links-desktop">
                <a href="#features" style=link_style class="nav-link">"Features"</a>
                <a href="#pricing" style=link_style class="nav-link">"Pricing"</a>
                <a href="#docs" style=link_style class="nav-link">"Docs"</a>
                <a href="#signup" style=cta_button_style class="cta-btn">
                    "Start Free"
                </a>
            </div>

            // Hamburger button (mobile only)
            <button
                style=hamburger_style
                class=move || if menu_open.get() { "hamburger-btn open" } else { "hamburger-btn" }
                on:click=toggle_menu
                aria-label="Toggle menu"
            >
                <span style=hamburger_line_style class="line1"></span>
                <span style=hamburger_line_style class="line2"></span>
                <span style=hamburger_line_style class="line3"></span>
            </button>

            // Mobile menu backdrop
            <div style=backdrop_style on:click=close_menu></div>

            // Mobile menu
            <div style=mobile_menu_style>
                <a href="#features" style=mobile_link_style class="mobile-link" on:click=close_menu>"Features"</a>
                <a href="#pricing" style=mobile_link_style class="mobile-link" on:click=close_menu>"Pricing"</a>
                <a href="#docs" style=mobile_link_style class="mobile-link" on:click=close_menu>"Docs"</a>
                <a href="#signup" style=mobile_cta_style on:click=close_menu>"Start Free"</a>
            </div>
        </nav>
    }
}
