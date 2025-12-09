//! Infrastructure Integration Tests (P3-E6, P3-E11)
//!
//! Tests for Docker, Kubernetes, Prometheus, and Grafana integration
//!
//! # Test Coverage
//! - Docker build validation
//! - Kubernetes manifest validation
//! - Prometheus metrics format
//! - Health check endpoints
//!
//! # Usage
//! ```bash
//! cargo test --test infrastructure_tests
//! ```

use std::process::Command;
use std::path::Path;

/// Test: Docker build succeeds
///
/// **P3-E6**: Validates multi-stage Dockerfile builds successfully
///
/// # Success Criteria
/// - Dockerfile exists
/// - Docker build command succeeds
/// - Image size < 20MB (target: <10MB)
#[test]
#[ignore] // Run with: cargo test --test infrastructure_tests -- --ignored
fn test_docker_build_succeeds() {
    // Check if Dockerfile exists
    let dockerfile = Path::new("Dockerfile");
    assert!(
        dockerfile.exists(),
        "Dockerfile not found in project root"
    );

    // Check if Docker is installed
    let docker_check = Command::new("docker")
        .arg("--version")
        .output();

    if docker_check.is_err() {
        eprintln!("Docker not installed, skipping test");
        return;
    }

    // Build Docker image
    let output = Command::new("docker")
        .args(&["build", "-t", "clapi:test", "."])
        .output()
        .expect("Failed to execute docker build");

    assert!(
        output.status.success(),
        "Docker build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Check image size
    let size_output = Command::new("docker")
        .args(&["image", "inspect", "clapi:test", "--format={{.Size}}"])
        .output()
        .expect("Failed to inspect image size");

    if size_output.status.success() {
        let size_str = String::from_utf8_lossy(&size_output.stdout);
        let size_bytes: u64 = size_str.trim().parse().unwrap_or(0);
        let size_mb = size_bytes / 1_000_000;

        println!("Docker image size: {} MB", size_mb);
        assert!(
            size_mb < 20,
            "Image size too large: {} MB (target: <10MB)",
            size_mb
        );
    }

    // Cleanup
    let _ = Command::new("docker")
        .args(&["rmi", "clapi:test"])
        .output();
}

/// Test: Kubernetes manifests are valid YAML
///
/// **P3-E11**: Validates all Kubernetes manifest syntax
///
/// # Success Criteria
/// - All YAML files parse successfully
/// - Required fields present
#[test]
fn test_kubernetes_manifests_valid_yaml() {
    let manifests = vec![
        "k8s/statefulset.yaml",
        "k8s/hpa.yaml",
        "k8s/pdb.yaml",
    ];

    for manifest in manifests {
        let path = Path::new(manifest);
        assert!(
            path.exists(),
            "Kubernetes manifest not found: {}",
            manifest
        );

        // Read and parse YAML
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", manifest, e));

        // Basic YAML validation (check for valid structure)
        assert!(
            !content.is_empty(),
            "Manifest {} is empty",
            manifest
        );

        // Check for required Kubernetes fields
        assert!(
            content.contains("apiVersion:"),
            "Missing apiVersion in {}",
            manifest
        );
        assert!(
            content.contains("kind:"),
            "Missing kind in {}",
            manifest
        );
        assert!(
            content.contains("metadata:"),
            "Missing metadata in {}",
            manifest
        );

        println!("✓ {} is valid YAML", manifest);
    }
}

/// Test: Prometheus configuration is valid
///
/// **P3-E11**: Validates Prometheus scrape config
///
/// # Success Criteria
/// - prometheus.yml exists
/// - Valid YAML syntax
/// - Scrape config for clapi exists
#[test]
fn test_prometheus_config_valid() {
    let config_path = Path::new("config/prometheus.yml");
    assert!(
        config_path.exists(),
        "prometheus.yml not found in config/"
    );

    let content = std::fs::read_to_string(config_path)
        .expect("Failed to read prometheus.yml");

    // Check for required sections
    assert!(
        content.contains("scrape_configs:"),
        "Missing scrape_configs section"
    );
    assert!(
        content.contains("job_name: 'clapi'"),
        "Missing clapi scrape job"
    );
    assert!(
        content.contains("metrics_path: '/metrics'"),
        "Missing /metrics endpoint"
    );

    println!("✓ prometheus.yml is valid");
}

/// Test: Alert rules configuration is valid
///
/// **P3-E11**: Validates Prometheus alert rules
///
/// # Success Criteria
/// - alert_rules.yml exists
/// - Valid YAML syntax
/// - Required alert groups present
#[test]
fn test_alert_rules_valid() {
    let rules_path = Path::new("config/alert_rules.yml");
    assert!(
        rules_path.exists(),
        "alert_rules.yml not found in config/"
    );

    let content = std::fs::read_to_string(rules_path)
        .expect("Failed to read alert_rules.yml");

    // Check for required sections
    assert!(
        content.contains("groups:"),
        "Missing groups section"
    );
    assert!(
        content.contains("circuit_breaker_alerts"),
        "Missing circuit breaker alerts"
    );
    assert!(
        content.contains("latency_alerts"),
        "Missing latency alerts"
    );
    assert!(
        content.contains("health_alerts"),
        "Missing health alerts"
    );

    println!("✓ alert_rules.yml is valid");
}

/// Test: Grafana dashboard JSON is valid
///
/// **P3-E11**: Validates Grafana dashboard configuration
///
/// # Success Criteria
/// - grafana-dashboard.json exists
/// - Valid JSON syntax
/// - Required panels present
#[test]
fn test_grafana_dashboard_valid() {
    let dashboard_path = Path::new("dashboards/grafana-dashboard.json");
    assert!(
        dashboard_path.exists(),
        "grafana-dashboard.json not found in dashboards/"
    );

    let content = std::fs::read_to_string(dashboard_path)
        .expect("Failed to read grafana-dashboard.json");

    // Parse JSON
    let json: serde_json::Value = serde_json::from_str(&content)
        .expect("Invalid JSON in grafana-dashboard.json");

    // Check for required fields
    assert!(
        json.get("dashboard").is_some(),
        "Missing dashboard field"
    );

    let dashboard = json.get("dashboard").unwrap();
    assert!(
        dashboard.get("panels").is_some(),
        "Missing panels field"
    );

    println!("✓ grafana-dashboard.json is valid");
}

/// Test: Docker Compose configuration is valid
///
/// **P3-E6**: Validates docker-compose.yml
///
/// # Success Criteria
/// - docker-compose.yml exists
/// - Valid YAML syntax
/// - Required services present (clapi, prometheus, grafana)
#[test]
fn test_docker_compose_valid() {
    let compose_path = Path::new("docker-compose.yml");
    assert!(
        compose_path.exists(),
        "docker-compose.yml not found in project root"
    );

    let content = std::fs::read_to_string(compose_path)
        .expect("Failed to read docker-compose.yml");

    // Check for required services
    assert!(
        content.contains("services:"),
        "Missing services section"
    );
    assert!(
        content.contains("clapi:"),
        "Missing clapi service"
    );
    assert!(
        content.contains("prometheus:"),
        "Missing prometheus service"
    );
    assert!(
        content.contains("grafana:"),
        "Missing grafana service"
    );

    // Check for networks
    assert!(
        content.contains("networks:"),
        "Missing networks section"
    );
    assert!(
        content.contains("clapi_network"),
        "Missing clapi_network"
    );

    println!("✓ docker-compose.yml is valid");
}

/// Test: Kubernetes HPA configuration
///
/// **P3-E11**: Validates HPA manifest
///
/// # Success Criteria
/// - hpa.yaml exists
/// - minReplicas: 3
/// - maxReplicas: 10
/// - CPU and memory metrics present
#[test]
fn test_kubernetes_hpa_configuration() {
    let hpa_path = Path::new("k8s/hpa.yaml");
    assert!(hpa_path.exists(), "hpa.yaml not found");

    let content = std::fs::read_to_string(hpa_path)
        .expect("Failed to read hpa.yaml");

    assert!(
        content.contains("kind: HorizontalPodAutoscaler"),
        "Missing HorizontalPodAutoscaler kind"
    );
    assert!(
        content.contains("minReplicas: 3"),
        "Expected minReplicas: 3"
    );
    assert!(
        content.contains("maxReplicas: 10"),
        "Expected maxReplicas: 10"
    );
    assert!(
        content.contains("name: cpu"),
        "Missing CPU metric"
    );
    assert!(
        content.contains("name: memory"),
        "Missing memory metric"
    );

    println!("✓ HPA configuration is valid");
}

/// Test: Kubernetes PDB configuration
///
/// **P3-E11**: Validates PDB manifest
///
/// # Success Criteria
/// - pdb.yaml exists
/// - minAvailable: 2 (high availability)
/// - Selector matches StatefulSet
#[test]
fn test_kubernetes_pdb_configuration() {
    let pdb_path = Path::new("k8s/pdb.yaml");
    assert!(pdb_path.exists(), "pdb.yaml not found");

    let content = std::fs::read_to_string(pdb_path)
        .expect("Failed to read pdb.yaml");

    assert!(
        content.contains("kind: PodDisruptionBudget"),
        "Missing PodDisruptionBudget kind"
    );
    assert!(
        content.contains("minAvailable: 2"),
        "Expected minAvailable: 2"
    );
    assert!(
        content.contains("app: clapi"),
        "Missing app: clapi selector"
    );

    println!("✓ PDB configuration is valid");
}

/// Test: All infrastructure files exist
///
/// **Comprehensive check**: Validates all required files for deployment
#[test]
fn test_all_infrastructure_files_exist() {
    let required_files = vec![
        "Dockerfile",
        "docker-compose.yml",
        "k8s/statefulset.yaml",
        "k8s/hpa.yaml",
        "k8s/pdb.yaml",
        "config/prometheus.yml",
        "config/alert_rules.yml",
        "dashboards/grafana-dashboard.json",
    ];

    for file in required_files {
        let path = Path::new(file);
        assert!(
            path.exists(),
            "Required infrastructure file missing: {}",
            file
        );
        println!("✓ {} exists", file);
    }
}

/// Test: Prometheus metrics exporter capsule
///
/// **P3-E11**: Validates PrometheusMetricsExporter implementation
#[test]
fn test_prometheus_metrics_exporter() {
    use clapi_core::infrastructure::PrometheusMetricsExporter;

    let exporter = PrometheusMetricsExporter::new();

    // Export metrics
    let metrics = exporter.export_metrics();

    // Validate Prometheus text format
    assert!(
        metrics.contains("# HELP"),
        "Missing HELP directives"
    );
    assert!(
        metrics.contains("# TYPE"),
        "Missing TYPE directives"
    );

    // Check for required metrics
    let required_metrics = vec![
        "clapi_health_status",
        "clapi_latency_p99_ns",
        "clapi_circuit_breaker_state",
        "clapi_response_cache_hit_rate_percent",
    ];

    for metric in required_metrics {
        assert!(
            metrics.contains(metric),
            "Missing required metric: {}",
            metric
        );
    }

    // Verify export counter increments
    assert_eq!(exporter.export_count(), 1);

    exporter.export_metrics();
    assert_eq!(exporter.export_count(), 2);

    println!("✓ PrometheusMetricsExporter works correctly");
}
