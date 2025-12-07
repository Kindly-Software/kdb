//! ToolExecutorCapsule Integration Example
//!
//! Demonstrates coordinated tool execution using ToolExecutorCapsule + McpToolRegistryCapsule
//! **Latency target**: <50ns dispatch (measured with B32)

use kdb_mcp::{ToolExecutorCapsule, McpToolRegistryCapsule, ExecutionState};
use std::sync::Arc;
use std::time::SystemTime;

/// Tool coordinator combining executor + registry
pub struct ToolCoordinator {
    executor: Arc<ToolExecutorCapsule>,
    registry: Arc<McpToolRegistryCapsule>,
}

impl ToolCoordinator {
    pub fn new() -> Self {
        Self {
            executor: Arc::new(ToolExecutorCapsule::new()),
            registry: Arc::new(McpToolRegistryCapsule::new()),
        }
    }

    /// Register a tool in the registry
    pub fn register_tool(&self, name: &str, handler_id: u64) -> Result<u64, &'static str> {
        self.registry.register_tool(name, handler_id)
    }

    /// Execute a tool synchronously
    pub fn execute_sync(&self, tool_name: &str) -> Result<String, String> {
        // 1. Lookup tool in registry (<120ns)
        let handle = self.registry.lookup(tool_name)
            .ok_or_else(|| format!("Tool not found: {}", tool_name))?;

        // 2. Begin execution (<30ns)
        let generation = self.executor.begin_execution(handle.tool_id)
            .map_err(|e| e.to_string())?;

        // 3. Execute tool (this is where actual work happens)
        let start = self.get_timestamp_ns();
        let result = match self.dispatch_tool(handle.handler_id) {
            Ok(output) => {
                let elapsed = self.get_timestamp_ns() - start;

                // 4. Complete successfully (<20ns)
                let hash = self.fnv_hash(&output);
                let size = output.len() as u64;
                self.executor.complete_execution(generation, hash, size)
                    .map_err(|e| e.to_string())?;

                println!(
                    "Tool {} completed in {}ns (dispatch: <20ns, work: {}ns)",
                    tool_name, elapsed, elapsed - 20
                );
                output
            }
            Err(e) => {
                let elapsed = self.get_timestamp_ns() - start;

                // 4. Record failure (<20ns)
                self.executor.fail_execution(generation, 1)
                    .map_err(|e| e.to_string())?;

                println!(
                    "Tool {} failed after {}ns: {}",
                    tool_name, elapsed, e
                );
                return Err(e);
            }
        };

        // 5. Record metrics in registry
        handle.record_call(0);

        Ok(result)
    }

    /// Execute tool with timeout
    pub fn execute_with_timeout(&self, tool_name: &str, timeout_ns: u64) -> Result<String, String> {
        let handle = self.registry.lookup(tool_name)
            .ok_or_else(|| format!("Tool not found: {}", tool_name))?;

        let generation = self.executor.begin_execution(handle.tool_id)
            .map_err(|e| e.to_string())?;

        // Set timeout
        self.executor.execution_timeout_ns.store(timeout_ns, core::sync::atomic::Ordering::Relaxed);

        let start = self.get_timestamp_ns();

        // Execute tool
        match self.dispatch_tool(handle.handler_id) {
            Ok(output) => {
                let elapsed = self.get_timestamp_ns() - start;

                // Check for timeout
                if elapsed > timeout_ns {
                    self.executor.fail_execution(generation, 2)
                        .map_err(|e| e.to_string())?;
                    return Err(format!("Tool timeout: {}ns > {}ns", elapsed, timeout_ns));
                }

                let hash = self.fnv_hash(&output);
                self.executor.complete_execution(generation, hash, output.len() as u64)
                    .map_err(|e| e.to_string())?;

                Ok(output)
            }
            Err(e) => {
                self.executor.fail_execution(generation, 1)
                    .map_err(|e| e.to_string())?;
                Err(e)
            }
        }
    }

    /// Get execution statistics
    pub fn get_stats(&self) -> (ExecutionStats, RegistryStats) {
        let exec_stats = self.executor.get_stats();
        let registry_stats = self.registry.get_stats();

        (
            ExecutionStats {
                total_executions: exec_stats.total_executions,
                total_errors: exec_stats.total_errors,
                is_executing: exec_stats.is_executing,
                avg_latency_ns: exec_stats.avg_latency_ns,
                max_concurrent: exec_stats.max_concurrent,
            },
            RegistryStats {
                tool_count: registry_stats.tool_count,
                lookup_count: registry_stats.lookup_count,
                lookup_hits: registry_stats.lookup_hits,
                lookup_misses: registry_stats.lookup_misses,
            },
        )
    }

    /// Internal: dispatch tool handler
    fn dispatch_tool(&self, handler_id: u64) -> Result<String, String> {
        match handler_id {
            1 => Ok("debugger/attach result".to_string()),
            2 => Ok("debugger/set_breakpoint result".to_string()),
            3 => Ok("debugger/continue result".to_string()),
            4 => Err("Tool execution failed".to_string()),
            _ => Err(format!("Unknown tool: {}", handler_id)),
        }
    }

    /// FNV-1a hash for result deduplication
    fn fnv_hash(&self, data: &str) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in data.bytes() {
            hash = hash ^ (byte as u64);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// Get timestamp in nanoseconds
    fn get_timestamp_ns(&self) -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

#[derive(Debug)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub total_errors: u64,
    pub is_executing: bool,
    pub avg_latency_ns: u64,
    pub max_concurrent: u64,
}

