# Build Notes for Clippy Capsule Verify

## Important: Rustc Private Dependencies

This crate uses `rustc_private` dependencies which are compiler-internal APIs. These dependencies do NOT have traditional version numbers in Cargo.toml.

### Why `0.0.0` versions?

The rustc internal crates (`rustc_ast`, `rustc_hir`, etc.) use `0.0.0` as placeholder versions. They resolve to the version matching your installed rustc compiler.

### Build Requirements

1. **Nightly Rust**: Required for `#![feature(rustc_private)]`
2. **rustc-dev**: Component providing rustc internal libraries
3. **Matching versions**: All rustc crates automatically match your compiler version

### Installation

```bash
# Install nightly rust
rustup toolchain install nightly

# Install rustc-dev component (provides rustc internal libraries)
rustup component add rustc-dev --toolchain nightly

# Build the lint crate
cargo +nightly build --release
```

### Why This Approach?

**Alternative approaches considered**:

1. **Fork Clippy**: Too heavyweight, hard to maintain
2. **Procedural macro**: Cannot intercept struct definitions
3. **Custom lint crate**: ✅ Chosen - Clean, maintainable, standard approach

### Common Build Issues

#### Issue: "can't find crate for `rustc_ast`"

**Solution**: Install rustc-dev component

```bash
rustup component add rustc-dev --toolchain nightly
```

#### Issue: "feature `rustc_private` is unstable"

**Solution**: Use nightly toolchain

```bash
cargo +nightly build
```

#### Issue: "failed to resolve: use of undeclared crate or module"

**Solution**: Ensure all rustc dependencies are listed in Cargo.toml

### Development Workflow

```bash
# Build
cargo +nightly build

# Test
cargo +nightly test

# Run on another crate
cd ../atomic_capsule
cargo +nightly clippy -- -D clippy::missing_capsule_verification
```

### CI/CD Configuration

GitHub Actions:

```yaml
- name: Install rust toolchain
  uses: actions-rs/toolchain@v1
  with:
    toolchain: nightly
    components: rustc-dev, clippy
    override: true

- name: Build lint crate
  run: cargo +nightly build --release
```

### Dependency Resolution

The rustc internal crates resolve as follows:

```
rustc_ast     → matches your rustc version (e.g., 1.77.0-nightly)
rustc_hir     → matches your rustc version
rustc_lint    → matches your rustc version
rustc_middle  → matches your rustc version
rustc_session → matches your rustc version
rustc_span    → matches your rustc version
```

This automatic matching ensures the lint works with your exact compiler version.

### Alternative: Standalone Binary (Future)

For users who don't want to deal with rustc_private, we could provide:

1. **Pre-compiled binary**: Download + run (no build required)
2. **Docker image**: Contains all dependencies
3. **GitHub Action**: Run as CI step (no installation)

These are planned for V0.2.0 to improve accessibility.

### Troubleshooting

**Problem**: Build fails with "mismatched types"

**Cause**: Rustc internal APIs changed between nightly versions

**Solution**: Use the same nightly version consistently:

```bash
# Pin nightly version
rustup override set nightly-2024-10-15

# Rebuild
cargo +nightly build
```

**Problem**: Lint not detected when running clippy

**Cause**: Lint not loaded by clippy

**Solution**: Ensure lint is registered correctly (check `src/lib.rs`)

---

## Summary

✅ Use nightly Rust
✅ Install rustc-dev component
✅ `0.0.0` versions are normal (resolve to compiler version)
✅ Build with `cargo +nightly build`

This is the standard approach for custom Clippy lints and is well-supported.
