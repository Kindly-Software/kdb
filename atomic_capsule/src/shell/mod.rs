//! # Shell Module - Universal Shell Alias Management
//!
//! **UCE34 Framework: Q1-Q34 Systematic Discovery**
//!
//! ## Phase 1: Problem Understanding (Q1-Q9)
//!
//! **Q1: What specific problem are we solving?**
//! - Shell alias management is manual and error-prone (editing .bashrc, .zshrc, .fish)
//! - No atomic updates (concurrent edits can corrupt config files)
//! - No conflict detection (duplicate aliases overwrite each other)
//! - No validation (typos, missing commands)
//! - No audit trail (who added what when)
//! - No multi-shell support (must edit 3 different files)
//!
//! **Q2: What are the constraints?**
//! - MUST be 100% lockfree (atomic_capsule mandate)
//! - MUST use DaemonCoordinatorCapsule for multi-process coordination
//! - MUST support multiple shells (.bashrc, .zshrc, .fish)
//! - MUST validate aliases (command exists, name valid)
//! - MUST detect conflicts (duplicate names)
//! - MUST provide atomic updates (no partial writes)
//! - MUST have audit trail (Q34 compliance)
//!
//! **Q3: What are the inputs/outputs?**
//! - Input: Alias name + target command
//! - Output: Updated shell config files atomically
//!
//! **Q4: What is the data shape?**
//! ```ignore
//! Alias {
//!     name: String,           // "g"
//!     command: String,        // "git-coordinated-v2"
//!     shell: ShellType,       // Bash/Zsh/Fish
//!     added_by_pid: u32,      // Process that added it
//!     timestamp: u64,         // When added
//! }
//! ```
//!
//! **Q5: What are the edge cases?**
//! - Duplicate alias names (conflict)
//! - Alias to non-existent command
//! - Invalid alias names (spaces, special chars)
//! - Concurrent updates to same shell config
//! - Shell config file doesn't exist
//! - Permission denied
//!
//! **Q6: What is the complexity?**
//! - Parsing: Moderate (shell syntax varies)
//! - Validation: Simple (command lookup, name check)
//! - Coordination: Moderate (DaemonCapsule integration)
//! - Overall: 4-5/10 complexity
//!
//! **Q7: What are the performance requirements?**
//! - Add alias: <1ms (not latency-critical)
//! - List aliases: <100μs
//! - Atomic update: <10ms (file write)
//! - Conflict detection: <100ns (hash lookup)
//!
//! **Q8: What is the failure mode?**
//! - Add fails → Config unchanged, error returned
//! - Validation fails → Error before write
//! - Concurrent update → DaemonCapsule coordinates
//! - File corrupted → Restore from backup
//!
//! **Q9: What are the dependencies?**
//! - DaemonCoordinatorCapsule (for coordination)
//! - File I/O (shell config read/write)
//! - Command validation (which/type lookup)
//! - Shell detection (current shell)
//!
//! ## Phase 2: Tier Selection (Q10-Q12)
//!
//! **Q10a: PROFILE FIRST**
//! - Not applicable (new feature, not optimizing)
//! - Bottleneck: File I/O (10ms), not computation
//!
//! **Q10b: ANALYZE BOTTLENECK**
//! - File write dominates (10ms)
//! - Coordination overhead should be <1% (<100μs)
//! - Parsing is negligible (<100μs)
//!
//! **Q10c: CHOOSE TIER**
//!
//! Evaluate tiers:
//! - **T0 (Auditable)**: ✅ YES - Audit who added which alias
//! - **T1 (Atomic)**: ✅ YES - DaemonCapsule for coordination
//! - **T2 (SIMD)**: ❌ NO - No vectorizable operations
//! - **T3 (Fixed-Point)**: ❌ NO - No arithmetic
//! - **T4 (Batch)**: ❌ NO - Aliases added individually
//! - **T5 (Streaming)**: ❌ NO - Not incremental
//! - **T6 (Mixed)**: ✅ MAYBE - T0 + T1 + T9 (persistent shell config)
//! - **T9 (Persistent)**: ✅ YES - Shell config files are persistent
//! - Other tiers: ❌ NO
//!
//! **DECISION**: **T6 Mixed (T0 Auditable + T1 Atomic + T9 Persistent)**
//!
//! **Speedup target**: Not about speed, about **correctness** (atomic updates, no corruption)
//!
//! **Q11: Rust transformation - how do we implement this lockfree?**
//!
//! Core algorithm:
//! ```ignore
//! impl AliasCapsule {
//!     pub fn add_alias(&self, name: &str, command: &str) -> Result<(), AliasError> {
//!         // 1. Validate inputs
//!         self.validate_alias_name(name)?;
//!         self.validate_command_exists(command)?;
//!
//!         // 2. Acquire coordination lock via DaemonCapsule
//!         let _guard = self.coordinator.acquire()?;
//!
//!         // 3. Read current config
//!         let config = self.read_shell_config()?;
//!
//!         // 4. Check for conflicts
//!         if config.has_alias(name) {
//!             return Err(AliasError::AlreadyExists { name: name.to_string() });
//!         }
//!
//!         // 5. Add alias
//!         let new_alias = Alias { ... };
//!
//!         // 6. Write atomically (tmp file + rename)
//!         self.write_shell_config_atomic(&config.with_alias(new_alias))?;
//!
//!         // 7. Audit log
//!         // (DaemonCoordinatorCapsule handles this automatically)
//!
//!         Ok(())
//!     }
//! }
//! ```
//!
//! **Q12: Nightly features needed?**
//! - ❌ NO nightly features required (stable Rust sufficient)
//!
//! ## Modules
//! - **error**: Error types for alias operations
//! - **parser**: Shell config parsing (Bash/Zsh/Fish)
//! - **alias**: AliasCapsule implementation (T6 Mixed)

pub mod error;
pub mod parser;
pub mod alias;

pub use error::{AliasError, AliasResult};
pub use parser::{ShellType, ShellConfig, Alias};
pub use alias::AliasCapsule;
