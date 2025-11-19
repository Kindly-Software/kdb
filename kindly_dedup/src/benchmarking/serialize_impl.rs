//! CapsuleSerialize implementations for benchmarking types
//!
//! **ZERO SERDE DEPENDENCY** - All serialization via atomic_capsule primitives
//!
//! Implements WriteJson/ParseJson for:
//! - BenchmarkAuditEntry
//! - BenchmarkConfig
//! - BenchmarkResult
//! - AccuracyMetrics
//! - EnvironmentInfo

use crate::benchmarking::audit_logger::{AccuracyMetrics, BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult, Hash256};
use crate::benchmarking::environment::EnvironmentInfo;
use crate::serialize_helpers::{
    get_field_required, serialize_struct, write_field, JsonError, JsonParserCapsule, JsonValue,
    JsonWriterCapsule, ParseJson, WriteJson,
};

// ============================================================================
// WriteJson Implementations
// ============================================================================

impl WriteJson for EnvironmentInfo {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        let mut first = true;
        write_field(writer, "rustc_version", &self.rustc_version, &mut first)?;
        write_field(writer, "cpu_model", &self.cpu_model, &mut first)?;
        write_field(writer, "cpu_cores", &self.cpu_cores, &mut first)?;
        write_field(writer, "os_version", &self.os_version, &mut first)?;
        write_field(writer, "feature_flags", &self.feature_flags, &mut first)?;
        write_field(writer, "git_commit", &self.git_commit, &mut first)?;
        write_field(writer, "git_dirty", &self.git_dirty, &mut first)?;
        Ok(())
    }
}

impl WriteJson for AccuracyMetrics {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        let mut first = true;
        write_field(writer, "recall", &self.recall, &mut first)?;
        write_field(writer, "precision", &self.precision, &mut first)?;
        write_field(writer, "f1", &self.f1, &mut first)?;
        write_field(writer, "true_positives", &self.true_positives, &mut first)?;
        write_field(writer, "false_positives", &self.false_positives, &mut first)?;
        write_field(writer, "true_negatives", &self.true_negatives, &mut first)?;
        write_field(writer, "false_negatives", &self.false_negatives, &mut first)?;
        Ok(())
    }
}

impl WriteJson for BenchmarkResult {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        let mut first = true;
        write_field(writer, "throughput_docs_per_sec", &self.throughput_docs_per_sec, &mut first)?;
        write_field(writer, "latency_p50_us", &self.latency_p50_us, &mut first)?;
        write_field(writer, "latency_p95_us", &self.latency_p95_us, &mut first)?;
        write_field(writer, "latency_p99_us", &self.latency_p99_us, &mut first)?;
        write_field(writer, "latency_mean_us", &self.latency_mean_us, &mut first)?;
        write_field(writer, "latency_stddev_us", &self.latency_stddev_us, &mut first)?;
        write_field(writer, "ci_95_lower_us", &self.ci_95_lower_us, &mut first)?;
        write_field(writer, "ci_95_upper_us", &self.ci_95_upper_us, &mut first)?;
        write_field(writer, "accuracy", &self.accuracy, &mut first)?;
        Ok(())
    }
}

impl WriteJson for BenchmarkConfig {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        let mut first = true;
        write_field(writer, "dataset", &self.dataset, &mut first)?;
        write_field(writer, "threads", &self.threads, &mut first)?;
        write_field(writer, "features", &self.features, &mut first)?;
        write_field(writer, "warmup_iterations", &self.warmup_iterations, &mut first)?;
        write_field(writer, "measurement_iterations", &self.measurement_iterations, &mut first)?;
        Ok(())
    }
}

impl WriteJson for BenchmarkAuditEntry {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        let mut first = true;
        write_field(writer, "benchmark_id", &self.benchmark_id, &mut first)?;
        write_field(writer, "timestamp", &self.timestamp, &mut first)?;

        // Environment (nested object)
        if !first {
            writer.write_comma()?;
        }
        first = false;
        writer.write_string("environment")?;
        writer.write_colon()?;
        writer.start_object()?;
        self.environment.write_json(writer)?;
        writer.end_object()?;

        // Config (nested object)
        writer.write_comma()?;
        writer.write_string("config")?;
        writer.write_colon()?;
        writer.start_object()?;
        self.config.write_json(writer)?;
        writer.end_object()?;

        // Hashes (hex-encoded)
        write_field(writer, "input_hash", &hash_to_hex(&self.input_hash), &mut first)?;

