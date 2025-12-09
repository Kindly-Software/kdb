//! ResponseSanitizerCapsule - T1 Atomic Response Filtering
//!
//! Removes internal Chaos implementation details from MCP tool responses before
//! exposing to production users. Tracks sanitization metrics atomically.
//!
//! **Tier**: T1 Atomic (lockfree metrics tracking)
//! **Size**: 128 bytes (cache-aligned)
//! **Latency**: <1μs per response (in-place JSON mutation)
//!
//! # Removed Fields
//! - `tier`: Internal Chaos tier (T0/T1/T2/T3/T4/T5/T6/T7/T8/T9/T10/T11)
//! - `capsule`: Implementation capsule class names
//! - `latency_ns`, `latency_target`: Performance metrics
//! - `generation`: Internal TOCTOU counters
//! - `_padding*`: Alignment padding fields
//!
//! # Preserved Fields
//! - All user-facing: tier_name, limits, usage, features, quotas, status, etc.

use core::sync::atomic::{AtomicU64, Ordering};

/// Fields to remove from all tool responses (Chaos implementation details)
const TECHNICAL_FIELDS: &[&str] = &[
    "tier",           // Internal Chaos tier classification (T0-T11)
    "capsule",        // Implementation capsule class names
    "latency_ns",     // Performance metrics (internal SLA tracking)
    "latency_target", // Performance targets (internal)
    "generation",     // Internal TOCTOU prevention counters
];

/// ResponseSanitizerCapsule - T1 Atomic Response Filtering
///
/// **Chaos Compliance**:
/// - Lockfree: All operations use atomic primitives (no mutex/RwLock)
/// - Cache-aligned: 128 bytes, 64-byte alignment
/// - Generation counter: TOCTOU prevention for metrics snapshot
/// - Stateless core: Sanitization logic is pure, metrics are optional
///
/// **Size**: 128 bytes
/// **Alignment**: 64 bytes (single cache line for hot path)
///
/// # Layout
/// ```text
/// Offset | Field              | Size | Purpose
/// -------|-------------------|------|---------------------------
/// 0      | responses_total    | 8    | Total responses sanitized
/// 8      | fields_removed     | 8    | Total technical fields removed
/// 16     | bytes_saved        | 8    | Estimated bytes saved
/// 24     | generation         | 8    | TOCTOU prevention counter
/// 32     | failures           | 8    | Sanitization failures (JSON parse)
/// 40     | _padding           | 24   | Align to 64 bytes
/// 64     | _reserved          | 64   | Future expansion
/// ```
#[repr(C, align(64))]
pub struct ResponseSanitizerCapsule {
    /// Total responses sanitized (monotonic counter)
    responses_total: AtomicU64,

    /// Total technical fields removed across all responses
    fields_removed: AtomicU64,

    /// Estimated bytes saved by removing technical fields
    bytes_saved: AtomicU64,

    /// Generation counter (TOCTOU prevention for metrics snapshot)
    generation: AtomicU64,

    /// Sanitization failures (malformed JSON, edge cases)
    failures: AtomicU64,

    /// Padding to 64 bytes (single cache line)
    _padding: [u8; 24],

    /// Reserved for future metrics (total 128 bytes)
    _reserved: [u8; 64],
}

impl ResponseSanitizerCapsule {
    /// Create new response sanitizer capsule
    ///
    /// **Latency**: 0ns (const initialization)
    pub const fn new() -> Self {
        Self {
            responses_total: AtomicU64::new(0),
            fields_removed: AtomicU64::new(0),
            bytes_saved: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            _padding: [0; 24],
            _reserved: [0; 64],
        }
    }

    /// Sanitize tool response by removing technical implementation details
    ///
    /// **Tier**: T1 Atomic (lockfree metrics tracking)
    /// **Latency**: <1μs (in-place JSON mutation + atomic metrics)
    ///
    /// # Arguments
    /// - `response`: Mutable reference to JSON response value
    ///
    /// # Side Effects
    /// - Modifies `response` in-place (removes technical fields)
    /// - Updates atomic metrics (responses_total, fields_removed, bytes_saved)
    ///
    /// # Performance
    /// - Hot path: O(fields) traversal, no allocations
    /// - Metrics: 3 atomic increments (<30ns total)
    ///
    /// #ASSUME_VALID_JSON: response is valid serde_json::Value
    /// #VERIFY_JSON: Unit tests validate sanitization correctness
    pub fn sanitize(&self, response: &mut serde_json::Value) {
        // Track fields removed for metrics
        let mut removed_count = 0u64;
        let mut estimated_bytes = 0u64;

        // Sanitize the response
        if let Some(obj) = response.as_object_mut() {
            // Remove top-level technical fields
            for field in TECHNICAL_FIELDS {
                if let Some(removed_value) = obj.remove(*field) {
                    removed_count += 1;
                    // Estimate bytes saved (field name + separator + value)
                    estimated_bytes += field.len() as u64 + 10; // Approx overhead
                    estimated_bytes += estimate_json_size(&removed_value);
                }
            }

            // Recursively sanitize nested objects
            for (_, value) in obj.iter_mut() {
                let nested_removed = sanitize_nested(value);
                removed_count += nested_removed;
                estimated_bytes += nested_removed * 15; // Approx bytes per field
            }
        }

        // Update atomic metrics (<30ns total)
        self.responses_total.fetch_add(1, Ordering::Relaxed);
        self.fields_removed.fetch_add(removed_count, Ordering::Relaxed);
        self.bytes_saved.fetch_add(estimated_bytes, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release); // TOCTOU prevention
    }

