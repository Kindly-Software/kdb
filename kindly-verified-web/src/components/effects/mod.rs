//! Leptos component wrappers for computational capsules
//!
//! This module provides Leptos component wrappers for all 5 computational capsules,
//! enabling seamless WASM integration with reactive signals and effects.
//!
//! ## Components
//!
//! 1. **NeomorphButton** - Soft 3D button with glassmorphism (T1+T3)
//! 2. **ForensicDashboard** - 10-bar animated detector dashboard (T2+T5+T1)
//! 3. **ParallaxHero** - 3-layer depth scrolling effect (T1+T3+T5)
//! 4. **ParticleScanning** - 500 particles physics simulation (T2+T4+T5)
//! 5. **LiquidMeter** - Metaball confidence meter (T2+T3+T5)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::components::effects::NeomorphButton;
//! use leptos::prelude::*;
//!
//! #[component]
//! pub fn MyPage() -> impl IntoView {
//!     view! {
//!         <NeomorphButton on_click=Callback::new(|_| log::info!("Clicked!"))>
//!             "Click me"
//!         </NeomorphButton>
//!     }
//! }
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: All components use Q10 tier selection
//! - **Chaos**: Wrap 100% lockfree capsules
//! - **Leptos Patterns**: Use `Effect::new`, `signal`, `Memo::new`
//! - **WASM-Friendly**: No multi-threading, proper effect cleanup

pub mod neomorph_button;
pub mod forensic_dashboard;
pub mod parallax_hero;
pub mod particle_scanning;
pub mod liquid_meter;

