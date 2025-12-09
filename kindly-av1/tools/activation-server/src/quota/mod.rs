//! Quota Module - Usage Tracking and Metering
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! ## Purpose
//!
//! Track video encoding usage (minutes processed) per user with monthly quotas:
//! - Basic: 10 min/month
//! - Pro: 200 min/month
//! - Ultra: 1000 min/month
//!
//! ## Architecture
//!
//! - QuotaTrackerCapsule (256B cache-aligned) per API key
//! - SQLite storage for persistent usage data
//! - Lockfree usage updates (<20ns atomic increments)
//! - Monthly reset logic (automatic on first day of month)
//!
//! ## Modules
//!
//! - `tiers` - QuotaTrackerCapsule tier mapping and usage tracking
//! - `metering` - Usage metering with SQLite persistence
//!
//! ## Framework Compliance
//!
//! - UCE34 Q10: T1 Atomic tier (QuotaTrackerCapsule)
//! - Chaos: 100% lockfree hot path (<10ns checks, <20ns increments)
//! - ASSUM: Usage persistence guaranteed (SQLite ACID)
//! - T28: Unit tests for quota enforcement

pub mod metering;
pub mod tiers;

pub use metering::{UsageMeteringSystem, UsageRecord, MeteringError};
pub use tiers::{QuotaManager, QuotaCheckResult};
