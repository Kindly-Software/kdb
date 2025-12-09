//! KindlyDB Client Integration
//!
//! Async HTTP client for KindlyDB user and audit storage.
//!
//! # Connection Details
//!
//! - Host: kindly-hub (192.168.0.38)
//! - Port: 8080 (KindlyDB HTTP API)
//! - Protocol: HTTP JSON
//!
//! # Features
//!
//! - User record CRUD operations
//! - Q34-compliant audit logging with hash chain
//! - Mock client for testing
//!
//! # Example
//!
//! ```rust,ignore
//! use kdb_signup::db::{KindlyDbClient, User};
//!
//! let client = KindlyDbClient::from_env()?;
//! let user_id = client.create_user(&user).await?;
//! ```

mod kindlydb_client;

pub use kindlydb_client::{
    compute_audit_hash, DbError, KindlyDbClient, MockKindlyDbClient, SignupAuditEntry, User,
};
