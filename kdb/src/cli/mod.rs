//! KDB CLI Module - REPL interface for debugger
//!
//! Architecture: 4 T0 Auditable Capsules
//! 1. CLICapsule - Main coordination
//! 2. CommandDispatcherCapsule - Command routing
//! 3. REPLCapsule - Interactive loop
//! 4. AuditLogCapsule - Q34 hash-chain logging

pub mod audit;
pub mod commands;
// pub mod dispatcher;  // TODO: Fix PtraceWrapperCapsule imports (not needed for MCP/REST audit metrics)
pub mod repl;

pub use audit::AuditLogCapsule;
pub use commands::Command;
// pub use dispatcher::CommandDispatcherCapsule;  // TODO: Re-enable after fixing imports
pub use repl::REPLCapsule;
