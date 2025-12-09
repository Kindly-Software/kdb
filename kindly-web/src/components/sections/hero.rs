use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys;
use crate::components::common::{Button, ButtonVariant, ButtonSize};
use crate::utils::glassmorphism::{hero_gradient, gold_gradient_text, hero_overlay};

#[component]
pub fn Hero() -> impl IntoView {
    let (copied, set_copied) = signal(false);

    view! {
        <section
            class="hero"
            style=move || format!(
                "{}; \
                 min-height: auto; \
                 display: flex; \
                 flex-direction: column; \
                 justify-content: flex-start; \
                 align-items: center; \
                 text-align: center; \
                 position: relative; \
                 padding: 10rem 2rem 8rem; \
                 overflow: hidden; \
                 z-index: 1;",
                hero_gradient()
            )
        >
            // Gradient overlay for depth
            <div
                style=move || format!(
                    "{}; \
                     position: absolute; \
                     top: 0; \
                     left: 0; \
                     right: 0; \
                     bottom: 0; \
                     pointer-events: none;",
                    hero_overlay()
                )
            />

            <div
                class="hero-content"
                style="max-width: 1200px; \
                       margin: 0 auto; \
                       text-align: center; \
                       position: relative; \
                       z-index: 1;"
            >
                <h1
                    style=move || format!(
                        "{}; \
                         font-family: 'Space Grotesk', sans-serif; \
                         font-size: clamp(2.2rem, 6vw, 3.5rem); \
                         font-weight: 700; \
                         margin-bottom: 1.5rem; \
                         line-height: 1.2; \
                         max-width: 900px;",
                        gold_gradient_text()
                    )
                >
                    "Deduplicate Datasets in Seconds, Not Hours"
                </h1>

                <p
                    class="hero-subtitle"
                    style="font-size: clamp(1.05rem, 2.5vw, 1.35rem); \
                           color: rgba(255, 255, 255, 0.7); \
                           max-width: 650px; \
                           margin-bottom: 1rem; \
                           line-height: 1.6;"
                >
                    "Up to 190× faster than Python | 300K docs/sec | Production-ready Rust"
                </p>

                <p
                    class="hero-disclaimer"
                    style="font-size: 0.875rem; \
                           color: rgba(255, 255, 255, 0.7); \
                           margin-bottom: 2rem; \
                           font-style: italic;"
                >
                    "Benchmarked on AMD Ryzen 9 6900HX (8 cores/16 threads, 64GB RAM). Single-threaded: 38× faster."
                </p>

                // Quick Start Docker Command (moved above buttons)
                <div
                    class="hero-docker"
                    style="max-width: 700px; \
                           margin: 0 auto 3rem auto;"
                >
                    <div
                        style="color: rgba(255, 255, 255, 0.9); \
                               font-size: 0.875rem; \
                               font-weight: 700; \
                               text-transform: uppercase; \
                               letter-spacing: 0.05em; \
                               margin-bottom: 0.75rem; \
                               text-align: center;"
                    >
                        "Quick Start"
                    </div>
                    <div
                        on:click=move |_| {
                            if let Some(window) = web_sys::window() {
                                let clipboard = window.navigator().clipboard();
                                let _ = clipboard.write_text("docker pull samuelduchaine/kindly-dedup:trial");
                                set_copied.set(true);

                                // Auto-hide notification after 2 seconds
                                set_timeout(move || {
                                    set_copied.set(false);
                                }, std::time::Duration::from_secs(2));
                            }
                        }
                        style="background: linear-gradient(135deg, rgba(0, 0, 0, 0.9), rgba(26, 0, 40, 0.95)); \
                               backdrop-filter: blur(16px); \
                               -webkit-backdrop-filter: blur(16px); \
                               padding: 1.5rem 1.5rem; \
                               border-radius: 12px; \
                               border: 2px solid #FFD700; \
                               box-shadow: 0 16px 48px rgba(255, 215, 0, 0.4), \
                                           0 0 20px rgba(255, 215, 0, 0.3), \
                                           inset 0 1px 0 rgba(255, 255, 255, 0.1); \
                               cursor: pointer; \
                               transition: all 0.2s ease; \
                               position: relative; \
                               animation: pulse-glow 2s ease-in-out infinite;"
                        on:mouseenter=move |e| {
                            if let Some(target) = e.target() {
                                let _ = target.dyn_ref::<web_sys::HtmlElement>().map(|el| {
                                    let _ = el.style().set_property("border-color", "#FFED4E");
                                    let _ = el.style().set_property("box-shadow", "0 20px 60px rgba(255, 215, 0, 0.5), 0 0 30px rgba(255, 215, 0, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.1)");
                                    let _ = el.style().set_property("transform", "translateY(-2px) scale(1.02)");
                                });
                            }
                        }
                        on:mouseleave=move |e| {
                            if let Some(target) = e.target() {
                                let _ = target.dyn_ref::<web_sys::HtmlElement>().map(|el| {
                                    let _ = el.style().set_property("border-color", "#FFD700");
                                    let _ = el.style().set_property("box-shadow", "0 16px 48px rgba(255, 215, 0, 0.4), 0 0 20px rgba(255, 215, 0, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.1)");
                                    let _ = el.style().set_property("transform", "translateY(0) scale(1)");
                                });
                            }
                        }
                    >
                        <code
                            style="display: block; \
                                   font-family: 'Courier New', monospace; \
                                   color: #FFD700; \
                                   font-size: 1rem; \
                                   font-weight: 600; \
                                   text-align: center; \
                                   word-break: break-all; \
                                   line-height: 1.6; \
                                   user-select: all;"
                        >
                            "docker pull samuelduchaine/kindly-dedup:trial"
                        </code>
                        <div
                            style="margin-top: 0.75rem; \
                                   font-size: 0.75rem; \
                                   color: #FFD700; \
                                   text-align: center; \
                                   text-transform: uppercase; \
                                   letter-spacing: 0.1em; \
                                   font-weight: 600;"
                        >
                            "📋 Click to copy"
                        </div>
                    </div>
                </div>

                // Copied notification
                <div
                    style=move || format!(
                        "position: fixed; \
                         top: 7rem; \
                         right: 2rem; \
                         background: linear-gradient(135deg, rgba(75, 0, 130, 0.95), rgba(138, 43, 226, 0.95)); \
                         backdrop-filter: blur(16px); \
                         -webkit-backdrop-filter: blur(16px); \
                         padding: 1rem 2rem; \
                         border-radius: 12px; \
                         border: 2px solid rgba(255, 215, 0, 0.8); \
                         box-shadow: 0 8px 32px rgba(255, 215, 0, 0.4); \
                         color: #FFD700; \
                         font-weight: 700; \
                         font-size: 1rem; \
                         z-index: 1000; \
                         transition: all 0.3s ease; \
                         opacity: {}; \
                         transform: translateX({}); \
                         pointer-events: none;",
                        if copied.get() { "1" } else { "0" },
                        if copied.get() { "0" } else { "100px" }
                    )
                >
                    "✓ Copied to clipboard"
                </div>

                // Action buttons (moved below Docker command)
                <div
                    class="hero-actions"
                    style="display: flex; \
                           gap: 1.5rem; \
                           justify-content: center; \
                           flex-wrap: wrap;"
                >
                    <Button
                        variant=ButtonVariant::Secondary
                        size=ButtonSize::Large
                        on_click=Box::new(move || {
                            if let Some(window) = web_sys::window() {
                                let _ = window.location().set_hash("#performance");
                            }
                        })
                    >
                        "View Benchmarks"
                    </Button>
                    <Button
                        variant=ButtonVariant::Outlined
                        size=ButtonSize::Large
                        on_click=Box::new(move || {
                            if let Some(window) = web_sys::window() {
                                let _ = window.location().set_hash("#features");
                            }
                        })
                    >
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
