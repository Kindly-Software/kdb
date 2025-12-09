use leptos::prelude::*;
use std::time::Duration;
use std::sync::Arc;
use crate::components::UploadZone;
use crate::capsules::{
    ParallaxHeroCapsule, LiquidMorphingMeterCapsule,
    ForensicDashboardCapsule, NeomorphGlassButtonCapsule,
};
use crate::utils::*;

/// Test page with all 5 computational capsule effects integrated
#[component]
pub fn TestPage() -> impl IntoView {
    // State signals
    let (image_data, set_image_data) = signal(None::<Vec<u8>>);
    let (image_name, set_image_name) = signal(String::new());
    let (is_analyzing, set_is_analyzing) = signal(false);
    let (confidence, set_confidence) = signal(0.0f32);
    let (detector_confidences, set_detector_confidences) = signal(vec![0.0f32; 10]);
    let (detector_results, set_detector_results) = signal(None::<Vec<f32>>);

    // Parallax hero capsule reference (for potential scroll integration)
    let _parallax_capsule = ParallaxHeroCapsule::new(800.0, 2000.0);

    // Liquid morphing meter capsule (Arc-wrapped by new())
    let liquid_meter = LiquidMorphingMeterCapsule::new();

    // Forensic dashboard capsule (10 detectors) - wrap in Arc for sharing across closures
    let forensic_dashboard = Arc::new(ForensicDashboardCapsule::new());

    // Neomorph button capsule
    let reset_button = Arc::new(NeomorphGlassButtonCapsule::new(0x663399, 0xFFD700));

    // Simulated detection with 3-second delay
    let liquid_meter_clone1 = liquid_meter.clone();
    let forensic_dashboard_clone1 = forensic_dashboard.clone();
    Effect::new(move |_| {
        if is_analyzing.get() {
            log::info!("State: analyzing = true");

            // Update liquid meter to 0 initially
            liquid_meter_clone1.set_confidence(0.0);

            // Clone again for inner set_timeout closure
            let liquid_meter_inner = liquid_meter_clone1.clone();
            let forensic_dashboard_inner = forensic_dashboard_clone1.clone();

            // Simulate detection after 3 seconds
            set_timeout(
                move || {
                    // Simulate detection results
                    let mock_confidences = vec![
                        0.85, // EXIF Integrity Seal
                        0.72, // Chromatic Aberration Guard
                        0.91, // Compression Artifact Sentinel
                        0.68, // Noise Pattern Oracle
                        0.88, // Frequency Domain Augur
                        0.75, // Edge Consistency Praetor
                        0.82, // Color Distribution Legate
                        0.79, // Metadata Chain Curator
                        0.86, // Statistical Harmony Consul
                        0.90, // Neural Pattern Imperator
                    ];

                    // Calculate average confidence
                    let avg_confidence = mock_confidences.iter().sum::<f32>() / mock_confidences.len() as f32;

                    // Update state
                    set_detector_confidences.set(mock_confidences.clone());
                    set_confidence.set(avg_confidence);
                    set_detector_results.set(Some(mock_confidences));

                    // Update liquid meter to final confidence
                    liquid_meter_inner.set_confidence(avg_confidence);

                    // Update forensic dashboard with detector results
                    for (i, &conf) in detector_results.get().unwrap_or_default().iter().enumerate() {
                        forensic_dashboard_inner.update_detector(i, conf);
                    }

                    // Start dashboard animation
                    forensic_dashboard_inner.start_animation();

                    // Stop analyzing
                    set_is_analyzing.set(false);
                    log::info!("State: analyzing = false, confidence = {:.2}", avg_confidence);
                },
                Duration::from_secs(3),
            );
        }
    });

    // Animate liquid meter during morphing
    let liquid_meter_clone2 = liquid_meter.clone();
    Effect::new(move |_| {
        if is_analyzing.get() {
            // Clone again for inner set_interval closure
            let liquid_meter_interval = liquid_meter_clone2.clone();

            // Animate meter morphing
            set_interval(
                move || {
                    liquid_meter_interval.tick(16); // Tick with 16ms delta
                },
                Duration::from_millis(16),
            );
        }
    });

    let _reset_btn = reset_button.clone();
    let reset_btn_for_click = reset_button.clone();

    // Pre-compute styles outside view! macro to avoid FnOnce capture
    let container_style = "position: relative;
             min-height: 100vh;
             overflow-x: hidden;";

    let (parallax_layer_style, _) = signal(format!(
        "position: fixed;
         top: 0;
         left: 0;
         width: 100%;
         height: 100%;
         z-index: -1;
         background: {};
         pointer-events: none;",
        gradient_hero()
    ));

    view! {
        // Effect 1: ParallaxHero (full-page parallax background)
        <div style=container_style>
            // Parallax Background Layers
            <div style=move || parallax_layer_style.get()>
                // Layer 0: Purple Nebula (0.2× scroll speed)
                <div style="
                    position: absolute;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 150%;
                    background: linear-gradient(135deg, #1a0033 0%, #2d1b4e 100%);
                    opacity: 0.6;
                    transform: translateY(var(--parallax-0, 0px));
                "></div>

                // Layer 1: Gold Particles (0.5× scroll speed)
                <div style="
                    position: absolute;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100%;
                    background: radial-gradient(circle 2px at 20% 30%, rgba(255, 215, 0, 0.3), transparent 2px),
                                radial-gradient(circle 2px at 60% 70%, rgba(255, 215, 0, 0.2), transparent 2px),
                                radial-gradient(circle 2px at 80% 10%, rgba(255, 215, 0, 0.25), transparent 2px),
                                radial-gradient(circle 2px at 10% 90%, rgba(255, 215, 0, 0.2), transparent 2px),
                                radial-gradient(circle 2px at 40% 50%, rgba(255, 215, 0, 0.3), transparent 2px);
                    background-size: 500px 500px;
                    transform: translateY(var(--parallax-1, 0px));
                "></div>
            </div>

            // Main content container
            <div style=format!(
                "position: relative;
                 z-index: 1;
                 padding: {} {};
                 min-height: 100vh;
                 display: flex;
                 flex-direction: column;
                 justify-content: center;",
                SPACING_2XL,
                SPACING_MD
            )>
                <div style=format!(
                    "max-width: 1200px;
                     margin: 0 auto;
                     width: 100%;"
                )>
                    // Header
                    <header style=format!(
                        "text-align: center;
                         margin-bottom: {};",
                        SPACING_3XL
                    )>
                        <h1 style=text_heading_lg()>
                            "Test Your Image"
                        </h1>
                        <p style=format!(
                            "{}\
                             margin-top: {};",
                            text_body(),
                            SPACING_MD
                        )>
                            "Drag and drop an image or click to browse"
                        </p>
                    </header>

                    // Effect 2: UploadZone with LiquidMeter
                    <UploadZone
                        on_file_selected=Callback::new(move |(data, name): (Vec<u8>, String)| {
                            set_image_data.set(Some(data));
                            set_image_name.set(name.clone());
                            set_is_analyzing.set(true);

                            // Reset particle capsule for new analysis
                            log::info!("File selected: {}", name);
                        })
                    />

                    // Effect 3: LiquidMeter (confidence visualization)
                    <div style=format!(
                        "margin-top: {};
                         display: flex;
                         justify-content: center;
                         align-items: center;
                         min-height: 200px;",
                        SPACING_2XL
                    )>
                        <canvas
                            id="liquid-meter-canvas"
                            width="200"
                            height="200"
                            style=format!(
                                "max-width: 100%;
                                 height: auto;
                                 filter: drop-shadow(0 0 20px rgba(255, 215, 0, 0.5));"
                            )
                        ></canvas>
                    </div>

                    // Results Section
                    {move || {
                        image_data.get().map(|_data| {
                            view! {
                                <div style=format!(
                                    "{}
                                     margin-top: {};",
                                    card_glass(),
                                    SPACING_2XL
                                )>
                                    <h3 style=text_heading_md()>
                                        "Analysis Results"
                                    </h3>
                                    <p style=format!(
                                        "{}\
                                         margin-top: {};",
                                        text_caption(),
                                        SPACING_SM
                                    )>
                                        {format!("File: {}", image_name.get())}
                                    </p>

                                    // Effect 4: ParticleScanning (during analysis)
                                    {move || {
                                        if is_analyzing.get() {
                                            view! {
                                                <div style=format!(
                                                    "position: relative;
                                                     width: 100%;
                                                     height: 400px;
                                                     border-radius: 12px;
                                                     background: linear-gradient(135deg, rgba(102, 51, 153, 0.1), rgba(255, 215, 0, 0.1));
                                                     border: 1px solid rgba(255, 215, 0, 0.2);
                                                     margin-top: {};
                                                     overflow: hidden;",
                                                    SPACING_XL
                                                )>
                                                    <canvas
                                                        id="particle-scanning-canvas"
                                                        width="1920"
                                                        height="1080"
                                                        style="
                                                            position: absolute;
                                                            top: 0;
                                                            left: 0;
                                                            width: 100%;
                                                            height: 100%;
                                                            object-fit: contain;
                                                        "
                                                    ></canvas>
                                                    <div style=format!(
                                                        "position: absolute;
                                                         bottom: {};
                                                         left: 50%;
                                                         transform: translateX(-50%);
                                                         {}
                                                         padding: {} {};",
                                                        SPACING_MD,
                                                        card_glass(),
                                                        SPACING_SM,
                                                        SPACING_MD
                                                    )>
                                                        <p style=format!("{}\
                                                                         color: {};",
                                                                         text_caption(),
                                                                         COLOR_GOLD
                                                        )>
                                                            "Analyzing image particles..."
                                                        </p>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }
                                    }}

                                    // Effect 5: ForensicDashboard (after analysis complete)
                                    {move || {
                                        if !is_analyzing.get() && image_data.get().is_some() && detector_results.get().is_some() {
                                            view! {
                                                <div style=format!(
                                                    "margin-top: {};",
                                                    SPACING_2XL
                                                )>
                                                    <h4 style=text_heading_sm()>
                                                        "Detector Confidence Scores"
                                                    </h4>

                                                    // 10 Detector bars with staggered animation
                                                    <div style=format!(
                                                        "display: grid;
                                                         grid-template-columns: 1fr 1fr;
                                                         gap: {};
                                                         margin-top: {};",
                                                        SPACING_MD,
                                                        SPACING_MD
                                                    )>
                                                        {detector_confidences.get().iter().enumerate().map(|(idx, &conf)| {
                                                            let detector_names = vec![
                                                                "EXIF Integrity Seal",
                                                                "Chromatic Aberration Guard",
                                                                "Compression Artifact Sentinel",
                                                                "Noise Pattern Oracle",
                                                                "Frequency Domain Augur",
                                                                "Edge Consistency Praetor",
                                                                "Color Distribution Legate",
                                                                "Metadata Chain Curator",
                                                                "Statistical Harmony Consul",
                                                                "Neural Pattern Imperator",
                                                            ];

                                                            let name = *detector_names.get(idx).unwrap_or(&"Unknown Detector");
                                                            let color = if conf >= 0.8 {
                                                                "#10B981" // Green
                                                            } else if conf >= 0.6 {
                                                                "#FFD700" // Gold
                                                            } else if conf >= 0.4 {
                                                                "#FFA500" // Orange
                                                            } else {
                                                                "#EF4444" // Red
                                                            };

                                                            view! {
                                                                <div style=format!(
                                                                    "{}
                                                                     padding: {};",
                                                                    card_glass(),
                                                                    SPACING_MD
                                                                )>
                                                                    <div style="
                                                                        display: flex;
                                                                        justify-content: space-between;
                                                                        align-items: center;
                                                                        margin-bottom: 8px;
                                                                    ">
                                                                        <p style=format!(
                                                                            "{}
                                                                             font-size: 0.875rem;",
                                                                            text_body()
                                                                        )>
                                                                            {name}
                                                                        </p>
                                                                        <span style=format!(
                                                                            "{}
                                                                             color: {};
                                                                             font-weight: 600;",
                                                                            text_caption(),
                                                                            color
                                                                        )>
                                                                            {format!("{:.1}%", conf * 100.0)}
                                                                        </span>
                                                                    </div>
                                                                    <div style="
                                                                        width: 100%;
                                                                        height: 8px;
                                                                        background: rgba(255, 215, 0, 0.1);
                                                                        border-radius: 4px;
                                                                        overflow: hidden;
                                                                        border: 1px solid rgba(255, 215, 0, 0.2);
                                                                    ">
                                                                        <div style=format!(
                                                                            "height: 100%;
                                                                             background: linear-gradient(90deg, {} 0%, {}  100%);
                                                                             width: {}%;
                                                                             transition: width 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);",
                                                                            color,
                                                                            color,
                                                                            (conf * 100.0) as i32
                                                                        )></div>
                                                                    </div>
                                                                </div>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    </div>

                                                    // Overall confidence score
                                                    <div style=format!(
                                                        "{}
                                                         margin-top: {};
                                                         text-align: center;
                                                         padding: {};",
                                                        card_glass(),
                                                        SPACING_2XL,
                                                        SPACING_XL
                                                    )>
                                                        <p style=format!(
                                                            "{}
                                                             color: {};
                                                             margin-bottom: {};",
                                                            text_caption(),
                                                            COLOR_GOLD,
                                                            SPACING_SM
                                                        )>
                                                            "Overall Confidence"
                                                        </p>
                                                        <p style=format!(
                                                            "{}
                                                             color: {};",
                                                            text_heading_md(),
                                                            COLOR_GOLD
                                                        )>
                                                            {format!("{:.1}%", confidence.get() * 100.0)}
                                                        </p>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }
                                    }}
                                </div>
                            }
                        })
                    }}

                    // Effect 5 Alternative: NeomorphButton (Test Another Image)
                    <Show when=move || image_data.get().is_some() && !is_analyzing.get() fallback=move || {
                        view! { <div></div> }
                    }>
                        <div style=format!(
                            "margin-top: {};
                             display: flex;
                             justify-content: center;",
                            SPACING_3XL
                        )>
                            <button
                                on:click={
                                    let reset_btn_click = reset_btn_for_click.clone();
                                    move |_| {
                                        let reset_btn_clone1 = reset_btn_click.clone();
                                        reset_btn_clone1.set_hover(true);
                                        let reset_btn_clone2 = reset_btn_click.clone();
                                        set_timeout(
                                            move || {
                                                reset_btn_clone2.set_hover(false);
                                                // Reset and test again
                                                set_image_data.set(None);
                                                set_detector_results.set(None);
                                                set_confidence.set(0.0);
                                                set_detector_confidences.set(vec![0.0f32; 10]);
                                                log::info!("Reset for next image test");
                                            },
                                            Duration::from_millis(200),
                                        );
                                    }
                                }
                                style="
                                     padding: 1rem 1.5rem;
                                     border: none;
                                     border-radius: 12px;
                                     font-size: 1rem;
                                     font-weight: 600;
                                     cursor: pointer;
                                     transition: all 200ms cubic-bezier(0.34, 1.56, 0.64, 1);
                                     background: linear-gradient(135deg, #663399 0%, #7d4fb8 100%);
                                     color: white;
                                     box-shadow: 0px 8px 16px rgba(102, 51, 153, 0.3),
                                                 0px 2px 4px rgba(255, 215, 0, 0.2);
                                "
                            >
                                "Test Another Image"
                            </button>
                        </div>
                    </Show>
                </div>
            </div>
        </div>
    }
}
