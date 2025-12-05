//! MCP Tools Modules
//!
//! Debugging tools (in server.rs):
//! 1. debugger/attach - Attach to process
//! 2. debugger/set_breakpoint - Add breakpoint
//! 3. debugger/continue - Resume execution
//! 4. debugger/step_forward - Single step
//! 5. debugger/step_backward - Time-travel!
//! 6. debugger/get_stack_trace - SIMD stack unwind
//! 7. debugger/get_variables - Read memory
//! 8. debugger/find_similar_bugs - T10 probabilistic
//! 9. debugger/export_trace - T5 streaming export
//! 10. debugger/quota_status - Quota tier/limits/usage (T1 Atomic, <70ns)
//! 11. debugger/license_info - License tier/validation/expiry (T1 Atomic, <10ns)
//!
//! Document tools (in document.rs):
//! 1. xpath_query - XPath XML queries (T6 Mixed)
//! 2. validate_schema - XML schema validation (T2 SIMD)
//! 3. cache_stats - Cache statistics (T0 Auditable)
//! 4. preload_documents - Batch document loading (T4 Batch)

pub mod document;

// Re-export key types for convenience
pub use document::{
    XPathQueryToolCapsule, SchemaValidatorToolCapsule, CacheStatsToolCapsule,
    PreloaderToolCapsule, RequestContextCapsule, ResponseBuilderCapsule,
    CacheStatsSnapshot,
};

#[cfg(feature = "tool-executor")]
pub use document::{register_document_tools, execute_tool};
