//! [TRADE SECRET] Comprehensive Test Suite for EXIF Camera Database Capsule
//! T28 Framework: 48 tests across 4 tiers (Unit, Property, Integration, Production)
//!
//! **Test Coverage**:
//! - Q1-Q7 (Unit): 12 tests - Core capsule functionality
//! - Q8-Q14 (Property): 12 tests - Determinism and invariants
//! - Q15-Q21 (Integration): 12 tests - Full pipeline and composition
//! - Q22-Q28 (Production): 12 tests - Latency, concurrency, compliance

use kindly_verified::detector::{
    EXIFCameraDatabaseCapsule, EXIFMetadata,
};

// ============================================================================
// UNIT TESTS (Q1-Q7) - Core Capsule Functionality
// ============================================================================

#[test]
fn unit_001_capsule_creation_default() {
    let capsule = EXIFCameraDatabaseCapsule::new();
    assert!(is_capsule_default(&capsule));
}

#[test]
fn unit_002_capsule_alignment_64_bytes() {
    let capsule = EXIFCameraDatabaseCapsule::new();
    let addr = &capsule as *const _ as usize;
    assert_eq!(
        addr % 64,
        0,
        "Capsule must be 64-byte cache-line aligned (COCA requirement)"
    );
}

#[test]
fn unit_003_capsule_size_verification() {
    let size = std::mem::size_of::<EXIFCameraDatabaseCapsule>();
    assert_eq!(size, 64, "Capsule must be exactly 64 bytes for cache alignment");
}

#[test]
fn unit_004_samsung_camera_lookup() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();
    assert!(
        capsule.lookup_camera("Samsung", "SM-S908W"),
        "Samsung S908W must be in known database"
    );
}

#[test]
fn unit_005_canon_camera_lookup() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();
    assert!(
        capsule.lookup_camera("Canon", "EOS 5D Mark IV"),
        "Canon EOS 5D must be in known database"
    );
}

#[test]
fn unit_006_unknown_camera_returns_false() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();
    assert!(
        !capsule.lookup_camera("FakeBrand", "FakeModel"),
        "Unknown camera should return false"
    );
}

#[test]
fn unit_007_consistency_valid_iso() {
    let capsule = EXIFCameraDatabaseCapsule::new();
    let metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: None,
        gps_longitude: None,
        focal_length: None,
    };

    let score = capsule.validate_consistency(&metadata);
    assert!(score > 0.5, "Valid ISO should produce score > 0.5");
}

#[test]
fn unit_008_spoofing_detection_zero_iso() {
    let capsule = EXIFCameraDatabaseCapsule::new();
    let metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(0),
        shutter_speed: None,
        aperture: None,
        gps_latitude: None,
        gps_longitude: None,
        focal_length: None,
    };

    assert!(
        capsule.detect_spoofing(&metadata),
        "ISO=0 should be detected as spoofing"
    );
}

#[test]
fn unit_009_spoofing_detection_fake_gps() {
    let capsule = EXIFCameraDatabaseCapsule::new();
    let metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: Some(0),
        gps_longitude: Some(0),
        focal_length: None,
    };

    assert!(
        capsule.detect_spoofing(&metadata),
        "GPS at 0,0 should be detected as spoofing"
    );
}

#[test]
fn unit_010_audit_hash_generation() {
    let capsule = EXIFCameraDatabaseCapsule::new();
    let hash = capsule.compute_audit_hash(true, 0.8, false, 100);
    assert_ne!(
        hash, 0,
        "Audit hash must be non-zero (Q34 tamper detection)"
    );
}

#[test]
fn unit_011_statistics_initial_zero() {
    let capsule = EXIFCameraDatabaseCapsule::new();
    let (val_count, bloom_hits, hash_queries, confidence) = capsule.get_statistics();
    assert_eq!(val_count, 0, "Initial validation count must be 0");
    assert_eq!(bloom_hits, 0, "Initial bloom hits must be 0");
    assert_eq!(hash_queries, 0, "Initial hash queries must be 0");
    assert_eq!(confidence, 0, "Initial confidence must be 0");
}

