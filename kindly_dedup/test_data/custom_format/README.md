# Test Corpus for Custom Data Loading

## Overview

This directory contains test corpus files for validating the custom data loading functionality in kindly_dedup.

## Files

### Valid Corpus Files (10 documents each)

1. **test_corpus.jsonl** - JSON Lines format (recommended)
   - Format: One JSON object per line
   - Fields: `id` (number), `text` (string)
   - Size: 10 documents
   - Known duplicates: (0,2), (1,4), (3,8)

2. **test_corpus.json** - JSON array format
   - Format: Single JSON array containing objects
   - Fields: `id` (number), `text` (string)
   - Size: 10 documents
   - Content: Identical to test_corpus.jsonl

3. **test_corpus.txt** - Plain text format
   - Format: One document per line
   - Auto-generated IDs: doc_0, doc_1, ..., doc_9
   - Size: 10 documents
   - Content: Identical text to other formats

### Test Files (Error Handling)

4. **test_invalid.jsonl** - Malformed JSONL
   - 5 lines total: 3 valid, 2 invalid
   - Used to test error handling and recovery

5. **test_empty.txt** - Empty file
   - Zero bytes
   - Used to test empty file handling

## Known Duplicates in Test Corpus

The test corpus contains **3 exact duplicate pairs**:

1. **doc_0 = doc_2**: "Machine learning is a subset of artificial intelligence that focuses on the development of algorithms."

2. **doc_1 = doc_4**: "Deep learning neural networks have revolutionized computer vision and natural language processing tasks."

3. **doc_3 = doc_8**: "Data science combines statistics programming and domain expertise to extract insights from data."

**Unique documents**: doc_5, doc_6, doc_7, doc_9

## Usage in Tests

### Unit Tests (T28 Q1-Q7)

```rust
use kindly_dedup::custom_data::load_jsonl;

let path = PathBuf::from("test_data/custom_format/test_corpus.jsonl");
let docs = load_jsonl(&path, None).unwrap();

assert_eq!(docs.len(), 10);
```

### Integration Tests (T28 Q15-Q21)

```rust
use kindly_dedup::{custom_data::load_custom_corpus, DedupPipeline};

// Load corpus with auto-detection
let path = PathBuf::from("test_data/custom_format/test_corpus.jsonl");
let docs = load_custom_corpus(&path, None).unwrap();

// Deduplicate
let mut pipeline = DedupPipeline::new(docs.len());
for doc in &docs {
    pipeline.add_document(doc.id, &doc.text).unwrap();
}

let clusters = pipeline.find_duplicates(0.85).unwrap();

// Should find 3 clusters (one for each duplicate pair)
assert!(clusters.len() >= 3);
```

### Error Handling Tests

```rust
use kindly_dedup::custom_data::{load_jsonl, CustomDataError};

// Test file not found
let result = load_jsonl("nonexistent.jsonl", None);
assert!(matches!(result.unwrap_err(), CustomDataError::FileNotFound(_)));

// Test empty file
let result = load_jsonl("test_data/custom_format/test_empty.txt", None);
assert!(matches!(result.unwrap_err(), CustomDataError::EmptyFile(_)));

// Test invalid JSONL
let result = load_jsonl("test_data/custom_format/test_invalid.jsonl", None);
assert!(matches!(result.unwrap_err(), CustomDataError::InvalidJsonl { .. }));
```

## Test Coverage (T28 Framework)

### Q1-Q7: Unit Tests (11 tests)
- ✅ Format detection (JSONL, JSON, plain text, unknown)
- ✅ Valid file loading (all 3 formats)
- ✅ Empty file handling
- ✅ File not found handling
- ✅ Progress tracking (lockfree atomic counters)

### Q8-Q14: Property Tests (7 tests in integration suite)
- ✅ Reproducibility (same input → same output)
- ✅ Format consistency (all 3 formats load same content)
- ✅ Document count accuracy
- ✅ No data loss
- ✅ Duplicate detection accuracy
- ✅ Unique document preservation

### Q15-Q21: Integration Tests (7 tests in integration suite)
- ✅ Auto-detect and load (all formats)
- ✅ Integration with DedupPipeline
- ✅ Error recovery and graceful handling
- ✅ Empty file handling
- ✅ Mixed format detection
- ✅ Backward compatibility
- ✅ Case-insensitive extensions

### Q22-Q28: Production Tests (7 tests in integration suite)
- ✅ All formats work end-to-end
- ✅ Error messages are helpful
- ✅ Performance acceptable (<100ms for 10 docs)
- ✅ Memory efficiency
- ✅ Data integrity verification
- ✅ Concurrent loads (thread safety)
- ✅ Graceful degradation

**Total Test Coverage**: 32 comprehensive tests across all T28 tiers

## Format Specifications

### JSONL Format

```jsonl
{"id": 0, "text": "Document content"}
{"id": 1, "text": "Another document"}
```

**Requirements**:
- One JSON object per line
- Each object has `id` (number) and `text` (string) fields
- Empty lines are skipped
- Invalid lines trigger errors

### JSON Array Format

```json
[
  {"id": 0, "text": "Document content"},
  {"id": 1, "text": "Another document"}
]
```

**Requirements**:
- Valid JSON array
- Each object has `id` (number) and `text` (string) fields
- Empty arrays trigger errors

### Plain Text Format

```text
Document content on line 1
Document content on line 2
```

**Requirements**:
- One document per line
- IDs auto-generated: doc_0, doc_1, ...
- Empty lines are skipped

## Generating Additional Test Data

To create larger test corpora for performance testing:

```bash
# Generate 1K documents
for i in {0..999}; do
    echo "{\"id\": $i, \"text\": \"Test document $i\"}" >> test_1k.jsonl
done

# Generate 10K documents
for i in {0..9999}; do
    echo "{\"id\": $i, \"text\": \"Test document $i\"}" >> test_10k.jsonl
done
```

## Validation

To verify test corpus integrity:

```bash
# Count documents
wc -l test_corpus.jsonl  # Should output: 10

# Validate JSONL syntax
cat test_corpus.jsonl | jq -c '.' > /dev/null && echo "Valid JSONL"

# Validate JSON syntax
jq '.' test_corpus.json > /dev/null && echo "Valid JSON"

# Check for duplicates
cat test_corpus.txt | sort | uniq -d  # Should show 3 duplicates
```

## Framework Compliance

- **UCE34 Q1-Q7**: Simple file I/O, no capsules needed (synchronous, bounded memory)
- **ASSUM**: 100% safe (no unsafe code, bounded allocations)
- **B32**: Performance validated (<100ms for 10 docs)
- **T28**: Comprehensive 32-test suite (Q1-Q28 coverage)
- **Chaos**: Lockfree progress tracking (AtomicU64)

## References

- Main documentation: `/home/samuel/Primitives/kindly_dedup/CUSTOM_DATA_TESTING.md`
- Source code: `/home/samuel/Primitives/kindly_dedup/src/custom_data.rs`
- Integration tests: `/home/samuel/Primitives/kindly_dedup/tests/custom_data_integration_tests.rs`
