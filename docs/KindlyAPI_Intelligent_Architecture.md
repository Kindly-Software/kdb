# KindlyAPI Intelligent Architecture: The 7,000-Line Innovation

**Version**: 1.0
**Date**: 2025-10-03
**Purpose**: Comprehensive architectural specification for the 10 intelligent MCP generation features

---

## Executive Summary

This document specifies the architecture for KindlyAPI's **primary innovation**: intelligent MCP generation that makes APIs feel native to LLMs. These 10 features (~7,000 lines) sit **above** the capsule runtime foundation and deliver user-facing "magic."

**Design Principles:**
1. **Intelligence over complexity**: Simple interfaces hiding sophisticated logic
2. **Context-aware**: Learn from past API calls to improve future ones
3. **Composable**: Features work independently but enhance each other
4. **Deterministic**: Intelligent decisions must be reproducible
5. **Auditable**: All intelligent actions logged to ALE-128

**Foundation Integration:**
All intelligent features leverage the existing capsule runtime (28 crates):
- **ACB-64**: Circuit breaker for graceful degradation
- **ALE-128**: Tamper-evident audit for all intelligent decisions
- **AIS-128**: Health state for runtime decisions
- **AIA-1024**: Analytics for intelligent feature metrics (NEW)

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    LLM (Claude, GPT, etc.)                      │
└────────────────────────────┬────────────────────────────────────┘
                             │ MCP Protocol (JSON-RPC via stdio)
┌────────────────────────────┴────────────────────────────────────┐
│                   MCP Server Interface Layer                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ integrate_api│  │ call_endpoint│  │extend_mcp_svr│          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
└─────────┼──────────────────┼──────────────────┼─────────────────┘
          │                  │                  │
┌─────────┴──────────────────┴──────────────────┴─────────────────┐
│              INTELLIGENT GENERATION LAYER (~7,000 lines)         │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 1. Endpoint Relationship Graph (~800 lines)                │ │
│  │    ├─ Dependency analyzer                                  │ │
│  │    ├─ Workflow pattern detector                            │ │
│  │    └─ Composite tool generator                             │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 2. Parameter Inference Engine (~600 lines)                 │ │
│  │    ├─ Context tracker (recent calls)                       │ │
│  │    ├─ Pattern matcher (customer_id from create_customer)   │ │
│  │    └─ Smart defaults (currency, timeout)                   │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 3. Multi-API Coordinator (~1000 lines)                     │ │
│  │    ├─ Cross-API workflow orchestrator                      │ │
│  │    ├─ Atomic transaction manager                           │ │
│  │    └─ Rollback engine                                      │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 4. Error Recovery System (~500 lines)                      │ │
│  │    ├─ OAuth refresh handler                                │ │
│  │    ├─ Endpoint migration engine                            │ │
│  │    └─ Intelligent retry policy                             │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 5. Response Normalizer (~400 lines)                        │ │
│  │    ├─ Schema mapper                                        │ │
│  │    ├─ Type coercer                                         │ │
│  │    └─ Field harmonizer                                     │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 6. Intelligent Cache (~700 lines)                          │ │
│  │    ├─ Invalidation rule engine                             │ │
│  │    ├─ Cross-endpoint dependency tracker                    │ │
│  │    └─ LRU with freshness guarantees                        │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 7. OAuth Flow Automator (~900 lines)                       │ │
│  │    ├─ Browser flow handler                                 │ │
│  │    ├─ PKCE support                                         │ │
│  │    └─ Background token refresher                           │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 8. Version Migrator (~600 lines)                           │ │
│  │    ├─ Deprecation detector                                 │ │
│  │    ├─ Parameter mapper                                     │ │
│  │    └─ Breaking change analyzer                             │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 9. Composite Tool Generator (~800 lines)                   │ │
│  │    ├─ High-level operation templates                       │ │
│  │    ├─ Business workflow builder                            │ │
│  │    └─ Custom tool assembler                                │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 10. Cross-API Intelligence (~500 lines)                    │ │
│  │    ├─ Usage pattern analyzer                               │ │
│  │    ├─ Multi-API recommender                                │ │
│  │    └─ Compatibility checker                                │ │
│  └────────────────────────────────────────────────────────────┘ │
└──────────────────────────┬───────────────────────────────────────┘
                           │
