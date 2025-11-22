# kindly-web - Premium WASM Marketing Site - Build Guide

**Version**: 1.0.0
**Status**: Production Deployed (Fly.io)
**Framework**: Leptos 0.7 (CSR-only WASM)
**Endpoint**: https://kindly-software-website.fly.dev

## Quick Start

```bash
# Install trunk (WASM bundler)
cargo install trunk

# Build and serve locally
trunk serve --open

# Or build release
trunk build --release
```

## Deployment (Production)

### Fly.io Deployment (Current)
```bash
# Build release WASM
trunk build --release

# Deploy to Fly.io
flyctl deploy

# View logs
flyctl logs

# Check status
flyctl status
```

**Production URL**: https://kindly-software-website.fly.dev

## Build Configurations

### Development Build (Hot Reload)
```bash
# Install trunk
cargo install trunk

# Serve with hot reload (default port 8080)
trunk serve

# Or specify port
trunk serve --port 3000

# Open browser automatically
trunk serve --open
```

### Release Build (Production)
```bash
# Build optimized WASM
trunk build --release

# Output directory: dist/
# - index.html (~17KB)
# - kindly-web-*.wasm (~665KB)
# - kindly-web-*.js (~45KB)

# Total: ~727KB (gzipped ~200KB)
```

### Optimized Build (Maximum Compression)
```bash
# Build with wasm-opt
trunk build --release

# Further optimize with wasm-opt manually
wasm-opt -Oz -o dist/optimized.wasm dist/kindly-web-*.wasm

# Total after wasm-opt: ~550KB (gzipped ~170KB)
```

## Trunk Configuration

### Trunk.toml
```toml
[build]
target = "index.html"
release = true
dist = "dist"
public_url = "/"

[watch]
ignore = ["dist"]

[serve]
port = 8080
address = "0.0.0.0"
open = false
```

## Project Structure

```
kindly-web/
├── index.html          # Entry point
├── Trunk.toml          # Trunk configuration
├── Cargo.toml          # Rust dependencies
├── src/
│   ├── main.rs         # App entry point
│   ├── app.rs          # Root component
│   ├── pages/          # Page components
│   │   ├── home.rs     # Hero page (190× claim)
│   │   ├── pricing.rs  # Stripe checkout
│   │   ├── success.rs  # Payment success
│   │   └── cancel.rs   # Payment cancelled
│   └── components/     # Reusable components
│       ├── header.rs   # Navigation
│       └── footer.rs   # Footer
└── style/
    └── main.css        # Tailwind CSS (optional)
```

## Development Workflow

### Local Development
```bash
# Start dev server with hot reload
trunk serve

# Navigate to http://localhost:8080

# Make changes to src/*.rs - auto-reloads browser
```

### Testing Stripe Integration
```bash
# Set environment variables (for API calls)
export STRIPE_API_URL=https://kindly-dedup-webhook.fly.dev

# Run dev server
trunk serve

# Test checkout flow:
# 1. Click "Buy Now" button
# 2. Redirected to Stripe checkout
# 3. Use test card: 4242 4242 4242 4242
# 4. Redirected back to /pricing/success
```

## WASM-Specific Optimizations

### Cargo.toml
```toml
[profile.release]
opt-level = "z"  # Optimize for size
lto = true       # Link-time optimization
codegen-units = 1  # Single codegen unit
panic = "abort"  # Reduce binary size
strip = true     # Strip debug symbols

[profile.release.package.web-sys]
opt-level = "z"

[profile.release.package.wasm-bindgen]
opt-level = "z"
```

### Further Optimization
```bash
# Install wasm-pack
cargo install wasm-pack

# Install wasm-opt
npm install -g wasm-opt

# Build with wasm-pack
wasm-pack build --target web --release

# Optimize with wasm-opt
wasm-opt -Oz pkg/kindly_web_bg.wasm -o pkg/optimized.wasm
```

## Performance Targets (Google Core Web Vitals)

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| **LCP** (Largest Contentful Paint) | <2.5s | <750ms | ✅ Good |
| **FID** (First Input Delay) | <100ms | <100ms | ✅ Good |
| **CLS** (Cumulative Layout Shift) | <0.1 | 0 | ✅ Good |

**Bundle Size**:
- HTML: 17KB
- WASM: 665KB (gzipped 200KB)
- JS: 45KB (gzipped 15KB)
- Total: 727KB (gzipped 232KB)

## Styling

### Tailwind CSS (If Using)
```bash
# Install Tailwind
npm install -D tailwindcss

# Initialize Tailwind
npx tailwindcss init

# Build CSS
npx tailwindcss -i ./style/input.css -o ./style/output.css --watch

# Link in index.html
# <link rel="stylesheet" href="/style/output.css">
```

### Pure CSS (Current)
```html
<!-- index.html -->
<link rel="stylesheet" href="/style/main.css">
```

## Deployment

### Static Hosting (Recommended)

#### Fly.io
```bash
# Build release
trunk build --release

# Deploy
flyctl deploy

# Uses nginx to serve static files
```

#### Netlify
```bash
# Build command
trunk build --release

# Publish directory
dist

# Deploy
netlify deploy --prod --dir=dist
```

#### Vercel
```bash
# Build command
trunk build --release

# Output directory
dist

# Deploy
vercel --prod
```

