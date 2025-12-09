# KindlyAPI Data Structures: Complete Reference

**Version**: 1.0
**Date**: 2025-10-03
**Purpose**: Comprehensive data structure specifications for intelligent MCP generation

---

## Overview

This document defines all data structures used in KindlyAPI's intelligent generation layer. These structures support the 10 intelligent features while integrating with the capsule runtime foundation.

**Design Principles:**
1. **Serialize-friendly**: All structures support serde for persistence
2. **Cache-aware**: Critical structures aligned to cache lines where needed
3. **Type-safe**: Use newtype wrappers for IDs to prevent mixing
4. **Audit-ready**: All mutations produce ALE-128 events

---

## Core Identity Types

### Type-Safe IDs (Newtype Wrappers)

```rust
use serde::{Deserialize, Serialize};
use std::fmt;

/// Integration ID (int_xxxxxxxxxxxx - 96-bit AID)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntegrationId([u8; 12]);

impl IntegrationId {
    pub fn new() -> Self {
        // AID-96 generation: time | node | counter | class
        // See atomic_id_96 crate for implementation
        Self(aid_96::generate(AidClass::Integration))
    }

    pub fn from_str(s: &str) -> Result<Self, ParseError> {
        // Parse "int_base32encoded" format
        let bytes = base32::decode(&s[4..])?;  // Skip "int_" prefix
        Ok(Self(bytes.try_into()?))
    }
}

impl fmt::Display for IntegrationId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "int_{}", base32::encode(&self.0))
    }
}

/// Endpoint ID (hashed from integration + path + method)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EndpointId(u64);

impl EndpointId {
    pub fn new(integration: IntegrationId, method: HttpMethod, path: &str) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        integration.hash(&mut hasher);
        method.hash(&mut hasher);
        path.hash(&mut hasher);
        Self(hasher.finish())
    }
}

/// Workflow ID (wf_xxxxxxxxxxxx)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowId([u8; 12]);

/// Object Type (customer, subscription, invoice, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectType(String);

impl ObjectType {
    pub fn customer() -> Self { Self("customer".into()) }
    pub fn subscription() -> Self { Self("subscription".into()) }
    pub fn invoice() -> Self { Self("invoice".into()) }
}
```

---

## Feature 1: Endpoint Relationships

### Dependency Graph

```rust
use petgraph::graph::{DiGraph, NodeIndex};

/// Directed graph of endpoint dependencies
pub struct DependencyGraph {
    graph: DiGraph<Endpoint, DependencyEdge>,
    endpoint_to_node: HashMap<EndpointId, NodeIndex>,
}

/// Node in dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub id: EndpointId,
    pub integration_id: IntegrationId,
    pub method: HttpMethod,
    pub path: String,
    pub operation_id: Option<String>,
    pub parameters: Vec<Parameter>,
    pub response_schema: Option<JsonSchema>,
}

/// Edge representing dependency between endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub dependency_type: DependencyType,
    pub parameter_flow: Vec<ParameterFlow>,
    pub confidence: f64,  // 0.0-1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    /// A must complete before B (create_customer → create_subscription)
    Sequential,
    /// A and B can run concurrently (get_customer || get_payment_methods)
    Parallel,
    /// B only runs if A succeeds with specific result
    Conditional,
    /// B is idempotent and can retry
    Idempotent,
}

/// How parameter flows from one endpoint to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterFlow {
    pub source_field: String,      // Response field from source endpoint
    pub target_parameter: String,  // Parameter name in target endpoint
    pub transform: Option<ParameterTransform>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterTransform {
    Identity,                        // value → value
    Extract { field: String },       // {id: "123"} → "123"
    Wrap { wrapper: String },        // "123" → {customer: "123"}
    Custom { function: String },     // Named transform function
}

impl DependencyGraph {
    /// Find all downstream dependencies of an endpoint
    pub fn downstream(&self, endpoint_id: EndpointId) -> Vec<EndpointId> {
        let node = self.endpoint_to_node.get(&endpoint_id)?;
        self.graph
            .neighbors(*node)
            .map(|idx| self.graph[idx].id)
            .collect()
    }

    /// Find shortest path between two endpoints
    pub fn path(&self, from: EndpointId, to: EndpointId) -> Option<Vec<EndpointId>> {
        use petgraph::algo::dijkstra;

        let from_node = self.endpoint_to_node.get(&from)?;
        let to_node = self.endpoint_to_node.get(&to)?;

        let path_map = dijkstra(&self.graph, *from_node, Some(*to_node), |_| 1);
        // Reconstruct path from path_map
        // ...
    }
}
```

### Workflow Pattern

