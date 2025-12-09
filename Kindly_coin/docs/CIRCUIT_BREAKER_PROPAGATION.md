# Circuit Breaker Propagation - Kindly Coin Security Architecture
**Multi-Layer Protection Cascade Design**

**Version**: 1.0
**Date**: 2025-10-07
**Framework**: ASSUM Safety + I20 Integration
**Reference**: `/home/samuel/Primitives/docs/ATOMIC_CAPSULE_FAILURE_MODES.md`

---

## Executive Summary

Circuit breaker propagation is the **most critical integration pattern** in Kindly Coin. When fraud or system anomalies are detected at any layer, protection must cascade instantly across all components to prevent loss of funds or network compromise.

### Design Goals

1. **<1ms propagation**: Protection escalation reaches all layers in sub-millisecond
2. **100% lockfree**: No mutex contention during cascade
3. **Graceful degradation**: Reduce functionality before halting
4. **Instant recovery**: Manual override available for false positives
5. **Auditability**: Complete propagation trace for forensics

### Protection Levels

| Level | Description | Action | Trigger | Recovery |
|-------|-------------|--------|---------|----------|
| **L0 (Normal)** | Healthy operation | Full functionality | Default | N/A |
| **L1 (Caution)** | Minor anomaly detected | Size reduction (1/φ) | >2% unusual activity | Auto after 60s |
| **L2 (Warning)** | Significant risk | Size reduction (1/φ²) | >5% unusual activity | Manual after investigation |
| **L3 (Emergency)** | Critical threat | Halt all transactions | Fraud confirmed, >10% unusual | Manual with multi-sig |

---

## Architecture: Multi-Layer Circuit Breaker Stack

```
┌─────────────────────────────────────────────────────────────┐
│                    Circuit Breaker Stack                    │
├─────────────────────────────────────────────────────────────┤
│  L5: Governance Layer                                       │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  GovernanceCircuitBreaker (GCB)                      │  │
│  │  - KYC fraud detection                               │  │
│  │  - Tax evasion detection                             │  │
│  │  - Compliance violation detection                    │  │
│  └──────────────────────────────────────────────────────┘  │
│                           ↓ Propagate                       │
├─────────────────────────────────────────────────────────────┤
│  L4: UBI Layer                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  UbiCircuitBreaker (UCB)                             │  │
│  │  - Sybil attack detection                            │  │
│  │  - Claim fraud detection                             │  │
│  │  - Distribution anomaly detection                    │  │
│  └──────────────────────────────────────────────────────┘  │
│                           ↓ Propagate                       │
├─────────────────────────────────────────────────────────────┤
│  L3: Network Layer                                          │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  NetworkCircuitBreaker (NCB)                         │  │
│  │  - DoS attack detection                              │  │
│  │  - Spam transaction detection                        │  │
│  │  - Gossip anomaly detection                          │  │
│  └──────────────────────────────────────────────────────┘  │
│                           ↓ Propagate                       │
├─────────────────────────────────────────────────────────────┤
│  L2: Consensus Layer                                        │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  ConsensusCircuitBreaker (CCB)                       │  │
│  │  - Double-spend detection                            │  │
│  │  - Fork detection                                    │  │
│  │  - Validator misbehavior detection                   │  │
│  └──────────────────────────────────────────────────────┘  │
│                           ↓ Propagate                       │
├─────────────────────────────────────────────────────────────┤
│  L1: Core Layer                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  CoreCircuitBreaker (CCB)                            │  │
│  │  - Invalid signature detection                       │  │
│  │  - Balance overflow detection                        │  │
│  │  - Nonce anomaly detection                           │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Pattern 1: Upward Propagation (Bottom-Up)

### Trigger: Core layer detects anomaly

**Scenario**: Core layer detects 100 transactions with invalid signatures from same sender in 1 second.

```rust
// Core Layer Detection
pub struct CoreCircuitBreaker {
    protection_level: AtomicU64,  // Packed: level:2 | cause:6 | timestamp:56
    invalid_sig_count: AtomicU64,
    last_escalation_ns: AtomicU64,
}

