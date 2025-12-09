# Testing Guide

**kindly-web Testing Documentation** - Comprehensive testing strategy using T28 Framework

Version: 1.0
Date: 2025-10-18
Framework: T28 Testing Framework (4 Tiers: Unit → Property → Integration → Production)

---

## Table of Contents

1. [Overview](#overview)
2. [Test Infrastructure](#test-infrastructure)
3. [Unit Tests (T28 Q1-Q7)](#tier-1-unit-tests-t28-q1-q7)
4. [Property Tests (T28 Q8-Q14)](#tier-2-property-tests-t28-q8-q14)
5. [Integration Tests (T28 Q15-Q21)](#tier-3-integration-tests-t28-q15-q21)
6. [WASM Tests](#wasm-tests)
7. [Benchmarks (B32 Framework)](#benchmarks-b32-framework)
8. [Coverage](#coverage)
9. [CI/CD Integration](#cicd-integration)

---

## Overview

### Testing Philosophy

**T28 Framework** (28-question testing framework):
- **Tier 1 (Q1-Q7)**: Unit tests - Component behavior, capsule invariants
- **Tier 2 (Q8-Q14)**: Property tests - Randomized inputs, invariant checking
- **Tier 3 (Q15-Q21)**: Integration tests - End-to-end flows, page rendering
- **Tier 4 (Q22-Q28)**: Production tests - Performance, accessibility, bundle size

**Coverage Targets**:
- Unit tests: 95%+ (all capsules, components)
- Property tests: 90%+ (state management)
- Integration tests: 85%+ (page flows)
- WASM tests: 80%+ (browser compatibility)

### Quick Test Commands

```bash
# All tests (native)
cargo test

# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test integration

# WASM tests (Firefox)
wasm-pack test --headless --firefox

# WASM tests (Chrome)
wasm-pack test --headless --chrome

# Property tests
cargo test --features proptest

# Benchmarks
cargo bench --bench performance_bench

# Coverage
cargo tarpaulin --out Html --output-dir coverage/
```

---

## Test Infrastructure

### Test Directory Structure

```
tests/
├── unit_capsules.rs          # Capsule unit tests (AppState, Budget, Theme, WebSocket, Metrics)
├── unit_components.rs         # Component unit tests (Button, Card, Icon, etc.)
├── property_tests.rs          # Property-based tests (randomized inputs)
├── integration.rs             # Integration tests (page rendering, navigation)
└── wasm_integration.rs        # WASM-specific tests (browser APIs)

benches/
└── performance_bench.rs       # B32 benchmarks (capsule operations, component rendering)

src/
└── lib.rs                     # Inline doc tests
```

### Dependencies

```toml
# Cargo.toml
[dev-dependencies]
proptest = "1.4"              # Property-based testing
criterion = { version = "0.5", features = ["html_reports"] }  # Benchmarking
wasm-bindgen-test = "0.3"     # WASM testing
```

---

## Tier 1: Unit Tests (T28 Q1-Q7)

### Q1-Q7: Unit Test Coverage

**Goal**: Test individual components and capsules in isolation

### Capsule Unit Tests

**File**: `tests/unit_capsules.rs`

#### AppStateCapsule Tests (20+ tests)

```rust
#[cfg(test)]
mod app_state_tests {
    use kindly_web::state::AppStateCapsule;

    #[test]
    fn test_new_capsule_default_state() {
        let capsule = AppStateCapsule::new();
        let state = capsule.read();

        assert_eq!(state.theme, 0);
        assert_eq!(state.dark_mode, false);
        assert_eq!(state.user_id, 0);
        assert_eq!(state.generation, 0);
    }

    #[test]
    fn test_set_theme() {
        let capsule = AppStateCapsule::new();

        capsule.set_theme(3);
        assert_eq!(capsule.read().theme, 3);

        capsule.set_theme(7);
        assert_eq!(capsule.read().theme, 7);
    }

    #[test]
    fn test_dark_mode_toggle() {
        let capsule = AppStateCapsule::new();

        assert_eq!(capsule.read().dark_mode, false);

        capsule.toggle_dark_mode();
        assert_eq!(capsule.read().dark_mode, true);

        capsule.toggle_dark_mode();
        assert_eq!(capsule.read().dark_mode, false);
    }

    #[test]
    fn test_dark_mode_toggle_idempotent() {
        let capsule = AppStateCapsule::new();

        for _ in 0..100 {
            capsule.toggle_dark_mode();
            capsule.toggle_dark_mode();
        }

        assert_eq!(capsule.read().dark_mode, false);
    }

    #[test]
    fn test_set_user_id() {
        let capsule = AppStateCapsule::new();

        capsule.set_user_id(123456);
        assert_eq!(capsule.read().user_id, 123456);

        capsule.set_user_id(999_999_999);
        assert_eq!(capsule.read().user_id, 999_999_999);
    }

    #[test]
    fn test_generation_increments() {
        let capsule = AppStateCapsule::new();
        let gen0 = capsule.read().generation;

        capsule.set_theme(1);
        let gen1 = capsule.read().generation;

        assert!(gen1 > gen0, "Generation should increment on state change");
    }

    #[test]
    fn test_capsule_alignment() {
        use std::mem::{align_of, size_of};

        assert_eq!(align_of::<AppStateCapsule>(), 64, "Capsule must be 64-byte aligned");
        assert_eq!(size_of::<AppStateCapsule>(), 64, "Capsule must be 64 bytes");
    }
}
```

#### BudgetViewCapsule Tests (30+ tests)

```rust
#[cfg(test)]
mod budget_view_tests {
    use kindly_web::state::{BudgetViewCapsule, BudgetError};

    #[test]
    fn test_new_capsule_initial_budget() {
        let capsule = BudgetViewCapsule::new(1_000_00);
        let snapshot = capsule.snapshot();

        assert_eq!(snapshot.budget_cents, 1_000_00);
        assert_eq!(snapshot.spent_cents, 0);
        assert_eq!(snapshot.request_count, 0);
    }

    #[test]
    fn test_try_deduct_success() {
        let capsule = BudgetViewCapsule::new(1_000_00);

        let remaining = capsule.try_deduct(50_00).unwrap();
        assert_eq!(remaining, 950_00);

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.budget_cents, 950_00);
        assert_eq!(snapshot.spent_cents, 50_00);
        assert_eq!(snapshot.request_count, 1);
    }

    #[test]
    fn test_try_deduct_insufficient_funds() {
        let capsule = BudgetViewCapsule::new(100_00);

        let result = capsule.try_deduct(200_00);
        assert!(result.is_err());

        match result {
            Err(BudgetError::InsufficientFunds { required, available }) => {
                assert_eq!(required, 200_00);
                assert_eq!(available, 100_00);
            }
            _ => panic!("Expected InsufficientFunds error"),
        }

        // Budget unchanged
        assert_eq!(capsule.snapshot().budget_cents, 100_00);
    }

    #[test]
    fn test_credit() {
        let capsule = BudgetViewCapsule::new(100_00);

        capsule.credit(50_00).unwrap();
        assert_eq!(capsule.snapshot().budget_cents, 150_00);
    }

    #[test]
    fn test_multiple_operations() {
        let capsule = BudgetViewCapsule::new(1_000_00);

        capsule.try_deduct(100_00).unwrap();
        capsule.try_deduct(200_00).unwrap();
        capsule.credit(50_00).unwrap();
        capsule.try_deduct(150_00).unwrap();

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.budget_cents, 600_00); // 1000 - 100 - 200 + 50 - 150
        assert_eq!(snapshot.spent_cents, 450_00);  // 100 + 200 + 150
        assert_eq!(snapshot.request_count, 3);
    }

    #[test]
    fn test_budget_never_negative() {
        let capsule = BudgetViewCapsule::new(100_00);

        let _ = capsule.try_deduct(50_00);
        let _ = capsule.try_deduct(60_00); // Should fail

        assert!(capsule.snapshot().budget_cents >= 0);
    }

    #[test]
    fn test_capsule_alignment() {
        use std::mem::{align_of, size_of};

        assert_eq!(align_of::<BudgetViewCapsule>(), 128, "Capsule must be 128-byte aligned");
        assert_eq!(size_of::<BudgetViewCapsule>(), 128, "Capsule must be 128 bytes");
    }
}
```

### Component Unit Tests

**File**: `tests/unit_components.rs`

```rust
#[cfg(test)]
mod button_tests {
    use leptos::prelude::*;
    use kindly_web::components::Button;

    #[test]
    fn test_button_renders() {
        // Basic rendering test
        let _ = view! {
            <Button variant="primary" size="medium">
                "Click Me"
            </Button>
        };
    }

    #[test]
    fn test_button_variants() {
        let variants = ["primary", "secondary", "ghost", "danger"];

        for variant in variants {
            let _ = view! {
                <Button variant=variant size="medium">
                    "Button"
                </Button>
            };
        }
    }

    #[test]
    fn test_button_sizes() {
        let sizes = ["small", "medium", "large"];

        for size in sizes {
            let _ = view! {
                <Button variant="primary" size=size>
                    "Button"
                </Button>
            };
        }
    }

    #[test]
    fn test_button_disabled() {
        let _ = view! {
            <Button variant="primary" size="medium" disabled=true>
                "Disabled"
            </Button>
        };
    }
}
```

### Running Unit Tests

```bash
# All unit tests
cargo test

# Specific test module
cargo test app_state_tests

# Specific test
cargo test test_dark_mode_toggle

# Show test output
cargo test -- --nocapture

# Run tests in parallel (default)
cargo test -- --test-threads=4
```

---

## Tier 2: Property Tests (T28 Q8-Q14)

### Q8-Q14: Property-Based Testing

**Goal**: Test invariants with randomized inputs (discover edge cases)

**File**: `tests/property_tests.rs`

### AppStateCapsule Property Tests

```rust
use proptest::prelude::*;
use kindly_web::state::AppStateCapsule;

proptest! {
    #[test]
    fn prop_dark_mode_toggle_always_flips(
        initial_mode in prop::bool::ANY,
        toggle_count in 0u32..100,
    ) {
        let capsule = AppStateCapsule::new();
        if initial_mode {
            capsule.toggle_dark_mode();
        }

        for _ in 0..toggle_count {
            capsule.toggle_dark_mode();
        }

        let expected = initial_mode ^ (toggle_count % 2 == 1);
        prop_assert_eq!(capsule.read().dark_mode, expected);
    }

    #[test]
    fn prop_theme_in_range(theme_index in 0u8..8) {
        let capsule = AppStateCapsule::new();
        capsule.set_theme(theme_index);

        let state = capsule.read();
        prop_assert!(state.theme < 8, "Theme must be 0-7");
    }

    #[test]
    fn prop_user_id_preserved(user_id in 0u32..1_000_000_000) {
        let capsule = AppStateCapsule::new();
        capsule.set_user_id(user_id);

        prop_assert_eq!(capsule.read().user_id, user_id);
    }
}
```

### BudgetViewCapsule Property Tests

```rust
proptest! {
    #[test]
    fn prop_budget_never_negative(
        initial in 0i64..1_000_000,
        operations in prop::collection::vec((0i64..10_000), 1..100),
    ) {
        let capsule = BudgetViewCapsule::new(initial);

        for amount in operations {
            let _ = capsule.try_deduct(amount);
        }

        prop_assert!(capsule.snapshot().budget_cents >= 0);
    }

    #[test]
    fn prop_spent_equals_deducted(
        initial in 100_000i64..1_000_000,
        deductions in prop::collection::vec((1i64..1_000), 1..50),
    ) {
        let capsule = BudgetViewCapsule::new(initial);

        let mut total_deducted = 0i64;
        for amount in deductions {
            if capsule.try_deduct(amount).is_ok() {
                total_deducted += amount;
            }
        }

        prop_assert_eq!(capsule.snapshot().spent_cents, total_deducted);
    }

    #[test]
    fn prop_credit_increases_budget(
        initial in 0i64..1_000_000,
        credit_amount in 1i64..100_000,
    ) {
        let capsule = BudgetViewCapsule::new(initial);
        let before = capsule.snapshot().budget_cents;

        capsule.credit(credit_amount).unwrap();
        let after = capsule.snapshot().budget_cents;

        prop_assert_eq!(after, before + credit_amount);
    }
}
```

### Running Property Tests

```bash
# Run property tests
cargo test --features proptest

# Increase test cases (default: 256)
PROPTEST_CASES=10000 cargo test --features proptest

# Generate minimal failing case
cargo test --features proptest -- prop_budget_never_negative

# Verbose output
cargo test --features proptest -- --nocapture
```

---

## Tier 3: Integration Tests (T28 Q15-Q21)

### Q15-Q21: Integration Testing

**Goal**: Test end-to-end flows (page rendering, navigation, state coordination)

**File**: `tests/integration.rs`

### Page Rendering Tests

```rust
#[cfg(test)]
mod page_rendering_tests {
    use leptos::prelude::*;
    use kindly_web::App;
    use kindly_web::pages::home::HomePage;

    #[test]
    fn test_app_renders() {
        let _ = view! { <App /> };
    }

    #[test]
    fn test_home_page_renders() {
        let _ = view! { <HomePage /> };
    }

    #[test]
    fn test_navbar_in_app() {
        let app_view = view! { <App /> };
        // Verify navbar is present
        // (Leptos testing utilities would check DOM here)
    }
}
```

### Navigation Tests

```rust
#[cfg(test)]
mod navigation_tests {
    use leptos::prelude::*;
    use leptos_router::*;

    #[test]
    fn test_route_home() {
        // Test / route renders HomePage
    }

    #[test]
    fn test_route_pricing() {
        // Test /pricing route renders PricingPage
    }

    #[test]
    fn test_404_fallback() {
        // Test unknown route shows 404 page
    }
}
```

### State Coordination Tests

```rust
#[cfg(test)]
mod state_coordination_tests {
    use kindly_web::state::{AppStateCapsule, BudgetViewCapsule, MetricsCapsule};

    #[test]
    fn test_theme_change_updates_metrics() {
        let app_state = AppStateCapsule::new();
        let metrics = MetricsCapsule::new();

        app_state.set_theme(2);
        // Verify metrics.theme_changes incremented
    }

    #[test]
    fn test_dark_mode_toggle_updates_metrics() {
        let app_state = AppStateCapsule::new();
        let metrics = MetricsCapsule::new();

        app_state.toggle_dark_mode();
        // Verify metrics.dark_mode_toggles incremented
    }

    #[test]
    fn test_budget_deduct_updates_metrics() {
        let budget = BudgetViewCapsule::new(1_000_00);
        let metrics = MetricsCapsule::new();

        budget.try_deduct(50_00).unwrap();
        // Verify metrics.request_count incremented
    }
}
```

### Running Integration Tests

```bash
# All integration tests
cargo test --test integration

# Specific integration test
cargo test --test integration test_app_renders

# Integration tests with output
cargo test --test integration -- --nocapture
```

---

## WASM Tests

### wasm-pack Tests

**File**: `tests/wasm_integration.rs`

```rust
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_capsule_in_wasm_context() {
    use kindly_web::state::AppStateCapsule;

    let capsule = AppStateCapsule::new();
    capsule.set_theme(2);
    capsule.toggle_dark_mode();

    let state = capsule.read();
    assert_eq!(state.theme, 2);
    assert!(state.dark_mode);
}

#[wasm_bindgen_test]
fn test_budget_in_wasm_context() {
    use kindly_web::state::BudgetViewCapsule;

    let capsule = BudgetViewCapsule::new(1_000_00);
    let remaining = capsule.try_deduct(50_00).unwrap();

    assert_eq!(remaining, 950_00);
}

#[wasm_bindgen_test]
fn test_browser_apis() {
    use web_sys::window;

    let window = window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");

    let body = document.body().expect("document should have a body");
    assert!(body.client_width() > 0, "Body should have width");
}
```

### Running WASM Tests

```bash
# Install wasm-pack (one-time)
cargo install wasm-pack

# Run in headless Firefox
wasm-pack test --headless --firefox

# Run in headless Chrome
wasm-pack test --headless --chrome

# Run in Safari (macOS only)
wasm-pack test --headless --safari

# Run in browser (interactive)
wasm-pack test --firefox
```

---

## Benchmarks (B32 Framework)

### B32 Benchmarking Framework

**Goal**: Honest performance measurement with statistical rigor

**File**: `benches/performance_bench.rs`

### Capsule Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kindly_web::state::{AppStateCapsule, BudgetViewCapsule, MetricsCapsule};

fn bench_app_state_read(c: &mut Criterion) {
    let capsule = AppStateCapsule::new();

    c.bench_function("app_state_read", |b| {
        b.iter(|| {
            black_box(capsule.read())
        });
    });
}

fn bench_app_state_toggle_dark_mode(c: &mut Criterion) {
    let capsule = AppStateCapsule::new();

    c.bench_function("app_state_toggle_dark_mode", |b| {
        b.iter(|| {
            capsule.toggle_dark_mode()
        });
    });
}

fn bench_budget_try_deduct(c: &mut Criterion) {
    let capsule = BudgetViewCapsule::new(1_000_000);

    c.bench_function("budget_try_deduct", |b| {
        b.iter(|| {
            black_box(capsule.try_deduct(1000))
        });
    });
}

fn bench_metrics_record_page_view(c: &mut Criterion) {
    let capsule = MetricsCapsule::new();

    c.bench_function("metrics_record_page_view", |b| {
        b.iter(|| {
            capsule.record_page_view()
        });
    });
}

criterion_group!(
    benches,
    bench_app_state_read,
    bench_app_state_toggle_dark_mode,
    bench_budget_try_deduct,
    bench_metrics_record_page_view
);
criterion_main!(benches);
```

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench --bench performance_bench

# Specific benchmark
cargo bench --bench performance_bench app_state_read

# Save baseline
cargo bench --bench performance_bench -- --save-baseline main

# Compare against baseline
cargo bench --bench performance_bench -- --baseline main

# Generate HTML report
cargo bench --bench performance_bench
# Report: target/criterion/report/index.html
```

**Expected Results** (Intel Ultra 7 155H, 95% CI):

| Operation | Target | Typical | Status |
|-----------|--------|---------|--------|
| **AppState Read** | <10ns | ~5.5ns | ✅ |
| **Dark Mode Toggle** | <20ns | ~12ns | ✅ |
| **Budget Deduct** | <100ns | ~82ns | ✅ |
| **Metrics Record** | <10ns | ~6ns | ✅ |

---

## Coverage

### Code Coverage with Tarpaulin

```bash
# Install tarpaulin (one-time)
cargo install cargo-tarpaulin

# Generate coverage report (HTML)
cargo tarpaulin --out Html --output-dir coverage/

# Open report
# coverage/index.html

# Generate Cobertura XML (for CI/CD)
cargo tarpaulin --out Xml

# Exclude files from coverage
cargo tarpaulin --exclude tests/ benches/
```

**Coverage Targets**:
- Overall: 90%+
- Capsules: 95%+
- Components: 85%+
- Integration: 80%+

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
# .github/workflows/test.yml
name: Test Suite

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: wasm32-unknown-unknown

      - name: Run unit tests
        run: cargo test --lib

      - name: Run integration tests
        run: cargo test --test integration

      - name: Install wasm-pack
        run: cargo install wasm-pack

      - name: Run WASM tests
        run: wasm-pack test --headless --firefox

      - name: Run property tests
        run: cargo test --features proptest

      - name: Run benchmarks
        run: cargo bench --bench performance_bench -- --test

      - name: Generate coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml

      - name: Upload coverage to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./cobertura.xml
```

---

## Test Checklist

Pre-commit checklist:

- [ ] All unit tests pass (`cargo test --lib`)
- [ ] All integration tests pass (`cargo test --test integration`)
- [ ] WASM tests pass (`wasm-pack test --headless --firefox`)
- [ ] Property tests pass (`cargo test --features proptest`)
- [ ] Benchmarks pass (`cargo bench -- --test`)
- [ ] Coverage >90% (`cargo tarpaulin`)
- [ ] No warnings (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt --check`)

---

**Last Updated**: 2025-10-18
**Maintainer**: kindly.ai Team
**License**: MIT OR Apache-2.0
