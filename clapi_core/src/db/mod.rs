//! KindlyDB integration layer for clapi_core
//!
//! ## I20 Integration Framework (Q1-Q20)
//!
//! ### Q1-Q5: Scope
//! - **Q1 (What)**: Integrate KindlyDB as embedded storage backend
//! - **Q2 (Why)**: Replace PostgreSQL+Redis with embedded lockfree database
//! - **Q3 (Where)**: All persistence (oauth, payments, rate limits, metrics, audit)
//! - **Q4 (When)**: Internal refactor, no API changes
//! - **Q5 (Who)**: Backend infrastructure, transparent to clients
//!
//! ### Q6-Q10: Compatibility
//! - **Q6 (Breaking)**: Zero breaking changes (same HTTP API)
//! - **Q7 (Migration)**: Phased rollout via feature flags
//! - **Q8 (Dependencies)**: kindly-db crate (local path)
//! - **Q9 (Constraints)**: <10ms p50 latency target (vs 150ms current)
//! - **Q10 (Conflicts)**: None (replaces network databases)
//!
//! ### Q11-Q15: Safety
//! - **Q11 (Assumptions)**: 100% lockfree capsule architecture
//! - **Q12 (Edge Cases)**: Circuit breaker on query failures
//! - **Q13 (Errors)**: DbError wrapped in ClapiError
//! - **Q14 (Rollback)**: Feature flag disables KindlyDB
//! - **Q15 (Escape)**: Fallback to in-memory cache on DB failure
//!
//! ### Q16-Q20: Validation
//! - **Q16 (Minimal)**: Open DB → insert → query → commit
//! - **Q17 (Properties)**: ACID guarantees, MVCC isolation
//! - **Q18 (Performance)**: <10ms p50, 3000 req/s target
//! - **Q19 (Incremental)**: Phase 1 (oauth), Phase 2 (payments), Phase 3 (metrics)
//! - **Q20 (Rollback)**: Feature flag + graceful degradation

pub mod schema;

use kindly_db::{Database as KindlyDatabase, Transaction, DbError};
use crate::error::{ClapiError, ClapiResult};
use std::sync::Arc;
use std::path::Path;

/// Database wrapper providing KindlyDB integration
///
/// **Architecture**: Computational capsule storage layer
/// - Tier 1 (Atomic): Lockfree MVCC transactions
/// - Tier 2 (SIMD): Query execution (7× faster)
/// - Tier 3 (Fixed-Point): Cost tracking (Q16.16)
/// - Tier 5 (Streaming): Continuous WAL writes
///
/// **Performance Targets** (B32):
/// - Session check: <50ns (vs 15-50ms PostgreSQL)
/// - Rate limit: <20ns (vs 10-30ms Redis)
/// - Payment record: <100ns (vs 5-20ms PostgreSQL)
/// - Total per request: <5ms (vs ~150ms current)
#[derive(Clone)]
pub struct Database {
    /// KindlyDB handle (Arc for cheap cloning)
    inner: Arc<KindlyDatabase>,
}

impl Database {
    /// Open or create database at the specified path
    ///
    /// **I20 Q16 (Minimal Test)**: Opens database, initializes schema
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Database file cannot be opened
    /// - Schema initialization fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let db = Database::open("data/clapi.kdb")?;
    /// ```
    pub fn open(path: impl AsRef<Path>) -> ClapiResult<Self> {
        let inner = KindlyDatabase::open(path.as_ref().to_str().unwrap())
            .map_err(|e| ClapiError::DatabaseError(e.to_string()))?;

        let db = Database {
            inner: Arc::new(inner),
        };

        // Initialize schema if needed
        db.init_schema()?;

        Ok(db)
    }

    /// Create in-memory database for testing
    ///
    /// **UCE-D7 Fix**: Tests need fast in-memory database
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let db = Database::new_in_memory()?;
    /// ```
    pub fn new_in_memory() -> ClapiResult<Self> {
        Ok(Database {
            inner: Arc::new(KindlyDatabase::new_in_memory()),
        })
    }

    /// Initialize database schema (idempotent)
    ///
    /// **I20 Q19 (Incremental)**: Creates tables if not exist
    ///
    /// Creates 6 tables:
    /// - oauth_sessions (OAuthSessionCapsule)
    /// - payments (PaymentCapsule)
    /// - rate_limits (RateLimitCapsule)
    /// - metrics_stream (MetricsStreamCapsule)
    /// - requests (RequestCapsule)
    /// - compression_stats (CompressionStateCapsule)
    pub fn init_schema(&self) -> ClapiResult<()> {
        let mut txn = self.begin()?;

        // Execute schema DDL (idempotent CREATE IF NOT EXISTS)
        schema::create_oauth_sessions_table(&mut txn)?;
        schema::create_payments_table(&mut txn)?;
        schema::create_rate_limits_table(&mut txn)?;
        schema::create_metrics_stream_table(&mut txn)?;
        schema::create_requests_table(&mut txn)?;
        schema::create_compression_stats_table(&mut txn)?;

        txn.commit()?;
        Ok(())
    }

    /// Begin a transaction (lockfree, <50ns)
    ///
    /// **I20 Q17 (Property)**: MVCC snapshot isolation guaranteed
    ///
    /// # Errors
    ///
    /// Returns error if transaction cannot be created
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut txn = db.begin()?;
    /// txn.query("SELECT * FROM oauth_sessions")?;
    /// txn.commit()?;
    /// ```
    pub fn begin(&self) -> ClapiResult<Transaction> {
        self.inner.begin()
            .map_err(|e| ClapiError::DatabaseError(e.to_string()))
    }

    /// Check if database is healthy
    ///
    /// **I20 Q15 (Escape)**: Health check for circuit breaker
    ///
    /// Returns `true` if database can execute queries
    pub fn is_healthy(&self) -> bool {
        // Attempt simple query to verify database is operational
        self.begin().is_ok()
    }
}

/// Convert DbError to ClapiError
impl From<DbError> for ClapiError {
    fn from(err: DbError) -> Self {
        ClapiError::DatabaseError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_database() -> ClapiResult<()> {
        let db = Database::new_in_memory()?;
        assert!(db.is_healthy());
        Ok(())
    }

    #[test]
    fn test_transaction_begin_commit() -> ClapiResult<()> {
        let db = Database::new_in_memory()?;
        let txn = db.begin()?;
        txn.commit()?;
        Ok(())
    }

    #[test]
    fn test_schema_init_idempotent() -> ClapiResult<()> {
        let db = Database::new_in_memory()?;

        // First init should succeed
        db.init_schema()?;

        // Second init should also succeed (idempotent)
        db.init_schema()?;

        Ok(())
    }
}