┌──────────────────────────┴───────────────────────────────────────┐
│           CAPSULE RUNTIME FOUNDATION (Already Built)             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐           │
│  │  ACB-64  │ │ ALE-128  │ │ AIS-128  │ │ AIA-1024 │           │
│  │ Breaker  │ │  Audit   │ │  Health  │ │Analytics │           │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘           │
└──────────────────────────────────────────────────────────────────┘
```

---

## Feature 1: Intelligent Endpoint Relationships (~800 lines)

### Purpose
Auto-detect dependencies between API endpoints and generate composite tools that combine multiple operations.

### Components

#### 1.1 Dependency Analyzer (~300 lines)
**Input**: OpenAPI spec
**Output**: Directed graph of endpoint dependencies

```rust
pub struct DependencyAnalyzer {
    graph: DirectedGraph<EndpointId, DependencyType>,
    spec: ApiSpec,
}

pub enum DependencyType {
    Sequential,    // A must complete before B (create_customer → create_subscription)
    Parallel,      // A and B can run concurrently (get_customer || get_payment_methods)
    Conditional,   // B only runs if A succeeds with specific result
    Idempotent,    // B can retry without side effects
}

impl DependencyAnalyzer {
    /// Analyze OpenAPI spec to detect endpoint relationships
    pub fn analyze(&self) -> Result<DependencyGraph, AnalysisError> {
        // 1. Parameter matching: "customer_id" parameter → must come from "create_customer"
        // 2. Response field matching: "id" in response → used as parameter in other endpoints
        // 3. HTTP method analysis: POST creates, GET reads, DELETE removes
        // 4. Path structure: /customers/{id}/subscriptions implies dependency on /customers/{id}
    }
}
```

**Example Output:**
```
create_customer (POST /v1/customers)
  ↓ Sequential (provides customer_id)
create_subscription (POST /v1/subscriptions)
  ↓ Sequential (provides subscription_id)
create_invoice (POST /v1/invoices)

get_customer (GET /v1/customers/{id})
  ║ Parallel (read-only)
get_payment_methods (GET /v1/customers/{id}/payment_methods)
```

#### 1.2 Workflow Pattern Detector (~300 lines)
**Input**: Dependency graph + historical usage patterns (from AIA-1024)
**Output**: Common workflow sequences

```rust
pub struct WorkflowDetector {
    patterns: Vec<WorkflowPattern>,
    usage_history: HashMap<IntegrationId, Vec<CallSequence>>,
}

pub struct WorkflowPattern {
    name: String,
    steps: Vec<EndpointId>,
    frequency: f64,  // 0.0-1.0 (how often this sequence appears)
    confidence: f64, // 0.0-1.0 (statistical confidence)
}

impl WorkflowDetector {
    /// Detect common sequences from usage history
    pub fn detect_patterns(&self) -> Vec<WorkflowPattern> {
        // 1. Mine sequential patterns from ALE-128 audit log
        // 2. Cluster similar sequences (create_customer+subscription+invoice)
        // 3. Score by frequency (87% of users do this sequence)
        // 4. Filter by confidence threshold (p-value < 0.05)
    }
}
```

**Example Detected Pattern:**
```rust
WorkflowPattern {
    name: "create_customer_and_subscribe",
    steps: vec!["create_customer", "create_subscription", "create_invoice"],
    frequency: 0.87,  // 87% of users follow this sequence
    confidence: 0.95, // 95% confidence (statistically significant)
}
```

#### 1.3 Composite Tool Generator (~200 lines)
**Input**: Workflow patterns
**Output**: High-level MCP tools

```rust
pub struct CompositeToolGenerator {
    patterns: Vec<WorkflowPattern>,
    spec: ApiSpec,
}

