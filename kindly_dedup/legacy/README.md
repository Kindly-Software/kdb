# Kindly_Dedup Legacy Archive

This directory contains archived and superseded components from kindly_dedup development.

## Directory Contents

### kindly-web-archived-2025-11-10

**Status**: ARCHIVED - Test/Development Version (Not Production)

**Why Archived**:
- This is a nested test/development version of kindly-web inside the kindly_dedup project
- It was created for testing atomic_capsule integration with leptos/WebAssembly
- It is NOT the production kindly-web website
- **DO NOT USE THIS** - it will cause confusion with the actual production website
- Archived to maintain codebase clarity and avoid accidental references

**What It Contains**:
- Leptos (CSR) web frontend integration (WASM, 2MB target bundle size)
- atomic_capsule integration tests (no_std, WASM-compatible)
- Benchmark reports and compilation status reports
- Theme system (theme capsules, style builders)
- Navigation and UI state management

**Size**: 4.1 GB (mostly target/ build artifacts)

**File Structure**:
```
kindly-web-archived-2025-11-10/
├── src/
│   ├── main.rs
│   ├── components/   (sections, theme, layout)
│   ├── state/        (navigation, ui, metrics, capsules)
│   ├── utils/        (theme, style builders)
│   └── error.rs
├── benches/
│   └── THEME_BENCHMARK_REPORT.md
├── tests/
├── style/
├── Cargo.toml        (Leptos + atomic_capsule 0.4.0)
├── Cargo.lock
├── target/           (build artifacts)
├── BENCHMARK_STATUS.md
├── COMPILATION_REPORT.md
└── COMPILATION_STATUS_FINAL.md
```

## Production Website Location

**The ACTUAL production kindly-web is here**:
```
/home/samuel/Primitives/kindly-web/
```

This is the version deployed to Fly.io and running in production. Use this for all website-related work.

**What's Different**:
- Archived version: Test harness inside kindly_dedup, atomic_capsule integration experiments
- Production version: Full website at `/home/samuel/Primitives/kindly-web`, deployed and live

## Why Archived

1. **Confusion Prevention**: Nested version shares name with production website
2. **Codebase Clarity**: kindly_dedup should not have a production website inside it
3. **Reference Preservation**: Kept all code for future reference (no deletion)
4. **Workspace Cleanup**: Removed from parent workspace members to avoid conflicts

## If You Need This Code

### For Reference/Learning
Browse the archived files:
```bash
ls -la /home/samuel/Primitives/kindly_dedup/legacy/kindly-web-archived-2025-11-10/
```

### For Restoration
If you need to restore this for some reason:
```bash
mv /home/samuel/Primitives/kindly_dedup/legacy/kindly-web-archived-2025-11-10 /home/samuel/Primitives/kindly_dedup/src/kindly-web
```

### For Rebuilding
The Cargo.toml shows it requires:
- Leptos 0.8 (CSR mode)
- atomic_capsule 0.4.0 (no_std, WASM-compatible)
- wasm-bindgen
- leptos_meta, leptos_router

Note: It may not compile with current versions of atomic_capsule (now at 0.6.0).

## Archive Date

Archived: 2025-11-10

**Reason**: Cleanup during kindly_dedup phase 2.4.1 (derive macro migration)

## References

- **Production Website**: `/home/samuel/Primitives/kindly-web/`
- **Kindly_Dedup Main**: `/home/samuel/Primitives/kindly_dedup/`
- **Atomic Capsule**: `/home/samuel/Primitives/atomic_capsule/`

## Search Results

Previous references to this nested kindly-web appear in:
- Build logs and compilation reports (check_output.txt, benchmark_results.txt, etc.)
- These references are now superseded - use production version instead

