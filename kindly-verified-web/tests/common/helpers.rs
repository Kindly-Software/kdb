/// Test utilities and helpers for kindly-verified-web integration tests
/// Provides real test image data, mock factories, and common assertions

use std::collections::HashMap;
use std::sync::Arc;

/// Minimal valid PNG (8×8 transparent)
pub fn create_test_png() -> Vec<u8> {
    // This is a real minimal PNG - 8x8 RGBA transparent
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08,
        0x08, 0x06, 0x00, 0x00, 0x00, 0xC4, 0x0F, 0xBE, 0x8B, 0x00, 0x00, 0x00,
        0x1D, 0x49, 0x44, 0x41, 0x54, 0x08, 0x99, 0x01, 0x0C, 0x00, 0xF3, 0xFF,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF1, 0x07,
        0x19, 0xFB, 0x00, 0x01, 0xC9, 0x68, 0x32, 0xDE, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

/// Create a slightly larger test image (128×128)
pub fn create_large_test_png() -> Vec<u8> {
    // Minimal 128x128 PNG
    let mut png = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80,
        0x08, 0x06, 0x00, 0x00, 0x00, 0xC4, 0x0F, 0xBE, 0x8B,
    ];
    // Add minimal IDAT chunks (just fill with zeros for testing)
    for _ in 0..256 {
        png.push(0x00);
    }
    png.extend_from_slice(&[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]);
    png
}

/// Mock detector result with confidence scores
#[derive(Debug, Clone)]
pub struct MockDetectionResult {
    pub detector_confidences: Vec<f32>,
    pub overall_confidence: f32,
    pub detector_names: Vec<String>,
}

impl Default for MockDetectionResult {
    fn default() -> Self {
        Self {
            detector_confidences: vec![
                0.85, // EXIF Integrity Seal
                0.72, // Chromatic Aberration Guard
                0.91, // Compression Artifact Sentinel
                0.68, // Noise Pattern Oracle
                0.88, // Frequency Domain Augur
                0.75, // Edge Consistency Praetor
                0.82, // Color Distribution Legate
                0.79, // Metadata Chain Curator
                0.86, // Statistical Harmony Consul
                0.90, // Neural Pattern Imperator
            ],
            overall_confidence: 0.816, // Average of above
            detector_names: vec![
                "EXIF Integrity Seal".to_string(),
                "Chromatic Aberration Guard".to_string(),
                "Compression Artifact Sentinel".to_string(),
                "Noise Pattern Oracle".to_string(),
                "Frequency Domain Augur".to_string(),
                "Edge Consistency Praetor".to_string(),
                "Color Distribution Legate".to_string(),
                "Metadata Chain Curator".to_string(),
                "Statistical Harmony Consul".to_string(),
                "Neural Pattern Imperator".to_string(),
            ],
        }
    }
}

/// Mock detection entry for IndexedDB storage
#[derive(Debug, Clone)]
pub struct MockDetectionEntry {
    pub id: String,
    pub timestamp: u64,
    pub image_hash: String,
    pub confidence: f32,
    pub detector_results: Vec<f32>,
}

impl Default for MockDetectionEntry {
    fn default() -> Self {
        Self {
            id: uuid_v4(),
            timestamp: current_timestamp(),
            image_hash: "abc123def456".to_string(),
            confidence: 0.816,
            detector_results: MockDetectionResult::default().detector_confidences,
        }
    }
}

/// Generate a UUID v4 (simple mock, not cryptographically secure)
pub fn uuid_v4() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut hasher = DefaultHasher::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    now.hash(&mut hasher);

    let hash = hasher.finish();
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (hash >> 96) as u32,
        ((hash >> 80) & 0xFFFF) as u16,
        ((hash >> 64) & 0xFFFF) as u16,
        ((hash >> 48) & 0xFFFF) as u16,
        hash & 0xFFFFFFFFFFFF
    )
}

/// Get current timestamp in milliseconds
pub fn current_timestamp() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now()) as u64
    }
}

