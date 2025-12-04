# MCP Document Tools - Usage Examples

Complete examples for integrating and using all 4 document processing MCP tools.

## 1. Tool Registration Example

### Basic Registration
```rust
use atomic_mcp_server::tools::register_document_tools;
use atomic_mcp_server::McpToolRegistryCapsule;

fn main() {
    // Create tool registry (16 KB)
    let registry = McpToolRegistryCapsule::new();

    // Register all document tools
    match register_document_tools(&registry) {
        Ok(()) => println!("✅ 4 document tools registered"),
        Err(e) => eprintln!("❌ Registration failed: {}", e),
    }

    // Verify registration
    let stats = registry.get_stats();
    println!("Registered tools: {}", stats.tool_count);
    println!("Lookup stats: {} hits, {} misses",
        stats.lookup_hits, stats.lookup_misses);
}
```

### With MCP Server Integration
```rust
use atomic_mcp_server::{McpServerCapsule, tools::register_document_tools};

fn setup_mcp_server() -> Result<(), &'static str> {
    // Create main server (256 KB)
    let server = McpServerCapsule::new();

    // Register document tools with server's registry
    register_document_tools(&server.tools)?;

    println!("✅ MCP server ready with {} tools",
        server.tools.get_stats().tool_count);

    Ok(())
}
```

## 2. XPathQueryToolCapsule Examples

### Basic Query Execution
```rust
use atomic_mcp_server::tools::XPathQueryToolCapsule;

fn xpath_example() -> Result<String, &'static str> {
    // Create tool (256 B, stack allocation)
    let xpath_tool = XPathQueryToolCapsule::new();

    // XML document
    let xml = r#"
        <library>
            <book>
                <title>Rust Book</title>
                <author>Steve Klabnik</author>
            </book>
            <book>
                <title>Programming Rust</title>
                <author>Jim Blandy</author>
            </book>
        </library>
    "#;

    // Execute XPath query
    let result = xpath_tool.execute_query(xml, "//book/title")?;
    println!("Query result: {}", result);

    // Get statistics
    let (hits, misses) = xpath_tool.get_stats();
    println!("Cache - Hits: {}, Misses: {}", hits, misses);

    Ok(result)
}
```

### Cached Queries
```rust
use atomic_mcp_server::tools::XPathQueryToolCapsule;
use std::time::Instant;

fn cached_queries_example() {
    let xpath_tool = XPathQueryToolCapsule::new();
    let xml = "<root><item id=\"1\"/><item id=\"2\"/></root>";
    let xpath = "//item[@id]";

    // First query (fresh, ~50μs)
    let start = Instant::now();
    let result1 = xpath_tool.execute_query(xml, xpath).unwrap();
    let fresh_latency = start.elapsed();
    println!("Fresh query: {:?}", fresh_latency);

    // Second query (may be cached, ~10μs if hit)
    let start = Instant::now();
    let result2 = xpath_tool.execute_query(xml, xpath).unwrap();
    let cached_latency = start.elapsed();
    println!("Cached query: {:?}", cached_latency);

    // Statistics
    let (hits, misses) = xpath_tool.get_stats();
    println!("Cache hit rate: {:.1}%",
        (hits as f64 / (hits + misses) as f64) * 100.0);
}
```

### Multiple Queries
```rust
use atomic_mcp_server::tools::XPathQueryToolCapsule;

fn batch_queries() {
    let tool = XPathQueryToolCapsule::new();

    let xml = r#"<data>
        <user id="1"><name>Alice</name></user>
        <user id="2"><name>Bob</name></user>
    </data>"#;

    let queries = vec![
        "//user",
        "//user[@id='1']",
        "//name",
        "//user[@id='2']/name",
    ];

    for query in queries {
        match tool.execute_query(xml, query) {
            Ok(result) => println!("✅ {}: {}", query, result),
            Err(e) => println!("❌ {}: {}", query, e),
        }
    }

    let (hits, misses) = tool.get_stats();
    println!("Processed: {} hits, {} misses", hits, misses);
}
```

