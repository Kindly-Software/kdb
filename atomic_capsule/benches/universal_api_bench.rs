// UniversalApiMetaCapsule B32 Benchmark Suite
//
// Methodology: Fair baseline comparisons (not strawman)
// - Baseline: Raw protocol-specific handlers (no metacapsule overhead)
// - Optimized: Full UniversalApiMetaCapsule with middleware
// - 1000+ iterations, 95% CI via Criterion.rs
//
// Expected Performance Claims (conservative):
// - Protocol detection: <50ns overhead
// - Middleware execution: ~50ns per middleware × N
// - Circuit breaker check: <50ns overhead
// - Total overhead: <200ns (0 middleware) to <600ns (7 middleware)
//
// Framework Compliance:
// - B32: Fair baselines, 95% CI, realistic workloads
// - UCE34: Q10 T6 tier selection validated
// - ASSUM: All benchmarks measure safe code only

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

// Import UniversalApiMetaCapsule
use atomic_capsule::meta::{
    UniversalApiMetaCapsule,
    UniversalRequest,
    UniversalResponse,
    ProtocolType,
    MiddlewareFn,
    MiddlewareError,
};

// ============================================================================
// Mock Request/Response (test harness)
// ============================================================================

struct MockRequest {
    headers: Vec<(&'static str, &'static str)>,
    method: &'static str,
    path: &'static str,
    body: Vec<u8>,
}

impl MockRequest {
    fn new(method: &'static str, path: &'static str) -> Self {
        Self {
            headers: Vec::new(),
            method,
            path,
            body: Vec::new(),
        }
    }

    fn with_header(mut self, name: &'static str, value: &'static str) -> Self {
        self.headers.push((name, value));
        self
    }
}

impl UniversalRequest for MockRequest {
    fn method(&self) -> &str { self.method }
    fn path(&self) -> &str { self.path }
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
    }
    fn body(&self) -> &[u8] { &self.body }
    fn protocol(&self) -> ProtocolType { ProtocolType::REST }
}

// ============================================================================
// Baseline: Raw Protocol Detection (no metacapsule)
// ============================================================================

fn detect_protocol_baseline(request: &MockRequest) -> ProtocolType {
    // Baseline: Direct header checks (no abstraction overhead)
    if let Some(upgrade) = request.header("Upgrade") {
        if upgrade.eq_ignore_ascii_case("websocket") {
            return ProtocolType::WebSocket;
        }
    }

    if let Some(content_type) = request.header("Content-Type") {
        let ct = content_type.split(';').next().unwrap_or(content_type).trim();
        match ct {
            "application/graphql" => return ProtocolType::GraphQL,
            "application/grpc" => return ProtocolType::Grpc,
            "application/json-rpc" => return ProtocolType::JsonRPC,
            _ => {}
        }
    }

    ProtocolType::REST
}

// ============================================================================
// Middleware (for overhead measurement)
// ============================================================================

fn middleware_noop(_request: &dyn UniversalRequest) -> Result<(), MiddlewareError> {
    Ok(())
}

fn middleware_auth_check(request: &dyn UniversalRequest) -> Result<(), MiddlewareError> {
    // Simulate auth header check (realistic overhead)
    if request.header("Authorization").is_none() {
        return Err(MiddlewareError::AuthFailed {
            reason: "No auth header".to_string(),
        });
    }
    Ok(())
}

// ============================================================================
// B32 Benchmarks
// ============================================================================

fn bench_rest_baseline(c: &mut Criterion) {
    let request = MockRequest::new("GET", "/api/users")
        .with_header("Content-Type", "application/json");

    c.bench_function("REST: Baseline (direct protocol detection)", |b| {
        b.iter(|| {
            let protocol = detect_protocol_baseline(black_box(&request));
            black_box(protocol);
        })
    });
}

fn bench_rest_universal_api(c: &mut Criterion) {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/api/users")
        .with_header("Content-Type", "application/json");

    c.bench_function("REST: UniversalApiMetaCapsule (protocol detection)", |b| {
        b.iter(|| {
            let protocol = capsule.detect_protocol(black_box(&request));
            black_box(protocol);
        })
    });
}

fn bench_rest_with_middleware(c: &mut Criterion) {
    let mut group = c.benchmark_group("REST: Middleware Overhead");

    for middleware_count in [0, 1, 3, 7].iter() {
        let capsule = UniversalApiMetaCapsule::new();

        // Register N middleware
        for _ in 0..*middleware_count {
            capsule.register_middleware(middleware_noop).unwrap();
        }

        let request = MockRequest::new("GET", "/api/users")
            .with_header("Content-Type", "application/json");

        group.bench_with_input(
            BenchmarkId::from_parameter(middleware_count),
            middleware_count,
            |b, _| {
                b.iter(|| {
                    let result = capsule.route(black_box(&request));
                    black_box(result);
                })
            },
        );
    }

    group.finish();
}

fn bench_graphql_baseline(c: &mut Criterion) {
    let request = MockRequest::new("POST", "/graphql")
        .with_header("Content-Type", "application/graphql");

    c.bench_function("GraphQL: Baseline (direct protocol detection)", |b| {
        b.iter(|| {
            let protocol = detect_protocol_baseline(black_box(&request));
            black_box(protocol);
        })
    });
}

fn bench_graphql_universal_api(c: &mut Criterion) {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/graphql")
        .with_header("Content-Type", "application/graphql");

    c.bench_function("GraphQL: UniversalApiMetaCapsule (protocol detection)", |b| {
        b.iter(|| {
            let protocol = capsule.detect_protocol(black_box(&request));
            black_box(protocol);
        })
    });
}

fn bench_jsonrpc_baseline(c: &mut Criterion) {
    let request = MockRequest::new("POST", "/rpc")
        .with_header("Content-Type", "application/json-rpc");

    c.bench_function("JSON-RPC: Baseline (direct protocol detection)", |b| {
        b.iter(|| {
            let protocol = detect_protocol_baseline(black_box(&request));
            black_box(protocol);
        })
    });
}

fn bench_jsonrpc_universal_api(c: &mut Criterion) {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/rpc")
        .with_header("Content-Type", "application/json-rpc");

    c.bench_function("JSON-RPC: UniversalApiMetaCapsule (protocol detection)", |b| {
        b.iter(|| {
            let protocol = capsule.detect_protocol(black_box(&request));
            black_box(protocol);
        })
    });
}

fn bench_circuit_breaker_overhead(c: &mut Criterion) {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/api/test")
        .with_header("Content-Type", "application/json");

    c.bench_function("Circuit Breaker: Check overhead", |b| {
        b.iter(|| {
            let result = capsule.route_with_breaker(black_box(&request));
            black_box(result);
        })
    });
}

fn bench_route_with_breaker_vs_route(c: &mut Criterion) {
    let mut group = c.benchmark_group("Route: With vs Without Breaker");

    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/api/users")
        .with_header("Content-Type", "application/json");

    group.bench_function("route() (no breaker)", |b| {
        b.iter(|| {
            let result = capsule.route(black_box(&request));
            black_box(result);
        })
    });

    group.bench_function("route_with_breaker()", |b| {
        b.iter(|| {
            let result = capsule.route_with_breaker(black_box(&request));
            black_box(result);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_rest_baseline,
    bench_rest_universal_api,
    bench_rest_with_middleware,
    bench_graphql_baseline,
    bench_graphql_universal_api,
    bench_jsonrpc_baseline,
    bench_jsonrpc_universal_api,
    bench_circuit_breaker_overhead,
    bench_route_with_breaker_vs_route
);
criterion_main!(benches);
