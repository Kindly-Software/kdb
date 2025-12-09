//! Scene Detection Demo
//!
//! Demonstrates SceneDetectionCapsule usage for AV1 encoding.
//!
//! # Example
//!
//! ```bash
//! cargo run --example scene_detection_demo --features "std,portable_simd"
//! ```

use atomic_capsule::encoder::{SceneDetectionCapsule, SceneDetectionStats};

fn main() {
    println!("=== Scene Detection Capsule Demo ===\n");

    // Create detector with default configuration
    let detector = SceneDetectionCapsule::new();
    println!("Created detector with default config:");
    println!("  - Threshold: {}% (Q8.8 = {})", 10, SceneDetectionCapsule::DEFAULT_THRESHOLD);
    println!("  - Sensitivity: {}", SceneDetectionCapsule::DEFAULT_SENSITIVITY);
    println!("  - Methods: All enabled\n");

    // Test 1: Similar frames (no scene change)
    println!("Test 1: Similar frames (no scene change)");
    let frame1 = vec![128u8; 1920 * 1080]; // Gray frame
    let frame2 = vec![130u8; 1920 * 1080]; // Slightly different gray

    detector.update_frame_stats(&frame1, 1920, 1080);
    let (is_scene_change, confidence) = detector.detect(&frame2, 1920, 1080);
    println!("  Frame 1 → Frame 2:");
    println!("    Scene change: {}", is_scene_change);
    println!("    Confidence: {:.2}\n", confidence);

    // Test 2: Large difference (scene change)
    println!("Test 2: Large difference (scene change expected)");
    let frame3 = vec![200u8; 1920 * 1080]; // Much brighter frame

    let (is_scene_change, confidence) = detector.detect(&frame3, 1920, 1080);
    println!("  Frame 2 → Frame 3:");
    println!("    Scene change: {}", is_scene_change);
    println!("    Confidence: {:.2}\n", confidence);

    // Test 3: Flash detection
    println!("Test 3: Flash detection (false positive rejection)");

    // Create new detector for clean flash test
    let flash_detector = SceneDetectionCapsule::new();

    // Frame 1: Normal brightness
    let normal1 = vec![100u8; 1920 * 1080];
    flash_detector.update_frame_stats(&normal1, 1920, 1080);

    // Frame 2: Flash (very bright)
    let flash_frame = vec![250u8; 1920 * 1080];
    flash_detector.update_frame_stats(&flash_frame, 1920, 1080);

    // Frame 3: Return to normal (flash recovery)
    let normal2 = vec![100u8; 1920 * 1080];
    flash_detector.update_frame_stats(&normal2, 1920, 1080);

    let is_flash = flash_detector.is_flash();
    println!("  Flash detected: {}", is_flash);
    println!("  (Flash detection prevents false positives)\n");

    // Test 4: Custom configuration (more sensitive)
    println!("Test 4: Custom configuration (high sensitivity)");
    let sensitive_detector = SceneDetectionCapsule::with_config(
        52,  // 20% threshold (more sensitive)
        200, // High sensitivity
        SceneDetectionCapsule::METHOD_ALL,
    );

    let frame4 = vec![140u8; 1920 * 1080];
    sensitive_detector.update_frame_stats(&frame4, 1920, 1080);

    let frame5 = vec![160u8; 1920 * 1080];
    let (is_scene_change, confidence) = sensitive_detector.detect(&frame5, 1920, 1080);
    println!("  High sensitivity detector:");
    println!("    Scene change: {}", is_scene_change);
    println!("    Confidence: {:.2}\n", confidence);

    // Test 5: Statistics
    println!("Test 5: Detection statistics");
    let stats = detector.get_stats();
    print_stats(&stats);

    println!("\n=== Demo Complete ===");
}

fn print_stats(stats: &SceneDetectionStats) {
    println!("  Scene count: {}", stats.scene_count);
    println!("  False positives: {}", stats.false_positive_count);
    println!("  Generation: {}", stats.generation);

    if stats.scene_count > 0 {
        let fp_rate = (stats.false_positive_count as f64 / stats.scene_count as f64) * 100.0;
        println!("  False positive rate: {:.2}%", fp_rate);
    }
}
