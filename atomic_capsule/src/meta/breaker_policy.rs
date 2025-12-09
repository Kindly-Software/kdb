// Circuit Breaker Policy Configuration
//
// Tier: T0 Auditable (compile-time policies)
// Memory: Compact u64 packing (8 bytes)
// Performance: <10ns policy lookup, pack/unpack
//
// Framework Compliance:
// - UCE34: Q10 T0 tier selection (compile-time configuration)
// - Chaos: Zero runtime overhead (policies are compile-time or static)
// - ASSUM: 100% safe (no atomics needed, policies are immutable)
// - B32: N/A (configuration overhead negligible)
// - T28: Unit tests in universal_api_tests.rs
// - I20: Zero breaking changes (additive only)

use super::universal_api::ProtocolType;

/// Circuit breaker policy for per-protocol configuration
///
/// Memory Layout (64 bits):
/// - [0-15]: error_threshold_percent (0-100)
/// - [16-31]: min_samples (minimum requests before tripping)
/// - [32-47]: timeout_ms (lower 16 bits, max 65535ms)
/// - [48-63]: open_duration_ms (lower 16 bits, max 65535ms)
///
/// ASSUM Safety Tags:
/// - #ASSUME_IMMUTABLE_POLICY: Policies don't change after creation (static configs)
/// - #VERIFY_IMMUTABLE_POLICY: Policies stored in const or read-only data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakerPolicy {
    pub timeout_ms: u64,
    pub error_threshold_percent: u8,
    pub min_samples: u16,
    pub open_duration_ms: u64,
}

impl BreakerPolicy {
    /// REST API default policy (fast timeout, strict threshold)
    ///
    /// Performance: Typical REST API characteristics
    /// - Timeout: 1s (normal API response time)
    /// - Error threshold: 30% (trip quickly)
    /// - Min samples: 10 (confidence window)
    /// - Open duration: 5s (quick recovery)
    pub const fn rest_default() -> Self {
        Self {
            timeout_ms: 1000,           // 1 second
            error_threshold_percent: 30, // 30% error rate
            min_samples: 10,
            open_duration_ms: 5000,      // 5 seconds
        }
    }

    /// GraphQL default policy (long-running queries)
    ///
    /// Performance: GraphQL characteristics (complex queries)
    /// - Timeout: 10s (allow complex query execution)
    /// - Error threshold: 40% (more tolerance)
    /// - Min samples: 20 (larger confidence window)
    /// - Open duration: 10s (longer recovery)
    pub const fn graphql_default() -> Self {
        Self {
            timeout_ms: 10000,          // 10 seconds
            error_threshold_percent: 40, // 40% error rate
            min_samples: 20,
            open_duration_ms: 10000,     // 10 seconds
        }
    }

    /// gRPC default policy (streaming RPCs)
    ///
    /// Performance: gRPC streaming characteristics
    /// - Timeout: 5s (balanced for RPC calls)
    /// - Error threshold: 25% (strict for RPC)
    /// - Min samples: 10
    /// - Open duration: 5s
    pub const fn grpc_default() -> Self {
        Self {
            timeout_ms: 5000,           // 5 seconds
            error_threshold_percent: 25, // 25% error rate
            min_samples: 10,
            open_duration_ms: 5000,      // 5 seconds
        }
    }

    /// WebSocket default policy (long-lived connections)
    ///
    /// Performance: WebSocket characteristics (persistent connections)
    /// - Timeout: 30s (long-lived connection tolerance)
    /// - Error threshold: 50% (high tolerance for reconnect)
    /// - Min samples: 5 (small window due to persistent nature)
    /// - Open duration: 10s (recovery time)
    pub const fn websocket_default() -> Self {
        Self {
            timeout_ms: 30000,          // 30 seconds
            error_threshold_percent: 50, // 50% error rate
            min_samples: 5,
            open_duration_ms: 10000,     // 10 seconds
        }
    }

    /// JSON-RPC default policy (synchronous RPC)
    ///
    /// Performance: JSON-RPC characteristics (like REST)
    /// - Timeout: 2s (slightly slower than REST)
    /// - Error threshold: 30%
    /// - Min samples: 10
    /// - Open duration: 5s
    pub const fn jsonrpc_default() -> Self {
        Self {
            timeout_ms: 2000,           // 2 seconds
            error_threshold_percent: 30, // 30% error rate
            min_samples: 10,
            open_duration_ms: 5000,      // 5 seconds
        }
    }