        // Result (nested object)
        writer.write_comma()?;
        writer.write_string("result")?;
        writer.write_colon()?;
        writer.start_object()?;
        self.result.write_json(writer)?;
        writer.end_object()?;

        // More hashes
        write_field(writer, "result_hash", &hash_to_hex(&self.result_hash), &mut first)?;
        write_field(writer, "prev_audit_hash", &hash_to_hex(&self.prev_audit_hash), &mut first)?;
        write_field(writer, "audit_hash", &hash_to_hex(&self.audit_hash), &mut first)?;

        Ok(())
    }
}

// ============================================================================
// ParseJson Implementations
// ============================================================================

impl ParseJson for EnvironmentInfo {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Object(fields) => {
                Ok(EnvironmentInfo {
                    rustc_version: String::parse_json(get_field_required(fields, "rustc_version")?)?,
                    cpu_model: String::parse_json(get_field_required(fields, "cpu_model")?)?,
                    cpu_cores: parse_usize(get_field_required(fields, "cpu_cores")?)?,
                    os_version: String::parse_json(get_field_required(fields, "os_version")?)?,
                    feature_flags: parse_vec_string(get_field_required(fields, "feature_flags")?)?,
                    git_commit: String::parse_json(get_field_required(fields, "git_commit")?)?,
                    git_dirty: bool::parse_json(get_field_required(fields, "git_dirty")?)?,
                })
            }
            _ => Err(JsonError::TypeMismatch("Expected object for EnvironmentInfo".into())),
        }
    }
}

impl ParseJson for AccuracyMetrics {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Object(fields) => {
                Ok(AccuracyMetrics {
                    recall: f64::parse_json(get_field_required(fields, "recall")?)?,
                    precision: f64::parse_json(get_field_required(fields, "precision")?)?,
                    f1: f64::parse_json(get_field_required(fields, "f1")?)?,
                    true_positives: parse_usize(get_field_required(fields, "true_positives")?)?,
                    false_positives: parse_usize(get_field_required(fields, "false_positives")?)?,
                    true_negatives: parse_usize(get_field_required(fields, "true_negatives")?)?,
                    false_negatives: parse_usize(get_field_required(fields, "false_negatives")?)?,
                })
            }
            _ => Err(JsonError::TypeMismatch("Expected object for AccuracyMetrics".into())),
        }
    }
}

impl ParseJson for BenchmarkResult {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Object(fields) => {
                let accuracy = match crate::serialize_helpers::get_field(fields, "accuracy") {
                    Some(JsonValue::Null) | None => None,
                    Some(v) => Some(AccuracyMetrics::parse_json(v)?),
                };

                Ok(BenchmarkResult {
                    throughput_docs_per_sec: f64::parse_json(get_field_required(fields, "throughput_docs_per_sec")?)?,
                    latency_p50_us: f64::parse_json(get_field_required(fields, "latency_p50_us")?)?,
                    latency_p95_us: f64::parse_json(get_field_required(fields, "latency_p95_us")?)?,
                    latency_p99_us: f64::parse_json(get_field_required(fields, "latency_p99_us")?)?,
                    latency_mean_us: f64::parse_json(get_field_required(fields, "latency_mean_us")?)?,
                    latency_stddev_us: f64::parse_json(get_field_required(fields, "latency_stddev_us")?)?,
                    ci_95_lower_us: f64::parse_json(get_field_required(fields, "ci_95_lower_us")?)?,
                    ci_95_upper_us: f64::parse_json(get_field_required(fields, "ci_95_upper_us")?)?,
                    accuracy,
                })
            }
            _ => Err(JsonError::TypeMismatch("Expected object for BenchmarkResult".into())),
        }
    }
}

impl ParseJson for BenchmarkConfig {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Object(fields) => {
                Ok(BenchmarkConfig {
                    dataset: String::parse_json(get_field_required(fields, "dataset")?)?,
                    threads: parse_usize(get_field_required(fields, "threads")?)?,
                    features: parse_vec_string(get_field_required(fields, "features")?)?,
                    warmup_iterations: parse_usize(get_field_required(fields, "warmup_iterations")?)?,
                    measurement_iterations: parse_usize(get_field_required(fields, "measurement_iterations")?)?,
                })
            }
            _ => Err(JsonError::TypeMismatch("Expected object for BenchmarkConfig".into())),
        }
    }
}

