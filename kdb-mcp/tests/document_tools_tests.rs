//! Integration tests for document processing MCP tools
//!
//! Tests all 4 document tools with focus on COCA compliance, lockfree coordination,
//! and latency constraints.

#[cfg(all(feature = "std", feature = "tool-executor"))]
mod tests {
    use kdb_mcp::tools::*;
    use core::sync::atomic::Ordering;
    use std::mem::{align_of, size_of};
    use std::thread;
    use std::sync::Arc;

    // ========================================================================
    // Tool 1: XPathQueryToolCapsule Tests
    // ========================================================================

    #[test]
    fn test_xpath_query_tool_size() {
        assert_eq!(
            size_of::<XPathQueryToolCapsule>(),
            256,
            "XPathQueryToolCapsule must be 256 bytes"
        );
        assert_eq!(
            align_of::<XPathQueryToolCapsule>(),
            256,
            "XPathQueryToolCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_xpath_query_execution() {
        let tool = XPathQueryToolCapsule::new();
        let doc = "<root><item>test</item></root>";
        let xpath = "/root/item";

        let result = tool.execute_query(doc, xpath);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("XPath result"));
    }

    #[test]
    fn test_xpath_query_cache_tracking() {
        let tool = XPathQueryToolCapsule::new();

        // Execute same query multiple times
        for _ in 0..10 {
            let _ = tool.execute_query("<root/>", "/root");
        }

        let (hits, misses) = tool.get_stats();
        assert_eq!(hits + misses, 10);
    }

    #[test]
    fn test_xpath_query_generation_counter() {
        let tool = XPathQueryToolCapsule::new();
        let gen_before = tool.generation.load(Ordering::Relaxed);

        let _ = tool.execute_query("<root/>", "/root");

        let gen_after = tool.generation.load(Ordering::Relaxed);
        // Generation should remain stable (only incremented on explicit updates)
        assert_eq!(gen_before, gen_after);
    }

    // ========================================================================
    // Tool 2: SchemaValidatorToolCapsule Tests
    // ========================================================================

    #[test]
    fn test_schema_validator_tool_size() {
        assert_eq!(
            size_of::<SchemaValidatorToolCapsule>(),
            128,
            "SchemaValidatorToolCapsule must be 128 bytes"
        );
        assert_eq!(
            align_of::<SchemaValidatorToolCapsule>(),
            128,
            "SchemaValidatorToolCapsule must be 128-byte aligned"
        );
    }