```rust
/// Detected common sequence of API calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPattern {
    pub id: WorkflowId,
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    pub frequency: f64,    // 0.0-1.0 (87% of users follow this pattern)
    pub confidence: f64,   // 0.0-1.0 (statistical significance)
    pub sample_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub endpoint_id: EndpointId,
    pub optional: bool,
    pub retry_on_failure: bool,
}

/// Generated composite tool from workflow pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeTool {
    pub name: String,
    pub description: String,
    pub input_schema: JsonSchema,
    pub execution_plan: ExecutionPlan,
    pub source_pattern: WorkflowId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub steps: Vec<ExecutionStep>,
    pub atomic: bool,           // All-or-nothing execution
    pub rollback_on_failure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub endpoint_id: EndpointId,
    pub parameter_bindings: HashMap<String, ParameterBinding>,
    pub rollback_action: Option<RollbackAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterBinding {
    /// Static value from input
    Static(serde_json::Value),
    /// Value from input field
    FromInput { field: String },
    /// Value from previous step's response
    FromStep { step_index: usize, field: String },
    /// Auto-inferred from context
    Inferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackAction {
    /// Delete created object
    DeleteObject {
        endpoint_id: EndpointId,
        id_field: String,  // Field containing object ID
    },
    /// Update object to undo changes
    UpdateObject {
        endpoint_id: EndpointId,
        id_field: String,
        params: HashMap<String, serde_json::Value>,
    },
    /// No rollback needed (read-only operation)
    NoOp,
}
```

---

## Feature 2: Parameter Inference

### Context Tracker

```rust
use std::collections::VecDeque;

/// Tracks recent API calls for parameter inference
pub struct InferenceContext {
    /// Last 100 API calls (ring buffer)
    pub recent_calls: VecDeque<CallContext>,
    /// Recently created objects by type
    pub created_objects: HashMap<ObjectType, Vec<ObjectRef>>,
    /// Smart defaults per integration
    pub defaults: HashMap<IntegrationId, SmartDefaults>,
    /// Environment (test/production)
    pub environment: Environment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallContext {
    pub integration_id: IntegrationId,
    pub endpoint_id: EndpointId,
    pub params: HashMap<String, serde_json::Value>,
    pub response: ApiResponse,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub status: u16,
    pub body: serde_json::Value,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRef {
    pub id: serde_json::Value,
    pub created_at: std::time::Instant,
    pub source_endpoint: EndpointId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    Test,
    Production,
}

impl InferenceContext {
    const MAX_RECENT_CALLS: usize = 100;

    /// Track a completed API call
    pub fn track_call(&mut self, call: CallContext) {
        // Add to recent calls (ring buffer)
        if self.recent_calls.len() >= Self::MAX_RECENT_CALLS {
            self.recent_calls.pop_front();
        }
        self.recent_calls.push_back(call.clone());

        // Extract created objects from response
        if call.response.status == 201 {  // Created
            if let Some(obj_type) = Self::infer_object_type(&call) {
                if let Some(id) = call.response.body.get("id") {
                    self.created_objects
                        .entry(obj_type)
                        .or_default()
                        .push(ObjectRef {
                            id: id.clone(),
                            created_at: call.timestamp,
                            source_endpoint: call.endpoint_id,
                        });
                }
            }
        }
    }

    fn infer_object_type(call: &CallContext) -> Option<ObjectType> {
        // Heuristic: extract from endpoint path
        // POST /v1/customers → customer
        // POST /v1/subscriptions → subscription
        // ...
    }
}
```

### Inference Rules

```rust
/// Rule for inferring parameter values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRule {
    pub parameter_pattern: ParameterPattern,
    pub source: InferenceSource,
    pub confidence: f64,  // 0.0-1.0
    pub priority: u8,     // Higher priority rules checked first
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterPattern {
    /// Exact parameter name match
    Exact(String),
    /// Regex pattern
    Regex(String),
    /// Parameter with specific type
    Typed { name: String, json_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InferenceSource {
    /// Use last created object of this type
    LastCreatedObject {
        object_type: ObjectType,
        field: Option<String>,  // Extract specific field, or use whole object
    },
    /// Use result from previous call in context
    PreviousCallResponse {
        endpoint_pattern: String,
        field: String,
    },
    /// Smart default value
    SmartDefault {
        value: serde_json::Value,
    },
    /// Contextual pattern (e.g., currency based on user location)
    ContextPattern {
        pattern_name: String,
    },
}

/// Inferred parameter value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferredValue {
    pub value: serde_json::Value,
    pub source: InferenceSource,
    pub confidence: f64,
    pub explanation: String,  // Human-readable: "Used customer ID from create_customer call 2s ago"
}

/// Smart defaults per integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartDefaults {
    pub defaults: HashMap<String, serde_json::Value>,
    pub environment_aware: bool,
}
```

