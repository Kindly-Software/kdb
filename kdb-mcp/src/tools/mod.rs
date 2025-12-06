//! MCP Tools Modules
//!
//! All 12 tools are implemented in server.rs:
//!
//! Debugging tools (1-9):
//! 1. debugger/attach - Attach to process
//! 2. debugger/set_breakpoint - Add breakpoint
//! 3. debugger/continue - Resume execution
//! 4. debugger/step_forward - Single step
//! 5. debugger/step_backward - Time-travel!
//! 6. debugger/get_stack_trace - SIMD stack unwind
//! 7. debugger/get_variables - Read memory
//! 8. debugger/find_similar_bugs - T10 probabilistic
//! 9. debugger/export_trace - T5 streaming export
//!
//! Admin tools (10-12):
//! 10. debugger/quota_status - Quota tier/limits/usage (T1 Atomic, <70ns)
//! 11. debugger/license_info - License tier/validation/expiry (T1 Atomic, <10ns)
//! 12. debugger/get_comprehensive_audit - Q34 compliance audit (<10us)

// Note: Document tools (xpath_query, validate_schema, cache_stats, preload_documents)
// have been removed to reduce bloat and focus on core debugging functionality.
