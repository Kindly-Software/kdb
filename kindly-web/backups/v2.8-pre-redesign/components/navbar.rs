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
            <div
                class="navbar-brand"
                style="color: #FFED4E; font-size: 1.25rem; font-weight: 700; letter-spacing: -0.01em;"
            >
                "Kindly Software"
            </div>

            <div
                class="navbar-actions"
                style="display: flex; gap: 1rem; align-items: center;"
            >
                <a
                    href="#features"
                    style="color: rgba(255, 255, 255, 0.9); text-decoration: none; font-weight: 500; transition: color 0.2s;"
                >
                    "Features"
                </a>
                <a
                    href="#pricing"
                    style="color: rgba(255, 255, 255, 0.9); text-decoration: none; font-weight: 500; transition: color 0.2s;"
                >
                    "Pricing"
                </a>
                <a
                    href="#demo"
                    style="color: rgba(255, 255, 255, 0.9); text-decoration: none; font-weight: 500; transition: color 0.2s;"
                >
                    "Demo"
                </a>
            </div>
        </nav>
    }
}
