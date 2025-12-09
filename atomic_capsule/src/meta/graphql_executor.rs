//! GraphQLExecutorCapsule - T1 Atomic GraphQL query executor
//!
//! Simple GraphQL query execution with basic parsing and JSON response formatting.
//! NOT a full GraphQL spec implementation - supports basic query/mutation operations.

use core::sync::atomic::{AtomicU64, Ordering};
use super::{ApiError, ApiErrorKind, ProtocolType};

#[cfg(feature = "std")]
use std::{string::{String, ToString}, vec::Vec, format};

// ============================================================================
// COMPLEXITY LIMITS - DoS PREVENTION
// ============================================================================

/// Maximum query depth (nested braces) to prevent deeply nested queries
///
/// Example attack: query { a { b { c { ... 1000 levels } } } }
const MAX_QUERY_DEPTH: usize = 10;

/// Maximum total field count to prevent unbounded heap allocation
///
/// Example attack: query { field1 field2 field3 ... 10000 fields }
const MAX_FIELD_COUNT: usize = 100;

/// Maximum query size in bytes (10KB) to prevent memory exhaustion
///
/// Example attack: Sending 100MB query strings
const MAX_QUERY_SIZE: usize = 10_000;

/// GraphQL operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}

/// Simple GraphQL query AST node
#[derive(Debug, Clone)]
pub struct QueryNode {
    pub operation: OperationType,
    pub fields: Vec<String>,
}

/// GraphQL executor with atomic statistics tracking
///
/// # ASSUM Tags
/// - #ASSUME_LOCKFREE_STATS: All statistics via AtomicU64, no mutex
/// - #ASSUME_SIMPLE_PARSER: Basic query parsing only, not full GraphQL spec
/// - #ASSUME_JSON_RESPONSE: Always returns JSON format responses
/// - #ASSUME_SCHEMA_PTR_VALID: Schema pointer must be valid when set
/// - #ASSUME_CACHE_ALIGNED: 256B alignment prevents false sharing
#[repr(C, align(256))]
pub struct GraphQLExecutorCapsule {
    /// Query statistics
    query_count: AtomicU64,
    error_count: AtomicU64,
    total_latency_ns: AtomicU64,

    /// Schema pointer (set at registration)
    schema_ptr: AtomicU64,

    /// Mutation count
    mutation_count: AtomicU64,

    /// Average query depth
    avg_depth: AtomicU64,

    /// Reserved for future use
    _reserved: [AtomicU64; 2],

    /// Padding to 256 bytes
    _padding: [u8; 192],
}

impl GraphQLExecutorCapsule {
    /// Create new GraphQL executor
    pub const fn new() -> Self {
        Self {
            query_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            schema_ptr: AtomicU64::new(0),
            mutation_count: AtomicU64::new(0),
            avg_depth: AtomicU64::new(0),
            _reserved: [AtomicU64::new(0), AtomicU64::new(0)],
            _padding: [0u8; 192],
        }
    }

    /// Execute GraphQL query
    ///
    /// # Arguments
    /// * `query` - GraphQL query string (e.g., "query { user { name email } }")
    ///
    /// # Returns
    /// JSON response string or error
    ///
    /// # ASSUM
    /// - #ASSUME_QUERY_VALID_UTF8: Query must be valid UTF-8
    /// - #ASSUME_BOUNDED_QUERY: Query size <= 10KB, depth <= 10, fields <= 100 (DoS prevention)
    ///   #VERIFY: validate_query_complexity() enforces bounds before parsing
    pub fn execute_query(&self, query: &str) -> Result<String, ApiError> {
        let start = self.timestamp_ns();

        // Validate query complexity BEFORE parsing (fail-fast DoS prevention)
        self.validate_query_complexity(query)?;

        // Parse query
        let ast = self.parse_query(query)?;

        // Update statistics
        match ast.operation {
            OperationType::Query => {
                self.query_count.fetch_add(1, Ordering::Relaxed);
            }
            OperationType::Mutation => {
                self.mutation_count.fetch_add(1, Ordering::Relaxed);
            }
            OperationType::Subscription => {
                // Not implemented
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err(ApiError::new(
                    ApiErrorKind::Unsupported,
                    "Subscriptions not supported",
                ));
            }
        }

        // Execute query (simplified - just return mock data)
        let response = self.execute_ast(&ast)?;

        // Record latency
        let latency = self.timestamp_ns() - start;
        self.total_latency_ns.fetch_add(latency, Ordering::Relaxed);

        Ok(response)
    }

