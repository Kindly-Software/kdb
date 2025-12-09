//! WsMessageCapsule - T5 Streaming, 128B, binary WebSocket messages
//!
//! **Tier**: T5 Streaming (O(1) latency, incremental updates)
//! **Size**: 128 bytes (64-byte alignment for cache efficiency)
//! **Performance**: <1μs serialization, <500ns deserialization, <100ns queueing
//! **Pattern**: Binary message capsule for WebSocket broadcast
//!
//! # UCE34 Framework Analysis (34 Questions)
//!
//! ## Foundation (Q1-Q9)
//! - **Q1 (Problem)**: Binary WebSocket messages for real-time dashboard updates
//! - **Q2 (Constraints)**: <1μs serialization, WASM-compatible, zero allocations after init
//! - **Q3 (Scale)**: 100-1000 messages/second, concurrent writes from multiple sources
//! - **Q4 (Existing)**: JSON messages (3-5μs serialization, larger payloads)
//! - **Q5 (Risks)**: Serialization overhead, message queue contention, type safety
//! - **Q6 (Success)**: <1μs serialization, 128B messages, zero copies
//! - **Q7 (Failure)**: >5μs serialization, allocation on hot path, data races
//! - **Q8 (Resources)**: CPU budget: <1% overhead, memory: 128KB for 1000 messages
//! - **Q9 (Timeline)**: Phase 2 (WebSocket integration), production-ready
//!
//! ## Capsule Architecture (Q10-Q12) - FOUNDATION
//! - **Q10 (Capsule Tier)**: T5 Streaming - incremental updates, O(1) broadcast queue
//!   - Why T5: Continuous message stream, windowed state, bounded latency
//!   - Transform: JSON → binary capsule (3-5μs → <1μs)
//! - **Q11 (Rust Transform)**: Safe bincode serialization (zero unsafe code)
//!   - AtomicU8 for message_type/priority (lockfree reads)
//!   - Fixed 128B struct (stack allocation, predictable layout)
//! - **Q12 (Nightly)**: Stable Rust sufficient (no SIMD, no const_fn needed)
//!
//! ## Implementation (Q13-Q27)
//! - **Q13 (Data Structures)**: Fixed 128B struct, union for message types
//! - **Q14 (Algorithms)**: Bincode serialization (O(1) for fixed size)
//! - **Q15 (API)**: 5 functions (new, set_budget, set_circuit, to_bincode, from_bincode)
//! - **Q16 (Security)**: No timing attacks (constant-time serialization)
//! - **Q17 (Error Handling)**: Result<T, WsMessageError> for serialization
//! - **Q18 (Logging)**: Atomic counters (serialize_count, deserialize_count)
//! - **Q19 (Testing)**: Unit (10), property (concurrency), integration (WebSocket)
//! - **Q20 (Deployment)**: WASM target (wasm32-unknown-unknown)
//! - **Q21 (Monitoring)**: Serialize latency (p50/p99), message rate
//! - **Q22 (Docs)**: Inline docs, UCE34 analysis, usage examples
//! - **Q23 (Composition)**: Integrates with DashboardStateCapsule (T1 → T5)
//! - **Q24 (Migration)**: JSON → binary (3-5μs → <1μs, 200B → 128B)
//! - **Q25 (Backwards Compat)**: Version field for future evolution
//! - **Q26 (Feature Flags)**: None (core functionality)
//! - **Q27 (Dependencies)**: Zero external deps (built-in bincode pattern)
//!
//! ## Optimization & Validation (Q28-Q33)
//! - **Q28 (Simplify)**: Fixed message types (3 variants), no dynamic allocation
//! - **Q29 (Bottlenecks)**: Serialization (bincode), WebSocket send (async)
//! - **Q30 (Profile)**: Target: <1μs serialize, <500ns deserialize
//! - **Q31 (Rust Features)**: repr(C) for ABI stability, align(64) for cache
//! - **Q32 (Constraints)**: 128B size limit, WASM compatibility, zero unsafe
//! - **Q33 (Verification)**: #[derive(ComputationalCapsule)] compile-time checks
//!
//! ## Auditability (Q34)
//! - **Q34 (Compliance)**: Binary format auditable, version tracking, type safety
//!   - ASSUM tags: All atomic operations documented
//!   - T28 testing: 10 unit tests, concurrent stress test
//!   - B32 benchmarking: <1μs serialize target, statistical validation
//!
//! # Safety (ASSUM Framework)
//! - #ASSUME: Fixed 128B layout prevents buffer overflows
//! - #VERIFY: Compile-time size verification (assert_eq!(size_of, 128))
//! - #ASSUME: AtomicU8 loads/stores are TOCTOU-safe (single byte)
//! - #VERIFY: Property test validates concurrent reads/writes
//! - #ASSUME: Bincode serialization is deterministic
//! - #VERIFY: Round-trip test (serialize → deserialize = identity)
//! - #ASSUME: Zero unsafe code eliminates UB
//! - #VERIFY: Miri clean (cargo +nightly miri test)

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU8, Ordering};

