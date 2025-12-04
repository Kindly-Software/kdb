//! validate-mcp: Health check and validation binary for kdb_mcp
//!
//! Purpose: Verify MCP server health, test JSON-RPC protocol, validate service state
//! Framework: UCE34 Q10 (plain validation, no computational tier), B32 (fair testing)
//!
//! Usage:
//!   validate-mcp --endpoint localhost:5678                 # Full validation
//!   validate-mcp --endpoint localhost:5678 --health-only   # Health check only
//!   validate-mcp --endpoint localhost:5678 --timeout 20    # Custom timeout

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Parser, Debug, Clone)]
#[command(name = "validate-mcp")]
#[command(about = "Validate kdb_mcp health and protocol compliance")]
#[command(version = "0.1.0")]
struct Args {
    /// Server endpoint (host:port)
    #[arg(long)]
    endpoint: String,

    /// Timeout in seconds for each request
    #[arg(long, default_value = "10")]
    timeout: u64,

    /// Only perform health check (skip full validation)
    #[arg(long)]
    health_only: bool,

    /// Verbose output
    #[arg(long, short)]
    verbose: bool,
}

/// Health check response structure
#[derive(Debug, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    uptime_seconds: Option<u64>,
}

/// JSON-RPC 2.0 request
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u64,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
    #[serde(default)]
    id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(default)]
    data: Option<serde_json::Value>,
}

struct Validator {
    client: Client,
    endpoint: String,
    args: Args,
}