/// Mock in-memory database for testing (simulates IndexedDB)
pub struct MockDatabase {
    store: Arc<std::sync::Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockDatabase {
    pub fn new() -> Self {
        Self {
            store: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn save(&self, key: String, value: Vec<u8>) -> Result<(), String> {
        self.store
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .insert(key, value);
        Ok(())
    }

    pub fn load(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.store
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .get(key)
            .cloned())
    }

    pub fn delete(&self, key: &str) -> Result<(), String> {
        self.store
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .remove(key);
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<String>, String> {
        Ok(self.store
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .keys()
            .cloned()
            .collect())
    }

    pub fn clear(&self) -> Result<(), String> {
        self.store
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .clear();
        Ok(())
    }
}

impl Default for MockDatabase {
    fn default() -> Self {
        Self::new()
    }
}

/// Simulate batch upload progress
#[derive(Debug, Clone)]
pub struct BatchUploadProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub per_image_progress: Vec<u8>, // 0-100% for each image
}

impl Default for BatchUploadProgress {
    fn default() -> Self {
        Self {
            total: 0,
            completed: 0,
            failed: 0,
            per_image_progress: vec![],
        }
    }
}

/// Test assertion helpers
pub mod assertions {
    use super::*;

    /// Assert confidence is in valid range [0.0, 1.0]
    pub fn assert_valid_confidence(confidence: f32, msg: &str) {
        assert!(
            confidence >= 0.0 && confidence <= 1.0,
            "Invalid confidence: {} ({})",
            confidence,
            msg
        );
    }

    /// Assert all detector confidences are valid
    pub fn assert_valid_detector_confidences(confidences: &[f32], msg: &str) {
        assert!(
            confidences.len() <= 10,
            "Too many detectors: {} ({})",
            confidences.len(),
            msg
        );
        for (i, &conf) in confidences.iter().enumerate() {
            assert!(
                conf >= 0.0 && conf <= 1.0,
                "Invalid detector[{}] confidence: {} ({})",
                i,
                conf,
                msg
            );
        }
    }

    /// Assert overall confidence is close to average
    pub fn assert_confidence_is_average(
        overall: f32,
        individual: &[f32],
        tolerance: f32,
        msg: &str,
    ) {
        if !individual.is_empty() {
            let avg = individual.iter().sum::<f32>() / individual.len() as f32;
            assert!(
                (overall - avg).abs() < tolerance,
                "Confidence mismatch: overall={}, avg={}, tolerance={} ({})",
                overall,
                avg,
                tolerance,
                msg
            );
        }
    }

    /// Assert Byzantine colors are correct
    pub fn assert_byzantine_color(color_hex: &str, expected: &str) {
        assert_eq!(
            color_hex.to_lowercase(),
            expected.to_lowercase(),
            "Byzantine color mismatch"
        );
    }

    /// Assert progress is monotonically increasing
    pub fn assert_progress_increasing(progress: &[u8], msg: &str) {
        for i in 1..progress.len() {
            assert!(
                progress[i] >= progress[i - 1],
                "Progress not monotonic at index {}: {} < {} ({})",
                i,
                progress[i],
                progress[i - 1],
                msg
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_png_is_valid() {
        let png = create_test_png();
        assert!(png.len() > 8);
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn test_uuid_v4_format() {
        let uuid = uuid_v4();
        let parts: Vec<&str> = uuid.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }

    #[test]
    fn test_mock_database_operations() {
        let db = MockDatabase::new();
        db.save("key1".to_string(), vec![1, 2, 3]).unwrap();
        assert_eq!(db.load("key1").unwrap(), Some(vec![1, 2, 3]));
        db.delete("key1").unwrap();
        assert_eq!(db.load("key1").unwrap(), None);
    }

    #[test]
    fn test_mock_detection_result_default() {
        let result = MockDetectionResult::default();
        assert_eq!(result.detector_confidences.len(), 10);
        assert!(result.overall_confidence > 0.0 && result.overall_confidence < 1.0);
    }
}
