//! ParticleScanning - Particle physics canvas component (T2+T4+T5)
//!
//! Leptos wrapper for ParticleScanningCapsule with HTML5 Canvas rendering.

#![allow(deprecated)] // web_sys::CanvasRenderingContext2d::set_fill_style is deprecated but necessary for Canvas API

use leptos::prelude::*;
use std::sync::Arc;
use wasm_bindgen::{JsValue, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, window};

use crate::capsules::ParticleScanningCapsule;
use crate::utils::styles::*;

/// ParticleScanning - 500-particle physics simulation on Canvas
///
/// # Props
///
/// - `image_width` - Canvas width in pixels (Signal)
/// - `image_height` - Canvas height in pixels (Signal)
/// - `detector_results` - Optional detector results for coloring particles
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::effects::ParticleScanning;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     let (width, _) = signal(800.0);
///     let (height, _) = signal(600.0);
///
///     view! {
///         <ParticleScanning
///             image_width=width
///             image_height=height
///         />
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn ParticleScanning(
    #[prop(into)] image_width: Signal<f32>,
    #[prop(into)] image_height: Signal<f32>,
    #[prop(optional)] _detector_results: Option<Vec<f32>>,
) -> impl IntoView {
    // Create capsule instance
    let capsule = Arc::new(ParticleScanningCapsule::new(
        image_width.get(),
        image_height.get(),
    ));

    // Create canvas reference
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    // Animation state
    let (animation_active, _set_animation_active) = signal(true);
    let (_frame_count, set_frame_count) = signal(0u32);

    // Initialize canvas and start animation loop
    Effect::new(move |_| {
        let Some(canvas_elem) = canvas_ref.get() else {
            return;
        };

        let canvas: HtmlCanvasElement = canvas_elem.into();
        let width = image_width.get();
        let height = image_height.get();

        // Set canvas dimensions
        canvas.set_width(width as u32);
        canvas.set_height(height as u32);

        let ctx: CanvasRenderingContext2d = match canvas.get_context("2d") {
            Ok(Some(context)) => match context.dyn_into::<CanvasRenderingContext2d>() {
                Ok(ctx) => ctx,
                Err(_) => {
                    log::error!("Failed to cast context to CanvasRenderingContext2d");
                    return;
                }
            },
            _ => {
                log::error!("Failed to get canvas 2d context");
                return;
            }
        };

        if !animation_active.get() {
            return;
        }

        let window = match window() {
            Some(w) => w,
            None => {
                log::error!("Window not available");
                return;
            }
        };

        let handle_id = std::rc::Rc::new(std::cell::RefCell::new(None));

        let tick: wasm_bindgen::prelude::Closure<dyn FnMut()> = {
            let ctx_clone = ctx.clone();
            let _canvas_clone = canvas.clone();
            let capsule_clone = capsule.clone();
            let window_clone = window.clone();
            let handle_id_clone = handle_id.clone();

            wasm_bindgen::prelude::Closure::new(move || {
                set_frame_count.update(|f| *f = f.wrapping_add(1));

                // Calculate delta time (assuming 60fps)
                let delta_ms = 16.67; // ~60fps
                capsule_clone.tick(delta_ms as u32);

                // Clear canvas
                ctx_clone
                    .fill_rect(0.0, 0.0, width as f64, height as f64);

                let _ = ctx_clone.set_fill_style(&JsValue::from_str("rgba(26, 0, 51, 0.8)"));
                ctx_clone.fill_rect(0.0, 0.0, width as f64, height as f64);

                // Get active particles
                let particles = capsule_clone.get_active_particles();

                // Render particles
                for particle in particles.iter() {
                    let x = particle.x as f64;
                    let y = particle.y as f64;

                    // Set particle color
                    let color_hex = format!(
                        "rgba({}, {}, {}, 0.8)",
                        (particle.color >> 16) & 0xFF,
                        (particle.color >> 8) & 0xFF,
                        particle.color & 0xFF,
                    );
                    let _ = ctx_clone.set_fill_style(&JsValue::from_str(&color_hex));

                    // Draw particle as small circle
                    let _ = ctx_clone.begin_path();
                    let _ = ctx_clone.arc(x, y, particle.radius as f64, 0.0, std::f64::consts::TAU);
                    ctx_clone.fill();
                }

                // Request next frame
                if let Ok(id) =
                    window_clone.request_animation_frame(&js_sys::Function::new_no_args(""))
                {
                    *handle_id_clone.borrow_mut() = Some(id);
                }
            })
        };

        if let Ok(id) = window.request_animation_frame(tick.as_ref().unchecked_ref()) {
            *handle_id.borrow_mut() = Some(id);
        }

        tick.forget();
        // Note: animation frame will be cleaned up when component unmounts
    });

    // Update canvas dimensions if they change
    Effect::new(move |_| {
        let Some(canvas_elem) = canvas_ref.get() else {
            return;
        };

        let canvas: HtmlCanvasElement = canvas_elem.into();
        let width = image_width.get();
        let height = image_height.get();

        canvas.set_width(width as u32);
        canvas.set_height(height as u32);
        // Note: ParticleScanningCapsule doesn't have a resize() method
        // Dimensions are baked into the particle physics
    });

    let container_style = format!(
        "{}
         border-radius: 16px;
         overflow: hidden;
         position: relative;",
        glassmorphism(GlassBlur::Medium, 0.15)
    );

    let canvas_style = format!(
        "display: block;
         width: 100%;
         height: auto;
         background: linear-gradient(135deg, {} 0%, {} 100%);",
        COLOR_BG_DARK, COLOR_BG_MID
    );

    view! {
        <div style=container_style>
            <canvas
                node_ref=canvas_ref
                style=canvas_style
                width=image_width.get() as i32
                height=image_height.get() as i32
            />
        </div>
    }
}