impl CoreCircuitBreaker {
    pub fn check_invalid_signature_pattern(
        &self,
        sender: &[u8; 20],
        timestamp_ns: u64,
    ) -> PropagationDecision {
        let count = self.invalid_sig_count.fetch_add(1, Ordering::Relaxed);

        // Escalation threshold: 100 invalid sigs in 1 second
        if count > 100 {
            let last_escalation = self.last_escalation_ns.load(Ordering::Relaxed);
            let time_since_last = timestamp_ns.saturating_sub(last_escalation);

            if time_since_last < 1_000_000_000 {
                // <1s since last escalation - escalate to L1
                self.escalate_to_level1(BreakerCause::InvalidSignatureFlood);

                return PropagationDecision::EscalateUpward {
                    target_layers: vec![Layer::Network, Layer::Consensus],
                    reason: "Invalid signature flood detected",
                    sender: *sender,
                };
            }
        }

        PropagationDecision::NoEscalation
    }

    fn escalate_to_level1(&self, cause: BreakerCause) {
        let timestamp_ns = current_timestamp_ns();
        let new_state = Self::pack_state(
            ProtectionLevel::Level1,
            cause,
            timestamp_ns,
        );

        self.protection_level.store(new_state, Ordering::Release);
        self.last_escalation_ns.store(timestamp_ns, Ordering::Release);
    }
}
```

### Propagation Coordinator

```rust
pub struct CircuitBreakerPropagator {
    core_breaker: Arc<CoreCircuitBreaker>,
    consensus_breaker: Arc<ConsensusCircuitBreaker>,
    network_breaker: Arc<NetworkCircuitBreaker>,
    ubi_breaker: Arc<UbiCircuitBreaker>,
    governance_breaker: Arc<GovernanceCircuitBreaker>,
}

impl CircuitBreakerPropagator {
    /// Propagate protection escalation upward through layers
    ///
    /// # Performance
    ///
    /// <100ns per layer (atomic store only)
    ///
    /// # Safety (ASSUM)
    ///
    /// - `#ASSUME_UPWARD_PROPAGATION`: Higher layers receive protection signals instantly
    /// - `#VERIFY_PROPAGATION_LATENCY`: Measure end-to-end propagation <1ms
    pub fn propagate_upward(
        &self,
        source_layer: Layer,
        decision: PropagationDecision,
    ) -> Result<(), PropagationError> {
        match decision {
            PropagationDecision::EscalateUpward { target_layers, reason, sender } => {
                let propagation_start = Instant::now();

                for target in target_layers {
                    match target {
                        Layer::Network => {
                            self.network_breaker.receive_escalation(
                                source_layer,
                                ProtectionLevel::Level1,
                                reason,
                            )?;
                        }
                        Layer::Consensus => {
                            self.consensus_breaker.receive_escalation(
                                source_layer,
                                ProtectionLevel::Level1,
                                reason,
                            )?;
                        }
                        Layer::Ubi => {
                            self.ubi_breaker.receive_escalation(
                                source_layer,
                                ProtectionLevel::Level1,
                                reason,
                            )?;
                        }
                        Layer::Governance => {
                            self.governance_breaker.receive_escalation(
                                source_layer,
                                ProtectionLevel::Level1,
                                reason,
                            )?;
                        }
                        _ => {}
                    }
                }

                let propagation_latency = propagation_start.elapsed();

                // #ASSUME_PROPAGATION_LATENCY: <1ms for full stack propagation
                // #VERIFY_LATENCY_BUDGET: Alert if >1ms
                if propagation_latency.as_micros() > 1000 {
                    eprintln!(
                        "WARNING: Circuit breaker propagation exceeded budget: {}μs",
                        propagation_latency.as_micros()
                    );
                }

                Ok(())
            }
            PropagationDecision::NoEscalation => Ok(()),
        }
    }
}
```

---

## Pattern 2: Downward Propagation (Top-Down)

### Trigger: Governance layer detects fraud

**Scenario**: Governance layer detects KYC fraud - identity used in 1000+ accounts (Sybil attack).

```rust
pub struct GovernanceCircuitBreaker {
    protection_level: AtomicU64,
    kyc_fraud_count: AtomicU64,
}

