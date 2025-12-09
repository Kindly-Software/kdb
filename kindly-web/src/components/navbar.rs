use leptos::prelude::*;
use crate::utils::{glassmorphism::navbar_blur_responsive, layout::use_scroll_y};

#[component]
pub fn Navbar() -> impl IntoView {
    let scroll_y = use_scroll_y();

    let navbar_style = move || navbar_blur_responsive(scroll_y.get());

    view! {
        <nav
            class="navbar"
            style=move || format!(
                "{}; \
                 position: fixed; \
                 top: 0; \
                 left: 0; \
                 right: 0; \
                 z-index: 1000; \
                 padding: 1rem 2rem; \
                 display: flex; \
                 align-items: center; \
                 justify-content: space-between;",
                navbar_style()
            )
        >
            <a
                href="/"
                class="navbar-brand"
                style="color: #FFD700; \
                       font-size: 1.5rem; \
                       font-weight: 800; \
                       letter-spacing: -0.01em; \
                       text-shadow: 0 2px 4px rgba(255, 215, 0, 0.4); \
                       text-decoration: none; \
                       cursor: pointer;"
            >
                "Kindly Software 💜"
            </a>

            <div
                class="navbar-actions"
                style="display: flex; gap: 1.5rem; align-items: center;"
            >
                <a
                    href="#features"
                    style="color: rgba(255, 255, 255, 0.9); \
                           text-decoration: none; \
                           font-weight: 600; \
                           transition: all 0.2s; \
                           text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);"
                    onmouseenter="this.style.color='#FFD700'"
                    onmouseleave="this.style.color='rgba(255, 255, 255, 0.9)'"
                >
                    "Features"
                </a>
                <a
                    href="/pricing"
                    style="color: rgba(255, 255, 255, 0.9); \
                           text-decoration: none; \
                           font-weight: 600; \
                           transition: all 0.2s; \
                           text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);"
                    onmouseenter="this.style.color='#FFD700'"
                    onmouseleave="this.style.color='rgba(255, 255, 255, 0.9)'"
                >
                    "Pricing"
                </a>
                <a
                    href="#demo"
                    style="color: rgba(255, 255, 255, 0.9); \
                           text-decoration: none; \
                           font-weight: 600; \
                           transition: all 0.2s; \
                           text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);"
                    onmouseenter="this.style.color='#FFD700'"
                    onmouseleave="this.style.color='rgba(255, 255, 255, 0.9)'"
                >
                    "Demo"
                </a>
            </div>
        </nav>
    }
}