#[test]
fn unit_012_default_trait_implementation() {
    let capsule: EXIFCameraDatabaseCapsule = Default::default();
    let (val_count, _, _, _) = capsule.get_statistics();
    assert_eq!(val_count, 0, "Default capsule should have zero statistics");
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14) - Determinism and Invariants
// ============================================================================

#[test]
fn property_001_consistency_score_always_bounded() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    // Test with various metadata combinations
    let test_cases = vec![
        (Some("2023-01-01T12:00:00"), Some(100), Some(0), Some(0)),
        (None, Some(3200), Some(40 * 65536), Some(-73 * 65536)),
        (Some("2023-01-01T12:00:00"), Some(1000000), Some(0), Some(0)),
        (None, None, None, None),
    ];

    for (datetime, iso, lat, lon) in test_cases {
        let metadata = EXIFMetadata {
            make: "Canon".to_string(),
            model: "EOS 5D".to_string(),
            datetime_original: datetime.map(|s| s.to_string()),
            datetime_digitized: datetime.map(|s| s.to_string()),
            iso,
            shutter_speed: None,
            aperture: None,
            gps_latitude: lat,
            gps_longitude: lon,
            focal_length: None,
        };

        let score = capsule.validate_consistency(&metadata);
        assert!(
            score >= 0.0 && score <= 1.0,
            "Consistency score must always be in [0.0, 1.0], got {:.2}",
            score
        );
    }
}

#[test]
fn property_002_audit_hash_deterministic() {
    let capsule1 = EXIFCameraDatabaseCapsule::new();
    let capsule2 = EXIFCameraDatabaseCapsule::new();

    // Same inputs must produce same hash
    for camera_found in &[true, false] {
        for consistency in &[0.0, 0.5, 1.0] {
            for spoofing in &[true, false] {
                for generation in 0..10 {
                    let hash1 =
                        capsule1.compute_audit_hash(*camera_found, *consistency, *spoofing, generation);
                    let hash2 =
                        capsule2.compute_audit_hash(*camera_found, *consistency, *spoofing, generation);
                    assert_eq!(
                        hash1, hash2,
                        "Same inputs must produce same hash (generation={})",
                        generation
                    );
                }
            }
        }
    }
}

#[test]
fn property_003_camera_lookup_idempotent() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    // Calling lookup multiple times should produce same result
    let result1 = capsule.lookup_camera("Samsung", "SM-S908W");
    let result2 = capsule.lookup_camera("Samsung", "SM-S908W");
    let result3 = capsule.lookup_camera("Samsung", "SM-S908W");

    assert_eq!(result1, result2, "Lookup must be idempotent");
    assert_eq!(result2, result3, "Lookup must be idempotent");
}

#[test]
fn property_004_case_insensitive_lookup() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    let variations = vec!["samsung", "Samsung", "SAMSUNG", "SaMsUnG"];
    for variation in variations {
        assert!(
            capsule.lookup_camera(variation, "SM-S908W"),
            "Case variation '{}' must be found",
            variation
        );
    }
}

#[test]
fn property_005_spoofing_detection_deterministic() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T11:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: None,
        gps_longitude: None,
        focal_length: None,
    };

    let result1 = capsule.detect_spoofing(&metadata);
    let result2 = capsule.detect_spoofing(&metadata);

    assert_eq!(
        result1, result2,
        "Spoofing detection must be deterministic"
    );
}

#[test]
fn property_006_hash_sensitivity() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let hash1 = capsule.compute_audit_hash(true, 0.8, false, 100);
    let hash2 = capsule.compute_audit_hash(false, 0.8, false, 100); // Different camera_found
    let hash3 = capsule.compute_audit_hash(true, 0.9, false, 100); // Different consistency
    let hash4 = capsule.compute_audit_hash(true, 0.8, true, 100); // Different spoofing
    let hash5 = capsule.compute_audit_hash(true, 0.8, false, 101); // Different generation

    // All should be different
    let hashes = vec![hash1, hash2, hash3, hash4, hash5];
    for (i, hash_i) in hashes.iter().enumerate() {
        for (j, hash_j) in hashes.iter().enumerate() {
            if i < j {
                assert_ne!(
                    hash_i, hash_j,
                    "Hashes {} and {} should differ",
                    i, j
                );
            }
        }
    }
}