impl GovernanceCircuitBreaker {
    pub fn detect_sybil_attack(
        &self,
        identity_hash: &[u8; 32],
        account_count: usize,
    ) -> PropagationDecision {
        // Sybil threshold: Same identity used in >1000 accounts
        if account_count > 1000 {
            // Immediate L3 escalation (critical fraud)
            self.escalate_to_level3(BreakerCause::SybilAttack);

            return PropagationDecision::EscalateDownward {
                target_layers: vec![
                    Layer::Ubi,        // Stop UBI claims
                    Layer::Network,    // Block spam
                    Layer::Consensus,  // Prevent double-spend
                    Layer::Core,       // Halt transactions
                ],
                reason: "Sybil attack detected - 1000+ accounts with same identity",
                identity: *identity_hash,
                severity: Severity::Critical,
            };
        }

        PropagationDecision::NoEscalation
    }

    fn escalate_to_level3(&self, cause: BreakerCause) {
        let timestamp_ns = current_timestamp_ns();
        let new_state = Self::pack_state(
            ProtectionLevel::Level3,
            cause,
            timestamp_ns,
        );

        // Critical: Use SeqCst for immediate global visibility
        self.protection_level.store(new_state, Ordering::SeqCst);

        // Audit log (asynchronous, non-blocking)
        let _ = audit_log::record_critical_event(
            "circuit_breaker_l3_activation",
            cause,
            timestamp_ns,
        );
    }
}
```

### Downward Cascade

```rust
impl CircuitBreakerPropagator {
    /// Propagate critical protection downward through all layers
    ///
    /// # Performance
    ///
    /// <500ns for complete cascade (5 layers × 100ns)
    ///
    /// # Safety (ASSUM)
    ///
    /// - `#ASSUME_DOWNWARD_PROPAGATION`: All layers halt instantly on L3
    /// - `#VERIFY_HALT_EFFECTIVENESS`: Validate no transactions processed after L3
    pub fn propagate_downward(
        &self,
        source_layer: Layer,
        decision: PropagationDecision,
    ) -> Result<(), PropagationError> {
        match decision {
            PropagationDecision::EscalateDownward { target_layers, reason, identity, severity } => {
                let propagation_start = Instant::now();

                // L3 (Critical): Cascade to ALL layers immediately
                if severity == Severity::Critical {
                    // Atomic broadcast: Single memory barrier for all layers
                    let critical_state = Self::pack_critical_state(
                        ProtectionLevel::Level3,
                        BreakerCause::from_reason(reason),
                        current_timestamp_ns(),
                    );

                    // Use SeqCst for immediate global visibility
                    self.core_breaker.set_state_critical(critical_state);
                    self.consensus_breaker.set_state_critical(critical_state);
                    self.network_breaker.set_state_critical(critical_state);
                    self.ubi_breaker.set_state_critical(critical_state);

                    // #ASSUME_CRITICAL_HALT: All layers check protection level before every operation
                    // #VERIFY_HALT_TIMING: Validate <1ms from detection to complete halt
                } else {
                    // L1/L2: Targeted cascade to specific layers
                    for target in target_layers {
                        match target {
                            Layer::Core => {
                                self.core_breaker.receive_escalation(
                                    source_layer,
                                    ProtectionLevel::Level2,
                                    reason,
                                )?;
                            }
                            Layer::Consensus => {
                                self.consensus_breaker.receive_escalation(
                                    source_layer,
                                    ProtectionLevel::Level2,
                                    reason,
                                )?;
                            }
                            Layer::Network => {
                                self.network_breaker.receive_escalation(
                                    source_layer,
                                    ProtectionLevel::Level2,
                                    reason,
                                )?;
                            }
                            Layer::Ubi => {
                                self.ubi_breaker.receive_escalation(
                                    source_layer,
                                    ProtectionLevel::Level2,
                                    reason,
                                )?;
                            }
                            _ => {}
                        }
                    }
                }

                let propagation_latency = propagation_start.elapsed();

                // Critical path: Must complete in <1ms
                if propagation_latency.as_micros() > 1000 {
                    eprintln!(
                        "CRITICAL: Circuit breaker cascade exceeded 1ms: {}μs",
                        propagation_latency.as_micros()
                    );
                }

                Ok(())
            }
            PropagationDecision::NoEscalation => Ok(()),
        }
    }
}
```

---

## Pattern 3: Lateral Propagation (Peer-to-Peer)

### Trigger: Consensus layer detects fork

**Scenario**: Validator detects fork - two valid blocks at same height.

```rust
pub struct ConsensusCircuitBreaker {
    protection_level: AtomicU64,
    fork_count: AtomicU64,
}