    /// Validate query complexity to prevent DoS attacks
    ///
    /// Enforces:
    /// - MAX_QUERY_SIZE: 10KB maximum (prevents memory exhaustion)
    /// - MAX_QUERY_DEPTH: 10 levels maximum (prevents stack overflow)
    /// - MAX_FIELD_COUNT: 100 fields maximum (prevents unbounded heap allocation)
    ///
    /// # ASSUM
    /// - #ASSUME_BOUNDED_QUERY_SIZE: 10KB limit prevents OOM attacks
    ///   #VERIFY: Checked before any parsing or allocation
    /// - #ASSUME_BOUNDED_QUERY_DEPTH: 10 levels prevent deeply nested queries
    ///   #VERIFY: Brace nesting counted before parsing
    /// - #ASSUME_BOUNDED_FIELD_COUNT: 100 fields prevent heap exhaustion
    ///   #VERIFY: Field count estimated before allocation
    fn validate_query_complexity(&self, query: &str) -> Result<(), ApiError> {
        // 1. Check query size (fail-fast, no parsing)
        let query_size = query.len();
        if query_size > MAX_QUERY_SIZE {
            return Err(ApiError::InvalidRequest {
                protocol: ProtocolType::GraphQL,
                reason: format!("Query too large: {} bytes (max: {} bytes)", query_size, MAX_QUERY_SIZE),
            });
        }

        // 2. Check query depth (count nested braces)
        let mut depth = 0;
        let mut max_depth = 0;
        for ch in query.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    if depth > max_depth {
                        max_depth = depth;
                    }
                    if depth > MAX_QUERY_DEPTH {
                        return Err(ApiError::InvalidRequest {
                            protocol: ProtocolType::GraphQL,
                            reason: format!("Query too deep: {} levels (max: {} levels)", depth, MAX_QUERY_DEPTH),
                        });
                    }
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }

        // 3. Estimate field count (count whitespace-separated tokens)
        // This is a conservative upper bound (may over-count, but safe)
        let estimated_field_count = query
            .split(|c: char| c.is_whitespace() || c == '{' || c == '}')
            .filter(|s| !s.is_empty() && !s.starts_with("query") && !s.starts_with("mutation"))
            .count();

        if estimated_field_count > MAX_FIELD_COUNT {
            return Err(ApiError::InvalidRequest {
                protocol: ProtocolType::GraphQL,
                reason: format!("Too many fields: {} (max: {})", estimated_field_count, MAX_FIELD_COUNT),
            });
        }

        Ok(())
    }

    /// Parse GraphQL query into AST
    ///
    /// # ASSUM
    /// - #ASSUME_SIMPLE_SYNTAX: Only supports "query { fields }" and "mutation { fields }"
    /// - #ASSUME_NO_VARIABLES: Variables not supported in this simple implementation
    /// - #ASSUME_VALIDATED_COMPLEXITY: validate_query_complexity() called before this
    ///   #VERIFY: execute_query() enforces validation order
    fn parse_query(&self, query: &str) -> Result<QueryNode, ApiError> {
        let trimmed = query.trim();

        // Detect operation type
        let operation = if trimmed.starts_with("query") {
            OperationType::Query
        } else if trimmed.starts_with("mutation") {
            OperationType::Mutation
        } else if trimmed.starts_with("subscription") {
            OperationType::Subscription
        } else {
            // Default to query if no operation specified
            OperationType::Query
        };

        // Extract fields between { }
        let fields = self.extract_fields(trimmed)?;

        Ok(QueryNode { operation, fields })
    }

    /// Extract field names from query
    ///
    /// # ASSUM
    /// - #ASSUME_SIMPLE_FIELDS: Only extracts top-level field names
    /// - #ASSUME_NO_ALIASES: Field aliases not supported
    fn extract_fields(&self, query: &str) -> Result<Vec<String>, ApiError> {
        // Find first { and last }
        let start = query.find('{').ok_or_else(|| {
            ApiError::new(ApiErrorKind::ParseError, "Missing opening brace")
        })?;
        let end = query.rfind('}').ok_or_else(|| {
            ApiError::new(ApiErrorKind::ParseError, "Missing closing brace")
        })?;

        if start >= end {
            return Err(ApiError::new(ApiErrorKind::ParseError, "Invalid braces"));
        }

        // Extract content between braces
        let content = &query[start + 1..end];

        // Split by whitespace and filter empty
        let fields: Vec<String> = content
            .split(|c: char| c.is_whitespace() || c == '{' || c == '}')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if fields.is_empty() {
            return Err(ApiError::new(ApiErrorKind::ParseError, "No fields found"));
        }

        Ok(fields)
    }

    /// Execute parsed AST
    ///
    /// # ASSUM
    /// - #ASSUME_MOCK_DATA: Returns mock data, not real database queries
    /// - #ASSUME_JSON_FORMAT: Always returns valid JSON
    fn execute_ast(&self, ast: &QueryNode) -> Result<String, ApiError> {
        let mut response = String::from("{\"data\":{");

        for (i, field) in ast.fields.iter().enumerate() {
            if i > 0 {
                response.push_str(",");
            }

            // Generate mock field data based on field name
            response.push_str(&format!("\"{}\":", field));

            match field.as_str() {
                "user" => response.push_str("{\"name\":\"John Doe\",\"email\":\"john@example.com\"}"),
                "users" => response.push_str("[{\"name\":\"Alice\"},{\"name\":\"Bob\"}]"),
                "id" => response.push_str("\"123\""),
                "name" => response.push_str("\"Example\""),
                "count" => response.push_str("42"),
                "active" => response.push_str("true"),
                _ => response.push_str("null"),
            }
        }

        response.push_str("}}");

        Ok(response)
    }

    /// Get current timestamp in nanoseconds
    ///
    /// # ASSUM
    /// - #ASSUME_MONOTONIC_TIME: Uses simple counter for testing
    fn timestamp_ns(&self) -> u64 {
        // In production, use std::time::Instant or platform-specific monotonic clock
        self.query_count.load(Ordering::Relaxed) * 1000
    }

    /// Get query statistics
    pub fn get_stats(&self) -> GraphQLStats {
        GraphQLStats {
            query_count: self.query_count.load(Ordering::Relaxed),
            mutation_count: self.mutation_count.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            total_latency_ns: self.total_latency_ns.load(Ordering::Relaxed),
        }
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.query_count.store(0, Ordering::Relaxed);
        self.mutation_count.store(0, Ordering::Relaxed);
        self.error_count.store(0, Ordering::Relaxed);
        self.total_latency_ns.store(0, Ordering::Relaxed);
    }

    /// Set schema pointer
    ///
    /// # Safety
    /// Caller must ensure pointer remains valid for lifetime of executor
    ///
    /// # ASSUM
    /// - #ASSUME_SCHEMA_PTR_VALID: Pointer must be valid and properly aligned
    pub fn set_schema_ptr(&self, ptr: u64) {
        self.schema_ptr.store(ptr, Ordering::Release);
    }

    /// Get schema pointer
    pub fn get_schema_ptr(&self) -> u64 {
        self.schema_ptr.load(Ordering::Acquire)
    }
}