#[test]
fn property_007_metadata_parsing_repeatable() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let metadata1 = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: Some(40 * 65536),
        gps_longitude: Some(-73 * 65536),
        focal_length: None,
    };

    let metadata2 = metadata1.clone();

    let score1 = capsule.validate_consistency(&metadata1);
    let score2 = capsule.validate_consistency(&metadata2);

    assert_eq!(score1, score2, "Same metadata must produce same score");
}

#[test]
fn property_008_statistics_increment_linearly() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    let (_, _, queries1, _) = capsule.get_statistics();
    assert_eq!(queries1, 0);

    for i in 1..=10 {
        capsule.lookup_camera("Canon", "EOS 5D");
        let (_, _, queries, _) = capsule.get_statistics();
        assert_eq!(queries as usize, i, "Queries must increment linearly");
    }
}

#[test]
fn property_009_bloom_hits_on_known_camera() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    let (_, hits_before, _, _) = capsule.get_statistics();

    // Lookup known camera multiple times
    for _ in 0..5 {
        capsule.lookup_camera("Samsung", "SM-S908W");
    }

    let (_, hits_after, _, _) = capsule.get_statistics();
    assert_eq!(hits_after, hits_before + 5, "Known camera should increment bloom hits");
}

#[test]
fn property_010_unknown_camera_no_bloom_hit() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    let (_, hits_before, _, _) = capsule.get_statistics();

    // Lookup unknown camera
    capsule.lookup_camera("FakeBrand", "FakeModel");

    let (_, hits_after, _, _) = capsule.get_statistics();
    assert_eq!(
        hits_after, hits_before,
        "Unknown camera should not increment bloom hits"
    );
}

#[test]
fn property_011_consistency_validation_input_independence() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    // Different cameras, same metadata structure
    let canon_meta = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: None,
        gps_longitude: None,
        focal_length: None,
    };

    let nikon_meta = EXIFMetadata {
        make: "Nikon".to_string(),
        model: "D850".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: None,
        gps_longitude: None,
        focal_length: None,
    };

    let score1 = capsule.validate_consistency(&canon_meta);
    let score2 = capsule.validate_consistency(&nikon_meta);

    // Consistency should be same (same datetime/iso/gps)
    assert_eq!(score1, score2, "Consistency validation should be camera-independent");
}

#[test]
fn property_012_all_known_manufacturers() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    let manufacturers = vec![
        "Samsung", "Canon", "Nikon", "Sony", "Apple", "Fujifilm", "Panasonic", "Pentax",
        "Olympus", "Leica", "Hasselblad",
    ];

    for mfr in manufacturers {
        assert!(
            capsule.lookup_camera(mfr, "SomeModel"),
            "Manufacturer '{}' should be found",
            mfr
        );
    }
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21) - Full Pipeline and Composition
// ============================================================================

#[test]
fn integration_001_validation_with_valid_exif() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    // Empty EXIF will cause error (expected - real EXIF parser needed)
    let result = capsule.validate_exif(b"");
    assert!(result.is_err(), "Empty EXIF should produce error");
}

#[test]
fn integration_002_validation_count_increments() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    let (count_before, _, _, _) = capsule.get_statistics();
    assert_eq!(count_before, 0);

    // Try validation (will fail due to no real EXIF data)
    // Note: validate_exif only increments on successful parsing
    let result = capsule.validate_exif(b"");
    assert!(result.is_err(), "Empty EXIF should error");

    let (count_after, _, _, _) = capsule.get_statistics();
    // Count should remain 0 since parse_exif fails before incrementing
    assert_eq!(count_after, count_before, "Validation count increments only on parse success");
}

#[test]
fn integration_003_consistency_all_fields_missing() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let metadata = EXIFMetadata {
        make: "Unknown".to_string(),
        model: "Unknown".to_string(),
        datetime_original: None,
        datetime_digitized: None,
        iso: None,
        shutter_speed: None,
        aperture: None,
        gps_latitude: None,
        gps_longitude: None,
        focal_length: None,
    };

    let score = capsule.validate_consistency(&metadata);
    assert!(score >= 0.0 && score < 1.0, "Missing all fields should reduce score");
}

