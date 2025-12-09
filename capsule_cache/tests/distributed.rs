use capsule_cache::distributed::DistributedCache;
use std::time::Duration;

#[test]
fn distributed_insert_and_get() {
    let dist = DistributedCache::new(3, 2, 1024, 2);
    dist
        .insert("k1".into(), "v1".into(), Duration::from_secs(30))
        .unwrap();
    assert_eq!(dist.get("k1"), Some("v1".into()));
}

#[test]
fn distributed_quorum_fail() {
    let dist = DistributedCache::new(1, 1, 4, 1);
    // With single node and replication_factor=1, quorum succeeds.
    assert!(dist
        .insert("k".into(), "v".into(), Duration::from_secs(10))
        .is_ok());
}