/// GraphQL statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct GraphQLStats {
    pub query_count: u64,
    pub mutation_count: u64,
    pub error_count: u64,
    pub total_latency_ns: u64,
}

impl GraphQLStats {
    /// Calculate average latency per query
    pub fn avg_latency_ns(&self) -> u64 {
        let total_ops = self.query_count + self.mutation_count;
        if total_ops == 0 {
            0
        } else {
            self.total_latency_ns / total_ops
        }
    }

    /// Calculate error rate (0.0 to 1.0)
    pub fn error_rate(&self) -> f64 {
        let total_ops = self.query_count + self.mutation_count + self.error_count;
        if total_ops == 0 {
            0.0
        } else {
            self.error_count as f64 / total_ops as f64
        }
    }
}

impl Default for GraphQLExecutorCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification
const _: () = assert!(core::mem::size_of::<GraphQLExecutorCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<GraphQLExecutorCapsule>() == 256);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
        assert_eq!(core::mem::size_of::<GraphQLExecutorCapsule>(), 256);
        assert_eq!(core::mem::align_of::<GraphQLExecutorCapsule>(), 256);
    }

    #[test]
    fn test_simple_query() {
        let executor = GraphQLExecutorCapsule::new();
        let result = executor.execute_query("query { user }");
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("\"user\""));
    }

    #[test]
    fn test_mutation() {
        let executor = GraphQLExecutorCapsule::new();
        let result = executor.execute_query("mutation { createUser }");
        assert!(result.is_ok());

        let stats = executor.get_stats();
        assert_eq!(stats.mutation_count, 1);
    }

    #[test]
    fn test_statistics() {
        let executor = GraphQLExecutorCapsule::new();

        let _ = executor.execute_query("query { user }");
        let _ = executor.execute_query("query { users }");

        let stats = executor.get_stats();
        assert_eq!(stats.query_count, 2);
        assert_eq!(stats.error_count, 0);
    }

    #[test]
    fn test_invalid_query() {
        let executor = GraphQLExecutorCapsule::new();
        let result = executor.execute_query("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_fields() {
        let executor = GraphQLExecutorCapsule::new();
        let result = executor.execute_query("query { user name id }");
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("\"user\""));
        assert!(response.contains("\"name\""));
        assert!(response.contains("\"id\""));
    }

    // ========================================================================
    // DoS PREVENTION TESTS (MAX_QUERY_SIZE, MAX_QUERY_DEPTH, MAX_FIELD_COUNT)
    // ========================================================================

    #[test]
    fn test_dos_prevention_query_too_large() {
        let executor = GraphQLExecutorCapsule::new();

        // Create query > 10KB (MAX_QUERY_SIZE)
        let large_query = format!("query {{ {} }}", "a ".repeat(5_001)); // ~10KB+
        let result = executor.execute_query(&large_query);

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ApiError::InvalidRequest { protocol, reason } => {
                assert_eq!(protocol, ProtocolType::GraphQL);
                assert!(reason.contains("Query too large"));
                assert!(reason.contains("10000"));
            }
            _ => panic!("Expected InvalidRequest error, got: {:?}", err),
        }
    }

    #[test]
    fn test_dos_prevention_query_too_deep() {
        let executor = GraphQLExecutorCapsule::new();

        // Create query with depth > 10 (MAX_QUERY_DEPTH)
        // Example: query { a { b { c { d { e { f { g { h { i { j { k } } } } } } } } } }
        let mut deep_query = String::from("query ");
        for _ in 0..12 {
            deep_query.push_str("{ ");
        }
        deep_query.push_str("field ");
        for _ in 0..12 {
            deep_query.push_str("} ");
        }

        let result = executor.execute_query(&deep_query);

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ApiError::InvalidRequest { protocol, reason } => {
                assert_eq!(protocol, ProtocolType::GraphQL);
                assert!(reason.contains("Query too deep"));
                assert!(reason.contains("10"));
            }
            _ => panic!("Expected InvalidRequest error, got: {:?}", err),
        }
    }

    #[test]
    fn test_dos_prevention_too_many_fields() {
        let executor = GraphQLExecutorCapsule::new();

        // Create query with > 100 fields (MAX_FIELD_COUNT)
        let mut many_fields_query = String::from("query { ");
        for i in 0..110 {
            many_fields_query.push_str(&format!("field{} ", i));
        }
        many_fields_query.push_str("}");

        let result = executor.execute_query(&many_fields_query);

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ApiError::InvalidRequest { protocol, reason } => {
                assert_eq!(protocol, ProtocolType::GraphQL);
                assert!(reason.contains("Too many fields"));
                assert!(reason.contains("100"));
            }
            _ => panic!("Expected InvalidRequest error, got: {:?}", err),
        }
    }

    #[test]
    fn test_dos_prevention_valid_query_at_limits() {
        let executor = GraphQLExecutorCapsule::new();

        // Test query size just under limit (~9KB)
        let large_valid_query = format!("query {{ {} }}", "a ".repeat(4_000)); // ~8KB
        let result = executor.execute_query(&large_valid_query);
        assert!(result.is_ok()); // Should succeed

        // Test query depth exactly at limit (10 levels)
        let mut depth_10_query = String::from("query ");
        for _ in 0..10 {
            depth_10_query.push_str("{ ");
        }
        depth_10_query.push_str("field ");
        for _ in 0..10 {
            depth_10_query.push_str("} ");
        }
        let result = executor.execute_query(&depth_10_query);
        assert!(result.is_ok()); // Should succeed

        // Test field count just under limit (90 fields)
        let mut many_fields_valid = String::from("query { ");
        for i in 0..90 {
            many_fields_valid.push_str(&format!("field{} ", i));
        }
        many_fields_valid.push_str("}");
        let result = executor.execute_query(&many_fields_valid);
        assert!(result.is_ok()); // Should succeed
    }

    #[test]
    fn test_dos_prevention_error_messages() {
        let executor = GraphQLExecutorCapsule::new();

        // Test error message includes actual values
        let large_query = format!("query {{ {} }}", "a ".repeat(5_001));
        let result = executor.execute_query(&large_query);
        let err = result.unwrap_err();
        match err {
            ApiError::InvalidRequest { reason, .. } => {
                assert!(reason.contains("bytes"));
                assert!(reason.contains("max"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }

        // Test depth error message
        let mut deep_query = String::from("query ");
        for _ in 0..12 {
            deep_query.push_str("{ ");
        }
        deep_query.push_str("field ");
        for _ in 0..12 {
            deep_query.push_str("} ");
        }
        let result = executor.execute_query(&deep_query);
        let err = result.unwrap_err();
        match err {
            ApiError::InvalidRequest { reason, .. } => {
                assert!(reason.contains("levels"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }

        // Test field count error message
        let mut many_fields = String::from("query { ");
        for i in 0..110 {
            many_fields.push_str(&format!("field{} ", i));
        }
        many_fields.push_str("}");
        let result = executor.execute_query(&many_fields);
        let err = result.unwrap_err();
        match err {
            ApiError::InvalidRequest { reason, .. } => {
                assert!(reason.contains("fields"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }
}