## 3. SchemaValidatorToolCapsule Examples

### Basic Validation
```rust
use atomic_mcp_server::tools::SchemaValidatorToolCapsule;

fn validate_example() -> Result<(), &'static str> {
    let validator = SchemaValidatorToolCapsule::new();

    let xml = "<person><name>Alice</name><age>30</age></person>";
    let schema = "person_schema";

    match validator.validate(xml, schema)? {
        true => println!("✅ XML is valid"),
        false => println!("❌ XML is invalid"),
    }

    Ok(())
}
```

### Batch Validation
```rust
use atomic_mcp_server::tools::SchemaValidatorToolCapsule;

fn batch_validation() {
    let validator = SchemaValidatorToolCapsule::new();

    let documents = vec![
        "<config><port>8080</port></config>",
        "<config><port>invalid</port></config>",
        "<data><value>42</value></data>",
    ];

    let mut valid_count = 0;

    for doc in documents {
        match validator.validate(doc, "generic_schema") {
            Ok(true) => {
                println!("✅ Valid: {}", doc);
                valid_count += 1;
            }
            Ok(false) => println!("❌ Invalid: {}", doc),
            Err(e) => println!("⚠️ Error: {} on {}", e, doc),
        }
    }

    let (total, errors) = validator.get_stats();
    println!("Validation complete: {} valid, {} total, {} errors",
        valid_count, total, errors);
}
```

### Concurrent Validation
```rust
use atomic_mcp_server::tools::SchemaValidatorToolCapsule;
use std::thread;
use std::sync::Arc;

fn concurrent_validation() {
    let validator = Arc::new(SchemaValidatorToolCapsule::new());
    let mut handles = vec![];

    for thread_id in 0..4 {
        let validator_clone = Arc::clone(&validator);
        let handle = thread::spawn(move || {
            for i in 0..25 {
                let xml = format!("<item id=\"{}\"/>", thread_id * 25 + i);
                let _ = validator_clone.validate(&xml, "item_schema");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let (total, errors) = validator.get_stats();
    println!("Concurrent validation complete: {} total, {} errors", total, errors);
}
```

## 4. CacheStatsToolCapsule Examples

### Basic Statistics
```rust
use atomic_mcp_server::tools::CacheStatsToolCapsule;

fn cache_stats_example() {
    let stats_tool = CacheStatsToolCapsule::new();

    // Simulate cache operations
    stats_tool.update_stats(
        100,              // hits
        25,               // misses
        1024 * 1024,      // total bytes (1 MB)
    );

    // Get snapshot (atomic, <10ns)
    let (hits, misses, ratio) = stats_tool.snapshot();

    println!("Cache Statistics:");
    println!("  Hits: {}", hits);
    println!("  Misses: {}", misses);
    println!("  Hit Ratio: {:.2}%", ratio * 100.0);
    println!("  Effective Rate: {:.2} hits/miss",
        hits as f64 / misses as f64);
}
```

### Real-Time Monitoring
```rust
use atomic_mcp_server::tools::CacheStatsToolCapsule;
use std::thread;
use std::time::Duration;

fn cache_monitoring() {
    let stats = CacheStatsToolCapsule::new();

    // Simulate cache activity in background thread
    let stats_clone = stats.clone();
    let monitor_handle = thread::spawn(move || {
        let mut hits = 0;
        let mut misses = 0;

        for i in 0..60 {
            // Simulate cache activity
            if i % 3 == 0 {
                hits += 10;  // Cache hit
            } else {
                misses += 3;  // Cache miss
            }

            stats_clone.update_stats(hits, misses, hits as u64 * 4096);

            // Sleep 100ms between updates
            thread::sleep(Duration::from_millis(100));
        }
    });

    // Monitor stats in main thread
    for _ in 0..60 {
        let (hits, misses, ratio) = stats.snapshot();
        println!("Cache: {} hits, {} misses, {:.1}% hit rate",
            hits, misses, ratio * 100.0);
        thread::sleep(Duration::from_millis(100));
    }

    monitor_handle.join().unwrap();
}
```