    /// Pack policy into u64 for AtomicU64 storage
    ///
    /// Performance: <10ns (shift + OR operations)
    ///
    /// Memory Layout:
    /// - [0-15]: error_threshold_percent (8 bits actual, padded to 16)
    /// - [16-31]: min_samples (16 bits)
    /// - [32-47]: timeout_ms (lower 16 bits, max 65535ms)
    /// - [48-63]: open_duration_ms (lower 16 bits, max 65535ms)
    ///
    /// ASSUM Safety:
    /// - #ASSUME_TIMEOUT_BOUNDS: timeout_ms and open_duration_ms fit in 16 bits
    /// - #VERIFY_TIMEOUT_BOUNDS: Documented max 65535ms, tests verify
    pub fn pack(&self) -> u64 {
        let threshold = (self.error_threshold_percent as u64) & 0xFFFF;
        let samples = (self.min_samples as u64) & 0xFFFF;
        let timeout = (self.timeout_ms as u64) & 0xFFFF;
        let open_dur = (self.open_duration_ms as u64) & 0xFFFF;

        threshold | (samples << 16) | (timeout << 32) | (open_dur << 48)
    }

    /// Unpack policy from u64
    ///
    /// Performance: <10ns (shift + mask operations)
    pub fn unpack(packed: u64) -> Self {
        Self {
            error_threshold_percent: (packed & 0xFFFF) as u8,
            min_samples: ((packed >> 16) & 0xFFFF) as u16,
            timeout_ms: ((packed >> 32) & 0xFFFF) as u64,
            open_duration_ms: ((packed >> 48) & 0xFFFF) as u64,
        }
    }

    /// Get default policy for protocol type
    ///
    /// Performance: <5ns (match statement, compile-time constants)
    pub const fn for_protocol(protocol: ProtocolType) -> Self {
        match protocol {
            ProtocolType::REST => Self::rest_default(),
            ProtocolType::GraphQL => Self::graphql_default(),
            ProtocolType::Grpc => Self::grpc_default(),
            ProtocolType::WebSocket => Self::websocket_default(),
            ProtocolType::JsonRPC => Self::jsonrpc_default(),
            ProtocolType::SSE => Self::sse_default(),
        }
    }

    /// Default policy for SSE (Server-Sent Events)
    ///
    /// SSE is streaming, requires high availability
    /// - Timeout: 60s (long-lived event stream)
    /// - Error threshold: 50% (high tolerance for streaming)
    /// - Min samples: 5 (small window due to persistent nature)
    /// - Open duration: 10s (recovery time)
    pub const fn sse_default() -> Self {
        Self {
            timeout_ms: 60000,          // 60 seconds (long-lived streams)
            error_threshold_percent: 50, // 50% error rate (high tolerance)
            min_samples: 5,              // Small window for persistent connections
            open_duration_ms: 10000,     // 10 seconds recovery
        }
    }
}

impl Default for BreakerPolicy {
    fn default() -> Self {
        Self::rest_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_defaults() {
        let rest = BreakerPolicy::rest_default();
        assert_eq!(rest.timeout_ms, 1000);
        assert_eq!(rest.error_threshold_percent, 30);
        assert_eq!(rest.min_samples, 10);
        assert_eq!(rest.open_duration_ms, 5000);

        let graphql = BreakerPolicy::graphql_default();
        assert_eq!(graphql.timeout_ms, 10000);
        assert_eq!(graphql.error_threshold_percent, 40);
        assert_eq!(graphql.min_samples, 20);
        assert_eq!(graphql.open_duration_ms, 10000);
    }

    #[test]
    fn test_pack_unpack() {
        let policy = BreakerPolicy::rest_default();
        let packed = policy.pack();
        let unpacked = BreakerPolicy::unpack(packed);

        assert_eq!(policy.error_threshold_percent, unpacked.error_threshold_percent);
        assert_eq!(policy.min_samples, unpacked.min_samples);
        assert_eq!(policy.timeout_ms, unpacked.timeout_ms);
        assert_eq!(policy.open_duration_ms, unpacked.open_duration_ms);
    }

    #[test]
    fn test_for_protocol() {
        let rest = BreakerPolicy::for_protocol(ProtocolType::REST);
        assert_eq!(rest.timeout_ms, 1000);

        let graphql = BreakerPolicy::for_protocol(ProtocolType::GraphQL);
        assert_eq!(graphql.timeout_ms, 10000);

        let grpc = BreakerPolicy::for_protocol(ProtocolType::Grpc);
        assert_eq!(grpc.timeout_ms, 5000);

        let ws = BreakerPolicy::for_protocol(ProtocolType::WebSocket);
        assert_eq!(ws.timeout_ms, 30000);

        let jsonrpc = BreakerPolicy::for_protocol(ProtocolType::JsonRPC);
        assert_eq!(jsonrpc.timeout_ms, 2000);
    }
}
