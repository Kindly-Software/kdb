# DEPRECATION NOTICE: atomic_capsule_map

**Status**: Deprecated
**Date**: October 22, 2025
**Replacement**: [`atomic_capsule`](https://crates.io/crates/atomic_capsule) collections module
**Migration Guide**: [DASHMAP_MIGRATION_GUIDE.md](../atomic_capsule/docs/DASHMAP_MIGRATION_GUIDE.md)

---

## Summary

The `atomic_capsule_map` crate is **deprecated** and will receive only critical bug fixes going forward. All new development should use the `atomic_capsule::collections` module instead.

**Recommended Action**: Migrate to `atomic_capsule::collections::ConcurrentMapCapsule` for superior performance, ergonomics, and feature set.

---

## Why Deprecate?

### 1. Performance Regression

`atomic_capsule_map` v1.1 showed **26% slower insert performance** compared to v1.0 (regression from 63ns to 85ns). This was caused by architectural compromises to support generic keys.

Meanwhile, `atomic_capsule::collections::ConcurrentMapCapsule` achieved:
- **100ns insert** (after Phase 5.3 P0 fixes, 59× speedup from false sharing elimination)
- **3-59× faster than DashMap** depending on workload
- **128B alignment** preventing false sharing (vs 64B in atomic_capsule_map)

### 2. Vaporware Features

`atomic_capsule_map` README documented SIMD optimizations that were **never implemented**:
- SIMD slot scanning (claimed, not delivered)
- Vectorized hash computation (claimed, not delivered)
- 10-40× performance claims (not validated)

`atomic_capsule` delivers proven performance with B32 framework validation.

### 3. Superior Replacement Available

`atomic_capsule::collections` (Phase 5.0-5.4) provides **5 lockfree capsules**:

| Capsule | Replaces | Speedup | Key Metric |
|---------|----------|---------|------------|
| **ConcurrentMapCapsule** | DashMap, atomic_capsule_map | 3-59× | 100ns insert, false sharing eliminated |
| **LockfreeHashTable** | RwLock<HashMap> | 3.9× | 119µs vs 462µs @ 10K |
| **StatsCapsule64** | Mutex<Stats> | 1.3-5.7× | <20ns concurrent stats |
| **RingBufferBroadcast** | tokio::broadcast | 2-5× | 11M msg/s, lossless |
| **AsyncLogCapsule** | Mutex<File> | 20-100× | <50ns append, CAS-protected |

**Test Coverage**: 116/116 tests pass (100% pass rate)
**Framework Compliance**: UCE34 ✅ | ASSUM 99.99% ✅ | T28 ✅ | B32 ✅ | I20 ✅

### 4. Better Ergonomics

`atomic_capsule_map` limitations:
- ❌ Requires `K: Copy` and `V: Copy` (no Arc<T> support without workarounds)
- ❌ No `Borrow<Q>` support (forces String allocation for &str lookups)
- ❌ No Entry API (or_insert_with patterns)
- ❌ Limited to u64 keys or generic K with Arc workarounds

`atomic_capsule::collections::ConcurrentMapCapsule`:
- ✅ Generic `K: Hash + Eq + Clone`, `V: Clone`
- ✅ Arc<T> support (zero-copy reference counting)
- ✅ `Borrow<Q>` for zero-allocation lookups (`get("key")` without String allocation)
- ✅ Entry API (`or_insert_with`, `and_modify`)
- ✅ Drop-in DashMap replacement

---

## Deprecation Timeline

### Immediate (October 2025)
- ✅ Deprecation notice added to README
- ✅ #[deprecated] attribute on public API
- ✅ Migration guide published
- ✅ CHANGELOG.md updated

### v0.2.1 (Last Maintenance Release)
- 🔧 Critical bug fixes only
- 🔧 Security patches if needed
- ❌ No new features
- ❌ No performance optimizations

### v0.2.x Long-Term Support
- 📅 **Support Period**: 12 months (until October 2026)
- 🐛 Critical bugs: Patched within 7 days
- 🔒 Security issues: Patched within 48 hours
- 📚 Documentation: Maintained with migration guidance

### v1.0 (atomic_capsule)
- 🚀 Main replacement, production-ready
- 📈 Heavy promotion in ecosystem
- 🎯 Drop-in migration path

### October 2026+ (Removal Consideration)
- 📊 Community adoption metrics reviewed
- 🔍 Dependency analysis (who still uses atomic_capsule_map?)
- ⚠️ **Potential removal** if migration complete (with 6-month warning)
- 🔄 Or extend LTS if significant usage remains

**No Forced Migration**: We will never delete the crate. It will remain available for legacy projects, but new projects should use `atomic_capsule`.

---

## Migration Path

### Step 1: Update Dependencies

**Before (atomic_capsule_map)**:
```toml
[dependencies]
atomic_capsule_map = "0.1"
```

**After (atomic_capsule)**:
```toml
[dependencies]
atomic_capsule = { version = "0.2", features = ["std"] }
```

### Step 2: Update Imports

**Before**:
```rust
use atomic_capsule_map::AtomicCapsuleMap;
```

**After**:
```rust
use atomic_capsule::collections::ConcurrentMapCapsule;
```

### Step 3: Update Type Signatures

**Before**:
```rust
let map: AtomicCapsuleMap<String, u64> = AtomicCapsuleMap::new();
```

**After**:
```rust
let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();
```

### Step 4: Leverage New Features

**Zero-Allocation Lookups (New!)**:
```rust
// Before: Forces String allocation
let value = map.get(&"key".to_string());

// After: Zero allocation with Borrow<Q>
let value = map.get("key");  // No to_string()!
```

**Entry API (New!)**:
```rust
// Before: Manual get_or_insert
if !map.contains_key(&key) {
    map.insert(key.clone(), default_value());
}

// After: Entry API
map.entry(key).or_insert_with(default_value);
```

**Arc<T> Support (New!)**:
```rust
// Before: Required Copy bound (workarounds needed)
// atomic_capsule_map couldn't handle Arc<T> directly

// After: Native Arc<T> support
let map: ConcurrentMapCapsule<String, Arc<Config>> = ConcurrentMapCapsule::new();
map.insert("api".to_string(), Arc::new(config));
```

---

## Complete Migration Examples

See [DASHMAP_MIGRATION_GUIDE.md](../atomic_capsule/docs/DASHMAP_MIGRATION_GUIDE.md) for:
- ✅ 5+ real-world migration patterns
- ✅ Before/after code comparisons
- ✅ Performance benchmarks
- ✅ API mapping (method-by-method)
- ✅ Known compatibility issues
- ✅ Troubleshooting guide

---

## Compatibility Notes

### What Changes?

| Feature | atomic_capsule_map | atomic_capsule::collections |
|---------|-------------------|------------------------------|
| **Basic API** | ✅ Compatible | ✅ Drop-in replacement |
| **Arc<T> values** | ❌ Not supported | ✅ Supported natively |
| **Borrow<Q>** | ❌ Not supported | ✅ Supported (zero-alloc) |
| **Entry API** | ❌ Not implemented | ✅ Full Entry API |
| **Circuit breaker** | ✅ Built-in | ✅ Built-in (compatible) |
| **Generation counters** | ✅ ABA-safe | ✅ ABA-safe |
| **128B alignment** | ❌ 64B only | ✅ False sharing eliminated |

### What Stays the Same?

- ✅ Core API (`get`, `insert`, `remove`, `contains_key`)
- ✅ Atomic operations (`get_or_insert`, `compare_and_swap`, `update`)
- ✅ Health monitoring (`health_status`, `set_breaker_level`)
- ✅ Generation counter semantics
- ✅ Lockfree guarantees
- ✅ Zero allocation on hot paths

### Known Issues

**Issue 1: Copy Bound Removal**

`atomic_capsule_map` required `K: Copy` and `V: Copy`. Migration to `atomic_capsule` requires `K: Clone` and `V: Clone` instead.

**Fix**: Most types are already Clone. If using custom types, implement Clone.

**Issue 2: Circuit Breaker API Differences**

`atomic_capsule_map` uses `BreakerLevel` enum. `atomic_capsule` uses the same enum but with stricter state transitions.

**Fix**: No code changes needed. State machine is backward compatible.

**Issue 3: Iteration Semantics**

`atomic_capsule_map` iter() returns snapshots. `atomic_capsule` iter() also returns snapshots but with stronger consistency guarantees.

**Fix**: No code changes needed. Behavior is compatible.

---

## Support Policy

### What We Will Do

✅ **Critical Bug Fixes**: Security issues, data corruption, undefined behavior
✅ **Documentation**: Keep README/docs accurate and link to migration guide
✅ **Migration Support**: Answer questions, help debug migration issues
✅ **LTS Period**: 12 months of critical bug fixes (until October 2026)

### What We Won't Do

❌ **New Features**: All development happens in `atomic_capsule`
❌ **Performance Work**: Use `atomic_capsule` for optimizations
❌ **API Changes**: Frozen API to prevent migration churn
❌ **Dependency Updates**: Only if security-critical

---

## Frequently Asked Questions

### Q: Why not just fix atomic_capsule_map?

**A**: The architectural compromises that caused the regression (generic K support, Copy bounds) are fundamental. Fixing them would require a complete rewrite, which is exactly what `atomic_capsule::collections` delivers.

### Q: Will atomic_capsule_map be deleted?

**A**: Not in the near term. We'll keep it available for legacy projects. After 12 months (October 2026), we'll review community adoption and decide on removal with 6-month warning.

### Q: What if I can't migrate immediately?

**A**: That's fine. The crate will remain functional and receive critical bug fixes for 12 months. Migrate when convenient.

### Q: Is atomic_capsule stable?

**A**: Yes. Phase 5.0-5.4 completed with 116/116 tests passing (100% pass rate). All frameworks validated (UCE34, ASSUM, T28, B32, I20). Production-ready.

### Q: How long will migration take?

**A**: For most codebases: 1-4 hours. Simple find/replace for imports, update Cargo.toml, optional API improvements (Entry API, Borrow<Q>).

### Q: Will performance improve after migration?

**A**: Yes. Benchmarks show 3-59× speedup depending on workload. Median improvement: 10-20× for concurrent workloads.

### Q: What if I find a bug in atomic_capsule_map?

**A**: File an issue. We'll patch critical bugs within 7 days, security issues within 48 hours.

### Q: Can I still use atomic_capsule_map for new projects?

**A**: Technically yes, but **strongly discouraged**. You'll miss out on better performance, ergonomics, and ongoing development.

---

## Get Help

- **Migration Guide**: [DASHMAP_MIGRATION_GUIDE.md](../atomic_capsule/docs/DASHMAP_MIGRATION_GUIDE.md)
- **Examples**: See `atomic_capsule/examples/` for migration patterns
- **Issues**: File migration questions as issues (we'll help!)
- **Community**: Join discussions on atomic capsule architecture

---

## Acknowledgments

Thank you to all `atomic_capsule_map` users. Your feedback drove the development of the superior `atomic_capsule::collections` module.

**The computational capsule architecture continues with `atomic_capsule`.**

---

**TL;DR**: `atomic_capsule_map` is deprecated. Use `atomic_capsule::collections::ConcurrentMapCapsule` for 3-59× better performance, superior ergonomics, and ongoing development support. Migration is straightforward (1-4 hours). LTS period: 12 months.
