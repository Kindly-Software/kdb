//! Integration test for StreamingSignatureWriterCapsule with mmap
//!
//! Tests full mmap integration:
//! - Mmap creation and initialization
//! - Signature write/read roundtrip
//! - Crash recovery protocol
//! - Zero-copy reads

#[cfg(feature = "streaming")]
#[test]
fn test_signature_writer_mmap_integration() {
    use kindly_dedup::streaming::StreamingSignatureWriterCapsule;
    use std::fs;

    // Clean up previous test file
    let _ = fs::remove_file("/tmp/test_integration.mmap");

    // Create writer with mmap backing
    let mut writer = StreamingSignatureWriterCapsule::create("/tmp/test_integration.mmap", 100)
        .expect("Failed to create writer");

    assert_eq!(writer.position(), 0);
    assert_eq!(writer.generation(), 0);
    assert_eq!(writer.buffer_len(), 0);

    // Write 5 documents
    for i in 0..5 {
        writer
            .write_document(i, &format!("test document number {}", i))
            .expect("Failed to write document");
    }

    assert_eq!(writer.buffer_len(), 5);

    // Sync to mmap
    writer.sync().expect("Failed to sync");

    assert_eq!(writer.buffer_len(), 0);
    assert_eq!(writer.position(), 5);
    assert_eq!(writer.generation(), 2); // Incremented twice (odd then even)

    // Read back signatures (zero-copy from mmap)
    for i in 0..5 {
        let sig = writer
            .read_signature(i)
            .expect(&format!("Failed to read signature {}", i));

        // Verify signature is not default (all u16::MAX)
        assert_ne!(sig.signature(), &[u16::MAX; 128], "Signature {} is default", i);
    }

    // Clean up
    drop(writer);
    let _ = fs::remove_file("/tmp/test_integration.mmap");

    println!("✅ Mmap integration test passed!");
}

#[cfg(feature = "streaming")]
#[test]
fn test_signature_writer_crash_recovery() {
    use kindly_dedup::streaming::StreamingSignatureWriterCapsule;
    use std::fs;

    // Clean up previous test file
    let _ = fs::remove_file("/tmp/test_crash.mmap");

    // Create writer
    let mut writer = StreamingSignatureWriterCapsule::create("/tmp/test_crash.mmap", 100)
        .expect("Failed to create writer");

    // Write and sync normally
    writer.write_document(0, "test document").expect("Failed to write");
    writer.sync().expect("Failed to sync");

    assert_eq!(writer.generation() % 2, 0); // Even generation after sync

    // Simulate crash (manually set generation to odd)
    use std::sync::atomic::Ordering;
    writer.generation.store(7, Ordering::SeqCst);
    assert!(writer.detect_crash());

    // Recover
    writer.recover().expect("Failed to recover");

    assert_eq!(writer.generation() % 2, 0); // Even after recovery
    assert_eq!(writer.generation(), 6); // Rolled back from 7 to 6

    // Clean up
    drop(writer);
    let _ = fs::remove_file("/tmp/test_crash.mmap");

    println!("✅ Crash recovery test passed!");
}

#[cfg(feature = "streaming")]
#[test]
fn test_signature_writer_large_batch() {
    use kindly_dedup::streaming::StreamingSignatureWriterCapsule;
    use std::fs;

    // Clean up previous test file
    let _ = fs::remove_file("/tmp/test_large.mmap");

    // Create writer with capacity for 1000 signatures
    let mut writer = StreamingSignatureWriterCapsule::create("/tmp/test_large.mmap", 1000)
        .expect("Failed to create writer");

    // Write 100 documents
    for i in 0..100 {
        writer
            .write_document(i, &format!("large batch document {}", i))
            .expect("Failed to write document");
    }

    // Sync entire batch
    writer.sync().expect("Failed to sync");

    assert_eq!(writer.position(), 100);
    assert_eq!(writer.buffer_len(), 0);

    // Spot check a few signatures
    for i in [0, 25, 50, 75, 99].iter() {
        let sig = writer
            .read_signature(*i)
            .expect(&format!("Failed to read signature {}", i));
        assert_ne!(sig.signature(), &[u16::MAX; 128], "Signature {} is default", i);
    }

    // Clean up
    drop(writer);
    let _ = fs::remove_file("/tmp/test_large.mmap");

    println!("✅ Large batch test passed!");
}
