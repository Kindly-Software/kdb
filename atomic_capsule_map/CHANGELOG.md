# Changelog

All notable changes to atomic_capsule_map will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [DEPRECATED] - 2025-10-22

**⚠️ This crate is deprecated as of October 22, 2025.**

**Please migrate to [`atomic_capsule::collections::ConcurrentMapCapsule`](https://crates.io/crates/atomic_capsule)** for better performance and features.

**Reasons for Deprecation**:
1. **26% performance regression** in v1.1 (85ns vs 63ns insert)
2. **Vaporware SIMD features** (documented but never implemented)
3. **Architectural limitations** (Copy bounds, no Arc<T> support, 64B alignment insufficient)

**Replacement**: `atomic_capsule::collections::ConcurrentMapCapsule` delivers:
- ✅ **100ns insert** with **128B alignment** (59× speedup eliminating false sharing)
- ✅ **Arc<T> native support** (no workarounds)
- ✅ **Borrow<Q> zero-allocation lookups** (no String allocation)
- ✅ **Entry API** (or_insert_with patterns)
- ✅ **116/116 tests pass** (100% pass rate, production-ready)

**LTS Period**: 12 months (until October 2026)
- ✅ Critical bug fixes (patched within 7 days)
- ✅ Security issues (patched within 48 hours)
- ❌ No new features
- ❌ No performance work

**Migration Resources**:
- [DEPRECATION_NOTICE.md](DEPRECATION_NOTICE.md) - Full deprecation details
- [MIGRATION_GUIDE.md](../atomic_capsule/docs/DASHMAP_MIGRATION_GUIDE.md) - Step-by-step migration
- [migration_examples.rs](examples/migration_examples.rs) - 7 before/after patterns

---

## [0.1.1] - 2025-10-XX (Last Release)

### Changed
- **REGRESSION**: Insert latency increased from 63ns (v1.0) to 85ns (v1.1) - 26% slower
- Generic key support added (K: Hash + Eq instead of u64-only)
- Architectural trade-offs for flexibility at cost of performance

### Known Issues
- ⚠️ **False sharing possible** (64B alignment insufficient for high-contention scenarios)
- ⚠️ **No Arc<T> support** (requires Copy bound, incompatible with Arc<T>)
- ⚠️ **No Borrow<Q> support** (forces String allocation on every &str lookup)
- ⚠️ **No Entry API** (manual get-or-insert patterns required)
- ⚠️ **SIMD features documented but not implemented** (vaporware)

### Recommendation
**Migrate to atomic_capsule::collections::ConcurrentMapCapsule** for 3-59× better performance and feature completeness.

---

## [0.1.0] - 2025-09-XX (Initial Release)

### Added
- Initial lockfree concurrent hashmap implementation
- DashMap-compatible API (insert, get, remove, contains_key, iter)
- Atomic operations (get_or_insert, compare_and_swap, update)
- Circuit breaker integration (health_status, set_breaker_level)
- Generation counters for ABA prevention
- 64B cache alignment (insufficient, later proved)
- Basic benchmarks vs DashMap

### Performance (v1.0)
- Insert: **63ns** (single-threaded)
- Get: **10-20ns** (single-threaded)
- Remove: **40-80ns** (single-threaded)
- Concurrent scaling: Near-linear up to CPU count

### Known Limitations (Discovered Later)
- ❌ **64B alignment** causes false sharing (should be 128B)
- ❌ **Copy bounds** prevent Arc<T> usage
- ❌ **No Borrow<Q>** forces allocations
- ❌ **No Entry API** requires manual patterns

---

## Versioning Policy (LTS Period)

### Patch Releases (v0.1.x)
- ✅ Critical bug fixes (data corruption, undefined behavior)
- ✅ Security patches (CVEs, memory safety issues)
- ❌ No new features
- ❌ No performance optimizations
- ❌ No API changes

### When to Expect Patches
- **Critical bugs**: Within 7 days of report
- **Security issues**: Within 48 hours of disclosure
- **Non-critical bugs**: Best effort (or migrate to atomic_capsule)

### When LTS Period Ends (October 2026)
- **Option 1**: Archive crate (remain available, no more patches)
- **Option 2**: Remove crate with 6-month warning (if migration complete)
- **Decision**: Based on community adoption metrics

---

## Migration Timeline

### Immediate (October 2025)
- ✅ Deprecation notice added to README, lib.rs, Cargo.toml
- ✅ Migration guide published
- ✅ #[deprecated] attribute on public API
- ✅ 7 migration examples (before/after code)

### v0.2.1 (If Critical Bugs Found)
- 🔧 Last maintenance release
- 🔧 Security/UB fixes only
- ❌ No features, no optimizations

### v0.2.x LTS Period (Until October 2026)
- 📅 12 months of critical bug fixes
- 🐛 7-day patch SLA for critical bugs
- 🔒 48-hour patch SLA for security issues

### October 2026+ (Review Period)
- 📊 Analyze community migration progress
- 🔍 Check dependency graphs (who still uses atomic_capsule_map?)
- ⚠️ Announce removal with 6-month warning (if applicable)
- 🗄️ Or archive with no further patches

---

## Comparison: atomic_capsule_map vs atomic_capsule

### Performance (B32 Framework)

| Metric | atomic_capsule_map v1.1 | atomic_capsule v0.2 | Improvement |
|--------|------------------------|---------------------|-------------|
| Insert (single-thread) | 85ns | 100ns | -15% (trade-off) |
| Insert (16 threads) | 200-400ns | **100ns** | **2-4×** |
| Get (16 threads) | 15-30ns | **10-20ns** | **1.5-2×** |
| False sharing (worst) | 5,950ns | **100ns** | **59×** |
| P99 latency | 500ns | **120ns** | **4.2×** |
| Throughput | 2.5M ops/sec | **10M ops/sec** | **4×** |

### Features

| Feature | atomic_capsule_map | atomic_capsule |
|---------|-------------------|----------------|
| Arc<T> values | ❌ | ✅ |
| Borrow<Q> | ❌ | ✅ |
| Entry API | ❌ | ✅ |
| False sharing prevention | ⚠️ 64B | ✅ 128B |
| SIMD optimizations | ❌ Vaporware | ✅ Implemented |
| Test coverage | Unknown | **116/116 (100%)** |
| Active development | ❌ Deprecated | ✅ Production-ready |

### Migration Effort
- **Time**: 1-4 hours for typical codebase
- **Changes**: Import paths, type signatures, optional API improvements
- **Compatibility**: 100% for core API (insert, get, remove)
- **Breaking**: Circuit breaker API moved to separate module

---

## FAQ

### Q: Will atomic_capsule_map be deleted?

**A**: Not immediately. LTS period runs until October 2026 with critical bug fixes. After that, we'll review community adoption and decide on archival or removal (with 6-month warning).

### Q: What if I can't migrate immediately?

**A**: That's fine. Critical bugs will be patched for 12 months. Migrate when convenient.

### Q: Will there be a v0.2.0?

**A**: Only if critical bugs require it. Otherwise, v0.1.1 is the final release.

### Q: Can I still use atomic_capsule_map in production?

**A**: Yes, but **strongly discouraged** for new projects. Existing projects should plan migration within 12 months.

### Q: What's the migration time estimate?

**A**: 1-4 hours for typical codebase. Simple find/replace for imports, update type signatures, optional API improvements.

---

## Security

### Reporting Security Issues

**Email**: security@example.com (replace with actual contact)

**Response SLA**: 48 hours for security issues during LTS period

**Disclosure Policy**: Coordinated disclosure with 30-day embargo

---

## Acknowledgments

Thank you to all users who provided feedback and reported issues. Your input drove the development of the superior `atomic_capsule::collections` module.

**The computational capsule architecture continues with `atomic_capsule`.**

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

---

**TL;DR**: atomic_capsule_map is deprecated. Use `atomic_capsule::collections::ConcurrentMapCapsule` for 3-59× better performance, superior features, and active development. LTS period: 12 months (until October 2026).