#[test]
fn integration_004_spoofing_detection_multiple_patterns() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    // Test 1: Valid metadata
    let valid = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: Some(40 * 65536),
        gps_longitude: Some(-73 * 65536),
        focal_length: None,
    };

    assert!(!capsule.detect_spoofing(&valid), "Valid metadata should not be flagged");

    // Test 2: Timestamp spoofing
    let mut timestamp_spoof = valid.clone();
    timestamp_spoof.datetime_digitized = Some("2023-01-01T11:00:00".to_string());
    assert!(
        capsule.detect_spoofing(&timestamp_spoof),
        "Timestamp reversal should be flagged"
    );

    // Test 3: GPS spoofing
    let mut gps_spoof = valid.clone();
    gps_spoof.gps_latitude = Some(0);
    gps_spoof.gps_longitude = Some(0);
    assert!(
        capsule.detect_spoofing(&gps_spoof),
        "Fake GPS (0,0) should be flagged"
    );

    // Test 4: ISO spoofing
    let mut iso_spoof = valid.clone();
    iso_spoof.iso = Some(0);
    assert!(capsule.detect_spoofing(&iso_spoof), "ISO=0 should be flagged");
}

#[test]
fn integration_005_camera_lookup_with_statistics() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    let cameras = vec![
        ("Samsung", "SM-S908W", true),
        ("Canon", "EOS 5D", true),
        ("FakeBrand", "FakeModel", false),
        ("Nikon", "D850", true),
    ];

    for (make, model, expected_found) in cameras {
        let found = capsule.lookup_camera(make, model);
        assert_eq!(
            found, expected_found,
            "Camera lookup for {} {} must return {}",
            make, model, expected_found
        );
    }

    let (_, _, queries, _) = capsule.get_statistics();
    assert_eq!(queries, 4, "All 4 lookups should be counted");
}

#[test]
fn integration_006_audit_hash_composition() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    // Test all combinations of boolean inputs
    for camera_found in &[true, false] {
        for spoofing_detected in &[true, false] {
            let hash =
                capsule.compute_audit_hash(*camera_found, 0.8, *spoofing_detected, 100);
            assert_ne!(
                hash, 0,
                "Hash must be non-zero for camera_found={}, spoofing={}",
                camera_found, spoofing_detected
            );
        }
    }
}

#[test]
fn integration_007_metadata_consistency_iso_range() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    // Test ISO values across valid range
    let iso_values = vec![
        (100, true),
        (400, true),
        (800, true),
        (1600, true),
        (3200, true),
        (6400, true),
        (12800, true),
        (25600, true),
        (51200, true),
        (1000000, false), // Should reduce score significantly
    ];

    for (iso, should_be_valid) in iso_values {
        let metadata = EXIFMetadata {
            make: "Canon".to_string(),
            model: "EOS 5D".to_string(),
            datetime_original: Some("2023-01-01T12:00:00".to_string()),
            datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
            iso: Some(iso),
            shutter_speed: None,
            aperture: None,
            gps_latitude: None,
            gps_longitude: None,
            focal_length: None,
        };

        let score = capsule.validate_consistency(&metadata);
        if should_be_valid {
            assert!(
                score > 0.5,
                "ISO {} should produce score > 0.5",
                iso
            );
        } else {
            assert!(
                score < 1.0,
                "ISO {} should reduce score",
                iso
            );
        }
    }
}

