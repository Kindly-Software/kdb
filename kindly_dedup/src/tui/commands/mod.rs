//! TUI Command Workflows
//!
//! Complete E2E workflows for all 6 commands:
//! - /demo: Production demo wizard (3-tier validation)
//! - /dedup: Interactive deduplication workflow
//! - /verify: Audit trail validation (Q34 compliance)
//! - /benchmark: Performance validation suite (B32)
//! - /stats: Statistics analysis and visualization
//! - /help: Interactive help system
//!
//! **design**: Container modules coordinating DedupPipeline + components
//! **Framework Compliance**: UCE34 (Q1-Q34), COCA (100% lockfree primitives)

pub mod benchmark;
pub mod dedup;
pub mod demo;
pub mod help;
pub mod stats;
pub mod verify;

// Re-export main entry points
pub use benchmark::run_benchmark;
pub use dedup::run_dedup;
pub use demo::run_demo;
pub use help::run_help;
pub use stats::run_stats;
pub use verify::run_verify;