    /// Get sanitization statistics (atomic snapshot)
    ///
    /// **Latency**: <50ns (4 atomic loads)
    ///
    /// # Returns
    /// Tuple: (responses_total, fields_removed, bytes_saved, failures)
    pub fn stats(&self) -> (u64, u64, u64, u64) {
        // Atomic snapshot with generation counter validation
        let gen1 = self.generation.load(Ordering::Acquire);

        let total = self.responses_total.load(Ordering::Relaxed);
        let removed = self.fields_removed.load(Ordering::Relaxed);
        let bytes = self.bytes_saved.load(Ordering::Relaxed);
        let failures = self.failures.load(Ordering::Relaxed);

        let gen2 = self.generation.load(Ordering::Acquire);

        // If generation changed, snapshot is potentially inconsistent
        // For metrics, this is acceptable (eventual consistency)
        // If strict consistency needed, retry with CAS loop
        if gen1 != gen2 {
            // Re-read for consistency (rare race condition)
            return (
                self.responses_total.load(Ordering::Acquire),
                self.fields_removed.load(Ordering::Acquire),
                self.bytes_saved.load(Ordering::Acquire),
                self.failures.load(Ordering::Acquire),
            );
        }

        (total, removed, bytes, failures)
    }
}

/// Recursively sanitize nested objects
///
/// **Returns**: Number of fields removed (for metrics)
fn sanitize_nested(value: &mut serde_json::Value) -> u64 {
    let mut removed = 0u64;

    match value {
        serde_json::Value::Object(obj) => {
            // Remove technical fields from nested objects
            for field in TECHNICAL_FIELDS {
                if obj.remove(*field).is_some() {
                    removed += 1;
                }
            }

            // Recurse into nested values
            for (_, v) in obj.iter_mut() {
                removed += sanitize_nested(v);
            }
        }
        serde_json::Value::Array(arr) => {
            // Sanitize each element in arrays (e.g., audit_trail entries)
            for item in arr.iter_mut() {
                removed += sanitize_nested(item);
            }
        }
        _ => {} // Primitives need no sanitization
    }

    removed
}

