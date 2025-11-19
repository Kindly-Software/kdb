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
use atomic_capsule::serialize::{JsonWriterCapsule, JsonWriterError, JsonWriterResult, JsonParserCapsule, JsonValue, JsonParserError};

// ============================================================================
// WriteJson Trait (simplified, uses JsonWriterCapsule directly)
// ============================================================================

pub trait WriteJson {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()>;
}

// Reexports for compatibility
pub type JsonError = JsonWriterError;

// ============================================================================
// WriteJson Implementations
// ============================================================================

impl WriteJson for String {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        writer.write_string(self)
    }
}

impl WriteJson for usize {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        writer.write_u64(*self as u64)
    }
}

impl WriteJson for bool {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        writer.write_bool(*self)
    }
}

impl WriteJson for f64 {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        // Use write_literal for f64 (no native method in JsonWriterCapsule)
        writer.write_literal(&format!("{}", self))
    }
}

impl<T: WriteJson> WriteJson for Vec<T> {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        writer.start_array()?;
        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                writer.write_comma()?;
            }
            item.write_json(writer)?;
        }
        writer.end_array()
    }
}

impl<T: WriteJson> WriteJson for Option<T> {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        match self {
            Some(value) => value.write_json(writer),
            None => writer.write_null(),
        }
    }
}

impl WriteJson for EnvironmentInfo {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        write_field(writer, "rustc_version", &self.rustc_version, true)?;
        write_field(writer, "cpu_model", &self.cpu_model, false)?;
        write_field(writer, "cpu_cores", &self.cpu_cores, false)?;
        write_field(writer, "os_version", &self.os_version, false)?;
        write_field(writer, "feature_flags", &self.feature_flags, false)?;
        write_field(writer, "git_commit", &self.git_commit, false)?;
        write_field(writer, "git_dirty", &self.git_dirty, false)?;
        Ok(())
    }
}

impl WriteJson for AccuracyMetrics {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        write_field(writer, "recall", &self.recall, true)?;
        write_field(writer, "precision", &self.precision, false)?;
        write_field(writer, "f1", &self.f1, false)?;
        write_field(writer, "true_positives", &self.true_positives, false)?;
        write_field(writer, "false_positives", &self.false_positives, false)?;
        write_field(writer, "true_negatives", &self.true_negatives, false)?;
        write_field(writer, "false_negatives", &self.false_negatives, false)?;
        Ok(())
    }
}

impl WriteJson for BenchmarkResult {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        write_field(writer, "throughput_docs_per_sec", &self.throughput_docs_per_sec, true)?;
        write_field(writer, "latency_p50_us", &self.latency_p50_us, false)?;
        write_field(writer, "latency_p95_us", &self.latency_p95_us, false)?;
        write_field(writer, "latency_p99_us", &self.latency_p99_us, false)?;
        write_field(writer, "latency_mean_us", &self.latency_mean_us, false)?;
        write_field(writer, "latency_stddev_us", &self.latency_stddev_us, false)?;
        write_field(writer, "ci_95_lower_us", &self.ci_95_lower_us, false)?;
        write_field(writer, "ci_95_upper_us", &self.ci_95_upper_us, false)?;
        write_field(writer, "accuracy", &self.accuracy, false)?;
        Ok(())
    }
}

impl WriteJson for BenchmarkConfig {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        write_field(writer, "dataset", &self.dataset, true)?;
        write_field(writer, "threads", &self.threads, false)?;
        write_field(writer, "features", &self.features, false)?;
        write_field(writer, "warmup_iterations", &self.warmup_iterations, false)?;
        write_field(writer, "measurement_iterations", &self.measurement_iterations, false)?;
        Ok(())
    }
}

impl WriteJson for BenchmarkAuditEntry {
    fn write_json(&self, writer: &JsonWriterCapsule) -> JsonWriterResult<()> {
        write_field(writer, "benchmark_id", &self.benchmark_id, true)?;
        write_field(writer, "timestamp", &(self.timestamp as usize), false)?;

        // Environment (nested object)
        writer.write_comma()?;
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
        write_field(writer, "input_hash", &hash_to_hex(&self.input_hash), false)?;

        // Result (nested object)
        writer.write_comma()?;
        writer.write_string("result")?;
        writer.write_colon()?;
        writer.start_object()?;
        self.result.write_json(writer)?;
        writer.end_object()?;

        // More hashes
        write_field(writer, "result_hash", &hash_to_hex(&self.result_hash), false)?;
        write_field(writer, "prev_audit_hash", &hash_to_hex(&self.prev_audit_hash), false)?;
        write_field(writer, "audit_hash", &hash_to_hex(&self.audit_hash), false)?;

        Ok(())
    }
}

// ============================================================================
// ParseJson Implementations (simplified, direct JSON value matching)
// ============================================================================

pub trait ParseJson: Sized {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError>;
}

impl ParseJson for String {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::String(s) => Ok(s.clone()),
            _ => Err(JsonParserError::TypeMismatch("Expected string".to_string())),
        }
    }
}

impl ParseJson for usize {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::Number(n) => {
                if *n >= 0.0 && n.fract() == 0.0 {
                    Ok(*n as usize)
                } else {
                    Err(JsonParserError::TypeMismatch("Expected non-negative integer".to_string()))
                }
            }
            _ => Err(JsonParserError::TypeMismatch("Expected number".to_string())),
        }
    }
}

impl ParseJson for u64 {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::Number(n) => {
                if *n >= 0.0 && n.fract() == 0.0 {
                    Ok(*n as u64)
                } else {
                    Err(JsonParserError::TypeMismatch("Expected non-negative integer".to_string()))
                }
            }
            _ => Err(JsonParserError::TypeMismatch("Expected number".to_string())),
        }
    }
}