---

## Feature 3: Multi-API Orchestration

### Workflow Execution

```rust
/// Multi-API workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiApiWorkflow {
    pub id: WorkflowId,
    pub name: String,
    pub steps: Vec<MultiApiStep>,
    pub atomic: bool,
    pub timeout: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiApiStep {
    pub integration_id: IntegrationId,
    pub endpoint_id: EndpointId,
    pub params: HashMap<String, ParameterBinding>,
    pub optional: bool,
    pub rollback_action: Option<RollbackAction>,
}

/// Workflow execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub workflow_id: WorkflowId,
    pub execution_id: ExecutionId,
    pub started_at: std::time::Instant,
    pub status: WorkflowStatus,
    pub completed_steps: Vec<StepResult>,
    pub current_step: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    InProgress,
    Committed,
    RolledBack,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_index: usize,
    pub endpoint_id: EndpointId,
    pub params_used: HashMap<String, serde_json::Value>,
    pub response: ApiResponse,
    pub latency: std::time::Duration,
    pub rollback_action: Option<RollbackAction>,
}

/// Unique execution ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId([u8; 12]);
```

### Transaction Manager

```rust
/// Manages atomic multi-API transactions
pub struct TransactionManager {
    active_executions: HashMap<ExecutionId, WorkflowExecution>,
    committed: Vec<ExecutionId>,
    rolled_back: Vec<ExecutionId>,
}

impl TransactionManager {
    /// Start new workflow execution
    pub fn begin(&mut self, workflow: MultiApiWorkflow) -> ExecutionId {
        let execution_id = ExecutionId::new();
        let execution = WorkflowExecution {
            workflow_id: workflow.id,
            execution_id,
            started_at: std::time::Instant::now(),
            status: WorkflowStatus::InProgress,
            completed_steps: Vec::new(),
            current_step: Some(0),
        };
        self.active_executions.insert(execution_id, execution);
        execution_id
    }

    /// Record step completion
    pub fn record_step(&mut self, execution_id: ExecutionId, result: StepResult) {
        if let Some(execution) = self.active_executions.get_mut(&execution_id) {
            execution.completed_steps.push(result);
            execution.current_step = execution.current_step.map(|i| i + 1);
        }
    }

    /// Commit workflow (all steps succeeded)
    pub fn commit(&mut self, execution_id: ExecutionId) -> Result<(), TransactionError> {
        let execution = self.active_executions.remove(&execution_id)
            .ok_or(TransactionError::ExecutionNotFound)?;

        // Write MULTI_API_WORKFLOW event to ALE-128
        self.log_commit(&execution);

        self.committed.push(execution_id);
        Ok(())
    }

    /// Rollback workflow (one step failed)
    pub async fn rollback(&mut self, execution_id: ExecutionId) -> Result<(), TransactionError> {
        let execution = self.active_executions.remove(&execution_id)
            .ok_or(TransactionError::ExecutionNotFound)?;

        // Execute rollback actions in reverse order
        for step in execution.completed_steps.iter().rev() {
            if let Some(rollback) = &step.rollback_action {
                self.execute_rollback(rollback).await?;
            }
        }

        self.rolled_back.push(execution_id);
        Ok(())
    }
}
```

---

## Feature 4: Error Recovery

### OAuth Token State

```rust
use std::sync::atomic::{AtomicBool, Ordering};

/// OAuth token state per integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenState {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,  // "Bearer", etc.
    pub expires_at: std::time::Instant,
    pub scopes: Vec<String>,
    #[serde(skip)]
    pub refresh_in_progress: Arc<AtomicBool>,
}

impl TokenState {
    /// Check if token is expired or expiring soon
    pub fn needs_refresh(&self, threshold: std::time::Duration) -> bool {
        self.expires_at.saturating_duration_since(std::time::Instant::now()) < threshold
    }

    /// Check if refresh is already in progress (avoid concurrent refreshes)
    pub fn try_begin_refresh(&self) -> bool {
        self.refresh_in_progress.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).is_ok()
    }
}

/// OAuth configuration per integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub pkce_required: bool,
}
```

### Endpoint Migration