impl ConsensusCircuitBreaker {
    pub fn detect_fork(
        &self,
        block_height: u64,
        block_hash_a: &[u8; 32],
        block_hash_b: &[u8; 32],
    ) -> PropagationDecision {
        let fork_count = self.fork_count.fetch_add(1, Ordering::Relaxed);

        // Fork escalation: Immediate L2 on first fork, L3 on second fork
        let protection_level = if fork_count == 0 {
            ProtectionLevel::Level2
        } else {
            ProtectionLevel::Level3
        };

        self.escalate(protection_level, BreakerCause::ForkDetected);

        PropagationDecision::PropagateLateral {
            peer_layers: vec![
                Layer::Network,  // Notify network of fork
                Layer::Ubi,      // Pause UBI distribution during fork
            ],
            reason: format!(
                "Fork detected at height {}: {} vs {}",
                block_height,
                hex::encode(block_hash_a),
                hex::encode(block_hash_b)
            ),
            coordination_required: true,
        }
    }
}
```

### Lateral Coordination

```rust
impl CircuitBreakerPropagator {
    /// Propagate laterally to peer layers for coordination
    ///
    /// # Use Case
    ///
    /// Fork detection requires consensus + network coordination
    ///
    /// # Safety (ASSUM)
    ///
    /// - `#ASSUME_LATERAL_COORDINATION`: Peer layers coordinate resolution
    /// - `#VERIFY_COORDINATION_TIMEOUT`: Escalate to L3 if not resolved in 30s
    pub fn propagate_lateral(
        &self,
        source_layer: Layer,
        decision: PropagationDecision,
    ) -> Result<(), PropagationError> {
        match decision {
            PropagationDecision::PropagateLateral { peer_layers, reason, coordination_required } => {
                // Notify peer layers
                for peer in peer_layers {
                    match peer {
                        Layer::Network => {
                            self.network_breaker.receive_lateral_signal(
                                source_layer,
                                reason.clone(),
                                coordination_required,
                            )?;
                        }
                        Layer::Ubi => {
                            self.ubi_breaker.receive_lateral_signal(
                                source_layer,
                                reason.clone(),
                                coordination_required,
                            )?;
                        }
                        _ => {}
                    }
                }

                // If coordination required, start timeout monitor
                if coordination_required {
                    self.start_coordination_timeout(source_layer, 30_000_000_000)?; // 30s
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn start_coordination_timeout(
        &self,
        source_layer: Layer,
        timeout_ns: u64,
    ) -> Result<(), PropagationError> {
        // Spawn async timeout monitor
        let propagator = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_nanos(timeout_ns)).await;

            // Check if coordination resolved
            if !propagator.is_coordination_resolved(source_layer) {
                // Escalate to L3 if coordination timeout
                eprintln!(
                    "CRITICAL: Lateral coordination timeout for {:?} - escalating to L3",
                    source_layer
                );

                let _ = propagator.propagate_downward(
                    source_layer,
                    PropagationDecision::EscalateDownward {
                        target_layers: vec![Layer::Core, Layer::Consensus, Layer::Network, Layer::Ubi],
                        reason: "Coordination timeout - escalating to emergency halt",
                        identity: [0u8; 32],
                        severity: Severity::Critical,
                    },
                );
            }
        });

        Ok(())
    }
}
```

---

## Fast Path: Protection Level Check

Every operation in every layer checks protection level before proceeding:

```rust
/// Fast protection level check (<10ns)
///
/// # Performance
///
/// Single atomic load with Relaxed ordering
///
/// # Returns
///
/// - `true` if operation allowed
/// - `false` if protection active
#[inline(always)]
pub fn allows_operation(breaker: &CircuitBreakerCapsule) -> bool {
    let state = breaker.protection_level.load(Ordering::Relaxed);
    let level = (state >> 62) & 0x3;

    match level {
        0 => true,  // L0: Normal
        1 => {
            // L1: Reduce size by 1/φ, allow operation
            // (Size reduction handled by caller)
            true
        }
        2 => {
            // L2: Reduce size by 1/φ², allow operation
            // (Size reduction handled by caller)
            true
        }
        3 => false, // L3: Halt all operations
        _ => unreachable!(),
    }
}

/// Example usage in transaction validation
pub fn validate_transaction(
    tx: &AtomicTransactionCapsule,
    core_breaker: &CoreCircuitBreaker,
) -> Result<(), TransactionError> {
    // Check circuit breaker (<10ns)
    if !allows_operation(&core_breaker) {
        return Err(TransactionError::CircuitBreakerActive {
            level: core_breaker.get_level(),
        });
    }

    // Proceed with validation
    tx.read()?;
    // ...
    Ok(())
}
```

---

## Recovery: Manual Override with Multi-Sig

```rust
pub struct CircuitBreakerRecovery {
    propagator: Arc<CircuitBreakerPropagator>,
    admin_signatures: Arc<AtomicU64>,  // Packed: sig_count:8 | sig_bitmap:56
}

impl CircuitBreakerRecovery {
    /// Manual recovery from L3 protection (requires multi-sig)
    ///
    /// # Security (ASSUM)
    ///
    /// - `#ASSUME_MULTISIG_SECURE`: 3-of-5 admin signatures required
    /// - `#VERIFY_SIGNATURE_VALIDITY`: Ed25519 signature verification
    pub fn recover_from_l3(
        &self,
        admin_signatures: Vec<([u8; 64], [u8; 32])>, // (signature, pubkey)
    ) -> Result<(), RecoveryError> {
        // Verify at least 3 valid signatures
        let mut valid_sig_count = 0;
        for (signature, pubkey) in admin_signatures {
            if self.verify_admin_signature(&signature, &pubkey) {
                valid_sig_count += 1;
            }
        }

        if valid_sig_count < 3 {
            return Err(RecoveryError::InsufficientSignatures {
                required: 3,
                provided: valid_sig_count,
            });
        }

        // Gradual recovery: L3 → L2 → L1 → L0
        self.propagator.gradual_recovery_cascade()?;

        // Audit log
        audit_log::record_critical_event(
            "circuit_breaker_manual_recovery",
            BreakerCause::ManualOverride,
            current_timestamp_ns(),
        )?;

        Ok(())
    }