impl CompositeToolGenerator {
    /// Generate a composite MCP tool from a workflow pattern
    pub fn generate(&self, pattern: &WorkflowPattern) -> McpTool {
        // 1. Merge parameters from all steps
        // 2. Auto-fill intermediate parameters (customer_id flows from step 1 to step 2)
        // 3. Generate atomic execution plan (all-or-nothing)
        // 4. Add rollback logic for failures
    }
}
```

**Example Generated Tool:**
```json
{
  "name": "create_customer_and_subscribe",
  "description": "Create customer, subscription, and first invoice in one atomic operation (detected from 87% usage pattern)",
  "input_schema": {
    "type": "object",
    "properties": {
      "email": {"type": "string"},
      "plan_id": {"type": "string"},
      "payment_method": {"type": "string"}
    },
    "required": ["email", "plan_id"]
  },
  "execution": {
    "steps": [
      {"endpoint": "create_customer", "params": {"email": "$input.email"}},
      {"endpoint": "create_subscription", "params": {"customer": "$step1.id", "items": [{"plan": "$input.plan_id"}]}},
      {"endpoint": "create_invoice", "params": {"customer": "$step1.id", "subscription": "$step2.id"}}
    ],
    "atomic": true,
    "rollback_on_failure": true
  }
}
```

### Integration with Capsule Runtime
- **ALE-128**: Log WORKFLOW_DETECTED event when pattern discovered
- **AIA-1024**: Track composite tool usage (invocations, success rate, latency)
- **ACB-64**: If composite tool fails repeatedly, breaker flips (L0→L3)

### Testing Strategy
- **Unit**: Test dependency graph construction from mock OpenAPI specs
- **Property**: "If endpoint A returns field X and endpoint B requires parameter X, they must be connected"
- **Integration**: Generate composite tool for real Stripe API, execute end-to-end
- **Benchmark**: Dependency analysis <50ms for 500-endpoint API

---

## Feature 2: Smart Parameter Inference (~600 lines)

### Purpose
Auto-fill API parameters from context (previous calls, defaults, patterns) to reduce manual specification.

### Components

#### 2.1 Context Tracker (~200 lines)
**Input**: Stream of API calls
**Output**: Contextual state (recent objects created, common values)

```rust
pub struct ContextTracker {
    recent_calls: RingBuffer<CallContext, 100>,  // Last 100 calls
    created_objects: HashMap<ObjectType, Vec<ObjectRef>>,
}

pub struct CallContext {
    integration_id: IntegrationId,
    endpoint: EndpointId,
    params: HashMap<String, serde_json::Value>,
    response: ApiResponse,
    timestamp: Instant,
}

impl ContextTracker {
    /// Track a completed API call
    pub fn track(&mut self, call: CallContext) {
        self.recent_calls.push(call.clone());

        // Extract created objects from response
        if call.response.status == 201 {  // Created
            if let Some(id) = call.response.body.get("id") {
                let obj_type = self.infer_object_type(&call.endpoint);
                self.created_objects.entry(obj_type)
                    .or_default()
                    .push(ObjectRef {
                        id: id.clone(),
                        created_at: call.timestamp,
                    });
            }
        }
    }
}
```

#### 2.2 Pattern Matcher (~250 lines)
**Input**: Required parameter + context
**Output**: Inferred value (or None if no match)

```rust
pub struct PatternMatcher {
    rules: Vec<InferenceRule>,
}

pub struct InferenceRule {
    parameter_name: String,
    source: InferenceSource,
    confidence: f64,
}

pub enum InferenceSource {
    PreviousCallResponse { endpoint: String, field: String },
    LastCreatedObject { object_type: String },
    ContextPattern { pattern: String },
    SmartDefault { value: serde_json::Value },
}

impl PatternMatcher {
    /// Infer parameter value from context
    pub fn infer(&self, param: &Parameter, context: &ContextTracker) -> Option<InferredValue> {
        for rule in &self.rules {
            if rule.matches(param) {
                if let Some(value) = self.apply_rule(rule, context) {
                    return Some(InferredValue {
                        value,
                        source: rule.source.clone(),
                        confidence: rule.confidence,
                    });
                }
            }
        }
        None
    }

    fn apply_rule(&self, rule: &InferenceRule, context: &ContextTracker) -> Option<Value> {
        match &rule.source {
            InferenceSource::LastCreatedObject { object_type } => {
                // "customer_id" parameter → use last created customer ID
                context.created_objects.get(object_type)
                    .and_then(|objs| objs.last())
                    .map(|obj| obj.id.clone())
            },
            InferenceSource::SmartDefault { value } => {
                // "currency" parameter → default to "usd"
                Some(value.clone())
            },
            _ => None,
        }
    }
}
```

**Example Inference Rules:**
```rust
vec![
    InferenceRule {
        parameter_name: "customer_id".into(),
        source: InferenceSource::LastCreatedObject { object_type: "customer".into() },
        confidence: 0.95,
    },
    InferenceRule {
        parameter_name: "currency".into(),
        source: InferenceSource::SmartDefault { value: json!("usd") },
        confidence: 0.80,
    },
    InferenceRule {
        parameter_name: "timeout_ms".into(),
        source: InferenceSource::SmartDefault { value: json!(30000) },
        confidence: 0.70,
    },
]
```

#### 2.3 Smart Defaults (~150 lines)
**Input**: API spec + environment
**Output**: Default values for common parameters

```rust
pub struct SmartDefaults {
    environment: Environment,  // Test, Production
    api_patterns: HashMap<String, DefaultValue>,
}

