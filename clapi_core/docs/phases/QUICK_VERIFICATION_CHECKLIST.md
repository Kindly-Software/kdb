# Quick Verification Checklist

**Last Updated**: 2025-10-18
**Purpose**: Fast capsule verification workflow for clapi_core

---

## 1-Minute Verification

### Step 1: Scan for Capsules
```bash
# Check for capsule structures
grep -r "#\[repr(C, align(" clapi_core/src/
```

**Expected**: Only existing verified capsules (BudgetSlotCapsule, CircuitBreakerCapsule, etc.)

### Step 2: Clippy Lint
```bash
cargo clippy --manifest-path clapi_core/Cargo.toml \
    --all-features \
    -- -D clippy::missing_capsule_verification
```

**Expected**: Zero warnings

### Step 3: Classification
| Module Type | Verification Needed? | Example |
|-------------|---------------------|---------|
| Pure functions | ❌ NO | `client::const_hash` |
| Computational capsules | ✅ YES | `capsules::request_capsule128_enhanced` |
| HTTP handlers | ❌ NO | `proxy::server` |
| Config structs | ❌ NO | `proxy::config` |

---

## Decision Tree

```
New code added?
    │
    ├─→ Pure functions only?
    │       └─→ ✅ PASS (no verification needed)
    │
    ├─→ Contains #[repr(C, align(N))]?
    │       │
    │       ├─→ Has #[derive(ComputationalCapsule)]?
    │       │       └─→ ✅ PASS (automatic verification)
    │       │
    │       └─→ Missing derive?
    │               └─→ ❌ FAIL (add #[derive(ComputationalCapsule)])
    │
    └─→ Config/HTTP/Other?
            └─→ ✅ PASS (no verification needed)
```

---

## Automatic Verification (v0.4.0+)

### Recommended: Use Derive Macro

```rust
use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MyCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
```

**Benefits**:
- ✅ Zero manual work
- ✅ Compile-time safety (<20ms overhead)
- ✅ Zero runtime cost (0ns)
- ✅ Clear error messages

---

## Clippy Safety Net

Enable in crate root:
```rust
#![warn(clippy::missing_capsule_verification)]
```

Enforce in CI/CD:
```bash
cargo clippy --all-features -- -D clippy::missing_capsule_verification
```

**Detection Rate**: ~95% (module-level)

---

## Quick Tests

### Test 1: Build Check
```bash
cargo build --manifest-path clapi_core/Cargo.toml
```

### Test 2: Clippy Check
```bash
cargo clippy --manifest-path clapi_core/Cargo.toml --all-features
```

### Test 3: Unit Tests
```bash
cargo test --manifest-path clapi_core/Cargo.toml --lib
```

### Test 4: Miri (UB Check)
```bash
cargo +nightly miri test --manifest-path clapi_core/Cargo.toml
```

---

## Common Patterns

### Pattern 1: Pure Functions (NO verification)
```rust
// clapi_core/src/client/const_hash.rs
pub fn hash_for_budget_id(budget_id: &str) -> u64 {
    const_fast_hash(budget_id.as_bytes())
}
```
**Classification**: Tier 7 (Const), pure functions
**Verification**: ❌ Not needed

### Pattern 2: Computational Capsule (YES verification)
```rust
// clapi_core/src/capsules/request_capsule128_enhanced.rs
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct RequestCapsule128Enhanced {
    pub budget_hash: AtomicU64,
    pub amount_cents: AtomicI64,
    // ...
}
```
**Classification**: Tier 1 (Atomic)
**Verification**: ✅ Automatic (derive macro)

### Pattern 3: HTTP Types (NO verification)
```rust
// clapi_core/src/proxy/types.rs
#[derive(Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    // ...
}
```
**Classification**: Plain Rust struct
**Verification**: ❌ Not needed

---

## Framework Compliance (30-Second Check)

| Framework | Command | Expected |
|-----------|---------|----------|
| **UCE34 Q33** | Manual inspection | Zero unverified capsules |
| **ASSUM** | `grep -r "unsafe" src/` | All tagged with #ASSUME |
| **Clippy** | `cargo clippy --all-features` | Zero warnings |
| **T28** | `cargo test` | 100% pass |

---

## Emergency Contacts

**Infrastructure Issue** (disk corruption, build failures):
- Check: `/home/samuel/Primitives/clapi_core/VERIFICATION_REPORT_CLIENT_MODULE.md` § 7
- Solution: Filesystem check, build dir migration to tmpfs

**Code Issue** (unverified capsule):
- Add: `#[derive(ComputationalCapsule)]`
- Docs: `/home/samuel/Primitives/atomic_capsule_derive/README.md`

**Framework Question**:
- UCE34: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE34_FRAMEWORK.md`
- Verification: `/home/samuel/Primitives/atomic_capsule_derive/README.md`

---

## Production Deployment Checklist

Before merging to main:

- [ ] Zero clippy warnings (`cargo clippy --all-features -- -D warnings`)
- [ ] All tests pass (`cargo test --all-features`)
- [ ] No unverified capsules (`cargo clippy -- -D clippy::missing_capsule_verification`)
- [ ] Documentation updated (CLAUDE.md, module docs)
- [ ] Miri passes (`cargo +nightly miri test`)

**Time Estimate**: 2-5 minutes (assuming clean build environment)

---

## Notes

- **Pure functions**: Never need verification (no state, no capsules)
- **Derive macro**: Preferred method (zero manual work, <20ms compile overhead)
- **Clippy lint**: Safety net (catches ~95% of unverified capsules)
- **Infrastructure**: Filesystem corruption unrelated to code quality

**Last Verification**: 2025-10-18 (client module: zero capsules, zero verification needed)
