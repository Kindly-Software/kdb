//! Integration tests for Kindly Network
//!
//! ## Test Coverage (T28 Framework)
//!
//! - Transaction pool operations (insert, lookup, remove)
//! - Circuit breaker integration (L0-L3 protection)
//! - Gossip protocol (routing, duplicate detection)
//! - Mempool statistics (real-time tracking)
//! - P2P engine (stub validation)

use kindly_network::{
    AtomicTransactionPool, PoolConfig, PoolHealth,
    GossipCapsule, GossipMessage, MessageRoute,
    MempoolStats, P2PEngine, P2PConfig,
};
use kindly_core::{AtomicTransactionCapsule, TransactionData};
use std::sync::Arc;

#[test]
fn test_transaction_pool_basic_ops() {
    let pool = AtomicTransactionPool::new(PoolConfig::default());

    // Create test transaction
    let tx_hash = [1u8; 32];
    let tx_capsule = Arc::new(AtomicTransactionCapsule::new());

    // Insert transaction
    assert!(pool.insert(tx_hash, tx_capsule.clone()).is_ok());
    assert_eq!(pool.len(), 1);

    // Lookup transaction
    assert!(pool.get(&tx_hash).is_some());
    assert!(pool.contains(&tx_hash));

    // Remove transaction
    assert!(pool.remove(&tx_hash).is_some());
    assert_eq!(pool.len(), 0);
    assert!(!pool.contains(&tx_hash));
}

#[test]
fn test_transaction_pool_capacity() {
    let config = PoolConfig {
        max_size: 10,
        ..Default::default()
    };
    let pool = AtomicTransactionPool::new(config);

    // Fill pool to capacity
    for i in 0..10 {
        let mut tx_hash = [0u8; 32];
        tx_hash[0] = i as u8;
        let tx_capsule = Arc::new(AtomicTransactionCapsule::new());
        assert!(pool.insert(tx_hash, tx_capsule).is_ok());
    }

    assert_eq!(pool.len(), 10);

    // Attempt to exceed capacity
    let tx_hash = [99u8; 32];
    let tx_capsule = Arc::new(AtomicTransactionCapsule::new());
    assert!(pool.insert(tx_hash, tx_capsule).is_err());
}

#[test]
fn test_circuit_breaker_levels() {
    let pool = AtomicTransactionPool::new(PoolConfig::default());

    // L0: Normal operation
    assert!(pool.update_breaker_level(0).is_ok());
    let health = pool.health();
    assert_eq!(health.breaker_level, 0);

    // L1: Reduce load
    assert!(pool.update_breaker_level(1).is_ok());
    let health = pool.health();
    assert_eq!(health.breaker_level, 1);

    // L2: Emergency mode
    assert!(pool.update_breaker_level(2).is_ok());
    let health = pool.health();
    assert_eq!(health.breaker_level, 2);

    // L3: Circuit open
    assert!(pool.update_breaker_level(3).is_ok());
    let health = pool.health();
    assert_eq!(health.breaker_level, 3);

    // Invalid level
    assert!(pool.update_breaker_level(4).is_err());
}

#[test]
fn test_circuit_breaker_rejection() {
    let pool = AtomicTransactionPool::new(PoolConfig::default());

    // Set breaker to L2 (should reject new transactions)
    pool.update_breaker_level(2).unwrap();

    let tx_hash = [1u8; 32];
    let tx_capsule = Arc::new(AtomicTransactionCapsule::new());

    // Insert should be rejected
    let result = pool.insert(tx_hash, tx_capsule);
    assert!(result.is_err());

    let health = pool.health();
    assert_eq!(health.total_rejections, 1);
}

#[test]
fn test_gossip_capsule_routing() {
    let capsule = GossipCapsule::new();

    // Create test message
    let msg = GossipMessage {
        msg_hash: [1u8; 32],
        hop_count: 0,
        ttl: 8,
        payload: vec![1, 2, 3],
    };

    // Publish message
    assert!(capsule.publish(&msg).is_ok());

    // Read and check routing decision
    let result = capsule.read();
    assert!(result.is_ok());

    let (route, snapshot) = result.unwrap();
    assert_eq!(route, MessageRoute::Process); // hop_count == 0
    assert_eq!(snapshot.hop_count, 0);
    assert_eq!(snapshot.ttl, 8);
}