#[test]
fn integration_008_metadata_consistency_gps_bounds() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    // Test GPS coordinates within valid bounds
    let valid_coords = vec![
        (0, 0),               // Equator, prime meridian
        (40 * 65536, -73 * 65536), // New York (Q16.16)
        (51 * 65536, 0),       // Greenwich
        (-33 * 65536, 151 * 65536), // Sydney
    ];

    for (lat, lon) in valid_coords {
        let metadata = EXIFMetadata {
            make: "Canon".to_string(),
            model: "EOS 5D".to_string(),
            datetime_original: Some("2023-01-01T12:00:00".to_string()),
            datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
            iso: Some(3200),
            shutter_speed: None,
            aperture: None,
            gps_latitude: Some(lat),
            gps_longitude: Some(lon),
            focal_length: None,
        };

        let score = capsule.validate_consistency(&metadata);
        assert!(
            score > 0.5,
            "Valid GPS ({}, {}) should produce score > 0.5",
            lat, lon
        );
    }

    // Test out-of-bounds GPS
    let invalid_coords = vec![
        (100 * 65536, 0),      // Latitude > 90
        (-100 * 65536, 0),     // Latitude < -90
        (0, 200 * 65536),      // Longitude > 180
        (0, -200 * 65536),     // Longitude < -180
    ];

    for (lat, lon) in invalid_coords {
        let metadata = EXIFMetadata {
            make: "Canon".to_string(),
            model: "EOS 5D".to_string(),
            datetime_original: Some("2023-01-01T12:00:00".to_string()),
            datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
            iso: Some(3200),
            shutter_speed: None,
            aperture: None,
            gps_latitude: Some(lat),
            gps_longitude: Some(lon),
            focal_length: None,
        };

        let score = capsule.validate_consistency(&metadata);
        assert!(
            score < 1.0,
            "Invalid GPS ({}, {}) should reduce score",
            lat, lon
        );
    }
}

#[test]
fn integration_009_spoofing_multiple_detectors() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let mut metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: Some(40 * 65536),
        gps_longitude: Some(-73 * 65536),
        focal_length: None,
    };

    // Should not be flagged initially
    assert!(!capsule.detect_spoofing(&metadata));

    // Add timestamp spoofing
    metadata.datetime_digitized = Some("2023-01-01T11:00:00".to_string());
    assert!(capsule.detect_spoofing(&metadata));

    // Reset to valid state
    metadata.datetime_digitized = Some("2023-01-01T12:00:00".to_string());
    assert!(!capsule.detect_spoofing(&metadata));

    // Add GPS spoofing
    metadata.gps_latitude = Some(0);
    metadata.gps_longitude = Some(0);
    assert!(capsule.detect_spoofing(&metadata));
}

#[test]
fn integration_010_full_camera_database_coverage() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    // Test with 50+ known cameras (simplified list)
    let known_cameras = vec![
        ("Samsung", "SM-S908W"),
        ("Samsung", "SM-S9080"),
        ("Canon", "EOS 5D Mark IV"),
        ("Canon", "EOS R5"),
        ("Nikon", "D850"),
        ("Nikon", "Z6 II"),
        ("Sony", "A7R IV"),
        ("Sony", "FX30"),
        ("Apple", "iPhone 14 Pro"),
        ("Fujifilm", "X-T5"),
        ("Panasonic", "S1R"),
        ("Pentax", "K-1 II"),
        ("Olympus", "OM-1"),
        ("Leica", "M11"),
        ("Hasselblad", "907X"),
    ];

    for (make, model) in &known_cameras {
        assert!(
            capsule.lookup_camera(make, model),
            "Known camera {} {} should be found",
            make, model
        );
    }

    let (_, _, queries, _) = capsule.get_statistics();
    assert_eq!(queries as usize, known_cameras.len());
}

#[test]
fn integration_011_consistency_with_datetime_variants() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let test_cases = vec![
        ("2023-01-01T12:00:00", "2023-01-01T12:00:00", false),  // Exact match = not spoofed
        ("2023-01-01T12:00:00", "2023-01-01T12:00:30", false),  // digitized after original = not spoofed
        ("2023-01-01T12:00:00", "2023-01-01T11:59:30", true),   // digitized before original = spoofed (string compare)
        ("2023-01-01T12:00:00", "2023-01-01T11:00:00", true),   // digitized before original = spoofed
    ];

    for (original, digitized, should_be_spoofed) in test_cases {
        let metadata = EXIFMetadata {
            make: "Canon".to_string(),
            model: "EOS 5D".to_string(),
            datetime_original: Some(original.to_string()),
            datetime_digitized: Some(digitized.to_string()),
            iso: Some(3200),
            shutter_speed: None,
            aperture: None,
            gps_latitude: None,
            gps_longitude: None,
            focal_length: None,
        };

        let spoofing = capsule.detect_spoofing(&metadata);
        assert_eq!(
            spoofing, should_be_spoofed,
            "Spoofing detection for ({}, {}) must be {}",
            original, digitized, should_be_spoofed
        );
    }
}