pub enum Environment {
    Test,
    Production,
}

impl SmartDefaults {
    /// Get default value for parameter based on environment and patterns
    pub fn get_default(&self, param: &Parameter) -> Option<Value> {
        // 1. Environment-aware: test API keys, sandbox URLs
        if self.environment == Environment::Test {
            if param.name.contains("key") || param.name.contains("token") {
                return Some(json!("test_key_placeholder"));
            }
        }

        // 2. Common patterns: currency=usd, timeout=30s, limit=100
        self.api_patterns.get(&param.name).map(|d| d.value.clone())
    }
}
```

### Integration with Capsule Runtime
- **ALE-128**: Log PARAMETER_INFERRED event with source and confidence
- **AIA-1024**: Track inference accuracy (accepted vs rejected inferences)

### Testing Strategy
- **Unit**: Test inference rules individually
- **Property**: "If customer created 1ms ago, customer_id inference confidence > 0.9"
- **Integration**: Infer parameters for real Stripe subscription creation
- **Benchmark**: Inference <10ms for 20-parameter endpoint

---

## Feature 3: Multi-API Orchestration (~1000 lines)

### Purpose
Coordinate atomic operations across multiple APIs (Stripe + SendGrid + Twilio) with rollback on failure.

### Components

#### 3.1 Cross-API Workflow Orchestrator (~400 lines)
**Input**: Multi-API workflow definition
**Output**: Execution plan with rollback steps

```rust
pub struct WorkflowOrchestrator {
    integrations: HashMap<IntegrationId, Integration>,
}

pub struct MultiApiWorkflow {
    name: String,
    steps: Vec<WorkflowStep>,
    atomic: bool,  // All-or-nothing execution
}

pub struct WorkflowStep {
    integration_id: IntegrationId,
    endpoint: EndpointId,
    params: HashMap<String, ParamValue>,
    rollback: Option<RollbackAction>,
}

pub enum ParamValue {
    Static(Value),
    FromStep { step_index: usize, field: String },  // Use result from previous step
    Inferred,  // Use parameter inference
}

impl WorkflowOrchestrator {
    /// Execute multi-API workflow atomically
    pub async fn execute(&self, workflow: &MultiApiWorkflow) -> Result<WorkflowResult, WorkflowError> {
        let mut completed_steps = Vec::new();

        for (idx, step) in workflow.steps.iter().enumerate() {
            // Resolve parameters (static, from previous steps, or inferred)
            let params = self.resolve_params(&step.params, &completed_steps)?;

            // Execute step
            match self.call_endpoint(step.integration_id, step.endpoint, params).await {
                Ok(result) => {
                    completed_steps.push(StepResult { step: idx, result });
                },
                Err(e) if workflow.atomic => {
                    // Rollback all completed steps in reverse order
                    self.rollback(&completed_steps, workflow).await?;
                    return Err(WorkflowError::StepFailed { step: idx, error: e });
                },
                Err(e) => {
                    return Err(WorkflowError::StepFailed { step: idx, error: e });
                }
            }
        }

        Ok(WorkflowResult { steps: completed_steps })
    }
}
```

#### 3.2 Atomic Transaction Manager (~300 lines)
**Input**: Workflow execution state
**Output**: Commit or rollback decision

```rust
pub struct TransactionManager {
    active_workflows: HashMap<WorkflowId, WorkflowState>,
}

pub struct WorkflowState {
    workflow_id: WorkflowId,
    started_at: Instant,
    completed_steps: Vec<StepResult>,
    status: WorkflowStatus,
}

pub enum WorkflowStatus {
    InProgress,
    Committed,
    RolledBack,
    Failed,
}

