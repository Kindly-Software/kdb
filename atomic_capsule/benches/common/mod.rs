//! # Common Baseline Implementations for Benchmarks
//!
//! **B32 Framework Compliance**: Fair baselines for performance comparison
//!
//! ## Baseline Types
//!
//! - **RwLock<HashMap>**: Fair baseline for concurrent maps (not strawman)
//! - **Mutex<Vec>**: Fair baseline for concurrent logs
//! - **DashMap**: Optimized concurrent map (context comparison)
//! - **Manual Serialization**: Non-trait baseline for serialization
//!
//! ## Usage
//!
//! ```rust
//! use common::{baseline_hashmap_rwlock, baseline_vec_mutex};
//!
//! let map = baseline_hashmap_rwlock();
//! let log = baseline_vec_mutex();
//! ```

use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

// ============================================================================
// Map Baselines
// ============================================================================

/// Baseline: RwLock<HashMap> (fair concurrent map baseline)
pub fn baseline_hashmap_rwlock<K, V>() -> RwLock<HashMap<K, V>>
where
    K: std::hash::Hash + Eq,
{
    RwLock::new(HashMap::new())
}

/// Baseline: RwLock<HashMap> with capacity
pub fn baseline_hashmap_rwlock_with_capacity<K, V>(capacity: usize) -> RwLock<HashMap<K, V>>
where
    K: std::hash::Hash + Eq,
{
    RwLock::new(HashMap::with_capacity(capacity))
}

// ============================================================================
// Log/Vec Baselines
// ============================================================================

/// Baseline: Mutex<Vec> (fair append-only baseline)
pub fn baseline_vec_mutex<T>() -> Mutex<Vec<T>> {
    Mutex::new(Vec::new())
}

/// Baseline: Mutex<Vec> with capacity
pub fn baseline_vec_mutex_with_capacity<T>(capacity: usize) -> Mutex<Vec<T>> {
    Mutex::new(Vec::with_capacity(capacity))
}

// ============================================================================
// Serialization Baselines
// ============================================================================

#[cfg(feature = "capsule-serialize")]
use atomic_capsule::serialize::fixed_point_impls::Q16_16;

/// Baseline: Manual serialization without trait overhead
#[cfg(feature = "capsule-serialize")]
pub fn baseline_manual_serialize_q16_16(value: &Q16_16) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(&0x46495850u32.to_le_bytes()); // Magic 'FIXP'
    bytes.extend_from_slice(&1u16.to_le_bytes()); // Version
    bytes.extend_from_slice(&1u16.to_le_bytes()); // Field count
    bytes.extend_from_slice(&value.to_raw().to_le_bytes()); // Raw i32
    bytes.extend_from_slice(&0u64.to_le_bytes()); // No checksum
    bytes
}

/// Baseline: Manual deserialization without trait overhead
#[cfg(feature = "capsule-serialize")]
pub fn baseline_manual_deserialize_q16_16(bytes: &[u8]) -> Result<Q16_16, String> {
    if bytes.len() < 24 {
        return Err("Invalid length".to_string());
    }

    // Validate magic
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != 0x46495850 {
        return Err("Invalid magic".to_string());
    }

    // Extract raw value
    let raw = i32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    Ok(Q16_16::from_raw(raw))
}

// ============================================================================
// DashMap Wrapper (Optional Context Comparison)
// ============================================================================

/// DashMap wrapper for context comparison (not primary baseline)
#[cfg(feature = "std")]
pub fn baseline_dashmap<K, V>() -> dashmap::DashMap<K, V>
where
    K: std::hash::Hash + Eq,
{
    dashmap::DashMap::new()
}

/// DashMap with capacity
#[cfg(feature = "std")]
pub fn baseline_dashmap_with_capacity<K, V>(capacity: usize) -> dashmap::DashMap<K, V>
where
    K: std::hash::Hash + Eq,
{
    dashmap::DashMap::with_capacity(capacity)
}

// ============================================================================
// Test Helpers
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hashmap_rwlock_baseline() {
        let map = baseline_hashmap_rwlock::<String, u64>();
        {
            let mut w = map.write().unwrap();
            w.insert("key".to_string(), 42);
        }
        {
            let r = map.read().unwrap();
            assert_eq!(r.get("key"), Some(&42));
        }
    }

    #[test]
    fn test_vec_mutex_baseline() {
        let vec = baseline_vec_mutex::<u64>();
        {
            let mut v = vec.lock().unwrap();
            v.push(42);
        }
        {
            let v = vec.lock().unwrap();
            assert_eq!(v.len(), 1);
            assert_eq!(v[0], 42);
        }
    }

    #[cfg(feature = "capsule-serialize")]
    #[test]
    fn test_manual_serialize_roundtrip() {
        let q16 = Q16_16::from_f64(1234.5678);
        let bytes = baseline_manual_serialize_q16_16(&q16);
        let restored = baseline_manual_deserialize_q16_16(&bytes).unwrap();
        assert_eq!(q16.to_raw(), restored.to_raw());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_dashmap_baseline() {
        let map = baseline_dashmap::<String, u64>();
        map.insert("key".to_string(), 42);
        assert_eq!(map.get("key").map(|r| *r), Some(42));
    }
}