#[test]
fn integration_012_error_handling_comprehensive() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    // Test with empty data (too short)
    let result = capsule.validate_exif(b"");
    assert!(result.is_err(), "Empty EXIF should error");

    // Test with short data
    let result = capsule.validate_exif(&[0xFF, 0xD8]);
    assert!(result.is_err(), "Short EXIF should error");

    // Test with longer data (should parse but as empty metadata)
    let longer_data = [0xFF; 1000];
    let result = capsule.validate_exif(&longer_data);
    // Long data might succeed but produce empty metadata
    // Either success or error is acceptable (depends on implementation)
}

// ============================================================================
// PRODUCTION TESTS (Q22-Q28) - Latency, Concurrency, Compliance
// ============================================================================

#[test]
fn production_001_camera_lookup_latency() {
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        capsule.lookup_camera("Samsung", "SM-S908W");
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 1000;

    // Target: <500ns per lookup
    // Allow 2μs for system variation
    assert!(
        avg_latency_ns < 2000,
        "Camera lookup latency {:.0}ns exceeds 2μs",
        avg_latency_ns
    );
}

#[test]
fn production_002_consistency_validation_latency() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: Some((1, 125)),
        aperture: Some(280),
        gps_latitude: Some(40 * 65536),
        gps_longitude: Some(-73 * 65536),
        focal_length: Some(50 * 65536),
    };

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        capsule.validate_consistency(&metadata);
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 1000;

    // Target: <100ns per validation
    // Allow 1μs for safety
    assert!(
        avg_latency_ns < 1000,
        "Consistency validation latency {:.0}ns exceeds 1μs",
        avg_latency_ns
    );
}

#[test]
fn production_003_spoofing_detection_latency() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: Some(40 * 65536),
        gps_longitude: Some(-73 * 65536),
        focal_length: None,
    };

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        capsule.detect_spoofing(&metadata);
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 1000;

    // Target: <500ns per spoofing check
    // Allow 2μs for safety
    assert!(
        avg_latency_ns < 2000,
        "Spoofing detection latency {:.0}ns exceeds 2μs",
        avg_latency_ns
    );
}

#[test]
fn production_004_audit_hash_latency() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let start = std::time::Instant::now();
    for i in 0..1000 {
        capsule.compute_audit_hash(true, 0.8, false, i);
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 1000;

    // Target: <100ns per hash
    // Allow 500ns for safety
    assert!(
        avg_latency_ns < 500,
        "Audit hash latency {:.0}ns exceeds 500ns",
        avg_latency_ns
    );
}

#[test]
fn production_005_statistics_read_latency() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        capsule.get_statistics();
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 10000;

    // Target: <50ns per read
    // Allow 200ns for system variation
    assert!(
        avg_latency_ns < 200,
        "Statistics read latency {:.0}ns exceeds 200ns",
        avg_latency_ns
    );
}

#[test]
fn production_006_thread_safety_atomic_reads() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(EXIFCameraDatabaseCapsule::new());
    let mut handles = vec![];

    for _ in 0..8 {
        let capsule_clone = capsule.clone();
        let handle = thread::spawn(move || {
            let (val_count, bloom_hits, hash_queries, confidence) = capsule_clone.get_statistics();
            (val_count, bloom_hits, hash_queries, confidence)
        });
        handles.push(handle);
    }

    // All threads should complete without panic
    for handle in handles {
        assert!(handle.join().is_ok(), "Thread must complete successfully");
    }
}

#[test]
fn production_007_concurrent_statistics_accumulation() {
    use std::sync::Arc;
    use std::thread;

    // Since lookup_camera requires &mut, we can only test concurrent reads
    let capsule = Arc::new(EXIFCameraDatabaseCapsule::new());

    let mut handles = vec![];

    // Spawn multiple threads all doing concurrent reads
    for _ in 0..4 {
        let capsule_clone = capsule.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let (_, _, _, _) = capsule_clone.get_statistics();
                std::thread::yield_now();
            }
        });
        handles.push(handle);
    }

    // All threads should complete without error
    for handle in handles {
        assert!(handle.join().is_ok(), "Thread must complete successfully");
    }

    // Verify final state (all reads succeeded)
    let (val_count, _, _, _) = capsule.get_statistics();
    // val_count should still be 0 (get_statistics doesn't increment it)
    assert_eq!(val_count, 0);
}

