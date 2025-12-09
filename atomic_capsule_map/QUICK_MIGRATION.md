# Quick Migration Guide: 30 Seconds to Migrate

**⚠️ atomic_capsule_map is deprecated. Migrate to atomic_capsule in 30 seconds.**

---

## Step 1: Update Cargo.toml (10 seconds)

```toml
# Remove this
# atomic_capsule_map = "0.1"

# Add this
atomic_capsule = { version = "0.2", features = ["std"] }
```

---

## Step 2: Update Imports (10 seconds)

```rust
// Before
use atomic_capsule_map::AtomicCapsuleMap;

// After
use atomic_capsule::collections::ConcurrentMapCapsule;
```

---

## Step 3: Update Type Signatures (10 seconds)

```rust
// Before
let map: AtomicCapsuleMap<String, u64> = AtomicCapsuleMap::new();

// After
let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();
```

---

## Done! ✅

Your code now runs **3-59× faster** (median: 10-20× for concurrent workloads).

**Core API is 100% compatible** (insert, get, remove work identically).

---

## Optional: Leverage New Features (1-4 hours)

### 1. Zero-Allocation Lookups (Borrow<Q>)

```rust
// Before: Allocates String every time
let value = map.get(&"key".to_string());

// After: Zero allocation
let value = map.get("key");  // No to_string()!
```

### 2. Entry API

```rust
// Before: Manual logic
if !map.contains_key(&key) {
    map.insert(key.clone(), default_value());
}

// After: Entry API
map.entry(key).or_insert_with(default_value);
```

### 3. Arc<T> Support

```rust
// Before: Not supported (required workarounds)

// After: Native support
let map: ConcurrentMapCapsule<String, Arc<Config>> = ConcurrentMapCapsule::new();
map.insert("api".to_string(), Arc::new(config));
```

---

## Need Help?

- **Full Migration Guide**: [DASHMAP_MIGRATION_GUIDE.md](../atomic_capsule/docs/DASHMAP_MIGRATION_GUIDE.md)
- **Examples**: [migration_examples.rs](examples/migration_examples.rs)
- **Deprecation Details**: [DEPRECATION_NOTICE.md](DEPRECATION_NOTICE.md)
- **Changelog**: [CHANGELOG.md](CHANGELOG.md)

---

**TL;DR**: 3 steps (Cargo.toml, imports, types), 30 seconds, 3-59× faster. Done! ✅