impl TransactionManager {
    /// Commit workflow (all steps succeeded)
    pub fn commit(&mut self, workflow_id: WorkflowId) -> Result<(), TransactionError> {
        let state = self.active_workflows.get_mut(&workflow_id)
            .ok_or(TransactionError::WorkflowNotFound)?;

        state.status = WorkflowStatus::Committed;

        // Write MULTI_API_WORKFLOW event to ALE-128
        self.log_commit(state);

        Ok(())
    }

    /// Rollback workflow (one step failed)
    pub async fn rollback(&mut self, workflow_id: WorkflowId) -> Result<(), TransactionError> {
        let state = self.active_workflows.get(&workflow_id)
            .ok_or(TransactionError::WorkflowNotFound)?;

        // Execute rollback actions in reverse order
        for step in state.completed_steps.iter().rev() {
            if let Some(rollback) = &step.rollback_action {
                self.execute_rollback(rollback).await?;
            }
        }

        state.status = WorkflowStatus::RolledBack;
        Ok(())
    }
}
```

#### 3.3 Rollback Engine (~300 lines)
**Input**: Completed workflow steps
**Output**: Compensating actions to undo changes

```rust
pub struct RollbackEngine {
    strategies: HashMap<EndpointId, RollbackStrategy>,
}

pub enum RollbackStrategy {
    Delete { endpoint: String },  // DELETE /customers/{id}
    Update { endpoint: String, params: HashMap<String, Value> },  // PATCH /customers/{id} status=canceled
    NoOp,  // Read-only endpoint, nothing to rollback
    Custom { handler: Box<dyn Fn(&StepResult) -> RollbackAction> },
}

impl RollbackEngine {
    /// Generate rollback action for a completed step
    pub fn generate_rollback(&self, step: &WorkflowStep, result: &StepResult) -> Option<RollbackAction> {
        let strategy = self.strategies.get(&step.endpoint)?;

        match strategy {
            RollbackStrategy::Delete { endpoint } => {
                // Extract ID from result, generate DELETE call
                if let Some(id) = result.response.body.get("id") {
                    Some(RollbackAction::DeleteObject {
                        integration_id: step.integration_id,
                        endpoint: endpoint.replace("{id}", &id.to_string()),
                    })
                } else {
                    None
                }
            },
            RollbackStrategy::NoOp => None,
            _ => None,
        }
    }
}
```

**Example Rollback Strategies:**
```rust
// Stripe create_customer → DELETE /customers/{id}
RollbackStrategy::Delete { endpoint: "/v1/customers/{id}".into() }

// Stripe create_subscription → PATCH /v1/subscriptions/{id} {status: "canceled"}
RollbackStrategy::Update {
    endpoint: "/v1/subscriptions/{id}".into(),
    params: hashmap!{"status" => json!("canceled")},
}

// SendGrid send_email → NoOp (can't unsend email)
RollbackStrategy::NoOp
```

### Integration with Capsule Runtime
- **ALE-128**: Log MULTI_API_WORKFLOW event with all steps and rollback status
- **ACB-64**: If multi-API workflows fail repeatedly, flip breaker (L0→L3)
- **AIA-1024**: Track workflow success rate, latency distribution

### Testing Strategy
- **Unit**: Test rollback action generation for each strategy type
- **Property**: "If step N fails, steps 1..N-1 must be rolled back"
- **Integration**: Execute Stripe+SendGrid+Twilio workflow, force failure at each step, verify rollback
- **Chaos**: Random failures during workflow execution, verify atomicity

---

## Feature 4: Automatic Error Recovery (~500 lines)

### Purpose
Transparently handle OAuth token refresh, endpoint deprecation, and intelligent retries without user intervention.

### Components

#### 4.1 OAuth Refresh Handler (~200 lines)
```rust
pub struct OAuthRefreshHandler {
    tokens: HashMap<IntegrationId, TokenState>,
    refresh_scheduler: BackgroundScheduler,
}

pub struct TokenState {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Instant,
    refresh_in_progress: AtomicBool,
}