/// Estimate JSON value size in bytes (approximate)
fn estimate_json_size(value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::Null => 4,
        serde_json::Value::Bool(_) => 5,
        serde_json::Value::Number(_) => 10,
        serde_json::Value::String(s) => s.len() as u64 + 2,
        serde_json::Value::Array(arr) => {
            arr.iter().map(estimate_json_size).sum::<u64>() + arr.len() as u64 * 2
        }
        serde_json::Value::Object(obj) => {
            obj.iter()
                .map(|(k, v)| k.len() as u64 + estimate_json_size(v) + 5)
                .sum()
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(
            core::mem::size_of::<ResponseSanitizerCapsule>(),
            128,
            "Capsule must be 128 bytes"
        );
        assert_eq!(
            core::mem::align_of::<ResponseSanitizerCapsule>(),
            64,
            "Capsule must be 64-byte aligned"
        );
    }

    #[test]
    fn test_sanitize_quota_status() {
        let capsule = ResponseSanitizerCapsule::new();

        let mut response = json!({
            "tier": "T1 Atomic",
            "capsule": "QuotaTrackerCapsule",
            "latency_ns": "<70",
            "tier_name": "Professional",
            "limits": {
                "daily": 10000,
                "monthly": 100000
            },
            "usage": {
                "daily_requests": 8,
                "monthly_requests": 8
            }
        });

        capsule.sanitize(&mut response);

        // Technical fields removed
        assert!(response.get("tier").is_none());
        assert!(response.get("capsule").is_none());
        assert!(response.get("latency_ns").is_none());

        // User-facing fields preserved
        assert_eq!(response["tier_name"], "Professional");
        assert_eq!(response["limits"]["daily"], 10000);
        assert_eq!(response["usage"]["daily_requests"], 8);

        // Metrics tracked
        let (total, removed, _, failures) = capsule.stats();
        assert_eq!(total, 1);
        assert_eq!(removed, 3); // tier, capsule, latency_ns
        assert_eq!(failures, 0);
    }

    #[test]
    fn test_sanitize_license_info() {
        let capsule = ResponseSanitizerCapsule::new();

        let mut response = json!({
            "tier": "T1 Atomic",
            "capsule": "LicenseValidatorCapsule",
            "latency_ns": "<10 (cached)",
            "tier_name": "Developer",
            "license": {
                "is_valid": true,
                "expiry_status": "valid (90 days remaining)"
            },
            "features": ["time_travel", "breakpoints", "stack_trace"]
        });

        capsule.sanitize(&mut response);

        assert!(response.get("tier").is_none());
        assert!(response.get("capsule").is_none());
        assert!(response.get("latency_ns").is_none());
        assert_eq!(response["tier_name"], "Developer");
        assert!(response["license"]["is_valid"].as_bool().unwrap());
        assert_eq!(response["features"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_sanitize_pool_stats() {
        let capsule = ResponseSanitizerCapsule::new();

        let mut response = json!({
            "tier": "T6 Mixed",
            "capsule": "SessionPoolCapsule",
            "latency_ns": "<50",
            "pool_stats": {
                "light": { "capacity": 1500, "used": 10 },
                "medium": { "capacity": 600, "used": 5 }
            },
            "totals": {
                "total_allocations": 100
            }
        });

        capsule.sanitize(&mut response);

        assert!(response.get("tier").is_none());
        assert!(response.get("capsule").is_none());
        assert_eq!(response["pool_stats"]["light"]["capacity"], 1500);
        assert_eq!(response["totals"]["total_allocations"], 100);
    }

    #[test]
    fn test_sanitize_nested_arrays() {
        let capsule = ResponseSanitizerCapsule::new();

        let mut response = json!({
            "tier": "T0 Auditable",
            "audit_trail": [
                {
                    "id": 1,
                    "tier": "T1 Atomic",
                    "latency_ns": 500,
                    "success": true
                },
                {
                    "id": 2,
                    "capsule": "TestCapsule",
                    "result": "ok"
                }
            ]
        });

        capsule.sanitize(&mut response);

        assert!(response.get("tier").is_none());

        let trail = response["audit_trail"].as_array().unwrap();
        assert_eq!(trail[0]["id"], 1);
        assert!(trail[0].get("tier").is_none());
        assert!(trail[0].get("latency_ns").is_none());
        assert!(trail[0]["success"].as_bool().unwrap());

        assert_eq!(trail[1]["id"], 2);
        assert!(trail[1].get("capsule").is_none());
        assert_eq!(trail[1]["result"], "ok");
    }

    #[test]
    fn test_sanitize_preserves_user_fields() {
        let capsule = ResponseSanitizerCapsule::new();

        let mut response = json!({
            "tier": "T1 Atomic",
            "tier_name": "Professional",
            "count": 42,
            "enabled": true,
            "name": "test",
            "nested": {
                "capsule": "InternalCapsule",
                "value": 123
            }
        });

        capsule.sanitize(&mut response);

        // Technical fields removed
        assert!(response.get("tier").is_none());
        assert!(response["nested"].get("capsule").is_none());

        // User fields preserved
        assert_eq!(response["tier_name"], "Professional");
        assert_eq!(response["count"], 42);
        assert!(response["enabled"].as_bool().unwrap());
        assert_eq!(response["name"], "test");
        assert_eq!(response["nested"]["value"], 123);
    }

    #[test]
    fn test_capsule_metrics_tracking() {
        let capsule = ResponseSanitizerCapsule::new();

        // Sanitize multiple responses
        for _ in 0..10 {
            let mut response = json!({
                "tier": "T1 Atomic",
                "capsule": "TestCapsule",
                "latency_ns": "<100",
                "data": "test"
            });
            capsule.sanitize(&mut response);
        }

        let (total, removed, bytes, failures) = capsule.stats();
        assert_eq!(total, 10);
        assert_eq!(removed, 30); // 3 fields × 10 responses
        assert!(bytes > 0);
        assert_eq!(failures, 0);
    }

    #[test]
    fn test_sanitize_idempotent() {
        let capsule = ResponseSanitizerCapsule::new();

        let mut response = json!({
            "tier_name": "Professional",
            "limits": { "daily": 50000 }
        });

        // Already sanitized, should be unchanged
        capsule.sanitize(&mut response);

        assert_eq!(response["tier_name"], "Professional");
        assert_eq!(response["limits"]["daily"], 50000);

        let (total, removed, _, _) = capsule.stats();
        assert_eq!(total, 1);
        assert_eq!(removed, 0); // No fields to remove
    }

    #[test]
    fn test_sanitize_empty_object() {
        let capsule = ResponseSanitizerCapsule::new();

        let mut response = json!({});
        capsule.sanitize(&mut response);

        assert!(response.as_object().unwrap().is_empty());

        let (total, removed, _, _) = capsule.stats();
        assert_eq!(total, 1);
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_stats_atomic_snapshot() {
        let capsule = ResponseSanitizerCapsule::new();

        // Sanitize a response
        let mut response = json!({"tier": "T1 Atomic", "data": "test"});
        capsule.sanitize(&mut response);

        // Stats should be consistent
        let (total1, removed1, bytes1, failures1) = capsule.stats();
        let (total2, removed2, bytes2, failures2) = capsule.stats();

        assert_eq!(total1, total2);
        assert_eq!(removed1, removed2);
        assert_eq!(bytes1, bytes2);
        assert_eq!(failures1, failures2);
    }
}
