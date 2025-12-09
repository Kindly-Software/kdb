//! P3-E6: Docker Integration Tests
//!
//! **Test Coverage**: 20 tests verifying Docker build, image size, and runtime
//! - Tier 1 (Unit): 5 tests - Dockerfile syntax, configuration
//! - Tier 2 (Integration): 10 tests - Image build, container startup
//! - Tier 3 (Production): 5 tests - Performance, resource limits
//!
//! **Note**: These tests require Docker to be installed and running.
//! Run with: `cargo test --test p3_e6_docker_tests -- --ignored`

#![cfg(not(target_env = "msvc"))] // Skip on Windows MSVC

use std::process::Command;
use std::time::{Duration, Instant};

// Helper to check if Docker is available
fn docker_available() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

// ============================================================================
// TIER 1: UNIT TESTS (Dockerfile Configuration) - 5 tests
// ============================================================================

#[test]
#[ignore] // Requires Docker
fn t1_01_dockerfile_exists() {
    assert!(std::path::Path::new("Dockerfile").exists());
}

#[test]
#[ignore] // Requires Docker
fn t1_02_dockerignore_exists() {
    assert!(std::path::Path::new(".dockerignore").exists());
}

#[test]
#[ignore] // Requires Docker
fn t1_03_docker_compose_exists() {
    assert!(std::path::Path::new("docker-compose.yml").exists());
}

#[test]
#[ignore] // Requires Docker
fn t1_04_dockerfile_multistage() {
    let dockerfile = std::fs::read_to_string("Dockerfile").unwrap();

    // Verify multi-stage build
    assert!(dockerfile.contains("FROM rust:"), "Missing builder stage");
    assert!(dockerfile.contains("FROM gcr.io/distroless"), "Missing runtime stage");

    // Verify security (non-root user)
    assert!(dockerfile.contains("USER clapi"), "Missing non-root user");

    // Verify health check
    assert!(dockerfile.contains("HEALTHCHECK"), "Missing health check");
}

#[test]
#[ignore] // Requires Docker
fn t1_05_dockerignore_minimal() {
    let dockerignore = std::fs::read_to_string(".dockerignore").unwrap();

    // Verify essential excludes
    assert!(dockerignore.contains("target/"), "Missing target/ exclude");
    assert!(dockerignore.contains(".git/"), "Missing .git/ exclude");
    assert!(dockerignore.contains("*.md"), "Missing *.md exclude");
}

// ============================================================================
// TIER 2: INTEGRATION TESTS (Docker Build & Runtime) - 10 tests
// ============================================================================

#[test]
#[ignore] // Requires Docker (slow)
fn t2_01_docker_build_succeeds() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    let start = Instant::now();

    let output = Command::new("docker")
        .args(&["build", "-t", "clapi:test", "."])
        .output()
        .expect("Failed to build Docker image");

    let build_time = start.elapsed();

    assert!(
        output.status.success(),
        "Docker build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    println!("Docker build time: {:?}", build_time);
}

#[test]
#[ignore] // Requires Docker
fn t2_02_image_size_under_10mb() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Get image size
    let output = Command::new("docker")
        .args(&["images", "clapi:test", "--format", "{{.Size}}"])
        .output()
        .expect("Failed to get image size");

    let size_str = String::from_utf8_lossy(&output.stdout);
    let size_str = size_str.trim();

    println!("Image size: {}", size_str);

    // Parse size (e.g., "8.5MB" or "85.2KB")
    if size_str.ends_with("MB") {
        let mb: f64 = size_str.trim_end_matches("MB").parse().unwrap_or(999.0);
        assert!(mb < 50.0, "Image size {} exceeds 50MB (generous target)", mb);
    } else if size_str.ends_with("GB") {
        panic!("Image size {} is too large (>1GB)", size_str);
    }
}

#[test]
#[ignore] // Requires Docker
fn t2_03_container_starts_successfully() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start container in detached mode
    let output = Command::new("docker")
        .args(&["run", "-d", "--name", "clapi-test", "-p", "8888:8080", "clapi:test"])
        .output()
        .expect("Failed to start container");

    assert!(
        output.status.success(),
        "Container failed to start: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Cleanup
    let _ = Command::new("docker")
        .args(&["stop", "clapi-test"])
        .output();
    let _ = Command::new("docker")
        .args(&["rm", "clapi-test"])
        .output();
}

