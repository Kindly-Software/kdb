# kindly.video

Premium WASM landing page for kindly-av1 - the world's fastest GPU-accelerated AV1 encoder.

## Features

- **Pure Rust/WASM** - Built with Leptos 0.7 (CSR mode)
- **WebGL2 Effects** - GPU-accelerated mesh gradient background
- **Glassmorphism UI** - Modern frosted glass design
- **Byzantine Purple + Gold** - Premium branding theme
- **<2MB Bundle** - Optimized WASM output

## Tech Stack

- [Leptos 0.7](https://leptos.dev/) - Rust web framework
- [WebGL2](https://developer.mozilla.org/en-US/docs/Web/API/WebGL2RenderingContext) - GPU effects
- [Trunk](https://trunkrs.dev/) - WASM bundler
- GitHub Pages / Fly.io - Deployment

## Development

```bash
# Install dependencies
rustup target add wasm32-unknown-unknown
cargo install trunk wasm-bindgen-cli

# Run dev server
trunk serve

# Build for production
trunk build --release
```

## Deployment

Push to `main` branch - GitHub Actions will auto-deploy to GitHub Pages.

For custom domain (kindly.video):
1. Add CNAME file to dist/
2. Configure DNS to point to GitHub Pages

## License

Proprietary - Kindly Technologies