#### GitHub Pages
```bash
# Build release
trunk build --release

# Copy to gh-pages branch
git checkout gh-pages
cp -r dist/* .
git add .
git commit -m "Deploy"
git push origin gh-pages
```

### Docker Deployment

```dockerfile
# Multi-stage build
FROM rust:1.76-slim as builder

# Install trunk
RUN cargo install trunk wasm-bindgen-cli

WORKDIR /build
COPY . .

# Build WASM
RUN trunk build --release

# Nginx runtime
FROM nginx:alpine
COPY --from=builder /build/dist /usr/share/nginx/html
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
```

```bash
# Build Docker image
docker build -t kindly-web:1.0.0 .

# Run container
docker run -it --rm -p 8080:80 kindly-web:1.0.0

# Navigate to http://localhost:8080
```

## Environment Configuration

### API Endpoints
```rust
// src/config.rs
pub const STRIPE_API_URL: &str = if cfg!(debug_assertions) {
    "http://localhost:3000"  // Local development
} else {
    "https://kindly-dedup-webhook.fly.dev"  // Production
};
```

### Feature Flags
```rust
#[cfg(feature = "production")]
const ANALYTICS_ENABLED: bool = true;

#[cfg(not(feature = "production"))]
const ANALYTICS_ENABLED: bool = false;
```

## Common Issues

### Issue: WASM binary too large
```
warning: WASM binary is 2.5MB
```
**Fix**: Enable size optimizations in Cargo.toml:
```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
```

### Issue: Hot reload not working
```
error: trunk serve not detecting changes
```
**Fix**: Ensure Trunk.toml has correct watch settings:
```toml
[watch]
ignore = ["dist", "target"]
```

### Issue: CORS error when calling Stripe API
```
error: CORS policy blocked
```
**Fix**: Ensure Stripe webhook server has CORS headers:
```rust
// In kindly_dedup_stripe
.layer(CorsLayer::permissive())
```

### Issue: Leptos not found
```
error: can't find crate for `leptos`
```
**Fix**: Add leptos to Cargo.toml:
```toml
[dependencies]
leptos = { version = "0.7", features = ["csr", "nightly"] }
```

## Testing

### WASM Testing
```bash
# Install wasm-pack
cargo install wasm-pack

# Run tests in browser
wasm-pack test --headless --firefox

# Or Chrome
wasm-pack test --headless --chrome
```

### Manual Testing Checklist
- [ ] Home page loads (<750ms LCP)
- [ ] Navigation works (client-side routing)
- [ ] Pricing page displays correctly
- [ ] Early adopter counter updates (60s polling)
- [ ] Stripe checkout flow works
- [ ] Success page shows license key instructions
- [ ] Cancel page returns to pricing
- [ ] Mobile responsive (320px-1920px)
- [ ] Browser compatibility (Chrome, Firefox, Safari, Edge)

## Continuous Integration

```yaml
# .github/workflows/ci.yml
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: jetli/trunk-action@v0.4.0
      - run: trunk build --release
      - uses: actions/upload-artifact@v3
        with:
          name: dist
          path: dist/

  deploy:
    runs-on: ubuntu-latest
    needs: build
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: actions/download-artifact@v3
        with:
          name: dist
      - uses: superfly/flyctl-actions/setup-flyctl@master
      - run: flyctl deploy --remote-only
        env:
          FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
```

## Performance Monitoring

### Lighthouse CI
```bash
# Install Lighthouse
npm install -g @lhci/cli

# Run Lighthouse audit
lhci autorun --config=lighthouserc.json

# lighthouserc.json
{
  "ci": {
    "collect": {
      "url": ["https://kindly-software-website.fly.dev/"],
      "numberOfRuns": 3
    },
    "assert": {
      "assertions": {
        "categories:performance": ["error", {"minScore": 0.9}],
        "categories:accessibility": ["error", {"minScore": 0.9}]
      }
    }
  }
}
```

## References

- **Leptos Docs**: https://leptos-rs.github.io/leptos/
- **Trunk Docs**: https://trunkrs.dev/
- **WASM Optimization**: https://rustwasm.github.io/book/reference/code-size.html
- **Stripe Integration**: `/home/samuel/Primitives/kindly_dedup_stripe/CLAUDE.md`

## Quick Reference

| Use Case | Command |
|----------|---------|
| **Local Dev** | `trunk serve --open` |
| **Release Build** | `trunk build --release` |
| **Deploy to Fly.io** | `flyctl deploy` |
| **Test WASM** | `wasm-pack test --headless --firefox` |
| **Optimize WASM** | `wasm-opt -Oz -o dist/opt.wasm dist/kindly-web-*.wasm` |
| **View Bundle Size** | `ls -lh dist/` |
| **Lighthouse Audit** | `lhci autorun` |

## Performance Checklist

- [ ] Bundle size <750KB (gzipped <250KB)
- [ ] LCP <750ms (Google "Good" tier)
- [ ] FID <100ms
- [ ] CLS = 0
- [ ] Mobile responsive (320px-1920px)
- [ ] Browser compatibility (Chrome, Firefox, Safari, Edge)
- [ ] WASM loads and initializes <500ms
- [ ] No layout shifts during load
- [ ] Images optimized (WebP, lazy loading)
- [ ] Fonts optimized (WOFF2, preload)