impl OAuthRefreshHandler {
    /// Background task: refresh tokens before expiration
    pub async fn background_refresh(&self) {
        loop {
            for (integration_id, state) in &self.tokens {
                // Refresh 5 minutes before expiration
                if state.expires_at.saturating_duration_since(Instant::now()) < Duration::from_secs(300) {
                    self.refresh_token(integration_id).await;
                }
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }

    /// Transparent token refresh on 401 error
    pub async fn handle_auth_error(&self, integration_id: IntegrationId) -> Result<(), RefreshError> {
        // Log OAUTH_REFRESH event
        self.log_refresh_attempt(integration_id);

        // Perform refresh
        let new_token = self.refresh_token(integration_id).await?;

        // Update integration config
        self.update_token(integration_id, new_token)?;

        Ok(())
    }
}
```

#### 4.2 Endpoint Migration Engine (~200 lines)
```rust
pub struct EndpointMigrator {
    deprecation_map: HashMap<EndpointId, MigrationPath>,
}

pub struct MigrationPath {
    old_endpoint: String,
    new_endpoint: String,
    parameter_mapping: HashMap<String, ParameterMapping>,
    breaking_changes: Vec<BreakingChange>,
}

pub struct ParameterMapping {
    old_name: String,
    new_name: String,
    transform: Option<Box<dyn Fn(Value) -> Value>>,
}

impl EndpointMigrator {
    /// Auto-migrate deprecated endpoint to new version
    pub fn migrate(&self, endpoint: &str, params: HashMap<String, Value>) -> Result<MigratedCall, MigrationError> {
        let migration = self.deprecation_map.get(endpoint)
            .ok_or(MigrationError::NoMigrationPath)?;

        // Check for breaking changes
        if !migration.breaking_changes.is_empty() {
            return Err(MigrationError::UnsafeMigration {
                changes: migration.breaking_changes.clone()
            });
        }

        // Map parameters
        let mut new_params = HashMap::new();
        for (old_name, value) in params {
            if let Some(mapping) = migration.parameter_mapping.get(&old_name) {
                let new_value = if let Some(transform) = &mapping.transform {
                    transform(value)
                } else {
                    value
                };
                new_params.insert(mapping.new_name.clone(), new_value);
            }
        }

        // Log VERSION_MIGRATED event
        self.log_migration(endpoint, &migration.new_endpoint);

        Ok(MigratedCall {
            endpoint: migration.new_endpoint.clone(),
            params: new_params,
        })
    }
}
```

**Example Migration:**
```rust
// Stripe v1 → v2: POST /v1/charges → POST /v2/payment_intents
MigrationPath {
    old_endpoint: "/v1/charges".into(),
    new_endpoint: "/v2/payment_intents".into(),
    parameter_mapping: hashmap!{
        "amount" => ParameterMapping {
            old_name: "amount".into(),  // cents (integer)
            new_name: "amount_decimal".into(),  // decimal string
            transform: Some(Box::new(|v| json!((v.as_u64().unwrap() as f64 / 100.0).to_string()))),
        },
    },
    breaking_changes: vec![],
}
```

#### 4.3 Intelligent Retry Policy (~100 lines)
```rust
pub struct RetryPolicy {
    strategies: HashMap<ErrorType, RetryStrategy>,
}

pub enum RetryStrategy {
    Exponential { max_attempts: u8, base_delay: Duration },
    Linear { max_attempts: u8, delay: Duration },
    NoRetry,
}

pub enum ErrorType {
    Auth,           // 401, 403 → NoRetry (requires user action)
    RateLimit,      // 429 → Exponential backoff
    ServerError,    // 5xx → Exponential backoff
    Timeout,        // Network timeout → Exponential backoff
    ClientError,    // 400, 404 → NoRetry (bad request)
}

impl RetryPolicy {
    pub fn should_retry(&self, error: &ApiError, attempt: u8) -> Option<Duration> {
        let error_type = self.classify_error(error);
        let strategy = self.strategies.get(&error_type)?;

        match strategy {
            RetryStrategy::Exponential { max_attempts, base_delay } if attempt < *max_attempts => {
                Some(*base_delay * 2u32.pow(attempt as u32))
            },
            RetryStrategy::NoRetry => None,
            _ => None,
        }
    }
}
```

### Integration with Capsule Runtime
- **ALE-128**: Log OAUTH_REFRESH, VERSION_MIGRATED events
- **ACB-64**: Retry failures count toward breaker threshold

### Testing Strategy
- **Unit**: Test token expiration detection, migration mapping
- **Integration**: Simulate OAuth expiration, verify transparent refresh
- **Chaos**: Force endpoint deprecation, verify auto-migration

---

## Features 5-10: Abbreviated Specifications

*(Due to length constraints, providing high-level architecture only. Full specs available on request.)*

### Feature 5: Response Normalizer (~400 lines)
- **Schema Mapper**: Stripe `{id}` → PayPal `{customer_id}` → unified `{id, provider}`
- **Type Coercer**: String "123" → number 123
- **Field Harmonizer**: `email_address` → `email`

### Feature 6: Intelligent Cache (~700 lines)
- **Invalidation Rule Engine**: POST /customers/{id} → invalidate GET /customers/{id}
- **Cross-Endpoint Tracker**: POST /customers/{id}/cards → invalidate GET /customers/{id}
- **LRU with Freshness**: Configurable staleness tolerance per endpoint

### Feature 7: OAuth Flow Automator (~900 lines)
- **Browser Flow Handler**: Local HTTP server for OAuth callback
- **PKCE Support**: Secure OAuth 2.0
- **Background Refresher**: Refresh tokens before expiration

### Feature 8: Version Migrator (~600 lines)
- **Deprecation Detector**: Monitor API changelog, detect deprecated endpoints
- **Parameter Mapper**: Auto-convert old → new parameter formats
- **Breaking Change Analyzer**: Warn when migration is unsafe

### Feature 9: Composite Tool Generator (~800 lines)
- **High-Level Templates**: `setup_subscription_business` = customer + product + price + subscription + billing
- **Business Workflow Builder**: Visual workflow designer (TUI)
- **Custom Tool Assembler**: User-defined composite tools

### Feature 10: Cross-API Intelligence (~500 lines)
- **Usage Pattern Analyzer**: Mine ALE-128 for cross-API correlations
- **Multi-API Recommender**: "90% of Stripe users also use SendGrid"
- **Compatibility Checker**: Warn when APIs have conflicting OAuth scopes

---

## Data Structures

### Core Types

```rust
// Endpoint relationship graph
pub struct DependencyGraph {
    nodes: HashMap<EndpointId, Endpoint>,
    edges: HashMap<(EndpointId, EndpointId), DependencyType>,
}

// Parameter inference context
pub struct InferenceContext {
    recent_calls: RingBuffer<CallContext, 100>,
    created_objects: HashMap<ObjectType, Vec<ObjectRef>>,
    defaults: SmartDefaults,
}

// Multi-API workflow definition
pub struct Workflow {
    id: WorkflowId,
    name: String,
    steps: Vec<WorkflowStep>,
    atomic: bool,
}

// Cache invalidation rules
pub struct InvalidationRule {
    trigger_endpoint: EndpointPattern,
    invalidate_endpoints: Vec<EndpointPattern>,
}

// OAuth token state
pub struct TokenState {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Instant,
}
```

---

## Testing Strategy

### Unit Tests (~2,000 tests)
- Each feature has 150-300 unit tests
- Focus: Individual components (dependency analyzer, pattern matcher, etc.)
- Coverage: >90% line coverage

### Integration Tests (~500 tests)
- End-to-end workflows with real API specs
- Stripe, GitHub, OpenAI integration tests
- Multi-API orchestration scenarios

### Property Tests (~200 tests)
- Invariants: "Workflow rollback always succeeds or leaves system in safe state"
- Fuzz testing: Random OpenAPI specs, parameter combinations

### Benchmark Tests (~50 benchmarks)
- Dependency analysis: <50ms for 500-endpoint API
- Parameter inference: <10ms for 20-parameter endpoint
- Workflow orchestration: <200ms overhead for 5-step workflow

---

## Deployment Considerations

### Performance Targets
- Intelligent feature overhead: <200ms per API call
- Background tasks (OAuth refresh, pattern detection): <5% CPU
- Memory: <100MB for 50 integrations with full context

### Monitoring
- All intelligent actions logged to ALE-128
- AIA-1024 tracks feature usage, accuracy, latency
- Dashboard shows: workflows detected, parameters inferred, OAuth refreshes

### Graceful Degradation
- If intelligent features fail, fall back to basic OpenAPI parsing
- ACB-64 can disable intelligent features independently (e.g., L1 disables cross-API intelligence, L2 disables composite tools)

---

## Implementation Roadmap

**Week 1**: Endpoint Relationships + Parameter Inference
**Week 2**: OAuth Automation + Intelligent Caching + Error Recovery
**Week 3**: Multi-API Orchestration + Composite Tools + Response Normalization
**Week 4**: Cross-API Intelligence + Version Migration + Polish

Total: ~7,000 lines of sophisticated, user-facing innovation built on the solid capsule runtime foundation.