```rust
/// Mapping from deprecated endpoint to new version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPath {
    pub old_endpoint: EndpointId,
    pub new_endpoint: EndpointId,
    pub old_path: String,
    pub new_path: String,
    pub parameter_mappings: Vec<ParameterMapping>,
    pub breaking_changes: Vec<BreakingChange>,
    pub deprecation_date: Option<chrono::DateTime<chrono::Utc>>,
    pub sunset_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterMapping {
    pub old_name: String,
    pub new_name: String,
    pub transform: ParameterTransform,
    pub required_in_old: bool,
    pub required_in_new: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BreakingChange {
    ParameterRemoved { name: String },
    ParameterTypeChanged { name: String, old_type: String, new_type: String },
    ResponseSchemaChanged { description: String },
    BehaviorChanged { description: String },
}

/// Migration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigratedCall {
    pub endpoint_id: EndpointId,
    pub params: HashMap<String, serde_json::Value>,
    pub warnings: Vec<String>,
}
```

### Retry Policy

```rust
/// Retry policy per error type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub strategies: HashMap<ErrorType, RetryStrategy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorType {
    Auth,         // 401, 403
    RateLimit,    // 429
    ServerError,  // 5xx
    Timeout,      // Network timeout
    ClientError,  // 400, 404
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryStrategy {
    Exponential {
        max_attempts: u8,
        base_delay: std::time::Duration,
        max_delay: std::time::Duration,
    },
    Linear {
        max_attempts: u8,
        delay: std::time::Duration,
    },
    NoRetry,
}

/// Retry state for a specific call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryState {
    pub attempt: u8,
    pub next_retry_at: Option<std::time::Instant>,
    pub original_error: ApiError,
}
```

---

## Feature 5: Response Normalization

### Normalization Rules

```rust
/// Schema mapping rules per integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationRules {
    pub field_mappings: Vec<FieldMapping>,
    pub type_coercions: Vec<TypeCoercion>,
    pub enrichments: Vec<Enrichment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    pub source_field: String,
    pub target_field: String,
    pub transform: Option<FieldTransform>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldTransform {
    Identity,
    Extract { path: String },  // JSON path extraction
    Wrap { wrapper: String },
    Custom { function: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCoercion {
    pub field: String,
    pub from_type: JsonType,
    pub to_type: JsonType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsonType {
    String,
    Number,
    Boolean,
    Array,
    Object,
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enrichment {
    pub target_field: String,
    pub value_source: EnrichmentSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnrichmentSource {
    Constant(serde_json::Value),
    FromContext { field: String },
    Computed { function: String },
}
```

---

## Feature 6: Intelligent Cache

### Cache Entry

```rust
/// Cached API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: CacheKey,
    pub value: serde_json::Value,
    pub cached_at: std::time::Instant,
    pub expires_at: Option<std::time::Instant>,
    pub etag: Option<String>,
    pub invalidated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    pub integration_id: IntegrationId,
    pub endpoint_id: EndpointId,
    pub params_hash: u64,  // Hash of request parameters
}

impl CacheKey {
    pub fn new(integration_id: IntegrationId, endpoint_id: EndpointId, params: &HashMap<String, serde_json::Value>) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        // Sort parameters for consistent hashing
        let mut sorted_params: Vec<_> = params.iter().collect();
        sorted_params.sort_by_key(|(k, _)| *k);
        sorted_params.hash(&mut hasher);

        Self {
            integration_id,
            endpoint_id,
            params_hash: hasher.finish(),
        }
    }
}
```

### Invalidation Rules

```rust
/// Rules for cache invalidation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationRules {
    pub rules: Vec<InvalidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationRule {
    /// Endpoint that triggers invalidation
    pub trigger: EndpointPattern,
    /// Endpoints to invalidate
    pub invalidate: Vec<EndpointPattern>,
    /// Invalidation scope
    pub scope: InvalidationScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EndpointPattern {
    Exact(EndpointId),
    PathPattern(String),  // e.g., "/v1/customers/{id}/*"
    MethodAndPath { method: HttpMethod, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidationScope {
    /// Invalidate all cached entries for this endpoint
    All,
    /// Invalidate only entries matching parameter pattern
    Matching { param: String, value_from: ValueSource },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueSource {
    RequestParam(String),
    ResponseField(String),
}

/// Example rules:
/// POST /v1/customers/{id} → invalidate GET /v1/customers/{id}
/// POST /v1/customers/{id}/cards → invalidate GET /v1/customers/{id}, GET /v1/customers/{id}/cards
```

---

## Feature 7-10: Abbreviated Data Structures

### OAuth Flow State
```rust
pub struct OAuthFlowState {
    pub state: String,              // CSRF protection
    pub code_verifier: Option<String>,  // PKCE
    pub redirect_uri: String,
    pub requested_scopes: Vec<String>,
}
```

