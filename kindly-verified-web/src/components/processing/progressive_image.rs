//! ProgressiveImage - Progressive image loading component (T5+T4)
//!
//! Leptos wrapper for ProgressiveImageLoaderCapsule with blur-to-sharp
//! transitions for perceived performance.

use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;

use crate::capsules::{ProgressiveImageLoaderCapsule, ImageFormat, DecodeStage};
use crate::utils::styles::*;

/// ProgressiveImage - Progressive JPEG/PNG loading with blur-to-sharp
///
/// # Props
///
/// - `src` - Image source URL
/// - `alt` - Alternative text
/// - `format` - Image format (JPEG, PNG, WebP)
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::processing::ProgressiveImage;
/// use crate::capsules::ImageFormat;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     view! {
///         <ProgressiveImage
///             src="https://example.com/image.jpg"
///             alt="Example image"
///             format=ImageFormat::Jpeg
///         />
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn ProgressiveImage(
    src: String,
    #[prop(optional)] alt: Option<String>,
    #[prop(optional)] format: Option<ImageFormat>,
) -> impl IntoView {
    // Create capsule instance
    let capsule = Arc::new(
        ProgressiveImageLoaderCapsule::new(format.unwrap_or(ImageFormat::Jpeg))
    );

    // Create reactive signals for loading state
    let (current_stage, set_current_stage) = signal(DecodeStage::LowRes);
    let (progress_pct, set_progress_pct) = signal(0u32);
    let (image_data_url, set_image_data_url) = signal(None);
    let (blur_level, set_blur_level) = signal(20);

    let alt_text = alt.unwrap_or_else(|| "Loading image...".to_string());
    let src_clone_for_effect = src.clone();

    // Start loading image on mount
    Effect::new(move |_| {
        let src_effect = src_clone_for_effect.clone();
        let capsule_clone = capsule.clone();

        spawn_local(async move {
            // Fetch image data
            match fetch_image(&src_effect).await {
                Ok(data) => {
                    // Progressive decode
                    for chunk in data.chunks(64) {
                        match capsule_clone.feed_chunk(chunk) {
                            Ok(progress) => {
                                set_progress_pct.set(progress.overall_progress as u32);
                                set_current_stage.set(progress.stage);

                                // Reduce blur as quality improves
                                let blur = match progress.stage {
                                    DecodeStage::LowRes => 20,
                                    DecodeStage::MidRes => 15,
                                    DecodeStage::HighRes => 10,
                                    DecodeStage::Final => 5,
                                    DecodeStage::Complete => 0,
                                };
                                set_blur_level.set(blur);

                                // Continue decoding next chunk
                            }
                            Err(e) => {
                                log::error!("Decode error: {:?}", e);
                                break;
                            }
                        }
                    }

                    // Generate final image data URL
                    if let Some(_final_image) = capsule_clone.get_final_image() {
                        // Convert pixel data to data URL for display
                        // For now, store indication that loading is complete
                        set_image_data_url.set(Some("image-loaded".to_string()));
                        set_blur_level.set(0);
                    }
                }
                Err(e) => {
                    log::error!("Failed to fetch image: {}", e);
                }
            }
        });
    });

    // Container styles
    let container_style = "
        position: relative;
        overflow: hidden;
        border-radius: 8px;
        background: rgba(102, 51, 153, 0.1);
    ";

    let image_style = move || {
        format!(
            "width: 100%;
             height: 100%;
             object-fit: cover;
             filter: blur({}px);
             transition: filter 0.6s ease;
             opacity: {};",
            blur_level.get(),
            if image_data_url.get().is_some() { "1.0" } else { "0.5" }
        )
    };

    let loading_overlay_style = move || {
        let show = image_data_url.get().is_none();
        format!(
            "{}
             position: absolute;
             top: 0;
             left: 0;
             right: 0;
             bottom: 0;
             display: flex;
             flex-direction: column;
             align-items: center;
             justify-content: center;
             opacity: {};
             transition: opacity 0.3s ease;
             pointer-events: {};",
            glassmorphism(GlassBlur::Medium, 0.3),
            if show { "1.0" } else { "0.0" },
            if show { "auto" } else { "none" }
        )
    };

    let progress_bar_style = "
        width: 80%;
        height: 4px;
        background: rgba(255, 215, 0, 0.2);
        border-radius: 2px;
        overflow: hidden;
        margin-top: 1rem;
    ";

    let progress_fill_style = move || {
        format!(
            "height: 100%;
             width: {}%;
             background: linear-gradient(90deg, #FFD700, #663399);
             transition: width 0.3s ease;
             border-radius: 2px;",
            progress_pct.get()
        )
    };

    let stage_label_style = format!(
        "{}
         margin-bottom: 0.5rem;
         text-align: center;",
        text_caption()
    );

    view! {
        <div style=container_style>
            <img
                src=move || image_data_url.get().unwrap_or(src.clone())
                alt=alt_text
                style=image_style
                on:load=move |_| {
                    set_blur_level.set(0);
                }
            />

            <div style=loading_overlay_style>
                <div style=stage_label_style>
                    {move || {
                        format!(
                            "Stage {}: {:.0}%",
                            match current_stage.get() {
                                DecodeStage::LowRes => 1,
                                DecodeStage::MidRes => 2,
                                DecodeStage::HighRes => 3,
                                DecodeStage::Final => 4,
                                DecodeStage::Complete => 5,
                            },
                            progress_pct.get() as f32
                        )
                    }}
                </div>
                <div style=progress_bar_style>
                    <div style=progress_fill_style></div>
                </div>
            </div>
        </div>
    }
}

/// Fetch image data from URL (simplified, returns placeholder for now)
async fn fetch_image(url: &str) -> Result<Vec<u8>, String> {
    // For now, return a placeholder. In production, this would use proper fetch.
    // The actual fetch would require wasm_bindgen_futures which may not be in dependencies.
    log::info!("Fetching image from: {}", url);

    // Return a small placeholder buffer - in real implementation would fetch actual image
    Ok(vec![0xFF, 0xD8, 0xFF, 0xE0]) // JPEG header placeholder
}