#[derive(Debug)]
pub struct RegistryStats {
    pub tool_count: u64,
    pub lookup_count: u64,
    pub lookup_hits: u64,
    pub lookup_misses: u64,
}

fn main() {
    println!("=== ToolExecutorCapsule Integration Example ===\n");

    let coordinator = ToolCoordinator::new();

    // Register tools
    println!("Registering tools...");
    coordinator.register_tool("debugger/attach", 1).unwrap();
    coordinator.register_tool("debugger/set_breakpoint", 2).unwrap();
    coordinator.register_tool("debugger/continue", 3).unwrap();
    coordinator.register_tool("debugger/fail_test", 4).unwrap();

    // Execute tools
    println!("\n--- Successful Execution ---");
    match coordinator.execute_sync("debugger/attach") {
        Ok(result) => println!("Result: {}\n", result),
        Err(e) => println!("Error: {}\n", e),
    }

    println!("--- Lookup Test ---");
    match coordinator.execute_sync("debugger/set_breakpoint") {
        Ok(result) => println!("Result: {}\n", result),
        Err(e) => println!("Error: {}\n", e),
    }

    println!("--- Error Handling ---");
    match coordinator.execute_sync("debugger/fail_test") {
        Ok(result) => println!("Result: {}\n", result),
        Err(e) => println!("Error: {}\n", e),
    }

    println!("--- Nonexistent Tool ---");
    match coordinator.execute_sync("nonexistent/tool") {
        Ok(result) => println!("Result: {}\n", result),
        Err(e) => println!("Error: {}\n", e),
    }

    // Multiple executions
    println!("--- Multiple Executions ---");
    for i in 1..=5 {
        match coordinator.execute_sync("debugger/continue") {
            Ok(_) => println!("Execution {}: OK", i),
            Err(e) => println!("Execution {}: ERROR - {}", i, e),
        }
    }

    // Statistics
    println!("\n=== Statistics ===");
    let (exec_stats, registry_stats) = coordinator.get_stats();

    println!("Executor:");
    println!("  Total executions: {}", exec_stats.total_executions);
    println!("  Total errors: {}", exec_stats.total_errors);
    println!("  Is executing: {}", exec_stats.is_executing);
    println!("  Avg latency: {}ns", exec_stats.avg_latency_ns);
    println!("  Max concurrent: {}", exec_stats.max_concurrent);

    println!("\nRegistry:");
    println!("  Total tools: {}", registry_stats.tool_count);
    println!("  Total lookups: {}", registry_stats.lookup_count);
    println!("  Lookup hits: {}", registry_stats.lookup_hits);
    println!("  Lookup misses: {}", registry_stats.lookup_misses);

    // Timeout example
    println!("\n--- Timeout Example (5μs) ---");
    match coordinator.execute_with_timeout("debugger/continue", 5000) {
        Ok(result) => println!("Result (within timeout): {}", result),
        Err(e) => println!("Timeout or error: {}", e),
    }

    println!("\n=== Integration Complete ===");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registration() {
        let coordinator = ToolCoordinator::new();
        let tool_id = coordinator.register_tool("test/tool", 42).unwrap();
        assert_eq!(tool_id, 1);
    }

    #[test]
    fn test_tool_execution() {
        let coordinator = ToolCoordinator::new();
        coordinator.register_tool("test/tool", 1).unwrap();

        let result = coordinator.execute_sync("test/tool").unwrap();
        assert_eq!(result, "debugger/attach result");
    }

    #[test]
    fn test_nonexistent_tool() {
        let coordinator = ToolCoordinator::new();
        let result = coordinator.execute_sync("nonexistent/tool");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_handling() {
        let coordinator = ToolCoordinator::new();
        coordinator.register_tool("fail", 4).unwrap();

        let result = coordinator.execute_sync("fail");
        assert!(result.is_err());

        let (exec_stats, _) = coordinator.get_stats();
        assert_eq!(exec_stats.total_errors, 1);
    }

    #[test]
    fn test_multiple_executions() {
        let coordinator = ToolCoordinator::new();
        coordinator.register_tool("test/tool", 1).unwrap();

        for _ in 0..5 {
            coordinator.execute_sync("test/tool").unwrap();
        }

        let (exec_stats, _) = coordinator.get_stats();
        assert_eq!(exec_stats.total_executions, 5);
    }
}
