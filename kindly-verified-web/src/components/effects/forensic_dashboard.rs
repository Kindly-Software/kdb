//! ForensicDashboard - Image forensics detection dashboard component (T2+T5+T1)
//!
//! Leptos wrapper for ForensicDashboardCapsule with animated 10-bar detector visualization.

use leptos::prelude::*;
use std::sync::Arc;
use web_sys::window;

use crate::capsules::{ForensicDashboardCapsule, BarData};
use crate::utils::styles::*;

/// ForensicDashboard - Animated 10-bar detector visualization
///
/// # Props
///
/// - `detector_results` - Optional Vec of 10 confidence values (0.0-1.0)
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::effects::ForensicDashboard;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     let results = vec![0.95, 0.85, 0.72, 0.68, 0.91, 0.54, 0.79, 0.88, 0.67, 0.82];
///
///     view! {
///         <ForensicDashboard detector_results=Some(results) />
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn ForensicDashboard(
    #[prop(optional)] detector_results: Option<Vec<f32>>,
) -> impl IntoView {
    // Create capsule instance
    let capsule = Arc::new(ForensicDashboardCapsule::new());

    // Update capsule with detector results if provided
    if let Some(results) = detector_results {
        for (i, confidence) in results.iter().enumerate().take(10) {
            capsule.update_detector(i, *confidence);
        }
    }

    // Start animation on mount
    capsule.start_animation();

    // Create signal for animation frame counter
    let (_frame_count, set_frame_count) = signal(0u32);
    let (animation_active, _set_animation_active) = signal(true);

    // Clone capsule for effects
    let capsule_for_effect = capsule.clone();
    let capsule_for_memo = capsule.clone();

    // Start animation loop on mount
    Effect::new(move |_| {
        if !animation_active.get() {
            return;
        }

        let _window = window().expect("window not available");

        // Tick capsule directly on each effect trigger
        capsule_for_effect.tick_animation(16);
        set_frame_count.update(|f| *f = f.wrapping_add(1));
    });

    // Detector names (defined early to avoid move issues)
    let detector_names = vec![
        "EXIF Seal",
        "Chromatic Guard",
        "Compression",
        "Noise Pattern",
        "Frequency Domain",
        "Edge Consistency",
        "Color Distribution",
        "Metadata Chain",
        "Statistical Harmony",
        "Neural Pattern",
    ];

    // Get all bar data
    let bar_data = Memo::new(move |_| {
        capsule_for_memo.get_all_bars().to_vec()
    });

    let container_style = format!(
        "{}
         border-radius: 16px;
         padding: {};
         display: grid;
         grid-template-columns: repeat(auto-fit, minmax(100px, 1fr));
         gap: {};
         min-height: 280px;",
        glassmorphism(GlassBlur::Medium, 0.15),
        SPACING_LG,
        SPACING_MD
    );

    let bar_container_style = "
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.5rem;
    ";

    let bar_label_style = format!(
        "{}
         font-size: 0.75rem;
         text-align: center;
         line-height: 1.2;
         word-break: break-word;",
        text_caption()
    );

    let bar_track_style = "
        width: 24px;
        height: 120px;
        background: rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        border: 1px solid rgba(255, 215, 0, 0.2);
        position: relative;
        overflow: hidden;
    ";

    let create_bar_fill_style = move |bar_data: BarData| {
        let height_percent = (bar_data.progress * 100.0).min(100.0).max(0.0);
        let color = confidence_color(bar_data.confidence);

        format!(
            "width: 100%;
             height: {}%;
             background: linear-gradient(to top, {}, rgba(255, 215, 0, 0.3));
             transition: height 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94);
             position: absolute;
             bottom: 0;
             border-radius: 0 0 12px 12px;",
            height_percent, color
        )
    };

    let confidence_badge_style = "
        font-size: 0.75rem;
        font-weight: 600;
        color: #FFD700;
    ";

    let detector_names_arc = Arc::new(detector_names.clone());

    view! {
        <div style=container_style>
            <For
                each=move || bar_data.get().into_iter().enumerate()
                key=|(i, _)| *i
                children=move |(i, bar)| {
                    let detector_names_ref = detector_names_arc.clone();
                    let label = detector_names_ref.get(i).copied().unwrap_or("Unknown");
                    let confidence_pct = (bar.confidence * 100.0).round() as u32;
                    let bar_fill = create_bar_fill_style(bar);

                    view! {
                        <div style=bar_container_style>
                            <div style=bar_label_style.clone()>{label}</div>
                            <div style=bar_track_style>
                                <div style=bar_fill></div>
                            </div>
                            <div style=confidence_badge_style>{confidence_pct}%</div>
                        </div>
                    }
                }
            />
        </div>
    }
}
