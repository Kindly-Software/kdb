# Clippy Capsule Verification Lint

**Custom Clippy lint for detecting unverified computational capsules.**

## Overview

This lint catches capsules (structs with `#[repr(C, align(N))]`) that lack compile-time verification macros, preventing runtime bugs from alignment/size mismatches.

## UCE33 Framework Applied

- **Q30 (Validation)**: Compile-time verification enforcement
- **Q33 (Atomic Capsule)**: All capsules must be verified

## Lint Specification

- **Name**: `clippy::missing_capsule_verification`
- **Level**: Warning (upgrade to Error in CI/CD)
- **Trigger**: `#[repr(C, align(N))]` struct without verification
- **Message**: "Capsule missing compile-time verification"
- **Suggestion**: "Add verify_capsule_properties! macro"

## Installation

### Option 1: Build from source

```bash
# Build the lint crate
cd clippy-capsule-verify
cargo build --release

# Copy to clippy plugins directory
cp target/release/libclipper_capsule_verify.so ~/.cargo/clippy-plugins/
```

### Option 2: Use via Cargo.toml (workspace)

```toml
[workspace.lints.clippy]
missing_capsule_verification = "warn"
```

## Usage

### Basic usage

```bash
# Run with warning level
cargo clippy

# Enforce in CI (error level)
cargo clippy -- -D clippy::missing_capsule_verification
```

### Load custom lint

```bash
# Set clippy config directory
CLIPPY_CONF_DIR=path/to/clippy-capsule-verify cargo clippy
```

## Examples

### Bad: Missing verification

```rust
#[repr(C, align(64))]
struct UnverifiedCapsule {
    state: AtomicU64,
}

// Warning: capsule struct `UnverifiedCapsule` is missing compile-time verification
// Help: add verification: `verify_capsule_properties!(UnverifiedCapsule, 64, SIZE)`
```

### Good: Has verification macro

```rust
#[repr(C, align(64))]
struct VerifiedCapsule {
    state: AtomicU64,
}

// Manual verification
verify_capsule_properties!(VerifiedCapsule, 64, 8);
```

### Good: Has derive macro

```rust
#[derive(ComputationalCapsule)]
#[repr(C, align(64))]
struct DerivedCapsule {
    state: AtomicU64,
}

// Derive macro provides verification automatically
```

## Suppression

For special cases (e.g., external FFI types):

```rust
#[allow(clippy::missing_capsule_verification)]
#[repr(C, align(64))]
struct FfiCapsule {
    external_data: [u8; 64],
}
```

### When to suppress:

1. **External FFI types**: Types from C libraries that you cannot control
2. **Testing code**: Temporary test structures
3. **Legacy code**: Gradual migration (suppress temporarily)

### When NOT to suppress:

1. **Production capsules**: All production code MUST have verification
2. **Performance-critical paths**: Verification is zero-cost (compile-time only)
3. **New code**: Always add verification from day one

## CI/CD Integration

### GitHub Actions

```yaml
name: Clippy Verification Check

on: [push, pull_request]

jobs:
  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: nightly
          components: clippy

      - name: Run clippy with verification enforcement
        run: |
          cargo clippy --all-targets -- \
            -D clippy::missing_capsule_verification
```

### GitLab CI

```yaml
clippy:
  stage: test
  script:
    - cargo clippy --all-targets -- -D clippy::missing_capsule_verification
  only:
    - merge_requests
    - main
```

## Testing

```bash
# Run UI tests
cargo test

# Run on specific crate
cd ../atomic_capsule
cargo clippy -- -D clippy::missing_capsule_verification
```

## Implementation Details

### Detection Logic

The lint checks for:

1. **Struct with `#[repr(C, align(N))]`**: Triggers inspection
2. **Has `#[derive(ComputationalCapsule)]`**: ✅ OK (derive provides verification)
3. **Has verification macro in module**: ✅ OK (manual verification exists)
4. **Neither found**: ⚠️ Warning emitted

### Verification Macros Detected

- `verify_capsule_properties!` - Full verification (alignment + size)
- `verify_alignment_only!` - Alignment-only verification
- `verify_size_only!` - Size-only verification
- `verify_capsule!` - Trait-based verification

### How it works

1. **Parse attributes**: Find `#[repr(C, align(N))]`
2. **Check derive**: Look for `#[derive(ComputationalCapsule)]`
3. **Scan module HIR**: Look for `const _: () = { ... }` (verification macro expansion)
4. **Emit diagnostic**: If neither found, warn with helpful message

## Known Limitations

1. **Module-level detection**: Currently checks if ANY verification macro exists in the module (conservative)
2. **Macro name matching**: Cannot perfectly match macro arguments post-expansion
3. **Cross-module verification**: Verification in different module not detected

### Future Improvements

- [ ] Exact struct name matching in macro arguments (requires AST analysis)
- [ ] Cross-module verification detection
- [ ] Auto-fix suggestion (insert verification macro)
- [ ] Batch verification reporting

## Performance

- **Compile-time**: <1ms overhead per capsule (runs during normal clippy pass)
- **Runtime**: Zero cost (lint only affects compilation)

## ASSUM Framework

- `#ASSUME_VERIFICATION_EXISTS`: All capsules have verification
- `#VERIFY_LINT_DETECTS`: UI tests prove lint catches violations
- `#ASSUME_NO_FALSE_POSITIVES`: Derive macro + manual macros both accepted

## Version

Current version: **0.1.0**

## License

MIT OR Apache-2.0 (same as atomic_capsule)

## References

- [The Computational Capsule](../../Docs/The%20Computational%20Capsule.md) - Foundation
- [UCE33 Framework](../../projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE33_FRAMEWORK.md) - Systematic discovery
- [ASSUM Safety](../../projects/kindly-ecosystem/kindly-main/docs/frameworks/ASSUM_SAFETY.md) - Safety validation
- [atomic_capsule](../atomic_capsule/) - Foundation crate
