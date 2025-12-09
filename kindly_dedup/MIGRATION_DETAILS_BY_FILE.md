# CapsuleSerialize Test Migration - Detailed File-by-File Guide

## File-by-File Migration Details

### 1. tests/audit_unit_tests.rs (20 occurrences)

**Priority**: HIGH (Core audit tests)

**Changes required**:

```rust
// Line 82: Serialization
- let json = serde_json::to_string(&entry).unwrap();
+ let json = entry.to_json()?;

// Line 94: Serialization
- let json = serde_json::to_string(&original).unwrap();
+ let json = original.to_json()?;

// Line 97: Deserialization
- let deserialized: BenchmarkAuditEntry = serde_json::from_str(&json).unwrap();
+ let deserialized = BenchmarkAuditEntry::from_json(&json)?;

// Line 151: Serialization
- let json = serde_json::to_string(&env).unwrap();
+ let json = env.to_json()?;

// Line 173: Serialization
- let json = serde_json::to_string(&result).unwrap();
+ let json = result.to_json()?;

// Line 174: Deserialization
- let deserialized: BenchmarkResult = serde_json::from_str(&json).unwrap();
+ let deserialized = BenchmarkResult::from_json(&json)?;

// Line 187: Serialization
- let json = serde_json::to_string(&entry).unwrap();
+ let json = entry.to_json()?;

// Line 188: Deserialization
- let deserialized: BenchmarkAuditEntry = serde_json::from_str(&json).unwrap();
+ let deserialized = BenchmarkAuditEntry::from_json(&json)?;

// Line 220: Serialization
- let json = serde_json::to_string(&result).unwrap();
+ let json = result.to_json()?;

// Line 221: Deserialization
- let deserialized: BenchmarkResult = serde_json::from_str(&json).unwrap();
+ let deserialized = BenchmarkResult::from_json(&json)?;

// Line 237: Serialization
- let json = serde_json::to_string(&entry).unwrap();
+ let json = entry.to_json()?;

// Line 338: Serialization
- let json = serde_json::to_string(&config).unwrap();
+ let json = config.to_json()?;

// Line 339: Deserialization
- let deserialized: BenchmarkConfig = serde_json::from_str(&json).unwrap();
+ let deserialized = BenchmarkConfig::from_json(&json)?;

// Line 365: Serialization
- let json = serde_json::to_string(&result).unwrap();
+ let json = result.to_json()?;

// Line 366: Deserialization
- let deserialized: BenchmarkResult = serde_json::from_str(&json).unwrap();
+ let deserialized = BenchmarkResult::from_json(&json)?;

// Line 415: Serialization
- let json_some = serde_json::to_string(&result_with_accuracy).unwrap();
+ let json_some = result_with_accuracy.to_json()?;

// Line 416: Serialization
- let json_none = serde_json::to_string(&result_without_accuracy).unwrap();
+ let json_none = result_without_accuracy.to_json()?;

// Line 503: Serialization
- let _ = serde_json::to_string(&entry).unwrap();
+ let _ = entry.to_json()?;

// Line 581: Serialization to bytes
- let config_bytes = serde_json::to_vec(config).unwrap();
+ let config_bytes = config.serialize_deterministic().into();

// Line 589: Serialization to bytes
- let result_bytes = serde_json::to_vec(result).unwrap();
+ let result_bytes = result.serialize_deterministic().into();
```

**Assertion Updates**:
- Line 86: `.contains("benchmark_id")` → `.contains("\"benchmark_id\"")`
- Line 87: `.contains("rustc_version")` → `.contains("\"rustc_version\"")`
- Line 154: `.contains("\"feature_flags\":[]")` - Already correct format

**Estimated Time**: 8 minutes

---

### 2. tests/audit_property_tests.rs (12 occurrences)

**Priority**: HIGH (Property-based tests for invariants)

**Changes required**:

```rust
// Line 52: Deserialization
- let logged_entry: BenchmarkAuditEntry = serde_json::from_str(last_line).unwrap();
+ let logged_entry = BenchmarkAuditEntry::from_json(last_line)?;

// Line 71: Serialization
- let json1 = serde_json::to_string(&entry1).unwrap();
+ let json1 = entry1.to_json()?;

// Line 72: Serialization
- let json2 = serde_json::to_string(&entry2).unwrap();
+ let json2 = entry2.to_json()?;

// Line 232: Serialization
- let json = serde_json::to_string(&entry).unwrap();
+ let json = entry.to_json()?;

// Line 233: Deserialization
- let deserialized: BenchmarkResult = serde_json::from_str(&json).unwrap();
+ let deserialized = BenchmarkResult::from_json(&json)?;

// Line 250: Serialization
- let json = serde_json::to_string(&config).unwrap();
+ let json = config.to_json()?;

// Line 251: Deserialization
- let deserialized: BenchmarkConfig = serde_json::from_str(&json).unwrap();
+ let deserialized = BenchmarkConfig::from_json(&json)?;

// Line 270: Serialization
- let json = serde_json::to_string(&config).unwrap();
+ let json = config.to_json()?;

// Line 271: Deserialization
- let deserialized: BenchmarkConfig = serde_json::from_str(&json).unwrap();
+ let deserialized = BenchmarkConfig::from_json(&json)?;

// Line 307: Deserialization
- let mut entry: BenchmarkAuditEntry = serde_json::from_str(&tampered_lines[5]).unwrap();
+ let mut entry = BenchmarkAuditEntry::from_json(&tampered_lines[5])?;

// Line 309: Serialization
- tampered_lines[5] = serde_json::to_string(&entry).unwrap();
+ tampered_lines[5] = entry.to_json()?;

// Line 350: Deserialization
- let result: Result<BenchmarkAuditEntry, _> = serde_json::from_str(line);
+ let result = BenchmarkAuditEntry::from_json(line);
```

**Estimated Time**: 6 minutes

---

### 3. tests/audit_integration_tests.rs (6 occurrences)

**Priority**: HIGH (Integration tests for logging)

**Changes required**:

```rust
// Line 103: Deserialization
- let logged_entry: BenchmarkAuditEntry = serde_json::from_str(last_line).unwrap();
+ let logged_entry = BenchmarkAuditEntry::from_json(last_line)?;

// Line 122: Deserialization
- let logged_entry: BenchmarkAuditEntry = serde_json::from_str(content.trim()).unwrap();
+ let logged_entry = BenchmarkAuditEntry::from_json(content.trim())?;

// Line 173: Deserialization
- let mut entry: BenchmarkAuditEntry = serde_json::from_str(lines[1]).unwrap();
+ let mut entry = BenchmarkAuditEntry::from_json(lines[1])?;

// Line 175: Serialization
- tampered.push_str(&serde_json::to_string(&entry).unwrap());
+ tampered.push_str(&entry.to_json()?);

// Line 209: Deserialization
- let logged: BenchmarkAuditEntry = serde_json::from_str(last_line).unwrap();
+ let logged = BenchmarkAuditEntry::from_json(last_line)?;

// Line 350: Deserialization with map
- .map(|line| serde_json::from_str(line).unwrap())
+ .map(|line| BenchmarkAuditEntry::from_json(line))
```

**Estimated Time**: 4 minutes

---

### 4. tests/audit_production_tests.rs (4 occurrences)

**Priority**: HIGH (Production-scale validation)

**Changes required**:

```rust
// Line 190: Deserialization
- let mut entry: BenchmarkAuditEntry = serde_json::from_str(&tampered_lines[i]).unwrap();
+ let mut entry = BenchmarkAuditEntry::from_json(&tampered_lines[i])?;

// Line 192: Serialization
- tampered_lines[i] = serde_json::to_string(&entry).unwrap();
+ tampered_lines[i] = entry.to_json()?;

// Line 258: Deserialization (error handling)
- let entry: Result<BenchmarkAuditEntry, _> = serde_json::from_str(line);
+ let entry = BenchmarkAuditEntry::from_json(line);

// Line 400: Deserialization (error handling)
- let result: Result<BenchmarkAuditEntry, _> = serde_json::from_str(line);
+ let result = BenchmarkAuditEntry::from_json(line);
```

**Estimated Time**: 2 minutes

---

### 5. tests/b32_runner_tests.rs (2 occurrences)

**Priority**: MEDIUM (B32 framework validation)

**Changes required**:

```rust
// Line 311: Serialization
- let json = serde_json::to_string(&audit_env).unwrap();
+ let json = audit_env.to_json()?;

// Line 315: Deserialization
- let deserialized: EnvironmentInfo = serde_json::from_str(&json).unwrap();
+ let deserialized = EnvironmentInfo::from_json(&json)?;
```

**Estimated Time**: 1 minute

---

### 6. tests/dataset_manager_tests.rs (2 occurrences)

