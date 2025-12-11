//! Environment Variable Resolution Module
//!
//! Multi-source environment variable resolution with Q34 audit trail.
//!
//! ## Modules
//!
//! - `resolution` - T0 Auditable EnvResolutionCapsule (4KB)
//! - `dotenv_parser` - T1 Atomic DotenvParserCapsule (256B)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use kdb_mcp::configure::env::{EnvResolutionCapsule, EnvSource, DotenvParserCapsule};
//!
//! let resolver = EnvResolutionCapsule::new();
//!
//! // Resolve with fallback
//! let port = resolver.resolve_or("KDB_PORT", "8081");
//! println!("Port: {} (from {:?})", port.value, port.source);
//!
//! // Parse .env file
//! let parser = DotenvParserCapsule::new();
//! let content = "KEY=value\nOTHER=\"quoted\"";
//! let result = parser.parse(content, ".env");
//! for (key, value) in &result.variables {
//!     println!("{} = {}", key, value);
//! }
//! ```

mod resolution;
mod dotenv_parser;

pub use resolution::{
    // Core types
    EnvResolutionCapsule,
    EnvSource,
    ResolvedVariable,
    EnvStats,
    EnvResolutionError,
    // Utility functions
    fnv1a_hash,
    is_secret_key,
};

pub use dotenv_parser::{
    // Core types
    DotenvParserCapsule,
    ParsedEnvFile,
    ParseError,
    ErrorSeverity,
};
