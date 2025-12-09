//! ParallaxHero - 3-layer parallax scrolling component (T1+T3+T5)
//!
//! Leptos wrapper for ParallaxHeroCapsule with scroll-driven depth effect.

use leptos::prelude::*;
use std::sync::Arc;
use web_sys::window;
use wasm_bindgen::JsCast;

use crate::capsules::ParallaxHeroCapsule;
use crate::utils::styles::*;

/// ParallaxHero - 3-layer parallax scrolling effect
///
/// # Props
///
/// - `children` - Content to display (typically hero title/subtitle)
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::effects::ParallaxHero;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     view! {
///         <ParallaxHero>
///             <h1>"Kindly Verified"</h1>
///             <p>"AI Image Detection"</p>
///         </ParallaxHero>
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn ParallaxHero(children: Children) -> impl IntoView {
    // Create capsule instance with viewport height and max scroll
    let capsule = Arc::new(ParallaxHeroCapsule::new(100.0, 2000.0));

    // Create signal for scroll position
    let (_scroll_y, set_scroll_y) = signal(0.0);

    // Listen to window scroll events
    {
        let capsule_clone = capsule.clone();
        Effect::new(move |_| {
            let window = window().expect("window not available");
            let window_clone = window.clone();

            let handle_scroll: wasm_bindgen::prelude::Closure<dyn Fn()> = {
                let capsule_inner = capsule_clone.clone();
                let window_inner = window_clone.clone();
                wasm_bindgen::prelude::Closure::new(move || {
                    if let Ok(scroll) = window_inner.page_y_offset() {
                        set_scroll_y.set(scroll);
                        capsule_inner.update_scroll(scroll as f32);
                    }
                })
            };

            window
                .add_event_listener_with_callback(
                    "scroll",
                    handle_scroll.as_ref().unchecked_ref(),
                )
                .expect("failed to add scroll listener");

            // Note: Closure is leaked for 'static lifetime, cleanup happens on page navigation
            handle_scroll.forget();
        });
    }

    // Create memoized offset values for each layer
    let layer1_offset = {
        let capsule_clone = capsule.clone();
        Memo::new(move |_| {
            let offset_y = capsule_clone.get_layer_offset(0);
            format!("translateY({}px)", offset_y)
        })
    };

    let layer2_offset = {
        let capsule_clone = capsule.clone();
        Memo::new(move |_| {
            let offset_y = capsule_clone.get_layer_offset(1);
            format!("translateY({}px)", offset_y)
        })
    };

    let layer3_offset = {
        let capsule_clone = capsule.clone();
        Memo::new(move |_| {
            let offset_y = capsule_clone.get_layer_offset(2);
            format!("translateY({}px)", offset_y)
        })
    };

    let hero_container_style = format!(
        "{}
         min-height: 100vh;
         display: flex;
         flex-direction: column;
         align-items: center;
         justify-content: center;
         position: relative;
         overflow: hidden;
         gap: {};",
        glassmorphism(GlassBlur::Heavy, 0.1),
        SPACING_2XL
    );

    let layer_base_style = "
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
        will-change: transform;
    ";

    let layer1_style = format!(
        "{}
         background: linear-gradient(135deg, rgba(102, 51, 153, 0.15) 0%, transparent 100%);
         z-index: 1;",
        layer_base_style
    );

    let layer2_style = format!(
        "{}
         background: linear-gradient(135deg, rgba(255, 215, 0, 0.05) 0%, transparent 100%);
         z-index: 2;",
        layer_base_style
    );

    let layer3_style = format!(
        "{}
         z-index: 3;",
        layer_base_style
    );

    let content_style = format!(
        "{}
         text-align: center;
         z-index: 10;
         position: relative;",
        text_heading_xl()
    );

    view! {
        <div style=hero_container_style>
            // Layer 1 - Slowest (furthest back)
            <div
                style=layer1_style
                style:transform=layer1_offset
            />

            // Layer 2 - Medium speed
            <div
                style=layer2_style
                style:transform=layer2_offset
            />

            // Layer 3 - Fastest (closest)
            <div
                style=layer3_style
                style:transform=layer3_offset
            />

            // Content
            <div style=content_style>
                {children()}
            </div>
        </div>
    }
}