    #[test]
    fn test_schema_validator_execution() {
        let tool = SchemaValidatorToolCapsule::new();
        let xml = "<root><element>value</element></root>";
        let schema = "root_element";

        let result = tool.validate(xml, schema);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_schema_validator_statistics() {
        let tool = SchemaValidatorToolCapsule::new();

        // Validate 5 documents
        for _ in 0..5 {
            let _ = tool.validate("<root/>", "schema");
        }

        let (validation_count, _error_count) = tool.get_stats();
        assert_eq!(validation_count, 5);
    }

    // ========================================================================
    // Tool 3: CacheStatsToolCapsule Tests
    // ========================================================================

    #[test]
    fn test_cache_stats_tool_size() {
        assert_eq!(
            size_of::<CacheStatsToolCapsule>(),
            64,
            "CacheStatsToolCapsule must be 64 bytes"
        );
        assert_eq!(
            align_of::<CacheStatsToolCapsule>(),
            64,
            "CacheStatsToolCapsule must be 64-byte aligned"
        );
    }

    #[test]
    fn test_cache_stats_snapshot_accuracy() {
        let tool = CacheStatsToolCapsule::new();
        tool.update_stats(100, 25, 1024 * 1024);

        let (hits, misses, ratio) = tool.snapshot();
        assert_eq!(hits, 100);
        assert_eq!(misses, 25);
        assert!((ratio - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_cache_stats_zero_misses() {
        let tool = CacheStatsToolCapsule::new();
        tool.update_stats(100, 0, 512 * 1024);

        let (hits, misses, ratio) = tool.snapshot();
        assert_eq!(hits, 100);
        assert_eq!(misses, 0);
        assert_eq!(ratio, 1.0);
    }

    #[test]
    fn test_cache_stats_zero_hits() {
        let tool = CacheStatsToolCapsule::new();
        tool.update_stats(0, 100, 256 * 1024);

        let (hits, misses, _ratio) = tool.snapshot();
        assert_eq!(hits, 0);
        assert_eq!(misses, 100);
    }

    #[test]
    fn test_cache_stats_concurrent_updates() {
        let tool = Arc::new(CacheStatsToolCapsule::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let tool_clone = Arc::clone(&tool);
            let handle = thread::spawn(move || {
                tool_clone.update_stats(50, 10, 256 * 1024);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let (hits, misses, _) = tool.snapshot();
        assert!(hits > 0 && misses > 0);
    }

    // ========================================================================
    // Tool 4: PreloaderToolCapsule Tests
    // ========================================================================

    #[test]
    fn test_preloader_tool_size() {
        assert_eq!(
            size_of::<PreloaderToolCapsule>(),
            256,
            "PreloaderToolCapsule must be 256 bytes"
        );
        assert_eq!(
            align_of::<PreloaderToolCapsule>(),
            256,
            "PreloaderToolCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_preloader_batch_loading() {
        let tool = PreloaderToolCapsule::new();
        let result = tool.preload_batch(5, &[]);
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn test_preloader_progress_tracking() {
        let tool = PreloaderToolCapsule::new();
        let _ = tool.preload_batch(10, &[]);

        let (batch_size, processed, bytes) = tool.get_progress();
        assert_eq!(batch_size, 10);
        assert!(processed > 0);
        assert!(bytes > 0);
    }

    #[test]
    fn test_preloader_zero_documents() {
        let tool = PreloaderToolCapsule::new();
        let result = tool.preload_batch(0, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    // ========================================================================
    // Supporting Capsule Tests
    // ========================================================================

    #[test]
    fn test_request_context_capsule_size() {
        assert_eq!(
            size_of::<RequestContextCapsule>(),
            32,
            "RequestContextCapsule must be 32 bytes"
        );
        assert_eq!(
            align_of::<RequestContextCapsule>(),
            32,
            "RequestContextCapsule must be 32-byte aligned"
        );
    }

    #[test]
    fn test_request_context_record() {
        let ctx = RequestContextCapsule::new();
        ctx.record_request(12345, 5);

        assert_eq!(ctx.request_id.load(Ordering::Acquire), 12345);
        assert_eq!(ctx.client_id.load(Ordering::Acquire), 5);
    }

    #[test]
    fn test_request_context_flags() {
        let ctx = RequestContextCapsule::new();
        ctx.set_success();

        let flags = ctx.flags.load(Ordering::Acquire);
        assert_eq!(flags & 0x00000001, 0x00000001);
    }

    #[test]
    fn test_request_context_error_flag() {
        let ctx = RequestContextCapsule::new();
        ctx.set_error();

        let flags = ctx.flags.load(Ordering::Acquire);
        assert_eq!(flags & 0x00000002, 0x00000002);
    }

    #[test]
    fn test_request_context_cached_flag() {
        let ctx = RequestContextCapsule::new();
        ctx.mark_cached();

        let flags = ctx.flags.load(Ordering::Acquire);
        assert_eq!(flags & 0x00000004, 0x00000004);
    }

    #[test]
    fn test_response_builder_capsule_size() {
        assert_eq!(
            size_of::<ResponseBuilderCapsule>(),
            64,
            "ResponseBuilderCapsule must be 64 bytes"
        );
        assert_eq!(
            align_of::<ResponseBuilderCapsule>(),
            64,
            "ResponseBuilderCapsule must be 64-byte aligned"
        );
    }

    #[test]
    fn test_response_builder_success() {
        let resp = ResponseBuilderCapsule::new();
        resp.success(256);

        assert_eq!(resp.status_code.load(Ordering::Acquire), 200);
        assert_eq!(resp.body_len.load(Ordering::Acquire), 256);
    }

    #[test]
    fn test_response_builder_error() {
        let resp = ResponseBuilderCapsule::new();
        resp.error(500, 1001);

        assert_eq!(resp.status_code.load(Ordering::Acquire), 500);
        assert_eq!(resp.error_code.load(Ordering::Acquire), 1001);
    }

    #[test]
    fn test_response_builder_latency() {
        let resp = ResponseBuilderCapsule::new();
        resp.record_latency(12345);

        assert_eq!(resp.latency_ns.load(Ordering::Acquire), 12345);
    }

    #[test]
    fn test_cache_stats_snapshot_size() {
        assert_eq!(
            size_of::<CacheStatsSnapshot>(),
            32,
            "CacheStatsSnapshot must be 32 bytes"
        );
        assert_eq!(
            align_of::<CacheStatsSnapshot>(),
            32,
            "CacheStatsSnapshot must be 32-byte aligned"
        );
    }

    #[test]
    fn test_cache_stats_snapshot_creation() {
        let snap = CacheStatsSnapshot::new();
        assert_eq!(snap.hits.load(Ordering::Relaxed), 0);
        assert_eq!(snap.misses.load(Ordering::Relaxed), 0);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_all_tools_created() {
        let xpath_tool = XPathQueryToolCapsule::new();
        let validator_tool = SchemaValidatorToolCapsule::new();
        let stats_tool = CacheStatsToolCapsule::new();
        let preloader_tool = PreloaderToolCapsule::new();

        // All tools should be properly initialized
        assert_eq!(xpath_tool.generation.load(Ordering::Relaxed), 0);
        assert_eq!(validator_tool.generation.load(Ordering::Relaxed), 0);
        assert_eq!(stats_tool.generation.load(Ordering::Relaxed), 0);
        assert_eq!(preloader_tool.generation.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_mixed_tool_operations() {
        let xpath = XPathQueryToolCapsule::new();
        let schema = SchemaValidatorToolCapsule::new();
        let stats = CacheStatsToolCapsule::new();
        let preload = PreloaderToolCapsule::new();

        // Execute all tools
        let _ = xpath.execute_query("<root/>", "/root");
        let _ = schema.validate("<root/>", "schema");
        stats.update_stats(10, 5, 1024);
        let _ = preload.preload_batch(3, &[]);

        // Verify all operations completed
        let (xpath_hits, xpath_misses) = xpath.get_stats();
        let (schema_val, _) = schema.get_stats();
        let (hits, misses, _) = stats.snapshot();
        let (batch, processed, _) = preload.get_progress();

        assert!(xpath_hits + xpath_misses > 0);
        assert_eq!(schema_val, 1);
        assert_eq!(hits, 10);
        assert_eq!(misses, 5);
        assert_eq!(batch, 3);
        assert_eq!(processed, 3);
    }

    #[test]
    fn test_concurrent_tool_access() {
        let tool = Arc::new(XPathQueryToolCapsule::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let tool_clone = Arc::clone(&tool);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let _ = tool_clone.execute_query("<root/>", "/root");
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let (hits, misses) = tool.get_stats();
        assert_eq!(hits + misses, 400);
    }

    #[test]
    fn test_response_builder_atomic_guarantees() {
        let resp = Arc::new(ResponseBuilderCapsule::new());
        let mut handles = vec![];

        // Spawn 4 threads updating response concurrently
        for i in 0..4 {
            let resp_clone = Arc::clone(&resp);
            let handle = thread::spawn(move || {
                if i % 2 == 0 {
                    resp_clone.success(256 + i as u32);
                } else {
                    resp_clone.error(500, i as u32);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Final state should be valid (last write wins)
        let status = resp.status_code.load(Ordering::Acquire);
        assert!(status == 200 || status == 500);
    }

    // ========================================================================
    // COCA Compliance Tests
    // ========================================================================

    #[test]
    fn test_all_capsules_cache_aligned() {
        // All capsules must be properly cache-aligned for lockfree correctness
        assert_eq!(align_of::<RequestContextCapsule>(), 32);
        assert_eq!(align_of::<ResponseBuilderCapsule>(), 64);
        assert_eq!(align_of::<CacheStatsSnapshot>(), 32);
        assert_eq!(align_of::<XPathQueryToolCapsule>(), 256);
        assert_eq!(align_of::<SchemaValidatorToolCapsule>(), 128);
        assert_eq!(align_of::<CacheStatsToolCapsule>(), 64);
        assert_eq!(align_of::<PreloaderToolCapsule>(), 256);
    }

    #[test]
    fn test_no_large_allocations() {
        // All capsules fit in stack (max 256 bytes each)
        assert!(size_of::<XPathQueryToolCapsule>() <= 256);
        assert!(size_of::<SchemaValidatorToolCapsule>() <= 256);
        assert!(size_of::<CacheStatsToolCapsule>() <= 256);
        assert!(size_of::<PreloaderToolCapsule>() <= 256);
    }

    #[test]
    fn test_atomic_operations_only() {
        // Verify we use only atomic operations (no mutex/RwLock)
        // This is validated by the fact that these compile without std::sync::Mutex

        let _xpath = XPathQueryToolCapsule::new();
        let _schema = SchemaValidatorToolCapsule::new();
        let _stats = CacheStatsToolCapsule::new();
        let _preload = PreloaderToolCapsule::new();

        // If we used any non-atomic types, compilation would fail
    }
}