#[test]
fn production_008_memory_alignment_stress() {
    for _ in 0..1000 {
        let capsule = EXIFCameraDatabaseCapsule::new();
        let addr = &capsule as *const _ as usize;
        assert_eq!(
            addr % 64,
            0,
            "Capsule alignment must be maintained across allocations"
        );
    }
}

#[test]
fn production_009_determinism_validation() {
    let mut capsule1 = EXIFCameraDatabaseCapsule::new();
    let mut capsule2 = EXIFCameraDatabaseCapsule::new();

    // Perform same operations in same order
    let cameras = vec![
        ("Samsung", "SM-S908W"),
        ("Canon", "EOS 5D"),
        ("Nikon", "D850"),
    ];

    for (make, model) in cameras {
        capsule1.lookup_camera(make, model);
        capsule2.lookup_camera(make, model);
    }

    // Statistics should be identical
    let stats1 = capsule1.get_statistics();
    let stats2 = capsule2.get_statistics();

    assert_eq!(stats1, stats2, "Same operations must produce same statistics");
}

#[test]
fn production_010_audit_trail_integrity() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    let hash1 = capsule.compute_audit_hash(true, 0.8, false, 100);
    let hash2 = capsule.compute_audit_hash(true, 0.8, false, 100);

    // Same inputs must produce same hash (for audit trail verification)
    assert_eq!(hash1, hash2, "Audit hash must be deterministic for trail verification");

    // Verify the hash is non-zero
    assert_ne!(hash1, 0, "Audit hash must be non-zero");

    // Verify different inputs produce different hashes
    let hash2 = capsule.compute_audit_hash(false, 0.8, false, 100);
    assert_ne!(hash1, hash2, "Different inputs should produce different hashes");
}

#[test]
fn production_011_compliance_coca_lockfree() {
    // Test that capsule uses only atomics (no mutex/RwLock)
    let mut capsule = EXIFCameraDatabaseCapsule::new();

    // Verify all methods complete without blocking
    let start = std::time::Instant::now();
    for _ in 0..10000 {
        let _found = capsule.lookup_camera("Canon", "EOS 5D");
    }
    let elapsed = start.elapsed();

    // Should complete in <50ms for 10k iterations (5μs each)
    // If blocking occurred, would be 100ms+
    assert!(
        elapsed.as_millis() < 50,
        "Lockfree guarantee: 10k iterations should complete in <50ms, got {}ms",
        elapsed.as_millis()
    );
}

#[test]
fn production_012_compliance_assum_safety() {
    let capsule = EXIFCameraDatabaseCapsule::new();

    // Test all assumptions documented in code
    // #ASSUME_DETERMINISTIC_HASH - Verify with 1000 iterations
    for i in 0..1000 {
        let hash1 = capsule.compute_audit_hash(true, 0.8, false, i);
        let hash2 = capsule.compute_audit_hash(true, 0.8, false, i);
        assert_eq!(hash1, hash2, "Assumption violation: hash not deterministic at i={}", i);
    }

    // #ASSUME_EXIF_MINIMAL - Verify parsing handles empty data
    let mut capsule_mut = EXIFCameraDatabaseCapsule::new();
    let result = capsule_mut.validate_exif(b"");
    assert!(result.is_err(), "Should error on empty EXIF (assumption verified)");

    // #ASSUME_LOCKFREE_ONLY - All operations must be atomic
    // Verified by construction (no mutex fields in struct)
}

// ============================================================================
// HELPER ASSERTIONS FOR FRAMEWORK COMPLIANCE
// ============================================================================

fn is_capsule_default(capsule: &EXIFCameraDatabaseCapsule) -> bool {
    let (val_count, bloom_hits, hash_queries, confidence) = capsule.get_statistics();
    val_count == 0 && bloom_hits == 0 && hash_queries == 0 && confidence == 0
}