**Priority**: MEDIUM (Dataset management)

**Changes required**:

```rust
// Line 213: Import
- use serde_json;
+ // Remove: will use CapsuleSerialize instead

// Line 229: Serialization
- let json = serde_json::to_string(&manifest).unwrap();
+ let json = manifest.to_json()?;

// Line 230: Deserialization
- let parsed: TestManifest = serde_json::from_str(&json).unwrap();
+ let parsed = TestManifest::from_json(&json)?;
```

**Note**: `TestManifest` type must also have `#[derive(CapsuleSerialize)]` added

**Estimated Time**: 1 minute

---

### 7. tests/integration_tests.rs (1 occurrence)

**Priority**: MEDIUM (Integration tests)

**Changes required**:

```rust
// Line 649: External JSON parsing (Document type)
- let documents: Vec<Document> = serde_json::from_reader(reader).expect("Failed to parse JSON");
+ let documents: Vec<Document> = reader.lines()
+     .map(|line| Document::from_json(&line?))
+     .collect::<Result<Vec<_>, _>>()?;
```

**Note**: This is non-CapsuleSerialize type (`Document`), may need special handling

**Estimated Time**: 2 minutes

---

### 8. tests/server_tests.rs (3 occurrences)

**Priority**: MEDIUM (Server integration)

**Changes required**:

```rust
// Line 13: Import
- use serde_json::json;
+ use atomic_capsule::serialize::CapsuleSerialize;

// Line 54: HTTP response parsing
- let body: serde_json::Value = response.json().await.unwrap();
+ // Keep serde_json for HTTP response parsing (not CapsuleSerialize scope)
+ let body: serde_json::Value = response.json().await.unwrap();

// Line 501: HTTP response parsing
- let error_body: serde_json::Value = response.json().await.unwrap();
+ // Keep serde_json for HTTP response parsing (not CapsuleSerialize scope)
+ let error_body: serde_json::Value = response.json().await.unwrap();
```

**Note**: HTTP response parsing should keep serde_json (client-side deserialization)

**Estimated Time**: 0 minutes (no changes needed for HTTP)

---

### 9. tests/v1_0_benchmark_tests.rs (5 occurrences)

**Priority**: MEDIUM (Benchmark validation)

**Changes required**:

```rust
// Line 57: JSON macro (not affected)
- let json = serde_json::json!({ ... });
+ // Keep as-is: serde_json::json! macro has no CapsuleSerialize equivalent

// Line 79: Parsing
- let doc: serde_json::Value = serde_json::from_str(&line)...
+ let doc: serde_json::Value = serde_json::from_str(&line)...  // Keep as-is (external data)

// Line 80: Error handling (keep as-is)
// Line 188: Keep as-is (external document parsing)
// Line 297: Keep as-is (external JSON data)
```

**Note**: These tests deal with external JSON data (documents), not audit types. Keep serde_json.

**Estimated Time**: 0 minutes (no changes needed)

---

### 10. tests/protection_integration_tests.rs (5 occurrences)

**Priority**: MEDIUM (Protection mechanisms)

**Changes required**:

```rust
// Line 201: Serialization of event
- let event_json = serde_json::to_string(&event);
+ let event_json = event.to_json();  // Note: may return Result

// Line 959: Serialization (assertion)
- assert!(serde_json::to_string(&event1).is_ok());
+ assert!(event1.to_json().is_ok());

// Line 962: Serialization (assertion)
- assert!(serde_json::to_string(&event2).is_ok());
+ assert!(event2.to_json().is_ok());

// Line 1136: Serialization (assertion)
- assert!(serde_json::to_string(&event).is_ok());
+ assert!(event.to_json().is_ok());

// Line 1363: Serialization (assertion)
- assert!(serde_json::to_string(&event).is_ok());
+ assert!(event.to_json().is_ok());
```

**Note**: `event` type must have `#[derive(CapsuleSerialize)]` added

**Estimated Time**: 2 minutes

---

### 11. tests/cli_unit_tests.rs (1 occurrence)

**Priority**: LOW (Comment only)

**Changes required**:

```rust
// Line 714: Comment
- // Simple serialization (actual would use serde_json or bincode)
+ // Simple serialization (actual would use CapsuleSerialize or bincode)
```

**Estimated Time**: 0.5 minutes

---

### 12. tests/demo_production_tests.rs (1 occurrence)

**Priority**: LOW (External data parsing)

**Changes required**:

```rust
// Line 464: External JSON parsing
- let parsed: serde_json::Value = serde_json::from_str(lines[0]).expect("Failed to parse JSONL");
+ // Keep as-is: Parsing external benchmark output data
```

**Estimated Time**: 0 minutes (no changes needed)

---

### 13. tests/week2_format_integration_tests.rs.skip (Not counted)

**Status**: Currently skipped with `.skip` extension

**Action**: When activated, apply same pattern as other integration tests

---

## Summary by Priority

### High Priority (Must Migrate - Core Audit Types)
1. **audit_unit_tests.rs** - 20 occurrences (8 min)
2. **audit_property_tests.rs** - 12 occurrences (6 min)
3. **audit_integration_tests.rs** - 6 occurrences (4 min)
4. **audit_production_tests.rs** - 4 occurrences (2 min)

**Subtotal**: 42 occurrences, 20 minutes

### Medium Priority (Should Migrate - Related Types)
5. **b32_runner_tests.rs** - 2 occurrences (1 min)
6. **dataset_manager_tests.rs** - 2 occurrences (1 min)
7. **integration_tests.rs** - 1 occurrence (2 min)
8. **protection_integration_tests.rs** - 5 occurrences (2 min)

**Subtotal**: 10 occurrences, 6 minutes

### Low Priority (Optional - External Data or Comments)
9. **server_tests.rs** - 3 occurrences (0 min - HTTP data)
10. **v1_0_benchmark_tests.rs** - 5 occurrences (0 min - external data)
11. **cli_unit_tests.rs** - 1 occurrence (0.5 min - comment)
12. **demo_production_tests.rs** - 1 occurrence (0 min - external data)

**Subtotal**: 10 occurrences, 0.5 minutes

---

## Total Migration Cost

**All migrations**: 52 serde_json occurrences, 26.5 minutes
- High priority: 42 occurrences, 20 minutes (must do)
- Medium priority: 10 occurrences, 6 minutes (should do)
- Low priority: 10 occurrences, 0.5 minutes (optional)

**Verification**: 5 minutes (compile, test, review)

**Total**: ~35 minutes sequential, ~15 minutes parallel

---

## Type Migration Dependencies

Before test migration can proceed, these types must have `#[derive(CapsuleSerialize)]`:

| Type | Current Module | Tests Affected | Impact |
|------|----------------|----------------|--------|
| **BenchmarkAuditEntry** | `src/benchmarking/audit_logger.rs` | Unit, Property, Integration, Production | HIGH |
| **BenchmarkConfig** | `src/benchmarking/audit_logger.rs` | Unit, Property | HIGH |
| **BenchmarkResult** | `src/benchmarking/audit_logger.rs` | Unit, Property, Production | HIGH |
| **AccuracyMetrics** | `src/benchmarking/audit_logger.rs` | Unit, Property | HIGH |
| **EnvironmentInfo** | `src/benchmarking/environment.rs` | Unit, B32 Runner | MEDIUM |
| **TestManifest** | `tests/dataset_manager_tests.rs` | Dataset Manager Tests | MEDIUM |
| **Document** | (external type) | Integration Tests | MEDIUM |
| **ProtectionEvent** | (TBD) | Protection Integration Tests | MEDIUM |

---

## Next Steps

1. **Wait for Agent 1-4** to add `#[derive(CapsuleSerialize)]` to types
2. **Run this migration** following the line-by-line changes above
3. **Test**: `cargo test --lib --all-features`
4. **Commit**: `git commit -m "[TRADE SECRET] refactor(serialize): Migrate tests to CapsuleSerialize"`
5. **Validate**: All tests pass, no new warnings

---

## Error Handling Changes

Current pattern:
```rust
let json = serde_json::to_string(&entry).unwrap();  // Panics on error
```

After migration:
```rust
let json = entry.to_json()?;  // Returns Result, propagates error
```

This changes error semantics:
- **Before**: `.unwrap()` → panic on serialization error
- **After**: `?` → propagate Result<String, SerializationError>

**Impact**: Test functions must return `Result` instead of `()` in some cases.

Affected test structure:
```rust
// Before
#[test]
fn test_audit_serialization() {
    let entry = create_test_entry("test");
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.is_empty());
}

// After
#[test]
fn test_audit_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let entry = create_test_entry("test");
    let json = entry.to_json()?;
    assert!(!json.is_empty());
    Ok(())
}
```

---

**Status**: READY TO MIGRATE (once Agent 1-4 complete type derivation)