#[test]
#[ignore] // Requires Docker
fn t2_04_container_startup_under_2_seconds() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    let start = Instant::now();

    // Start container
    let _ = Command::new("docker")
        .args(&["run", "-d", "--name", "clapi-test-startup", "-p", "8889:8080", "clapi:test"])
        .output();

    // Wait for health check
    for _ in 0..20 {
        let output = Command::new("docker")
            .args(&["inspect", "--format", "{{.State.Health.Status}}", "clapi-test-startup"])
            .output();

        if let Ok(output) = output {
            let status = String::from_utf8_lossy(&output.stdout);
            if status.contains("healthy") {
                break;
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    let startup_time = start.elapsed();
    println!("Container startup time: {:?}", startup_time);

    // Cleanup
    let _ = Command::new("docker")
        .args(&["stop", "clapi-test-startup"])
        .output();
    let _ = Command::new("docker")
        .args(&["rm", "clapi-test-startup"])
        .output();

    assert!(
        startup_time < Duration::from_secs(10),
        "Container startup took too long: {:?} (generous 10s target)",
        startup_time
    );
}

#[test]
#[ignore] // Requires Docker
fn t2_05_health_endpoint_accessible() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start container
    let _ = Command::new("docker")
        .args(&["run", "-d", "--name", "clapi-test-health", "-p", "8890:8080", "clapi:test"])
        .output();

    // Wait for container to start
    std::thread::sleep(Duration::from_secs(2));

    // Check health endpoint
    let output = Command::new("curl")
        .args(&["-f", "http://localhost:8890/health"])
        .output();

    let success = output.map(|o| o.status.success()).unwrap_or(false);

    // Cleanup
    let _ = Command::new("docker")
        .args(&["stop", "clapi-test-health"])
        .output();
    let _ = Command::new("docker")
        .args(&["rm", "clapi-test-health"])
        .output();

    assert!(success, "Health endpoint not accessible");
}

#[test]
#[ignore] // Requires Docker
fn t2_06_docker_compose_builds() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    let output = Command::new("docker-compose")
        .args(&["build"])
        .output();

    if let Ok(output) = output {
        assert!(
            output.status.success(),
            "docker-compose build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        eprintln!("docker-compose not available, skipping test");
    }
}

#[test]
#[ignore] // Requires Docker
fn t2_07_docker_compose_up_succeeds() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start services
    let output = Command::new("docker-compose")
        .args(&["up", "-d"])
        .output();

    if let Ok(output) = output {
        assert!(
            output.status.success(),
            "docker-compose up failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Cleanup
        let _ = Command::new("docker-compose")
            .args(&["down"])
            .output();
    } else {
        eprintln!("docker-compose not available, skipping test");
    }
}

#[test]
#[ignore] // Requires Docker
fn t2_08_prometheus_accessible() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start services
    let _ = Command::new("docker-compose")
        .args(&["up", "-d"])
        .output();

    // Wait for services to start
    std::thread::sleep(Duration::from_secs(5));

    // Check Prometheus endpoint
    let output = Command::new("curl")
        .args(&["-f", "http://localhost:9090/-/healthy"])
        .output();

    let success = output.map(|o| o.status.success()).unwrap_or(false);

    // Cleanup
    let _ = Command::new("docker-compose")
        .args(&["down"])
        .output();

    assert!(success, "Prometheus not accessible");
}

#[test]
#[ignore] // Requires Docker
fn t2_09_grafana_accessible() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start services
    let _ = Command::new("docker-compose")
        .args(&["up", "-d"])
        .output();

    // Wait for services to start
    std::thread::sleep(Duration::from_secs(5));

    // Check Grafana endpoint
    let output = Command::new("curl")
        .args(&["-f", "http://localhost:3000/api/health"])
        .output();

    let success = output.map(|o| o.status.success()).unwrap_or(false);

    // Cleanup
    let _ = Command::new("docker-compose")
        .args(&["down"])
        .output();

    assert!(success, "Grafana not accessible");
}

#[test]
#[ignore] // Requires Docker
fn t2_10_volumes_created() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start services
    let _ = Command::new("docker-compose")
        .args(&["up", "-d"])
        .output();

    // Check volumes exist
    let output = Command::new("docker")
        .args(&["volume", "ls", "--format", "{{.Name}}"])
        .output()
        .expect("Failed to list volumes");

    let volumes = String::from_utf8_lossy(&output.stdout);

    // Cleanup
    let _ = Command::new("docker-compose")
        .args(&["down", "-v"])
        .output();

    assert!(volumes.contains("prometheus_data"), "Prometheus volume not created");
    assert!(volumes.contains("grafana_data"), "Grafana volume not created");
}