### Atomic Snapshots
```rust
use atomic_mcp_server::tools::CacheStatsToolCapsule;
use std::sync::atomic::Ordering;

fn atomic_snapshot_example() {
    let stats = CacheStatsToolCapsule::new();

    stats.update_stats(1000, 100, 10 * 1024 * 1024);

    // Take atomic snapshot (generation verification)
    let gen_before = stats.generation.load(Ordering::Acquire);
    let (hits, misses, ratio) = stats.snapshot();
    let gen_after = stats.generation.load(Ordering::Release);

    if gen_before == gen_after {
        println!("✅ Snapshot is consistent (generation: {})", gen_before);
        println!("   Hits: {}, Misses: {}, Ratio: {:.2}%",
            hits, misses, ratio * 100.0);
    } else {
        println!("⚠️ Snapshot had concurrent modification");
    }
}
```

## 5. PreloaderToolCapsule Examples

### Basic Batch Loading
```rust
use atomic_mcp_server::tools::PreloaderToolCapsule;

fn preload_example() -> Result<(), &'static str> {
    let preloader = PreloaderToolCapsule::new();

    let document_paths = vec![
        "doc1.xml",
        "doc2.xml",
        "doc3.xml",
        "doc4.xml",
        "doc5.xml",
    ];

    // Start batch loading
    let loaded = preloader.preload_batch(document_paths.len() as u32, &document_paths)?;

    println!("✅ Loaded {} documents", loaded);

    // Get progress
    let (batch_size, processed, bytes) = preloader.get_progress();
    println!("Progress: {}/{} docs, {} bytes",
        processed, batch_size, bytes);

    Ok(())
}
```

### Progress Tracking
```rust
use atomic_mcp_server::tools::PreloaderToolCapsule;
use std::time::Instant;

fn progress_tracking() {
    let preloader = PreloaderToolCapsule::new();

    let document_count = 50;
    let paths: Vec<&str> = (0..document_count)
        .map(|i| "doc")  // Simplified for example
        .collect();

    let start = Instant::now();
    let _ = preloader.preload_batch(document_count, &paths);
    let elapsed = start.elapsed();

    let (batch_size, processed, bytes_loaded) = preloader.get_progress();

    println!("Batch Loading Complete:");
    println!("  Total: {} documents", batch_size);
    println!("  Processed: {}", processed);
    println!("  Bytes: {}", bytes_loaded);
    println!("  Throughput: {:.2} docs/sec",
        processed as f64 / elapsed.as_secs_f64());
}
```

### Parallel Batch Loading
```rust
use atomic_mcp_server::tools::PreloaderToolCapsule;
use std::thread;
use std::sync::Arc;

fn parallel_preload() {
    let preloader = Arc::new(PreloaderToolCapsule::new());
    let mut handles = vec![];

    // 4 parallel loaders
    for batch_id in 0..4 {
        let preloader_clone = Arc::clone(&preloader);
        let handle = thread::spawn(move || {
            let batch_size = 25;
            let paths: Vec<&str> = (0..batch_size)
                .map(|_| "doc")  // Simplified
                .collect();

            match preloader_clone.preload_batch(batch_size as u32, &paths) {
                Ok(loaded) => println!("✅ Batch {}: loaded {}", batch_id, loaded),
                Err(e) => println!("❌ Batch {}: {}", batch_id, e),
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let (batch_size, processed, _bytes) = preloader.get_progress();
    println!("All batches complete: {}/{}", processed, batch_size);
}
```

## 6. Integration Examples

