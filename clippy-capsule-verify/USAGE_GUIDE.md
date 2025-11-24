# Clippy Capsule Verification - Usage Guide

**Practical guide for enforcing capsule verification in your project.**

## Quick Start

### 1. Add lint to your project

Add to `.cargo/config.toml`:

```toml
[target.'cfg(all())']
rustflags = [
    # Load custom clippy lint
    "--extern", "clippy_capsule_verify=path/to/clippy-capsule-verify/target/release/libclipper_capsule_verify.so"
]
```

### 2. Enable in CI/CD

```yaml
# .github/workflows/clippy.yml
- name: Clippy verification check
  run: |
    cargo clippy --all-targets -- \
      -D clippy::missing_capsule_verification
```

### 3. Fix warnings

```bash
# Find all unverified capsules
cargo clippy 2>&1 | grep "missing_capsule_verification"

# Example output:
# warning: capsule struct `MyCapsule` is missing compile-time verification
#   --> src/my_module.rs:10:1
#    |
# 10 | #[repr(C, align(64))]
#    | ^^^^^^^^^^^^^^^^^^^^^
#    |
#    = help: add verification: `verify_capsule_properties!(MyCapsule, 64, SIZE)`
```

## Real-World Examples

### Example 1: Atomic Circuit Breaker

**Before (unverified):**

```rust
#[repr(C, align(64))]
struct CircuitBreakerCapsule {
    state: AtomicU64,
}

// ⚠️ Warning: missing verification
```

**After (verified):**

```rust
use atomic_capsule::verify_capsule_properties;

#[repr(C, align(64))]
struct CircuitBreakerCapsule {
    state: AtomicU64,
}

// ✅ Verified at compile-time
verify_capsule_properties!(CircuitBreakerCapsule, 64, 8);
```

### Example 2: SIMD Trading Signal

**Before (unverified):**

```rust
#[cfg(feature = "portable_simd")]
#[repr(C, align(64))]
struct SimdSignalCapsule {
    signals: std::simd::f32x8,
}

// ⚠️ Warning: missing verification
```

**After (verified):**

```rust
#[cfg(feature = "portable_simd")]
use atomic_capsule::verify_simd_capsule;

#[cfg(feature = "portable_simd")]
#[repr(C, align(64))]
struct SimdSignalCapsule {
    signals: std::simd::f32x8,
}

#[cfg(feature = "portable_simd")]
// ✅ SIMD-specific verification
verify_simd_capsule!(SimdSignalCapsule, 64, 32);
```

### Example 3: Dual-Channel Coordination

**Before (unverified):**

```rust
#[repr(C, align(128))]
struct DualAtomicCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
}

// ⚠️ Warning: missing verification
```

**After (verified):**

```rust
use atomic_capsule::verify_dual_atomic_u64;

#[repr(C, align(128))]
struct DualAtomicCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
}

// ✅ DualAtomicU64 pattern verification
verify_dual_atomic_u64!(DualAtomicCapsule);
```

### Example 4: Derive Macro (No manual verification needed)

```rust
use atomic_capsule::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[repr(C, align(64))]
struct AutoVerifiedCapsule {
    state: AtomicU64,
}

// ✅ No warning - derive macro provides verification
```

## Migration Strategy

### Phase 1: Audit (Week 1)

Run lint in warning mode to find all unverified capsules:

```bash
# Count unverified capsules
cargo clippy 2>&1 | grep -c "missing_capsule_verification"

# List all locations
cargo clippy 2>&1 | grep -A 3 "missing_capsule_verification" > audit.txt
```

### Phase 2: Fix Critical (Week 2)

Add verification to hot-path capsules first:

1. Circuit breakers (<100ns latency)
2. Market data capsules (high-frequency)
3. Risk management (safety-critical)

```bash
# Fix one module at a time
cargo clippy --package my_trading_core -- \
  -D clippy::missing_capsule_verification
```

### Phase 3: Fix Remaining (Week 3-4)

Add verification to remaining capsules:

```bash
# Enforce project-wide
cargo clippy --workspace -- \
  -D clippy::missing_capsule_verification
```

### Phase 4: Lock Down (Week 5)

Enable in CI to prevent regressions:

```yaml
# Block PRs with unverified capsules
- name: Enforce verification
  run: cargo clippy -- -D clippy::missing_capsule_verification
```

## Suppression Guidelines

### ✅ Acceptable Suppressions

**FFI Types:**
```rust
// External C library types
#[allow(clippy::missing_capsule_verification)]
#[repr(C, align(64))]
struct ExternalCapsule {
    // Controlled by external library
    opaque_data: [u8; 64],
}
```

