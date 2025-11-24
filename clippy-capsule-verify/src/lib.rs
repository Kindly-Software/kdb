//! # Clippy Capsule Verification Lint
//!
//! **Custom Clippy lint for detecting unverified computational capsules.**

#![feature(rustc_private)]
#![warn(missing_docs, rust_2018_idioms)]

#[allow(unused_extern_crates)]
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

mod capsule_lint;
mod utils;
mod size_validation;
mod diagnostics;
mod assum_diagnostics;
mod alignment_violation;
mod atomic_field_violation;
mod mutex_violation;
mod generation_violation;
mod memory_ordering_violation;
mod scattered_atomics_violation;
mod padding_violation;
mod assum_violation;
// Still to implement: repr_c_violation, toctou_violation

use rustc_lint::LintStore;
use rustc_session::Session;

/// Registers all capsule verification lints with rustc
///
/// This function is called by rustc when loading the plugin.
/// It registers 9 lints across 3 priority levels:
/// - P0 Critical (Deny): 4 lints
/// - P1 High (Warn): 3 lints
/// - P2 Medium (Allow): 2 lints
#[no_mangle]
pub fn register_lints(_sess: &Session, lint_store: &mut LintStore) {
    lint_store.register_lints(&[
        // P0 Critical - Deny level
        mutex_violation::CAPSULE_MUTEX_VIOLATION,
        alignment_violation::CAPSULE_UNALIGNED_VIOLATION,
        generation_violation::CAPSULE_MISSING_GENERATION,
        atomic_field_violation::CAPSULE_NON_ATOMIC_FIELD,
        // P1 High - Warn level
        capsule_lint::MISSING_CAPSULE_VERIFICATION,
        scattered_atomics_violation::CAPSULE_SCATTERED_ATOMICS,
        padding_violation::CAPSULE_INCORRECT_PADDING,
        // P2 Medium - Allow level (opt-in)
        memory_ordering_violation::CAPSULE_MEMORY_ORDERING,
        assum_violation::CAPSULE_MISSING_ASSUM,
    ]);
    lint_store.register_late_pass(|_| Box::new(mutex_violation::CapsuleLockfreeViolation));
    lint_store.register_late_pass(|_| Box::new(alignment_violation::CapsuleAlignmentViolation));
    lint_store.register_late_pass(|_| Box::new(generation_violation::CapsuleGenerationViolation));
    lint_store.register_late_pass(|_| Box::new(atomic_field_violation::CapsuleAtomicFieldViolation));
    lint_store.register_late_pass(|_| Box::new(capsule_lint::MissingCapsuleVerification));
    lint_store.register_late_pass(|_| Box::new(memory_ordering_violation::CapsuleMemoryOrderingViolation));
    lint_store.register_late_pass(|_| Box::new(scattered_atomics_violation::CapsuleScatteredAtomics));
    lint_store.register_late_pass(|_| Box::new(padding_violation::CapsulePaddingViolation));
    lint_store.register_late_pass(|_| Box::new(assum_violation::CapsuleAssumViolation));
}

/// Crate version from Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