#[test]
fn test_gossip_duplicate_detection() {
    let capsule = GossipCapsule::new();

    let msg = GossipMessage {
        msg_hash: [2u8; 32],
        hop_count: 0,
        ttl: 8,
        payload: vec![],
    };

    capsule.publish(&msg).unwrap();
    let generation = capsule.generation();

    // Same generation should be duplicate
    assert!(capsule.is_duplicate(generation));

    // Different generation should not be duplicate
    assert!(!capsule.is_duplicate(generation + 1));
}

#[test]
fn test_gossip_hop_increment() {
    let capsule = GossipCapsule::new();

    let msg = GossipMessage {
        msg_hash: [3u8; 32],
        hop_count: 0,
        ttl: 3,
        payload: vec![],
    };

    capsule.publish(&msg).unwrap();

    // Increment hop count
    assert!(capsule.increment_hop().is_ok());
    assert_eq!(capsule.hop_count(), 1);
    assert_eq!(capsule.ttl(), 2);

    // Continue incrementing until TTL expires
    capsule.increment_hop().unwrap();
    capsule.increment_hop().unwrap();

    assert_eq!(capsule.ttl(), 0);
    assert!(capsule.increment_hop().is_err()); // Should error when TTL is 0
}

#[test]
fn test_gossip_ttl_expiration() {
    let capsule = GossipCapsule::new();

    let msg = GossipMessage {
        msg_hash: [4u8; 32],
        hop_count: 0,
        ttl: 0, // Already expired
        payload: vec![],
    };

    // Publishing with TTL=0 should fail
    assert!(capsule.publish(&msg).is_err());
}

#[test]
fn test_mempool_stats_tracking() {
    let stats = MempoolStats::new();

    // Record some transactions
    stats.record_received();
    stats.record_accepted(100); // 1% fee

    stats.record_received();
    stats.record_accepted(200); // 2% fee

    stats.record_received();
    stats.record_rejected();

    let snapshot = stats.snapshot();

    assert_eq!(snapshot.total_received, 3);
    assert_eq!(snapshot.total_accepted, 2);
    assert_eq!(snapshot.total_rejected, 1);
    assert_eq!(snapshot.pending_count, 2);
    assert_eq!(snapshot.min_fee_bp, 100);
    assert_eq!(snapshot.max_fee_bp, 200);
    assert_eq!(snapshot.avg_fee_bp, 150); // (100 + 200) / 2
}

#[test]
fn test_mempool_stats_confirmation() {
    let stats = MempoolStats::new();

    stats.record_received();
    stats.record_accepted(100);
    assert_eq!(stats.snapshot().pending_count, 1);

    stats.record_confirmed();
    assert_eq!(stats.snapshot().pending_count, 0);
    assert_eq!(stats.snapshot().total_confirmed, 1);
}

#[test]
fn test_mempool_stats_health() {
    let stats = MempoolStats::new();

    stats.record_breaker_trigger();
    stats.record_pool_full();
    stats.record_duplicate();
    stats.record_invalid();

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.breaker_triggers, 1);
    assert_eq!(snapshot.pool_full_events, 1);
    assert_eq!(snapshot.duplicate_attempts, 1);
    assert_eq!(snapshot.invalid_txs, 1);
}

#[tokio::test]
async fn test_p2p_engine_creation() {
    let pool = Arc::new(AtomicTransactionPool::new(PoolConfig::default()));
    let config = P2PConfig::default();
    let engine = P2PEngine::new(pool.clone(), config);

    // Verify engine initialization
    assert_eq!(engine.connected_peers().len(), 0);
    assert_eq!(engine.tx_pool().len(), 0);

    let stats = engine.stats();
    assert_eq!(stats.total_sent, 0);
    assert_eq!(stats.total_received, 0);
}