    fn verify_admin_signature(
        &self,
        signature: &[u8; 64],
        pubkey: &[u8; 32],
    ) -> bool {
        // Ed25519 signature verification
        use ed25519_dalek::{PublicKey, Signature, Verifier};

        let public_key = PublicKey::from_bytes(pubkey).ok();
        let sig = Signature::from_bytes(signature).ok();

        if let (Some(pk), Some(s)) = (public_key, sig) {
            let message = b"KINDLY_COIN_CIRCUIT_BREAKER_RECOVERY";
            pk.verify(message, &s).is_ok()
        } else {
            false
        }
    }
}

impl CircuitBreakerPropagator {
    /// Gradual recovery cascade: L3 → L2 → L1 → L0
    ///
    /// # Safety
    ///
    /// 30-second intervals between levels to validate stability
    fn gradual_recovery_cascade(&self) -> Result<(), RecoveryError> {
        // Step 1: L3 → L2 (allow reduced operations)
        self.set_all_layers(ProtectionLevel::Level2)?;
        std::thread::sleep(Duration::from_secs(30));

        // Verify no anomalies during L2
        if self.detect_anomalies_during_recovery() {
            return Err(RecoveryError::AnomalyDuringRecovery);
        }

        // Step 2: L2 → L1 (less restrictive)
        self.set_all_layers(ProtectionLevel::Level1)?;
        std::thread::sleep(Duration::from_secs(30));

        // Verify no anomalies during L1
        if self.detect_anomalies_during_recovery() {
            return Err(RecoveryError::AnomalyDuringRecovery);
        }

        // Step 3: L1 → L0 (full recovery)
        self.set_all_layers(ProtectionLevel::Normal)?;

        Ok(())
    }

