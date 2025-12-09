//! Migration execution engine

use crate::MigrationContext;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub struct_name: String,
    pub elapsed_seconds: f64,
}

pub fn migrate_capsule(context: &MigrationContext, _dry_run: bool) -> Result<MigrationResult> {
    // Stub for now - full implementation in phase 2
    Ok(MigrationResult {
        struct_name: context.struct_name.clone(),
        elapsed_seconds: 0.0,
    })
}
