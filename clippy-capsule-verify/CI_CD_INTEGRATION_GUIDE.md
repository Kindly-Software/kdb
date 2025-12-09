# CI/CD Integration Guide

**clippy-capsule-verify** - Automated Chaos compliance enforcement in CI/CD pipelines.

## Quick Start

### GitHub Actions

```yaml
# .github/workflows/clippy-capsule-verify.yml
name: Chaos Compliance Check

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  coca-verification:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust nightly
        uses: actions-rs/toolchain@v1
        with:
          toolchain: nightly
          components: clippy
          override: true

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo index
        uses: actions/cache@v3
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-index-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache target directory
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-target-${{ hashFiles('**/Cargo.lock') }}

      - name: Run P0 Critical Lints (Deny Level)
        run: |
          cargo clippy --all-features --all-targets -- \
            -D clippy::capsule_mutex_violation \
            -D clippy::capsule_unaligned_violation \
            -D clippy::capsule_non_atomic_field \
            -D clippy::capsule_missing_generation

      - name: Run P1 High Lints (Warn Level)
        run: |
          cargo clippy --all-features --all-targets -- \
            -W clippy::missing_capsule_verification \
            -W clippy::capsule_scattered_atomics \
            -W clippy::capsule_incorrect_padding
        continue-on-error: true

      - name: Generate lint report
        if: always()
        run: |
          cargo clippy --all-features --message-format=json -- \
            -D clippy::capsule_mutex_violation \
            -D clippy::capsule_unaligned_violation \
            -D clippy::capsule_non_atomic_field \
            -D clippy::capsule_missing_generation \
          > clippy-report.json

      - name: Upload lint report
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: clippy-coca-report
          path: clippy-report.json
```

### GitLab CI

```yaml
# .gitlab-ci.yml
stages:
  - test
  - report

variables:
  CARGO_HOME: "${CI_PROJECT_DIR}/.cargo"

.rust-cache:
  cache:
    key: "${CI_COMMIT_REF_SLUG}"
    paths:
      - .cargo/
      - target/

coca-p0-critical:
  stage: test
  extends: .rust-cache
  image: rustlang/rust:nightly
  script:
    - rustc --version && cargo --version
    - cargo clippy --all-features --all-targets --
        -D clippy::capsule_mutex_violation
        -D clippy::capsule_unaligned_violation
        -D clippy::capsule_non_atomic_field
        -D clippy::capsule_missing_generation
  only:
    - merge_requests
    - main
    - develop

coca-p1-warnings:
  stage: test
  extends: .rust-cache
  image: rustlang/rust:nightly
  script:
    - cargo clippy --all-features --all-targets --
        -W clippy::missing_capsule_verification
        -W clippy::capsule_scattered_atomics
        -W clippy::capsule_incorrect_padding
  allow_failure: true
  only:
    - merge_requests
    - main

coca-report:
  stage: report
  extends: .rust-cache
  image: rustlang/rust:nightly
  script:
    - cargo clippy --all-features --message-format=json --
        -D clippy::capsule_mutex_violation
        -D clippy::capsule_unaligned_violation
        -D clippy::capsule_non_atomic_field
        -D clippy::capsule_missing_generation
      > clippy-coca-report.json
  artifacts:
    reports:
      codequality: clippy-coca-report.json
    paths:
      - clippy-coca-report.json
    expire_in: 30 days
  when: always
```

### Jenkins Pipeline

```groovy
// Jenkinsfile
pipeline {
    agent any

    environment {
        RUSTUP_HOME = "${WORKSPACE}/.rustup"
        CARGO_HOME = "${WORKSPACE}/.cargo"
    }

    stages {
        stage('Setup') {
            steps {
                sh 'rustup toolchain install nightly'
                sh 'rustup component add clippy --toolchain nightly'
            }
        }

        stage('Chaos P0 Critical Checks') {
            steps {
                sh '''
                    cargo +nightly clippy --all-features --all-targets -- \
                        -D clippy::capsule_mutex_violation \
                        -D clippy::capsule_unaligned_violation \
                        -D clippy::capsule_non_atomic_field \
                        -D clippy::capsule_missing_generation
                '''
            }
        }

        stage('Chaos P1 Warnings') {
            steps {
                sh '''
                    cargo +nightly clippy --all-features --all-targets -- \
                        -W clippy::missing_capsule_verification \
                        -W clippy::capsule_scattered_atomics \
                        -W clippy::capsule_incorrect_padding || true
                '''
            }
        }

        stage('Generate Report') {
            steps {
                sh '''
                    cargo +nightly clippy --all-features --message-format=json -- \
                        -D clippy::capsule_mutex_violation \
                        -D clippy::capsule_unaligned_violation \
                        -D clippy::capsule_non_atomic_field \
                        -D clippy::capsule_missing_generation \
                    > clippy-coca-report.json || true
                '''
                archiveArtifacts artifacts: 'clippy-coca-report.json', fingerprint: true
            }
        }
    }

    post {
        always {
            cleanWs()
        }
    }
}
```

## Pre-commit Hook

Install locally for instant feedback before commit:

```bash
#!/bin/bash
# .git/hooks/pre-commit

echo "Running Chaos compliance checks..."

# Run P0 critical lints
if ! cargo clippy --all-features --all-targets -- \
    -D clippy::capsule_mutex_violation \
    -D clippy::capsule_unaligned_violation \
    -D clippy::capsule_non_atomic_field \
    -D clippy::capsule_missing_generation 2>&1 | tee /tmp/clippy-coca.log; then

    echo "❌ Chaos P0 Critical violations detected!"
    echo "Fix violations before committing:"
    cat /tmp/clippy-coca.log
    exit 1
fi

# Run P1 warnings (non-blocking)
cargo clippy --all-features --all-targets -- \
    -W clippy::missing_capsule_verification \
    -W clippy::capsule_scattered_atomics \
    -W clippy::capsule_incorrect_padding || true

echo "✅ Chaos compliance checks passed!"
exit 0
```

Install:
```bash
chmod +x .git/hooks/pre-commit
```

## Cargo Configuration

Add to `.cargo/config.toml` for project-wide defaults:

```toml
[target.'cfg(all())']
rustflags = [
    # P0 Critical (always deny)
    "-D", "clippy::capsule_mutex_violation",
    "-D", "clippy::capsule_unaligned_violation",
    "-D", "clippy::capsule_non_atomic_field",
    "-D", "clippy::capsule_missing_generation",

    # P1 High (warn, non-blocking)
    "-W", "clippy::missing_capsule_verification",
    "-W", "clippy::capsule_scattered_atomics",
    "-W", "clippy::capsule_incorrect_padding",
]
```

## Workspace-level Configuration

For multi-crate workspaces, add to root `Cargo.toml`:

```toml
[workspace.lints.clippy]
# P0 Critical (deny)
capsule_mutex_violation = "deny"
capsule_unaligned_violation = "deny"
capsule_non_atomic_field = "deny"
capsule_missing_generation = "deny"

# P1 High (warn)
missing_capsule_verification = "warn"
capsule_scattered_atomics = "warn"
capsule_incorrect_padding = "warn"

# P2 Medium (allow by default, opt-in)
capsule_memory_ordering = "allow"
```

## Customization Examples

### Gradual Rollout (Warnings First)

```bash
# Week 1: All lints as warnings (collect data)
cargo clippy --all-features -- \
    -W clippy::capsule_mutex_violation \
    -W clippy::capsule_unaligned_violation \
    -W clippy::capsule_non_atomic_field \
    -W clippy::capsule_missing_generation

# Week 2: Fix violations

# Week 3: Upgrade to deny (enforce)
cargo clippy --all-features -- \
    -D clippy::capsule_mutex_violation \
    -D clippy::capsule_unaligned_violation \
    -D clippy::capsule_non_atomic_field \
    -D clippy::capsule_missing_generation
```

### Per-Module Suppression

```rust
// For legacy code under migration
#[allow(clippy::capsule_mutex_violation)]
mod legacy {
    // Temporary suppression during refactoring
}

// For external FFI types
#[allow(clippy::capsule_unaligned_violation)]
#[repr(C, align(64))]
struct ExternalFFIType {
    // Cannot control external library alignment
}
```

### Advanced: Custom Lint Levels per Environment

```bash
# Development: Warnings only
CLIPPY_Chaos_LEVEL=warn cargo clippy

# CI/CD: Deny level
CLIPPY_Chaos_LEVEL=deny cargo clippy

# Production: Full enforcement + P2 opt-in
cargo clippy --all-features -- \
    -D clippy::capsule_mutex_violation \
    -D clippy::capsule_unaligned_violation \
    -D clippy::capsule_non_atomic_field \
    -D clippy::capsule_missing_generation \
    -W clippy::missing_capsule_verification \
    -W clippy::capsule_scattered_atomics \
    -W clippy::capsule_incorrect_padding \
    -W clippy::capsule_memory_ordering
```

## Troubleshooting

### False Positives

If lint incorrectly flags valid code:

1. **Verify Chaos compliance**: Check if code actually violates mandate
2. **Add suppression with justification**:
   ```rust
   #[allow(clippy::capsule_scattered_atomics)] // Justification: DualAtomicU64 pattern
   ```
3. **Report issue**: File bug with minimal reproduction case

### Performance Impact

If builds slow down:

1. **Measure overhead**: Compare with/without lints
2. **Enable caching**: CI cache `~/.cargo` and `target/`
3. **Incremental builds**: Use `cargo clippy` (not `cargo clean`)
4. **Parallel jobs**: `cargo clippy --jobs $(nproc)`

### Nightly Dependency

Lints require nightly Rust (rustc_private):

```bash
# Install nightly
rustup toolchain install nightly

# Use nightly for lints only (stable for builds)
cargo +nightly clippy --all-features
cargo build --release  # Uses stable
```

## Best Practices

1. **Start with P0 Critical**: Deny level in CI/CD immediately
2. **Enable P1 as Warnings**: Non-blocking, gradual adoption
3. **Cache CI dependencies**: Speed up builds (30-60s → 5-10s)
4. **Pre-commit hooks**: Instant feedback (save CI minutes)
5. **Report generation**: Track violations over time
6. **Suppression audit**: Review #[allow] attributes quarterly

## Support

- **Documentation**: `/home/samuel/Primitives/clippy-capsule-verify/README.md`
- **Issues**: File bug reports with minimal reproduction
- **Questions**: Consult `/home/samuel/Docs/The Computational Capsule.md`