    fn set_all_layers(&self, level: ProtectionLevel) -> Result<(), RecoveryError> {
        self.core_breaker.set_level(level)?;
        self.consensus_breaker.set_level(level)?;
        self.network_breaker.set_level(level)?;
        self.ubi_breaker.set_level(level)?;
        self.governance_breaker.set_level(level)?;
        Ok(())
    }

    fn detect_anomalies_during_recovery(&self) -> bool {
        // Check for fraud patterns, invalid transactions, etc.
        self.core_breaker.get_anomaly_count() > 10 ||
        self.consensus_breaker.get_fork_count() > 0 ||
        self.ubi_breaker.get_fraud_count() > 5
    }
}
```

---

## Monitoring & Alerting

```rust
pub struct CircuitBreakerMonitor {
    propagation_latency_histogram: Histogram,
    false_positive_count: AtomicU64,
    escalation_count: AtomicU64,
}

impl CircuitBreakerMonitor {
    pub fn alert_thresholds(&self) -> Vec<Alert> {
        let mut alerts = Vec::new();

        // Alert: High propagation latency
        let p99_latency_ns = self.propagation_latency_histogram.percentile(0.99);
        if p99_latency_ns > 1_000_000 {
            alerts.push(Alert::Critical {
                component: "CircuitBreakerPropagation",
                metric: "propagation_latency_p99",
                value_ns: p99_latency_ns,
                threshold_ns: 1_000_000, // 1ms
            });
        }

        // Alert: High false positive rate
        let false_positives = self.false_positive_count.load(Ordering::Relaxed);
        let total_escalations = self.escalation_count.load(Ordering::Relaxed);

        if total_escalations > 0 {
            let false_positive_rate = false_positives as f64 / total_escalations as f64;

            if false_positive_rate > 0.05 {
                alerts.push(Alert::Warning {
                    component: "CircuitBreakerPropagation",
                    metric: "false_positive_rate",
                    value_pct: (false_positive_rate * 100.0) as u64,
                    threshold_pct: 5,
                });
            }
        }

        alerts
    }
}
```

---

## Summary: Propagation Patterns

| Pattern | Direction | Trigger | Latency | Use Case |
|---------|-----------|---------|---------|----------|
| **Upward** | Bottom→Top | Core anomaly | <100ns/layer | Invalid signature flood |
| **Downward** | Top→Bottom | Governance fraud | <500ns total | Sybil attack, KYC fraud |
| **Lateral** | Peer↔Peer | Consensus fork | <200ns/peer | Fork resolution, coordination |

### Performance Budget

- **L1/L2 Escalation**: <1ms end-to-end
- **L3 Halt**: <100μs (critical path)
- **Recovery**: 60-90s (gradual with validation)

### Safety Guarantees (ASSUM)

```rust
/// #ASSUME_PROPAGATION_INSTANT: <1ms for full stack cascade
/// #VERIFY_PROPAGATION_TIMING: Benchmark measures actual latency

/// #ASSUME_HALT_EFFECTIVE: L3 blocks all operations
/// #VERIFY_HALT_COVERAGE: Property tests validate no operations during L3

/// #ASSUME_RECOVERY_SAFE: Gradual recovery detects new anomalies
/// #VERIFY_RECOVERY_STABILITY: 30s intervals with anomaly detection

/// #ASSUME_MULTISIG_SECURE: 3-of-5 signatures prevent unauthorized recovery
/// #VERIFY_SIGNATURE_VALIDITY: Ed25519 cryptographic verification
```

---

## Integration with I20 Framework

**I20 Q12 (Failure Cascades)**: Circuit breaker prevents unbounded cascades by halting at layer boundaries.

**I20 Q15 (Escape Hatches)**: Multi-sig manual override provides ultimate escape hatch for false positives.

**I20 Q19 (Integration Strategy)**: Feature flags allow gradual rollout of circuit breaker integration.

**I20 Q20 (Rollback Plan)**: Instant disable via feature flag, gradual recovery via multi-sig.

---

## References

- **ASSUM Safety**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/ASSUM_SAFETY.md`
- **Failure Modes**: `/home/samuel/Primitives/docs/ATOMIC_CAPSULE_FAILURE_MODES.md`
- **I20 Integration**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/I20_INTEGRATION_FRAMEWORK.md`

---

**Circuit Breaker Propagation: Where <1ms protection meets network-wide security.**