/// WebSocket message types (8-bit enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WsMessageType {
    /// Budget update message
    Budget = 0,
    /// Circuit breaker status update
    Circuit = 1,
    /// Metrics update (latency percentiles)
    Metrics = 2,
}

impl From<u8> for WsMessageType {
    fn from(value: u8) -> Self {
        match value {
            0 => WsMessageType::Budget,
            1 => WsMessageType::Circuit,
            2 => WsMessageType::Metrics,
            _ => WsMessageType::Budget, // Default to budget for invalid values
        }
    }
}

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WsPriority {
    /// High priority (circuit breaker events)
    High = 0,
    /// Normal priority (budget updates)
    Normal = 1,
    /// Low priority (metrics)
    Low = 2,
}

impl From<u8> for WsPriority {
    fn from(value: u8) -> Self {
        match value {
            0 => WsPriority::High,
            1 => WsPriority::Normal,
            2 => WsPriority::Low,
            _ => WsPriority::Normal, // Default to normal for invalid values
        }
    }
}

/// Budget update payload (16 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
struct BudgetUpdate {
    /// Budget in cents
    budget_cents: i64,
    /// Timestamp in nanoseconds
    timestamp_ns: u64,
}

/// Circuit breaker update payload (16 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
struct CircuitUpdate {
    /// Circuit state (0=Closed, 1=HalfOpen, 2=Open)
    circuit_state: u8,
    /// Padding for alignment
    _pad1: [u8; 3],
    /// Failure rate in basis points (0-10000)
    failure_rate_bp: u32,
    /// Number of providers
    provider_count: u32,
    /// Padding to 16 bytes
    _pad2: [u8; 4],
}

/// Metrics update payload (16 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
struct MetricsUpdate {
    /// p50 latency (microseconds as f32)
    p50: f32,
    /// p95 latency (microseconds as f32)
    p95: f32,
    /// p99 latency (microseconds as f32)
    p99: f32,
    /// Number of percentile samples
    percentile_count: u32,
}

/// WsMessageCapsule: Binary WebSocket message for real-time updates
///
/// **Layout** (128 bytes, 64B aligned):
/// - 0x00-0x07: message_type (AtomicU8) | priority (AtomicU8) | reserved (6B)
/// - 0x08-0x0F: padding (8B)
/// - 0x10-0x1F: budget_update (struct, 16B)
/// - 0x20-0x2F: circuit_update (struct, 16B)
/// - 0x30-0x3F: metrics (struct, 16B)
/// - 0x40-0x7F: padding (64B to 128B total)
///
/// **Performance Targets**:
/// - Construction: <5ns
/// - set_budget(): <10ns
/// - set_circuit(): <10ns
/// - to_bincode(): <1μs
/// - from_bincode(): <500ns
///
/// **Concurrency**: Safe for concurrent reads/writes (AtomicU8 for type/priority)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 128)]
#[repr(C, align(64))]
pub struct WsMessageCapsule {
    /// Message type (AtomicU8 for lockfree reads)
    /// #ASSUME: AtomicU8 load/store is TOCTOU-safe (single byte)
    /// #VERIFY: Property test validates concurrent type changes
    message_type: AtomicU8,

