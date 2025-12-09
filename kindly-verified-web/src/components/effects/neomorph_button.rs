//! NeomorphButton - Soft 3D button component (T1+T3)
//!
//! Leptos wrapper for NeomorphGlassButtonCapsule with hover and press state tracking.

use leptos::prelude::*;
use std::sync::Arc;

use crate::capsules::NeomorphGlassButtonCapsule;
use crate::utils::styles::*;

/// NeomorphButton - Interactive soft 3D button with glassmorphism
///
/// # Props
///
/// - `on_click` - Callback fired when button clicked
/// - `disabled` - Whether button is disabled
/// - `children` - Button label/content
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::effects::NeomorphButton;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     let handle_click = move || log::info!("Button clicked!");
///
///     view! {
///         <NeomorphButton
///             on_click=Callback::new(move |_| handle_click())
///         >
///             "Click me"
///         </NeomorphButton>
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn NeomorphButton(
    #[prop(optional)] on_click: Option<Callback<()>>,
    #[prop(optional)] disabled: Option<bool>,
    children: Children,
) -> impl IntoView {
    // Create capsule instance (Arc for shared ownership if needed in future)
    // Use Byzantine purple (#663399) and gold (#FFD700) colors
    let capsule = Arc::new(NeomorphGlassButtonCapsule::new(0x663399, 0xFFD700));

    // Create reactive signals for state
    let is_disabled = Signal::derive(move || disabled.unwrap_or(false));
    let (hover, set_hover) = signal(false);
    let (pressed, set_pressed) = signal(false);

    // Sync button state to capsule
    let capsule_disabled = capsule.clone();
    Effect::new(move |_| {
        capsule_disabled.set_disabled(is_disabled.get());
    });

    let capsule_hover = capsule.clone();
    Effect::new(move |_| {
        capsule_hover.set_hover(hover.get());
    });

    let capsule_pressed = capsule.clone();
    Effect::new(move |_| {
        capsule_pressed.set_pressed(pressed.get());
    });

    // Get CSS style string from capsule
    let capsule_style_memo = capsule.clone();
    let capsule_style = Memo::new(move |_| capsule_style_memo.get_style_string());

    // Handle mouse events
    let handle_mouseenter = move |_| {
        if !is_disabled.get() {
            set_hover.set(true);
        }
    };

    let handle_mouseleave = move |_| {
        set_hover.set(false);
        set_pressed.set(false);
    };

    let handle_mousedown = move |_| {
        if !is_disabled.get() {
            set_pressed.set(true);
        }
    };

    let handle_mouseup = move |_| {
        set_pressed.set(false);
    };

    let handle_click = move |_| {
        if !is_disabled.get() {
            if let Some(callback) = on_click {
                callback.run(());
            }
        }
    };

    let button_style = move || {
        let base_style = format!(
            "{}
             padding: {} {};
             border-radius: 12px;
             font-weight: 600;
             font-size: 1rem;
             cursor: {};
             border: none;
             transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
             color: white;
             outline: none;
             user-select: none;
             {}",
            glassmorphism(GlassBlur::Medium, 0.2),
            SPACING_MD,
            SPACING_XL,
            if is_disabled.get() { "not-allowed" } else { "pointer" },
            capsule_style.get()
        );

        if hover.get() && !is_disabled.get() {
            format!("{} {}", base_style, glow_gold())
        } else {
            base_style
        }
    };

    let opacity = move || {
        if is_disabled.get() {
            "0.5"
        } else {
            "1.0"
        }
    };

    view! {
        <button
            style=button_style
            style:opacity=opacity
            on:mouseenter=handle_mouseenter
            on:mouseleave=handle_mouseleave
            on:mousedown=handle_mousedown
            on:mouseup=handle_mouseup
            on:click=handle_click
            disabled=is_disabled
        >
            {children()}
        </button>
    }
}