**Test Fixtures:**
```rust
#[cfg(test)]
mod tests {
    #[allow(clippy::missing_capsule_verification)]
    #[repr(C, align(64))]
    struct TestCapsule {
        // Temporary test structure
        data: u64,
    }
}
```

**Gradual Migration:**
```rust
// TODO: Add verification before 2025-12-01
#[allow(clippy::missing_capsule_verification)]
#[repr(C, align(64))]
struct LegacyCapsule {
    state: AtomicU64,
}
```

### ❌ Unacceptable Suppressions

**Production Code:**
```rust
// BAD: Production hot path without verification
#[allow(clippy::missing_capsule_verification)]  // ❌ NOT OK
#[repr(C, align(64))]
struct TradingSignalCapsule {
    price: AtomicU64,
}
```

**Safety-Critical Code:**
```rust
// BAD: Risk management without verification
#[allow(clippy::missing_capsule_verification)]  // ❌ NOT OK
#[repr(C, align(64))]
struct RiskLimitCapsule {
    max_position: AtomicU64,
}
```

## Performance Impact

### Compile-Time

- **Lint overhead**: <1ms per capsule (runs during clippy pass)
- **Verification macros**: 0ms runtime (compile-time only)
- **Total impact**: Negligible (<0.1% build time increase)

### Runtime

- **Verification macros**: Zero cost (const assertions)
- **Lint checks**: Zero cost (compile-time only)
- **Performance**: No runtime impact

## Troubleshooting

### Issue: False positives

**Symptom:**
```
warning: capsule struct `MyVerifiedCapsule` is missing compile-time verification
```

But you have verification:
```rust
verify_capsule_properties!(MyVerifiedCapsule, 64, 8);
```

**Solution:**

Ensure verification is in the same module:

```rust
mod my_module {
    #[repr(C, align(64))]
    struct MyVerifiedCapsule { /* ... */ }

    // ✅ Verification in same module
    verify_capsule_properties!(MyVerifiedCapsule, 64, 8);
}
```

### Issue: Cross-module verification not detected

**Symptom:**

Verification in separate module not detected:

```rust
// mod_a.rs
#[repr(C, align(64))]
pub struct MyCapsule { /* ... */ }

// mod_b.rs (different module)
verify_capsule_properties!(MyCapsule, 64, 8);  // ⚠️ Not detected
```

**Solution:**

Move verification to same module or use `#[allow]`:

```rust
// mod_a.rs
#[repr(C, align(64))]
pub struct MyCapsule { /* ... */ }

// ✅ Same module
verify_capsule_properties!(MyCapsule, 64, 8);
```

### Issue: Derive macro not recognized

**Symptom:**

Derive macro triggers warning:

```rust
#[derive(ComputationalCapsule)]  // Still warns?
#[repr(C, align(64))]
struct MyCapsule { /* ... */ }
```

**Solution:**

Check derive macro is from `atomic_capsule`:

```rust
use atomic_capsule::ComputationalCapsule;  // ✅ Correct import

#[derive(ComputationalCapsule)]
#[repr(C, align(64))]
struct MyCapsule { /* ... */ }
```

## Best Practices

### 1. Add verification immediately

```rust
// ✅ Define + verify together
#[repr(C, align(64))]
struct NewCapsule { /* ... */ }
verify_capsule_properties!(NewCapsule, 64, SIZE);
```

### 2. Use derive for simple capsules

```rust
// ✅ Derive for standard patterns
#[derive(ComputationalCapsule)]
#[repr(C, align(64))]
struct SimpleCapsule { /* ... */ }
```

### 3. Manual verification for complex patterns

```rust
// ✅ Manual for custom patterns
#[repr(C, align(128))]
struct ComplexCapsule { /* ... */ }

verify_dual_atomic_u64!(ComplexCapsule);
verify_generation_counter!(ComplexCapsule, generation);
verify_thread_safe!(ComplexCapsule);
```

### 4. Document suppressions

```rust
// ✅ Explain WHY suppressed
/// FFI type from external library - verification not possible
#[allow(clippy::missing_capsule_verification)]
#[repr(C, align(64))]
struct ExternalType { /* ... */ }
```

## References

- [README.md](README.md) - Overview and installation
- [The Computational Capsule](../../Docs/The%20Computational%20Capsule.md) - Foundation
- [atomic_capsule verification](../atomic_capsule/src/verification.rs) - Verification macros
- [UCE33 Q33](../../projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE33_FRAMEWORK.md) - Validation framework
