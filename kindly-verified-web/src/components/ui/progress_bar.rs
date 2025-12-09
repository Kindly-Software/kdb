//! ProgressBar - Smooth progress animation component (T1+T3)
//!
//! Leptos wrapper for ProgressBarCapsule with Q16.16 fixed-point
//! animations and Byzantine color theming.

use leptos::prelude::*;
use std::sync::Arc;
use wasm_bindgen::JsCast;

use crate::capsules::ProgressBarCapsule;
use crate::utils::styles::*;

/// ProgressBar - Smooth progress animation with Byzantine colors
///
/// # Props
///
/// - `progress` - Progress value (0.0-1.0)
/// - `show_percentage` - Show percentage text
/// - `show_label` - Show custom label text
/// - `color` - Color override (default: Byzantine purple to gold gradient)
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::ui::ProgressBar;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     let (progress, set_progress) = signal(0.0f32);
///
///     view! {
///         <ProgressBar
///             progress=progress
///             show_percentage=true
///             show_label=Some("Uploading...".to_string())
///         />
///         <button on:click=move |_| set_progress.set(progress.get() + 0.1)>
///             "Increment"
///         </button>
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn ProgressBar(
    progress: Signal<f32>,
    #[prop(optional)] show_percentage: Option<bool>,
    #[prop(optional)] show_label: Option<String>,
    #[prop(optional)] color: Option<String>,
) -> impl IntoView {
    // Create capsule instance
    let capsule = Arc::new(ProgressBarCapsule::new());

    // Clone for each effect
    let capsule_for_sync = capsule.clone();
    let capsule_for_get = capsule.clone();
    let capsule_for_tick = capsule.clone();

    // Sync progress to capsule
    Effect::new(move |_| {
        let value = progress.get().max(0.0).min(1.0);
        capsule_for_sync.set_progress(value);
    });

    // Get animated progress from capsule with Q16.16 easing
    let (animated_progress, set_animated_progress) = signal(0.0f32);

    Effect::new(move |_| {
        let current = capsule_for_get.get_progress();
        set_animated_progress.set(current);
    });

    // Animation frame updates
    Effect::new(move |_| {
        let capsule_clone = capsule_for_tick.clone();
        if let Some(window) = web_sys::window() {
            let tick: wasm_bindgen::prelude::Closure<dyn Fn()> = {
                wasm_bindgen::prelude::Closure::new(move || {
                    capsule_clone.tick(16); // 16ms per frame
                    set_animated_progress.set(capsule_clone.get_progress());
                })
            };

            let _ = window.request_animation_frame(tick.as_ref().unchecked_ref()).ok();
            tick.forget();
        }
    });

    // Container styles
    let container_style = "
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
        width: 100%;
    ";

    let header_style = "
        display: flex;
        justify-content: space-between;
        align-items: center;
    ";

    let (label_style, _) = signal(format!(
        "{}",
        text_body()
    ));

    let (percentage_style, _) = signal(format!(
        "{}
         color: #FFD700;
         font-weight: 600;",
        text_body()
    ));

    let bar_container_style = "
        width: 100%;
        height: 8px;
        background: rgba(255, 215, 0, 0.1);
        border-radius: 4px;
        overflow: hidden;
        border: 1px solid rgba(255, 215, 0, 0.2);
    ";

    let bar_fill_style = move || {
        let percentage = (animated_progress.get() * 100.0).clamp(0.0, 100.0);
        let color = color.clone().unwrap_or_else(|| {
            if animated_progress.get() >= 1.0 {
                "linear-gradient(90deg, #FFD700, #663399)".to_string()
            } else if animated_progress.get() >= 0.8 {
                "linear-gradient(90deg, #10B981, #FFD700)".to_string()
            } else if animated_progress.get() >= 0.5 {
                "linear-gradient(90deg, #F59E0B, #FFD700)".to_string()
            } else {
                "linear-gradient(90deg, #3B82F6, #663399)".to_string()
            }
        });

        format!(
            "height: 100%;
             width: {}%;
             background: {};
             transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
             border-radius: 4px;
             box-shadow: 0 0 8px rgba(255, 215, 0, 0.3);",
            percentage, color
        )
    };

    let animated_percentage = move || {
        format!("{:.0}%", (animated_progress.get() * 100.0).clamp(0.0, 100.0))
    };

    let completion_style = move || {
        if animated_progress.get() >= 1.0 {
            format!(
                "{}
                 color: #10B981;
                 animation: pulse 2s infinite;",
                text_caption()
            )
        } else {
            "".to_string()
        }
    };

    let label_text = show_label.clone();
    let label_text_for_map = show_label.clone();
    let show_pct = show_percentage.unwrap_or(false);

    view! {
        <div style=container_style>
            <Show
                when=move || label_text.is_some() || show_pct
            >
                <div style=header_style>
                    {label_text_for_map.as_ref().map(|label| {
                        view! {
                            <div style=move || label_style.get()>
                                {label.clone()}
                            </div>
                        }
                    })}

                    <Show when=move || show_pct>
                        <div style=move || percentage_style.get()>
                            {animated_percentage}
                        </div>
                    </Show>
                </div>
            </Show>

            <div style=bar_container_style>
                <div style=bar_fill_style></div>
            </div>

            <Show when=move || { animated_progress.get() >= 1.0f32 }>
                <div style=completion_style>
                    "✓ Complete"
                </div>
            </Show>
        </div>
    }
}
