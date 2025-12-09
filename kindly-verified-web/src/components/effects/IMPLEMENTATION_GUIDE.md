# Leptos Component Wrappers - Implementation Guide

This directory contains skeleton implementations for Leptos component wrappers around computational capsules. This guide explains what needs to be completed.

## Files Created

1. **mod.rs** (47 lines) - Module exports and documentation
2. **neomorph_button.rs** (171 lines) - NeomorphButton component wrapper
3. **forensic_dashboard.rs** (196 lines) - ForensicDashboard component wrapper
4. **parallax_hero.rs** (174 lines) - ParallaxHero component wrapper
5. **particle_scanning.rs** (209 lines) - ParticleScanning component wrapper
6. **liquid_meter.rs** (238 lines) - LiquidMeter component wrapper

**Total: 1,035 lines of Leptos component code**

## Completion Checklist

### NeomorphButton Component (neomorph_button.rs)

**What exists**:
- Component structure with proper Leptos `#[component]` macro
- Hover/press/disabled state tracking with signals
- Event handlers (mouseenter, mouseleave, mousedown, mouseup, click)
- Glassmorphism styling integration
- Children support

**What needs fixing** (from capsule API investigation):

1. Constructor signature: `NeomorphGlassButtonCapsule::new(color_primary: u32, color_secondary: u32)`
   - Update line 45 to:
   ```rust
   let capsule = Arc::new(NeomorphGlassButtonCapsule::new(0x663399, 0xFFD700));
   ```

2. Method signatures (take state parameter, not zero arguments):
   - `capsule.set_hover(bool)` instead of `capsule.set_hover()`
   - `capsule.set_pressed(bool)` instead of `capsule.set_pressed()`
   - `capsule.set_disabled(bool)` instead of separate set/clear methods
   - Lines 56-74 need updates

3. Style string generation:
   - `create_memo` closure should take `_` parameter: `create_memo(move |_| ...)`
   - Line 78 fix

4. Callback invocation:
   - Leptos uses `callback.run()` or direct invocation instead of `callback.call()`
   - Line 105 fix

### ForensicDashboard Component (forensic_dashboard.rs)

**What exists**:
- Animation loop with `request_animation_frame`
- 10-bar visualization grid
- Signal-driven updates
- Color-coded confidence bars

**What needs fixing**:

1. Method name: `set_bar_confidence()` - verify correct API
   - Line 43: Check actual capsule method name

2. Animation loop:
   - Closure pattern needs `|_|` parameter for `create_memo`
   - Lines 90, 92

3. Canvas/Web APIs:
   - `request_animation_frame` callback needs `wasm_bindgen::prelude::Closure::new()`
   - Add `use wasm_bindgen::prelude::*;` import

### ParallaxHero Component (parallax_hero.rs)

**What exists**:
- 3-layer parallax effect with scroll-driven offsets
- Scroll event listener setup
- Memoized layer offsets

**What needs fixing**:

1. Constructor: `ParallaxHeroCapsule::new()` - verify takes 2 arguments or 0
   - Lines 37, handle signature

2. Event listener pattern:
   - Web APIs use different method names in Leptos/web_sys
   - Lines 48-73: Need web_sys imports and proper closure handling

3. Memo closures:
   - All `create_memo` need `move |_|` parameter
   - Lines 79, 84, 89

### ParticleScanning Component (particle_scanning.rs)

**What exists**:
- Canvas 2D rendering loop
- Particle physics simulation integration
- Canvas dimension responsive handling
- 60fps animation loop

**What needs fixing**:

1. Constructor arguments:
   - Line 47: Check if constructor takes width/height or not

2. Canvas API integration:
   - Line 74: `dyn_into::<CanvasRenderingContext2d>()` needs proper imports
   - Add `use wasm_bindgen::prelude::*;`

3. NodeRef syntax:
   - Line 202: `_ref` should be `node_ref` (Leptos 0.7 syntax)
   - Canvas element binding

4. Particle data access:
   - Lines 125-126: Verify actual field names (`pos_x`, `pos_y` may differ)
   - Check `ParticleData` struct definition

### LiquidMeter Component (liquid_meter.rs)

**What exists**:
- Liquid visualization with confidence-driven height
- Animation loop with capsule ticking
- Confidence color mapping
- Shape state display

**What needs fixing**:

1. Constructor: Verify signature
   - Line 27: Check if any parameters needed