### Version Detection
```rust
pub struct VersionInfo {
    pub current_version: String,
    pub deprecated_endpoints: Vec<EndpointId>,
    pub migration_paths: HashMap<EndpointId, MigrationPath>,
}
```

### Composite Tool Template
```rust
pub struct ToolTemplate {
    pub name: String,
    pub category: String,
    pub steps: Vec<TemplateStep>,
    pub customizable_params: Vec<String>,
}
```

### Cross-API Recommendation
```rust
pub struct ApiRecommendation {
    pub source_api: IntegrationId,
    pub recommended_api: String,
    pub reason: String,
    pub correlation_score: f64,
}
```

---

## Integration with Capsule Runtime

### New Capsules (Data Structures)

#### AIA-1024 (API Integration Analytics)
```rust
#[repr(C, align(128))]
pub struct AIA1024 {
    /// Header (128 bits)
    pub head: AtomicU128,  // commit:1 | ver:8 | integration_id:96 | ...

    /// Per-endpoint metrics (896 bits)
    pub metrics: [EndpointMetrics; 7],  // 7 endpoints × 128 bits
}

#[repr(C)]
pub struct EndpointMetrics {
    pub endpoint_id: u64,
    pub call_count: u32,
    pub error_count: u32,
    pub p50_latency_us: u32,
    pub p99_latency_us: u32,
}
```

#### AMC-512 (API Marketplace Catalog Entry)
```rust
#[repr(C, align(64))]
pub struct AMC512 {
    /// Header (128 bits)
    pub head: AtomicU128,  // commit:1 | ver:8 | api_id:96 | ...

    /// Catalog metadata (384 bits)
    pub popularity_score: u32,
    pub quality_score: u32,
    pub community_rating: u16,
    pub has_workflows: bool,
    pub oauth_required: bool,
    // ...
}
```

#### AEH-2048 (API Extension Heuristics)
```rust
#[repr(C, align(256))]
pub struct AEH2048 {
    /// Header (128 bits)
    pub head: AtomicU128,

    /// Gap analysis (1920 bits)
    pub official_endpoint_count: u16,
    pub total_endpoint_count: u16,
    pub gap_percentage: u16,
    pub workflow_coverage: u16,
    pub composite_tool_count: u32,
    // Endpoint hash list, normalization rules, etc.
}
```

---

## Serialization & Persistence

### Storage Format

```rust
/// On-disk format for persistence
#[derive(Serialize, Deserialize)]
pub struct PersistedState {
    pub version: u32,
    pub integrations: Vec<Integration>,
    pub workflows: Vec<MultiApiWorkflow>,
    pub cache: Vec<CacheEntry>,
    pub inference_context: InferenceContext,
}

/// Storage trait for swappable backends
pub trait Storage {
    fn save(&self, key: &str, value: &[u8]) -> Result<(), StorageError>;
    fn load(&self, key: &str) -> Result<Vec<u8>, StorageError>;
    fn delete(&self, key: &str) -> Result<(), StorageError>;
}

/// Implementations: FileStorage, MemoryStorage, RocksDB, etc.
```

---

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("Invalid integration ID: {0}")]
    InvalidIntegrationId(String),

    #[error("Workflow not found: {0:?}")]
    WorkflowNotFound(WorkflowId),

    #[error("Parameter inference failed: {0}")]
    InferenceFailed(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
```

---

## Testing Utilities

```rust
#[cfg(test)]
pub mod test_utils {
    use super::*;

    /// Create mock integration for testing
    pub fn mock_integration(name: &str) -> Integration {
        Integration {
            id: IntegrationId::new(),
            api_name: name.to_string(),
            base_url: format!("https://api.{}.com", name),
            auth: AuthConfig::ApiKey { key: "test_key".into() },
            endpoints: vec![],
        }
    }

    /// Create mock workflow pattern
    pub fn mock_workflow_pattern(name: &str, steps: Vec<EndpointId>) -> WorkflowPattern {
        WorkflowPattern {
            id: WorkflowId::new(),
            name: name.to_string(),
            description: format!("Mock workflow: {}", name),
            steps: steps.into_iter().map(|id| WorkflowStep {
                endpoint_id: id,
                optional: false,
                retry_on_failure: true,
            }).collect(),
            frequency: 0.87,
            confidence: 0.95,
            sample_size: 1000,
        }
    }
}
```

---

## Summary

**Total Data Structures**: ~50 core types
**Serialization**: All serde-compatible
**Cache-Aware**: Critical capsules (AIA-1024, AMC-512, AEH-2048) are cache-aligned
**Type-Safe**: Newtype wrappers prevent ID confusion
**Audit-Ready**: All mutations produce ALE-128 events

These data structures form the foundation for the 7,000 lines of intelligent generation logic.