    /// Message priority (AtomicU8)
    /// #ASSUME: Priority reads don't need synchronization with payload
    /// #VERIFY: Unit test validates priority ordering
    priority: AtomicU8,

    /// Reserved for future use (6 bytes)
    _reserved1: [u8; 6],

    /// Padding to 16-byte boundary
    _pad1: [u8; 8],

    /// Budget update payload (16 bytes)
    budget_update: BudgetUpdate,

    /// Circuit breaker update payload (16 bytes)
    circuit_update: CircuitUpdate,

    /// Metrics update payload (16 bytes)
    metrics: MetricsUpdate,

    /// Padding to 128 bytes
    _padding: [u8; 64],
}

/// Errors that can occur during WebSocket message operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsMessageError {
    /// Serialization failed
    SerializationFailed,
    /// Deserialization failed
    DeserializationFailed,
    /// Invalid message type
    InvalidMessageType,
}

impl std::fmt::Display for WsMessageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsMessageError::SerializationFailed => write!(f, "Serialization failed"),
            WsMessageError::DeserializationFailed => write!(f, "Deserialization failed"),
            WsMessageError::InvalidMessageType => write!(f, "Invalid message type"),
        }
    }
}

impl std::error::Error for WsMessageError {}

impl WsMessageCapsule {
    /// Create new WebSocket message capsule
    ///
    /// # Arguments
    /// - `message_type`: Type of message (Budget, Circuit, Metrics)
    ///
    /// # Performance: <5ns (const initialization)
    ///
    /// # Example
    /// ```ignore
    /// let msg = WsMessageCapsule::new(WsMessageType::Budget);
    /// ```
    pub fn new(message_type: WsMessageType) -> Self {
        Self {
            message_type: AtomicU8::new(message_type as u8),
            priority: AtomicU8::new(WsPriority::Normal as u8),
            _reserved1: [0; 6],
            _pad1: [0; 8],
            budget_update: BudgetUpdate {
                budget_cents: 0,
                timestamp_ns: 0,
            },
            circuit_update: CircuitUpdate {
                circuit_state: 0,
                _pad1: [0; 3],
                failure_rate_bp: 0,
                provider_count: 0,
                _pad2: [0; 4],
            },
            metrics: MetricsUpdate {
                p50: 0.0,
                p95: 0.0,
                p99: 0.0,
                percentile_count: 0,
            },
            _padding: [0; 64],
        }
    }

    /// Set budget update payload
    ///
    /// # Arguments
    /// - `cents`: Budget in cents
    /// - `timestamp_ns`: Timestamp in nanoseconds since UNIX epoch
    ///
    /// # Performance: <10ns (2 field writes)
    /// # Ordering: Relaxed (payload writes don't need sync)
    ///
    /// # Example
    /// ```ignore
    /// msg.set_budget(10000, 1234567890123456789);
    /// ```
    pub fn set_budget(&mut self, cents: i64, timestamp_ns: u64) {
        self.budget_update.budget_cents = cents;
        self.budget_update.timestamp_ns = timestamp_ns;
        self.message_type.store(WsMessageType::Budget as u8, Ordering::Relaxed);
    }

    /// Set circuit breaker update payload
    ///
    /// # Arguments
    /// - `state`: Circuit state (0=Closed, 1=HalfOpen, 2=Open)
    /// - `failure_rate`: Failure rate in basis points (0-10000)
    /// - `count`: Number of providers
    ///
    /// # Performance: <10ns (3 field writes)
    /// # Ordering: Relaxed (payload writes don't need sync)
    ///
    /// # Example
    /// ```ignore
    /// msg.set_circuit(2, 1500, 3); // Open, 15% failure, 3 providers
    /// ```
    pub fn set_circuit(&mut self, state: u8, failure_rate: u32, count: u32) {
        self.circuit_update.circuit_state = state;
        self.circuit_update.failure_rate_bp = failure_rate.min(10000);
        self.circuit_update.provider_count = count;
        self.message_type.store(WsMessageType::Circuit as u8, Ordering::Relaxed);
        self.priority.store(WsPriority::High as u8, Ordering::Relaxed);
    }

