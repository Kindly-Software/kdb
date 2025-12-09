# kindly-web Quick Reference Guide

## Color Scheme at a Glance

### Primary Colors
- **Byzantine Purple**: `#663399` (primary brand color)
- **Metallic Gold**: `#FFD700` (accent, CTA buttons, text highlights)

### Gradients
```
Hero Background:     radial-gradient(ellipse at top, #4B0082 → #0A0014)
Gold Text Gradient:  linear-gradient(135deg, #FFD700 → #FFED4E)
Purple Gradient:     linear-gradient(135deg, #8A2BE2 → #663399)
```

### Glass Effects
```
Navbar:  blur(16px) saturate(150%), rgba(102,51,153,0.4)
Cards:   blur(16px) saturate(180%), rgba(26,0,40,0.8)
Hover:   blur(24px) saturate(180%), lifted +4px
```

---

## Component Quick Start

### Button Types
```rust
// Primary (Gold)
<Button variant=ButtonVariant::Primary>
  "Click me"
</Button>

// Secondary (Purple glass)
<Button variant=ButtonVariant::Secondary>
  "Secondary"
</Button>

// Outlined (Gold border)
<Button variant=ButtonVariant::Outlined>
  "Outlined"
</Button>

// Sizes: Small, Medium, Large
<Button size=ButtonSize::Large>
  "Large Button"
</Button>
```

### Styling Inline
```rust
view! {
    <div style=move || format!(
        "background: {}; color: #FFFFFF; padding: 2rem;",
        GRADIENT_HERO
    )>
        "Content"
    </div>
}
```

### Responsive Layout
```rust
let breakpoint = use_breakpoint();
view! {
    <div>
        {move || match breakpoint.get() {
            Breakpoint::Xs | Breakpoint::Sm => view! { <MobileVersion /> },
            _ => view! { <DesktopVersion /> },
        }}
    </div>
}
```

### Sticky Navbar with Scroll Effect
```rust
let scroll_y = use_scroll_y();
view! {
    <nav style=move || glassmorphism::navbar_blur_responsive(scroll_y.get())>
        "Navigation"
    </nav>
}
```

---

## Design Tokens Quick Lookup

### Typography
```
Heading:    3.75rem / 800 weight / -0.02em letter-spacing
Subheading: 1.5rem / 400 weight
Body:       1rem / 400 weight / 1.5 line-height
Small:      0.875rem / 400 weight
```

### Spacing (8px grid)
```
4px   (0.25rem)  - SPACING_1
8px   (0.5rem)   - SPACING_2
12px  (0.75rem)  - SPACING_3
16px  (1rem)     - SPACING_4
24px  (1.5rem)   - SPACING_6
32px  (2rem)     - SPACING_8
```

### Border Radius
```
4px   - RADIUS_SM   (buttons, small elements)
8px   - RADIUS_MD   (cards)
16px  - RADIUS_LG   (large cards, containers)
24px  - RADIUS_XL   (hero sections)
∞     - RADIUS_FULL (circles, badges)
```

### Blur Levels
```
8px   - blur-sm  (light, hover state)
16px  - blur-md  (default navbar)
24px  - blur-lg  (strong, card hover)
32px  - blur-xl  (heavy, scrolled navbar)
```

---

## File Organization

### Adding a New Page
1. Create file in `src/pages/mypage.rs`
2. Add route in `lib.rs`:
   ```rust
   <Route path=path!("/mypage") view=MyPage />
   ```
3. Export in `pages/mod.rs`

### Adding a New Component
1. **Atom**: `src/components/common/mycomponent.rs`
2. **Molecule**: `src/components/molecular/mycomponent.rs`
3. **Organism**: `src/components/sections/mycomponent.rs`
4. Export in respective `mod.rs`

### Styling New Components
1. Use `theme.rs` constants for colors
2. Use `glassmorphism.rs` functions for glass effects
3. Use `layout.rs` for responsive patterns
4. Inline styles as format strings

---

## Build & Deploy Checklist

### Development
```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve
# http://127.0.0.1:8080
```

### Production Build
```bash
trunk build --release
# Output: dist/ directory
```

