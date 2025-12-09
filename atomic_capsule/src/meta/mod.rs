// Meta-coordination module: Universal API composition patterns
//
// Architecture: T6 Mixed tier - orchestrates T1/T2/T5/T8 primitives
// Status: Week 4 Implementation (Circuit breaker + Week 2 REST/JSON-RPC)
// Framework: UCE34 Q1-Q34, Chaos 100% lockfree

pub mod universal_api;
pub mod breaker_policy;
#[cfg(feature = "std")]
pub mod fallback;

// Week 2: REST and JSON-RPC protocol handlers
pub mod rest_handler;
pub mod jsonrpc_handler;
pub mod adapters;

// Week 3: GraphQL, gRPC, WebSocket protocol handlers
pub mod graphql_executor;
pub mod grpc_multiplexer;
pub mod websocket_state;

// Week 4: SSE (Server-Sent Events) protocol handler
pub mod sse_handler;

// P2-1: GraphQL Federation support (T2+T4 SIMD + Batch)
#[cfg(feature = "graphql-federation")]
pub mod graphql_federation;

// P2-3: Telemetry aggregation (Prometheus exporter)
#[cfg(feature = "telemetry-prometheus")]
pub mod telemetry;

// P2-4: Q34 Audit Trail System (SOX/SOC2/GDPR/HIPAA compliance)
#[cfg(feature = "audit-q34")]
pub mod audit_trail;

// P2-5: HTTP/3 Adapter (bridges QUIC to UniversalRequest)
#[cfg(feature = "http3-support")]
pub mod http3_adapter;

pub use universal_api::{
    UniversalApiMetaCapsule,
    UniversalRequest,
    UniversalResponse,
    ProtocolType,
    ApiError,
    ApiErrorKind,
    MiddlewareFn,
    MiddlewareError,
};

pub use breaker_policy::BreakerPolicy;

#[cfg(feature = "std")]
pub use fallback::FallbackResponse;

// Week 2 exports
pub use rest_handler::{RestHandler, RestResponse};
pub use jsonrpc_handler::{JsonRpcHandler, JsonRpcResponse, MethodRegistry};
pub use adapters::{
    HttpUniversalRequest,
    HttpUniversalResponse,
    JsonRpcUniversalRequest,
    JsonRpcUniversalResponse,
};

// Week 3 exports
pub use graphql_executor::{GraphQLExecutorCapsule, GraphQLStats, OperationType, QueryNode};
pub use grpc_multiplexer::{GrpcMultiplexer, GrpcStats, RpcType, ProtoField, WireType};
pub use websocket_state::{WebSocketStateCapsule, WsStats, WsState, WsOpcode, WebSocketFrame};

// Week 4 exports (SSE)
pub use sse_handler::{SseHandler, SseEventCapsule, SseStreamCapsule, SseConnectionState, SseResponse};

// P2-1 exports (GraphQL Federation)
#[cfg(feature = "graphql-federation")]
pub use graphql_federation::{
    FederatedSchemaCapsule,
    FederatedQueryPlannerCapsule,
    FederatedServiceRegistryCapsule,
    KeyDirective,
    ExtendsDirective,
    EntityDefinition,
    QueryPlan,
    ServiceRequest,
    QueryPlannerStats,
    ServiceRegistryStats,
};

// P2-3 exports (Telemetry)
#[cfg(feature = "telemetry-prometheus")]
pub use telemetry::{
    TelemetryAggregatorCapsule,
    TelemetrySnapshot,
    PrometheusExporterCapsule,
};

// P2-4 exports (Q34 Audit Trail)
#[cfg(feature = "audit-q34")]
pub use audit_trail::{
    AuditRecordCapsule,
    AuditTrailCapsule,
    AuditPolicyCapsule,
    AuditActionType,
};

// P2-5 exports (HTTP/3 Adapter)
#[cfg(feature = "http3-support")]
pub use http3_adapter::{Http3Adapter, Http3UniversalRequest};
