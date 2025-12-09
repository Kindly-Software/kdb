//! Infrastructure module - Prometheus, Docker, Kubernetes integration
//!
//! This module provides infrastructure integration for clapi_core:
//! - Prometheus metrics export (lockfree, <1μs)
//! - Docker health checks
//! - Kubernetes probes
//!
//! # Modules
//! - `prometheus_exporter`: Prometheus text exposition format exporter

pub mod prometheus_exporter;

pub use prometheus_exporter::PrometheusMetricsExporter;