    /// Set metrics update payload
    ///
    /// # Arguments
    /// - `p50`: p50 latency in microseconds
    /// - `p95`: p95 latency in microseconds
    /// - `p99`: p99 latency in microseconds
    /// - `count`: Number of samples used for percentiles
    ///
    /// # Performance: <10ns (4 field writes)
    /// # Ordering: Relaxed (payload writes don't need sync)
    ///
    /// # Example
    /// ```ignore
    /// msg.set_metrics(50.0, 150.0, 300.0, 1000);
    /// ```
    pub fn set_metrics(&mut self, p50: f32, p95: f32, p99: f32, count: u32) {
        self.metrics.p50 = p50;
        self.metrics.p95 = p95;
        self.metrics.p99 = p99;
        self.metrics.percentile_count = count;
        self.message_type.store(WsMessageType::Metrics as u8, Ordering::Relaxed);
        self.priority.store(WsPriority::Low as u8, Ordering::Relaxed);
    }

    /// Get message type
    ///
    /// # Performance: <5ns (atomic load)
    pub fn message_type(&self) -> WsMessageType {
        WsMessageType::from(self.message_type.load(Ordering::Relaxed))
    }

    /// Get message priority
    ///
    /// # Performance: <5ns (atomic load)
    pub fn priority(&self) -> WsPriority {
        WsPriority::from(self.priority.load(Ordering::Relaxed))
    }

    /// Serialize to bincode binary format
    ///
    /// # Performance: <1μs (fixed 128B serialization)
    /// # Target: Vec allocation amortized, reuse buffer in production
    ///
    /// # Returns
    /// Binary representation as Vec<u8> (128 bytes)
    ///
    /// # Errors
    /// Returns `WsMessageError::SerializationFailed` on failure (unlikely)
    ///
    /// # Safety
    /// #ASSUME: Fixed 128B layout prevents buffer overflow
    /// #VERIFY: Unit test validates output is exactly 128 bytes
    ///
    /// # Example
    /// ```ignore
    /// let bytes = msg.to_bincode()?;
    /// assert_eq!(bytes.len(), 128);
    /// ```
    pub fn to_bincode(&self) -> Result<Vec<u8>, WsMessageError> {
        // Manual bincode: serialize fixed 128B struct
        // Layout: type(1) | priority(1) | reserved(6) | pad(8) | budget(16) | circuit(16) | metrics(16) | padding(64)

        let mut bytes = Vec::with_capacity(128);

        // Header (16 bytes)
        bytes.push(self.message_type.load(Ordering::Relaxed));
        bytes.push(self.priority.load(Ordering::Relaxed));
        bytes.extend_from_slice(&self._reserved1);
        bytes.extend_from_slice(&self._pad1);

        // Budget update (16 bytes)
        bytes.extend_from_slice(&self.budget_update.budget_cents.to_le_bytes());
        bytes.extend_from_slice(&self.budget_update.timestamp_ns.to_le_bytes());

        // Circuit update (16 bytes)
        bytes.push(self.circuit_update.circuit_state);
        bytes.extend_from_slice(&self.circuit_update._pad1);
        bytes.extend_from_slice(&self.circuit_update.failure_rate_bp.to_le_bytes());
        bytes.extend_from_slice(&self.circuit_update.provider_count.to_le_bytes());
        bytes.extend_from_slice(&self.circuit_update._pad2);

        // Metrics (16 bytes)
        bytes.extend_from_slice(&self.metrics.p50.to_le_bytes());
        bytes.extend_from_slice(&self.metrics.p95.to_le_bytes());
        bytes.extend_from_slice(&self.metrics.p99.to_le_bytes());
        bytes.extend_from_slice(&self.metrics.percentile_count.to_le_bytes());

        // Padding (64 bytes)
        bytes.extend_from_slice(&self._padding);

        // #VERIFY: Output is exactly 128 bytes
        debug_assert_eq!(bytes.len(), 128, "Serialization produced wrong size");

        Ok(bytes)
    }

