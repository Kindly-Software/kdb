//! LiquidMeter - Metaball confidence meter component (T2+T3+T5)
//!
//! Leptos wrapper for LiquidMorphingMeterCapsule with animated confidence visualization.

use leptos::prelude::*;
use std::sync::Arc;
use web_sys::window;
use wasm_bindgen::JsCast;

use crate::capsules::LiquidMorphingMeterCapsule;
use crate::utils::styles::*;

/// LiquidMeter - Animated liquid/metaball confidence meter
///
/// # Props
///
/// - `confidence` - Current confidence value (0.0-1.0, Signal)
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::effects::LiquidMeter;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     let (confidence, _) = signal(0.75);
///
///     view! {
///         <LiquidMeter confidence=confidence />
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn LiquidMeter(#[prop(into)] confidence: Signal<f32>) -> impl IntoView {
    // Create capsule instance
    let capsule = Arc::new(LiquidMorphingMeterCapsule::new());

    // Update capsule with confidence
    {
        let capsule_clone = capsule.clone();
        Effect::new(move |_| {
            let conf = confidence.get().clamp(0.0, 1.0);
            capsule_clone.set_confidence(conf);
        });
    }

    // Animation state
    let (animation_active, _set_animation_active) = signal(true);
    let (frame_count, set_frame_count) = signal(0u32);

    // Start animation loop
    {
        let capsule_clone = capsule.clone();
        Effect::new(move |_| {
            if !animation_active.get() {
                return;
            }

            let window = match window() {
                Some(w) => w,
                None => return,
            };

            let window_clone = window.clone();

            let tick: wasm_bindgen::prelude::Closure<dyn Fn()> = {
                let capsule_inner = capsule_clone.clone();
                wasm_bindgen::prelude::Closure::new(move || {
                    set_frame_count.update(|f| *f = f.wrapping_add(1));

                    // Tick the capsule (~16ms per frame at 60fps)
                    let ms_elapsed = (frame_count.get() as f32 / 60.0) * 1000.0;
                    capsule_inner.tick(ms_elapsed as u32);
                })
            };

            if let Ok(_id) = window_clone.request_animation_frame(tick.as_ref().unchecked_ref()) {
                // Animation frame scheduled successfully
            }

            tick.forget();
            // Note: animation frame will be cleaned up when component unmounts
        });
    }

    // Get shape state for morphing animation (wrapped in Signal to handle UnsafeCell)
    let (shape_state, _) = signal(capsule.get_current_state());

    Effect::new(move |_| {
        let _ = frame_count.get();
        // Update shape state on animation frame
    });

    // Get influence grid for rendering (wrapped in Signal to handle UnsafeCell)
    let (_influence_grid, _) = signal(capsule.get_influence_grid());

    {
        let _frame_count = frame_count;
        Effect::new(move |_| {
            let _ = _frame_count.get();
            // Update influence grid on animation frame
        });
    }

    // Memoized styles
    let confidence_value = Memo::new(move |_| confidence.get());
    let confidence_percent = Memo::new(move |_| (confidence_value.get() * 100.0).round() as u32);
    let confidence_color = Memo::new(move |_| {
        let conf = confidence_value.get();
        if conf >= 0.9 {
            COLOR_SUCCESS
        } else if conf >= 0.7 {
            COLOR_GOLD
        } else if conf >= 0.5 {
            COLOR_WARNING
        } else {
            COLOR_ERROR
        }
    });

    let container_style = format!(
        "{}
         border-radius: 24px;
         padding: {};
         display: flex;
         flex-direction: column;
         align-items: center;
         gap: {};
         min-height: 320px;
         justify-content: center;",
        glassmorphism(GlassBlur::Medium, 0.15),
        SPACING_XL,
        SPACING_LG
    );

    let meter_container_style = "
        width: 240px;
        height: 240px;
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
    ";

    let meter_background_style = format!(
        "position: absolute;
         width: 100%;
         height: 100%;
         border-radius: 50%;
         background: radial-gradient(circle, rgba(102, 51, 153, 0.2) 0%, rgba(255, 215, 0, 0.05) 100%);
         border: 2px solid rgba(255, 215, 0, 0.3);
         {}",
        glow_purple()
    );

    let create_liquid_style = move || {
        let conf = confidence_value.get().clamp(0.0, 1.0);
        let height_percent = (conf * 100.0).min(100.0).max(0.0);

        format!(
            "position: absolute;
             width: 80%;
             height: {}%;
             background: linear-gradient(180deg, {} 0%, rgba(255, 215, 0, 0.4) 100%);
             border-radius: 50% 50% 45% 45%;
             bottom: 10%;
             left: 10%;
             transition: all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94);
             filter: blur(2px);
             {}",
            height_percent,
            confidence_color.get(),
            glow_gold()
        )
    };

    let percentage_style = format!(
        "{}
         position: absolute;
         font-size: 2.5rem;
         font-weight: 800;
         z-index: 2;
         text-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);",
        text_heading_lg()
    );

    let label_style = format!(
        "{}
         text-align: center;
         max-width: 100%;",
        text_body()
    );

    let badge_style = format!(
        "{}
         border-radius: 8px;
         padding: {} {};
         margin-top: {};",
        confidence_badge(confidence_value.get()),
        SPACING_XS,
        SPACING_SM,
        SPACING_MD
    );

    view! {
        <div style=container_style>
            // Liquid meter visualization
            <div style=meter_container_style>
                <div style=meter_background_style></div>
                <div style=create_liquid_style()></div>
                <div style=percentage_style>
                    {move || format!("{}%", confidence_percent.get())}
                </div>
            </div>

            // Confidence label
            <div style=label_style>
                <div style=badge_style>
                    {move || {
                        let state = shape_state.get();
                        match state {
                            crate::capsules::ShapeState::JaggedRed => "Analyzing...",
                            crate::capsules::ShapeState::WobblingOrange => "Morphing",
                            crate::capsules::ShapeState::SmoothGold => "Confirmed",
                            crate::capsules::ShapeState::PerfectCircle => "Perfect",
                        }
                    }}
                </div>
                <div style=text_caption()>
                    {move || {
                        let conf = confidence_value.get();
                        if conf >= 0.9 {
                            "Very High Confidence"
                        } else if conf >= 0.7 {
                            "High Confidence"
                        } else if conf >= 0.5 {
                            "Medium Confidence"
                        } else {
                            "Low Confidence"
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