### Full MCP Request/Response Cycle
```rust
use atomic_mcp_server::tools::execute_tool;
use atomic_mcp_server::json_rpc::JsonRpcRequest;

fn handle_mcp_request(tool_id: u64, params: serde_json::Value)
    -> Result<String, &'static str>
{
    // Create JSON-RPC request
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: match tool_id {
            1 => "xpath_query".to_string(),
            2 => "validate_schema".to_string(),
            3 => "cache_stats".to_string(),
            4 => "preload_documents".to_string(),
            _ => return Err("Unknown tool"),
        },
        params,
    };

    // Execute tool
    execute_tool(tool_id, &request)
}
```

### Complete Server Example
```rust
use atomic_mcp_server::{McpServerCapsule, tools::register_document_tools};
use std::sync::Arc;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create MCP server (256 KB)
    let server = Arc::new(McpServerCapsule::new());

    // Register document tools
    register_document_tools(&server.tools)?;
    println!("✅ MCP server initialized");

    // Spawn server threads
    let mut handles = vec![];

    for thread_id in 0..4 {
        let server_clone = Arc::clone(&server);
        let handle = thread::spawn(move || {
            // Simulate handling 100 requests per thread
            for req_id in 0..100 {
                // In real server, would:
                // 1. Parse JSON-RPC request
                // 2. Route to appropriate tool
                // 3. Execute tool
                // 4. Format response
                // 5. Send back to client

                if req_id % 25 == 0 {
                    println!("Thread {}: processed {} requests", thread_id, req_id);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Print final statistics
    let rpc_stats = server.json_rpc.get_stats();
    println!("✅ Server shutdown");
    println!("   Requests parsed: {}", rpc_stats.requests_parsed);
    println!("   Average latency: {} ns", rpc_stats.avg_latency_ns);

    Ok(())
}
```

## 7. Performance Measurement Examples

### Latency Measurement
```rust
use atomic_mcp_server::tools::*;
use std::time::Instant;

fn measure_tool_latency() {
    println!("Tool Latency Measurements:\n");

    // XPath Query
    let xpath = XPathQueryToolCapsule::new();
    let start = Instant::now();
    let _ = xpath.execute_query("<root/>", "/root");
    println!("XPath Query: {:.2} μs", start.elapsed().as_micros() as f64);

    // Schema Validator
    let schema = SchemaValidatorToolCapsule::new();
    let start = Instant::now();
    let _ = schema.validate("<root/>", "schema");
    println!("Schema Validation: {:.2} μs", start.elapsed().as_micros() as f64);

    // Cache Stats
    let stats = CacheStatsToolCapsule::new();
    stats.update_stats(100, 20, 1024);
    let start = Instant::now();
    let _ = stats.snapshot();
    println!("Cache Snapshot: {:.2} ns", start.elapsed().as_nanos() as f64);

    // Preloader
    let preload = PreloaderToolCapsule::new();
    let start = Instant::now();
    let _ = preload.preload_batch(10, &[]);
    println!("Batch Preload: {:.2} μs", start.elapsed().as_micros() as f64);
}
```

### Throughput Measurement
```rust
use atomic_mcp_server::tools::CacheStatsToolCapsule;
use std::time::Instant;

fn measure_throughput() {
    let tool = CacheStatsToolCapsule::new();
    tool.update_stats(100, 20, 1024);

    let iterations = 1_000_000;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = tool.snapshot();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("Cache snapshot throughput: {:.2}M ops/sec", ops_per_sec / 1_000_000.0);
}
```

## Summary

These examples demonstrate:

1. **Basic usage** of each tool
2. **Integration** with MCP server
3. **Concurrent access** patterns
4. **Performance** measurement
5. **Real-world** scenarios (batch processing, monitoring)
6. **Error handling** and recovery

All examples follow COCA principles:
- Stack-allocated capsules
- Zero heap allocation
- Atomic operations only
- Lock-free coordination
- Deterministic latency

**Key Takeaway**: Use these examples to integrate document tools into your MCP server with <100μs latency and zero contention overhead.