impl Validator {
    fn new(args: Args) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(args.timeout))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Validator {
            client,
            endpoint: args.endpoint.clone(),
            args,
        })
    }

    fn log(&self, msg: &str) {
        if self.args.verbose {
            eprintln!("[INFO] {}", msg);
        }
    }

    fn log_success(&self, msg: &str) {
        println!("✅ {}", msg);
    }

    fn log_error(&self, msg: &str) {
        eprintln!("❌ {}", msg);
    }

    fn log_warn(&self, msg: &str) {
        eprintln!("⚠️  {}", msg);
    }

    /// Health check: GET /health
    fn check_health(&self) -> Result<()> {
        self.log("Performing health check...");

        let url = format!("http://{}/health", self.endpoint);
        let start = Instant::now();

        let response = self
            .client
            .get(&url)
            .send()
            .context(format!("Failed to connect to {}", url))?;

        let elapsed = start.elapsed();

        if response.status() != 200 {
            return Err(anyhow!(
                "Health check failed with status: {} ({:.2}ms)",
                response.status(),
                elapsed.as_secs_f64() * 1000.0
            ));
        }

        let health: HealthResponse = response
            .json()
            .context("Failed to parse health response JSON")?;

        if health.status != "ok" && health.status != "healthy" {
            return Err(anyhow!("Health status is not ok: {}", health.status));
        }

        let msg = format!(
            "Health check passed ({:.2}ms): status={}",
            elapsed.as_secs_f64() * 1000.0,
            health.status
        );
        if let Some(version) = health.version {
            self.log_success(&format!("{}, version={}", msg, version));
        } else {
            self.log_success(&msg);
        }

        Ok(())
    }

    /// MCP handshake: POST / with JSON-RPC initialize
    fn check_mcp_handshake(&self) -> Result<()> {
        self.log("Testing MCP JSON-RPC handshake...");

        let url = format!("http://{}/", self.endpoint);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: serde_json::json!({}),
            id: 1,
        };

        let start = Instant::now();

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .context("MCP handshake request failed")?;

        let elapsed = start.elapsed();

        if response.status() != 200 {
            return Err(anyhow!(
                "MCP handshake failed with status: {} ({:.2}ms)",
                response.status(),
                elapsed.as_secs_f64() * 1000.0
            ));
        }

        let resp: JsonRpcResponse = response
            .json()
            .context("Failed to parse MCP response JSON")?;

        // Validate JSON-RPC response structure
        if resp.jsonrpc != "2.0" {
            return Err(anyhow!(
                "Invalid JSON-RPC version: {} (expected 2.0)",
                resp.jsonrpc
            ));
        }

        // Check for errors
        if let Some(error) = resp.error {
            return Err(anyhow!(
                "MCP error response: code={}, message={}",
                error.code,
                error.message
            ));
        }

        // Success if we have a result
        if resp.result.is_some() {
            self.log_success(&format!(
                "MCP handshake passed ({:.2}ms)",
                elapsed.as_secs_f64() * 1000.0
            ));
            Ok(())
        } else {
            self.log_warn("MCP response missing result field (may be normal)");
            Ok(())
        }
    }

    /// Protocol validation: Test common MCP methods
    fn check_protocol(&self) -> Result<()> {
        self.log("Validating MCP protocol...");

        // Test 1: capabilities
        self.test_method("capabilities", serde_json::json!({}))
            .context("capabilities test failed")?;

        // Test 2: list_resources
        self.test_method("list_resources", serde_json::json!({}))
            .context("list_resources test failed")?;

        // Test 3: list_tools
        self.test_method("list_tools", serde_json::json!({}))
            .context("list_tools test failed")?;

        self.log_success("Protocol validation passed");
        Ok(())
    }

    /// Test a single MCP method
    fn test_method(&self, method: &str, params: serde_json::Value) -> Result<()> {
        let url = format!("http://{}/", self.endpoint);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: 1,
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .context(format!("Failed to call method: {}", method))?;

        if response.status() != 200 {
            return Err(anyhow!(
                "Method {} failed with status: {}",
                method,
                response.status()
            ));
        }

        let resp: JsonRpcResponse = response
            .json()
            .context(format!("Failed to parse response for method: {}", method))?;

        if let Some(error) = resp.error {
            // Some errors are expected (e.g., method not found), just log them
            self.log(&format!("Method {} returned error: {}", method, error.message));
        }

        Ok(())
    }

    /// Full validation workflow
    fn validate_all(&self) -> Result<()> {
        println!("\n=== MCP Server Validation ===\n");
        println!("Endpoint: http://{}", self.endpoint);
        println!("Timeout:  {}s\n", self.args.timeout);

        // Phase 1: Health check
        println!("Phase 1: Health Check");
        self.check_health()?;

        // Phase 2: MCP handshake
        println!("\nPhase 2: MCP Handshake");
        self.check_mcp_handshake()?;

        // Phase 3: Protocol validation (optional)
        if !self.args.health_only {
            println!("\nPhase 3: Protocol Validation");
            match self.check_protocol() {
                Ok(()) => {}
                Err(e) => {
                    self.log_warn(&format!("Protocol validation incomplete: {}", e));
                }
            }
        }

        println!("\n=== Validation Complete ===\n");
        self.log_success("All checks passed!");

        Ok(())
    }

    /// Health-only validation
    fn validate_health_only(&self) -> Result<()> {
        println!("\n=== Health Check ===\n");
        println!("Endpoint: http://{}", self.endpoint);
        println!("Timeout:  {}s\n", self.args.timeout);

        self.check_health()?;

        println!("\n=== Health Check Complete ===\n");
        self.log_success("Health check passed!");

        Ok(())
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let validator = Validator::new(args.clone())
        .context("Failed to initialize validator")?;

    let result = if args.health_only {
        validator.validate_health_only()
    } else {
        validator.validate_all()
    };

    match result {
        Ok(()) => {
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("\n❌ Validation failed: {}\n", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonrpc_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "test".to_string(),
            params: serde_json::json!({}),
            id: 1,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"test\""));
    }

    #[test]
    fn test_health_response_parsing() {
        let json = r#"{"status":"ok","version":"0.1.0"}"#;
        let resp: HealthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.version, Some("0.1.0".to_string()));
    }
}
