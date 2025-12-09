# Capsule Verification Migrator

Automated tool for migrating manual verification macros to `#[derive(ComputationalCapsule)]`.

## Features

- ✅ **Automatic detection** of verification patterns
- ✅ **Dry-run mode** for safe preview
- ✅ **Backup creation** before modifications  
- ✅ **Priority-based migration** (P0, P1, P2)
- ✅ **Generic type support** (alignment-only)
- ✅ **Detailed migration report**

## Installation

```bash
cd tools/capsule_verification_migrator
cargo build --release
```

## Usage

### Dry Run (Preview Changes)
```bash
# Preview all changes without applying
cargo run --release -- ../../src --verbose

# Preview only P2 (infrastructure) capsules
cargo run --release -- ../../src --priority P2
```

### Apply Migrations
```bash
# Apply all migrations with backups
cargo run --release -- ../../src --apply

# Apply without backups (dangerous!)
cargo run --release -- ../../src --apply --no-backup

# Apply only P2 priority
cargo run --release -- ../../src --apply --priority P2
```

## Migration Patterns

### Pattern 1: verify_capsule_properties!
```rust
// BEFORE
struct MyCapsule { ... }
verify_capsule_properties!(MyCapsule, 128, 128);

// AFTER
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
struct MyCapsule { ... }
```

### Pattern 2: verify_alignment_only!
```rust
// BEFORE
struct GenericCapsule<T> { ... }
verify_alignment_only!(GenericCapsule<T>, 64);

// AFTER
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]  // No size for generics
struct GenericCapsule<T> { ... }
```

### Pattern 3: Manual assert_eq!
```rust
// BEFORE
struct ManualCapsule { ... }
assert_eq!(std::mem::size_of::<ManualCapsule>(), 256);
assert_eq!(std::mem::align_of::<ManualCapsule>(), 256);

// AFTER  
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
struct ManualCapsule { ... }
```

## Priority Classifications

### P0: Production Critical (Manual Review Required)
- Core coordination primitives (DualAtomicU64, etc.)
- Protection system capsules (TRADE SECRET)
- Foundation collections

**Recommendation**: Migrate manually with careful review

### P1: Active Development (Semi-Automated Safe)
- CNLS quantum wave mechanics
- HTTP system capsules
- B-tree hybrid structures
- SIMD primitives

**Recommendation**: Use tool with manual verification

### P2: Infrastructure (Fully Automated)
- Persistent storage capsules
- Composite structures
- Simple fixed-size capsules

**Recommendation**: Safe for automated migration

## Output Example

```
╔════════════════════════════════════════════════════════════╗
║     COMPUTATIONAL CAPSULE VERIFICATION MIGRATOR v0.1.0     ║
╚════════════════════════════════════════════════════════════╝

Mode: DRY RUN MODE
Path: ../../src

Checking src/collections/concurrent_map.rs
Checking src/protection/orchestrator.rs
Checking src/http/state.rs

════════════════════════════════════════════════════════════════
MIGRATION SUMMARY
════════════════════════════════════════════════════════════════

✅ 48 Migrated successfully
   → HttpStateCapsule (src/http/state.rs:266)
   → ProtectionOrchestratorCapsule (src/protection/orchestrator.rs:89)
   → MmapRegion (src/mmap/mod.rs:45)
   ...

⚠️  5 Skipped
   → CNLSRuleCapsule - Already has #[derive(ComputationalCapsule)]
   → ComplexCell - Already has #[derive(ComputationalCapsule)]
   ...

❌ 0 Errors

════════════════════════════════════════════════════════════════

ℹ Run with --apply to apply these changes
```

## Edge Cases Handled

### 1. Already Migrated
Capsules with existing `#[derive(ComputationalCapsule)]` are skipped.

### 2. Generic Types
Generic capsules automatically get alignment-only verification:
```rust
#[capsule(alignment = 128)]  // No size specified
```

### 3. Multiple Verification
Removes all duplicate verification (macro + assert):
```rust
// Removes both:
verify_capsule_properties!(Capsule, 64, 64);
assert_eq!(size_of::<Capsule>(), 64);
```

### 4. Module Paths
Handles both local and qualified macro calls:
```rust
crate::verify_capsule_properties!(Capsule, 64, 64);
crate::verification::verify_capsule_properties!(Capsule, 64, 64);
```

## Safety Features

1. **Dry run by default** - No changes without `--apply`
2. **Automatic backups** - `.rs.backup` files created
3. **Validation** - Checks alignment == size when both specified
4. **Error reporting** - Detailed errors for failed migrations
5. **Rollback** - Backups allow easy reversion

## Limitations

1. **Comments**: May not preserve all comment positions
2. **Formatting**: Use `cargo fmt` after migration
3. **Complex macros**: Custom verification patterns need manual migration
4. **Cross-file**: Each file processed independently

## Troubleshooting

### "Failed to parse file"
- File may have syntax errors
- Check with `cargo check` first

### "No candidates found"
- Struct may not have `#[repr(C, align(...))]`
- Already migrated capsules are skipped

### "Failed to apply migration"
- Complex macro usage may need manual intervention
- Check the specific file manually

## Testing

```bash
# Run tool tests
cargo test

# Verify migrations don't break compilation
cargo run --release -- ../../src --apply
cd ../..
cargo test --all-features --lib
```

## Rollback

If migrations cause issues:
```bash
# Restore from backups
find src -name "*.rs.backup" -exec sh -c 'mv "$0" "${0%.backup}"' {} \;

# Or use git
git checkout -- src/
```

## Performance

- Processes ~100 files/second
- Memory usage: <50MB for entire atomic_capsule codebase
- Creates backups at ~1GB/s (SSD)

## Contributing

The tool is designed for the v0.4.0 → v0.7.0 migration and will be deprecated afterward.
For improvements, update `src/main.rs` and add tests.
