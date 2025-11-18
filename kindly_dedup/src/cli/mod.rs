//! CLI Framework - kindly_dedup User Experience
//!
//! # Purpose
//! Provides a CliCapsule-based command-line interface for kindly_dedup with:
//! - Zero-dependency argument parsing (migrated from clap)
//! - Compile-time verification via CommandSpec builder
//! - Complete argument validation and help text
//! - Production-ready error handling
//! - META_CAPSULE protection integration (Q34 audit trail)
//!
//! # Design Principles
//! - Zero Dependencies: CliCapsule from atomic_capsule (no clap directly)
//! - Type Safety: Enum parsing for command validation
//! - Comprehensive Help: Every command includes examples and validation
//! - Silent Protection: Checkpoints never block UX (<200ns overhead)
//! - UCE34 Compliance: Q31 (Simplicity), Q32 (Constraints), Q33 (Validation), Q34 (Auditability)
//!
//! # Framework Compliance
//! - UCE34: Q1-Q34 (T0 auditable tier selection, builder patterns, Q34 audit trail)
//! - ASSUM: 99.99% safe (no unsafe code)
//! - COCA: 100% lockfree (atomic_capsule primitives only)
//! - I20: 20/20 integration questions (protection + TUI composition)

pub mod args_new;
pub mod dispatch;
pub mod protection_integration;

// Re-export new CliCapsule-based types
pub use args_new::{
    build_cli, parse_cli, BenchmarkArgs, BenchmarkSuite, Commands, CorpusSize, DedupArgs, DemoArgs, DemoMode,
    GlobalArgs, HelpArgs, OutputFormat, StatsArgs, VerifyArgs,
};

pub use dispatch::{dispatch, CommandHash};
pub use protection_integration::{
    checkpoint_after_phase, checkpoint_before_command, init_protection_silent, sanitize_protection_error,
};