### Deploy to Fly.io
```bash
fly deploy
# Deploys dist/ to kindly.software
```

### Bundle Size Check
```bash
ls -lh dist/kindly_web_bg.wasm
# Should be ~360KB raw, ~180KB gzipped
```

---

## Common Patterns

### Grid Layout
```rust
view! {
    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 2rem;">
        {/* Cards */}
    </div>
}
```

### Flex Container
```rust
view! {
    <div style="display: flex; gap: 1rem; justify-content: center; flex-wrap: wrap;">
        {/* Items */}
    </div>
}
```

### Gradient Text
```rust
view! {
    <h1 style=glassmorphism::gold_gradient_text()>
        "Lightning-Fast"
    </h1>
}
```

### Card with Glass Effect
```rust
view! {
    <div style=glassmorphism::card_style()>
        "Card content"
    </div>
}
```

### Feature List
```rust
view! {
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
        <div>"✓ Feature 1"</div>
        <div>"✓ Feature 2"</div>
    </div>
}
```

---

## Testing Commands

```bash
# Run all tests
cargo test --lib

# Run specific test
cargo test button_tests

# Watch mode (requires cargo-watch)
cargo watch -x test

# With logging
RUST_LOG=debug cargo test -- --nocapture
```

---

## Performance Tips

1. **Minimize rerenders** - Use `move ||` carefully
2. **Use signals** - `signal()` for reactive state
3. **Cache expensive computations** - Don't compute styles every render
4. **Lazy load heavy components** - Use conditional rendering
5. **Optimize bundle** - Remove unused dependencies

---

## Stripe Integration

### Creating Checkout Session
```rust
let result = stripe_api::create_checkout_session(
    "price_1SS3YVJfpUw0xSwgHxzaAbUw"  // Early adopter price ID
).await;

match result {
    Ok(session_id) => { /* Redirect to Stripe */ },
    Err(e) => { /* Show error */ },
}
```

### Getting Remaining Count
```rust
let count = stripe_api::get_early_adopter_remaining().await?;
let remaining = format!("{} of {} remaining", count.remaining, count.limit);
```

---

## Troubleshooting

### Build Errors
- `wasm32-unknown-unknown not found`: Run `rustup target add wasm32-unknown-unknown`
- `trunk not found`: Run `cargo install trunk`
- WASM compilation error: Check for `unsafe` code or use stable features only

### Runtime Errors
- Styles not applied: Check CSS syntax in format strings
- Component not rendering: Check route path matches exactly
- API calls failing: Check CORS headers, verify base URL in `stripe_api.rs`

### Performance Issues
- Slow renders: Profile with `trunk build --release` and check WASM size
- Slow page load: Check LCP with Chrome DevTools
- High memory: Look for signal leaks or unclosed effects

---

## Key Files Reference

| File | Purpose | Lines |
|------|---------|-------|
| `lib.rs` | App root, routing, gold borders | 87 |
| `components/common/button.rs` | Button component | 152 |
| `utils/theme.rs` | Design tokens | 171 |
| `utils/glassmorphism.rs` | Glass effects | 272 |
| `utils/layout.rs` | Responsive breakpoints | 147 |
| `components/sections/hero.rs` | Hero banner | 243 |
| `pages/pricing_stripe.rs` | Stripe checkout page | 213 |
| `index.html` | HTML template + CSS | 560 |

---

## Resources

- **Leptos Docs**: https://leptos.dev
- **WASM**: https://www.rust-lang.org/what/wasm/
- **Trunk**: https://trunkrs.io
- **Fly.io**: https://fly.io
- **Stripe API**: https://stripe.com/docs/api

---

## Summary

**kindly-web is a production-ready Rust WASM SPA with:**
- Pure Leptos 0.7 (CSR mode)
- Byzantine purple + metallic gold design
- Glassmorphism (macOS Big Sur style)
- Stripe payment integration
- 1,787 lines of Rust
- ~180KB gzipped bundle
- No CSS files (inline styles only)
- Atomic design pattern (atoms → molecules → organisms)
- 100% type-safe components
- Trade secret protected (plain patterns, no capsules)