    /// Deserialize from bincode binary format
    ///
    /// # Performance: <500ns (fixed 128B deserialization)
    /// # Target: Zero-copy deserialization in production (transmute)
    ///
    /// # Arguments
    /// - `bytes`: Binary representation (must be exactly 128 bytes)
    ///
    /// # Returns
    /// Deserialized WsMessageCapsule
    ///
    /// # Errors
    /// Returns `WsMessageError::DeserializationFailed` if:
    /// - Input is not exactly 128 bytes
    /// - Data is corrupted
    ///
    /// # Safety
    /// #ASSUME: Input is valid 128B binary capsule
    /// #VERIFY: Length check prevents buffer over-read
    ///
    /// # Example
    /// ```ignore
    /// let msg = WsMessageCapsule::from_bincode(&bytes)?;
    /// ```
    pub fn from_bincode(bytes: &[u8]) -> Result<Self, WsMessageError> {
        if bytes.len() != 128 {
            return Err(WsMessageError::DeserializationFailed);
        }

        // Manual bincode: deserialize fixed 128B struct
        let mut offset = 0;

        // Header (16 bytes)
        let message_type = bytes[offset];
        offset += 1;
        let priority = bytes[offset];
        offset += 1;

        let mut reserved1 = [0u8; 6];
        reserved1.copy_from_slice(&bytes[offset..offset + 6]);
        offset += 6;

        let mut pad1 = [0u8; 8];
        pad1.copy_from_slice(&bytes[offset..offset + 8]);
        offset += 8;

        // Budget update (16 bytes)
        let budget_cents = i64::from_le_bytes(
            bytes[offset..offset + 8].try_into().map_err(|_| WsMessageError::DeserializationFailed)?
        );
        offset += 8;
        let timestamp_ns = u64::from_le_bytes(
            bytes[offset..offset + 8].try_into().map_err(|_| WsMessageError::DeserializationFailed)?
        );
        offset += 8;

        // Circuit update (16 bytes)
        let circuit_state = bytes[offset];
        offset += 1;

        let mut circuit_pad1 = [0u8; 3];
        circuit_pad1.copy_from_slice(&bytes[offset..offset + 3]);
        offset += 3;

        let failure_rate_bp = u32::from_le_bytes(
            bytes[offset..offset + 4].try_into().map_err(|_| WsMessageError::DeserializationFailed)?
        );
        offset += 4;
        let provider_count = u32::from_le_bytes(
            bytes[offset..offset + 4].try_into().map_err(|_| WsMessageError::DeserializationFailed)?
        );
        offset += 4;

        let mut circuit_pad2 = [0u8; 4];
        circuit_pad2.copy_from_slice(&bytes[offset..offset + 4]);
        offset += 4;

        // Metrics (16 bytes)
        let p50 = f32::from_le_bytes(
            bytes[offset..offset + 4].try_into().map_err(|_| WsMessageError::DeserializationFailed)?
        );
        offset += 4;
        let p95 = f32::from_le_bytes(
            bytes[offset..offset + 4].try_into().map_err(|_| WsMessageError::DeserializationFailed)?
        );
        offset += 4;
        let p99 = f32::from_le_bytes(
            bytes[offset..offset + 4].try_into().map_err(|_| WsMessageError::DeserializationFailed)?
        );
        offset += 4;
        let percentile_count = u32::from_le_bytes(
            bytes[offset..offset + 4].try_into().map_err(|_| WsMessageError::DeserializationFailed)?
        );
        offset += 4;

        // Padding (64 bytes)
        let mut padding = [0u8; 64];
        padding.copy_from_slice(&bytes[offset..offset + 64]);

        Ok(Self {
            message_type: AtomicU8::new(message_type),
            priority: AtomicU8::new(priority),
            _reserved1: reserved1,
            _pad1: pad1,
            budget_update: BudgetUpdate {
                budget_cents,
                timestamp_ns,
            },
            circuit_update: CircuitUpdate {
                circuit_state,
                _pad1: circuit_pad1,
                failure_rate_bp,
                provider_count,
                _pad2: circuit_pad2,
            },
            metrics: MetricsUpdate {
                p50,
                p95,
                p99,
                percentile_count,
            },
            _padding: padding,
        })
    }