#[tokio::test]
async fn test_p2p_engine_start_stop() {
    let pool = Arc::new(AtomicTransactionPool::new(PoolConfig::default()));
    let engine = P2PEngine::new(pool, P2PConfig::default());

    // Phase 1: Start/stop should succeed (stub implementation)
    assert!(engine.start().await.is_ok());
    assert!(engine.stop().await.is_ok());
}

#[tokio::test]
async fn test_p2p_broadcast_transaction() {
    let pool = Arc::new(AtomicTransactionPool::new(PoolConfig::default()));
    let engine = P2PEngine::new(pool.clone(), P2PConfig::default());

    let tx_data = TransactionData {
        sender: [1u8; 20],
        recipient: [2u8; 20],
        amount: 1000,
        fee: 10,
        nonce: 1,
        timestamp: 12345,
        tx_hash: [3u8; 32],
    };

    let signature = [0u8; 64];

    // Broadcast should succeed (stub implementation)
    let result = engine.broadcast_transaction(tx_data.clone(), signature).await;
    assert!(result.is_ok());

    // Transaction should be in pool
    assert!(pool.contains(&tx_data.tx_hash));

    let stats = engine.stats();
    assert_eq!(stats.total_sent, 1);
}

#[tokio::test]
async fn test_p2p_gossip_message_handling() {
    let pool = Arc::new(AtomicTransactionPool::new(PoolConfig::default()));
    let engine = P2PEngine::new(pool, P2PConfig::default());

    let msg = GossipMessage {
        msg_hash: [5u8; 32],
        hop_count: 0,
        ttl: 8,
        payload: vec![1, 2, 3],
    };

    let result = engine.handle_gossip_message(msg).await;
    assert!(result.is_ok());

    let route = result.unwrap();
    assert_eq!(route, MessageRoute::Process);

    let stats = engine.stats();
    assert_eq!(stats.total_received, 1);
}

#[test]
fn test_pool_health_snapshot() {
    let pool = AtomicTransactionPool::new(PoolConfig::default());

    // Insert some transactions
    for i in 0..5 {
        let mut tx_hash = [0u8; 32];
        tx_hash[0] = i;
        let tx_capsule = Arc::new(AtomicTransactionCapsule::new());
        pool.insert(tx_hash, tx_capsule).unwrap();
    }

    let health = pool.health();
    assert_eq!(health.pool_size, 5);
    assert_eq!(health.total_inserts, 5);
    assert_eq!(health.breaker_level, 0);
}

#[test]
fn test_pool_clear() {
    let pool = AtomicTransactionPool::new(PoolConfig::default());

    // Insert transactions
    for i in 0..10 {
        let mut tx_hash = [0u8; 32];
        tx_hash[0] = i;
        let tx_capsule = Arc::new(AtomicTransactionCapsule::new());
        pool.insert(tx_hash, tx_capsule).unwrap();
    }

    assert_eq!(pool.len(), 10);

    // Clear pool
    pool.clear();
    assert_eq!(pool.len(), 0);
    assert!(pool.is_empty());
}

#[test]
fn test_concurrent_pool_operations() {
    use std::thread;

    let pool = Arc::new(AtomicTransactionPool::new(PoolConfig {
        max_size: 1000,
        ..Default::default()
    }));

    let mut handles = vec![];

    // Spawn multiple threads to insert transactions
    for thread_id in 0..10 {
        let pool_clone = pool.clone();
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let mut tx_hash = [0u8; 32];
                tx_hash[0] = thread_id;
                tx_hash[1] = i;
                let tx_capsule = Arc::new(AtomicTransactionCapsule::new());
                let _ = pool_clone.insert(tx_hash, tx_capsule);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Pool should have 1000 transactions (all threads succeeded)
    assert_eq!(pool.len(), 1000);
}

#[test]
fn test_stats_reset() {
    let mut stats = MempoolStats::new();

    stats.record_received();
    stats.record_accepted(100);
    assert_eq!(stats.snapshot().total_received, 1);

    stats.reset();
    assert_eq!(stats.snapshot().total_received, 0);
    assert_eq!(stats.snapshot().total_accepted, 0);
}