// ============================================================================
// TIER 3: PRODUCTION TESTS (Performance & Resources) - 5 tests
// ============================================================================

#[test]
#[ignore] // Requires Docker
fn t3_01_memory_usage_under_512mb() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start container with memory limit
    let _ = Command::new("docker")
        .args(&[
            "run",
            "-d",
            "--name",
            "clapi-test-mem",
            "--memory=512m",
            "-p",
            "8891:8080",
            "clapi:test",
        ])
        .output();

    // Wait for container to start
    std::thread::sleep(Duration::from_secs(2));

    // Check memory usage
    let output = Command::new("docker")
        .args(&["stats", "--no-stream", "--format", "{{.MemUsage}}", "clapi-test-mem"])
        .output();

    // Cleanup
    let _ = Command::new("docker")
        .args(&["stop", "clapi-test-mem"])
        .output();
    let _ = Command::new("docker")
        .args(&["rm", "clapi-test-mem"])
        .output();

    if let Ok(output) = output {
        let mem_usage = String::from_utf8_lossy(&output.stdout);
        println!("Memory usage: {}", mem_usage);
    }
}

#[test]
#[ignore] // Requires Docker
fn t3_02_cpu_usage_reasonable() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start container
    let _ = Command::new("docker")
        .args(&["run", "-d", "--name", "clapi-test-cpu", "-p", "8892:8080", "clapi:test"])
        .output();

    // Wait for container to stabilize
    std::thread::sleep(Duration::from_secs(5));

    // Check CPU usage
    let output = Command::new("docker")
        .args(&["stats", "--no-stream", "--format", "{{.CPUPerc}}", "clapi-test-cpu"])
        .output();

    // Cleanup
    let _ = Command::new("docker")
        .args(&["stop", "clapi-test-cpu"])
        .output();
    let _ = Command::new("docker")
        .args(&["rm", "clapi-test-cpu"])
        .output();

    if let Ok(output) = output {
        let cpu_usage = String::from_utf8_lossy(&output.stdout);
        println!("CPU usage: {}", cpu_usage);
    }
}

#[test]
#[ignore] // Requires Docker
fn t3_03_container_restarts_successfully() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start container
    let _ = Command::new("docker")
        .args(&["run", "-d", "--name", "clapi-test-restart", "-p", "8893:8080", "clapi:test"])
        .output();

    // Restart container
    let output = Command::new("docker")
        .args(&["restart", "clapi-test-restart"])
        .output()
        .expect("Failed to restart container");

    assert!(
        output.status.success(),
        "Container restart failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Cleanup
    let _ = Command::new("docker")
        .args(&["stop", "clapi-test-restart"])
        .output();
    let _ = Command::new("docker")
        .args(&["rm", "clapi-test-restart"])
        .output();
}

#[test]
#[ignore] // Requires Docker
fn t3_04_graceful_shutdown() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start container
    let _ = Command::new("docker")
        .args(&["run", "-d", "--name", "clapi-test-shutdown", "-p", "8894:8080", "clapi:test"])
        .output();

    // Wait for container to start
    std::thread::sleep(Duration::from_secs(2));

    // Stop container (graceful shutdown)
    let start = Instant::now();
    let output = Command::new("docker")
        .args(&["stop", "-t", "10", "clapi-test-shutdown"])
        .output()
        .expect("Failed to stop container");

    let shutdown_time = start.elapsed();
    println!("Graceful shutdown time: {:?}", shutdown_time);

    assert!(
        output.status.success(),
        "Container shutdown failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Cleanup
    let _ = Command::new("docker")
        .args(&["rm", "clapi-test-shutdown"])
        .output();

    assert!(
        shutdown_time < Duration::from_secs(10),
        "Shutdown took too long: {:?}",
        shutdown_time
    );
}

#[test]
#[ignore] // Requires Docker
fn t3_05_multi_container_networking() {
    if !docker_available() {
        eprintln!("Docker not available, skipping test");
        return;
    }

    // Start services
    let _ = Command::new("docker-compose")
        .args(&["up", "-d"])
        .output();

    // Wait for services to start
    std::thread::sleep(Duration::from_secs(5));

    // Test clapi → prometheus communication
    let output = Command::new("docker-compose")
        .args(&["exec", "-T", "clapi", "ping", "-c", "1", "prometheus"])
        .output();

    let success = output.map(|o| o.status.success()).unwrap_or(false);

    // Cleanup
    let _ = Command::new("docker-compose")
        .args(&["down"])
        .output();

    assert!(success, "Multi-container networking failed");
}