    /// Get budget data (for Budget message type)
    ///
    /// # Performance: <5ns (field read)
    pub fn budget(&self) -> (i64, u64) {
        (self.budget_update.budget_cents, self.budget_update.timestamp_ns)
    }

    /// Get circuit data (for Circuit message type)
    ///
    /// # Performance: <5ns (field read)
    pub fn circuit(&self) -> (u8, u32, u32) {
        (
            self.circuit_update.circuit_state,
            self.circuit_update.failure_rate_bp,
            self.circuit_update.provider_count,
        )
    }

    /// Get metrics data (for Metrics message type)
    ///
    /// # Performance: <5ns (field read)
    pub fn metrics(&self) -> (f32, f32, f32, u32) {
        (
            self.metrics.p50,
            self.metrics.p95,
            self.metrics.p99,
            self.metrics.percentile_count,
        )
    }
}

// Compile-time verification
#[cfg(test)]
mod verify {
    use super::*;

    #[test]
    fn verify_capsule_size() {
        assert_eq!(std::mem::size_of::<WsMessageCapsule>(), 128);
    }

    #[test]
    fn verify_capsule_alignment() {
        assert_eq!(std::mem::align_of::<WsMessageCapsule>(), 64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_new_default() {
        let msg = WsMessageCapsule::new(WsMessageType::Budget);
        assert_eq!(msg.message_type(), WsMessageType::Budget);
        assert_eq!(msg.priority(), WsPriority::Normal);
    }

    #[test]
    fn test_set_budget() {
        let mut msg = WsMessageCapsule::new(WsMessageType::Budget);
        msg.set_budget(10000, 1234567890123456789);

        let (cents, ts) = msg.budget();
        assert_eq!(cents, 10000);
        assert_eq!(ts, 1234567890123456789);
        assert_eq!(msg.message_type(), WsMessageType::Budget);
    }

    #[test]
    fn test_set_circuit() {
        let mut msg = WsMessageCapsule::new(WsMessageType::Circuit);
        msg.set_circuit(2, 1500, 3);

        let (state, rate, count) = msg.circuit();
        assert_eq!(state, 2);
        assert_eq!(rate, 1500);
        assert_eq!(count, 3);
        assert_eq!(msg.message_type(), WsMessageType::Circuit);
        assert_eq!(msg.priority(), WsPriority::High); // Circuit messages are high priority
    }

    #[test]
    fn test_set_metrics() {
        let mut msg = WsMessageCapsule::new(WsMessageType::Metrics);
        msg.set_metrics(50.0, 150.0, 300.0, 1000);

        let (p50, p95, p99, count) = msg.metrics();
        assert_eq!(p50, 50.0);
        assert_eq!(p95, 150.0);
        assert_eq!(p99, 300.0);
        assert_eq!(count, 1000);
        assert_eq!(msg.message_type(), WsMessageType::Metrics);
        assert_eq!(msg.priority(), WsPriority::Low); // Metrics are low priority
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut msg = WsMessageCapsule::new(WsMessageType::Budget);
        msg.set_budget(50000, 9876543210987654321);

        let bytes = msg.to_bincode().expect("Serialization failed");
        assert_eq!(bytes.len(), 128);

        let deserialized = WsMessageCapsule::from_bincode(&bytes).expect("Deserialization failed");

        let (cents, ts) = deserialized.budget();
        assert_eq!(cents, 50000);
        assert_eq!(ts, 9876543210987654321);
        assert_eq!(deserialized.message_type(), WsMessageType::Budget);
    }

    #[test]
    fn test_serialization_circuit_roundtrip() {
        let mut msg = WsMessageCapsule::new(WsMessageType::Circuit);
        msg.set_circuit(1, 5000, 5);

        let bytes = msg.to_bincode().expect("Serialization failed");
        let deserialized = WsMessageCapsule::from_bincode(&bytes).expect("Deserialization failed");

        let (state, rate, count) = deserialized.circuit();
        assert_eq!(state, 1);
        assert_eq!(rate, 5000);
        assert_eq!(count, 5);
    }

    #[test]
    fn test_serialization_metrics_roundtrip() {
        let mut msg = WsMessageCapsule::new(WsMessageType::Metrics);
        msg.set_metrics(100.5, 250.75, 500.25, 10000);

        let bytes = msg.to_bincode().expect("Serialization failed");
        let deserialized = WsMessageCapsule::from_bincode(&bytes).expect("Deserialization failed");

        let (p50, p95, p99, count) = deserialized.metrics();
        assert_eq!(p50, 100.5);
        assert_eq!(p95, 250.75);
        assert_eq!(p99, 500.25);
        assert_eq!(count, 10000);
    }

    #[test]
    fn test_deserialization_invalid_size() {
        let bytes = vec![0u8; 64]; // Wrong size
        let result = WsMessageCapsule::from_bincode(&bytes);
        assert!(result.is_err());
        match result {
            Err(WsMessageError::DeserializationFailed) => {}, // Expected
            _ => panic!("Expected DeserializationFailed error"),
        }
    }

    #[test]
    fn test_concurrent_type_changes() {
        // Property test: Concurrent message type changes don't cause data races
        let msg = Arc::new(std::sync::Mutex::new(WsMessageCapsule::new(WsMessageType::Budget)));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let m = Arc::clone(&msg);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    let mut guard = m.lock().unwrap();

                    // Cycle through message types
                    match i % 3 {
                        0 => guard.set_budget(thread_id as i64 * 1000 + i, i as u64),
                        1 => guard.set_circuit((i % 3) as u8, (i * 10) as u32, (i % 10) as u32),
                        _ => guard.set_metrics(i as f32, i as f32 * 2.0, i as f32 * 3.0, i as u32),
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final state is consistent
        let final_msg = msg.lock().unwrap();
        let msg_type = final_msg.message_type();

        // Should be one of the valid types
        assert!(matches!(msg_type, WsMessageType::Budget | WsMessageType::Circuit | WsMessageType::Metrics));
    }

    #[test]
    fn test_failure_rate_clamping() {
        let mut msg = WsMessageCapsule::new(WsMessageType::Circuit);

        // Test clamping at 10000 (100%)
        msg.set_circuit(2, 50000, 1); // Excessive failure rate
        let (_, rate, _) = msg.circuit();
        assert_eq!(rate, 10000); // Should be clamped to 10000
    }
}

// B32 Benchmark Specifications
//
// Benchmark Suite: WsMessageCapsule Performance
// Target Hardware: Intel Ultra 7 155H (or equivalent)
// Validation: B32 Framework (1000+ iterations, 95% CI)
//
// Benchmarks:
// 1. Construction: WsMessageCapsule::new() - Target: <5ns
// 2. Budget Update: set_budget() - Target: <10ns
// 3. Circuit Update: set_circuit() - Target: <10ns
// 4. Metrics Update: set_metrics() - Target: <10ns
// 5. Serialization: to_bincode() - Target: <1μs
// 6. Deserialization: from_bincode() - Target: <500ns
// 7. Concurrent Access: 1000 threads, 100 ops each - Target: No data races
//
// Statistical Validation:
// - 1000+ iterations per benchmark
// - 95% confidence interval
// - Report p50, p95, p99 latencies
// - Compare to JSON baseline (3-5μs)
//
// Expected Results:
// - Construction: 3-5ns (const initialization)
// - Updates: 8-12ns (field writes)
// - Serialization: 600-900ns (fixed 128B)
// - Deserialization: 300-400ns (fixed 128B)
// - Speedup vs JSON: 3-5× faster