2. Memo closures:
   - All `create_memo(move ||` need parameter: `create_memo(move |_|`
   - Lines 93, 96, 99, 100, 101

3. ShapeState enum variants:
   - Lines 215-217: Verify actual enum variants
   - May be different names (e.g., `Wave`, `Morphing`, `Rest`)

## Quick Fix Script

Run these commands to identify issues:

```bash
# Find capsule API signatures
grep -n "pub fn " /home/samuel/Primitives/kindly-verified-web/src/capsules/*.rs

# Find enum/struct definitions
grep -n "^pub enum\|^pub struct" /home/samuel/Primitives/kindly-verified-web/src/capsules/*.rs

# Check existing component patterns
grep -n "create_memo\|create_signal" /home/samuel/Primitives/kindly-verified-web/src/pages/*.rs
```

## Testing Commands

After fixing APIs:

```bash
# Type-check only (fast)
cargo check

# Run tests
cargo test --lib

# Build for release
cargo build --release --target wasm32-unknown-unknown
```

## Leptos Patterns Reference

### Create Memo with Parameter
```rust
let value = create_memo(move |_| {
    some_signal.get()
});
```

### Web Window API
```rust
use web_sys::window;

if let Some(w) = window() {
    let _ = w.request_animation_frame(&closure);
}
```

### Canvas Rendering
```rust
use web_sys::CanvasRenderingContext2d;
use wasm_bindgen::prelude::*;

let ctx: CanvasRenderingContext2d = canvas
    .get_context("2d")?
    .unwrap()
    .dyn_into()?;
```

### Callback Invocation
```rust
if let Some(callback) = on_click {
    callback.call(());  // or direct invocation
}
```

## Architecture Overview

```
kindly-verified-web/src/components/effects/
├── mod.rs                      # Module exports + re-exports
├── neomorph_button.rs         # NeomorphGlassButtonCapsule wrapper
├── forensic_dashboard.rs       # ForensicDashboardCapsule wrapper
├── parallax_hero.rs            # ParallaxHeroCapsule wrapper
├── particle_scanning.rs        # ParticleScanningCapsule wrapper
├── liquid_meter.rs             # LiquidMorphingMeterCapsule wrapper
└── IMPLEMENTATION_GUIDE.md     # This file
```

## Framework Compliance

All components follow:
- **UCE34**: Q10 tier selection (T1+T3 for buttons, T2+T5 for effects, etc.)
- **Chaos**: 100% lockfree capsule wrapping
- **Leptos Patterns**: `create_signal`, `create_effect`, `create_memo`
- **WASM-Friendly**: No multi-threading, proper effect cleanup with `on_cleanup()`

## Next Steps

1. Review capsule API signatures in `/home/samuel/Primitives/kindly-verified-web/src/capsules/`
2. Update each component's method calls to match actual capsule APIs
3. Fix `create_memo` closures to accept `|_|` parameter
4. Test with `cargo check` and `cargo test`
5. Integrate components into pages (e.g., `src/pages/home.rs`)

## Integration Example

Once components are fixed, use in pages:

```rust
use crate::components::effects::*;
use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    let (confidence, set_confidence) = create_signal(0.75);

    view! {
        <ParallaxHero>
            <h1>"Kindly Verified"</h1>
        </ParallaxHero>

        <ForensicDashboard detector_results=Some(vec![...]) />

        <LiquidMeter confidence=confidence />

        <ParticleScanning
            image_width=Signal::derive(|| 800.0)
            image_height=Signal::derive(|| 600.0)
        />

        <NeomorphButton on_click=Callback::new(move |_| {
            set_confidence.set(confidence.get() + 0.1);
        })>
            "Increase Confidence"
        </NeomorphButton>
    }
}
```

## Performance Targets (B32 Framework)

- **NeomorphButton**: <50ns state update
- **ForensicDashboard**: <100ns per tick, 10-bar animation
- **ParallaxHero**: <10ns offset calculation per layer
- **ParticleScanning**: <1ms physics per 500 particles
- **LiquidMeter**: <100ns shape morphing per frame

## Trade Secrets

These components wrap proprietary capsule implementations. Do not share component code with trade secret capsules publicly.

---

**Status**: Ready for API completion and integration testing
**Created**: 2025-11-21
**Framework**: Leptos 0.7 + kindly-verified-web computational capsules