impl ParseJson for BenchmarkAuditEntry {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Object(fields) => {
                Ok(BenchmarkAuditEntry {
                    benchmark_id: String::parse_json(get_field_required(fields, "benchmark_id")?)?,
                    timestamp: u64::parse_json(get_field_required(fields, "timestamp")?)?,
                    environment: EnvironmentInfo::parse_json(get_field_required(fields, "environment")?)?,
                    config: BenchmarkConfig::parse_json(get_field_required(fields, "config")?)?,
                    input_hash: hex_to_hash(&String::parse_json(get_field_required(fields, "input_hash")?)?)?,
                    result: BenchmarkResult::parse_json(get_field_required(fields, "result")?)?,
                    result_hash: hex_to_hash(&String::parse_json(get_field_required(fields, "result_hash")?)?)?,
                    prev_audit_hash: hex_to_hash(&String::parse_json(get_field_required(fields, "prev_audit_hash")?)?)?,
                    audit_hash: hex_to_hash(&String::parse_json(get_field_required(fields, "audit_hash")?)?)?,
                })
            }
            _ => Err(JsonError::TypeMismatch("Expected object for BenchmarkAuditEntry".into())),
        }
    }
}

// ============================================================================
// Public API (serde-compatible interface)
// ============================================================================

/// Serialize to JSON string (serde_json::to_string replacement)
pub fn to_json_string<T: WriteJson>(value: &T) -> Result<String, JsonError> {
    serialize_struct(|writer| value.write_json(writer))
}

/// Serialize to JSON bytes (serde_json::to_vec replacement)
pub fn to_json_vec<T: WriteJson>(value: &T) -> Result<Vec<u8>, JsonError> {
    let json = to_json_string(value)?;
    Ok(json.into_bytes())
}

/// Deserialize from JSON string (serde_json::from_str replacement)
pub fn from_json_string<T: ParseJson>(json: &str) -> Result<T, JsonError> {
    let mut parser = JsonParserCapsule::new(json);
    let value = parser.parse()?;
    T::parse_json(&value)
}

// ============================================================================
// Helper Functions
// ============================================================================

fn hash_to_hex(hash: &Hash256) -> String {
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_to_hash(hex: &str) -> Result<Hash256, JsonError> {
    if hex.len() != 64 {
        return Err(JsonError::Custom(format!("Invalid hex hash length: {} (expected 64)", hex.len())));
    }

    let mut hash = [0u8; 32];
    for i in 0..32 {
        let byte_str = &hex[i * 2..i * 2 + 2];
        hash[i] = u8::from_str_radix(byte_str, 16)
            .map_err(|_| JsonError::Custom(format!("Invalid hex digit: {}", byte_str)))?;
    }
    Ok(hash)
}

fn parse_usize(value: &JsonValue) -> Result<usize, JsonError> {
    match value {
        JsonValue::Number(n) => {
            if *n >= 0.0 && n.fract() == 0.0 {
                Ok(*n as usize)
            } else {
                Err(JsonError::TypeMismatch("Expected non-negative integer for usize".into()))
            }
        }
        _ => Err(JsonError::TypeMismatch("Expected number for usize".into())),
    }
}

fn parse_vec_string(value: &JsonValue) -> Result<Vec<String>, JsonError> {
    match value {
        JsonValue::Array(items) => {
            items.iter().map(|item| String::parse_json(item)).collect()
        }
        _ => Err(JsonError::TypeMismatch("Expected array for Vec<String>".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_info_roundtrip() {
        let env = EnvironmentInfo {
            rustc_version: "1.84.0".to_string(),
            cpu_model: "AMD Ryzen 9 6900HX".to_string(),
            cpu_cores: 16,
            os_version: "Ubuntu 24.04".to_string(),
            feature_flags: vec!["simd-minhash".to_string()],
            git_commit: "abc123".to_string(),
            git_dirty: false,
        };

        let json = to_json_string(&env).unwrap();
        assert!(json.contains("\"rustc_version\":\"1.84.0\""));

        let parsed: EnvironmentInfo = from_json_string(&json).unwrap();
        assert_eq!(parsed.rustc_version, "1.84.0");
        assert_eq!(parsed.cpu_cores, 16);
    }

    #[test]
    fn test_hash_hex_roundtrip() {
        let hash: Hash256 = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
                             0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
                             0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00,
                             0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x07, 0x18];

        let hex = hash_to_hex(&hash);
        assert_eq!(hex.len(), 64);

        let parsed = hex_to_hash(&hex).unwrap();
        assert_eq!(parsed, hash);
    }
}