impl ParseJson for bool {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::Bool(b) => Ok(*b),
            _ => Err(JsonParserError::TypeMismatch("Expected bool".to_string())),
        }
    }
}

impl ParseJson for f64 {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::Number(n) => Ok(*n),
            _ => Err(JsonParserError::TypeMismatch("Expected number".to_string())),
        }
    }
}

impl<T: ParseJson> ParseJson for Vec<T> {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::Array(items) => items.iter().map(|item| T::parse_json(item)).collect(),
            _ => Err(JsonParserError::TypeMismatch("Expected array".to_string())),
        }
    }
}

impl ParseJson for EnvironmentInfo {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::Object(fields) => {
                Ok(EnvironmentInfo {
                    rustc_version: String::parse_json(get_field_required(fields, "rustc_version")?)?,
                    cpu_model: String::parse_json(get_field_required(fields, "cpu_model")?)?,
                    cpu_cores: usize::parse_json(get_field_required(fields, "cpu_cores")?)?,
                    os_version: String::parse_json(get_field_required(fields, "os_version")?)?,
                    feature_flags: Vec::<String>::parse_json(get_field_required(fields, "feature_flags")?)?,
                    git_commit: String::parse_json(get_field_required(fields, "git_commit")?)?,
                    git_dirty: bool::parse_json(get_field_required(fields, "git_dirty")?)?,
                })
            }
            _ => Err(JsonParserError::TypeMismatch("Expected object for EnvironmentInfo".to_string())),
        }
    }
}

impl ParseJson for AccuracyMetrics {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::Object(fields) => {
                Ok(AccuracyMetrics {
                    recall: f64::parse_json(get_field_required(fields, "recall")?)?,
                    precision: f64::parse_json(get_field_required(fields, "precision")?)?,
                    f1: f64::parse_json(get_field_required(fields, "f1")?)?,
                    true_positives: usize::parse_json(get_field_required(fields, "true_positives")?)?,
                    false_positives: usize::parse_json(get_field_required(fields, "false_positives")?)?,
                    true_negatives: usize::parse_json(get_field_required(fields, "true_negatives")?)?,
                    false_negatives: usize::parse_json(get_field_required(fields, "false_negatives")?)?,
                })
            }
            _ => Err(JsonParserError::TypeMismatch("Expected object for AccuracyMetrics".to_string())),
        }
    }
}

impl ParseJson for BenchmarkResult {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::Object(fields) => {
                let accuracy = match get_field(fields, "accuracy") {
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
            _ => Err(JsonParserError::TypeMismatch("Expected object for BenchmarkResult".to_string())),
        }
    }
}

impl ParseJson for BenchmarkConfig {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
        match value {
            JsonValue::Object(fields) => {
                Ok(BenchmarkConfig {
                    dataset: String::parse_json(get_field_required(fields, "dataset")?)?,
                    threads: usize::parse_json(get_field_required(fields, "threads")?)?,
                    features: Vec::<String>::parse_json(get_field_required(fields, "features")?)?,
                    warmup_iterations: usize::parse_json(get_field_required(fields, "warmup_iterations")?)?,
                    measurement_iterations: usize::parse_json(get_field_required(fields, "measurement_iterations")?)?,
                })
            }
            _ => Err(JsonParserError::TypeMismatch("Expected object for BenchmarkConfig".to_string())),
        }
    }
}

impl ParseJson for BenchmarkAuditEntry {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonParserError> {
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
            _ => Err(JsonParserError::TypeMismatch("Expected object for BenchmarkAuditEntry".to_string())),
        }
    }
}

// ============================================================================
// Public API (serde-compatible interface)
// ============================================================================

/// Serialize to JSON string (serde_json::to_string replacement)
pub fn to_json_string<T: WriteJson>(value: &T) -> Result<String, JsonWriterError> {
    let writer = JsonWriterCapsule::new();
    writer.start_object()?;
    value.write_json(&writer)?;
    writer.end_object()?;
    writer.finalize()
}

/// Serialize to JSON bytes (serde_json::to_vec replacement)
pub fn to_json_vec<T: WriteJson>(value: &T) -> Result<Vec<u8>, JsonWriterError> {
    let json = to_json_string(value)?;
    Ok(json.into_bytes())
}

/// Deserialize from JSON string (serde_json::from_str replacement)
pub fn from_json_string<T: ParseJson>(json: &str) -> Result<T, JsonParserError> {
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

fn hex_to_hash(hex: &str) -> Result<Hash256, JsonParserError> {
    if hex.len() != 64 {
        return Err(JsonParserError::InvalidFormat(format!("Invalid hex hash length: {} (expected 64)", hex.len())));
    }

    let mut hash = [0u8; 32];
    for i in 0..32 {
        let byte_str = &hex[i * 2..i * 2 + 2];
        hash[i] = u8::from_str_radix(byte_str, 16)
            .map_err(|_| JsonParserError::InvalidFormat(format!("Invalid hex digit: {}", byte_str)))?;
    }
    Ok(hash)
}

fn write_field<T: WriteJson>(
    writer: &JsonWriterCapsule,
    name: &str,
    value: &T,
    first: bool,
) -> JsonWriterResult<()> {
    if !first {
        writer.write_comma()?;
    }
    writer.write_string(name)?;
    writer.write_colon()?;
    value.write_json(writer)
}

fn get_field<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
) -> Option<&'a JsonValue> {
    fields.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
}

fn get_field_required<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
) -> Result<&'a JsonValue, JsonParserError> {
    get_field(fields, name)
        .ok_or_else(|| JsonParserError::MissingField(name.to_string()))
}
